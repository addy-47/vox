use anyhow::{anyhow, bail, Result};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;

use crate::core::settings::DeepgramVoiceAgentConfig;
use crate::services::realtime::{
    RealtimeAudioConfig, RealtimeProviderKind, RealtimeSession, RealtimeVoiceProvider,
    DEEPGRAM_DEFAULT_WS_URL, DEEPGRAM_HEALTH_CHECK_ADDR, DEFAULT_INPUT_SAMPLE_RATE,
    DEFAULT_OUTPUT_SAMPLE_RATE, LOG_INTERVAL_PACKETS, MAX_RECONNECT_ATTEMPTS,
    RECONNECT_BASE_DELAY_SECS, RECONNECT_FACTOR_SECS, WS_HEALTH_CHECK_TIMEOUT,
    WS_KEEPALIVE_INTERVAL,
};

type WsWriter = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

type WsReader = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

/// Realtime duplex voice provider connecting to the Deepgram Voice Agent WebSocket API.
pub struct DeepgramVoiceAgentProvider {
    config: DeepgramVoiceAgentConfig,
    system_prompt: String,
    state_rx: tokio::sync::watch::Receiver<crate::core::state::InteractionState>,
    turn_id: Arc<std::sync::atomic::AtomicU32>,
    turn_token: Arc<Mutex<tokio_util::sync::CancellationToken>>,
    turn_epoch: Arc<std::sync::atomic::AtomicU64>,
}

impl DeepgramVoiceAgentProvider {
    /// Creates a new DeepgramVoiceAgentProvider instance.
    pub fn new(
        config: DeepgramVoiceAgentConfig,
        system_prompt: String,
        state_rx: tokio::sync::watch::Receiver<crate::core::state::InteractionState>,
        turn_id: Arc<std::sync::atomic::AtomicU32>,
        turn_token: Arc<Mutex<tokio_util::sync::CancellationToken>>,
        turn_epoch: Arc<std::sync::atomic::AtomicU64>,
    ) -> Self {
        Self {
            config,
            system_prompt,
            state_rx,
            turn_id,
            turn_token,
            turn_epoch,
        }
    }
}

impl RealtimeVoiceProvider for DeepgramVoiceAgentProvider {
    /// Returns the DeepgramVoiceAgent provider kind.
    fn kind(&self) -> RealtimeProviderKind {
        RealtimeProviderKind::DeepgramVoiceAgent
    }

    /// Returns audio sampling configuration for Deepgram (16kHz in, 24kHz out).
    fn audio_config(&self) -> RealtimeAudioConfig {
        RealtimeAudioConfig {
            input_sample_rate: DEFAULT_INPUT_SAMPLE_RATE,
            output_sample_rate: DEFAULT_OUTPUT_SAMPLE_RATE,
            requires_input_resampling: false,
            requires_output_resampling: false,
        }
    }

    /// Establishes the WebSocket connection to Deepgram Voice Agent and spawns background streaming tasks.
    fn connect(
        &self,
        interaction_mode: crate::core::settings::InteractionMode,
    ) -> Result<(
        Box<dyn RealtimeSession>,
        tokio::sync::mpsc::Receiver<crate::services::realtime::RealtimeProviderEvent>,
    )> {
        log::debug!(
            "[DeepgramVoiceAgent] Connecting with interaction_mode: {:?}",
            interaction_mode
        );
        let handle = tokio::runtime::Handle::current();

        if self.config.api_key.is_empty() {
            bail!("No API key configured for Deepgram Voice Agent. Please check settings.");
        }

        let api_key = &self.config.api_key;
        let url = std::env::var("DEEPGRAM_AGENT_ENDPOINT_OVERRIDE")
            .unwrap_or_else(|_| DEEPGRAM_DEFAULT_WS_URL.to_string());

        let (mut ws_write, mut ws_read) = tokio::task::block_in_place(|| {
            handle.block_on(async {
                perform_handshake(&url, api_key, &self.config, &self.system_prompt).await
            })
        })?;

        let url_clone = url.clone();
        let api_key_clone = self.config.api_key.clone();
        let config_clone = self.config.clone();
        let system_prompt_clone = self.system_prompt.clone();
        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::channel::<Vec<i16>>(
            crate::services::realtime::BRIDGE_CHANNEL_CAPACITY,
        );
        let (control_tx, mut control_rx) = tokio::sync::mpsc::channel::<ControlEvent>(
            crate::services::realtime::BRIDGE_CHANNEL_CAPACITY,
        );
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let (provider_event_tx, provider_event_rx) =
            tokio::sync::mpsc::channel::<crate::services::realtime::RealtimeProviderEvent>(
                crate::services::realtime::BRIDGE_CHANNEL_CAPACITY,
            );

        let state_rx_clone = self.state_rx.clone();
        let turn_id_reconnect = self.turn_id.clone();
        let ws_connected = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let ws_connected_clone = ws_connected.clone();

        let state = Arc::new(Mutex::new(SessionState {
            last_assistant_text: String::new(),
            turn_id: self.turn_id.clone(),
            turn_token: self.turn_token.clone(),
            turn_epoch: self.turn_epoch.clone(),
            server_turn_cursor: None,
        }));

        let state_clone = state.clone();
        let provider_event_tx_clone = provider_event_tx.clone();

        let ws_sender: Arc<Mutex<Option<UnboundedSender<Message>>>> = Arc::new(Mutex::new(None));
        let ws_sender_audio = ws_sender.clone();
        let ws_sender_control = ws_sender.clone();
        let terminated = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let terminated_clone = terminated.clone();

        let audio_sender_task = handle.spawn(async move {
            let mut packet_count: u64 = 0;
            while let Some(pcm) = audio_rx.recv().await {
                let bytes: Vec<u8> = pcm.iter().flat_map(|&s| s.to_le_bytes()).collect();
                let msg = Message::Binary(bytes.into());

                let opt_tx = {
                    let guard = ws_sender_audio.lock();
                    guard.clone()
                };
                if let Some(tx) = opt_tx {
                    if let Err(e) = tx.send(msg) {
                        log::warn!(
                            "[DeepgramVoiceAgent] Failed to forward audio packet: {:?}",
                            e
                        );
                    }
                }
                packet_count += 1;
                if packet_count.is_multiple_of(LOG_INTERVAL_PACKETS) {
                    log::debug!(
                        "[DeepgramVoiceAgent] Sent {} raw audio blocks to WebSocket.",
                        packet_count
                    );
                }
            }
        });

        let control_sender_task = handle.spawn(async move {
            while let Some(evt) = control_rx.recv().await {
                let msg = match evt {
                    ControlEvent::Interrupt => serde_json::json!({
                        "type": "Clear"
                    })
                    .to_string(),
                };

                let opt_tx = {
                    let guard = ws_sender_control.lock();
                    guard.clone()
                };
                if let Some(tx) = opt_tx {
                    if let Err(e) = tx.send(Message::Text(msg.into())) {
                        log::warn!(
                            "[DeepgramVoiceAgent] Failed to forward control event: {:?}",
                            e
                        );
                    }
                }
            }
        });

        let ws_sender_keepalive = ws_sender.clone();
        let ws_connected_keepalive = ws_connected.clone();
        let keepalive_task = handle.spawn(async move {
            while ws_connected_keepalive.load(Ordering::SeqCst) {
                tokio::time::sleep(WS_KEEPALIVE_INTERVAL).await;
                if !ws_connected_keepalive.load(Ordering::SeqCst) {
                    break;
                }
                let keepalive_msg = serde_json::json!({
                    "type": "KeepAlive"
                })
                .to_string();

                let opt_tx = {
                    let guard = ws_sender_keepalive.lock();
                    guard.clone()
                };
                if let Some(tx) = opt_tx {
                    log::debug!("[DeepgramVoiceAgent] Sending KeepAlive message.");
                    if let Err(e) = tx.send(Message::Text(keepalive_msg.into())) {
                        log::warn!(
                            "[DeepgramVoiceAgent] Failed to send KeepAlive message: {:?}",
                            e
                        );
                    }
                }
            }
        });

        let (ws_write_tx, mut ws_write_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
        *ws_sender.lock() = Some(ws_write_tx);

        let write_task = handle.spawn(async move {
            while let Some(msg) = ws_write_rx.recv().await {
                if let Err(e) = ws_write.send(msg).await {
                    log::error!("[DeepgramVoiceAgent] WebSocket write error: {:?}", e);
                    break;
                }
            }
        });

        let (reconnect_tx, mut reconnect_rx) = tokio::sync::oneshot::channel::<()>();
        let provider_event_tx_recv = provider_event_tx.clone();
        let state_recv = state_clone.clone();
        let state_rx_for_rec = state_rx_clone.clone();
        let ws_connected_clone_for_rec = ws_connected_clone.clone();

        let receiver_task = handle.spawn(async move {
            let state_rx_clone = state_rx_for_rec;
            let ws_connected_clone = ws_connected_clone_for_rec;
            let mut reconnect_tx_opt = Some(reconnect_tx);
            while let Some(res) = ws_read.next().await {
                match res {
                    Ok(Message::Text(text)) => {
                        let text_str: &str = &text;
                        if let Err(e) = handle_deepgram_server_message(
                            text_str,
                            &provider_event_tx_recv,
                            &state_recv,
                        ) {
                            log::error!("[DeepgramVoiceAgent] Message handling error: {:?}", e);
                        }
                    }
                    Ok(Message::Binary(bytes)) => {
                        let pcm: Vec<i16> = bytes
                            .chunks_exact(2)
                            .map(|c| i16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        if let Err(e) = provider_event_tx_recv
                            .send(crate::services::realtime::RealtimeProviderEvent::AudioChunk(pcm))
                            .await
                        {
                            log::warn!("[DeepgramVoiceAgent] Failed to forward AudioChunk: {:?}", e);
                        }
                    }
                    Ok(Message::Close(cf)) => {
                        log::info!("[DeepgramVoiceAgent] WebSocket closed by server: {:?}", cf);
                        break;
                    }
                    Err(e) => {
                        log::error!("[DeepgramVoiceAgent] WebSocket read error: {:?}", e);
                        break;
                    }
                    _ => {}
                }
            }
            ws_connected_clone.store(false, Ordering::SeqCst);
            if *state_rx_clone.borrow() == crate::core::state::InteractionState::Paused {
                log::info!("[DeepgramVoiceAgent] WebSocket disconnected silently during pause.");
            } else if let Some(tx) = reconnect_tx_opt.take() {
                if tx.send(()).is_err() {
                    log::warn!("[DeepgramVoiceAgent] Failed to send reconnect notification (receiver dropped).");
                }
            }
        });

        handle.spawn(async move {
            let mut active_write_task = Some(write_task);
            let mut active_receiver_task = Some(receiver_task);

            'reconnect_loop: loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        log::info!("[DeepgramVoiceAgent] Session shutdown requested. Aborting tasks.");
                        *ws_sender.lock() = None;
                        if let Some(t) = active_write_task.take() { t.abort(); }
                        if let Some(t) = active_receiver_task.take() { t.abort(); }
                        audio_sender_task.abort();
                        control_sender_task.abort();
                        keepalive_task.abort();
                        break 'reconnect_loop;
                    }
                    _ = &mut reconnect_rx => {
                        log::warn!("[DeepgramVoiceAgent] Connection dropped. Cleaning up active connection tasks...");
                        *ws_sender.lock() = None;
                        if let Some(t) = active_write_task.take() { t.abort(); }
                        if let Some(t) = active_receiver_task.take() { t.abort(); }
                    }
                }

                let mut reconnect_attempts = 0;
                let mut reconnected = false;

                while reconnect_attempts < MAX_RECONNECT_ATTEMPTS {
                    log::info!(
                        "[DeepgramVoiceAgent] Reconnecting to Deepgram Voice Agent (attempt {}/{})...",
                        reconnect_attempts + 1,
                        MAX_RECONNECT_ATTEMPTS
                    );

                    let delay_secs = RECONNECT_FACTOR_SECS * (reconnect_attempts as u64) + RECONNECT_BASE_DELAY_SECS;
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;

                    let system_prompt_clone = system_prompt_clone.clone();
                    match perform_handshake(&url_clone, &api_key_clone, &config_clone, &system_prompt_clone).await {
                        Ok((mut new_ws_write, mut new_ws_read)) => {
                            log::info!("[DeepgramVoiceAgent] Reconnection handshake completed successfully!");
                            reconnected = true;
                            ws_connected_clone.store(true, Ordering::SeqCst);

                            let (new_ws_write_tx, mut new_ws_write_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
                            *ws_sender.lock() = Some(new_ws_write_tx);

                            let new_write_task = tokio::spawn(async move {
                                while let Some(msg) = new_ws_write_rx.recv().await {
                                    if let Err(e) = new_ws_write.send(msg).await {
                                        log::error!("[DeepgramVoiceAgent] Reconnected WS write error: {:?}", e);
                                        break;
                                    }
                                }
                            });

                            let (new_reconnect_tx, new_reconnect_rx) = tokio::sync::oneshot::channel::<()>();
                            reconnect_rx = new_reconnect_rx;

                            let new_provider_event_tx = provider_event_tx_clone.clone();
                            let new_state_recv = state_clone.clone();
                            let state_rx_rec = state_rx_clone.clone();
                            let ws_conn_rec = ws_connected_clone.clone();

                            let new_receiver_task = tokio::spawn(async move {
                                let mut reconnect_tx_opt = Some(new_reconnect_tx);
                                while let Some(res) = new_ws_read.next().await {
                                    match res {
                                        Ok(Message::Text(text)) => {
                                            let text_str: &str = &text;
                                            if let Err(e) = handle_deepgram_server_message(text_str, &new_provider_event_tx, &new_state_recv) {
                                                log::error!("[DeepgramVoiceAgent] Reconnected Message handling error: {:?}", e);
                                            }
                                        }
                                        Ok(Message::Binary(bytes)) => {
                                            let pcm: Vec<i16> = bytes
                                                .chunks_exact(2)
                                                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                                                .collect();
                                            if let Err(e) = new_provider_event_tx
                                                .send(crate::services::realtime::RealtimeProviderEvent::AudioChunk(pcm))
                                                .await
                                            {
                                                log::warn!("[DeepgramVoiceAgent] Reconnected AudioChunk send error: {:?}", e);
                                            }
                                        }
                                        Ok(Message::Close(cf)) => {
                                            log::info!("[DeepgramVoiceAgent] Reconnected WebSocket closed by server: {:?}", cf);
                                            break;
                                        }
                                        Err(e) => {
                                            log::error!("[DeepgramVoiceAgent] Reconnected WebSocket read error: {:?}", e);
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                ws_conn_rec.store(false, Ordering::SeqCst);
                                if *state_rx_rec.borrow() == crate::core::state::InteractionState::Paused {
                                    log::info!("[DeepgramVoiceAgent] Reconnected WebSocket disconnected silently during pause.");
                                } else if let Some(tx) = reconnect_tx_opt.take() {
                                    if tx.send(()).is_err() {
                                        log::warn!("[DeepgramVoiceAgent] Failed to send reconnected reconnect signal.");
                                    }
                                }
                            });

                            active_write_task = Some(new_write_task);
                            active_receiver_task = Some(new_receiver_task);
                            break;
                        }
                        Err(e) => {
                            log::error!("[DeepgramVoiceAgent] Reconnection attempt failed: {:?}", e);
                            reconnect_attempts += 1;
                        }
                    }
                }

                if !reconnected {
                    log::error!("[DeepgramVoiceAgent] Max reconnection attempts reached. Terminating session.");
                    let err_turn_id = turn_id_reconnect.load(Ordering::Relaxed);
                    if let Err(e) = provider_event_tx_clone.try_send(crate::services::realtime::RealtimeProviderEvent::Error {
                        turn_id: err_turn_id,
                        message: "Deepgram connection lost permanently after multiple retries.".to_string(),
                    }) {
                        log::warn!("[DeepgramVoiceAgent] Failed to send permanent error event: {:?}", e);
                    }
                    terminated_clone.store(true, Ordering::SeqCst);
                    *ws_sender.lock() = None;
                    audio_sender_task.abort();
                    control_sender_task.abort();
                    break 'reconnect_loop;
                }
            }
        });

        Ok((
            Box::new(DeepgramVoiceAgentSession {
                audio_tx,
                control_tx,
                shutdown_tx: parking_lot::Mutex::new(Some(shutdown_tx)),
                terminated,
            }),
            provider_event_rx,
        ))
    }

    /// Performs a network health check by probing the Deepgram Voice Agent TCP endpoint.
    fn health_check(&self) -> bool {
        use std::net::ToSocketAddrs;
        if let Ok(mut addrs) = DEEPGRAM_HEALTH_CHECK_ADDR.to_socket_addrs() {
            if let Some(addr) = addrs.next() {
                return std::net::TcpStream::connect_timeout(&addr, WS_HEALTH_CHECK_TIMEOUT)
                    .is_ok();
            }
        }
        false
    }
}

/// Connects to Deepgram WebSocket and exchanges the initial Settings JSON handshake.
async fn perform_handshake(
    url: &str,
    api_key: &str,
    config: &DeepgramVoiceAgentConfig,
    system_prompt: &str,
) -> Result<(WsWriter, WsReader)> {
    log::info!(
        "[DeepgramVoiceAgent] Connecting to Deepgram Voice Agent WebSocket: {}",
        url
    );

    let mut request =
        tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(url)?;
    request.headers_mut().insert(
        "Authorization",
        tokio_tungstenite::tungstenite::http::HeaderValue::from_str(&format!("Token {}", api_key))?,
    );

    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| anyhow!("Deepgram WebSocket connection failed: {:?}", e))?;

    let (mut ws_write, mut ws_read) = ws_stream.split();
    log::info!(
        "[DeepgramVoiceAgent] WebSocket connection established. Sending Settings config frame."
    );

    let model = if config.model.is_empty() {
        "gpt-4o-mini"
    } else {
        &config.model
    };
    let (provider_type, model_name) =
        if model.starts_with("gpt-") || model.starts_with("o1") || model.starts_with("o3") {
            ("open_ai", model)
        } else if model.starts_with("claude-") {
            ("anthropic", model)
        } else if model.starts_with("gemini-") {
            ("google", model)
        } else {
            ("open_ai", model)
        };

    let voice_model = match config.voice.as_str() {
        "Aoede" => "aura-asteria-en",
        "Charon" => "aura-orpheus-en",
        "Fenrir" => "aura-zeus-en",
        "Kore" => "aura-stella-en",
        "Puck" => "aura-athena-en",
        other => other,
    };

    let settings_json = serde_json::json!({
        "type": "Settings",
        "audio": {
            "input": {
                "encoding": "linear16",
                "sample_rate": DEFAULT_INPUT_SAMPLE_RATE
            },
            "output": {
                "encoding": "linear16",
                "sample_rate": DEFAULT_OUTPUT_SAMPLE_RATE,
                "container": "none"
            }
        },
        "agent": {
            "listen": {
                "provider": {
                    "type": "deepgram",
                    "model": "flux-general-multi",
                    "version": "v2",
                    "eot_threshold": 0.5,
                    "eager_eot_threshold": 0.4
                }
            },
            "think": {
                "provider": {
                    "type": provider_type,
                    "model": model_name,
                    "temperature": config.temperature
                },
                "prompt": system_prompt
            },
            "speak": {
                "provider": {
                    "type": "deepgram",
                    "model": voice_model
                }
            }
        }
    });

    ws_write
        .send(Message::Text(settings_json.to_string().into()))
        .await
        .map_err(|e| anyhow!("Failed to send Settings JSON frame: {:?}", e))?;

    log::info!(
        "[DeepgramVoiceAgent] Settings frame sent. Waiting for Welcome and SettingsApplied..."
    );

    let mut welcome_received = false;
    let mut settings_applied_received = false;

    let handshake_timeout =
        tokio::time::timeout(crate::services::realtime::WS_HANDSHAKE_TIMEOUT, async {
            while let Some(res) = ws_read.next().await {
                match res {
                    Ok(Message::Text(text)) => {
                        let val: serde_json::Value =
                            serde_json::from_str(&text).unwrap_or_default();
                        if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
                            if msg_type == "Welcome" {
                                log::info!("[DeepgramVoiceAgent] Received Welcome event.");
                                welcome_received = true;
                            } else if msg_type == "SettingsApplied" {
                                log::info!("[DeepgramVoiceAgent] Received SettingsApplied event.");
                                settings_applied_received = true;
                            } else if msg_type == "Error" || msg_type == "Warning" {
                                log::error!(
                                "[DeepgramVoiceAgent] Server error/warning during handshake: {:?}",
                                val
                            );
                                return Err(anyhow!("Deepgram error during handshake: {:?}", val));
                            }

                            if welcome_received && settings_applied_received {
                                return Ok(());
                            }
                        }
                    }
                    Ok(msg) => {
                        log::debug!(
                            "[DeepgramVoiceAgent] Received non-text message during handshake: {:?}",
                            msg
                        );
                    }
                    Err(e) => {
                        return Err(anyhow!("WebSocket error during handshake: {:?}", e));
                    }
                }
            }
            Err(anyhow!(
                "WebSocket stream terminated before handshake complete"
            ))
        })
        .await;

    match handshake_timeout {
        Ok(Ok(())) => {
            log::info!("[DeepgramVoiceAgent] Handshake completed successfully.");
            Ok((ws_write, ws_read))
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(anyhow!(
            "Handshake timed out after {} seconds",
            crate::services::realtime::WS_HANDSHAKE_TIMEOUT.as_secs()
        )),
    }
}

enum ControlEvent {
    Interrupt,
}

struct SessionState {
    last_assistant_text: String,
    turn_id: Arc<std::sync::atomic::AtomicU32>,
    turn_token: Arc<Mutex<tokio_util::sync::CancellationToken>>,
    turn_epoch: Arc<std::sync::atomic::AtomicU64>,
    /// Session-bound cursor tracking asynchronous server-side response turns from Deepgram.
    server_turn_cursor: Option<u32>,
}

impl SessionState {
    fn current_or_new_turn_id(&mut self) -> u32 {
        if let Some(id) = self.server_turn_cursor {
            id
        } else {
            self.turn_epoch.fetch_add(1, Ordering::Relaxed);
            {
                let mut guard = self.turn_token.lock();
                guard.cancel();
                *guard = tokio_util::sync::CancellationToken::new();
            }
            let new_id = self.turn_id.fetch_add(1, Ordering::Relaxed) + 1;
            self.server_turn_cursor = Some(new_id);
            new_id
        }
    }

    fn peek_or_current_turn_id(&self) -> u32 {
        self.server_turn_cursor
            .unwrap_or_else(|| self.turn_id.load(Ordering::Relaxed))
    }
}

/// Active duplex session interacting with Deepgram Voice Agent via background channels.
pub struct DeepgramVoiceAgentSession {
    audio_tx: tokio::sync::mpsc::Sender<Vec<i16>>,
    control_tx: tokio::sync::mpsc::Sender<ControlEvent>,
    shutdown_tx: parking_lot::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    terminated: Arc<std::sync::atomic::AtomicBool>,
}

impl RealtimeSession for DeepgramVoiceAgentSession {
    /// Enqueues PCM audio chunk for transmission to Deepgram.
    fn send_audio(&self, pcm: &[i16]) -> Result<()> {
        if self.terminated.load(Ordering::Relaxed) {
            bail!("Deepgram Voice Agent session is terminated");
        }
        if let Err(tokio::sync::mpsc::error::TrySendError::Full(_)) =
            self.audio_tx.try_send(pcm.to_vec())
        {
            log::warn!("[DeepgramVoiceAgent] Audio queue full — dropped frame");
        }
        Ok(())
    }

    /// Commits an atomic speech turn by enqueuing the audio buffer to Deepgram.
    fn commit_speech_turn(&self, pcm: &[i16]) -> Result<()> {
        self.send_audio(pcm)
    }

    /// Sends interrupt cancellation event to Deepgram.
    fn cancel(&self) -> Result<()> {
        if self.terminated.load(Ordering::Relaxed) {
            bail!("Deepgram Voice Agent session is terminated");
        }
        self.control_tx
            .try_send(ControlEvent::Interrupt)
            .map_err(|e| anyhow!("Failed to send interrupt control event: {:?}", e))
    }

    /// Terminates the active session and signals background worker tasks to shutdown.
    fn disconnect(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.lock().take() {
            if let Err(e) = tx.send(()) {
                log::warn!("[DeepgramVoiceAgent] Shutdown signal drop: {:?}", e);
            }
        }
        Ok(())
    }
}

/// Parses and routes incoming Deepgram Voice Agent JSON protocol messages.
fn handle_deepgram_server_message(
    text: &str,
    provider_event_tx: &tokio::sync::mpsc::Sender<crate::services::realtime::RealtimeProviderEvent>,
    state: &Arc<Mutex<SessionState>>,
) -> Result<()> {
    let val: serde_json::Value = serde_json::from_str(text)?;

    if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
        log::trace!("[DeepgramVoiceAgent] Inbound message type: {}", msg_type);
        match msg_type {
            "UserStartedSpeaking" => {
                log::info!("[DeepgramVoiceAgent] User started speaking (barge-in).");
                let mut s_lock = state.lock();
                s_lock.last_assistant_text.clear();
                let tid = s_lock.peek_or_current_turn_id();
                s_lock.server_turn_cursor = None;
                if let Err(e) = provider_event_tx.try_send(
                    crate::services::realtime::RealtimeProviderEvent::Interrupted { turn_id: tid },
                ) {
                    log::warn!(
                        "[DeepgramVoiceAgent] Failed to forward Interrupted event: {:?}",
                        e
                    );
                }
            }
            // NOTE: Function calling / tool roundtrip hook for future expansion.
            // When Deepgram sends FunctionCallRequest with {id, name, arguments}, execute the client tool
            // and reply with FunctionCallResponse frame {"type": "FunctionCallResponse", "id": id, "output": result}.
            "FunctionCallRequest" => {
                log::debug!("[DeepgramVoiceAgent] Received FunctionCallRequest frame (client-side execution hook reserved): {:?}", val);
            }
            "ConversationText" => {
                let role = val.get("role").and_then(|v| v.as_str()).unwrap_or("");
                let content = val.get("content").and_then(|v| v.as_str()).unwrap_or("");
                log::trace!(
                    "[DeepgramVoiceAgent] ConversationText role={}: {:?}",
                    role,
                    content
                );

                if role == "user" {
                    log::debug!("[DeepgramVoiceAgent] User final transcript: {:?}", content);
                    let turn_id = state.lock().current_or_new_turn_id();
                    if let Err(e) = provider_event_tx.try_send(
                        crate::services::realtime::RealtimeProviderEvent::TranscriptFinal {
                            turn_id,
                            text: content.to_string(),
                        },
                    ) {
                        log::warn!(
                            "[DeepgramVoiceAgent] Failed to forward TranscriptFinal event: {:?}",
                            e
                        );
                    }
                } else if role == "assistant" {
                    log::debug!("[DeepgramVoiceAgent] Assistant transcript: {:?}", content);
                    let mut s_lock = state.lock();
                    let turn_id = s_lock.current_or_new_turn_id();
                    let last_text = &s_lock.last_assistant_text;
                    if content.starts_with(last_text) {
                        let delta = &content[last_text.len()..];
                        if !delta.is_empty() {
                            if let Err(e) = provider_event_tx.try_send(
                                crate::services::realtime::RealtimeProviderEvent::LlmToken {
                                    turn_id,
                                    token: delta.to_string(),
                                },
                            ) {
                                log::warn!(
                                    "[DeepgramVoiceAgent] Failed to forward LlmToken event: {:?}",
                                    e
                                );
                            }
                        }
                    } else {
                        if let Err(e) = provider_event_tx.try_send(
                            crate::services::realtime::RealtimeProviderEvent::LlmToken {
                                turn_id,
                                token: content.to_string(),
                            },
                        ) {
                            log::warn!(
                                "[DeepgramVoiceAgent] Failed to forward LlmToken event: {:?}",
                                e
                            );
                        }
                    }
                    s_lock.last_assistant_text = content.to_string();
                }
            }
            "AgentAudioDone" => {
                log::debug!("[DeepgramVoiceAgent] Agent audio done.");
                let mut s_lock = state.lock();
                s_lock.last_assistant_text.clear();
                let finished_turn_id = s_lock
                    .server_turn_cursor
                    .take()
                    .unwrap_or_else(|| s_lock.turn_id.load(Ordering::Relaxed));
                if let Err(e) = provider_event_tx.try_send(
                    crate::services::realtime::RealtimeProviderEvent::LlmFinished {
                        turn_id: finished_turn_id,
                    },
                ) {
                    log::warn!(
                        "[DeepgramVoiceAgent] Failed to forward LlmFinished event: {:?}",
                        e
                    );
                }
            }
            "Error" | "Warning" => {
                log::error!("[DeepgramVoiceAgent] Server error/warning: {:?}", val);
                if let Some(err_msg) = val.get("message").and_then(|v| v.as_str()) {
                    let err_turn_id = state.lock().peek_or_current_turn_id();
                    if let Err(e) = provider_event_tx.try_send(
                        crate::services::realtime::RealtimeProviderEvent::Error {
                            turn_id: err_turn_id,
                            message: format!("Deepgram server error: {}", err_msg),
                        },
                    ) {
                        log::warn!(
                            "[DeepgramVoiceAgent] Failed to forward Error event: {:?}",
                            e
                        );
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}
