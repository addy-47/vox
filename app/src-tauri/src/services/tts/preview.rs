use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::{
    core::{
        error::VoxIpcError,
        state::{AppState, InteractionState},
    },
    services::tts::providers::edge_tts,
    utils::paths::{cache_dir, model_dir},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoicePreviewResult {
    pub cached: bool,
    pub path: String,
    pub duration_ms: u64,
}

static PREVIEW_SYNTHESIZER: once_cell::sync::Lazy<PreviewSynthesizer> =
    once_cell::sync::Lazy::new(PreviewSynthesizer::new);

pub struct PreviewSynthesizer {
    active_abort: Arc<Mutex<Option<futures_util::future::AbortHandle>>>,
}

impl Default for PreviewSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewSynthesizer {
    pub fn new() -> Self {
        Self {
            active_abort: Arc::new(Mutex::new(None)),
        }
    }

    pub fn global() -> &'static Self {
        &PREVIEW_SYNTHESIZER
    }

    pub async fn request_preview(
        &self,
        voice_id: &str,
        prompt: Option<&str>,
        state: &AppState,
    ) -> Result<VoicePreviewResult, VoxIpcError> {
        // 1. Session gate: Do not run preview if conversation is active
        let interaction_state = state.pipeline.state();
        if interaction_state != InteractionState::Idle {
            return Err(VoxIpcError::Internal(
                "Cannot preview voice while assistant session is busy".to_string(),
            ));
        }

        // 2. Resolve clean prompt with 120-char limit
        let default_text = "Hello, this is a preview of my voice.";
        let clean_prompt = prompt
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .unwrap_or(default_text);
        let clean_prompt = if clean_prompt.len() > 120 {
            &clean_prompt[..120]
        } else {
            clean_prompt
        };

        // 3. Compute cache path in ~/.vox/cache/tts/previews/
        let previews_dir = cache_dir().join("tts").join("previews");
        let _ = std::fs::create_dir_all(&previews_dir);

        let mut hasher = Sha256::new();
        hasher.update(format!("{}_{}", voice_id, clean_prompt).as_bytes());
        let key = format!("{:x}.wav", hasher.finalize());
        let target_path = previews_dir.join(&key);

        if target_path.exists() {
            let duration_ms = read_wav_duration_ms(&target_path).unwrap_or(2500);
            return Ok(VoicePreviewResult {
                cached: true,
                path: target_path.to_string_lossy().into_owned(),
                duration_ms,
            });
        }

        // 4. Abort any previous in-flight preview
        let voice_id_owned = voice_id.to_string();
        let prompt_owned = clean_prompt.to_string();
        let target_path_clone = target_path.clone();

        let (task, handle) = futures_util::future::abortable(async move {
            synthesize_voice_to_path(&voice_id_owned, &prompt_owned, &target_path_clone).await
        });

        {
            let mut lock = self.active_abort.lock().await;
            if let Some(prev) = lock.take() {
                prev.abort();
            }
            *lock = Some(handle);
        }

        match task.await {
            Ok(Ok(())) => {
                let duration_ms = read_wav_duration_ms(&target_path).unwrap_or(2500);
                self.enforce_lru_cache_bounds(&previews_dir).await;
                Ok(VoicePreviewResult {
                    cached: false,
                    path: target_path.to_string_lossy().into_owned(),
                    duration_ms,
                })
            }
            Ok(Err(e)) => Err(VoxIpcError::Engine(format!(
                "Preview synthesis failed: {}",
                e
            ))),
            Err(_) => Err(VoxIpcError::Internal(
                "Preview synthesis cancelled".to_string(),
            )),
        }
    }

    async fn enforce_lru_cache_bounds(&self, dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut files: Vec<(PathBuf, SystemTime, u64)> = Vec::new();
            let mut total_bytes: u64 = 0;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("wav") {
                    if let Ok(meta) = entry.metadata() {
                        let len = meta.len();
                        let modified = meta.modified().unwrap_or(UNIX_EPOCH);
                        total_bytes += len;
                        files.push((path, modified, len));
                    }
                }
            }

            // Cap at 50 files or 25 MB
            const MAX_FILES: usize = 50;
            const MAX_BYTES: u64 = 25 * 1024 * 1024;
            if files.len() > MAX_FILES || total_bytes > MAX_BYTES {
                files.sort_by_key(|(_, modified, _)| *modified);
                for (path, _, len) in files {
                    let _ = std::fs::remove_file(path);
                    total_bytes = total_bytes.saturating_sub(len);
                    if total_bytes <= MAX_BYTES {
                        break;
                    }
                }
            }
        }
    }
}

async fn synthesize_voice_to_path(voice_id: &str, text: &str, target_path: &Path) -> Result<()> {
    // 1. Packaged clips for Zipvoice
    let zipvoice_clip = model_dir("tts/zipvoice/voices").join(voice_id).join("clip.wav");
    if zipvoice_clip.exists() {
        std::fs::copy(&zipvoice_clip, target_path)?;
        return Ok(());
    }

    let zipvoice_fp32_clip = model_dir("tts/zipvoice-fp32/voices").join(voice_id).join("clip.wav");
    if zipvoice_fp32_clip.exists() {
        std::fs::copy(&zipvoice_fp32_clip, target_path)?;
        return Ok(());
    }

    // 2. Chatterbox packaged voices
    let clean_chatterbox_id = voice_id.strip_prefix("chatterbox_voice_").unwrap_or(voice_id);
    let chatterbox_source = model_dir("tts/chatterbox/voices").join(clean_chatterbox_id).join("source.wav");
    if chatterbox_source.exists() {
        std::fs::copy(&chatterbox_source, target_path)?;
        return Ok(());
    }

    // 3. User cloned custom voices in ~/.vox/voices/{id}/source.wav
    let custom_voice_source = crate::utils::paths::voice_dir(voice_id).join("source.wav");
    if custom_voice_source.exists() {
        std::fs::copy(&custom_voice_source, target_path)?;
        return Ok(());
    }

    // 4. Pre-bundled voice sample clips in models/tts/previews/{id}.wav
    let bundled_clip = model_dir("tts/previews").join(format!("{}.wav", voice_id));
    if bundled_clip.exists() {
        std::fs::copy(&bundled_clip, target_path)?;
        return Ok(());
    }

    // 5. Native Edge TTS online voice preview
    if voice_id.contains('-') {
        return edge_tts::synthesize_to_wav(voice_id, text, target_path).await;
    }

    // 6. Online fallback for bundled voices when audio clip is not yet pre-rendered on disk
    let fallback_edge_voice = match voice_id.to_lowercase().as_str() {
        "bella" | "aria" => "en-US-AriaNeural",
        "nova" | "nicole" => "en-US-JennyNeural",
        "ethan" | "adam" => "en-US-GuyNeural",
        "liam" => "en-US-ChristopherNeural",
        "kai" => "en-US-EricNeural",
        "serene" => "en-US-MichelleNeural",
        "jun" => "en-US-AndrewNeural",
        "emma" | "isabella" => "en-GB-SoniaNeural",
        "george" => "en-GB-RyanNeural",
        "james" | "david" | "alex" | "ryan" => "en-US-GuyNeural",
        "sophia" | "olivia" | "ava" | "mia" => "en-US-AriaNeural",
        _ => "en-US-AriaNeural",
    };

    match edge_tts::synthesize_to_wav(fallback_edge_voice, text, target_path).await {
        Ok(()) => Ok(()),
        Err(e) => {
            log::warn!(
                "[Preview] Online fallback synthesis for '{}' failed: {}. Offline mode requires pre-baked preview clips.",
                voice_id,
                e
            );
            Err(anyhow::anyhow!(
                "Voice preview unavailable offline for '{}'. Connect to the internet or install voice model clips.",
                voice_id
            ))
        }
    }
}

fn read_wav_duration_ms(path: &Path) -> Result<u64> {
    let reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.sample_rate == 0 {
        return Ok(2500);
    }
    let duration_sec = reader.duration() as f64 / spec.sample_rate as f64;
    Ok((duration_sec * 1000.0) as u64)
}
