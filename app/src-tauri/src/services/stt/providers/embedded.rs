use super::{SttProvider, SttProviderKind};
use crate::services::stt::SttEngine;
use parking_lot::Mutex;
use std::path::PathBuf;

struct EmbeddedSttProviderInner {
    model_path: PathBuf,
    model_type: String,
    nemotron_engine: Option<super::super::nemotron_onnx::SttEngine>,
    qwen_engine: Option<super::super::qwen_onnx::SttEngine>,
    stitched_transcript: String,
}

impl EmbeddedSttProviderInner {
    /// Loads the target ONNX speech recognition engine into memory if not already active.
    fn ensure_loaded(&mut self) -> anyhow::Result<()> {
        if self.nemotron_engine.is_some() || self.qwen_engine.is_some() {
            return Ok(());
        }

        log::info!(
            "[STT] Lazy-loading embedded STT engine: {}",
            self.model_type
        );
        match self.model_type.as_str() {
            "nvidia_nemotron" | "nemotron" => {
                let engine = super::super::nemotron_onnx::SttEngine::new(&self.model_path)?;
                self.nemotron_engine = Some(engine);
            }
            _ => {
                let engine = super::super::qwen_onnx::SttEngine::new(&self.model_path)?;
                self.qwen_engine = Some(engine);
            }
        }
        Ok(())
    }
}

/// Local embedded Speech-to-Text provider wrapping ONNX inference models (Qwen3-ASR or Nemotron-3.5).
pub struct EmbeddedSttProvider {
    inner: Mutex<EmbeddedSttProviderInner>,
}

impl EmbeddedSttProvider {
    /// Instantiates an embedded speech-to-text provider with lazy engine loading on first transcription.
    pub fn new(model_path: &std::path::Path, model_type: &str) -> anyhow::Result<Self> {
        Ok(Self {
            inner: Mutex::new(EmbeddedSttProviderInner {
                model_path: model_path.to_path_buf(),
                model_type: model_type.to_string(),
                nemotron_engine: None,
                qwen_engine: None,
                stitched_transcript: String::new(),
            }),
        })
    }
}

impl SttProvider for EmbeddedSttProvider {
    /// Transcribes streaming audio chunks with transcript stitching and final turn flushing.
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String> {
        let mut inner = self.inner.lock();
        inner.ensure_loaded()?;

        let transcript = if let Some(ref engine) = inner.nemotron_engine {
            engine.transcribe(chunk)?
        } else if let Some(ref engine) = inner.qwen_engine {
            engine.transcribe(chunk)?
        } else {
            anyhow::bail!("No STT engine initialized");
        };

        if !transcript.is_empty() {
            inner.stitched_transcript = if inner.stitched_transcript.is_empty() {
                transcript
            } else {
                crate::services::stt::stitch_transcripts(&inner.stitched_transcript, &transcript)
            };
        }

        if is_final {
            let result = std::mem::take(&mut inner.stitched_transcript);
            Ok(result)
        } else {
            Ok(inner.stitched_transcript.clone())
        }
    }

    /// Clears internal accumulated stitched transcripts.
    fn reset_state(&self) -> anyhow::Result<()> {
        let mut inner = self.inner.lock();
        inner.stitched_transcript.clear();
        Ok(())
    }

    /// Returns true if an engine is loaded or the model file path exists.
    fn health_check(&self) -> bool {
        let inner = self.inner.lock();
        inner.nemotron_engine.is_some() || inner.qwen_engine.is_some() || inner.model_path.exists()
    }

    /// Returns the SttProviderKind::Embedded variant identifier.
    fn kind(&self) -> SttProviderKind {
        SttProviderKind::Embedded
    }
}
