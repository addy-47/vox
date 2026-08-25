use anyhow::{anyhow, bail, Result};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;

use crate::core::events::VoxEvent;
use crate::core::settings::DeepgramVoiceAgentConfig;
use crate::services::realtime::{
    RealtimeAudioConfig, RealtimeProviderKind, RealtimeSession, RealtimeVoiceProvider,
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
    is_paused: Arc<std::sync::atomic::AtomicBool>,
}

impl DeepgramVoiceAgentProvider {
    /// Creates a new DeepgramVoiceAgentProvider instance.
    pub fn new(
        config: DeepgramVoiceAgentConfig,
        system_prompt: String,
        is_paused: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            config,
            system_prompt,
            is_paused,
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
            input_sample_rate: 16000,
            output_sample_rate: 24000,
            requires_input_resampling: false,
            requires_output_resampling: false,
        }
    }

    /// Establishes the WebSocket connection to Deepgram Voice Agent and spawns background streaming tasks.
    fn connect(
        &self,
        _interaction_mode: crate::core::settings::InteractionMode,
        playback_tx: tokio::sync::mpsc::Sender<Vec<i16>>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<Box<dyn RealtimeSession>> {
        let handle = tokio::runtime::Handle::current();

        if self.config.api_key.is_empty() {
            bail!("No API key configured for Deepgram Voice Agent. Please check settings.");
        }

        let api_key = &self.config.api_key;
        let url = std::env::var("DEEPGRAM_AGENT_ENDPOINT_OVERRIDE")
            .unwrap_or_else(|_| "wss://agent.deepgram.com/v1/agent/converse".to_string());

        // Perform initial connection and setup handshake synchronously
        let (mut ws_write, mut ws_read) = tokio::task::block_in_place(|| {
            handle.block_on(async {
                perform_handshake(&url, api_key, &self.config, &self.system_prompt).await
            })
        })?;

        let url_clone = url.clone();
        let api_key_clone = self.config.api_key.clone();
        let config_clone = self.config.clone();
        let system_prompt_clone = self.system_prompt.clone();
        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<i16>>();
        let (control_tx, mut control_rx) = tokio::sync::mpsc::unbounded_channel::<ControlEvent>();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let is_paused_clone = self.is_paused.clone();
        let ws_connected = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let ws_connected_clone = ws_connected.clone();
        let last_activity_time = Arc::new(std::sync::atomic::AtomicU64::new(
            chrono::Utc::now().timestamp_millis() as u64,
        ));
        let last_activity_time_sender = last_activity_time.clone();

        let state = Arc::new(Mutex::new(SessionState {
            last_assistant_text: String::new(),
            last_activity_time: last_activity_time.clone(),
        }));

        let state_clone = state.clone();
        let playback_tx_clone = playback_tx.clone();
        let event_tx_clone = event_tx.clone();

        let ws_sender: Arc<Mutex<Option<UnboundedSender<Message>>>> = Arc::new(Mutex::new(None));
        let ws_sender_audio = ws_sender.clone();
        let ws_sender_control = ws_sender.clone();

        // Spawn persistent Audio Sender Task
        let audio_sender_task = handle.spawn(async move {
            let mut packet_count = 0;
            while let Some(pcm) = audio_rx.recv().await {
                let bytes: Vec<u8> = pcm.iter().flat_map(|&s| s.to_le_bytes()).collect();
                let msg = Message::Binary(bytes.into());

                let opt_tx = {
                    let guard = ws_sender_audio.lock();
                    guard.clone()
                };
                if let Some(tx) = opt_tx {
                    if let Err(e) = tx.send(msg) {
                        log::warn!("[DeepgramVoiceAgent] Failed to forward audio packet: {:?}", e);
                    }
                }
                packet_count += 1;
                if packet_count % 100 == 0 {
                    log::debug!(
                        "[DeepgramVoiceAgent] Sent {} raw audio blocks to WebSocket.",
                        packet_count
                    );
                }
            }
        });

        // Spawn persistent Control Sender Task
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
                        log::warn!("[DeepgramVoiceAgent] Failed to forward control event: {:?}", e);
                    }
                }
            }
        });

        // Spawn periodic KeepAlive Task to prevent CLIENT_MESSAGE_TIMEOUT from Deepgram
        let ws_sender_keepalive = ws_sender.clone();
        let ws_connected_keepalive = ws_connected.clone();
        let keepalive_task = handle.spawn(async move {
            while ws_connected_keepalive.load(Ordering::SeqCst) {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
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
                        log::warn!("[DeepgramVoiceAgent] Failed to send KeepAlive message: {:?}", e);
                    }
                }
            }
        });

        // Set up the first active connection
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
        let playback_tx_recv = playback_tx.clone();
        let event_tx_recv = event_tx.clone();
        let state_recv = state_clone.clone();
        let is_paused_clone_for_rec = is_paused_clone.clone();
        let ws_connected_clone_for_rec = ws_connected_clone.clone();

        let receiver_task = handle.spawn(async move {
            let is_paused_clone = is_paused_clone_for_rec;
            let ws_connected_clone = ws_connected_clone_for_rec;
            let mut reconnect_tx_opt = Some(reconnect_tx);
            while let Some(res) = ws_read.next().await {
                match res {
                    Ok(Message::Text(text)) => {
                        let text_str: &str = &text;
                        if let Err(e) = handle_deepgram_server_message(
                            text_str,
                            &playback_tx_recv,
                            &event_tx_recv,
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
                        if let Err(e) = playback_tx_recv.try_send(pcm) {
                            log::warn!("[DeepgramVoiceAgent] Playback bridge buffer full: {:?}", e);
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
            if is_paused_clone.load(Ordering::SeqCst) {
                log::info!("[DeepgramVoiceAgent] WebSocket disconnected silently during pause.");
            } else {
                if let Some(tx) = reconnect_tx_opt.take() {
                    let _ = tx.send(());
                }
            }
        });

        // Spawn connection lifecycle orchestrator to handle future reconnects in background
        handle.spawn(async move {
            let mut active_write_task = Some(write_task);
            let mut active_receiver_task = Some(receiver_task);

            let max_reconnect_attempts = 3;

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

                // Attempt reconnect
                let mut reconnect_attempts = 0;
                let mut reconnected = false;

                while reconnect_attempts < max_reconnect_attempts {
                    log::info!(
                        "[DeepgramVoiceAgent] Reconnecting to Deepgram Voice Agent (attempt {}/{})...",
                        reconnect_attempts + 1,
                        max_reconnect_attempts
                    );

                    tokio::time::sleep(std::time::Duration::from_secs(2 * reconnect_attempts as u64 + 1)).await;

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

                            let new_playback_tx = playback_tx_clone.clone();
                            let new_event_tx = event_tx_clone.clone();
                            let new_state_recv = state_clone.clone();
                            let is_paused_rec = is_paused_clone.clone();
                            let ws_conn_rec = ws_connected_clone.clone();

                            let new_receiver_task = tokio::spawn(async move {
                                let mut reconnect_tx_opt = Some(new_reconnect_tx);
                                while let Some(res) = new_ws_read.next().await {
                                    match res {
                                        Ok(Message::Text(text)) => {
                                            let text_str: &str = &text;
                                            if let Err(e) = handle_deepgram_server_message(text_str, &new_playback_tx, &new_event_tx, &new_state_recv) {
                                                log::error!("[DeepgramVoiceAgent] Reconnected Message handling error: {:?}", e);
                                            }
                                        }
                                        Ok(Message::Binary(bytes)) => {
                                            let pcm: Vec<i16> = bytes
                                                .chunks_exact(2)
                                                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                                                .collect();
                                            if let Err(e) = new_playback_tx.try_send(pcm) {
                                                log::warn!("[DeepgramVoiceAgent] Reconnected Playback bridge buffer full: {:?}", e);
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
                                if is_paused_rec.load(Ordering::SeqCst) {
                                    log::info!("[DeepgramVoiceAgent] Reconnected WebSocket disconnected silently during pause.");
                                } else {
                                    if let Some(tx) = reconnect_tx_opt.take() {
                                        let _ = tx.send(());
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
                    let _ = event_tx_clone.send(VoxEvent::Error {
                        turn_id: 0,
                        message: "Deepgram connection lost permanently after multiple retries.".to_string(),
                    });
                    *ws_sender.lock() = None;
                    audio_sender_task.abort();
                    control_sender_task.abort();
                    break 'reconnect_loop;
                }
            }
        });

        Ok(Box::new(DeepgramVoiceAgentSession {
            audio_tx,
            control_tx,
            shutdown_tx: parking_lot::Mutex::new(Some(shutdown_tx)),
            ws_connected,
            last_activity_time: last_activity_time_sender,
        }))
    }

    /// Performs a network health check by probing the Deepgram Voice Agent TCP endpoint.
    fn health_check(&self) -> bool {
        use std::net::ToSocketAddrs;
        if let Ok(mut addrs) = "agent.deepgram.com:443".to_socket_addrs() {
            if let Some(addr) = addrs.next() {
                return std::net::TcpStream::connect_timeout(
                    &addr,
                    std::time::Duration::from_secs(2),
                )
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
                "sample_rate": 16000
            },
            "output": {
                "encoding": "linear16",
                "sample_rate": 24000,
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

    let handshake_timeout = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(res) = ws_read.next().await {
            match res {
                Ok(Message::Text(text)) => {
                    let val: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
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
        Err(_) => Err(anyhow!("Handshake timed out after 5 seconds")),
    }
}

enum ControlEvent {
    Interrupt,
}

struct SessionState {
    last_assistant_text: String,
    last_activity_time: Arc<std::sync::atomic::AtomicU64>,
}

/// Active duplex session interacting with Deepgram Voice Agent via background channels.
pub struct DeepgramVoiceAgentSession {
    audio_tx: tokio::sync::mpsc::UnboundedSender<Vec<i16>>,
    control_tx: tokio::sync::mpsc::UnboundedSender<ControlEvent>,
    shutdown_tx: parking_lot::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    ws_connected: Arc<std::sync::atomic::AtomicBool>,
    last_activity_time: Arc<std::sync::atomic::AtomicU64>,
}

impl RealtimeSession for DeepgramVoiceAgentSession {
    /// Enqueues PCM audio chunk for transmission to Deepgram.
    fn send_audio(&self, pcm: &[i16]) -> Result<()> {
        self.last_activity_time.store(
            chrono::Utc::now().timestamp_millis() as u64,
            Ordering::Relaxed,
        );
        self.audio_tx
            .send(pcm.to_vec())
            .map_err(|e| anyhow!("Failed to write to Deepgram audio queue: {:?}", e))
    }

    /// Sends interrupt cancellation event to Deepgram.
    fn cancel(&self) -> Result<()> {
        self.last_activity_time.store(
            chrono::Utc::now().timestamp_millis() as u64,
            Ordering::Relaxed,
        );
        self.control_tx
            .send(ControlEvent::Interrupt)
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

    /// Signals start of speech activity.
    fn activity_start(&self) -> Result<()> {
        Ok(())
    }

    /// Signals end of speech activity.
    fn activity_end(&self) -> Result<()> {
        Ok(())
    }

    /// Returns whether the Deepgram WebSocket is actively connected.
    fn is_connected(&self) -> bool {
        self.ws_connected.load(Ordering::SeqCst)
    }

    /// Returns timestamp of the most recent network activity.
    fn last_activity_time(&self) -> u64 {
        self.last_activity_time.load(Ordering::Relaxed)
    }
}

/// Parses and routes incoming Deepgram Voice Agent JSON protocol messages.
fn handle_deepgram_server_message(
    text: &str,
    _playback_tx: &tokio::sync::mpsc::Sender<Vec<i16>>,
    event_tx: &Sender<VoxEvent>,
    state: &Arc<Mutex<SessionState>>,
) -> Result<()> {
    let val: serde_json::Value = serde_json::from_str(text)?;

    {
        let s_lock = state.lock();
        s_lock.last_activity_time.store(
            chrono::Utc::now().timestamp_millis() as u64,
            Ordering::Relaxed,
        );
    }

    if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
        match msg_type {
            "UserStartedSpeaking" => {
                log::info!("[DeepgramVoiceAgent] User started speaking (barge-in).");
                let mut s_lock = state.lock();
                s_lock.last_assistant_text.clear();
                if let Err(e) = event_tx.send(VoxEvent::Cancelled { turn_id: 0 }) {
                    log::warn!("[DeepgramVoiceAgent] Failed to send Cancelled event: {:?}", e);
                }
            }
            "ConversationText" => {
                let role = val.get("role").and_then(|v| v.as_str()).unwrap_or("");
                let content = val.get("content").and_then(|v| v.as_str()).unwrap_or("");

                if role == "user" {
                    log::debug!("[DeepgramVoiceAgent] User final transcript: {:?}", content);
                    if let Err(e) = event_tx.send(VoxEvent::TranscriptFinal {
                        turn_id: 0,
                        owner: crate::core::state::InteractionOwner::MainWindow,
                        text: content.to_string(),
                    }) {
                        log::warn!("[DeepgramVoiceAgent] Failed to send TranscriptFinal event: {:?}", e);
                    }
                } else if role == "assistant" {
                    log::debug!("[DeepgramVoiceAgent] Assistant transcript: {:?}", content);
                    let mut s_lock = state.lock();
                    let last_text = &s_lock.last_assistant_text;
                    if content.starts_with(last_text) {
                        let delta = &content[last_text.len()..];
                        if !delta.is_empty() {
                            if let Err(e) = event_tx.send(VoxEvent::LlmToken {
                                turn_id: 0,
                                token: delta.to_string(),
                            }) {
                                log::warn!("[DeepgramVoiceAgent] Failed to send LlmToken event: {:?}", e);
                            }
                        }
                    } else {
                        if let Err(e) = event_tx.send(VoxEvent::LlmToken {
                            turn_id: 0,
                            token: content.to_string(),
                        }) {
                            log::warn!("[DeepgramVoiceAgent] Failed to send LlmToken event: {:?}", e);
                        }
                    }
                    s_lock.last_assistant_text = content.to_string();
                }
            }
            "AgentAudioDone" => {
                log::debug!("[DeepgramVoiceAgent] Agent audio done.");
                let mut s_lock = state.lock();
                s_lock.last_assistant_text.clear();
                if let Err(e) = event_tx.send(VoxEvent::LlmFinished { turn_id: 0 }) {
                    log::warn!("[DeepgramVoiceAgent] Failed to send LlmFinished event: {:?}", e);
                }
            }
            "Error" | "Warning" => {
                log::error!("[DeepgramVoiceAgent] Server error/warning: {:?}", val);
                if let Some(err_msg) = val.get("message").and_then(|v| v.as_str()) {
                    if let Err(e) = event_tx.send(VoxEvent::Error {
                        turn_id: 0,
                        message: format!("Deepgram server error: {}", err_msg),
                    }) {
                        log::warn!("[DeepgramVoiceAgent] Failed to send Error event: {:?}", e);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}
