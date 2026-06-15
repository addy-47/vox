use std::time::Instant;

/// Latency telemetry for a single pipeline turn.
///
/// Every field is an `Option<Instant>` — `None` means that stage hasn't fired yet.
/// Call `latency_report()` at any point to get a JSON snapshot of elapsed ms.
#[derive(Debug, Default)]
pub struct PipelineMetrics {
    pub speech_start: Option<Instant>,
    pub speech_end: Option<Instant>,
    pub first_partial: Option<Instant>,
    pub final_transcript: Option<Instant>,
    pub llm_start: Option<Instant>,
    pub first_token: Option<Instant>,
    pub llm_end: Option<Instant>,
    pub tts_start: Option<Instant>,
    pub first_audio: Option<Instant>,
    pub tts_end: Option<Instant>,
    pub playback_start: Option<Instant>,
    pub playback_finish: Option<Instant>,

    // Memory footprints (MB)
    pub stt_mem_mb: u64,
    pub llm_mem_mb: u64,
    pub tts_mem_mb: u64,

    // Data metrics
    pub input_len_chars: usize,
    pub output_len_chars: usize,
    pub tokens_generated: usize,
}

impl PipelineMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn mark(&mut self, field: MetricField) {
        let now = Instant::now();
        match field {
            MetricField::SpeechStart => self.speech_start.get_or_insert(now),
            MetricField::SpeechEnd => self.speech_end.get_or_insert(now),
            MetricField::FirstPartial => self.first_partial.get_or_insert(now),
            MetricField::FinalTranscript => self.final_transcript.get_or_insert(now),
            MetricField::LlmStart => self.llm_start.get_or_insert(now),
            MetricField::FirstToken => self.first_token.get_or_insert(now),
            MetricField::LlmEnd => self.llm_end.get_or_insert(now),
            MetricField::TtsStart => self.tts_start.get_or_insert(now),
            MetricField::FirstAudio => self.first_audio.get_or_insert(now),
            MetricField::TtsEnd => self.tts_end.get_or_insert(now),
            MetricField::PlaybackStart => self.playback_start.get_or_insert(now),
            MetricField::PlaybackFinish => self.playback_finish.get_or_insert(now),
        };
    }

    pub fn latency_report(
        &self,
        input_duration: f64,
        output_duration: f64,
        mode: crate::core::settings::PipelineMode,
        is_ptt: bool,
    ) -> serde_json::Value {
        let round = |val: f64| (val * 100.0).round() / 100.0;

        let diff_sec = |a: Option<Instant>, b: Option<Instant>| -> Option<f64> {
            match (a, b) {
                (Some(start), Some(end)) => Some(end.duration_since(start).as_secs_f64()),
                _ => None,
            }
        };

        let mem = crate::utils::bench_reporter::BenchReporter::get_memory_snapshot();

        match mode {
            crate::core::settings::PipelineMode::Realtime => {
                let user_speech_end = self.speech_end.or(self.final_transcript);

                // Cloud Latency Metrics
                let ttft = diff_sec(user_speech_end, self.first_token).map(round);
                let ttfa = diff_sec(user_speech_end, self.first_audio).map(round);
                let server_turn = diff_sec(user_speech_end, self.llm_end).map(round);

                // Throughput (Server TPS)
                let server_tps = match (server_turn, self.tokens_generated) {
                    (Some(dur), tokens) if dur > 0.0 => round(tokens as f64 / dur),
                    _ => 0.0,
                };

                let user_speech_dur = diff_sec(self.speech_start, user_speech_end).map(round);

                serde_json::json!({
                    "mode": format!("Realtime/{}", if is_ptt { "PTT" } else { "Passive" }),
                    "latency": {
                        "user_speech_duration_sec": user_speech_dur,
                        "ttft_sec": ttft,
                        "ttfa_sec": ttfa,
                        "server_turn_sec": server_turn,
                        "stt_proc_sec": serde_json::Value::Null,
                        "llm_proc_sec": serde_json::Value::Null,
                        "tts_proc_sec": serde_json::Value::Null
                    },
                    "memory_mb": {
                        "stt": 0,
                        "llm": 0,
                        "tts": 0,
                        "total": mem.rss_mb
                    },
                    "throughput": {
                        "stt_rtf": 0.0,
                        "tts_rtf": 0.0,
                        "llm_tps": 0.0,
                        "server_tps": server_tps
                    },
                    "data": {
                        "input_chars": self.input_len_chars,
                        "output_chars": self.output_len_chars,
                        "tokens": self.tokens_generated
                    },
                    "summary": format!(
                        "Cloud S2S ({}) | TTFT: {}s | TTFA: {}s | Server Turn: {}s | Server TPS: {} | RAM: {}MB",
                        if is_ptt { "PTT" } else { "Passive" },
                        ttft.unwrap_or(0.0),
                        ttfa.unwrap_or(0.0),
                        server_turn.unwrap_or(0.0),
                        server_tps,
                        mem.rss_mb
                    )
                })
            }
            crate::core::settings::PipelineMode::Modular => {
                // Latencies
                let ttft = diff_sec(self.final_transcript, self.first_token).map(round);
                let ttfa = diff_sec(self.final_transcript, self.first_audio).map(round);

                // Step durations
                let stt_duration = diff_sec(self.speech_start, self.final_transcript).unwrap_or(0.0);
                let llm_duration = diff_sec(self.llm_start, self.llm_end).unwrap_or(0.0);
                let tts_duration = diff_sec(self.tts_start, self.tts_end).unwrap_or(0.0);

                // Throughput
                let stt_rtf = if input_duration > 0.0 {
                    round(stt_duration / input_duration)
                } else {
                    0.0
                };
                let tts_rtf = if output_duration > 0.0 {
                    round(tts_duration / output_duration)
                } else {
                    0.0
                };
                let tps = if llm_duration > 0.0 {
                    round(self.tokens_generated as f64 / llm_duration)
                } else {
                    0.0
                };

                let total_mem = if self.stt_mem_mb > 0 || self.llm_mem_mb > 0 || self.tts_mem_mb > 0 {
                    self.stt_mem_mb + self.llm_mem_mb + self.tts_mem_mb
                } else {
                    mem.rss_mb
                };

                serde_json::json!({
                    "mode": format!("Modular/{}", if is_ptt { "PTT" } else { "Passive" }),
                    "latency": {
                        "ttft_sec": ttft,
                        "ttfa_sec": ttfa,
                        "stt_proc_sec": round(stt_duration),
                        "llm_proc_sec": round(llm_duration),
                        "tts_proc_sec": round(tts_duration)
                    },
                    "memory_mb": {
                        "stt": self.stt_mem_mb,
                        "llm": self.llm_mem_mb,
                        "tts": self.tts_mem_mb,
                        "total": total_mem
                    },
                    "throughput": {
                        "stt_rtf": stt_rtf,
                        "tts_rtf": tts_rtf,
                        "llm_tps": tps
                    },
                    "data": {
                        "input_chars": self.input_len_chars,
                        "output_chars": self.output_len_chars,
                        "tokens": self.tokens_generated
                    },
                    "summary": format!(
                        "Local/Hybrid ({}) | TTFT: {}s | TTFA: {}s | STT_RTF: {} | LLM_TPS: {} | TTS_RTF: {} | RAM: {}MB",
                        if is_ptt { "PTT" } else { "Passive" },
                        ttft.unwrap_or(0.0),
                        ttfa.unwrap_or(0.0),
                        stt_rtf,
                        tps,
                        tts_rtf,
                        total_mem
                    )
                })
            }
        }
    }
}

pub enum MetricField {
    SpeechStart,
    SpeechEnd,
    FirstPartial,
    FinalTranscript,
    LlmStart,
    FirstToken,
    LlmEnd,
    TtsStart,
    FirstAudio,
    TtsEnd,
    PlaybackStart,
    PlaybackFinish,
}
