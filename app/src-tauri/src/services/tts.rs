use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsKokoroModelConfig, OfflineTtsVitsModelConfig,
};

use crate::core::events::VoxEvent;
use crate::core::constants::{
    MODEL_FILE_TTS_ONNX, MODEL_FILE_TTS_VOICES, MODEL_FILE_TTS_TOKENS, 
    MODEL_FILE_TTS_ESPEAK, MODEL_FILE_TTS_HI_ONNX
};

// ─── TTS Engine (Multi-Model Routing) ─────────────────────────────────────────

pub struct TtsEngine {
    en_tts: OfflineTts,
    hi_tts: OfflineTts,
}

impl TtsEngine {
    /// Initialize the TTS engine with both English and Hindi models.
    pub fn new(en_model_dir: &Path, hi_model_dir: &Path) -> Result<Self> {
        log::info!("[TTS] Initializing Multi-Model TTS engine...");

        // 1. English (Kokoro-82M)
        let en_config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kokoro: OfflineTtsKokoroModelConfig {
                    model: Some(en_model_dir.join(MODEL_FILE_TTS_ONNX).to_string_lossy().into()),
                    voices: Some(en_model_dir.join(MODEL_FILE_TTS_VOICES).to_string_lossy().into()),
                    tokens: Some(en_model_dir.join(MODEL_FILE_TTS_TOKENS).to_string_lossy().into()),
                    data_dir: Some(en_model_dir.join(MODEL_FILE_TTS_ESPEAK).to_string_lossy().into()),
                    length_scale: 1.0,
                    ..Default::default()
                },
                num_threads: 2,
                debug: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let en_tts = OfflineTts::create(&en_config)
            .ok_or_else(|| anyhow!("[TTS] Failed to create English (Kokoro) TTS instance."))?;

        // 2. Hindi (Piper VITS)
        // Note: Piper models in sherpa-onnx use the VITS config block.
        let hi_config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                vits: OfflineTtsVitsModelConfig {
                    model: Some(hi_model_dir.join(MODEL_FILE_TTS_HI_ONNX).to_string_lossy().into()),
                    tokens: Some(hi_model_dir.join(MODEL_FILE_TTS_TOKENS).to_string_lossy().into()),
                    data_dir: Some(hi_model_dir.join(MODEL_FILE_TTS_ESPEAK).to_string_lossy().into()),
                    noise_scale: 0.667,
                    noise_scale_w: 0.8,
                    length_scale: 1.0,
                    ..Default::default()
                },
                num_threads: 2,
                debug: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let hi_tts = OfflineTts::create(&hi_config)
            .ok_or_else(|| anyhow!("[TTS] Failed to create Hindi (Piper) TTS instance."))?;

        log::info!(
            "[TTS] Multi-model engine online. (EN: {}Hz, HI: {}Hz)", 
            en_tts.sample_rate(), hi_tts.sample_rate()
        );

        Ok(Self { en_tts, hi_tts })
    }

    /// Detect if string contains Devanagari (Hindi) characters.
    fn is_hindi(&self, text: &str) -> bool {
        text.chars().any(|c| c >= '\u{0900}' && c <= '\u{097F}' )
    }

    /// Synthesize text and stream audio chunks to the pipeline.
    ///
    /// Routes text to either Kokoro (English) or Piper (Hindi) based on characters.
    pub fn synthesize_chunk(
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

        // Language Routing Logic (Zero-latency Unicode check)
        let is_hi = self.is_hindi(text);
        let tts_instance = if is_hi { &self.hi_tts } else { &self.en_tts };
        
        // For Hindi, we usually use sid=0 for single-voice Piper models.
        // For English (Kokoro), voice_sid maps to the voice index in voices.bin.
        let actual_sid = if is_hi { 0 } else { voice_sid };

        let gen_config = GenerationConfig {
            sid: actual_sid,
            speed: 1.0,
            ..Default::default()
        };

        log::debug!(
            "[TTS] Synthesizing ({}) turn {}: {:?}", 
            if is_hi { "HI" } else { "EN" }, turn_id, text
        );

        let start = std::time::Instant::now();
        let mut total_samples = 0;

        let cancel_closure = Arc::clone(&cancel);
        let event_tx_closure = event_tx.clone();

        let audio = tts_instance.generate_with_config(
            text,
            &gen_config,
            Some(move |samples: &[f32], progress: f32| -> bool {
                if cancel_closure.load(Ordering::Relaxed) {
                    return false;
                }

                if !samples.is_empty() {
                    if let Err(e) = event_tx_closure.send(VoxEvent::TtsChunk {
                        turn_id,
                        samples: samples.to_vec(),
                    }) {
                        log::error!("[TTS] Failed to send TtsChunk: {}", e);
                        return false;
                    }
                }

                log::trace!("[TTS] Progress: {:.1}%", progress * 100.0);
                true
            }),
        );

        if audio.is_none() {
            if cancel.load(Ordering::Relaxed) {
                log::info!("[TTS] Synthesis stopped (turn {})", turn_id);
            } else {
                return Err(anyhow!("[TTS] Generation failed"));
            }
        } else {
            if let Some(audio_data) = audio {
                total_samples = audio_data.samples().len();
            }
            
            let elapsed = start.elapsed().as_secs_f32();
            let audio_duration = total_samples as f32 / tts_instance.sample_rate() as f32;
            let rtf = if audio_duration > 0.0 { elapsed / audio_duration } else { 0.0 };

            log::info!(
                "[TTS] {} Synthesis complete (turn {}). Duration: {:.2}s, RTF: {:.3}",
                if is_hi { "HI" } else { "EN" }, turn_id, audio_duration, rtf
            );

            let _ = event_tx.send(VoxEvent::TtsFinished { turn_id });
        }

        Ok(())
    }
}
