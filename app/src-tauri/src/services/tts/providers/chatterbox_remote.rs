use anyhow::{anyhow, Result};
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use super::{TtsProvider, TtsProviderKind};
use crate::core::events::VoxEvent;

const MIN_QUALITY_STEPS: u32 = 2;
const MAX_QUALITY_STEPS: u32 = 10;
const MIN_SPEED: f32 = 0.7;
const MAX_SPEED: f32 = 2.0;
const CHUNK_SIZE: usize = 2048;

/// Remote speech synthesis provider offloading Chatterbox inference to a GPU server via HTTP streaming.
pub struct ChatterboxRemoteProvider {
    client: reqwest::blocking::Client,
    endpoint: String,
    language: String,
    quality_steps: AtomicU32,
    speed: AtomicU32,
}

impl ChatterboxRemoteProvider {
    /// Creates a new ChatterboxRemoteProvider and performs initial endpoint connectivity verification.
    pub fn new(
        endpoint: &str,
        language: &str,
        quality_steps: u32,
        speed: f32,
        remote_path: &str,
    ) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(None)
            .pool_max_idle_per_host(5)
            .build()
            .map_err(|e| anyhow!("Failed to build reqwest client: {}", e))?;

        let prov = Self {
            client,
            endpoint: endpoint.to_string(),
            language: language.to_string(),
            quality_steps: AtomicU32::new(
                quality_steps.clamp(MIN_QUALITY_STEPS, MAX_QUALITY_STEPS),
            ),
            speed: AtomicU32::new(speed.clamp(MIN_SPEED, MAX_SPEED).to_bits()),
        };

        if !prov.health_check() {
            log::warn!(
                "[ChatterboxRemote] Initial health check failed for {}",
                endpoint
            );
        } else {
            log::info!(
                "[ChatterboxRemote] Connected to remote TTS server at {}",
                endpoint
            );

            let load_url = format!("{}/models/load", endpoint.trim_end_matches('/'));
            let t3_path = format!(
                "{}/models/tts/chatterbox/chatterbox-t3-mtl-q4_0.gguf",
                remote_path.trim_end_matches('/')
            );
            let s3gen_path = format!(
                "{}/models/tts/chatterbox/chatterbox-s3gen-mtl-f16.gguf",
                remote_path.trim_end_matches('/')
            );

            let payload = serde_json::json!({
                "t3_gguf_path": t3_path,
                "s3gen_gguf_path": s3gen_path,
                "language": language,
                "gpu_layers": 99,
                "cfm_steps": 10,
                "stream_chunk_tokens": 16,
            });

            log::info!(
                "[ChatterboxRemote] Requesting remote model load from: {}",
                load_url
            );
            match prov.client.post(&load_url).json(&payload).send() {
                Ok(res) => {
                    if res.status().is_success() {
                        log::info!(
                            "[ChatterboxRemote] Models loaded successfully on remote GPU server."
                        );
                    } else {
                        log::warn!(
                            "[ChatterboxRemote] Remote load models returned status: {}",
                            res.status()
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[ChatterboxRemote] Failed to send load models command: {}",
                        e
                    );
                }
            }
        }

        Ok(prov)
    }
}

/// Applies time-stretch playback rate scaling on 24kHz audio via linear interpolation.
fn apply_speed_stretch(samples: &[f32], speed: f32) -> Vec<f32> {
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

/// Reads streaming binary f32 PCM samples from HTTP response and streams 2048-sample audio events.
fn stream_pcm_response(
    mut response: reqwest::blocking::Response,
    speed: f32,
    turn_id: u32,
    cancel: &Arc<AtomicBool>,
    event_tx: &Sender<VoxEvent>,
) -> Result<usize> {
    let mut byte_buf = Vec::new();
    let mut raw_pcm_samples = Vec::new();
    let mut total_samples_received = 0;
    let mut buf = [0u8; 8192];

    loop {
        if cancel.load(Ordering::Relaxed) {
            log::info!("[ChatterboxRemote] Synthesis cancelled for turn {}", turn_id);
            return Ok(total_samples_received);
        }

        match response.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                byte_buf.extend_from_slice(&buf[..n]);

                let num_samples = byte_buf.len() / 4;
                if num_samples > 0 {
                    let consumed_bytes = num_samples * 4;
                    for chunk in byte_buf[..consumed_bytes].chunks_exact(4) {
                        let val = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                        raw_pcm_samples.push(val);
                    }
                    byte_buf.drain(..consumed_bytes);
                }

                while raw_pcm_samples.len() >= CHUNK_SIZE {
                    if cancel.load(Ordering::Relaxed) {
                        return Ok(total_samples_received);
                    }

                    let chunk_samples = raw_pcm_samples.drain(..CHUNK_SIZE).collect::<Vec<f32>>();
                    total_samples_received += chunk_samples.len();

                    let stretched_chunk = if (speed - 1.0).abs() >= 0.01 {
                        apply_speed_stretch(&chunk_samples, speed)
                    } else {
                        chunk_samples
                    };

                    if event_tx
                        .send(VoxEvent::TtsChunk {
                            turn_id,
                            samples: stretched_chunk,
                        })
                        .is_err()
                    {
                        log::warn!("[ChatterboxRemote] event_tx closed, stopping synthesis");
                        return Ok(total_samples_received);
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(anyhow!("Error reading remote stream: {}", e)),
        }
    }

    if !raw_pcm_samples.is_empty() {
        total_samples_received += raw_pcm_samples.len();
        let stretched_chunk = if (speed - 1.0).abs() >= 0.01 {
            apply_speed_stretch(&raw_pcm_samples, speed)
        } else {
            raw_pcm_samples
        };

        if let Err(e) = event_tx.send(VoxEvent::TtsChunk {
            turn_id,
            samples: stretched_chunk,
        }) {
            log::warn!("[ChatterboxRemote] Error sending final chunk: {:?}", e);
        }
    }

    Ok(total_samples_received)
}

impl TtsProvider for ChatterboxRemoteProvider {
    /// Hot-updates the number of remote diffusion quality steps.
    fn set_quality_steps(&self, steps: u32) {
        let clamped = steps.clamp(MIN_QUALITY_STEPS, MAX_QUALITY_STEPS);
        self.quality_steps.store(clamped, Ordering::Relaxed);
        log::info!("[ChatterboxRemote] Quality steps set to {}", clamped);
    }

    /// Hot-updates the remote speech playback speed factor.
    fn set_speed(&self, speed: f32) {
        let clamped = speed.clamp(MIN_SPEED, MAX_SPEED);
        self.speed.store(clamped.to_bits(), Ordering::Relaxed);
        log::info!("[ChatterboxRemote] Speed set to {:.2}", clamped);
    }

    /// Returns the TtsProviderKind::ChatterboxRemote variant identifier.
    fn kind(&self) -> TtsProviderKind {
        TtsProviderKind::ChatterboxRemote
    }

    /// Performs HTTP GET health check against the remote server health endpoint.
    fn health_check(&self) -> bool {
        let url = format!("{}/health", self.endpoint.trim_end_matches('/'));
        match self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(3))
            .send()
        {
            Ok(res) => {
                if res.status().is_success() {
                    if let Ok(body) = res.json::<serde_json::Value>() {
                        return body.get("status").and_then(|s| s.as_str()) == Some("ok");
                    }
                }
                false
            }
            Err(_) => false,
        }
    }

    /// Submits synthesis job to remote GPU server and streams audio chunks back to local pipeline.
    fn synthesize_chunk(
        &self,
        text: &str,
        turn_id: u32,
        cancel: Arc<AtomicBool>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<()> {
        if cancel.load(Ordering::Relaxed) || text.trim().is_empty() {
            return Ok(());
        }

        log::info!(
            "[ChatterboxRemote] Synthesizing turn {} via remote: '{}'",
            turn_id,
            text.chars().take(80).collect::<String>()
        );

        let start = std::time::Instant::now();
        let quality_steps = self.quality_steps.load(Ordering::Relaxed);

        let payload = serde_json::json!({
            "text": text,
            "language": self.language,
            "quality_steps": quality_steps
        });

        let url = format!("{}/tts/stream-pcm", self.endpoint.trim_end_matches('/'));

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .map_err(|e| anyhow!("Failed to send remote TTS request to {}: {}", url, e))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().unwrap_or_default();
            return Err(anyhow!(
                "Remote server returned error status {}: {}",
                status,
                err_text
            ));
        }

        let speed = f32::from_bits(self.speed.load(Ordering::Relaxed));
        let total_samples = stream_pcm_response(response, speed, turn_id, &cancel, &event_tx)?;

        let elapsed = start.elapsed().as_secs_f32();
        let audio_duration = total_samples as f32 / 24000.0;
        let rtf = if audio_duration > 0.0 {
            elapsed / audio_duration
        } else {
            0.0
        };

        log::info!(
            "[ChatterboxRemote] Remote synthesis complete (turn {}). {:.2}s audio, RTF: {:.3}",
            turn_id,
            audio_duration,
            rtf,
        );

        if let Err(e) = event_tx.send(VoxEvent::TtsFinished { turn_id, rtf }) {
            log::warn!("[ChatterboxRemote] Error sending TtsFinished: {:?}", e);
        }

        Ok(())
    }
}
