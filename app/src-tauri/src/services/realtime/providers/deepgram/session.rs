use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, bail, Result};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::services::realtime::{
    transport::{FrameAction, ProviderDriver},
    Actionability, OutboundCommand, PipelineImpact, RealtimeProviderEvent, RealtimeSession,
    WS_KEEPALIVE_INTERVAL,
};

pub(super) struct DeepgramSessionState {
    pub(super) last_assistant_text: String,
    pub(super) turn_id: Arc<AtomicU32>,
    pub(super) server_turn_cursor: Option<u32>,
}

impl DeepgramSessionState {
    pub(super) fn current_or_new_turn_id(&mut self) -> u32 {
        if let Some(id) = self.server_turn_cursor {
            id
        } else {
            let id = self.turn_id.load(Ordering::Relaxed);
            self.server_turn_cursor = Some(id);
            id
        }
    }

    pub(super) fn peek_or_current_turn_id(&self) -> u32 {
        self.server_turn_cursor
            .unwrap_or_else(|| self.turn_id.load(Ordering::Relaxed))
    }
}

pub(crate) struct DeepgramDriver {
    pub(super) state: Arc<Mutex<DeepgramSessionState>>,
}

impl ProviderDriver for DeepgramDriver {
    fn encode(&self, cmd: OutboundCommand) -> Option<Message> {
        match cmd {
            OutboundCommand::Audio(pcm) => {
                let bytes: Vec<u8> = pcm.iter().flat_map(|&s| s.to_le_bytes()).collect();
                Some(Message::Binary(bytes.into()))
            }
            OutboundCommand::Interrupt => {
                let msg = serde_json::json!({ "type": "Clear" }).to_string();
                Some(Message::Text(msg.into()))
            }
            OutboundCommand::KeepAlive => {
                let msg = serde_json::json!({ "type": "KeepAlive" }).to_string();
                Some(Message::Text(msg.into()))
            }
            OutboundCommand::ActivityStart | OutboundCommand::ActivityEnd => None,
        }
    }

    fn handle_frame(
        &self,
        msg: Message,
        event_tx: &mpsc::Sender<RealtimeProviderEvent>,
    ) -> FrameAction {
        match msg {
            Message::Text(text) => {
                if let Err(e) = dispatch_deepgram_server_message(&text, event_tx, &self.state) {
                    log::error!("[DeepgramVoiceAgent] Message handling error: {:?}", e);
                }
            }
            Message::Binary(bytes) => {
                let pcm: Vec<i16> = bytes
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();
                if let Err(e) = event_tx.try_send(RealtimeProviderEvent::AudioChunk(pcm)) {
                    log::warn!("[DeepgramVoiceAgent] Failed to forward AudioChunk: {:?}", e);
                }
            }
            _ => {}
        }
        FrameAction::Continue
    }

    fn keepalive_interval(&self) -> Option<Duration> {
        Some(WS_KEEPALIVE_INTERVAL)
    }
}

pub(crate) struct DeepgramVoiceAgentSession {
    pub(super) outbound_tx: mpsc::Sender<OutboundCommand>,
    pub(super) shutdown_tx: parking_lot::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub(super) terminated: Arc<AtomicBool>,
}

impl RealtimeSession for DeepgramVoiceAgentSession {
    fn send_audio(&self, pcm: &[i16]) -> Result<()> {
        if self.terminated.load(Ordering::Relaxed) {
            bail!("Deepgram Voice Agent session is terminated");
        }
        if let Err(tokio::sync::mpsc::error::TrySendError::Full(_)) = self
            .outbound_tx
            .try_send(OutboundCommand::Audio(pcm.to_vec()))
        {
            log::warn!("[DeepgramVoiceAgent] Audio queue full — dropped frame");
        }
        Ok(())
    }

    fn commit_speech_turn(&self, pcm: &[i16]) -> Result<()> {
        self.send_audio(pcm)
    }

    fn cancel(&self) -> Result<()> {
        if self.terminated.load(Ordering::Relaxed) {
            bail!("Deepgram Voice Agent session is terminated");
        }
        self.outbound_tx
            .try_send(OutboundCommand::Interrupt)
            .map_err(|e| anyhow!("Failed to send interrupt control event: {:?}", e))
    }

    fn disconnect(&self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.lock().take() {
            if let Err(e) = tx.send(()) {
                log::warn!("[DeepgramVoiceAgent] Shutdown signal drop: {:?}", e);
            }
        }
        Ok(())
    }
}

fn dispatch_deepgram_server_message(
    text: &str,
    provider_event_tx: &mpsc::Sender<RealtimeProviderEvent>,
    state: &Arc<Mutex<DeepgramSessionState>>,
) -> Result<()> {
    let val: serde_json::Value = serde_json::from_str(text)?;

    if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
        log::trace!("[DeepgramVoiceAgent] Inbound message type: {}", msg_type);
        match msg_type {
            "UserStartedSpeaking" => {
                log::info!("[DeepgramVoiceAgent] User started speaking (barge-in).");
                let mut s_lock = state.lock();
                s_lock.last_assistant_text.clear();
                s_lock.server_turn_cursor = None;
                if let Err(e) = provider_event_tx.try_send(RealtimeProviderEvent::SpeechStart) {
                    log::warn!(
                        "[DeepgramVoiceAgent] Failed to forward SpeechStart event: {:?}",
                        e
                    );
                }
            }
            "UserStoppedSpeaking" => {
                log::info!("[DeepgramVoiceAgent] User stopped speaking.");
                if let Err(e) = provider_event_tx.try_send(RealtimeProviderEvent::SpeechEnd) {
                    log::warn!(
                        "[DeepgramVoiceAgent] Failed to forward SpeechEnd event: {:?}",
                        e
                    );
                }
            }
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
                    if let Err(e) =
                        provider_event_tx.try_send(RealtimeProviderEvent::TranscriptFinal {
                            turn_id,
                            text: content.to_string(),
                        })
                    {
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
                            if let Err(e) =
                                provider_event_tx.try_send(RealtimeProviderEvent::LlmToken {
                                    turn_id,
                                    token: delta.to_string(),
                                })
                            {
                                log::warn!(
                                    "[DeepgramVoiceAgent] Failed to forward LlmToken event: {:?}",
                                    e
                                );
                            }
                        }
                    } else if let Err(e) =
                        provider_event_tx.try_send(RealtimeProviderEvent::LlmToken {
                            turn_id,
                            token: content.to_string(),
                        })
                    {
                        log::warn!(
                            "[DeepgramVoiceAgent] Failed to forward LlmToken event: {:?}",
                            e
                        );
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
                if let Err(e) = provider_event_tx.try_send(RealtimeProviderEvent::LlmFinished {
                    turn_id: finished_turn_id,
                }) {
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
                    let is_auth = err_msg.contains("401")
                        || err_msg.contains("Unauthorized")
                        || err_msg.contains("API key");
                    let (impact, actionability) = if is_auth {
                        (
                            PipelineImpact::SessionHalted,
                            Actionability::Actionable {
                                category: "auth_failure".to_string(),
                                hint: "Deepgram API key is invalid or expired. Update in Settings."
                                    .to_string(),
                            },
                        )
                    } else {
                        (PipelineImpact::TurnAborted, Actionability::None)
                    };
                    if let Err(e) = provider_event_tx.try_send(RealtimeProviderEvent::Error {
                        turn_id: err_turn_id,
                        message: format!("Deepgram server error: {}", err_msg),
                        impact,
                        actionability,
                    }) {
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
