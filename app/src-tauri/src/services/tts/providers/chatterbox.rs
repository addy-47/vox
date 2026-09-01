use super::{TtsProvider, TtsProviderKind};
use crate::core::events::VoxEvent;
use crate::services::tts::{
    MAX_QUALITY_STEPS_CHATTERBOX, MIN_QUALITY_STEPS, MIN_SPEED, MODEL_FILE_TTS_CHATTERBOX_S3GEN,
    MODEL_FILE_TTS_CHATTERBOX_T3, TTS_CHUNK_SIZE, TTS_SAMPLE_RATE,
};
use anyhow::{anyhow, Result};
use chatterbox_rs::{Engine, EngineOptions};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// Speech synthesis engine wrapping the local Chatterbox GGUF model via chatterbox-rs.
pub struct ChatterboxEngine {
    engine: Mutex<Engine>,
    quality_steps: AtomicU32,
    speed: AtomicU32,
}

impl ChatterboxEngine {
    /// Loads Chatterbox T3 and S3Gen GGUF models and initializes the inference engine.
    pub fn new(
        model_path: &Path,
        language: &str,
        quality_steps: u32,
        speed: f32,
        reference_audio: Option<&str>,
    ) -> Result<Self> {
        let t3_path = model_path.join(MODEL_FILE_TTS_CHATTERBOX_T3);
        let s3_path = model_path.join(MODEL_FILE_TTS_CHATTERBOX_S3GEN);

        if !t3_path.exists() {
            return Err(anyhow!(
                "Chatterbox T3 model not found: {}",
                t3_path.display()
            ));
        }
        if !s3_path.exists() {
            return Err(anyhow!(
                "Chatterbox S3Gen model not found: {}",
                s3_path.display()
            ));
        }

        let cfm = quality_steps.clamp(MIN_QUALITY_STEPS, MAX_QUALITY_STEPS_CHATTERBOX) as i32;
        let ref_audio = reference_audio.unwrap_or("").to_string();

        if !ref_audio.is_empty() {
            if std::path::Path::new(&ref_audio).exists() {
                log::info!(
                    "[Chatterbox] Loading engine with voice clone. lang={}, cfm_steps={}, speed={:.2}, ref={}",
                    language, cfm, speed, ref_audio
                );
            } else {
                log::warn!(
                    "[Chatterbox] reference_audio path not found: {}. Falling back to built-in voice.",
                    ref_audio
                );
            }
        } else {
            log::info!(
                "[Chatterbox] Loading engine. lang={}, cfm_steps={}, speed={:.2}",
                language,
                cfm,
                speed
            );
        }

        let mut opts = EngineOptions {
            t3_gguf_path: t3_path.to_string_lossy().into_owned(),
            s3gen_gguf_path: s3_path.to_string_lossy().into_owned(),
            language: language.to_string(),
            n_gpu_layers: 0,
            cfm_steps: cfm,
            seed: 42,
            temperature: 0.8,
            top_k: 1000,
            top_p: 0.95,
            repeat_penalty: 1.2,
            verbose: false,
            ..Default::default()
        };

        if !ref_audio.is_empty() {
            if std::path::Path::new(&ref_audio).is_dir() {
                opts.voice_dir = ref_audio;
            } else {
                opts.reference_audio = ref_audio;
            }
        }

        let engine =
            Engine::new(opts).map_err(|e| anyhow!("Failed to create Chatterbox engine: {}", e))?;

        log::info!("[Chatterbox] Engine ready.");

        Ok(Self {
            engine: Mutex::new(engine),
            quality_steps: AtomicU32::new(cfm as u32),
            speed: AtomicU32::new(speed.to_bits()),
        })
    }

    /// Applies time-stretch playback rate scaling on 24kHz audio via linear interpolation.
    fn apply_speed(samples: &[f32], speed: f32) -> Vec<f32> {
        if (speed - 1.0).abs() < 0.01 || samples.is_empty() {
            return samples.to_vec();
        }
        let target_len = (samples.len() as f32 / speed) as usize;
        if target_len == 0 {
            return samples.to_vec();
        }
        let ratio = samples.len() as f64 / target_len as f64;
        let mut out = Vec::with_capacity(target_len);
        for i in 0..target_len {
            let src_idx = i as f64 * ratio;
            let idx = src_idx as usize;
            let frac = src_idx - idx as f64;
            let next = (idx + 1).min(samples.len() - 1);
            let s = samples[idx] as f64 * (1.0 - frac) + samples[next] as f64 * frac;
            out.push(s as f32);
        }
        out
    }
}

impl TtsProvider for ChatterboxEngine {
    /// Hot-updates the number of diffusion quality steps.
    fn set_quality_steps(&self, steps: u32) {
        let clamped = steps.clamp(MIN_QUALITY_STEPS, MAX_QUALITY_STEPS_CHATTERBOX);
        self.quality_steps.store(clamped, Ordering::Relaxed);
        log::info!("[Chatterbox] Quality steps set to {} (cfm_steps)", clamped);
    }

    /// Hot-updates the speech playback speed factor.
    fn set_speed(&self, speed: f32) {
        let clamped = speed.clamp(MIN_SPEED, crate::services::tts::MAX_SPEED);
        self.speed.store(clamped.to_bits(), Ordering::Relaxed);
        log::info!("[Chatterbox] Speed set to {:.2}", clamped);
    }

    /// Returns the TtsProviderKind::Chatterbox variant identifier.
    fn kind(&self) -> TtsProviderKind {
        TtsProviderKind::Chatterbox
    }

    /// Returns true confirming the engine is initialized and available.
    fn health_check(&self) -> bool {
        true
    }

    /// Synthesizes input text chunk and feeds 24kHz audio directly to PlaybackEngine.
    fn synthesize_chunk(
        &self,
        text: &str,
        turn_id: u32,
        cancel: Arc<AtomicBool>,
        playback: &Arc<crate::services::audio::PlaybackEngine>,
        _event_tx: Sender<VoxEvent>,
        telemetry_rtf: Option<&Arc<AtomicU32>>,
    ) -> Result<()> {
        log::info!(
            "[Chatterbox] Starting synthesis for text (turn {}): '{}'",
            turn_id,
            text
        );

        let start = std::time::Instant::now();

        let pcm = {
            let engine = self.engine.lock();
            let result = engine
                .synthesize(text)
                .map_err(|e| anyhow!("Chatterbox synthesis failed: {}", e))?;
            result.pcm
        };

        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let elapsed = start.elapsed().as_secs_f32();
        let speed = f32::from_bits(self.speed.load(Ordering::Relaxed));
        let output = if (speed - 1.0).abs() >= 0.01 {
            Self::apply_speed(&pcm, speed)
        } else {
            pcm
        };

        for chunk in output.chunks(TTS_CHUNK_SIZE) {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            playback.ingest_chunk(chunk);
        }

        let audio_duration = output.len() as f32 / TTS_SAMPLE_RATE as f32;
        let rtf = if audio_duration > 0.0 {
            elapsed / audio_duration
        } else {
            0.0
        };

        log::info!(
            "[Chatterbox] Synthesis complete (turn {}). {:.2}s audio, RTF: {:.3}, speed: {:.2}",
            turn_id,
            audio_duration,
            rtf,
            speed,
        );

        if let Some(rtf_handle) = telemetry_rtf {
            rtf_handle.store(rtf.to_bits(), Ordering::Relaxed);
        }

        Ok(())
    }
}
