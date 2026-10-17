use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsModelConfig, OfflineTtsSupertonicModelConfig,
};
use tokio::sync::Mutex;

use crate::{
    core::{
        defaults::{DEFAULT_TTS_SPEED, DEFAULT_TTS_THREADS},
        error::VoxIpcError,
        state::{AppState, InteractionState},
    },
    persistence::voices::get_voice,
    services::{
        translit::is_devanagari,
        tts::{
            factory::caps_for_id,
            providers::{
                azure_tts, edge_tts, google_tts,
                kokoro::{
                    KOKORO_SILENCE_SCALE, MODEL_DIRNAME_TTS_KOKORO_ESPEAK,
                    MODEL_FILE_TTS_KOKORO_LEXICON_US, MODEL_FILE_TTS_KOKORO_MODEL,
                    MODEL_FILE_TTS_KOKORO_TOKENS, MODEL_FILE_TTS_KOKORO_VOICES,
                },
                supertonic::{
                    resample_44100_to_24000, BiquadFilter, MODEL_FILE_TTS_SUPER_CONFIG,
                    MODEL_FILE_TTS_SUPER_DURATION_PREDICTOR, MODEL_FILE_TTS_SUPER_INDEXER,
                    MODEL_FILE_TTS_SUPER_TEXT_ENCODER, MODEL_FILE_TTS_SUPER_VECTOR_ESTIMATOR,
                    MODEL_FILE_TTS_SUPER_VOCODER, MODEL_FILE_TTS_SUPER_VOICE, SUPERTONIC_STEPS,
                },
            },
            voice::{
                get_supertonic_voice_profiles, get_voice_profiles, resolve_catalog_index,
                write_f32_wav,
            },
            KOKORO_MODEL_DIR, SUPERTONIC_MODEL_DIR, TTS_SAMPLE_RATE,
        },
    },
    utils::paths::{cache_dir, model_dir},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoicePreviewResult {
    pub cached: bool,
    pub path: String,
    pub duration_ms: u64,
    pub audio_base64: Option<String>,
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

    /// Resolves cached preview audio or synthesizes it with the voice's owning provider.
    pub async fn request_preview(
        &self,
        voice_id: &str,
        prompt: Option<&str>,
        engine: Option<&str>,
        state: &Arc<AppState>,
    ) -> Result<VoicePreviewResult, VoxIpcError> {
        let interaction_state = state.pipeline.state();
        if interaction_state != InteractionState::Idle {
            return Err(VoxIpcError::Internal(
                "Cannot preview voice while assistant session is busy".to_string(),
            ));
        }

        let clean_prompt = resolve_preview_text(prompt);
        let previews_dir = cache_dir().join("tts").join("previews");
        if let Err(e) = std::fs::create_dir_all(&previews_dir) {
            log::warn!("[Preview] Failed to create previews dir: {}", e);
        }

        let mut hasher = Sha256::new();
        hasher.update(format!("{}_{}", voice_id, clean_prompt).as_bytes());
        let key = format!("{:x}.wav", hasher.finalize());
        let target_path = previews_dir.join(&key);

        if target_path.exists() {
            let duration_ms = read_wav_duration_ms(&target_path).unwrap_or(2500);
            let audio_base64 = std::fs::read(&target_path)
                .ok()
                .map(|b| base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &b));
            log::info!(
                "[Preview] Cache hit voice={} duration_ms={} path={}",
                voice_id,
                duration_ms,
                key
            );
            return Ok(VoicePreviewResult {
                cached: true,
                path: target_path.to_string_lossy().into_owned(),
                duration_ms,
                audio_base64,
            });
        }

        let provider = resolve_preview_provider(state, voice_id, engine).await?;
        let (speed, threads) = (DEFAULT_TTS_SPEED, DEFAULT_TTS_THREADS);
        log::info!(
            "[Preview] Synthesizing voice={} provider={} engine_hint={:?}",
            voice_id,
            provider,
            engine
        );

        let voice_id_owned = voice_id.to_string();
        let prompt_owned = clean_prompt;
        let provider_owned = provider;
        let target_path_clone = target_path.clone();
        let state_clone = Arc::clone(state);
        let (task, handle) = futures_util::future::abortable(async move {
            synthesize_for_provider(
                &state_clone,
                &provider_owned,
                &voice_id_owned,
                &prompt_owned,
                speed,
                threads,
                &target_path_clone,
            )
            .await
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
                let audio_base64 = std::fs::read(&target_path)
                    .ok()
                    .map(|b| base64::Engine::encode(&base64::prelude::BASE64_STANDARD, &b));
                log::info!(
                    "[Preview] Synthesized voice={} duration_ms={} bytes={}",
                    voice_id,
                    duration_ms,
                    audio_base64.as_ref().map(|s| s.len()).unwrap_or(0)
                );
                self.enforce_lru_cache_bounds(&previews_dir).await;
                Ok(VoicePreviewResult {
                    cached: false,
                    path: target_path.to_string_lossy().into_owned(),
                    duration_ms,
                    audio_base64,
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

/// Trims the preview prompt to the 120-character ceiling with a default fallback.
fn resolve_preview_text(prompt: Option<&str>) -> String {
    const DEFAULT_TEXT: &str = "Hello, this is a preview of my voice.";
    const MAX_CHARS: usize = 120;
    let clean = prompt.map(|p| p.trim()).filter(|p| !p.is_empty());
    match clean {
        Some(p) if p.len() > MAX_CHARS => p[..MAX_CHARS].to_string(),
        Some(p) => p.to_string(),
        None => DEFAULT_TEXT.to_string(),
    }
}

/// Resolves the owning provider for a voice: Turso row first, modal engine hint second, saved engine last.
async fn resolve_preview_provider(
    state: &AppState,
    voice_id: &str,
    engine: Option<&str>,
) -> Result<String, VoxIpcError> {
    if let Ok(conn) = state.db.connect() {
        if let Ok(Some(entry)) = get_voice(&conn, voice_id).await {
            if !entry.provider_id.trim().is_empty() {
                return Ok(entry.provider_id);
            }
        }
    }
    if let Some(normalized) = normalize_engine_id(engine.unwrap_or_default()) {
        return Ok(normalized);
    }
    if let Ok(settings) = state.settings.read() {
        let active = settings.tts.active.model.clone();
        if !active.trim().is_empty() {
            return Ok(active);
        }
    }
    if voice_id.contains('-') {
        return Ok("edge_tts".to_string());
    }
    Err(VoxIpcError::InvalidArgument(format!(
        "Cannot resolve provider for voice '{}'",
        voice_id
    )))
}

/// Normalizes a frontend engine id to a known backend provider id.
fn normalize_engine_id(engine: &str) -> Option<String> {
    let normalized = engine.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    match caps_for_id(&normalized) {
        Ok(_) => Some(normalized),
        Err(_) => None,
    }
}

/// Dispatches preview synthesis to the voice's owning provider on the correct thread.
async fn synthesize_for_provider(
    state: &AppState,
    provider: &str,
    voice_id: &str,
    text: &str,
    speed: f32,
    threads: u32,
    target_path: &Path,
) -> Result<()> {
    if let Some(clip) = find_reference_clip(voice_id) {
        std::fs::copy(&clip, target_path).context("Failed to copy reference clip")?;
        log::info!(
            "[Preview] Reference clip voice={} clip={:?}",
            voice_id,
            clip
        );
        return Ok(());
    }
    match provider {
        "supertonic" => {
            let (voice, text_o, target_o) = (
                voice_id.to_string(),
                text.to_string(),
                target_path.to_path_buf(),
            );
            tokio::task::spawn_blocking(move || {
                synthesize_supertonic_preview(&voice, &text_o, speed, threads, &target_o)
            })
            .await
            .context("Preview synthesis task panicked")??;
            Ok(())
        }
        "kokoro" => {
            let (voice, text_o, target_o) = (
                voice_id.to_string(),
                text.to_string(),
                target_path.to_path_buf(),
            );
            tokio::task::spawn_blocking(move || {
                synthesize_kokoro_preview(&voice, &text_o, speed, threads, &target_o)
            })
            .await
            .context("Preview synthesis task panicked")??;
            Ok(())
        }
        "edge_tts" => {
            log::info!("[Preview] Edge online synthesis voice={}", voice_id);
            edge_tts::synthesize_to_wav(voice_id, text, target_path).await
        }
        "google_tts" => {
            log::info!("[Preview] Google Cloud TTS online synthesis voice={}", voice_id);
            let project_override = state
                .settings
                .read()
                .ok()
                .and_then(|s| s.providers.tts.google_tts.project_override.clone());
            google_tts::synthesize_to_wav(
                voice_id,
                text,
                speed,
                project_override.as_deref(),
                target_path,
            )
            .await
        }
        "azure_tts" => {
            log::info!("[Preview] Azure AI Speech online synthesis voice={}", voice_id);
            let (region, key, resource_id): (String, String, Option<String>) = {
                let s = state.settings.read().map_err(|e| anyhow!("Failed to read settings: {}", e))?;
                (
                    s.providers.tts.azure_tts.region_or_endpoint.clone(),
                    s.providers.tts.azure_tts.key.clone(),
                    s.providers.tts.azure_tts.resource_id.clone(),
                )
            };
            azure_tts::synthesize_to_wav(
                &region,
                voice_id,
                &key,
                resource_id.as_deref(),
                text,
                speed,
                target_path,
            )
            .await
        }
        "chatterbox" | "chatterbox_remote" | "zipvoice" => Err(anyhow::anyhow!(
            "Voice preview unavailable for '{}': reference audio is not installed.",
            voice_id
        )),
        _ if voice_id.contains('-') => {
            edge_tts::synthesize_to_wav(voice_id, text, target_path).await
        }
        _ => Err(anyhow::anyhow!(
            "Voice preview unavailable for '{}' (provider '{}').",
            voice_id,
            provider
        )),
    }
}

/// Locates a voice's own reference audio when its family ships or stores one.
fn find_reference_clip(voice_id: &str) -> Option<PathBuf> {
    let zipvoice_clip = model_dir("tts/zipvoice/voices")
        .join(voice_id)
        .join("clip.wav");
    if zipvoice_clip.exists() {
        return Some(zipvoice_clip);
    }
    let zipvoice_fp32_clip = model_dir("tts/zipvoice-fp32/voices")
        .join(voice_id)
        .join("clip.wav");
    if zipvoice_fp32_clip.exists() {
        return Some(zipvoice_fp32_clip);
    }
    let clean_chatterbox_id = voice_id
        .strip_prefix("chatterbox_voice_")
        .unwrap_or(voice_id);
    let chatterbox_source = model_dir("tts/chatterbox/voices")
        .join(clean_chatterbox_id)
        .join("source.wav");
    if chatterbox_source.exists() {
        return Some(chatterbox_source);
    }
    let custom_voice_source = crate::utils::paths::voice_dir(voice_id).join("source.wav");
    if custom_voice_source.exists() {
        return Some(custom_voice_source);
    }
    let bundled_clip = model_dir("tts/previews").join(format!("{}.wav", voice_id));
    if bundled_clip.exists() {
        return Some(bundled_clip);
    }
    None
}

/// Synthesizes catalog preview audio with the local Supertonic ONNX model on a blocking thread.
fn synthesize_supertonic_preview(
    voice_id: &str,
    text: &str,
    speed: f32,
    threads: u32,
    target_path: &Path,
) -> Result<()> {
    let model_path = model_dir(SUPERTONIC_MODEL_DIR);
    let mp = |f: &str| -> String { model_path.join(f).to_string_lossy().into() };
    let config = OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            supertonic: OfflineTtsSupertonicModelConfig {
                duration_predictor: Some(mp(MODEL_FILE_TTS_SUPER_DURATION_PREDICTOR)),
                text_encoder: Some(mp(MODEL_FILE_TTS_SUPER_TEXT_ENCODER)),
                vector_estimator: Some(mp(MODEL_FILE_TTS_SUPER_VECTOR_ESTIMATOR)),
                vocoder: Some(mp(MODEL_FILE_TTS_SUPER_VOCODER)),
                tts_json: Some(mp(MODEL_FILE_TTS_SUPER_CONFIG)),
                unicode_indexer: Some(mp(MODEL_FILE_TTS_SUPER_INDEXER)),
                voice_style: Some(mp(MODEL_FILE_TTS_SUPER_VOICE)),
            },
            num_threads: threads as i32,
            debug: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let tts = OfflineTts::create(&config).context("Supertonic preview engine failed to load")?;
    let sid = resolve_catalog_index(&get_supertonic_voice_profiles(), voice_id);
    let lang = if is_devanagari(text) { "hi" } else { "en" };
    let mut extra = HashMap::new();
    extra.insert("lang".to_string(), json!(lang));
    let gen_config = GenerationConfig {
        sid,
        num_steps: SUPERTONIC_STEPS,
        speed,
        silence_scale: 0.85,
        extra: Some(extra),
        ..Default::default()
    };
    let audio = tts
        .generate_with_config(text, &gen_config, None::<fn(&[f32], f32) -> bool>)
        .context("Supertonic preview generation failed")?;
    let mut lpf = BiquadFilter::new_lpf_11k();
    let samples_24k = resample_44100_to_24000(audio.samples(), &mut lpf);
    if samples_24k.is_empty() {
        anyhow::bail!("Supertonic preview generated no audio");
    }
    write_f32_wav(target_path, &samples_24k, TTS_SAMPLE_RATE)
        .map_err(|e| anyhow::anyhow!("Failed to write preview WAV: {}", e))?;
    log::info!(
        "[Preview] Supertonic synthesis voice={} sid={}",
        voice_id,
        sid
    );
    Ok(())
}

/// Synthesizes catalog preview audio with the local Kokoro ONNX model on a blocking thread.
fn synthesize_kokoro_preview(
    voice_id: &str,
    text: &str,
    speed: f32,
    threads: u32,
    target_path: &Path,
) -> Result<()> {
    let model_path = model_dir(KOKORO_MODEL_DIR);
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
            num_threads: threads as i32,
            debug: false,
            provider: Some("cpu".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let tts = OfflineTts::create(&config).context("Kokoro preview engine failed to load")?;
    let sample_rate = tts.sample_rate() as u32;
    let sid = resolve_catalog_index(&get_voice_profiles(), voice_id);
    let gen_config = GenerationConfig {
        sid,
        speed,
        silence_scale: KOKORO_SILENCE_SCALE,
        ..Default::default()
    };
    let audio = tts
        .generate_with_config(text, &gen_config, None::<fn(&[f32], f32) -> bool>)
        .context("Kokoro preview generation failed")?;
    if audio.samples().is_empty() {
        anyhow::bail!("Kokoro preview generated no audio");
    }
    write_f32_wav(target_path, audio.samples(), sample_rate)
        .map_err(|e| anyhow::anyhow!("Failed to write preview WAV: {}", e))?;
    log::info!("[Preview] Kokoro synthesis voice={} sid={}", voice_id, sid);
    Ok(())
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
