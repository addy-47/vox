use super::{TtsProvider, TtsProviderKind};
use crate::core::events::VoxEvent;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsSupertonicModelConfig,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

const MIN_QUALITY_STEPS: u32 = 2;
const MAX_QUALITY_STEPS: u32 = 12;
const MIN_SPEED: f32 = 0.7;
const MAX_SPEED: f32 = 2.0;
const SUPER_SAMPLE_RATE: u32 = 44100;

struct BiquadFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadFilter {
    /// Creates a 2nd-order Butterworth low-pass filter at 11kHz cutoff for 44.1kHz audio.
    fn new_lpf_11k() -> Self {
        Self {
            b0: 0.291851,
            b1: 0.583701,
            b2: 0.291851,
            a1: -0.004173,
            a2: 0.171576,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// Processes an individual audio sample through the direct form biquad filter.
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

/// Applies 11kHz anti-aliasing filter and resamples 44.1kHz audio down to standard 24kHz.
fn resample_44100_to_24000(input: &[f32], lpf: &mut BiquadFilter) -> Vec<f32> {
    let ratio = SUPER_SAMPLE_RATE as f32 / 24000.0;
    let out_len = (input.len() as f32 / ratio) as usize;
    let mut output = Vec::with_capacity(out_len);

    let filtered: Vec<f32> = input.iter().map(|&x| lpf.process(x)).collect();

    let mut src_idx: f32 = 0.0;
    while (src_idx as usize) < filtered.len() {
        let idx = src_idx as usize;
        let next_idx = (idx + 1).min(filtered.len() - 1);
        let frac = src_idx - idx as f32;
        output.push((1.0 - frac) * filtered[idx] + frac * filtered[next_idx]);
        src_idx += ratio;
    }
    output
}

/// Speech synthesis engine wrapping the Supertonic ONNX model via Sherpa-ONNX.
pub struct TtsEngine {
    tts: Mutex<OfflineTts>,
    quality_steps: AtomicU32,
    speed: AtomicF32,
    voice: i32,
}

struct AtomicF32 {
    inner: AtomicU32,
}

impl AtomicF32 {
    /// Creates a new atomic float from an initial f32 value.
    const fn new(val: f32) -> Self {
        Self {
            inner: AtomicU32::new(val.to_bits()),
        }
    }

    /// Loads the current float value with the given atomic memory ordering.
    fn load(&self, order: Ordering) -> f32 {
        f32::from_bits(self.inner.load(order))
    }

    /// Stores a float value with the given atomic memory ordering.
    fn store(&self, val: f32, order: Ordering) {
        self.inner.store(val.to_bits(), order);
    }
}

impl TtsEngine {
    /// Initializes Supertonic ONNX offline TTS components from the specified model directory.
    pub fn new(model_path: &Path, voice: i32, quality_steps: u32, speed: f32) -> Result<Self> {
        let mp = |f: &str| -> String { model_path.join(f).to_string_lossy().into() };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                supertonic: OfflineTtsSupertonicModelConfig {
                    duration_predictor: Some(mp(
                        crate::services::tts::MODEL_FILE_TTS_SUPER_DURATION_PREDICTOR,
                    )),
                    text_encoder: Some(mp(crate::services::tts::MODEL_FILE_TTS_SUPER_TEXT_ENCODER)),
                    vector_estimator: Some(mp(
                        crate::services::tts::MODEL_FILE_TTS_SUPER_VECTOR_ESTIMATOR,
                    )),
                    vocoder: Some(mp(crate::services::tts::MODEL_FILE_TTS_SUPER_VOCODER)),
                    tts_json: Some(mp(crate::services::tts::MODEL_FILE_TTS_SUPER_CONFIG)),
                    unicode_indexer: Some(mp(crate::services::tts::MODEL_FILE_TTS_SUPER_INDEXER)),
                    voice_style: Some(mp(crate::services::tts::MODEL_FILE_TTS_SUPER_VOICE)),
                },
                num_threads: 2,
                debug: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let tts = OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("[Supertonic] Failed to create OfflineTts"))?;

        log::info!(
            "[Supertonic] Engine ready. {} speakers, {}Hz",
            tts.num_speakers(),
            tts.sample_rate(),
        );

        Ok(Self {
            tts: Mutex::new(tts),
            quality_steps: AtomicU32::new(
                quality_steps.clamp(MIN_QUALITY_STEPS, MAX_QUALITY_STEPS),
            ),
            speed: AtomicF32::new(speed.clamp(MIN_SPEED, MAX_SPEED)),
            voice: voice.clamp(0, 9),
        })
    }
}

impl TtsProvider for TtsEngine {
    /// Hot-updates the number of Supertonic diffusion quality steps.
    fn set_quality_steps(&self, steps: u32) {
        self.quality_steps.store(
            steps.clamp(MIN_QUALITY_STEPS, MAX_QUALITY_STEPS),
            Ordering::Relaxed,
        );
    }

    /// Hot-updates the playback speed factor.
    fn set_speed(&self, speed: f32) {
        self.speed
            .store(speed.clamp(MIN_SPEED, MAX_SPEED), Ordering::Relaxed);
    }

    /// Returns the TtsProviderKind::Supertonic variant identifier.
    fn kind(&self) -> TtsProviderKind {
        TtsProviderKind::Supertonic
    }

    /// Returns true confirming the engine is loaded in memory.
    fn health_check(&self) -> bool {
        true
    }

    /// Synthesizes text chunk, resamples stream to 24kHz in real-time, and dispatches chunk events.
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

        let lang = if crate::services::utils::is_devanagari(text) {
            "hi"
        } else {
            "en"
        };

        let sid = self.voice;

        log::info!(
            "[Supertonic] Synthesizing turn {} ({}): '{}' sid={}",
            turn_id,
            lang,
            text,
            sid
        );

        let steps = self.quality_steps.load(Ordering::Relaxed) as i32;
        let spd = self.speed.load(Ordering::Relaxed);

        let start = std::time::Instant::now();

        let mut extra = HashMap::new();
        extra.insert("lang".to_string(), serde_json::json!(lang));

        let gen_config = GenerationConfig {
            sid,
            num_steps: steps,
            speed: spd,
            silence_scale: 0.1,
            extra: Some(extra),
            ..Default::default()
        };

        let cancel_cb = cancel.clone();
        let event_tx_cb = event_tx.clone();
        let mut lpf = BiquadFilter::new_lpf_11k();

        let tts_guard = self.tts.lock();
        let audio = tts_guard.generate_with_config(
            text,
            &gen_config,
            Some(move |raw_samples: &[f32], _progress: f32| -> bool {
                if cancel_cb.load(Ordering::Relaxed) {
                    return false;
                }
                if raw_samples.is_empty() {
                    return true;
                }
                let samples_24k = resample_44100_to_24000(raw_samples, &mut lpf);
                if let Err(e) = event_tx_cb.send(VoxEvent::TtsChunk {
                    turn_id,
                    samples: samples_24k,
                }) {
                    log::warn!("[Supertonic] Error sending TtsChunk: {:?}", e);
                }
                true
            }),
        );
        drop(tts_guard);

        let elapsed = start.elapsed().as_secs_f32();

        let audio_duration = if let Some(ref audio_data) = audio {
            audio_data.samples().len() as f32 / audio_data.sample_rate() as f32
        } else {
            0.0
        };

        let rtf = if audio_duration > 0.0 {
            elapsed / audio_duration
        } else {
            0.0
        };

        if audio.is_none() && !cancel.load(Ordering::Relaxed) {
            return Err(anyhow!("[Supertonic] Generation failed"));
        }

        log::info!(
            "[Supertonic] Synthesis complete (turn {}). {:.2}s audio, RTF: {:.3}",
            turn_id,
            audio_duration,
            rtf
        );

        if let Err(e) = event_tx.send(VoxEvent::TtsFinished { turn_id, rtf }) {
            log::warn!("[Supertonic] Error sending TtsFinished: {:?}", e);
        }
        Ok(())
    }
}
