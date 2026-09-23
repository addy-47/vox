use std::{
    fmt::Write,
    net::{TcpStream, ToSocketAddrs},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::Sender,
        Arc, LazyLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use chrono::{SecondsFormat, Utc};
use futures_util::{SinkExt, StreamExt};
use rustls::crypto::ring::default_provider;
use sha2::{Digest, Sha256};
use tokio::{
    net::TcpStream as TokioTcpStream,
    runtime::{Builder as TokioRuntimeBuilder, Runtime as TokioRuntime},
    time::{sleep, timeout},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;

use super::{SynthesisContext, TtsProvider, TtsProviderKind};
use crate::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::{AudioIntent, VoxEvent},
    },
    services::{
        audio::playback::PlaybackEngine,
        tts::{
            EDGE_TTS_DEFAULT_VOICE, EDGE_TTS_HOST, EDGE_TTS_ORIGIN, EDGE_TTS_PORT,
            EDGE_TTS_SEC_MS_GEC_VERSION, EDGE_TTS_USER_AGENT, EDGE_TTS_WIN_EPOCH,
            EDGE_TTS_WS_URL_BASE, MAX_SPEED_EDGE, MIN_SPEED_EDGE, TTS_CHUNK_SIZE, TTS_SAMPLE_RATE,
        },
    },
};

/// Returns the Microsoft Edge ReadAloud client token bytes as a decoded UTF-8 string.
pub fn get_trusted_client_token() -> String {
    let bytes: [u8; 32] = [
        0x36, 0x41, 0x35, 0x41, 0x41, 0x31, 0x44, 0x34, 0x45, 0x41, 0x46, 0x46, 0x34, 0x45, 0x39,
        0x46, 0x42, 0x33, 0x37, 0x45, 0x32, 0x33, 0x44, 0x36, 0x38, 0x34, 0x39, 0x31, 0x44, 0x36,
        0x46, 0x34,
    ];
    String::from_utf8_lossy(&bytes).to_string()
}

/// Generates the SHA-256 hash token required for Microsoft Edge ReadAloud authentication.
pub fn generate_sec_ms_gec() -> String {
    let unix_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut ticks = unix_ts + EDGE_TTS_WIN_EPOCH;
    ticks -= ticks % 300;
    let ticks_ns = (ticks as u128) * 10_000_000;
    let str_to_hash = format!("{}{}", ticks_ns, get_trusted_client_token());
    let mut hasher = Sha256::new();
    hasher.update(str_to_hash.as_bytes());
    let hash = hasher.finalize();
    let mut hex_str = String::with_capacity(64);
    for b in hash {
        if let Err(e) = write!(hex_str, "{:02X}", b) {
            log::warn!("[EdgeTTS] Failed to format hash byte: {}", e);
        }
    }
    hex_str
}

/// Expands a short voice identifier into the full Microsoft Server Speech voice name.
pub fn resolve_full_voice_name(voice: &str) -> String {
    if voice.contains("Microsoft Server Speech") {
        voice.to_string()
    } else if let Some((lang, name)) = voice.rsplit_once('-') {
        format!(
            "Microsoft Server Speech Text to Speech Voice ({}, {})",
            lang, name
        )
    } else {
        format!(
            "Microsoft Server Speech Text to Speech Voice (en-US, {})",
            voice
        )
    }
}

/// Text-to-speech synthesis provider connecting to Microsoft Edge ReadAloud WebSocket service.
pub struct EdgeTtsProvider {
    voice: String,
    speed: AtomicU32,
}

impl EdgeTtsProvider {
    /// Creates a new EdgeTtsProvider configured with the given voice name or default Aria.
    pub fn new(voice: Option<&str>) -> Self {
        Self {
            voice: voice.unwrap_or(EDGE_TTS_DEFAULT_VOICE).to_string(),
            speed: AtomicU32::new(1.0f32.to_bits()),
        }
    }
}

type EdgeWsStream = WebSocketStream<MaybeTlsStream<TokioTcpStream>>;

/// Connects to the Microsoft Speech Platform ReadAloud WebSocket with retries.
async fn connect_edge_websocket(event_tx: &Sender<VoxEvent>, turn_id: u32) -> Option<EdgeWsStream> {
    for attempt in 1..=3 {
        let conn_id = Uuid::new_v4().simple().to_string();
        let sec_ms_gec = generate_sec_ms_gec();
        let url_str = format!(
            "{}?TrustedClientToken={}&ConnectionId={}&Sec-MS-GEC={}&Sec-MS-GEC-Version={}",
            EDGE_TTS_WS_URL_BASE,
            get_trusted_client_token(),
            conn_id,
            sec_ms_gec,
            EDGE_TTS_SEC_MS_GEC_VERSION
        );

        let mut req = match url_str.into_client_request() {
            Ok(r) => r,
            Err(e) => {
                if let Err(send_err) = event_tx.send(VoxEvent::Error(PipelineError {
                    turn_id,
                    message: format!("Edge TTS URL parse error: {}", e),
                    source: "EdgeTts".to_string(),
                    impact: PipelineImpact::Degraded,
                })) {
                    log::warn!("[EdgeTTS] Failed to emit error event: {}", send_err);
                }
                return None;
            }
        };

        let muid = Uuid::new_v4().simple().to_string().to_uppercase();
        let headers = req.headers_mut();
        if let Ok(val) = EDGE_TTS_HOST.parse() {
            headers.insert("Host", val);
        }
        if let Ok(val) = EDGE_TTS_USER_AGENT.parse() {
            headers.insert("User-Agent", val);
        }
        if let Ok(val) = EDGE_TTS_ORIGIN.parse() {
            headers.insert("Origin", val);
        }
        if let Ok(val) = "no-cache".parse() {
            headers.insert("Pragma", val);
        }
        if let Ok(val) = "no-cache".parse() {
            headers.insert("Cache-Control", val);
        }
        if let Ok(val) = "en-US,en;q=0.9".parse() {
            headers.insert("Accept-Language", val);
        }
        if let Ok(val) = format!("muid={};", muid).parse() {
            headers.insert("Cookie", val);
        }

        match connect_async(req).await {
            Ok((ws, _)) => {
                log::debug!("[EdgeTTS] Connected successfully on attempt {}", attempt);
                return Some(ws);
            }
            Err(e) => {
                log::warn!("[EdgeTTS] Connection attempt {} failed: {:?}", attempt, e);
                if attempt == 3 {
                    if let Err(send_err) = event_tx.send(VoxEvent::Error(PipelineError {
                        turn_id,
                        message: format!("Edge TTS WebSocket connect error (attempt 3/3): {:?}", e),
                        source: "EdgeTts".to_string(),
                        impact: PipelineImpact::Degraded,
                    })) {
                        log::warn!("[EdgeTTS] Failed to emit error event: {}", send_err);
                    }
                    return None;
                }
                sleep(Duration::from_millis(150)).await;
            }
        }
    }
    None
}

/// Transmits the initial speech.config and SSML request frames over the WebSocket.
async fn send_ssml_request(
    ws_stream: &mut EdgeWsStream,
    text_clean: &str,
    full_voice: &str,
    speed_pct: &str,
    event_tx: &Sender<VoxEvent>,
    turn_id: u32,
) -> Result<()> {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    let speech_config = "Content-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{\"context\":{\"synthesis\":{\"audio\":{\"metadataoptions\":{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"},\"outputFormat\":\"raw-24khz-16bit-mono-pcm\"}}}}";

    if let Err(e) = ws_stream.send(Message::Text(speech_config.into())).await {
        if let Err(send_err) = event_tx.send(VoxEvent::Error(PipelineError {
            turn_id,
            message: format!("Edge TTS config send error: {}", e),
            source: "EdgeTts".to_string(),
            impact: PipelineImpact::Degraded,
        })) {
            log::warn!("[EdgeTTS] Failed to send error event: {}", send_err);
        }
        return Err(e.into());
    }

    let req_id = Uuid::new_v4().simple().to_string();
    let escaped_text = text_clean
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let ssml_body = format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'><voice name='{}'><prosody pitch='+0Hz' rate='{}' volume='+0%'>{}</prosody></voice></speak>",
        full_voice, speed_pct, escaped_text
    );
    let ssml_msg = format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{}Z\r\nPath:ssml\r\n\r\n{}",
        req_id, now, ssml_body
    );

    if let Err(e) = ws_stream.send(Message::Text(ssml_msg.into())).await {
        if let Err(send_err) = event_tx.send(VoxEvent::Error(PipelineError {
            turn_id,
            message: format!("Edge TTS SSML send error: {}", e),
            source: "EdgeTts".to_string(),
            impact: PipelineImpact::Degraded,
        })) {
            log::warn!("[EdgeTTS] Failed to send error event: {}", send_err);
        }
        return Err(e.into());
    }

    Ok(())
}

/// Receives Microsoft binary raw-24khz-16bit-mono-pcm frames, streaming directly into PlaybackEngine.
async fn stream_pcm_payload(
    ws_stream: &mut EdgeWsStream,
    cancel: &Arc<AtomicBool>,
    playback: &Arc<PlaybackEngine>,
    intent: AudioIntent,
) -> usize {
    let mut total_samples = 0;
    let mut remainder_byte: Option<u8> = None;

    let res = timeout(Duration::from_secs(30), async {
        while let Some(msg_res) = ws_stream.next().await {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            match msg_res {
                Ok(Message::Binary(bin)) => {
                    if bin.len() >= 2 {
                        let header_len = u16::from_be_bytes([bin[0], bin[1]]) as usize;
                        if bin.len() >= 2 + header_len {
                            let payload = &bin[2 + header_len..];
                            if payload.is_empty() {
                                continue;
                            }

                            let mut samples = Vec::with_capacity(payload.len() / 2 + 1);
                            let mut offset = 0;
                            if let Some(prev) = remainder_byte.take() {
                                let s = i16::from_le_bytes([prev, payload[0]]);
                                samples.push(s as f32 / 32768.0);
                                offset = 1;
                            }
                            while offset + 1 < payload.len() {
                                let s = i16::from_le_bytes([payload[offset], payload[offset + 1]]);
                                samples.push(s as f32 / 32768.0);
                                offset += 2;
                            }
                            if offset < payload.len() {
                                remainder_byte = Some(payload[offset]);
                            }

                            if !samples.is_empty() && !cancel.load(Ordering::Relaxed) {
                                total_samples += samples.len();
                                for chunk in samples.chunks(TTS_CHUNK_SIZE) {
                                    if cancel.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    playback.ingest_chunk_with_intent(chunk, intent);
                                }
                            }
                        }
                    }
                }
                Ok(Message::Text(txt)) => {
                    if txt.contains("Path:turn.end") {
                        break;
                    }
                }
                Err(e) => {
                    log::warn!("[EdgeTTS] WebSocket receive error: {:?}", e);
                    break;
                }
                _ => {}
            }
        }
    })
    .await;

    if res.is_err() {
        log::warn!("[EdgeTTS] Timed out waiting for audio frames from Edge TTS server");
    }

    total_samples
}

impl TtsProvider for EdgeTtsProvider {
    /// Synthesizes text via Microsoft Edge ReadAloud cloud WebSocket and streams 24kHz PCM directly to PlaybackEngine.
    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> Result<()> {
        let text_clean = text.trim();
        log::debug!("[EdgeTTS] Entering synthesize_chunk: '{}'", text_clean);
        if text_clean.is_empty() {
            return Ok(());
        }

        let start_time = Instant::now();
        let full_voice = resolve_full_voice_name(&self.voice);
        let speed = f32::from_bits(self.speed.load(Ordering::Relaxed));
        let speed_pct = format!("{:+}%", ((speed - 1.0) * 100.0) as i32);

        static EDGE_TTS_RUNTIME: LazyLock<TokioRuntime> = LazyLock::new(|| {
            TokioRuntimeBuilder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .thread_name("vox-edge-tts")
                .build()
                .expect("Failed to build Edge TTS shared Tokio runtime")
        });

        if let Err(e) = default_provider().install_default() {
            log::debug!(
                "[EdgeTTS] Ring crypto provider already set or error: {:?}",
                e
            );
        }

        let turn_id = ctx.turn_id;
        let event_tx = ctx.event_tx.clone();
        let cancel = ctx.cancel.clone();
        let playback = Arc::clone(ctx.playback);
        let intent = ctx.intent;
        let telemetry_rtf = ctx.telemetry_rtf.cloned();

        EDGE_TTS_RUNTIME.block_on(async move {
            let mut ws_stream = match connect_edge_websocket(&event_tx, turn_id).await {
                Some(ws) => ws,
                None => return,
            };

            if send_ssml_request(
                &mut ws_stream,
                text_clean,
                &full_voice,
                &speed_pct,
                &event_tx,
                turn_id,
            )
            .await
            .is_err()
            {
                return;
            }

            let total_samples =
                stream_pcm_payload(&mut ws_stream, &cancel, &playback, intent).await;

            if total_samples > 0 && !cancel.load(Ordering::Relaxed) {
                let total_dur = total_samples as f32 / TTS_SAMPLE_RATE as f32;
                let proc_time = start_time.elapsed().as_secs_f32();
                let rtf = if total_dur > 0.0 {
                    proc_time / total_dur
                } else {
                    0.0
                };

                log::info!(
                    "[EdgeTTS] Synthesis complete (turn {}). {:.2}s audio, RTF: {:.3}",
                    turn_id,
                    total_dur,
                    rtf
                );

                if let Some(rtf_handle) = telemetry_rtf {
                    rtf_handle.store(rtf.to_bits(), Ordering::Relaxed);
                }
            }
        });

        Ok(())
    }

    /// Hot-updates the speech playback speed factor.
    fn set_speed(&self, speed: f32) {
        let clamped = speed.clamp(MIN_SPEED_EDGE, MAX_SPEED_EDGE);
        self.speed.store(clamped.to_bits(), Ordering::Relaxed);
    }

    /// Returns the TtsProviderKind::EdgeTts variant identifier.
    fn kind(&self) -> TtsProviderKind {
        TtsProviderKind::EdgeTts
    }

    /// Checks network reachability against Microsoft Speech Platform endpoint.
    fn health_check(&self) -> bool {
        let host_port = format!("{}:{}", EDGE_TTS_HOST, EDGE_TTS_PORT);
        if let Ok(mut addrs) = host_port.to_socket_addrs() {
            if let Some(addr) = addrs.next() {
                return TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok();
            }
        }
        false
    }
}
