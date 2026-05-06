use std::time::Instant;

/// Latency telemetry for a single pipeline turn.
///
/// Every field is an `Option<Instant>` — `None` means that stage hasn't fired yet.
/// Call `latency_report()` at any point to get a JSON snapshot of elapsed ms.
#[derive(Debug, Default)]
pub struct PipelineMetrics {
    pub speech_start:     Option<Instant>,
    pub first_partial:    Option<Instant>,
    pub final_transcript: Option<Instant>,
    pub llm_start:        Option<Instant>,
    pub first_token:      Option<Instant>,
    pub tts_start:        Option<Instant>,
    pub first_audio:      Option<Instant>,
    pub playback_start:   Option<Instant>,
    pub playback_finish:  Option<Instant>,
}

impl PipelineMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all timestamps for a new turn.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Record a stage timestamp the first time it's called (idempotent).
    pub fn mark(&mut self, field: MetricField) {
        let now = Instant::now();
        match field {
            MetricField::SpeechStart     => self.speech_start     .get_or_insert(now),
            MetricField::FirstPartial    => self.first_partial    .get_or_insert(now),
            MetricField::FinalTranscript => self.final_transcript .get_or_insert(now),
            MetricField::LlmStart        => self.llm_start        .get_or_insert(now),
            MetricField::FirstToken      => self.first_token      .get_or_insert(now),
            MetricField::TtsStart        => self.tts_start        .get_or_insert(now),
            MetricField::FirstAudio      => self.first_audio      .get_or_insert(now),
            MetricField::PlaybackStart   => self.playback_start   .get_or_insert(now),
            MetricField::PlaybackFinish  => self.playback_finish  .get_or_insert(now),
        };
    }

    /// Returns a JSON-compatible summary of all measured latencies in milliseconds.
    pub fn latency_report(&self) -> serde_json::Value {
        let ms = |a: Option<Instant>, b: Option<Instant>| -> serde_json::Value {
            match (a, b) {
                (Some(start), Some(end)) => {
                    let millis = end.duration_since(start).as_secs_f64() * 1000.0;
                    serde_json::json!(millis)
                }
                _ => serde_json::Value::Null,
            }
        };

        serde_json::json!({
            "speech_to_first_partial_ms":    ms(self.speech_start,     self.first_partial),
            "speech_to_final_ms":            ms(self.speech_start,     self.final_transcript),
            "transcript_to_llm_start_ms":    ms(self.final_transcript, self.llm_start),
            "llm_to_first_token_ms":         ms(self.llm_start,        self.first_token),
            "first_token_to_tts_ms":         ms(self.first_token,      self.tts_start),
            "tts_to_first_audio_ms":         ms(self.tts_start,        self.first_audio),
            "first_audio_to_playback_ms":    ms(self.first_audio,      self.playback_start),
            "total_voice_to_voice_ms":       ms(self.speech_start,     self.playback_start),
            "playback_duration_ms":          ms(self.playback_start,   self.playback_finish),
        })
    }
}

pub enum MetricField {
    SpeechStart,
    FirstPartial,
    FinalTranscript,
    LlmStart,
    FirstToken,
    TtsStart,
    FirstAudio,
    PlaybackStart,
    PlaybackFinish,
}
