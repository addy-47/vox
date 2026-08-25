use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use futures_util::SinkExt;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use super::{TtsProvider, TtsProviderKind};
use crate::core::events::VoxEvent;
use crate::services::audio::decode::decode_bytes_to_24khz_mono;

const SEC_MS_GEC_VERSION: &str = "1-143.0.3650.75";
const ORIGIN_HEADER: &str = "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold";
const USER_AGENT_HEADER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0";
const WIN_EPOCH: u64 = 11_644_473_600;

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
    use std::fmt::Write;
    let unix_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut ticks = unix_ts + WIN_EPOCH;
    ticks -= ticks % 300;
    let ticks_ns = (ticks as u128) * 10_000_000;
    let str_to_hash = format!("{}{}", ticks_ns, get_trusted_client_token());
    let mut hasher = Sha256::new();
    hasher.update(str_to_hash.as_bytes());
    let hash = hasher.finalize();
    let mut hex_str = String::with_capacity(64);
    for b in hash {
        let _ = write!(hex_str, "{:02X}", b);
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
            voice: voice.unwrap_or("en-US-AriaNeural").to_string(),
            speed: AtomicU32::new(1.0f32.to_bits()),
        }
    }
}

type EdgeWsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Connects to the Microsoft Speech Platform ReadAloud WebSocket with retries.
async fn connect_edge_websocket(event_tx: &Sender<VoxEvent>, turn_id: u32) -> Option<EdgeWsStream> {
    for attempt in 1..=3 {
        let conn_id = uuid::Uuid::new_v4().simple().to_string();
        let sec_ms_gec = generate_sec_ms_gec();
        let url_str = format!(
            "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken={}&ConnectionId={}&Sec-MS-GEC={}&Sec-MS-GEC-Version={}",
            get_trusted_client_token(), conn_id, sec_ms_gec, SEC_MS_GEC_VERSION
        );

        let mut req = match url_str.into_client_request() {
            Ok(r) => r,
            Err(e) => {
                let _ = event_tx.send(VoxEvent::Error {
                    turn_id,
                    message: format!("Edge TTS URL parse error: {}", e),
                });
                return None;
            }
        };

        let muid = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
        let headers = req.headers_mut();
        let _ = headers.insert("Host", "speech.platform.bing.com".parse().unwrap());
        let _ = headers.insert("User-Agent", USER_AGENT_HEADER.parse().unwrap());
        let _ = headers.insert("Origin", ORIGIN_HEADER.parse().unwrap());
        let _ = headers.insert("Pragma", "no-cache".parse().unwrap());
        let _ = headers.insert("Cache-Control", "no-cache".parse().unwrap());
        let _ = headers.insert("Accept-Language", "en-US,en;q=0.9".parse().unwrap());
        let _ = headers.insert("Cookie", format!("muid={};", muid).parse().unwrap());

        match connect_async(req).await {
            Ok((ws, _)) => {
                log::debug!("[EdgeTTS] Connected successfully on attempt {}", attempt);
                return Some(ws);
            }
            Err(e) => {
                log::warn!("[EdgeTTS] Connection attempt {} failed: {:?}", attempt, e);
                if attempt == 3 {
                    let _ = event_tx.send(VoxEvent::Error {
                        turn_id,
                        message: format!("Edge TTS WebSocket connect error (attempt 3/3): {:?}", e),
                    });
                    return None;
                }
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
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
    let now = chrono::Utc::now()
        .format("%a %b %d %Y %H:%M:%S GMT+0000 (Coordinated Universal Time)")
        .to_string();

    let cfg_msg = format!(
        "X-Timestamp:{}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"}},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}\r\n",
        now
    );

    if let Err(e) = ws_stream.send(Message::Text(cfg_msg.into())).await {
        let _ = event_tx.send(VoxEvent::Error {
            turn_id,
            message: format!("Edge TTS config send error: {}", e),
        });
        return Err(e.into());
    }

    let req_id = uuid::Uuid::new_v4().simple().to_string();
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
        let _ = event_tx.send(VoxEvent::Error {
            turn_id,
            message: format!("Edge TTS SSML send error: {}", e),
        });
        return Err(e.into());
    }

    Ok(())
}

/// Receives and strips Microsoft binary audio framing headers, returning raw MP3 byte stream.
async fn collect_mp3_payload(ws_stream: &mut EdgeWsStream, cancel: &Arc<AtomicBool>) -> Vec<u8> {
    let mut mp3_buffer = Vec::new();

    while let Some(msg_res) = ws_stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        match &msg_res {
            Ok(Message::Binary(bin)) => {
                if bin.len() >= 2 {
                    let header_len = u16::from_be_bytes([bin[0], bin[1]]) as usize;
                    if bin.len() >= 2 + header_len {
                        let payload = &bin[2 + header_len..];
                        mp3_buffer.extend_from_slice(payload);
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

    mp3_buffer
}

impl TtsProvider for EdgeTtsProvider {
    /// Synthesizes text via Microsoft Edge ReadAloud cloud WebSocket and decodes output to 24kHz PCM.
    fn synthesize_chunk(
        &self,
        text: &str,
        turn_id: u32,
        cancel: Arc<AtomicBool>,
        event_tx: Sender<VoxEvent>,
    ) -> anyhow::Result<()> {
        let text_clean = text.trim();
        log::debug!("[EdgeTTS] Entering synthesize_chunk: '{}'", text_clean);
        if text_clean.is_empty() {
            return Ok(());
        }

        let start_time = Instant::now();
        let full_voice = resolve_full_voice_name(&self.voice);
        let speed = f32::from_bits(self.speed.load(Ordering::Relaxed));
        let speed_pct = format!("{:+}%", ((speed - 1.0) * 100.0) as i32);

        let rt = match tokio::runtime::Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                log::error!("[EdgeTTS] Failed to build Tokio runtime: {}", e);
                let _ = event_tx.send(VoxEvent::Error {
                    turn_id,
                    message: format!("Edge TTS Tokio runtime error: {}", e),
                });
                return Err(e.into());
            }
        };

        let _ = rustls::crypto::ring::default_provider().install_default();

        rt.block_on(async move {
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

            let mp3_buffer = collect_mp3_payload(&mut ws_stream, &cancel).await;

            if !mp3_buffer.is_empty() {
                match decode_bytes_to_24khz_mono(&mp3_buffer, "mp3") {
                    Ok(decoded) => {
                        let total_dur = decoded.duration_secs;
                        let proc_time = start_time.elapsed().as_secs_f32();
                        let rtf = if total_dur > 0.0 {
                            proc_time / total_dur
                        } else {
                            0.0
                        };

                        if let Err(e) = event_tx.send(VoxEvent::TtsChunk {
                            turn_id,
                            samples: decoded.samples,
                        }) {
                            log::warn!("[EdgeTTS] Error sending TtsChunk: {:?}", e);
                        }
                        if let Err(e) = event_tx.send(VoxEvent::TtsFinished { turn_id, rtf }) {
                            log::warn!("[EdgeTTS] Error sending TtsFinished: {:?}", e);
                        }
                    }
                    Err(e) => {
                        let _ = event_tx.send(VoxEvent::Error {
                            turn_id,
                            message: format!("Edge TTS MP3 decode error: {}", e),
                        });
                    }
                }
            }
        });

        Ok(())
    }

    /// Hot-updates the speech playback speed factor.
    fn set_speed(&self, speed: f32) {
        let clamped = speed.clamp(0.5, 2.0);
        self.speed.store(clamped.to_bits(), Ordering::Relaxed);
    }

    /// Returns the TtsProviderKind::EdgeTts variant identifier.
    fn kind(&self) -> TtsProviderKind {
        TtsProviderKind::EdgeTts
    }

    /// Checks network reachability against Microsoft Speech Platform endpoint.
    fn health_check(&self) -> bool {
        use std::net::ToSocketAddrs;
        if let Ok(mut addrs) = "speech.platform.bing.com:443".to_socket_addrs() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_tts_synthesis() {
        let sec_ms_gec = generate_sec_ms_gec();
        assert_eq!(sec_ms_gec.len(), 64);
    }
}
