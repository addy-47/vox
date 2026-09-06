use std::path::PathBuf;

use parking_lot::Mutex;

use super::{
    nemotron::SttEngine as NemotronEngine, qwen::SttEngine as QwenEngine, stitch_transcripts,
    SttEngine, SttProvider, SttProviderKind,
};

struct EmbeddedSttProviderInner {
    model_path: PathBuf,
    model_type: String,
    num_threads: u32,
    nemotron_engine: Option<NemotronEngine>,
    qwen_engine: Option<QwenEngine>,
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
                let engine = NemotronEngine::new(&self.model_path, self.num_threads)?;
                self.nemotron_engine = Some(engine);
            }
            _ => {
                let engine = QwenEngine::new(&self.model_path, self.num_threads)?;
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
    pub fn new(
        model_path: &std::path::Path,
        model_type: &str,
        num_threads: u32,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            inner: Mutex::new(EmbeddedSttProviderInner {
                model_path: model_path.to_path_buf(),
                model_type: model_type.to_string(),
                num_threads,
                nemotron_engine: None,
                qwen_engine: None,
                stitched_transcript: String::new(),
            }),
        })
    }
}

impl SttProvider for EmbeddedSttProvider {
    /// Transcribes streaming audio chunks or finalizes the active turn transcript.
    fn transcribe_chunk(&self, chunk: &[f32], is_final: bool) -> anyhow::Result<String> {
        let mut inner = self.inner.lock();
        inner.ensure_loaded()?;

        if is_final {
            let transcript = if let Some(ref engine) = inner.nemotron_engine {
                if chunk.is_empty() {
                    engine.finalize_stream()?
                } else {
                    engine.transcribe(chunk)?
                }
            } else if let Some(ref engine) = inner.qwen_engine {
                engine.transcribe(chunk)?
            } else {
                anyhow::bail!("No STT engine initialized");
            };

            inner.stitched_transcript.clear();
            Ok(transcript)
        } else if let Some(ref engine) = inner.nemotron_engine {
            engine.accept_audio_chunk(chunk)?;
            let partial = engine.get_partial_result()?;
            inner.stitched_transcript = partial.clone();
            Ok(partial)
        } else if let Some(ref engine) = inner.qwen_engine {
            let transcript = engine.transcribe(chunk)?;
            if !transcript.is_empty() {
                inner.stitched_transcript = if inner.stitched_transcript.is_empty() {
                    transcript
                } else {
                    stitch_transcripts(&inner.stitched_transcript, &transcript)
                };
            }
            Ok(inner.stitched_transcript.clone())
        } else {
            anyhow::bail!("No STT engine initialized");
        }
    }

    /// Clears internal accumulated stitched transcripts and resets active online stream.
    fn reset_state(&self) -> anyhow::Result<()> {
        let mut inner = self.inner.lock();
        inner.stitched_transcript.clear();
        if let Some(ref engine) = inner.nemotron_engine {
            engine.reset_stream()?;
        }
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
