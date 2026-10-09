pub mod error;
pub mod interrupt;
pub mod llm;
pub mod playback;
pub mod ptt;
pub mod session;
pub mod speech;
pub mod text;
pub mod transcript;

use std::mem::take;

use crate::services::harness::stages::streaming::ClauseChunker;

/// Canonical turn-level textual accumulator and TTS clause chunker.
#[derive(Debug, Clone)]
pub struct TurnAccumulator {
    pub chunker: ClauseChunker,
    pub assistant_response: String,
    pub user_transcript: String,
    next_clause_seq: u32,
}

impl Default for TurnAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl TurnAccumulator {
    /// Creates a new empty TurnAccumulator.
    pub fn new() -> Self {
        Self {
            chunker: ClauseChunker::new(),
            assistant_response: String::new(),
            user_transcript: String::new(),
            next_clause_seq: 0,
        }
    }

    /// Resets all internal buffers to clean state.
    pub fn clear(&mut self) {
        self.chunker.clear();
        self.assistant_response.clear();
        self.user_transcript.clear();
        self.next_clause_seq = 0;
    }

    /// Appends incoming token to assistant response and extracts speakable clauses.
    pub fn push_token(&mut self, token: &str) -> Vec<String> {
        self.assistant_response.push_str(token);
        self.chunker.push_str(token)
    }

    /// Flushes any remaining unpunctuated text from the clause chunker.
    pub fn flush_chunker(&mut self) -> Option<String> {
        self.chunker.flush()
    }

    /// Claims a contiguous block of TTS clause sequence ids for dispatched clauses.
    pub fn claim_clause_ids(&mut self, count: usize) -> u32 {
        let first = self.next_clause_seq;
        self.next_clause_seq += count as u32;
        first
    }

    /// Sets the recognized user transcript.
    pub fn set_user_transcript(&mut self, text: String) {
        self.user_transcript = text;
    }

    /// Extracts the full assistant response, leaving an empty string in place.
    pub fn take_assistant_response(&mut self) -> String {
        take(&mut self.assistant_response)
    }

    /// Returns a copy of the current user transcript.
    pub fn user_transcript(&self) -> String {
        self.user_transcript.clone()
    }
}

/// Main event dispatcher for the assistant domain.
pub fn handle_event<R: tauri::Runtime + 'static>(
    app: &tauri::AppHandle<R>,
    state: &crate::core::state::AppState,
    ctx: &crate::pipeline::router::RoutingContext,
    event: crate::core::events::VoxEvent,
) {
    use crate::core::{events::VoxEvent, state::InteractionOwner};

    match event {
        VoxEvent::SessionStart { owner, session_id } => {
            session::on_session_start(owner, session_id, app, state, ctx);
        }
        VoxEvent::PauseSession => session::on_pause(app, state, ctx),
        VoxEvent::ResumeSession => session::on_resume(app, state, ctx),
        VoxEvent::EndSession => session::on_end(app, state, ctx),

        VoxEvent::PttStart {
            owner: InteractionOwner::Assistant,
        } => {
            ptt::on_ptt_start(app, state, ctx);
        }
        VoxEvent::PttStop {
            owner: InteractionOwner::Assistant,
        } => {
            ptt::on_ptt_stop(app, state, ctx);
        }
        VoxEvent::PttCancel {
            owner: InteractionOwner::Assistant,
        } => {
            ptt::on_ptt_cancel(app, state, ctx);
        }
        VoxEvent::SpeechStart {
            owner: InteractionOwner::Assistant,
        } => {
            speech::on_speech_start(app, state, ctx);
        }
        VoxEvent::SpeechEnd {
            owner: InteractionOwner::Assistant,
        } => {
            speech::on_speech_end(app, state, ctx);
        }
        VoxEvent::TranscriptFinal {
            owner: InteractionOwner::Assistant,
            turn_id,
            text,
        } => {
            transcript::on_transcript_final(turn_id, text, app, state, ctx);
        }
        VoxEvent::Cancelled {
            owner: InteractionOwner::Assistant,
            turn_id,
        } => {
            error::on_cancelled(turn_id, app, state, ctx);
        }

        VoxEvent::TextInput { text } => {
            text::on_text_input(text, app, state, ctx);
        }
        VoxEvent::LlmFinished { turn_id } => {
            llm::on_llm_finished(turn_id, Some(app), state, ctx);
        }
        VoxEvent::PlaybackStarted { turn_id, intent } => {
            playback::on_playback_started(turn_id, intent, app, state, ctx);
        }
        VoxEvent::PlaybackFinished { turn_id, intent } => {
            playback::on_playback_finished(turn_id, intent, app, state, ctx);
        }
        _ => {}
    }
}
