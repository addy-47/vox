use super::SttEngine as SttEngineTrait;
use anyhow::{anyhow, Result};
use parakeet_rs::Nemotron;
use parking_lot::Mutex;
use std::path::Path;

const STRIDE_SAMPLES: usize = 8960;

/// Speech-to-text inference engine wrapping NVIDIA Nemotron-3.5 via parakeet-rs.
pub struct SttEngine {
    model: Mutex<Nemotron>,
}

impl SttEngine {
    /// Loads the pretrained Nemotron-3.5 ONNX model weights from the specified directory.
    pub fn new(model_dir: &Path) -> Result<Self> {
        log::info!("[STT] >>> Initializing parakeet-rs Nemotron-3.5 Engine...");

        let model = Nemotron::from_pretrained(model_dir, None).map_err(|e| {
            anyhow!(
                "Failed to load Nemotron model from {:?}: {:?}",
                model_dir,
                e
            )
        })?;

        log::info!("[STT] Nemotron-3.5 Engine loaded successfully.");
        Ok(Self {
            model: Mutex::new(model),
        })
    }
}

/// Transcribes audio frames in discrete 8960-sample strides with final partial chunk padding.
fn transcribe_strides(model: &mut Nemotron, audio: &[f32]) -> Result<String> {
    let mut full_text = String::new();
    let mut offset = 0usize;

    while offset + STRIDE_SAMPLES <= audio.len() {
        let chunk = &audio[offset..offset + STRIDE_SAMPLES];
        let text = model.transcribe_chunk(chunk).map_err(|e| {
            anyhow!(
                "Nemotron transcription failed at offset {}: {:?}",
                offset,
                e
            )
        })?;
        if !text.trim().is_empty() {
            full_text.push_str(&text);
        }
        offset += STRIDE_SAMPLES;
    }

    let remaining = audio.len() - offset;
    if remaining > 0 {
        let mut pad = Vec::with_capacity(STRIDE_SAMPLES);
        pad.extend_from_slice(&audio[offset..]);
        pad.resize(STRIDE_SAMPLES, 0.0);
        let text = model
            .transcribe_chunk(&pad)
            .map_err(|e| anyhow!("Nemotron final partial chunk failed: {:?}", e))?;
        if !text.trim().is_empty() {
            full_text.push_str(&text);
        }
    }

    Ok(full_text)
}

impl SttEngineTrait for SttEngine {
    /// Transcribes complete audio buffer and logs latency and real-time factor metrics.
    fn transcribe(&self, audio: &[f32]) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let start = std::time::Instant::now();
        let mut model_lock = self.model.lock();
        model_lock.reset();

        let full_text = transcribe_strides(&mut model_lock, audio)?;
        model_lock.reset();

        let elapsed = start.elapsed().as_secs_f32();
        let audio_duration = audio.len() as f32 / 16000.0;
        let rtf = if audio_duration > 0.0 {
            elapsed / audio_duration
        } else {
            0.0
        };

        log::info!(
            "[STT-Nemotron] Transcribed (Offline): {:?}. (Audio: {:.2}s, Latency: {:.2}s, RTF: {:.3})",
            full_text, audio_duration, elapsed, rtf
        );

        Ok(full_text)
    }

    /// Transcribes an individual streaming audio chunk without resetting recurrent state.
    fn transcribe_chunk(&self, chunk: &[f32], _is_final: bool) -> Result<String> {
        if chunk.is_empty() {
            return Ok(String::new());
        }

        let mut model_lock = self.model.lock();
        let text = model_lock
            .transcribe_chunk(chunk)
            .map_err(|e| anyhow!("Nemotron chunk transcription failed: {:?}", e))?;

        Ok(text)
    }

    /// Resets the internal recurrent hidden state of the streaming Nemotron model.
    fn reset_state(&self) -> Result<()> {
        let mut model_lock = self.model.lock();
        model_lock.reset();
        Ok(())
    }
}
