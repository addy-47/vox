use super::{TtsProvider, TtsProviderKind};
use crate::core::events::VoxEvent;
use crate::services::tts::{MAX_SPEED, MIN_SPEED};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig, OfflineTtsModelConfig,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

// ─── Structs & Enums ──────────────────────────────────────────────────────────

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

// ─── KokoroEngine Inherent Implementations ───────────────────────────────────

impl KokoroEngine {
    /// Initializes Kokoro multi-lang v1.1 offline TTS components from the specified model directory.
    pub fn new(model_path: &Path, voice: i32, speed: f32) -> Result<Self> {
        let mp = |f: &str| -> String { model_path.join(f).to_string_lossy().into() };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kokoro: OfflineTtsKokoroModelConfig {
                    model: Some(mp(crate::services::tts::MODEL_FILE_TTS_KOKORO_MODEL)),
                    voices: Some(mp(crate::services::tts::MODEL_FILE_TTS_KOKORO_VOICES)),
                    tokens: Some(mp(crate::services::tts::MODEL_FILE_TTS_KOKORO_TOKENS)),
                    data_dir: Some(mp(crate::services::tts::MODEL_DIRNAME_TTS_KOKORO_ESPEAK)),
                    length_scale: 1.0,
                    ..Default::default()
                },
                num_threads: 2,
                debug: false,
                provider: Some("cpu".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let tts = OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("[Kokoro] Failed to create OfflineTts instance"))?;

        log::info!(
            "[Kokoro] Initialized Kokoro Multi-Lang v1.1 (voice={}, speed={})",
            voice,
            speed
        );

        Ok(Self {
            tts: Mutex::new(tts),
            speed: AtomicF32::new(speed.clamp(MIN_SPEED, MAX_SPEED)),
            voice: AtomicI32::new(voice.max(0)),
        })
    }
}

// ─── Trait Implementations ───────────────────────────────────────────────────

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
    fn synthesize_chunk(
        &self,
        text: &str,
        turn_id: u32,
        cancel: Arc<AtomicBool>,
        playback: &Arc<crate::services::audio::PlaybackEngine>,
        _event_tx: Sender<VoxEvent>,
        telemetry_rtf: Option<&Arc<AtomicU32>>,
    ) -> Result<()> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let sid = self.voice.load(Ordering::Relaxed);

        log::info!(
            "[Kokoro] Synthesizing turn {}: '{}' sid={}",
            turn_id,
            text,
            sid
        );

        let start = std::time::Instant::now();
        let speed = self.speed.load(Ordering::Relaxed);

        let gen_config = GenerationConfig {
            sid,
            speed,
            silence_scale: 0.1,
            ..Default::default()
        };

        let cancel_cb = cancel.clone();
        let playback_cb = Arc::clone(playback);

        let tts_guard = self.tts.lock();
        let audio = tts_guard.generate_with_config(
            text,
            &gen_config,
            Some(move |raw_samples: &[f32], _progress| -> bool {
                if cancel_cb.load(Ordering::Relaxed) {
                    return false;
                }
                if raw_samples.is_empty() {
                    return true;
                }
                playback_cb.ingest_chunk(raw_samples);
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
            return Err(anyhow!("[Kokoro] Generation failed"));
        }

        log::info!(
            "[Kokoro] Synthesis complete (turn {}). {:.2}s audio, RTF: {:.3}",
            turn_id,
            audio_duration,
            rtf
        );

        if let Some(rtf_handle) = telemetry_rtf {
            rtf_handle.store(rtf.to_bits(), Ordering::Relaxed);
        }
        Ok(())
    }
}
