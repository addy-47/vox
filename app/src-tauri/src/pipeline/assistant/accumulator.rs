use crate::services::tts::actor::TtsClauseChunker;

/// Canonical turn-level textual accumulator and TTS clause chunker.
#[derive(Debug, Clone)]
pub struct TurnAccumulator {
    pub chunker: TtsClauseChunker,
    pub assistant_response: String,
    pub user_transcript: String,
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
            chunker: TtsClauseChunker::new(),
            assistant_response: String::new(),
            user_transcript: String::new(),
        }
    }

    /// Resets all internal buffers to clean state.
    pub fn clear(&mut self) {
        self.chunker.clear();
        self.assistant_response.clear();
        self.user_transcript.clear();
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

    /// Sets the recognized user transcript.
    pub fn set_user_transcript(&mut self, text: String) {
        self.user_transcript = text;
    }

    /// Extracts the full assistant response, leaving an empty string in place.
    pub fn take_assistant_response(&mut self) -> String {
        std::mem::take(&mut self.assistant_response)
    }

    /// Returns a copy of the current user transcript.
    pub fn user_transcript(&self) -> String {
        self.user_transcript.clone()
    }
}
