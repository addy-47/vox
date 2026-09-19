use std::path::Path;

use turso::Connection;

use crate::{
    core::settings::{TtsProviderConfig, VoxSettings},
    persistence::voices::get_voice,
    services::tts::{
        providers::TtsProvider, ChatterboxEngine, ChatterboxRemoteProvider, EdgeTtsProvider,
        KokoroEngine, TtsEngine as SupertonicEngine, CHATTERBOX_MODEL_DIR, KOKORO_MODEL_DIR,
    },
    utils::paths::model_dir,
};

/// Resolves a voice UUID to a WAV file path for Chatterbox voice conditioning.
pub async fn resolve_reference_audio(conn: &Connection, voice_id: Option<&str>) -> Option<String> {
    let id = voice_id?;
    let entry = get_voice(conn, id).await.ok()??;

    if let Some(ref dir) = entry.voice_dir {
        let path = Path::new(dir);
        if path.exists() && path.join("speaker_emb.npy").exists() {
            return Some(dir.clone());
        }
    }

    let wav = entry.wav_path?;
    if !Path::new(&wav).exists() {
        log::warn!(
            "[TTS Factory] Voice {} wav_path not found on disk: {}. Using built-in voice.",
            id,
            wav
        );
        return None;
    }
    Some(wav)
}

/// Creates a boxed TTS provider based on settings configuration.
pub fn create_tts_provider(
    settings: &VoxSettings,
    super_tts_path: &Path,
    reference_audio: Option<&str>,
) -> Result<Box<dyn TtsProvider>, String> {
    let provider_config = settings.tts.to_provider_config();
    let voice = settings.tts.voice_index;
    let quality_steps = settings.tts.quality_steps;
    let speed = settings.tts.speed;
    let num_threads = settings.tts.threads;

    match &provider_config {
        TtsProviderConfig::Supertonic => {
            log::info!("[TTS Factory] Initializing Supertonic engine");
            SupertonicEngine::new(super_tts_path, voice, quality_steps, speed, num_threads)
                .map(|e| Box::new(e) as Box<dyn TtsProvider>)
                .map_err(|e| format!("Failed to create Supertonic engine: {}", e))
        }
        TtsProviderConfig::Kokoro => {
            log::info!("[TTS Factory] Initializing Kokoro Multi-Lang engine");
            let kokoro_path = model_dir(KOKORO_MODEL_DIR);
            KokoroEngine::new(&kokoro_path, voice, speed, num_threads)
                .map(|e| Box::new(e) as Box<dyn TtsProvider>)
                .map_err(|e| format!("Failed to create Kokoro engine: {}", e))
        }
        TtsProviderConfig::Chatterbox {
            language,
            quality_steps: cb_quality,
            speed: cb_speed,
            voice_id: _,
        } => {
            log::info!("[TTS Factory] Initializing Chatterbox engine");
            let chatterbox_path = model_dir(CHATTERBOX_MODEL_DIR);
            ChatterboxEngine::new(
                &chatterbox_path,
                language,
                *cb_quality,
                *cb_speed,
                reference_audio,
            )
            .map(|e| Box::new(e) as Box<dyn TtsProvider>)
            .map_err(|e| format!("Failed to create Chatterbox engine: {}", e))
        }
        TtsProviderConfig::ChatterboxRemote {
            endpoint,
            language,
            quality_steps: remote_quality,
            speed: remote_speed,
            remote_path,
            voice_id: _,
        } => {
            log::info!("[TTS Factory] Initializing ChatterboxRemote provider");
            ChatterboxRemoteProvider::new(
                endpoint,
                language,
                *remote_quality,
                *remote_speed,
                remote_path,
            )
            .map(|p| Box::new(p) as Box<dyn TtsProvider>)
            .map_err(|e| format!("Failed to create ChatterboxRemote provider: {}", e))
        }
        TtsProviderConfig::EdgeTts { voice: edge_voice } => {
            log::info!("[TTS Factory] Initializing EdgeTTS provider");
            Ok(Box::new(EdgeTtsProvider::new(edge_voice.as_deref())))
        }
    }
}
