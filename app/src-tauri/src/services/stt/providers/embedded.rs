//! Embedded STT provider — wraps the local Qwen3-ASR and Nemotron-3.5 ONNX engines
//! behind a single `SttProvider` interface.
//!
//! This provider owns the stride-buffering and transcript-stitching logic that was
//! previously inline in `actor.rs`, keeping the actor engine-agnostic.

use super::{SttProvider, SttProviderKind};
use crate::services::stt::SttEngine;
use std::sync::Mutex;

// ─── Internal State ────────────────────────────────────────────────────────────

struct EmbeddedSttProviderInner {
    /// Nemotron engine, if active.
    nemotron_engine: Option<super::super::nemotron_onnx::SttEngine>,
    /// Qwen engine, if active.
    qwen_engine: Option<super::super::qwen_onnx::SttEngine>,

    // ── Stride buffering state (Nemotron only) ─────────────────────────────
    /// Accumulated audio samples not yet consumed by the engine.
    stt_audio_buffer: Vec<f32>,
    /// How many samples of the total turn utterance have been appended to the buffer.
    consumed_samples: usize,

    // ── Transcript stitching state (Qwen) / accumulation (Nemotron) ────────
    /// Full accumulated transcript for the current turn.
    stitched_transcript: String,
}

// ─── Provider ─────────────────────────────────────────────────────────────────

pub struct EmbeddedSttProvider {
    engine_type: String,
    inner: Mutex<EmbeddedSttProviderInner>,
}

impl EmbeddedSttProvider {
    /// Create a new embedded provider.
    ///
    /// `model_type`: `"nvidia_nemotron"` or any other value (treated as Qwen3-ASR).
    /// `model_path`: path to the model directory.
    pub fn new(model_path: &std::path::Path, model_type: &str) -> anyhow::Result<Self> {
        let (nemotron_engine, qwen_engine) = match model_type {
            "nvidia_nemotron" => {
                let engine = super::super::nemotron_onnx::SttEngine::new(model_path)?;
                (Some(engine), None)
            }
            _ => {
                let engine = super::super::qwen_onnx::SttEngine::new(model_path)?;
                (None, Some(engine))
            }
        };

        Ok(Self {
            engine_type: model_type.to_string(),
            inner: Mutex::new(EmbeddedSttProviderInner {
                nemotron_engine,
                qwen_engine,
                stt_audio_buffer: Vec::new(),
                consumed_samples: 0,
                stitched_transcript: String::new(),
            }),
        })
    }
}

// ─── SttProvider Trait Implementation ─────────────────────────────────────────

impl SttProvider for EmbeddedSttProvider {
    /// One-shot full transcription (offline, no streaming state used).
    fn transcribe(&self, audio: &[f32]) -> anyhow::Result<String> {
        let inner = self.inner.lock().unwrap();
        match self.engine_type.as_str() {
            "nvidia_nemotron" => {
                inner
                    .nemotron_engine
                    .as_ref()
                    .expect("Nemotron engine not initialized")
                    .transcribe(audio)
            }
            _ => {
                inner
                    .qwen_engine
                    .as_ref()
                    .expect("Qwen engine not initialized")
                    .transcribe(audio)
            }
        }
    }

    /// Streaming/partial transcription.
    ///
    /// `chunk` is the **full accumulated utterance** for the current turn.
    /// The provider incrementally feeds only new samples to the engine,
    /// maintaining internal stride buffers and transcript stitching.
    ///
    /// Returns the **full accumulated transcript** for the turn.
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String> {
        let mut inner = self.inner.lock().unwrap();

        match self.engine_type.as_str() {
            "nvidia_nemotron" => {
                // ── Nemotron path: stride-buffered streaming ────────────────

                // Append only newly-seen samples since the last chunk call
                if inner.consumed_samples < chunk.len() {
                    let new_samples = &chunk[inner.consumed_samples..];
                    inner.stt_audio_buffer.extend_from_slice(new_samples);
                    inner.consumed_samples = chunk.len();
                }

                // Process full stride chunks (560ms = 8960 samples @ 16 kHz)
                const STRIDE_SAMPLES: usize = 8960;
                while inner.stt_audio_buffer.len() >= STRIDE_SAMPLES {
                    let stride: Vec<f32> =
                        inner.stt_audio_buffer.drain(..STRIDE_SAMPLES).collect();
                    match inner
                        .nemotron_engine
                        .as_ref()
                        .expect("Nemotron engine not initialized")
                        .transcribe_chunk(&stride, false)
                    {
                        Ok(text) => {
                            if !text.trim().is_empty() {
                                inner.stitched_transcript.push_str(&text);
                            }
                        }
                        Err(e) => {
                            log::error!("[STT] Nemotron stride transcribe failed: {}", e);
                        }
                    }
                }

                if is_final {
                    // Flush remaining buffer with zero-padded final chunk
                    if !inner.stt_audio_buffer.is_empty() {
                        let mut pad = std::mem::take(&mut inner.stt_audio_buffer);
                        pad.resize(STRIDE_SAMPLES, 0.0);
                        match inner
                            .nemotron_engine
                            .as_ref()
                            .expect("Nemotron engine not initialized")
                            .transcribe_chunk(&pad, true)
                        {
                            Ok(text) => {
                                if !text.trim().is_empty() {
                                    inner.stitched_transcript.push_str(&text);
                                }
                            }
                            Err(e) => {
                                log::error!("[STT] Nemotron final flush failed: {}", e);
                            }
                        }
                    } else {
                        // Flush cache with zero-padding chunk even if buffer is empty
                        let _ = inner
                            .nemotron_engine
                            .as_ref()
                            .expect("Nemotron engine not initialized")
                            .transcribe_chunk(&vec![0.0; STRIDE_SAMPLES], true);
                    }

                    // Reset engine streaming state for the next utterance
                    let _ = inner
                        .nemotron_engine
                        .as_ref()
                        .expect("Nemotron engine not initialized")
                        .reset_state();

                    // Clear internal buffers (in case more partials arrive before reset_state)
                    inner.stt_audio_buffer.clear();
                    inner.consumed_samples = 0;
                }

                Ok(inner.stitched_transcript.clone())
            }
            _ => {
                // ── Qwen path: sliding window + stitch ─────────────────────

                // Sliding window: keep last 72000 samples (4.5s @ 16 kHz)
                let start_idx = chunk.len().saturating_sub(72000);
                let rolling = &chunk[start_idx..];

                match inner
                    .qwen_engine
                    .as_ref()
                    .expect("Qwen engine not initialized")
                    .transcribe(rolling)
                {
                    Ok(raw) => {
                        if start_idx == 0 {
                            // First partial: replace
                            inner.stitched_transcript = raw;
                        } else {
                            // Subsequent partials: stitch with overlap handling
                            inner.stitched_transcript =
                                crate::services::utils::stitch_transcripts(
                                    &inner.stitched_transcript,
                                    &raw,
                                );
                        }
                    }
                    Err(e) => {
                        log::error!("[STT] Qwen partial transcription failed: {}", e);
                    }
                }

                Ok(inner.stitched_transcript.clone())
            }
        }
    }

    /// Reset all internal state for the current turn.
    fn reset_state(&self) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();

        inner.stt_audio_buffer.clear();
        inner.consumed_samples = 0;
        inner.stitched_transcript.clear();

        match self.engine_type.as_str() {
            "nvidia_nemotron" => {
                if let Some(ref eng) = inner.nemotron_engine {
                    let _ = eng.reset_state();
                }
            }
            _ => {
                if let Some(ref eng) = inner.qwen_engine {
                    let _ = eng.reset_state();
                }
            }
        }

        Ok(())
    }

    fn health_check(&self) -> bool {
        // Simplified: provider was constructed successfully, so it's healthy.
        true
    }

    fn kind(&self) -> SttProviderKind {
        SttProviderKind::Embedded
    }
}
