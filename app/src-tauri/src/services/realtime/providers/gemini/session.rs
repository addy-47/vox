use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::services::realtime::transport::{FrameAction, ProviderDriver};
use crate::services::realtime::{
    OutboundCommand, RealtimeProviderEvent, RealtimeSession, LOG_INTERVAL_PACKETS,
};

use super::protocol::{encode_activity_end, encode_activity_start};

pub(super) struct GeminiSessionState {
    pub(super) interrupt_active: bool,
    pub(super) resume_handle: Option<String>,
    pub(super) model: String,
    pub(super) turn_id: Arc<std::sync::atomic::AtomicU32>,
    pub(super) server_turn_cursor: Option<u32>,
}

impl GeminiSessionState {
    pub(super) fn current_or_new_turn_id(&mut self) -> u32 {
        if let Some(id) = self.server_turn_cursor {
            id
        } else {
            let id = self.turn_id.load(Ordering::Relaxed);
            self.server_turn_cursor = Some(id);
            id
        }
    }
}

pub(crate) struct GeminiDriver {
    pub(super) state: Arc<Mutex<GeminiSessionState>>,
}

impl ProviderDriver for GeminiDriver {
    fn encode(&self, cmd: OutboundCommand) -> Option<Message> {
        match cmd {
            OutboundCommand::Audio(pcm) => {
                let mut bytes = Vec::with_capacity(pcm.len() * 2);
                for &s in &pcm {
                    bytes.extend_from_slice(&s.to_le_bytes());
                }
                let b64 = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &bytes);
                let json = serde_json::json!({
                    "realtimeInput": { "audio": { "mimeType": "audio/pcm;rate=16000", "data": b64 } }
                })
                .to_string();
                Some(Message::Text(json.into()))
            }
            OutboundCommand::ActivityStart => Some(Message::Text(encode_activity_start().into())),
            OutboundCommand::ActivityEnd => Some(Message::Text(encode_activity_end().into())),
            OutboundCommand::Interrupt => {
                self.state.lock().interrupt_active = true;

                Some(Message::Text(encode_activity_start().into()))
            }
            OutboundCommand::KeepAlive => None,
        }
    }

    fn handle_frame(
        &self,
        msg: Message,
        event_tx: &mpsc::Sender<RealtimeProviderEvent>,
    ) -> FrameAction {
        let text = match &msg {
            Message::Text(t) => t.as_str().to_owned(),
            Message::Binary(b) => String::from_utf8_lossy(b).into_owned(),
            _ => return FrameAction::Continue,
        };

        let val: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                log::error!("[GeminiLive] JSON parse error: {:?}", e);
                return FrameAction::Continue;
            }
        };

        if let Err(e) = dispatch_server_message(&val, event_tx, &self.state) {
            log::error!("[GeminiLive] Message dispatch error: {:?}", e);
        }

        if val.get("goAway").is_some() {
            log::warn!("[GeminiLive] goAway received.");
            return FrameAction::GoAway;
        }

        FrameAction::Continue
    }

    fn keepalive_interval(&self) -> Option<std::time::Duration> {
        None
    }
}

pub(crate) struct GeminiLiveSession {
    pub(super) outbound_tx: mpsc::Sender<OutboundCommand>,
    pub(super) shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub(super) terminated: Arc<AtomicBool>,
}

impl RealtimeSession for GeminiLiveSession {
    fn send_audio(&self, pcm: &[i16]) -> Result<()> {
        if self.terminated.load(Ordering::Relaxed) {
            bail!("Gemini Live session is terminated");
        }
        if let Err(tokio::sync::mpsc::error::TrySendError::Full(_)) = self
            .outbound_tx
            .try_send(OutboundCommand::Audio(pcm.to_vec()))
        {
            log::warn!("[GeminiLive] Audio queue full — dropped frame");
        }
        Ok(())
    }

    fn commit_speech_turn(&self, pcm: &[i16]) -> Result<()> {
        if self.terminated.load(Ordering::Relaxed) {
            bail!("Gemini Live session is terminated");
        }
        self.outbound_tx
            .try_send(OutboundCommand::ActivityStart)
            .map_err(|e| anyhow!("Failed to send ActivityStart: {:?}", e))?;
        if let Err(tokio::sync::mpsc::error::TrySendError::Full(_)) = self
            .outbound_tx
            .try_send(OutboundCommand::Audio(pcm.to_vec()))
        {
            log::warn!("[GeminiLive] Audio queue full on turn commit — dropped frame");
        }
        self.outbound_tx
            .try_send(OutboundCommand::ActivityEnd)
            .map_err(|e| anyhow!("Failed to send ActivityEnd: {:?}", e))?;
        Ok(())
    }

    fn cancel(&self) -> Result<()> {
        self.outbound_tx
            .try_send(OutboundCommand::Interrupt)
            .map_err(|e| anyhow!("Failed to send Interrupt: {:?}", e))
    }

    fn disconnect(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.lock().take() {
            if let Err(e) = tx.send(()) {
                log::warn!("[GeminiLive] Shutdown signal drop: {:?}", e);
            }
        }
        Ok(())
    }
}

fn dispatch_server_message(
    val: &serde_json::Value,
    event_tx: &mpsc::Sender<RealtimeProviderEvent>,
    state: &Arc<Mutex<GeminiSessionState>>,
) -> Result<()> {
    if let Some(resumption) = val.get("sessionResumptionUpdate") {
        if let Some(handle) = resumption.get("newHandle").and_then(|v| v.as_str()) {
            let model = {
                let mut s = state.lock();
                s.resume_handle = Some(handle.to_string());
                s.model.clone()
            };
            if let Err(e) = event_tx.try_send(RealtimeProviderEvent::SessionResumptionHandle {
                handle: handle.to_string(),
                model,
            }) {
                log::warn!(
                    "[GeminiLive] Failed to forward SessionResumptionHandle: {:?}",
                    e
                );
            }
        }
    }

    let Some(server_content) = val.get("serverContent") else {
        return Ok(());
    };

    let is_interrupted = server_content
        .get("interrupted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let interrupt_active = {
        let mut s = state.lock();
        if is_interrupted {
            s.interrupt_active = false;
            s.server_turn_cursor = None;
            if let Err(e) = event_tx.try_send(RealtimeProviderEvent::SpeechStart) {
                log::warn!("[GeminiLive] Failed to forward SpeechStart: {:?}", e);
            }
        }
        s.interrupt_active
    };

    if let Some(model_turn) = server_content.get("modelTurn") {
        if !interrupt_active {
            if let Err(e) = event_tx.try_send(RealtimeProviderEvent::SpeechEnd) {
                log::trace!("[GeminiLive] SpeechEnd forwarded on modelTurn: {:?}", e);
            }
            let turn_id = state.lock().current_or_new_turn_id();
            if let Some(parts) = model_turn.get("parts").and_then(|p| p.as_array()) {
                for part in parts {
                    if let Some(inline_data) = part.get("inlineData") {
                        if inline_data
                            .get("mimeType")
                            .and_then(|m| m.as_str())
                            .map(|m| m.starts_with("audio/"))
                            .unwrap_or(false)
                        {
                            if let Some(b64) = inline_data.get("data").and_then(|d| d.as_str()) {
                                let decoded =
                                    base64::Engine::decode(&base64::prelude::BASE64_STANDARD, b64)?;
                                let pcm: Vec<i16> = decoded
                                    .chunks_exact(2)
                                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                                    .collect();
                                static RECV_COUNT: std::sync::atomic::AtomicU64 =
                                    std::sync::atomic::AtomicU64::new(0);
                                let count = RECV_COUNT.fetch_add(1, Ordering::Relaxed);
                                if (count + 1).is_multiple_of(LOG_INTERVAL_PACKETS) {
                                    log::debug!(
                                        "[GeminiLive] Received {} audio chunks.",
                                        count + 1
                                    );
                                }
                                if let Err(e) =
                                    event_tx.try_send(RealtimeProviderEvent::AudioChunk(pcm))
                                {
                                    log::warn!("[GeminiLive] AudioChunk dropped: {:?}", e);
                                }
                            }
                        }
                    }
                    if let Some(token) = part.get("text").and_then(|t| t.as_str()) {
                        if let Err(e) = event_tx.try_send(RealtimeProviderEvent::LlmToken {
                            turn_id,
                            token: token.to_string(),
                        }) {
                            log::warn!("[GeminiLive] LlmToken dropped: {:?}", e);
                        }
                    }
                }
            }
        }
    }

    if let Some(tool_call) = server_content.get("toolCall") {
        log::debug!(
            "[GeminiLive] toolCall received (hook reserved): {:?}",
            tool_call
        );
    }

    if !interrupt_active {
        let turn_id = state.lock().current_or_new_turn_id();
        if let Some(text) = server_content
            .get("inputTranscription")
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
        {
            if let Err(e) = event_tx.try_send(RealtimeProviderEvent::TranscriptPartial {
                turn_id,
                text: text.to_string(),
            }) {
                log::warn!("[GeminiLive] TranscriptPartial dropped: {:?}", e);
            }
        }
        if let Some(text) = server_content
            .get("outputTranscription")
            .and_then(|t| t.get("text"))
            .and_then(|t| t.as_str())
        {
            if let Err(e) = event_tx.try_send(RealtimeProviderEvent::LlmToken {
                turn_id,
                token: text.to_string(),
            }) {
                log::warn!("[GeminiLive] outputTranscription token dropped: {:?}", e);
            }
        }
    }

    if server_content
        .get("turnComplete")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        let finished_turn_id = {
            let mut s = state.lock();
            s.interrupt_active = false;
            s.server_turn_cursor
                .take()
                .unwrap_or_else(|| s.turn_id.load(Ordering::Relaxed))
        };
        if let Err(e) = event_tx.try_send(RealtimeProviderEvent::LlmFinished {
            turn_id: finished_turn_id,
        }) {
            log::warn!("[GeminiLive] LlmFinished dropped: {:?}", e);
        }
    }

    Ok(())
}
