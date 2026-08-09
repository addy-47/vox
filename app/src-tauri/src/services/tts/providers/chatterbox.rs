//! Chatterbox TTS provider — wraps `chatterbox-rs` Engine.
//!
//! ## Config knobs from Vox settings
//! - `language` — set at construction (fixed, requires restart)
//! - `quality_steps` → `cfm_steps` — hot-updatable via `set_quality_steps()`
//! - `speed` — applied as time-stretch on output PCM via linear interpolation
//! - `voice` — not applicable (uses built-in reference voice)
//!
//! ## Output format
//! Native 24 kHz f32 mono PCM — no resampling needed.

use super::{TtsProvider, TtsProviderKind};
use crate::core::events::VoxEvent;
use anyhow::{anyhow, Result};
use chatterbox_rs::{Engine, EngineOptions};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// Minimum quality steps (fast, lower audio quality).
const MIN_QUALITY_STEPS: u32 = 2;
/// Maximum quality steps (highest quality).
const MAX_QUALITY_STEPS: u32 = 10;

/// Minimum speed factor.
const MIN_SPEED: f32 = 0.7;
/// Maximum speed factor.
const MAX_SPEED: f32 = 2.0;

pub struct ChatterboxEngine {
    /// The chatterbox-rs Engine, wrapped in a Mutex for interior mutability.
    engine: Mutex<Engine>,
    /// CFM steps (quality / diffusion steps), clamped 2-10.
    quality_steps: AtomicU32,
    /// Speed factor applied via time-stretch, clamped 0.7-2.0.
    speed: AtomicU32, // stored as f32 bits
    /// Language code — fixed at construction.
    #[allow(dead_code)]
    language: String,
}

impl ChatterboxEngine {
    /// Create a new Chatterbox TTS engine.
    ///
    /// `model_path` — directory containing chatterbox GGUFs (expects
    ///   `t3-q4_0.gguf` and `s3gen-f16.gguf`).
    /// `language` — language code ("en", "es", "fr", etc.).
    /// `quality_steps` — CFM steps (clamped 2-10).
    /// `speed` — playback speed factor (clamped 0.7-2.0, applied as time-stretch).
    /// `reference_audio` — optional absolute path to a source WAV for voice cloning.
    ///   `None` = use Chatterbox's built-in reference voice.
    pub fn new(
        model_path: &Path,
        language: &str,
        quality_steps: u32,
        speed: f32,
        reference_audio: Option<&str>,
    ) -> Result<Self> {
        let t3_path = model_path.join("t3-q4_0.gguf");
        let s3_path = model_path.join("s3gen-f16.gguf");

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

        let cfm = quality_steps.clamp(MIN_QUALITY_STEPS, MAX_QUALITY_STEPS) as i32;

        // Validate reference audio path if provided, and warn if missing.
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
            n_gpu_layers: 0, // Vox is CPU-only
            cfm_steps: cfm,
            seed: 42, // reproducible by default
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
            language: language.to_string(),
        })
    }

    /// Apply time-stretch via linear interpolation.
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
    fn set_quality_steps(&self, steps: u32) {
        let clamped = steps.clamp(MIN_QUALITY_STEPS, MAX_QUALITY_STEPS);
        self.quality_steps.store(clamped, Ordering::Relaxed);
        log::info!("[Chatterbox] Quality steps set to {} (cfm_steps)", clamped);
    }

    fn set_speed(&self, speed: f32) {
        let clamped = speed.clamp(MIN_SPEED, MAX_SPEED);
        self.speed.store(clamped.to_bits(), Ordering::Relaxed);
        log::info!("[Chatterbox] Speed set to {:.2}", clamped);
    }

    fn kind(&self) -> TtsProviderKind {
        TtsProviderKind::Chatterbox
    }

    fn health_check(&self) -> bool {
        // Engine is always healthy if constructed successfully.
        true
    }

    fn synthesize_chunk(
        &self,
        text: &str,
        turn_id: u32,
        cancel: Arc<AtomicBool>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<()> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        if text.trim().is_empty() {
            return Ok(());
        }

        log::info!(
            "[Chatterbox] Synthesizing turn {}: '{}'",
            turn_id,
            text.chars().take(80).collect::<String>()
        );

        let start = std::time::Instant::now();

        // Lock engine, synthesize, release lock before sending events.
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

        // Apply speed time-stretch.
        let speed_bits = self.speed.load(Ordering::Relaxed);
        let speed = f32::from_bits(speed_bits);
        let output = if (speed - 1.0).abs() >= 0.01 {
            Self::apply_speed(&pcm, speed)
        } else {
            pcm
        };

        // Send PCM in 2048-sample chunks (≈85 ms at 24 kHz).
        // This avoids starving the playback engine and keeps TTFA low.
        const CHUNK_SIZE: usize = 2048;
        for chunk in output.chunks(CHUNK_SIZE) {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            if event_tx
                .send(VoxEvent::TtsChunk {
                    turn_id,
                    samples: chunk.to_vec(),
                })
                .is_err()
            {
                log::warn!("[Chatterbox] event_tx closed, stopping synthesis");
                return Ok(());
            }
        }

        // Compute and report RTF.
        let audio_duration = output.len() as f32 / 24000.0;
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

        let _ = event_tx.send(VoxEvent::TtsFinished { turn_id, rtf });

        Ok(())
    }
}
