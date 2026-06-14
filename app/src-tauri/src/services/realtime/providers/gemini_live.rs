use anyhow::{anyhow, bail, Result};
use futures_util::{SinkExt, StreamExt};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;

use crate::core::events::VoxEvent;
use crate::core::settings::GeminiRealtimeConfig;
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

pub struct GeminiLiveProvider {
    config: GeminiRealtimeConfig,
    system_prompt: String,
    is_paused: Arc<std::sync::atomic::AtomicBool>,
}

impl GeminiLiveProvider {
    pub fn new(
        config: GeminiRealtimeConfig,
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

impl RealtimeVoiceProvider for GeminiLiveProvider {
    fn kind(&self) -> RealtimeProviderKind {
        RealtimeProviderKind::GeminiLive
    }

    fn audio_config(&self) -> RealtimeAudioConfig {
        RealtimeAudioConfig {
            input_sample_rate: 16000,
            output_sample_rate: 24000,
            requires_input_resampling: false,
            requires_output_resampling: true,
        }
    }

    fn connect(
        &self,
        interaction_mode: crate::core::settings::InteractionMode,
        playback_tx: tokio::sync::mpsc::Sender<Vec<i16>>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<Box<dyn RealtimeSession>> {
        let handle = tokio::runtime::Handle::current();

        if self.config.api_key.is_empty() {
            bail!("No API key configured for Gemini Live. Please check settings.");
        }

        let api_key = &self.config.api_key;
        let model = if self.config.model.starts_with("models/") {
            self.config.model.clone()
        } else {
            format!("models/{}", self.config.model)
        };

        let url = if let Ok(override_url) = std::env::var("GEMINI_LIVE_ENDPOINT_OVERRIDE") {
            let base = override_url.trim_end_matches('/');
            format!("{}/?key={}", base, api_key)
        } else {
            format!(
                "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent?key={}",
                api_key
            )
        };

        let is_ptt = interaction_mode == crate::core::settings::InteractionMode::PTT;

        // Perform initial connection and setup handshake synchronously
        let (mut ws_write, mut ws_read) = tokio::task::block_in_place(|| {
            handle.block_on(async {
                perform_handshake(
                    &url,
                    &model,
                    &self.config,
                    &self.system_prompt,
                    is_ptt,
                    self.config.resume_handle.clone(),
                )
                .await
            })
        })?;

        let config_clone = self.config.clone();
        let system_prompt_clone = self.system_prompt.clone();
        let (audio_tx, mut audio_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<i16>>();
        let (control_tx, mut control_rx) = tokio::sync::mpsc::unbounded_channel::<ControlEvent>();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Session metrics & resilience states
        let is_paused_clone = self.is_paused.clone();
        let ws_connected = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let ws_connected_clone = ws_connected.clone();
        let last_activity_time = Arc::new(std::sync::atomic::AtomicU64::new(
            chrono::Utc::now().timestamp_millis() as u64,
        ));
        let last_activity_time_sender = last_activity_time.clone();

        // SessionState tracking
        let state = Arc::new(Mutex::new(SessionState {
            interrupt_active: false,
            resume_handle: config_clone.resume_handle.clone(),
            model: model.clone(),
            last_activity_time: last_activity_time.clone(),
        }));

        let state_clone = state.clone();
        let playback_tx_clone = playback_tx.clone();
        let event_tx_clone = event_tx.clone();

        // Shared WebSocket writer sender channel - updated on every reconnect
        let ws_sender: Arc<Mutex<Option<UnboundedSender<Message>>>> = Arc::new(Mutex::new(None));
        let ws_sender_audio = ws_sender.clone();
        let ws_sender_control = ws_sender.clone();

        // Spawn persistent Audio Sender Task
        let audio_sender_task = handle.spawn(async move {
            let mut packet_count = 0;
            while let Some(pcm) = audio_rx.recv().await {
                let base64_audio =
                    base64::Engine::encode(&base64::prelude::BASE64_STANDARD, unsafe {
                        std::slice::from_raw_parts(pcm.as_ptr() as *const u8, pcm.len() * 2)
                    });
                let msg = serde_json::json!({
                    "realtimeInput": {
                        "mediaChunks": [
                            {
                                "mimeType": "audio/pcm;rate=16000",
                                "data": base64_audio
                            }
                        ]
                    }
                })
                .to_string();

                let opt_tx = {
                    let guard = ws_sender_audio.lock().unwrap();
                    guard.clone()
                };
                if let Some(tx) = opt_tx {
                    let _ = tx.send(Message::Text(msg.into()));
                }
                packet_count += 1;
                if packet_count % 100 == 0 {
                    log::info!(
                        "[GeminiLive] Sent {} raw audio blocks to WebSocket.",
                        packet_count
                    );
                }
            }
        });

        // Spawn persistent Control Sender Task
        let control_sender_task = handle.spawn(async move {
            while let Some(evt) = control_rx.recv().await {
                let msg = match evt {
                    ControlEvent::Text(txt) => serde_json::json!({
                        "realtimeInput": {
                            "text": txt
                        }
                    })
                    .to_string(),
                    ControlEvent::ActivityStart => serde_json::json!({
                        "realtimeInput": {
                            "activityStart": {}
                        }
                    })
                    .to_string(),
                    ControlEvent::ActivityEnd => serde_json::json!({
                        "realtimeInput": {
                            "activityEnd": {}
                        }
                    })
                    .to_string(),
                    ControlEvent::Interrupt => {
                        let start_msg = serde_json::json!({
                            "realtimeInput": {
                                "activityStart": {}
                            }
                        })
                        .to_string();

                        let opt_tx = {
                            let guard = ws_sender_control.lock().unwrap();
                            guard.clone()
                        };
                        if let Some(ref tx) = opt_tx {
                            let _ = tx.send(Message::Text(start_msg.into()));
                        }

                        if is_ptt {
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            let end_msg = serde_json::json!({
                                "realtimeInput": {
                                    "activityEnd": {}
                                }
                            })
                            .to_string();
                            if let Some(ref tx) = opt_tx {
                                let _ = tx.send(Message::Text(end_msg.into()));
                            }
                        }
                        continue;
                    }
                };

                let opt_tx = {
                    let guard = ws_sender_control.lock().unwrap();
                    guard.clone()
                };
                if let Some(tx) = opt_tx {
                    let _ = tx.send(Message::Text(msg.into()));
                }
            }
        });

        // Set up the first active connection
        let (ws_write_tx, mut ws_write_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
        *ws_sender.lock().unwrap() = Some(ws_write_tx);

        let write_task = handle.spawn(async move {
            while let Some(msg) = ws_write_rx.recv().await {
                if let Err(e) = ws_write.send(msg).await {
                    log::error!("[GeminiLive] WebSocket write error: {:?}", e);
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
                        if let Err(e) = handle_gemini_server_message(text_str, &playback_tx_recv, &event_tx_recv, &state_recv) {
                            log::error!("[GeminiLive] Message handling error: {:?}", e);
                        }
                        let val: serde_json::Value = serde_json::from_str(text_str).unwrap_or_default();
                        if val.get("goAway").is_some() {
                            log::warn!("[GeminiLive] Server requested session termination (goAway). Reconnecting...");
                            ws_connected_clone.store(false, Ordering::SeqCst);
                            if is_paused_clone.load(Ordering::SeqCst) {
                                break;
                            }
                            if let Some(tx) = reconnect_tx_opt.take() {
                                let _ = tx.send(());
                            }
                            break;
                        }
                    }
                    Ok(Message::Binary(bytes)) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let text_str = &*text;
                        if let Err(e) = handle_gemini_server_message(text_str, &playback_tx_recv, &event_tx_recv, &state_recv) {
                            log::error!("[GeminiLive] Message handling error: {:?}", e);
                        }
                        let val: serde_json::Value = serde_json::from_str(text_str).unwrap_or_default();
                        if val.get("goAway").is_some() {
                            log::warn!("[GeminiLive] Server requested session termination (goAway). Reconnecting...");
                            ws_connected_clone.store(false, Ordering::SeqCst);
                            if is_paused_clone.load(Ordering::SeqCst) {
                                break;
                            }
                            if let Some(tx) = reconnect_tx_opt.take() {
                                let _ = tx.send(());
                            }
                            break;
                        }
                    }
                    Ok(Message::Close(cf)) => {
                        log::info!("[GeminiLive] WebSocket closed by server: {:?}", cf);
                        break;
                    }
                    Err(e) => {
                        log::error!("[GeminiLive] WebSocket read error: {:?}", e);
                        break;
                    }
                    _ => {}
                }
            }
            ws_connected_clone.store(false, Ordering::SeqCst);
            if is_paused_clone.load(Ordering::SeqCst) {
                log::info!("[GeminiLive] WebSocket disconnected silently during pause.");
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
                // Wait for reconnect signal, shutdown signal, or task failure
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        log::info!("[GeminiLive] Session shutdown requested. Aborting tasks.");
                        *ws_sender.lock().unwrap() = None;
                        if let Some(t) = active_write_task.take() { t.abort(); }
                        if let Some(t) = active_receiver_task.take() { t.abort(); }
                        audio_sender_task.abort();
                        control_sender_task.abort();
                        break 'reconnect_loop;
                    }
                    _ = &mut reconnect_rx => {
                        log::warn!("[GeminiLive] Connection dropped. Cleaning up active connection tasks...");
                        *ws_sender.lock().unwrap() = None;
                        if let Some(t) = active_write_task.take() { t.abort(); }
                        if let Some(t) = active_receiver_task.take() { t.abort(); }
                    }
                }

                // Attempt reconnect
                let mut reconnect_attempts = 0;
                let mut reconnected = false;

                while reconnect_attempts < max_reconnect_attempts {
                    log::info!(
                        "[GeminiLive] Reconnecting to Gemini Live WebSocket (attempt {}/{})...",
                        reconnect_attempts + 1,
                        max_reconnect_attempts
                    );

                    // Sleep slightly before connection attempt
                    tokio::time::sleep(std::time::Duration::from_secs(2 * reconnect_attempts as u64 + 1)).await;

                    let current_resume_handle = {
                        let s_lock = state_clone.lock().unwrap();
                        s_lock.resume_handle.clone()
                    };

                    let system_prompt_clone = system_prompt_clone.clone();
                    match perform_handshake(&url, &model, &config_clone, &system_prompt_clone, is_ptt, current_resume_handle).await {
                        Ok((mut new_ws_write, mut new_ws_read)) => {
                            log::info!("[GeminiLive] Reconnection handshake completed successfully!");
                            reconnected = true;
                            ws_connected_clone.store(true, Ordering::SeqCst);

                            let (new_ws_write_tx, mut new_ws_write_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
                            *ws_sender.lock().unwrap() = Some(new_ws_write_tx);

                            let new_write_task = tokio::spawn(async move {
                                while let Some(msg) = new_ws_write_rx.recv().await {
                                    if let Err(e) = new_ws_write.send(msg).await {
                                        log::error!("[GeminiLive] Reconnected WS write error: {:?}", e);
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
                                            if let Err(e) = handle_gemini_server_message(text_str, &new_playback_tx, &new_event_tx, &new_state_recv) {
                                                log::error!("[GeminiLive] Reconnected Message handling error: {:?}", e);
                                            }
                                            let val: serde_json::Value = serde_json::from_str(text_str).unwrap_or_default();
                                            if val.get("goAway").is_some() {
                                                log::warn!("[GeminiLive] Reconnected Server requested session termination (goAway).");
                                                ws_conn_rec.store(false, Ordering::SeqCst);
                                                if is_paused_rec.load(Ordering::SeqCst) {
                                                    break;
                                                }
                                                if let Some(tx) = reconnect_tx_opt.take() {
                                                    let _ = tx.send(());
                                                }
                                                break;
                                            }
                                        }
                                        Ok(Message::Binary(bytes)) => {
                                            let text = String::from_utf8_lossy(&bytes);
                                            let text_str = &*text;
                                            if let Err(e) = handle_gemini_server_message(text_str, &new_playback_tx, &new_event_tx, &new_state_recv) {
                                                log::error!("[GeminiLive] Reconnected Message handling error: {:?}", e);
                                            }
                                            let val: serde_json::Value = serde_json::from_str(text_str).unwrap_or_default();
                                            if val.get("goAway").is_some() {
                                                log::warn!("[GeminiLive] Reconnected Server requested session termination (goAway).");
                                                ws_conn_rec.store(false, Ordering::SeqCst);
                                                if is_paused_rec.load(Ordering::SeqCst) {
                                                    break;
                                                }
                                                if let Some(tx) = reconnect_tx_opt.take() {
                                                    let _ = tx.send(());
                                                }
                                                break;
                                            }
                                        }
                                        Ok(Message::Close(cf)) => {
                                            log::info!("[GeminiLive] Reconnected WebSocket closed by server: {:?}", cf);
                                            break;
                                        }
                                        Err(e) => {
                                            log::error!("[GeminiLive] Reconnected WebSocket read error: {:?}", e);
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                ws_conn_rec.store(false, Ordering::SeqCst);
                                if is_paused_rec.load(Ordering::SeqCst) {
                                    log::info!("[GeminiLive] Reconnected WebSocket disconnected silently during pause.");
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
                            log::error!("[GeminiLive] Reconnection attempt failed: {:?}", e);
                            reconnect_attempts += 1;
                        }
                    }
                }

                if !reconnected {
                    log::error!("[GeminiLive] Max reconnection attempts reached. Terminating session orchestrator.");
                    
                    // Clear the cache file since token is now invalid
                    let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
                    if cache_path.exists() {
                        let _ = std::fs::remove_file(cache_path);
                    }

                    let _ = event_tx_clone.send(VoxEvent::Error {
                        turn_id: 0,
                        message: "Gemini connection lost permanently after multiple retries.".to_string(),
                    });
                    *ws_sender.lock().unwrap() = None;
                    audio_sender_task.abort();
                    control_sender_task.abort();
                    break 'reconnect_loop;
                }
            }
        });

        Ok(Box::new(GeminiLiveSession {
            audio_tx,
            control_tx,
            shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
            ws_connected,
            last_activity_time: last_activity_time_sender,
        }))
    }

    fn health_check(&self) -> bool {
        // TCP check to check endpoint reachability
        std::net::TcpStream::connect_timeout(
            &"generativelanguage.googleapis.com:443"
                .parse()
                .unwrap_or("142.250.190.42:443".parse().unwrap()),
            std::time::Duration::from_secs(2),
        )
        .is_ok()
    }
}

async fn perform_handshake(
    url: &str,
    model: &str,
    config: &GeminiRealtimeConfig,
    system_prompt: &str,
    is_ptt: bool,
    resume_handle: Option<String>,
) -> Result<(WsWriter, WsReader)> {
    log::info!("[GeminiLive] Connecting to Gemini Live WebSocket: {}", {
        if let Some(pos) = url.find("key=") {
            let key = &url[pos+4..];
            let redacted_key = if key.len() > 8 {
                format!("{}...", &key[..8])
            } else {
                "...".to_string()
            };
            format!("{}key={}", &url[..pos], redacted_key)
        } else {
            url.to_string()
        }
    });

    let ws_stream = tokio_tungstenite::connect_async(url)
        .await
        .map_err(|e| {
            let err_msg = format!("WebSocket connection failed: {:?}", e);
            log::error!("[GeminiLive] {}", err_msg);
            anyhow!("{}", err_msg)
        })?
        .0;
        
    let (mut ws_write, mut ws_read) = ws_stream.split();
    log::info!("[GeminiLive] WebSocket connection established. Sending setup config frame.");

    let formatted_model = if model.starts_with("models/") || model.starts_with("publishers/") {
        model.to_string()
    } else {
        format!("models/{}", model)
    };

    let mut setup_json = serde_json::json!({
        "setup": {
            "model": formatted_model,
            "generationConfig": {
                "responseModalities": ["AUDIO"],
                "speechConfig": {
                    "voiceConfig": {
                        "prebuiltVoiceConfig": {
                            "voiceName": if config.voice_name.is_empty() { "Aoede" } else { &config.voice_name }
                        }
                    },
                    "languageCode": if config.language_code.is_empty() { "en-US" } else { &config.language_code }
                },
                "temperature": config.temperature,
                "thinkingConfig": {
                    "thinkingBudget": 0
                }
            },
            "inputAudioTranscription": {},
            "outputAudioTranscription": {}
        }
    });

    let mut tools = vec![];
    if config.enable_web_search {
        tools.push(serde_json::json!({
            "googleSearchRetrieval": {}
        }));
    }
    if !tools.is_empty() {
        setup_json["setup"]["tools"] = serde_json::json!(tools);
    }

    if !system_prompt.is_empty() {
        setup_json["setup"]["systemInstruction"] = serde_json::json!({
            "parts": [
                { "text": system_prompt }
            ]
        });
    }

    let activity_detection = if is_ptt {
        serde_json::json!({
            "disabled": true
        })
    } else {
        serde_json::json!({
            "disabled": false,
            "startOfSpeechSensitivity": "START_SENSITIVITY_HIGH",
            "endOfSpeechSensitivity": "END_SENSITIVITY_HIGH",
            "prefixPaddingMs": 20,
            "silenceDurationMs": 100
        })
    };
    setup_json["setup"]["realtimeInputConfig"] = serde_json::json!({
        "automaticActivityDetection": activity_detection
    });

    if let Some(ref handle) = resume_handle {
        log::info!("[GeminiLive] Requesting session resumption with handle length {}", handle.len());
        setup_json["setup"]["sessionResumption"] = serde_json::json!({
            "handle": handle
        });
    }

    ws_write
        .send(Message::Text(setup_json.to_string().into()))
        .await
        .map_err(|e| {
            let err_msg = format!("Failed to send setup JSON frame: {:?}", e);
            log::error!("[GeminiLive] {}", err_msg);
            anyhow!("{}", err_msg)
        })?;

    log::info!("[GeminiLive] Setup config frame sent. Waiting for setupComplete response...");

    let setup_completed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(res) = ws_read.next().await {
            match res {
                Ok(Message::Text(text)) => {
                    let val: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                    if val.get("setupComplete").is_some() {
                        log::info!("[GeminiLive] Received setupComplete from Gemini Live server.");
                        return Ok(());
                    } else if let Some(err) = val.get("error") {
                        let err_msg = format!("Gemini setup error response: {:?}", err);
                        log::error!("[GeminiLive] {}", err_msg);
                        return Err(anyhow!("{}", err_msg));
                    }
                }
                Ok(Message::Binary(bytes)) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let val: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                    if val.get("setupComplete").is_some() {
                        log::info!("[GeminiLive] Received setupComplete (binary) from Gemini Live server.");
                        return Ok(());
                    } else if let Some(err) = val.get("error") {
                        let err_msg = format!("Gemini setup error response (binary): {:?}", err);
                        log::error!("[GeminiLive] {}", err_msg);
                        return Err(anyhow!("{}", err_msg));
                    }
                }
                Ok(msg) => {
                    let err_msg = format!("Unexpected message payload during setup: {:?}", msg);
                    log::error!("[GeminiLive] {}", err_msg);
                    return Err(anyhow!("{}", err_msg));
                }
                Err(e) => {
                    let err_msg = format!("WebSocket error during handshake: {:?}", e);
                    log::error!("[GeminiLive] {}", err_msg);
                    return Err(anyhow!("{}", err_msg));
                }
            }
        }
        let err_msg = "WebSocket stream terminated before setupComplete";
        log::error!("[GeminiLive] {}", err_msg);
        Err(anyhow!("{}", err_msg))
    })
    .await;

    match setup_completed {
        Ok(Ok(())) => {
            log::info!("[GeminiLive] Setup completed successfully.");
            Ok((ws_write, ws_read))
        }
        Ok(Err(e)) => {
            log::error!("[GeminiLive] Handshake failed: {:?}", e);
            Err(anyhow!("Handshake failed: {:?}", e))
        }
        Err(_) => {
            log::error!("[GeminiLive] Handshake timed out after 5 seconds");
            Err(anyhow!("Handshake timed out after 5 seconds"))
        }
    }
}

enum ControlEvent {
    #[allow(dead_code)]
    Text(String),
    Interrupt,
    ActivityStart,
    ActivityEnd,
}

struct SessionState {
    interrupt_active: bool,
    resume_handle: Option<String>,
    model: String,
    last_activity_time: Arc<std::sync::atomic::AtomicU64>,
}

pub struct GeminiLiveSession {
    audio_tx: tokio::sync::mpsc::UnboundedSender<Vec<i16>>,
    control_tx: tokio::sync::mpsc::UnboundedSender<ControlEvent>,
    shutdown_tx: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    ws_connected: Arc<std::sync::atomic::AtomicBool>,
    last_activity_time: Arc<std::sync::atomic::AtomicU64>,
}

impl RealtimeSession for GeminiLiveSession {
    fn send_audio(&self, pcm: &[i16]) -> Result<()> {
        self.last_activity_time.store(chrono::Utc::now().timestamp_millis() as u64, std::sync::atomic::Ordering::Relaxed);
        self.audio_tx
            .send(pcm.to_vec())
            .map_err(|e| anyhow!("Failed to write to S2S audio pipeline queue: {:?}", e))
    }

    fn cancel(&self) -> Result<()> {
        self.last_activity_time.store(chrono::Utc::now().timestamp_millis() as u64, std::sync::atomic::Ordering::Relaxed);
        self.control_tx
            .send(ControlEvent::Interrupt)
            .map_err(|e| anyhow!("Failed to send interrupt control event: {:?}", e))
    }

    fn disconnect(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
        Ok(())
    }

    fn activity_start(&self) -> Result<()> {
        self.last_activity_time.store(chrono::Utc::now().timestamp_millis() as u64, std::sync::atomic::Ordering::Relaxed);
        self.control_tx
            .send(ControlEvent::ActivityStart)
            .map_err(|e| anyhow!("Failed to send activity_start control event: {:?}", e))
    }

    fn activity_end(&self) -> Result<()> {
        self.last_activity_time.store(chrono::Utc::now().timestamp_millis() as u64, std::sync::atomic::Ordering::Relaxed);
        self.control_tx
            .send(ControlEvent::ActivityEnd)
            .map_err(|e| anyhow!("Failed to send activity_end control event: {:?}", e))
    }

    fn is_connected(&self) -> bool {
        self.ws_connected.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn last_activity_time(&self) -> u64 {
        self.last_activity_time.load(std::sync::atomic::Ordering::Relaxed)
    }
}

fn handle_gemini_server_message(
    text: &str,
    playback_tx: &tokio::sync::mpsc::Sender<Vec<i16>>,
    event_tx: &Sender<VoxEvent>,
    state: &Arc<Mutex<SessionState>>,
) -> Result<()> {
    let val: serde_json::Value = serde_json::from_str(text)?;

    // Update last activity timestamp
    {
        let s_lock = state.lock().unwrap();
        s_lock.last_activity_time.store(chrono::Utc::now().timestamp_millis() as u64, std::sync::atomic::Ordering::Relaxed);
    }

    // Handle sessionResumptionUpdate token storage
    if let Some(resumption) = val.get("sessionResumptionUpdate") {
        if let Some(new_handle) = resumption.get("newHandle").and_then(|v| v.as_str()) {
            let model = {
                let mut s_lock = state.lock().unwrap();
                s_lock.resume_handle = Some(new_handle.to_string());
                s_lock.model.clone()
            };
            log::info!(
                "[GeminiLive] Saved resumption token: {}",
                if new_handle.len() > 12 {
                    &new_handle[..12]
                } else {
                    new_handle
                }
            );

            // Write session cache file
            let cache_path = crate::utils::paths::cache_dir().join("realtime_session.json");
            let now_ms = chrono::Utc::now().timestamp_millis() as u64;
            let expires_at = now_ms + (2 * 60 * 60 * 1000); // 2 hours TTL
            let payload = serde_json::json!({
                "provider": "gemini_live",
                "handle": new_handle,
                "expires_at": expires_at,
                "model": model,
            });

            // Write atomically: write to .tmp then rename
            let tmp_path = cache_path.with_extension("tmp");
            if let Ok(payload_str) = serde_json::to_string_pretty(&payload) {
                if let Err(e) = std::fs::write(&tmp_path, payload_str) {
                    log::error!("[GeminiLive] Failed to write temporary session cache: {:?}", e);
                } else if let Err(e) = std::fs::rename(&tmp_path, &cache_path) {
                    log::error!("[GeminiLive] Failed to rename session cache file: {:?}", e);
                }
            }
        }
    }

    if let Some(server_content) = val.get("serverContent") {
        let mut s_lock = state.lock().unwrap();

        // 1. Interruption confirmed
        if server_content
            .get("interrupted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            log::info!("[GeminiLive] Interruption confirmed by Gemini Live server.");
            s_lock.interrupt_active = false;
            let _ = event_tx.send(VoxEvent::Cancelled { turn_id: 0 });
        }

        // 2. Handle modelTurn audio and text parts
        if let Some(model_turn) = server_content.get("modelTurn") {
            if s_lock.interrupt_active {
                // Drop frames during active interruption
                return Ok(());
            }

            if let Some(parts) = model_turn.get("parts").and_then(|p| p.as_array()) {
                for part in parts {
                    if let Some(inline_data) = part.get("inlineData") {
                        if let Some(mime_type) =
                            inline_data.get("mimeType").and_then(|m| m.as_str())
                        {
                            if mime_type.starts_with("audio/") {
                                if let Some(b64_data) =
                                    inline_data.get("data").and_then(|d| d.as_str())
                                {
                                    let decoded = base64::Engine::decode(
                                        &base64::prelude::BASE64_STANDARD,
                                        b64_data,
                                    )?;
                                    let pcm: Vec<i16> = decoded
                                        .chunks_exact(2)
                                        .map(|c| i16::from_ne_bytes([c[0], c[1]]))
                                        .collect();
                                    if let Err(e) = playback_tx.try_send(pcm) {
                                        log::warn!("[GeminiLive] Playback bridge buffer full: {:?}", e);
                                    } else {
                                        static RECV_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                                        let count = RECV_COUNT.fetch_add(1, Ordering::Relaxed);
                                        if count % 100 == 0 {
                                            log::info!("[GeminiLive] Received {} model audio response chunks.", count + 1);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(text_token) = part.get("text").and_then(|t| t.as_str()) {
                        log::info!("[GeminiLive] Received text token: {:?}", text_token);
                        let _ = event_tx.send(VoxEvent::LlmToken {
                            turn_id: 0,
                            token: text_token.to_string(),
                        });
                    }
                }
            }
        }

        // 3. Handle input speech transcription (ASR text)
        if let Some(input_transcription) = server_content.get("inputTranscription") {
            if !s_lock.interrupt_active {
                if let Some(text) = input_transcription.get("text").and_then(|t| t.as_str()) {
                    log::info!("[GeminiLive] Received input transcription (ASR): {:?}", text);
                    let _ = event_tx.send(VoxEvent::TranscriptFinal {
                        turn_id: 0,
                        owner: crate::core::state::InteractionOwner::MainWindow,
                        text: text.to_string(),
                    });
                }
            }
        }

        // 4. Handle output speech transcription (TTS text)
        if let Some(output_transcription) = server_content.get("outputTranscription") {
            if !s_lock.interrupt_active {
                if let Some(text) = output_transcription.get("text").and_then(|t| t.as_str()) {
                    log::info!("[GeminiLive] Received output transcription (TTS): {:?}", text);
                    let _ = event_tx.send(VoxEvent::LlmToken {
                        turn_id: 0,
                        token: text.to_string(),
                    });
                }
            }
        }

        // 5. Handle turn completion
        if server_content
            .get("turnComplete")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            log::info!("[GeminiLive] Turn completed.");
            s_lock.interrupt_active = false;
            let _ = event_tx.send(VoxEvent::LlmFinished { turn_id: 0 });
        }
    }

    Ok(())
}
