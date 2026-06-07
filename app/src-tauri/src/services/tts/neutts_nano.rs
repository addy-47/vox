use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use neutts::NeuTTS;
use crate::core::events::VoxEvent;
use crate::services::traits;

use std::collections::HashMap;

pub struct VoiceData {
    pub codes: Vec<i32>,
    pub ref_text: String,
}

pub struct TtsEngine {
    model: NeuTTS,
    voices: HashMap<i32, VoiceData>,
}

impl TtsEngine {
    pub fn new(backbone_path: &Path, decoder_path: &Path) -> Result<Self> {
        log::info!("[TTS NeuTTS] Initializing NeuTTS Nano engine...");
        
        // Limit threads to exactly 2 to prevent CPU thrashing
        std::env::set_var("GGML_NUM_THREADS", "2");
        std::env::set_var("OMP_NUM_THREADS", "2");
        
        let model = NeuTTS::load_with_decoder(backbone_path, decoder_path, "en-us")
            .map_err(|e| anyhow!("[TTS NeuTTS] Failed to load NeuTTS: {}", e))?;
            
        let voices_dir = backbone_path.parent()
            .ok_or_else(|| anyhow!("[TTS NeuTTS] Invalid backbone path"))?
            .join("voices");

        let voice_defs = [
            (200, "jo"),
            (201, "dave"),
            (202, "juliette"),
            (203, "greta"),
            (204, "mateo"),
        ];

        let mut voices = HashMap::new();
        for &(id, name) in &voice_defs {
            let npy_path = voices_dir.join(format!("{}.npy", name));
            let txt_path = voices_dir.join(format!("{}.txt", name));

            if !npy_path.exists() || !txt_path.exists() {
                log::warn!("[TTS NeuTTS] Voice files for {} not found at {:?}", name, voices_dir);
                continue;
            }

            let codes = model.load_ref_codes(&npy_path)
                .map_err(|e| anyhow!("[TTS NeuTTS] Failed to load voice codes for {}: {}", name, e))?;
                
            let ref_text = std::fs::read_to_string(&txt_path)
                .map_err(|e| anyhow!("[TTS NeuTTS] Failed to read voice transcript for {}: {}", name, e))?
                .trim()
                .to_string();

            voices.insert(id, VoiceData { codes, ref_text });
        }

        if voices.is_empty() {
            return Err(anyhow!("[TTS NeuTTS] No voice references loaded from {:?}", voices_dir));
        }

        Ok(Self { model, voices })
    }
}

impl traits::TtsEngine for TtsEngine {
    fn synthesize_chunk(
        &mut self,
        text: &str,
        voice_sid: i32,
        turn_id: u32,
        cancel: Arc<AtomicBool>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<()> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let is_hi = crate::services::utils::is_devanagari(text);
        if is_hi {
            self.model.language = "hi".to_string();
        } else {
            self.model.language = "en-us".to_string();
        }

        log::debug!(
            "[TTS NeuTTS] Synthesizing ({}) turn {}: {:?}",
            if is_hi { "HI" } else { "EN" }, turn_id, text
        );

        let start = std::time::Instant::now();
        
        // Select voice by ID (200-204), falling back to 200 (Jo) if not found
        let voice = self.voices.get(&voice_sid)
            .or_else(|| self.voices.get(&200))
            .ok_or_else(|| anyhow!("[TTS NeuTTS] No voices loaded in engine"))?;

        // Run inference with the selected speaker codes and reference transcript
        let audio = self.model.infer(text, &voice.codes, &voice.ref_text)
            .map_err(|e| anyhow!("[TTS NeuTTS] Inference failed: {}", e))?;

        if cancel.load(Ordering::Relaxed) {
            log::info!("[TTS NeuTTS] Synthesis cancelled (turn {})", turn_id);
            return Ok(());
        }

        let total_samples = audio.len();
        if total_samples > 0 {
            let _ = event_tx.send(VoxEvent::TtsChunk {
                turn_id,
                samples: audio,
            });
        }

        let elapsed = start.elapsed().as_secs_f32();
        let audio_duration = total_samples as f32 / 24000.0;
        let rtf = if audio_duration > 0.0 { elapsed / audio_duration } else { 0.0 };

        log::info!(
            "[TTS NeuTTS] {} Synthesis complete (turn {}). Duration: {:.2}s, RTF: {:.3}",
            if is_hi { "HI" } else { "EN" }, turn_id, audio_duration, rtf
        );

        let _ = event_tx.send(VoxEvent::TtsFinished { turn_id, rtf });

        Ok(())
    }
}
