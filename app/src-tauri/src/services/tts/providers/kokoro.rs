use std::{
    f32::consts,
    path::Path,
    sync::atomic::{AtomicI32, AtomicU32, Ordering},
    time::Instant,
};

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsModelConfig,
};

use super::{SynthesisContext, TtsProvider, TtsProviderKind};
use crate::{
    core::{
        error::{PipelineError, PipelineImpact},
        events::VoxEvent,
    },
    services::{
        translit::is_devanagari,
        tts::{
            KOKORO_SILENCE_SCALE, KOKORO_VOICE_ROW_BYTES, MAX_SPEED, MIN_SPEED,
            MODEL_DIRNAME_TTS_KOKORO_ESPEAK, MODEL_FILE_TTS_KOKORO_LEXICON_US,
            MODEL_FILE_TTS_KOKORO_MODEL, MODEL_FILE_TTS_KOKORO_TOKENS,
            MODEL_FILE_TTS_KOKORO_VOICES,
        },
    },
};

struct AtomicF32 {
    inner: AtomicU32,
}

impl AtomicF32 {
    const fn new(val: f32) -> Self {
        Self {
            inner: AtomicU32::new(val.to_bits()),
        }
    }

    fn load(&self, order: Ordering) -> f32 {
        f32::from_bits(self.inner.load(order))
    }

    fn store(&self, val: f32, order: Ordering) {
        self.inner.store(val.to_bits(), order);
    }
}

/// Speech synthesis engine wrapping the Kokoro multi-language ONNX model via Sherpa-ONNX.
pub struct KokoroEngine {
    tts: Mutex<OfflineTts>,
    speed: AtomicF32,
    voice: AtomicI32,
}

impl KokoroEngine {
    /// Initializes Kokoro multi-lang v1.1 offline TTS components from the specified model directory.
    pub fn new(model_path: &Path, voice: i32, speed: f32, num_threads: u32) -> Result<Self> {
        let mp = |f: &str| -> String { model_path.join(f).to_string_lossy().into() };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kokoro: OfflineTtsKokoroModelConfig {
                    model: Some(mp(MODEL_FILE_TTS_KOKORO_MODEL)),
                    voices: Some(mp(MODEL_FILE_TTS_KOKORO_VOICES)),
                    tokens: Some(mp(MODEL_FILE_TTS_KOKORO_TOKENS)),
                    data_dir: Some(mp(MODEL_DIRNAME_TTS_KOKORO_ESPEAK)),
                    lexicon: Some(mp(MODEL_FILE_TTS_KOKORO_LEXICON_US)),
                    length_scale: 1.0,
                    ..Default::default()
                },
                num_threads: num_threads as i32,
                debug: false,
                provider: Some("cpu".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let tts = OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("[Kokoro] Failed to create OfflineTts instance"))?;

        // Clamp sid into the voices file so a stale settings index can never
        // drive an out-of-bounds style lookup in the native engine.
        let voice_count = std::fs::metadata(mp(MODEL_FILE_TTS_KOKORO_VOICES))
            .map(|m| m.len() / KOKORO_VOICE_ROW_BYTES)
            .unwrap_or(0);
        let clamped_voice = if voice_count > 0 {
            voice.clamp(0, voice_count.saturating_sub(1) as i32)
        } else {
            voice.max(0)
        };

        log::info!(
            "[Kokoro] Initialized Kokoro Multi-Lang v1.1 (voice={}, speed={})",
            clamped_voice,
            speed
        );

        Ok(Self {
            tts: Mutex::new(tts),
            speed: AtomicF32::new(speed.clamp(MIN_SPEED, MAX_SPEED)),
            voice: AtomicI32::new(clamped_voice),
        })
    }
}

impl TtsProvider for KokoroEngine {
    /// Hot-updates the playback speed factor.
    fn set_speed(&self, speed: f32) {
        self.speed
            .store(speed.clamp(MIN_SPEED, MAX_SPEED), Ordering::Relaxed);
    }

    /// Hot-updates the active Kokoro speaker voice ID.
    fn set_voice(&self, voice: i32) {
        let clamped = voice.max(0);
        self.voice.store(clamped, Ordering::Relaxed);
        log::debug!("[Kokoro] Active speaker voice updated to {}", clamped);
    }

    /// Returns the TtsProviderKind::Kokoro variant identifier.
    fn kind(&self) -> TtsProviderKind {
        TtsProviderKind::Kokoro
    }

    /// Returns true confirming the engine is loaded in memory.
    fn health_check(&self) -> bool {
        true
    }

    /// Synthesizes text chunk directly into 24kHz audio and feeds to PlaybackEngine.
    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> Result<()> {
        if ctx.cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        if is_devanagari(text) {
            log::error!(
                "[Kokoro] Devanagari script detected in turn {}: Kokoro does not support Hindi synthesis",
                ctx.turn_id
            );
            if let Err(e) = ctx.event_tx.send(VoxEvent::Error(PipelineError {
                turn_id: ctx.turn_id,
                message: "Kokoro TTS does not support Hindi (Devanagari).".to_string(),
                source: "Kokoro".to_string(),
                impact: PipelineImpact::Degraded,
            })) {
                log::warn!("[Kokoro] Failed to emit error event: {}", e);
            }
            return Ok(());
        }

        let sid = self.voice.load(Ordering::Relaxed);

        log::info!(
            "[Kokoro] Synthesizing turn {}: '{}' sid={}",
            ctx.turn_id,
            text,
            sid
        );

        let start = Instant::now();
        let speed = self.speed.load(Ordering::Relaxed);

        let gen_config = GenerationConfig {
            sid,
            speed,
            silence_scale: KOKORO_SILENCE_SCALE,
            ..Default::default()
        };

        let intent = ctx.intent;

        let tts_guard = self.tts.lock();
        let audio =
            tts_guard.generate_with_config::<fn(&[f32], f32) -> bool>(text, &gen_config, None);
        drop(tts_guard);

        if ctx.cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let (samples, sample_rate) = match audio {
            Some(ref audio_data) => (audio_data.samples(), audio_data.sample_rate() as usize),
            None => {
                if !ctx.cancel.load(Ordering::Relaxed) {
                    return Err(anyhow!("[Kokoro] Generation failed"));
                }
                return Ok(());
            }
        };

        let processed = trim_and_fade_samples(samples, sample_rate);
        if !processed.is_empty() && !ctx.cancel.load(Ordering::Relaxed) {
            ctx.playback.ingest_chunk_with_intent(&processed, intent);
        }

        let elapsed = start.elapsed().as_secs_f32();

        let audio_duration = if sample_rate > 0 {
            processed.len() as f32 / sample_rate as f32
        } else {
            0.0
        };

        let rtf = if audio_duration > 0.0 {
            elapsed / audio_duration
        } else {
            0.0
        };

        log::info!(
            "[Kokoro] Synthesis complete (turn {}). {:.2}s audio, RTF: {:.3}",
            ctx.turn_id,
            audio_duration,
            rtf
        );

        if let Some(rtf_handle) = ctx.telemetry_rtf {
            rtf_handle.store(rtf.to_bits(), Ordering::Relaxed);
        }
        Ok(())
    }
}

/// Trims vocoder silence below -45 dBFS (amplitude ~0.0056) with attack/release padding,
/// and applies a 10ms equal-power crossfade envelope at boundaries to prevent clicks.
pub fn trim_and_fade_samples(samples: &[f32], sample_rate: usize) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    const SILENCE_THRESHOLD: f32 = 0.0056; // -45 dBFS
    let attack_margin = (sample_rate as f32 * 0.015) as usize; // 15ms
    let release_margin = (sample_rate as f32 * 0.040) as usize; // 40ms
    let fade_len = (sample_rate as f32 * 0.010) as usize; // 10ms

    // Find first sample above threshold
    let start_idx = samples
        .iter()
        .position(|&s| s.abs() >= SILENCE_THRESHOLD)
        .unwrap_or(0)
        .saturating_sub(attack_margin);

    // Find last sample above threshold
    let end_idx = samples
        .iter()
        .rposition(|&s| s.abs() >= SILENCE_THRESHOLD)
        .map(|idx| (idx + release_margin).min(samples.len()))
        .unwrap_or(samples.len());

    if start_idx >= end_idx {
        return Vec::new();
    }

    let mut trimmed = samples[start_idx..end_idx].to_vec();
    let n = trimmed.len();

    // Apply 10ms equal-power fade-in (sin)
    let actual_fade_in = fade_len.min(n / 2);
    if actual_fade_in > 0 {
        for (i, sample) in trimmed[..actual_fade_in].iter_mut().enumerate() {
            let t = i as f32 / actual_fade_in as f32;
            let gain = (t * consts::FRAC_PI_2).sin();
            *sample *= gain;
        }
    }

    // Apply 10ms equal-power fade-out (cos)
    let actual_fade_out = fade_len.min(n / 2);
    if actual_fade_out > 0 {
        let fade_start = n - actual_fade_out;
        for (j, sample) in trimmed[fade_start..].iter_mut().enumerate() {
            let t = j as f32 / actual_fade_out as f32;
            let gain = (t * consts::FRAC_PI_2).cos();
            *sample *= gain;
        }
    }

    trimmed
}
