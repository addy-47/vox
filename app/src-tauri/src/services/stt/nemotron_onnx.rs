use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::Mutex;
use parakeet_rs::Nemotron;
use crate::services::traits;

pub struct SttEngine {
    model: Mutex<Nemotron>,
}

impl SttEngine {
    pub fn new(model_dir: &Path) -> Result<Self> {
        log::info!("[STT] >>> Initializing parakeet-rs Nemotron-3.5 Engine...");
        
        let model = Nemotron::from_pretrained(model_dir, None)
            .map_err(|e| anyhow!("Failed to load Nemotron model from {:?}: {:?}", model_dir, e))?;
            
        log::info!("[STT] Nemotron-3.5 Engine loaded successfully.");
        Ok(Self { model: Mutex::new(model) })
    }
}

impl traits::SttEngine for SttEngine {
    fn transcribe(&self, audio: &[f32]) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let start = std::time::Instant::now();
        let mut model_lock = self.model.lock().unwrap();

        // Nemotron uses a streaming ASR model that expects 8960-sample (560ms) chunks.
        // The internal state persists across transcribe_chunk() calls and must not be
        // reset() until all chunks are processed.
        const STRIDE_SAMPLES: usize = 8960;
        let mut full_text = String::new();
        let mut offset = 0usize;

        while offset + STRIDE_SAMPLES <= audio.len() {
            let chunk = &audio[offset..offset + STRIDE_SAMPLES];
            let text = model_lock.transcribe_chunk(chunk)
                .map_err(|e| anyhow!("Nemotron transcription failed at offset {}: {:?}", offset, e))?;
            if !text.trim().is_empty() {
                full_text.push_str(&text);
            }
            offset += STRIDE_SAMPLES;
        }

        // Handle the final partial chunk by padding with zeros
        let remaining = audio.len() - offset;
        if remaining > 0 {
            let mut pad = Vec::with_capacity(STRIDE_SAMPLES);
            pad.extend_from_slice(&audio[offset..]);
            pad.resize(STRIDE_SAMPLES, 0.0);
            let text = model_lock.transcribe_chunk(&pad)
                .map_err(|e| anyhow!("Nemotron final partial chunk failed: {:?}", e))?;
            if !text.trim().is_empty() {
                full_text.push_str(&text);
            }
        }

        // Reset streaming state for the next utterance
        model_lock.reset();

        let elapsed = start.elapsed().as_secs_f32();
        let audio_duration = audio.len() as f32 / 16000.0;
        let rtf = if audio_duration > 0.0 { elapsed / audio_duration } else { 0.0 };

        log::info!(
            "[STT-Nemotron] Transcribed (Offline): {:?}. (Audio: {:.2}s, Latency: {:.2}s, RTF: {:.3})",
            full_text, audio_duration, elapsed, rtf
        );

        Ok(full_text)
    }

    fn transcribe_chunk(&self, chunk: &[f32], _is_final: bool) -> Result<String> {
        if chunk.is_empty() {
            return Ok(String::new());
        }

        let mut model_lock = self.model.lock().unwrap();
        let text = model_lock.transcribe_chunk(chunk)
            .map_err(|e| anyhow!("Nemotron chunk transcription failed: {:?}", e))?;

        Ok(text)
    }

    fn reset_state(&self) -> Result<()> {
        let mut model_lock = self.model.lock().unwrap();
        model_lock.reset();
        Ok(())
    }
}
