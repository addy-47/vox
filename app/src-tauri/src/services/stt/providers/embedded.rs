//! Embedded STT provider — wraps the local Qwen3-ASR and Nemotron-3.5 ONNX engines
//! behind a single `SttProvider` interface.
//!
//! This provider owns the stride-buffering and transcript-stitching logic that was
//! previously inline in `actor.rs`, keeping the actor engine-agnostic.

use super::{SttProvider, SttProviderKind};
use crate::services::stt::SttEngine;
use parking_lot::Mutex;

// ─── Internal State ────────────────────────────────────────────────────────────

struct EmbeddedSttProviderInner {
    /// Nemotron engine, if active.
    nemotron_engine: Option<super::super::nemotron_onnx::SttEngine>,
    /// Qwen engine, if active.
    qwen_engine: Option<super::super::qwen_onnx::SttEngine>,

    // ── Stride buffering state (Nemotron only) ─────────────────────────────
    /// Accumulated audio samples not yet consumed by the engine.
    stt_audio_buffer: Vec<f32>,

    // ── Transcript stitching state (Qwen) / accumulation (Nemotron) ────────
    /// Full accumulated transcript for the current turn.
    stitched_transcript: String,
}

// ─── Provider ─────────────────────────────────────────────────────────────────

pub struct EmbeddedSttProvider {
    _engine_type: String,
    inner: Mutex<EmbeddedSttProviderInner>,
}

impl EmbeddedSttProvider {
    /// Create a new embedded provider.
    ///
    /// `model_type`: `"nvidia_nemotron"`, `"nemotron"`, or any other value (treated as Qwen3-ASR).
    /// `model_path`: path to the model directory.
    pub fn new(model_path: &std::path::Path, model_type: &str) -> anyhow::Result<Self> {
        let (nemotron_engine, qwen_engine) = match model_type {
            "nvidia_nemotron" | "nemotron" => {
                let engine = super::super::nemotron_onnx::SttEngine::new(model_path)?;
                (Some(engine), None)
            }
            _ => {
                let engine = super::super::qwen_onnx::SttEngine::new(model_path)?;
                (None, Some(engine))
            }
        };

        Ok(Self {
            _engine_type: model_type.to_string(),
            inner: Mutex::new(EmbeddedSttProviderInner {
                nemotron_engine,
                qwen_engine,
                stt_audio_buffer: Vec::new(),
                stitched_transcript: String::new(),
            }),
        })
    }
}

impl SttProvider for EmbeddedSttProvider {
    fn transcribe(&self, audio: &[f32]) -> anyhow::Result<String> {
        let inner = self.inner.lock();
        if let Some(ref engine) = inner.nemotron_engine {
            engine.transcribe(audio)
        } else if let Some(ref engine) = inner.qwen_engine {
            engine.transcribe(audio)
        } else {
            anyhow::bail!("No STT engine initialized");
        }
    }

    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String> {
        let mut inner = self.inner.lock();

        if inner.nemotron_engine.is_some() {
            inner.stt_audio_buffer.extend_from_slice(chunk);

            if is_final {
                let full_audio = std::mem::take(&mut inner.stt_audio_buffer);
                let full_transcript = if let Some(ref engine) = inner.nemotron_engine {
                    engine.transcribe(&full_audio)?
                } else {
                    String::new()
                };
                inner.stitched_transcript = full_transcript.clone();
                Ok(full_transcript)
            } else {
                // Return accumulated transcript so far during streaming
                Ok(inner.stitched_transcript.clone())
            }
        } else if let Some(ref engine) = inner.qwen_engine {
            // Qwen3-ASR stitching logic
            let transcript = engine.transcribe(chunk)?;
            if !transcript.is_empty() {
                inner.stitched_transcript = transcript.clone();
            }
            if is_final {
                let result = inner.stitched_transcript.clone();
                inner.stitched_transcript.clear();
                Ok(result)
            } else {
                Ok(inner.stitched_transcript.clone())
            }
        } else {
            anyhow::bail!("No STT engine initialized");
        }
    }

    fn reset_state(&self) -> anyhow::Result<()> {
        let mut inner = self.inner.lock();
        inner.stt_audio_buffer.clear();
        inner.stitched_transcript.clear();
        Ok(())
    }

    fn health_check(&self) -> bool {
        let inner = self.inner.lock();
        inner.nemotron_engine.is_some() || inner.qwen_engine.is_some()
    }

    fn kind(&self) -> SttProviderKind {
        SttProviderKind::Embedded
    }
}
