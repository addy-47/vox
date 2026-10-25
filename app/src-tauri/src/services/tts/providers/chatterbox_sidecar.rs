use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, Result};

use super::{speed_range, SynthesisContext, TtsProvider};
use crate::services::{
    sidecar::{global_sidecar_supervisor, SidecarKind},
    tts::{ParamRange, ProviderCaps, TtsVoiceSource, MAX_SPEED, MIN_SPEED, TTS_CHUNK_SIZE},
};

pub struct ChatterboxSidecarProvider {
    model_dir: PathBuf,
    speed: AtomicU32,
    reference_audio: Option<String>,
}

impl ChatterboxSidecarProvider {
    pub const SPEED_RANGE: ParamRange = speed_range(MIN_SPEED, MAX_SPEED);

    pub fn new(
        model_path: &Path,
        _language: &str,
        speed: f32,
        reference_audio: Option<&str>,
    ) -> Result<Self> {
        Ok(Self {
            model_dir: model_path.to_path_buf(),
            speed: AtomicU32::new(speed.to_bits()),
            reference_audio: reference_audio.map(str::to_string),
        })
    }
}

impl TtsProvider for ChatterboxSidecarProvider {
    fn caps() -> ProviderCaps {
        ProviderCaps {
            voices: TtsVoiceSource::Custom,
            clone: true,
            speed_range: Self::SPEED_RANGE,
            execution: crate::services::tts::TtsExecutionKind::Embedded,
        }
    }

    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> Result<()> {
        let text_clean = text.trim();
        if text_clean.is_empty() {
            return Ok(());
        }

        let speed = f32::from_bits(self.speed.load(Ordering::Relaxed));
        let model_dir_str = self.model_dir.to_string_lossy().to_string();
        let ref_audio = self.reference_audio.clone();
        let playback = Arc::clone(ctx.playback);
        let intent = ctx.intent;
        let cancel = ctx.cancel.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let base_url = global_sidecar_supervisor()
                    .ensure_ready(SidecarKind::TtsClone, &[("--model-dir", &model_dir_str)])
                    .await
                    .map_err(|e| anyhow!("Failed to start Chatterbox sidecar: {}", e))?;

                let client = reqwest::Client::new();
                let url = format!("{}/v1/audio/speech", base_url);

                let payload = serde_json::json!({
                    "text": text_clean,
                    "speed": speed,
                    "reference_audio": ref_audio,
                });

                let resp = client
                    .post(&url)
                    .json(&payload)
                    .send()
                    .await
                    .map_err(|e| anyhow!("Chatterbox sidecar speech failed: {}", e))?;

                if !resp.status().is_success() {
                    return Err(anyhow!("Chatterbox sidecar error HTTP {}", resp.status()));
                }

                let pcm_bytes = resp.bytes().await?;
                let mut pending = Vec::with_capacity(pcm_bytes.len() / 2);

                for chunk in pcm_bytes.chunks_exact(2) {
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let s16 = i16::from_le_bytes([chunk[0], chunk[1]]);
                    pending.push(s16 as f32 / 32768.0);

                    if pending.len() >= TTS_CHUNK_SIZE {
                        let to_send: Vec<f32> = pending.drain(..TTS_CHUNK_SIZE).collect();
                        playback.ingest_chunk_with_intent(&to_send, intent);
                    }
                }

                if !pending.is_empty() && !cancel.load(Ordering::Relaxed) {
                    playback.ingest_chunk_with_intent(&pending, intent);
                }

                Ok(())
            })
        })
    }

    fn set_speed(&self, speed: f32) {
        let clamped = speed.clamp(Self::SPEED_RANGE.min, Self::SPEED_RANGE.max);
        self.speed.store(clamped.to_bits(), Ordering::Relaxed);
    }

    fn health_check(&self) -> bool {
        self.model_dir.exists()
    }
}
