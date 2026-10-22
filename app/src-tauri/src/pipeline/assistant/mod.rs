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
    /// Holds the earlier spoken fragment(s) when the user pauses mid-sentence (`Thinking`),
    /// resumes speaking (`SpeechStart`), and continues the same logical turn.
    pub continuation_prefix: String,
    /// True when a new `SpeechStart` interrupted `Thinking` before the previous segment's
    /// `TranscriptFinal` arrived from the STT worker, allowing `on_transcript_final` to
    /// capture the late-arriving segment into `continuation_prefix` while in `Listening`.
    pub awaiting_continuation: bool,
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
            continuation_prefix: String::new(),
            awaiting_continuation: false,
            next_clause_seq: 0,
        }
    }

    /// Resets all internal buffers (including continuation state) to a clean state.
    pub fn clear(&mut self) {
        self.chunker.clear();
        self.assistant_response.clear();
        self.user_transcript.clear();
        self.continuation_prefix.clear();
        self.awaiting_continuation = false;
        self.next_clause_seq = 0;
    }

    /// Resets assistant output and clause chunker state for a cancelled turn without wiping continuation prefix.
    pub fn reset_assistant_output(&mut self) {
        self.chunker.clear();
        self.assistant_response.clear();
        self.next_clause_seq = 0;
    }

    /// Preserves any recognized user transcript across a mid-thought continuation (`Thinking` -> `Listening`)
    /// while resetting assistant output and clause chunker state from the aborted premature LLM pass.
    pub fn stash_for_continuation(&mut self) {
        self.chunker.clear();
        self.assistant_response.clear();
        self.next_clause_seq = 0;

        let prev = self.user_transcript.trim().to_string();
        self.user_transcript.clear();
        if !prev.is_empty() {
            if self.continuation_prefix.is_empty() {
                self.continuation_prefix = prev;
            } else {
                self.continuation_prefix.push(' ');
                self.continuation_prefix.push_str(&prev);
            }
            self.awaiting_continuation = false;
        } else if self.continuation_prefix.is_empty() {
            // The previous segment's STT Final is still in flight on the STT thread;
            // mark awaiting_continuation so `on_transcript_final` captures it when it lands.
            self.awaiting_continuation = true;
        }
    }

    /// Appends a late-arriving segment transcript into `continuation_prefix`.
    pub fn append_continuation_prefix(&mut self, segment: &str) {
        let trimmed = segment.trim();
        if !trimmed.is_empty() {
            if self.continuation_prefix.is_empty() {
                self.continuation_prefix = trimmed.to_string();
            } else {
                self.continuation_prefix.push(' ');
                self.continuation_prefix.push_str(trimmed);
            }
        }
        self.awaiting_continuation = false;
    }

    /// Merges any stashed `continuation_prefix` with `new_segment` and stores it as `user_transcript`.
    pub fn resolve_merged_transcript(&mut self, new_segment: &str) -> String {
        self.awaiting_continuation = false;
        let trimmed_new = new_segment.trim();
        let prefix = take(&mut self.continuation_prefix);

        let merged = match (!prefix.is_empty(), !trimmed_new.is_empty()) {
            (true, true) => format!("{} {}", prefix, trimmed_new),
            (true, false) => prefix,
            (false, true) => trimmed_new.to_string(),
            (false, false) => String::new(),
        };

        self.user_transcript = merged.clone();
        merged
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

    /// Extracts the recognized user transcript, leaving an empty string in place.
    pub fn take_user_transcript(&mut self) -> String {
        take(&mut self.user_transcript)
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

#[cfg(test)]
mod tests {
    use super::TurnAccumulator;

    /// PURPOSE:  An interrupted turn must keep the words the user already said.
    /// ASSERTS:  The user transcript moves into the continuation prefix.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Clear the user transcript instead of stashing it.
    #[test]
    fn stash_for_continuation_moves_the_user_transcript_into_the_prefix() {
        let mut acc = TurnAccumulator::new();
        acc.set_user_transcript("first part".to_string());

        acc.stash_for_continuation();

        assert_eq!(acc.continuation_prefix, "first part");
        assert!(acc.user_transcript.is_empty());
        assert!(!acc.awaiting_continuation);
    }

    /// PURPOSE:  When nothing was recognised yet the late final must still be caught.
    /// ASSERTS:  The accumulator awaits the late final.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Drop the `awaiting_continuation` flag.
    #[test]
    fn stash_for_continuation_awaits_a_late_final() {
        let mut acc = TurnAccumulator::new();

        acc.stash_for_continuation();

        assert!(acc.continuation_prefix.is_empty());
        assert!(acc.awaiting_continuation);
    }

    /// PURPOSE:  A second stashed fragment must not overwrite the first.
    /// ASSERTS:  The prefix holds both fragments separated by a space.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Replace the prefix instead of appending to it.
    #[test]
    fn stash_for_continuation_appends_to_an_existing_prefix() {
        let mut acc = TurnAccumulator::new();
        acc.set_user_transcript("second part".to_string());
        acc.stash_for_continuation();
        acc.set_user_transcript("third part".to_string());

        acc.stash_for_continuation();

        assert_eq!(acc.continuation_prefix, "second part third part");
    }

    /// PURPOSE:  A late final must be appended, not dropped.
    /// ASSERTS:  The late segment joins the prefix with a space.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Ignore the segment argument.
    #[test]
    fn append_continuation_prefix_appends_the_late_segment() {
        let mut acc = TurnAccumulator::new();
        acc.stash_for_continuation();

        acc.append_continuation_prefix("late words");

        assert_eq!(acc.continuation_prefix, "late words");
        assert!(!acc.awaiting_continuation);
    }

    /// PURPOSE:  The stashed prefix and the new words must become one query.
    /// ASSERTS:  The merged text holds both parts in order.
    /// CLASS:    happy-path
    /// REQUIRES: none
    /// MUTANT:   Return only the new segment.
    #[test]
    fn resolve_merged_transcript_joins_the_prefix_and_the_new_text() {
        let mut acc = TurnAccumulator::new();
        acc.append_continuation_prefix("first part");

        let merged = acc.resolve_merged_transcript("second part");

        assert_eq!(merged, "first part second part");
        assert_eq!(acc.user_transcript, "first part second part");
        assert!(acc.continuation_prefix.is_empty());
    }

    /// PURPOSE:  An empty new turn must promote the prefix, not abort the turn.
    /// ASSERTS:  The merged text is the prefix alone.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Return an empty string when the new text is empty.
    #[test]
    fn resolve_merged_transcript_promotes_the_prefix() {
        let mut acc = TurnAccumulator::new();
        acc.append_continuation_prefix("first part");

        let merged = acc.resolve_merged_transcript("   ");

        assert_eq!(merged, "first part");
        assert_eq!(acc.user_transcript, "first part");
    }

    /// PURPOSE:  A new turn must not inherit the previous turn's continuation state.
    /// ASSERTS:  Clear resets the prefix, the transcript and the awaiting flag.
    /// CLASS:    edge-case
    /// REQUIRES: none
    /// MUTANT:   Leave the continuation prefix in place.
    #[test]
    fn clear_resets_the_continuation_state() {
        let mut acc = TurnAccumulator::new();
        acc.append_continuation_prefix("first part");
        acc.set_user_transcript("second part".to_string());

        acc.clear();

        assert!(acc.continuation_prefix.is_empty());
        assert!(acc.user_transcript.is_empty());
        assert!(!acc.awaiting_continuation);
    }
}
