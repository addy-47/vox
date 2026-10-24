use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use futures_util::StreamExt;

use super::{speed_range, SynthesisContext, TtsProvider};
use crate::services::{
    sidecar::{global_sidecar_supervisor, SidecarKind},
    tts::{
        ParamRange, ProviderCaps, TtsVoiceSource, MAX_SPEED, MIN_SPEED, TTS_CHUNK_SIZE,
        TTS_SAMPLE_RATE,
    },
};

pub enum CloudProviderKind {
    Google {
        voice: String,
        project_override: Option<String>,
    },
    Azure {
        region_or_endpoint: String,
        voice: String,
        key: String,
        resource_id: Option<String>,
    },
}

pub struct CloudTtsSidecarProvider {
    kind: CloudProviderKind,
    speed: AtomicU32,
}

impl CloudTtsSidecarProvider {
    pub const SPEED_RANGE: ParamRange = speed_range(MIN_SPEED, MAX_SPEED);

    pub fn new_google(voice: &str, project_override: Option<&str>) -> Result<Self, String> {
        Ok(Self {
            kind: CloudProviderKind::Google {
                voice: voice.to_string(),
                project_override: project_override.map(str::to_string),
            },
            speed: AtomicU32::new(1.0f32.to_bits()),
        })
    }

    pub fn new_azure(
        region_or_endpoint: &str,
        voice: &str,
        key: &str,
        resource_id: Option<&str>,
    ) -> Result<Self, String> {
        Ok(Self {
            kind: CloudProviderKind::Azure {
                region_or_endpoint: region_or_endpoint.to_string(),
                voice: voice.to_string(),
                key: key.to_string(),
                resource_id: resource_id.map(str::to_string),
            },
            speed: AtomicU32::new(1.0f32.to_bits()),
        })
    }
}

impl TtsProvider for CloudTtsSidecarProvider {
    fn caps() -> ProviderCaps {
        ProviderCaps {
            voices: TtsVoiceSource::Cloud,
            clone: false,
            speed_range: Self::SPEED_RANGE,
            execution: crate::services::tts::TtsExecutionKind::Cloud,
        }
    }

    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> Result<()> {
        let text_clean = text.trim();
        if text_clean.is_empty() {
            return Ok(());
        }

        let speed = f32::from_bits(self.speed.load(Ordering::Relaxed));
        let playback = Arc::clone(ctx.playback);
        let intent = ctx.intent;
        let cancel = ctx.cancel.clone();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let base_url = global_sidecar_supervisor()
                    .ensure_ready(SidecarKind::TtsCloud, &[])
                    .await
                    .map_err(|e| anyhow!("Failed to start Cloud TTS sidecar: {}", e))?;

                let client = reqwest::Client::new();

                let (endpoint, payload) = match &self.kind {
                    CloudProviderKind::Google {
                        voice,
                        project_override,
                    } => (
                        format!("{}/v1/google/speech", base_url),
                        serde_json::json!({
                            "voice": voice,
                            "text": text_clean,
                            "speed": speed,
                            "project_override": project_override,
                        }),
                    ),
                    CloudProviderKind::Azure {
                        region_or_endpoint,
                        voice,
                        key,
                        resource_id,
                    } => (
                        format!("{}/v1/azure/speech", base_url),
                        serde_json::json!({
                            "region_or_endpoint": region_or_endpoint,
                            "voice": voice,
                            "key": key,
                            "resource_id": resource_id,
                            "text": text_clean,
                            "speed": speed,
                        }),
                    ),
                };

                let resp = client
                    .post(&endpoint)
                    .json(&payload)
                    .send()
                    .await
                    .map_err(|e| anyhow!("Cloud TTS sidecar request failed: {}", e))?;

                if !resp.status().is_success() {
                    return Err(anyhow!("Cloud TTS sidecar error HTTP {}", resp.status()));
                }

                let mut stream = resp.bytes_stream();
                let mut byte_buf = Vec::new();
                let mut pending = Vec::new();

                while let Some(chunk_res) = stream.next().await {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let chunk = chunk_res.map_err(|e| anyhow!("Stream read error: {}", e))?;
                    byte_buf.extend_from_slice(&chunk);

                    let count = byte_buf.len() / 2;
                    if count > 0 {
                        for c in byte_buf[..count * 2].chunks_exact(2) {
                            let s16 = i16::from_le_bytes([c[0], c[1]]);
                            pending.push(s16 as f32 / 32768.0);
                        }
                        byte_buf.drain(..count * 2);
                    }

                    while pending.len() >= TTS_CHUNK_SIZE {
                        if cancel.load(Ordering::Relaxed) {
                            return Ok(());
                        }
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
        true
    }
}
