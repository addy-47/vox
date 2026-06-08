use crate::core::events::VoxEvent;
use crate::services::traits;
use anyhow::{anyhow, Result};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsSupertonicModelConfig,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

const SUPER_SAMPLE_RATE: u32 = 44100;

fn resample_44100_to_24000(input: &[f32]) -> Vec<f32> {
    let ratio = SUPER_SAMPLE_RATE as f32 / 24000.0;
    let out_len = (input.len() as f32 / ratio) as usize;
    let mut output = Vec::with_capacity(out_len);
    let mut src_idx: f32 = 0.0;
    while (src_idx as usize) < input.len() {
        let idx = src_idx as usize;
        let next_idx = (idx + 1).min(input.len() - 1);
        let frac = src_idx - idx as f32;
        output.push((1.0 - frac) * input[idx] + frac * input[next_idx]);
        src_idx += ratio;
    }
    output
}

pub struct TtsEngine {
    tts: OfflineTts,
    quality_steps: u32,
    speed: f32,
}

impl TtsEngine {
    pub fn new(model_path: &Path, quality_steps: u32, speed: f32) -> Result<Self> {
        let mp = |f: &str| -> String { model_path.join(f).to_string_lossy().into() };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                supertonic: OfflineTtsSupertonicModelConfig {
                    duration_predictor: Some(mp(
                        crate::core::constants::MODEL_FILE_TTS_SUPER_DURATION_PREDICTOR,
                    )),
                    text_encoder: Some(mp(
                        crate::core::constants::MODEL_FILE_TTS_SUPER_TEXT_ENCODER,
                    )),
                    vector_estimator: Some(mp(
                        crate::core::constants::MODEL_FILE_TTS_SUPER_VECTOR_ESTIMATOR,
                    )),
                    vocoder: Some(mp(crate::core::constants::MODEL_FILE_TTS_SUPER_VOCODER)),
                    tts_json: Some(mp(crate::core::constants::MODEL_FILE_TTS_SUPER_CONFIG)),
                    unicode_indexer: Some(mp(crate::core::constants::MODEL_FILE_TTS_SUPER_INDEXER)),
                    voice_style: Some(mp(crate::core::constants::MODEL_FILE_TTS_SUPER_VOICE)),
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
            tts,
            quality_steps: quality_steps.clamp(2, 12),
            speed: speed.clamp(0.7, 2.0),
        })
    }

    pub fn set_quality_steps(&mut self, steps: u32) {
        self.quality_steps = steps.clamp(2, 12);
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(0.7, 2.0);
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

        let lang = if crate::services::utils::is_devanagari(text) {
            "hi"
        } else {
            "en"
        };

        let sid = if voice_sid >= 100 {
            0
        } else {
            (voice_sid as i32).min(9).max(0)
        };

        log::info!(
            "[Supertonic] Synthesizing turn {} ({}): '{}' sid={}",
            turn_id,
            lang,
            text,
            sid
        );

        let start = std::time::Instant::now();

        let mut extra = HashMap::new();
        extra.insert("lang".to_string(), serde_json::json!(lang));

        let gen_config = GenerationConfig {
            sid,
            num_steps: self.quality_steps as i32,
            speed: self.speed,
            extra: Some(extra),
            ..Default::default()
        };

        let cancel_cb = cancel.clone();
        let event_tx_cb = event_tx.clone();

        let audio = self.tts.generate_with_config(
            text,
            &gen_config,
            Some(move |raw_samples: &[f32], _progress: f32| -> bool {
                if cancel_cb.load(Ordering::Relaxed) {
                    return false;
                }
                if raw_samples.is_empty() {
                    return true;
                }
                let samples_24k = resample_44100_to_24000(raw_samples);
                let _ = event_tx_cb.send(VoxEvent::TtsChunk {
                    turn_id,
                    samples: samples_24k,
                });
                true
            }),
        );

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

        let _ = event_tx.send(VoxEvent::TtsFinished { turn_id, rtf });
        Ok(())
    }
}
