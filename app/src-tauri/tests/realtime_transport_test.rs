//! ============================================================================
//! realtime_transport_test.rs — the realtime WebSocket harness encodes outbound commands, decodes inbound frames, recovers from a dropped connection, and halts when reconnection is impossible
//! ============================================================================
//! Category  : Integration (Seam 19 — realtime transport)
//! Component : src/services/realtime/transport/{connection,mod,health}.rs, src/services/realtime/session.rs
//! Binary    : realtime_transport_test
//! Prereqs   : none (in-process WebSocket origin; the ignored scenario needs a live cloud key)
//! Run       : cargo nextest run --test realtime_transport_test --release --test-threads=1
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
    core::{error::PipelineImpact, events::InteractionMode, state::InteractionState},
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

/// PURPOSE:  Every outbound command must be encoded by the provider driver
///           and delivered on the wire; a command that is built but never
///           written leaves the provider silent.
/// SCENARIO: Connect to a local WebSocket origin, spawn the harness and send
///           an activity start plus an audio chunk.
/// ASSERTS:  The origin receives exactly `activity_start` followed by
///           `pcm_samples:320`.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the `OutboundCommand::Audio` arm in `TestDriver::encode` —
///           the second frame assertion would fail.
#[tokio::test]
async fn outbound_commands_are_encoded_and_reach_the_origin() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();

            let msg1 = ws.next().await.unwrap().unwrap();
            assert_eq!(msg1.into_text().unwrap(), "activity_start");

            let msg2 = ws.next().await.unwrap().unwrap();
            assert_eq!(msg2.into_text().unwrap(), "pcm_samples:320");

            let _ = ws.close(None).await;
        });

        let (ws_write, ws_read) = connect_to_ws(server_addr).await;
        let (event_tx, _event_rx) = mpsc::channel(64);
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

        server_task.await.unwrap();
    })
    .await
    .expect("outbound_commands_are_encoded_and_reach_the_origin timed out");
}

/// PURPOSE:  An inbound binary frame must be decoded into the PCM samples
///           the provider sent; a mis-decoded frame plays back as noise.
/// SCENARIO: Have the origin send a binary frame carrying two little-endian
///           i16 samples and read the provider event.
/// ASSERTS:  An `AudioChunk` event arrives whose samples are exactly
///           `[16, 32]`.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Swap the byte order in the binary decoder — the sample
///           assertion would fail.
#[tokio::test]
async fn inbound_binary_frame_is_decoded_into_an_audio_chunk_event() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();

            // Send binary audio chunk back to client
            let audio_bytes = vec![0x10, 0x00, 0x20, 0x00]; // i16 values: [16, 32]
            ws.send(Message::Binary(audio_bytes.into())).await.unwrap();

            let _ = ws.close(None).await;
        });

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

        let _handles = spawn_harness(driver, config, init);

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
    .expect("inbound_binary_frame_is_decoded_into_an_audio_chunk_event timed out");
}

/// PURPOSE:  When the provider says goodbye the harness must reconnect and
///           keep the session usable; a session that dies on a polite
///           `go_away` ends the user's turn for no reason.
/// SCENARIO: The origin accepts one connection, sends `go_away` and closes,
///           then accepts a second connection and reports that it is ready.
/// ASSERTS:  The harness re-establishes the connection, the outbound command
///           reaches the new socket, and the harness is not terminated.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Do not reconnect on `FrameAction::GoAway` — the harness would
///           never reach the second connection and the origin assertion
///           would fail.
#[tokio::test]
async fn reconnect_after_go_away_re_establishes_the_connection() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        // The origin signals "second connection accepted" so the client never
        // has to sleep waiting for the reconnect to settle.
        let (ready_tx, mut ready_rx) = mpsc::channel::<()>(1);

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
            let _ = ready_tx.send(()).await;

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

        // Wait for the origin to confirm the second connection, then talk on it.
        ready_rx
            .recv()
            .await
            .expect("the harness never reconnected to the origin");

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
    .expect("reconnect_after_go_away_re_establishes_the_connection timed out");
}

/// PURPOSE:  When reconnection is impossible the harness must report a
///           session-halting error; silently going quiet leaves the user
///           talking to a dead microphone.
/// SCENARIO: The origin closes immediately and every reconnect attempt fails.
/// ASSERTS:  The emitted error carries `PipelineImpact::SessionHalted`, the
///           current turn id and a "permanently lost" message.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Emit a recoverable impact instead of `SessionHalted` — the
///           impact assertion would fail.
#[tokio::test]
async fn terminal_reconnect_failure_emits_a_session_halted_error() {
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

        assert!(
            handles.terminated.load(Ordering::SeqCst),
            "terminated atomic flag must be true after max reconnect attempts exhausted"
        );
    })
    .await
    .expect("terminal_reconnect_failure_emits_a_session_halted_error timed out");
}

/// PURPOSE:  The harness must give up after exactly the configured number of
///           reconnect attempts; retrying forever keeps the app busy and the
///           user waiting.
/// SCENARIO: Run the same always-failing reconnect scenario and count the
///           attempts the callback was asked to make.
/// ASSERTS:  The callback was invoked exactly `max_reconnect_attempts` times
///           and the harness reports itself terminated.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Ignore `max_reconnect_attempts` and retry forever — the attempt
///           count assertion would fail.
#[tokio::test]
async fn terminal_reconnect_failure_stops_after_the_configured_attempts() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_addr = listener.local_addr().unwrap();

        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let _ = ws.close(None).await;
        });

        let (ws_write, ws_read) = connect_to_ws(server_addr).await;
        let (event_tx, mut event_rx) = mpsc::channel(64);
        let (_state_tx, state_rx) = tokio::sync::watch::channel(InteractionState::Ready);
        let turn_id_ref = Arc::new(AtomicU32::new(42));

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

        // The terminal error is the harness's acknowledgement that it stopped.
        let _ = tokio::time::timeout(Duration::from_secs(5), event_rx.recv())
            .await
            .expect("Must emit terminal event before timeout");

        assert_eq!(
            attempts_counter.load(Ordering::SeqCst),
            2,
            "Must execute exactly max_reconnect_attempts"
        );
        assert!(
            handles.terminated.load(Ordering::SeqCst),
            "the harness must mark itself terminated once the attempts are exhausted"
        );
    })
    .await
    .expect("terminal_reconnect_failure_stops_after_the_configured_attempts timed out");
}

/// PURPOSE:  A session that is paused on purpose must not reconnect; the
///           user paused because the microphone is in use elsewhere, and a
///           reconnect would grab it again.
/// SCENARIO: Pause the session before the origin drops the connection, then
///           watch the reconnect flag.
/// ASSERTS:  The reconnect callback is never invoked and no error event is
///           emitted.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Ignore the `Paused` state when the connection drops — the
///           reconnect-flag assertion would fail.
#[tokio::test]
async fn paused_state_suppresses_the_reconnect() {
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
    .expect("paused_state_suppresses_the_reconnect timed out");
}

/// PURPOSE:  An expired session token must be purged from disk; reusing it
///           makes the provider reject every connection with an
///           authentication error the user cannot act on.
/// SCENARIO: Write a cache file whose expiry is three hours in the past and
///           create the realtime provider.
/// ASSERTS:  The cache file no longer exists afterwards.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Skip the expiry check in `create_realtime_provider` — the file
///           existence assertion would fail.
#[tokio::test]
async fn expired_session_cache_is_purged_on_provider_creation() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;

        let cache_file = cache_dir().join(SESSION_CACHE_FILENAME);
        std::fs::create_dir_all(cache_dir()).unwrap();

        let past_ms = chrono::Utc::now().timestamp_millis() as u64 - (3 * 3600 * 1000);
        let expired_json = serde_json::json!({
            "handle": "expired_session_token_123",
            "model": "gemini-2.0-flash",
            "expires_at": past_ms
        });
        std::fs::write(&cache_file, expired_json.to_string()).unwrap();
        assert!(cache_file.exists(), "Setup: expired cache file must exist");

        {
            let mut settings = state.settings.write().unwrap();
            settings.realtime.active = vox_lib::config::ActiveSelection {
                category: "cloud".to_string(),
                provider: Some("gemini_live".to_string()),
                model: "gemini-2.0-flash".to_string(),
            };
        }

        let _ = create_realtime_provider(&state);

        assert!(
            !cache_file.exists(),
            "Expired session cache file must be purged automatically on create_realtime_provider"
        );
    })
    .await
    .expect("expired_session_cache_is_purged_on_provider_creation timed out");
}

/// PURPOSE:  A valid session token must survive provider creation; purging it
///           forces a new handshake on every start.
/// SCENARIO: Write a cache file whose expiry is one hour in the future and
///           create the realtime provider.
/// ASSERTS:  The cache file still exists afterwards.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Purge the cache file unconditionally — the existence assertion
///           would fail.
#[tokio::test]
async fn unexpired_session_cache_is_retained_on_provider_creation() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;

        let cache_file = cache_dir().join(SESSION_CACHE_FILENAME);
        std::fs::create_dir_all(cache_dir()).unwrap();

        let future_ms = chrono::Utc::now().timestamp_millis() as u64 + (3600 * 1000);
        let valid_json = serde_json::json!({
            "handle": "valid_session_token_456",
            "model": "gemini-2.0-flash",
            "expires_at": future_ms
        });
        std::fs::write(&cache_file, valid_json.to_string()).unwrap();
        assert!(cache_file.exists(), "Setup: valid cache file must exist");

        {
            let mut settings = state.settings.write().unwrap();
            settings.realtime.active = vox_lib::config::ActiveSelection {
                category: "cloud".to_string(),
                provider: Some("gemini_live".to_string()),
                model: "gemini-2.0-flash".to_string(),
            };
        }

        let _ = create_realtime_provider(&state);

        assert!(
            cache_file.exists(),
            "Unexpired session cache file must NOT be purged"
        );
    })
    .await
    .expect("unexpired_session_cache_is_retained_on_provider_creation timed out");
}

/// PURPOSE:  The explicit purge must remove the session cache from disk;
///           a stale token file is a credential left on the user's disk.
/// SCENARIO: Write a valid cache file and call `purge_session_cache`.
/// ASSERTS:  The cache file no longer exists afterwards.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Make `purge_session_cache` a no-op — the existence assertion
///           would fail.
#[tokio::test]
async fn purge_session_cache_removes_the_cache_file() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();

        let cache_file = cache_dir().join(SESSION_CACHE_FILENAME);
        std::fs::create_dir_all(cache_dir()).unwrap();

        let future_ms = chrono::Utc::now().timestamp_millis() as u64 + (3600 * 1000);
        let valid_json = serde_json::json!({
            "handle": "valid_session_token_456",
            "model": "gemini-2.0-flash",
            "expires_at": future_ms
        });
        std::fs::write(&cache_file, valid_json.to_string()).unwrap();
        assert!(cache_file.exists(), "Setup: valid cache file must exist");

        purge_session_cache();

        assert!(
            !cache_file.exists(),
            "purge_session_cache must explicitly remove the cache file"
        );
    })
    .await
    .expect("purge_session_cache_removes_the_cache_file timed out");
}

/// PURPOSE:  A live cloud provider must complete a real handshake and a
///           clean disconnect; the offline transport tests only prove the
///           wiring, not the provider contract.
/// SCENARIO: Instantiate the configured cloud provider, run its health check,
///           connect in PTT mode and disconnect.
/// ASSERTS:  The provider is created, the health check succeeds, the
///           connection is established and the disconnect succeeds.
/// CLASS:    happy-path
/// REQUIRES: a live cloud API key (GEMINI_API_KEY or DEEPGRAM_API_KEY)
/// MUTANT:   Skip the health check in `create_realtime_provider` — the
///           health assertion would fail.
#[tokio::test]
#[ignore = "Requires live cloud API key (GEMINI_API_KEY or DEEPGRAM_API_KEY)"]
async fn live_cloud_realtime_handshake() {
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
    .expect("live_cloud_realtime_handshake timed out");
}
