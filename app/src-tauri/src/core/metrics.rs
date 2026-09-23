use std::{
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use parking_lot::Mutex;

use crate::{core::events::TurnMetricsPayload, monitoring::telemetry::TelemetryState};

/// Tracks timestamp milestones and computes exact latencies for assistant conversational turns.
#[derive(Debug, Default)]
pub struct TurnMetricsCollector {
    pub current_turn_id: AtomicU32,
    pub speech_start_ms: AtomicU64,
    pub speech_end_ms: AtomicU64,
    pub transcript_final_ms: AtomicU64,
    pub retrieval_duration_ms: AtomicU64,
    pub llm_dispatch_ms: AtomicU64,
    pub llm_first_token_ms: AtomicU64,
    pub llm_finish_ms: AtomicU64,
    pub llm_tokens: AtomicU32,
    pub llm_chars: AtomicU32,
    pub context_tokens_used: AtomicU32,
    pub context_window: AtomicU32,
    pub tool_name: Mutex<Option<String>>,
    pub tool_duration_ms: AtomicU64,
    pub tool_error: AtomicBool,
    pub tts_chunk0_dispatch_ms: AtomicU64,
    pub tts_first_audio_ms: AtomicU64,
    pub tts_last_chunk_finish_ms: AtomicU64,
    pub tts_jobs_completed: AtomicU32,
    pub tts_total_samples: AtomicU64,
    pub playback_start_ms: AtomicU64,
    pub playback_finish_ms: AtomicU64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl TurnMetricsCollector {
    /// Creates a new TurnMetricsCollector with zeroed timestamps.
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets all milestone timestamps for a fresh conversational turn.
    pub fn start_turn(&self, turn_id: u32) {
        self.current_turn_id.store(turn_id, Ordering::Relaxed);
        self.speech_start_ms.store(now_ms(), Ordering::Relaxed);
        self.speech_end_ms.store(0, Ordering::Relaxed);
        self.transcript_final_ms.store(0, Ordering::Relaxed);
        self.retrieval_duration_ms.store(0, Ordering::Relaxed);
        self.llm_dispatch_ms.store(0, Ordering::Relaxed);
        self.llm_first_token_ms.store(0, Ordering::Relaxed);
        self.llm_finish_ms.store(0, Ordering::Relaxed);
        self.llm_tokens.store(0, Ordering::Relaxed);
        self.llm_chars.store(0, Ordering::Relaxed);
        self.context_tokens_used.store(0, Ordering::Relaxed);
        self.context_window.store(0, Ordering::Relaxed);
        *self.tool_name.lock() = None;
        self.tool_duration_ms.store(0, Ordering::Relaxed);
        self.tool_error.store(false, Ordering::Relaxed);
        self.tts_chunk0_dispatch_ms.store(0, Ordering::Relaxed);
        self.tts_first_audio_ms.store(0, Ordering::Relaxed);
        self.tts_last_chunk_finish_ms.store(0, Ordering::Relaxed);
        self.tts_jobs_completed.store(0, Ordering::Relaxed);
        self.tts_total_samples.store(0, Ordering::Relaxed);
        self.playback_start_ms.store(0, Ordering::Relaxed);
        self.playback_finish_ms.store(0, Ordering::Relaxed);
    }

    /// Records the user speech completion boundary.
    pub fn record_speech_end(&self) {
        self.speech_end_ms.store(now_ms(), Ordering::Relaxed);
    }

    /// Records when the STT engine finalized the transcript.
    pub fn record_transcript_final(&self) {
        let ts = now_ms();
        self.transcript_final_ms.store(ts, Ordering::Relaxed);
        if self.speech_end_ms.load(Ordering::Relaxed) == 0 {
            self.speech_end_ms.store(ts, Ordering::Relaxed);
        }
    }

    /// Records time spent on context or episodic memory retrieval.
    pub fn record_retrieval(&self, duration_ms: u64) {
        self.retrieval_duration_ms
            .store(duration_ms, Ordering::Relaxed);
    }

    /// Records when request is dispatched to the LLM worker.
    pub fn record_llm_dispatch(&self) {
        self.llm_dispatch_ms.store(now_ms(), Ordering::Relaxed);
    }

    /// Records the first received token or tool call packet.
    pub fn record_llm_first_token(&self) {
        let _ = self.llm_first_token_ms.compare_exchange(
            0,
            now_ms(),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    /// Records when the LLM stream / generation completed.
    pub fn record_llm_finish(&self) {
        self.llm_finish_ms.store(now_ms(), Ordering::Relaxed);
    }

    /// Records LLM generated token and character counts.
    pub fn record_llm_stats(&self, chars: usize, tokens: usize) {
        self.llm_chars.store(chars as u32, Ordering::Relaxed);
        self.llm_tokens.store(tokens as u32, Ordering::Relaxed);
    }

    /// Records start of tool execution.
    pub fn record_tool_start(&self, name: &str) {
        *self.tool_name.lock() = Some(name.to_string());
    }

    /// Records completion of tool execution.
    pub fn record_tool_finish(&self, duration_ms: u64, is_error: bool) {
        self.tool_duration_ms.store(duration_ms, Ordering::Relaxed);
        self.tool_error.store(is_error, Ordering::Relaxed);
    }

    /// Records when chunk 0 was dispatched to the TTS actor.
    pub fn record_tts_chunk0_dispatch(&self) {
        let _ = self.tts_chunk0_dispatch_ms.compare_exchange(
            0,
            now_ms(),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    /// Records when the first audio samples were yielded by the TTS engine.
    pub fn record_tts_first_audio(&self) {
        let _ = self.tts_first_audio_ms.compare_exchange(
            0,
            now_ms(),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    /// Accumulates count of synthesized audio samples received in playback buffer.
    pub fn record_tts_samples(&self, samples: u64) {
        self.tts_total_samples.fetch_add(samples, Ordering::Relaxed);
    }

    /// Records completion of a TTS synthesis job.
    pub fn record_tts_job_completed(&self, is_last: bool) {
        self.tts_jobs_completed.fetch_add(1, Ordering::Relaxed);
        if is_last {
            self.tts_last_chunk_finish_ms
                .store(now_ms(), Ordering::Relaxed);
        }
    }

    /// Records the context tokens tracked and the total context window size.
    pub fn record_context_budget(&self, used_tokens: usize, window: usize) {
        self.context_tokens_used
            .store(used_tokens as u32, Ordering::Relaxed);
        self.context_window.store(window as u32, Ordering::Relaxed);
    }

    /// Builds a strongly-typed turn metrics payload reflecting current turn milestones and token utilization.
    pub fn build_payload(&self, turn_id: u32) -> TurnMetricsPayload {
        let speech_end = self.speech_end_ms.load(Ordering::Relaxed);
        let llm_dispatch = self.llm_dispatch_ms.load(Ordering::Relaxed);
        let llm_first_token = self.llm_first_token_ms.load(Ordering::Relaxed);
        let tts_dispatch = self.tts_chunk0_dispatch_ms.load(Ordering::Relaxed);
        let tts_first_audio = self.tts_first_audio_ms.load(Ordering::Relaxed);
        let playback_start = self.playback_start_ms.load(Ordering::Relaxed);

        let ttft_ms = if llm_first_token > llm_dispatch && llm_dispatch > 0 {
            (llm_first_token - llm_dispatch) as u32
        } else {
            0
        };

        let ttfa_ms = if tts_first_audio > tts_dispatch && tts_dispatch > 0 {
            (tts_first_audio - tts_dispatch) as u32
        } else {
            0
        };

        let total_voice_latency_ms = if speech_end > 0
            && playback_start > speech_end
            && (playback_start - speech_end <= 30000)
        {
            (playback_start - speech_end) as u32
        } else if llm_dispatch > 0 && playback_start > llm_dispatch {
            (playback_start - llm_dispatch) as u32
        } else {
            0
        };

        let prompt_tokens = self.context_tokens_used.load(Ordering::Relaxed);
        let completion_tokens = self.llm_tokens.load(Ordering::Relaxed);
        let total_context_used = prompt_tokens + completion_tokens;
        let context_win = self.context_window.load(Ordering::Relaxed);

        TurnMetricsPayload {
            turn_id,
            ttft_ms,
            ttfa_ms,
            total_voice_latency_ms,
            context_tokens_used: total_context_used,
            context_window: context_win,
        }
    }

    /// Records playback start, logs the complete timing breakdown, and updates telemetry.
    pub fn record_playback_started(
        &self,
        turn_id: u32,
        telemetry: &TelemetryState,
    ) -> TurnMetricsPayload {
        let playback_ts = now_ms();
        self.playback_start_ms.store(playback_ts, Ordering::Relaxed);

        let speech_end = self.speech_end_ms.load(Ordering::Relaxed);
        let transcript_final = self.transcript_final_ms.load(Ordering::Relaxed);
        let retrieval_dur = self.retrieval_duration_ms.load(Ordering::Relaxed);
        let llm_dispatch = self.llm_dispatch_ms.load(Ordering::Relaxed);
        let llm_first_token = self.llm_first_token_ms.load(Ordering::Relaxed);
        let llm_finish = self.llm_finish_ms.load(Ordering::Relaxed);
        let tts_dispatch = self.tts_chunk0_dispatch_ms.load(Ordering::Relaxed);
        let tts_first_audio = self.tts_first_audio_ms.load(Ordering::Relaxed);

        let stt_ms = if transcript_final > speech_end
            && speech_end > 0
            && (transcript_final - speech_end <= 15000)
        {
            transcript_final - speech_end
        } else {
            0
        };

        let dispatch_overhead_ms = if llm_dispatch > transcript_final && transcript_final > 0 {
            llm_dispatch - transcript_final
        } else {
            0
        };

        let ttft_ms = if llm_first_token > llm_dispatch && llm_dispatch > 0 {
            llm_first_token - llm_dispatch
        } else {
            0
        };

        let llm_duration_ms = if llm_finish > llm_dispatch && llm_dispatch > 0 {
            llm_finish - llm_dispatch
        } else {
            0
        };

        let tts_dispatch_latency_ms = if tts_dispatch > llm_first_token && llm_first_token > 0 {
            tts_dispatch - llm_first_token
        } else {
            0
        };

        let tts_first_audio_synthesis_ms = if tts_first_audio > tts_dispatch && tts_dispatch > 0 {
            tts_first_audio - tts_dispatch
        } else {
            0
        };

        let preroll_delay_ms = if playback_ts > tts_first_audio && tts_first_audio > 0 {
            playback_ts - tts_first_audio
        } else {
            0
        };

        let perceived_latency_ms = if llm_finish > 0 && playback_ts > llm_finish {
            playback_ts - llm_finish
        } else {
            0
        };

        let total_voice_latency_ms =
            if speech_end > 0 && playback_ts > speech_end && (playback_ts - speech_end <= 30000) {
                playback_ts - speech_end
            } else if llm_dispatch > 0 && playback_ts > llm_dispatch {
                playback_ts - llm_dispatch
            } else {
                0
            };

        if stt_ms > 0 {
            telemetry
                .latest_stt_ms
                .store(stt_ms as u32, Ordering::Relaxed);
        }
        if ttft_ms > 0 {
            telemetry
                .latest_ttft_ms
                .store(ttft_ms as u32, Ordering::Relaxed);
        }
        if perceived_latency_ms > 0 {
            telemetry
                .latest_playback_start_ms
                .store(perceived_latency_ms as u32, Ordering::Relaxed);
        }
        if total_voice_latency_ms > 0 {
            telemetry
                .latest_voice_latency_ms
                .store(total_voice_latency_ms as u32, Ordering::Relaxed);
        }

        let perceived_str = if llm_finish > 0 {
            format!("{}ms", perceived_latency_ms)
        } else {
            "streamed concurrently with LLM".to_string()
        };

        let tokens = self.llm_tokens.load(Ordering::Relaxed);
        let chars = self.llm_chars.load(Ordering::Relaxed);
        let tok_per_sec = if llm_duration_ms > 0 && tokens > 0 {
            (tokens as f32) / (llm_duration_ms as f32 / 1000.0)
        } else {
            0.0
        };

        let tool_lock = self.tool_name.lock();
        let tool_line = if let Some(ref name) = *tool_lock {
            let dur = self.tool_duration_ms.load(Ordering::Relaxed);
            let is_err = self.tool_error.load(Ordering::Relaxed);
            format!(
                "  ├─ Tool Execution:            {}ms [{}]{}\n",
                dur,
                name,
                if is_err { " (FAILED)" } else { "" }
            )
        } else {
            String::new()
        };
        drop(tool_lock);

        let retrieval_line = if retrieval_dur > 0 {
            format!("  ├─ Memory/Context Retrieval:  {}ms\n", retrieval_dur)
        } else {
            String::new()
        };

        log::info!(
            "[Pipeline::Metrics] ── Turn {} Onset Breakdown ──────────────────────────────────\n\
  ├─ STT / Transcription:       {}ms (speech end -> transcript final)\n\
{}\
  ├─ Dispatch Overhead:         {}ms (transcript final -> LLM dispatch)\n\
  ├─ LLM TTFT (First Token):     {}ms (LLM dispatch -> first token)\n\
  ├─ LLM Total Generation:      {}ms (LLM dispatch -> LLM finish)\n\
  ├─ LLM Throughput:            {} tokens, {} chars ({:.1} tok/s)\n\
{}\
  ├─ TTS Chunk 0 Dispatch:      {}ms (first token -> TTS queue)\n\
  ├─ TTS First PCM Slice:       {}ms (TTS dispatch -> first audio in buffer)\n\
  ├─ Preroll Cushion Delay:     {}ms (first audio in buffer -> speaker unmute)\n\
  ├─ Perceived Response Delay:  {}\n\
  └─ Total Voice Latency:       {}ms (user speech end -> speaker playing)\n\
──────────────────────────────────────────────────────────────────────────",
            turn_id,
            stt_ms,
            retrieval_line,
            dispatch_overhead_ms,
            ttft_ms,
            llm_duration_ms,
            tokens,
            chars,
            tok_per_sec,
            tool_line,
            tts_dispatch_latency_ms,
            tts_first_audio_synthesis_ms,
            preroll_delay_ms,
            perceived_str,
            total_voice_latency_ms
        );

        self.build_payload(turn_id)
    }

    /// Records playback finish and logs turn audio completion summary.
    pub fn record_playback_finished(&self, turn_id: u32) {
        let ts = now_ms();
        self.playback_finish_ms.store(ts, Ordering::Relaxed);

        let playback_start = self.playback_start_ms.load(Ordering::Relaxed);
        let playback_dur_ms = if ts > playback_start && playback_start > 0 {
            ts - playback_start
        } else {
            0
        };

        let tts_chunk0_dispatch = self.tts_chunk0_dispatch_ms.load(Ordering::Relaxed);
        let tts_last_chunk_finish = self.tts_last_chunk_finish_ms.load(Ordering::Relaxed);
        let tts_total_synthesis_ms =
            if tts_last_chunk_finish > tts_chunk0_dispatch && tts_chunk0_dispatch > 0 {
                tts_last_chunk_finish - tts_chunk0_dispatch
            } else {
                0
            };

        let samples = self.tts_total_samples.load(Ordering::Relaxed);
        let audio_dur_sec = samples as f32 / 24000.0;
        let jobs_count = self.tts_jobs_completed.load(Ordering::Relaxed);

        let speech_start = self.speech_start_ms.load(Ordering::Relaxed);
        let total_turn_sec = if ts > speech_start && speech_start > 0 {
            (ts - speech_start) as f32 / 1000.0
        } else {
            0.0
        };

        log::info!(
            "[Pipeline::Metrics] ── Turn {} Completion Summary ──────────────────────────────\n\
  ├─ TTS Completed Jobs:        {} chunk(s)\n\
  ├─ TTS Total Audio Generated: {:.2}s ({} samples @ 24kHz)\n\
  ├─ TTS Total Synthesis Time:  {}ms\n\
  ├─ Playback Active Duration:  {}ms\n\
  └─ End-to-End Turn Duration:  {:.2}s (speech start -> playback complete)\n\
──────────────────────────────────────────────────────────────────────────",
            turn_id,
            jobs_count,
            audio_dur_sec,
            samples,
            tts_total_synthesis_ms,
            playback_dur_ms,
            total_turn_sec
        );
    }
}
