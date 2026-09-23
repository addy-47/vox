//! ============================================================================
//! tests/realtime_transport_test.rs — Realtime S2S WebSocket Driver Transport Lifecycle Tests
//! ============================================================================
//! Category     : Integration Test (Seam 19)
//! Component    : services/realtime/transport/{connection,mod,health}, services/realtime/session
//! Prerequisites: Local in-process WebSocket server, isolated TempPathsGuard
//! Execution    : cargo nextest run --test realtime_transport_test --release --nocapture --test-threads=1
//!                (Cloud live test): cargo nextest run --test realtime_transport_test --release --nocapture --test-threads=1 -- --ignored
//! Metrics      : Wire framing encoding/decoding, reconnect backoff recovery, terminal failure halt,
//!                paused state suppression, session cache TTL purge
//! ============================================================================

mod common;

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use common::{harness::get_test_app_and_state, paths::TempPathsGuard};
use futures_util::{SinkExt, StreamExt};
use tokio::{net::TcpListener, sync::mpsc};
use tokio_tungstenite::tungstenite::Message;
use vox_lib::{
    core::{
        error::PipelineImpact,
        settings::{InteractionMode, RealtimeProviderKind},
        state::InteractionState,
    },
    services::realtime::{
        create_realtime_provider, purge_session_cache, spawn_harness, FrameAction, HarnessConfig,
        HarnessInit, OutboundCommand, ProviderDriver, RealtimeProviderEvent, ReconnectFn, WsReader,
        WsWriter, SESSION_CACHE_FILENAME,
    },
    utils::paths::cache_dir,
};

// ============================================================================
// Test Driver Implementation
// ============================================================================

struct TestDriver {
    keepalive: Option<Duration>,
}

impl TestDriver {
    fn new() -> Self {
        Self { keepalive: None }
    }
}

impl ProviderDriver for TestDriver {
    fn encode(&self, cmd: OutboundCommand) -> Option<Message> {
        match cmd {
            OutboundCommand::Audio(pcm) => {
                Some(Message::Text(format!("pcm_samples:{}", pcm.len()).into()))
            }
            OutboundCommand::ActivityStart => Some(Message::Text("activity_start".into())),
            OutboundCommand::ActivityEnd => Some(Message::Text("activity_end".into())),
            OutboundCommand::Interrupt => Some(Message::Text("interrupt".into())),
            OutboundCommand::KeepAlive => Some(Message::Text("ping".into())),
            OutboundCommand::ToolResponse { id, name, result } => Some(Message::Text(
                format!("tool_response:{id}:{name}:{result}").into(),
            )),
            OutboundCommand::Text(text) => Some(Message::Text(format!("text:{text}").into())),
        }
    }

    fn handle_frame(
        &self,
        msg: Message,
        event_tx: &mpsc::Sender<RealtimeProviderEvent>,
    ) -> FrameAction {
        match msg {
            Message::Text(t) => {
                if t == "go_away" {
                    FrameAction::GoAway
                } else {
                    let _ = event_tx.try_send(RealtimeProviderEvent::TranscriptFinal {
                        turn_id: 1,
                        text: t.to_string(),
                    });
                    FrameAction::Continue
                }
            }
            Message::Binary(bytes) => {
                let pcm: Vec<i16> = bytes
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();
                let _ = event_tx.try_send(RealtimeProviderEvent::AudioChunk(pcm));
                FrameAction::Continue
            }
            _ => FrameAction::Continue,
        }
    }

    fn keepalive_interval(&self) -> Option<Duration> {
        self.keepalive
    }
}

// ============================================================================
// Helper: Connect to local WS endpoint
// ============================================================================

async fn connect_to_ws(addr: SocketAddr) -> (WsWriter, WsReader) {
    let url = format!("ws://{}/realtime", addr);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("Failed to connect to local mock WS server");
    ws_stream.split()
}

/// Single home for the `HarnessInit` block previously duplicated across all
/// four subtests (identical shape, only the channel handles differ).
#[allow(clippy::too_many_arguments)]
fn make_harness_init(
    ws_write: WsWriter,
    ws_read: WsReader,
    reconnect_fn: ReconnectFn,
    provider_event_tx: mpsc::Sender<RealtimeProviderEvent>,
    state_rx: tokio::sync::watch::Receiver<InteractionState>,
    turn_id_ref: Arc<AtomicU32>,
) -> HarnessInit {
    HarnessInit {
        ws_write,
        ws_read,
        reconnect_fn,
        provider_event_tx,
        state_rx,
        turn_id_ref,
        tokio_handle: tokio::runtime::Handle::current(),
    }
}

// ============================================================================
// Subtest 1: Duplex Wire Framing & Outbound Command Encoding
// ============================================================================
/// Entry Seam A: `services::realtime::transport::connection::spawn_harness`
///
/// Verifies:
///   - Outbound commands (`Audio`, `ActivityStart`) are encoded via `ProviderDriver`
///     and successfully received across the WebSocket wire by the server.
///   - Inbound binary frames from the server are decoded by `ProviderDriver` into
///     `RealtimeProviderEvent::AudioChunk` events on `provider_event_tx`.
#[tokio::test]
async fn test_duplex_wire_framing_and_outbound_encoding() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // 1. Spawn Mock Server
        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();

            // Receive outbound commands from client
            let msg1 = ws.next().await.unwrap().unwrap();
            assert_eq!(msg1.into_text().unwrap(), "activity_start");

            let msg2 = ws.next().await.unwrap().unwrap();
            assert_eq!(msg2.into_text().unwrap(), "pcm_samples:320");

            // Send binary audio chunk back to client
            let audio_bytes = vec![0x10, 0x00, 0x20, 0x00]; // i16 values: [16, 32]
            ws.send(Message::Binary(audio_bytes.into())).await.unwrap();

            // Wait briefly before closing
            tokio::time::sleep(Duration::from_millis(100)).await;
            let _ = ws.close(None).await;
        });

        // 2. Client Connection & Harness Setup
        let (ws_write, ws_read) = connect_to_ws(server_addr).await;
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let (_state_tx, state_rx) = tokio::sync::watch::channel(InteractionState::Ready);
        let turn_id_ref = Arc::new(AtomicU32::new(1));

        let reconnect_fn: ReconnectFn = Box::new(move || {
            Box::pin(async move {
                anyhow::bail!("Reconnect not expected in wire framing test");
            })
        });

        let driver = Arc::new(TestDriver::new());
        let config = HarnessConfig {
            max_reconnect_attempts: 1,
            reconnect_base_delay_secs: 0,
            reconnect_factor_secs: 0,
        };

        let init = make_harness_init(
            ws_write,
            ws_read,
            reconnect_fn,
            event_tx,
            state_rx,
            turn_id_ref,
        );

        let handles = spawn_harness(driver, config, init);

        // 3. Dispatch outbound commands
        handles
            .outbound_tx
            .send(OutboundCommand::ActivityStart)
            .await
            .unwrap();
        handles
            .outbound_tx
            .send(OutboundCommand::Audio(vec![0; 320]))
            .await
            .unwrap();

        // 4. Await inbound audio chunk
        let event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
            .await
            .expect("Must receive inbound event within timeout")
            .expect("Channel must remain open");

        match event {
            RealtimeProviderEvent::AudioChunk(pcm) => {
                assert_eq!(
                    pcm,
                    vec![16, 32],
                    "Inbound audio chunk PCM must match wire bytes"
                );
            }
            other => panic!("Expected AudioChunk event, got: {:?}", other),
        }

        server_task.await.unwrap();
    })
    .await
    .expect("test_duplex_wire_framing_and_outbound_encoding timed out");
}

// ============================================================================
// Subtest 2: Reconnect Recovery
// ============================================================================
/// Entry Seam A: `services::realtime::transport::connection::spawn_harness`
///
/// Verifies:
///   - When the server abruptly closes the connection, the harness triggers the reconnect
///     callback and re-establishes a live connection.
///   - Outbound commands transmitted during/after reconnect reach the new server socket.
#[tokio::test]
async fn test_reconnect_backoff_and_recovery() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // Server accepts connection 1, sends go_away, then accepts connection 2
        let server_task = tokio::spawn(async move {
            // Connection 1
            let (stream1, _) = listener.accept().await.unwrap();
            let mut ws1 = tokio_tungstenite::accept_async(stream1).await.unwrap();
            // Send go_away to trigger graceful reconnect
            ws1.send(Message::Text("go_away".into())).await.unwrap();
            let _ = ws1.close(None).await;

            // Connection 2 (Reconnection)
            let (stream2, _) = listener.accept().await.unwrap();
            let mut ws2 = tokio_tungstenite::accept_async(stream2).await.unwrap();

            // Receive message from reconnected client
            let msg = ws2.next().await.unwrap().unwrap();
            assert_eq!(msg.into_text().unwrap(), "activity_start");

            let _ = ws2.close(None).await;
        });

        let (ws_write, ws_read) = connect_to_ws(server_addr).await;
        let (event_tx, _event_rx) = mpsc::channel(64);
        let (_state_tx, state_rx) = tokio::sync::watch::channel(InteractionState::Ready);
        let turn_id_ref = Arc::new(AtomicU32::new(1));

        let reconnect_addr = server_addr;
        let reconnect_fn: ReconnectFn = Box::new(move || {
            Box::pin(async move {
                let (w, r) = connect_to_ws(reconnect_addr).await;
                Ok((w, r))
            })
        });

        let driver = Arc::new(TestDriver::new());
        let config = HarnessConfig {
            max_reconnect_attempts: 2,
            reconnect_base_delay_secs: 0,
            reconnect_factor_secs: 0,
        };

        let init = make_harness_init(
            ws_write,
            ws_read,
            reconnect_fn,
            event_tx,
            state_rx,
            turn_id_ref,
        );

        let handles = spawn_harness(driver, config, init);

        // Wait briefly for go_away and reconnect to settle
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Dispatch outbound command over reconnected stream
        handles
            .outbound_tx
            .send(OutboundCommand::ActivityStart)
            .await
            .unwrap();

        server_task.await.unwrap();
        assert!(
            !handles.terminated.load(Ordering::SeqCst),
            "Harness must NOT be terminated after successful reconnect"
        );
    })
    .await
    .expect("test_reconnect_backoff_and_recovery timed out");
}

// ============================================================================
// Subtest 3: Terminal Reconnect Failure & Session Halted Impact
// ============================================================================
/// Entry Seam A: `services::realtime::transport::connection::spawn_harness`
///
/// Verifies:
///   - When reconnect attempts reach `max_reconnect_attempts` without success:
///     - `terminated` atomic flag is set to `true`.
///     - `RealtimeProviderEvent::Error` with `PipelineImpact::SessionHalted` is emitted.
#[tokio::test]
async fn test_terminal_reconnect_failure_and_halt() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            // Close immediately to force reconnect
            let _ = ws.close(None).await;
        });

        let (ws_write, ws_read) = connect_to_ws(server_addr).await;
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let (_state_tx, state_rx) = tokio::sync::watch::channel(InteractionState::Ready);
        let turn_id_ref = Arc::new(AtomicU32::new(42));

        // Reconnect callback always fails
        let attempts_counter = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts_counter.clone();
        let reconnect_fn: ReconnectFn = Box::new(move || {
            let c = attempts_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                anyhow::bail!("Simulated network outage");
            })
        });

        let driver = Arc::new(TestDriver::new());
        let config = HarnessConfig {
            max_reconnect_attempts: 2,
            reconnect_base_delay_secs: 0,
            reconnect_factor_secs: 0,
        };

        let init = make_harness_init(
            ws_write,
            ws_read,
            reconnect_fn,
            event_tx,
            state_rx,
            turn_id_ref,
        );

        let handles = spawn_harness(driver, config, init);

        server_task.await.unwrap();

        // Await terminal error event
        let error_event = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
            .await
            .expect("Must emit terminal event before timeout")
            .expect("Channel must yield event");

        match error_event {
            RealtimeProviderEvent::Error {
                impact,
                turn_id,
                message,
            } => {
                assert_eq!(
                    impact,
                    PipelineImpact::SessionHalted,
                    "Terminal reconnect failure must emit PipelineImpact::SessionHalted"
                );
                assert_eq!(turn_id, 42);
                assert!(message.contains("permanently lost"));
            }
            other => panic!("Expected Error event, got: {:?}", other),
        }

        // Verify terminated flag was set
        assert!(
            handles.terminated.load(Ordering::SeqCst),
            "terminated atomic flag must be true after max reconnect attempts exhausted"
        );
        assert_eq!(
            attempts_counter.load(Ordering::SeqCst),
            2,
            "Must execute exactly max_reconnect_attempts"
        );
    })
    .await
    .expect("test_terminal_reconnect_failure_and_halt timed out");
}

// ============================================================================
// Subtest 4: Paused State Suppresses Reconnect
// ============================================================================
/// Entry Seam A: `services::realtime::transport::connection::spawn_harness`
///
/// Verifies:
///   - If connection drops while `InteractionState == Paused`, the harness silently
///     terminates the connection tasks without triggering the reconnect callback.
#[tokio::test]
async fn test_paused_state_suppresses_reconnect() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = ws.close(None).await;
        });

        let (ws_write, ws_read) = connect_to_ws(server_addr).await;
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let (state_tx, state_rx) = tokio::sync::watch::channel(InteractionState::Ready);
        let turn_id_ref = Arc::new(AtomicU32::new(1));

        let reconnect_called = Arc::new(AtomicBool::new(false));
        let reconnect_called_clone = reconnect_called.clone();
        let reconnect_fn: ReconnectFn = Box::new(move || {
            let flag = reconnect_called_clone.clone();
            Box::pin(async move {
                flag.store(true, Ordering::SeqCst);
                anyhow::bail!("Should not be called");
            })
        });

        let driver = Arc::new(TestDriver::new());
        let config = HarnessConfig {
            max_reconnect_attempts: 2,
            reconnect_base_delay_secs: 0,
            reconnect_factor_secs: 0,
        };

        let init = make_harness_init(
            ws_write,
            ws_read,
            reconnect_fn,
            event_tx,
            state_rx,
            turn_id_ref,
        );

        let _handles = spawn_harness(driver, config, init);

        // Transition state to Paused BEFORE socket closes
        state_tx.send(InteractionState::Paused).unwrap();

        server_task.await.unwrap();

        // Suppression invariant: watch a full 500ms window (not a single 200ms
        // sample) so a late reconnect can never slip past the assertion.
        // The flag persists once set, so the helper fails fast on any firing.
        common::harness::assert_flag_remains_false(
            &reconnect_called,
            Duration::from_millis(500),
            "Paused-state reconnect suppression",
        )
        .await;

        // Also verify no terminal error was emitted (channel retains events,
        // so an endpoint check after the watch window is exhaustive)
        assert!(
            event_rx.try_recv().is_err(),
            "No error events should be emitted during silent paused disconnect"
        );
    })
    .await
    .expect("test_paused_state_suppresses_reconnect timed out");
}

// ============================================================================
// Subtest 5: Session Cache TTL Management & Disk Purge
// ============================================================================
/// Entry Seam B: `create_realtime_provider` & `purge_session_cache`
///
/// Verifies:
///   - Expired session tokens (> 2 hours) on disk are purged by `create_realtime_provider`.
///   - Unexpired tokens are retained and injected into provider configuration.
#[tokio::test]
async fn test_session_cache_ttl_and_purge() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;

        let cache_file = cache_dir().join(SESSION_CACHE_FILENAME);
        std::fs::create_dir_all(cache_dir()).unwrap();

        // 1. Expired Token: timestamp 3 hours in the past
        let past_ms = chrono::Utc::now().timestamp_millis() as u64 - (3 * 3600 * 1000);
        let expired_json = serde_json::json!({
            "handle": "expired_session_token_123",
            "model": "gemini-2.0-flash",
            "expires_at": past_ms
        });
        std::fs::write(&cache_file, expired_json.to_string()).unwrap();
        assert!(cache_file.exists(), "Setup: expired cache file must exist");

        // Trigger create_realtime_provider
        {
            let mut settings = state.settings.write().unwrap();
            settings.realtime.active = RealtimeProviderKind::GeminiLive;
        }

        let _ = create_realtime_provider(&state);

        // Verification: Expired cache file must be purged from disk
        assert!(
            !cache_file.exists(),
            "Expired session cache file must be purged automatically on create_realtime_provider"
        );

        // 2. Unexpired Token: timestamp 1 hour in the future
        let future_ms = chrono::Utc::now().timestamp_millis() as u64 + (3600 * 1000);
        let valid_json = serde_json::json!({
            "handle": "valid_session_token_456",
            "model": "gemini-2.0-flash",
            "expires_at": future_ms
        });
        std::fs::write(&cache_file, valid_json.to_string()).unwrap();
        assert!(cache_file.exists(), "Setup: valid cache file must exist");

        let _ = create_realtime_provider(&state);

        // Verification: Valid unexpired cache file must be retained
        assert!(
            cache_file.exists(),
            "Unexpired session cache file must NOT be purged"
        );

        // Explicit purge test
        purge_session_cache();
        assert!(
            !cache_file.exists(),
            "purge_session_cache must explicitly remove the cache file"
        );
    })
    .await
    .expect("test_session_cache_ttl_and_purge timed out");
}

// ============================================================================
// Subtest 6 (IGNORED): Live Cloud Provider Handshake
// ============================================================================
/// Cloud E2E test verifying live connection against configured cloud provider.
#[tokio::test]
#[ignore = "Requires live cloud API key (GEMINI_API_KEY or DEEPGRAM_API_KEY)"]
async fn test_live_cloud_realtime_handshake() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;

        let provider = create_realtime_provider(&state).expect("Must instantiate provider");
        let connected = provider.health_check();
        assert!(connected, "Live provider health check must succeed");

        let (session, _rx) = provider
            .connect(InteractionMode::PTT, &tokio::runtime::Handle::current())
            .expect("Must connect to live provider");

        session.disconnect().expect("Clean disconnect must succeed");
    })
    .await
    .expect("test_live_cloud_realtime_handshake timed out");
}
