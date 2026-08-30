#[derive(Debug, Clone)]
pub enum VoxEvent {
    SpeechStart {
        turn_id: u32,
    },
    SpeechEnd {
        turn_id: u32,
        audio_buffer: Vec<f32>,
    },
    TranscriptPartial {
        turn_id: u32,
        text: String,
    },
    TranscriptFinal {
        turn_id: u32,
        text: String,
    },
    LlmToken {
        turn_id: u32,
        token: String,
    },
    LlmFinished {
        turn_id: u32,
    },
    TtsChunk {
        turn_id: u32,
        samples: Vec<f32>,
    },
    TtsFinished {
        turn_id: u32,
        rtf: f32,
    },
    PlaybackStarted {
        turn_id: u32,
    },
    PlaybackFinished {
        turn_id: u32,
    },
    Cancelled {
        turn_id: u32,
    },
    Error {
        turn_id: u32,
        message: String,
    },
    Shutdown,
}
