use std::path::Path;

use turso::Connection;

use crate::{
    persistence::voices::get_voice,
    services::tts::{
        providers::TtsProvider, resolve_zipvoice_reference, ChatterboxEngine,
        ChatterboxRemoteProvider, EdgeTtsProvider, KokoroEngine, ProviderCaps,
        TtsActiveProvider, TtsEngine as SupertonicEngine, TtsProviderConfig, TtsSettings,
        ZipvoiceEngine, CHATTERBOX_MODEL_DIR, KOKORO_MODEL_DIR, ZIPVOICE_MODEL_DIR,
    },
    utils::paths::model_dir,
};

/// Resolves capabilities for a provider id, erroring on an unrecognised one.
pub fn caps_for_id(provider_id: &str) -> Result<ProviderCaps, String> {
    let provider: TtsActiveProvider = serde_json::from_value(serde_json::Value::String(
        provider_id.to_string(),
    ))
    .map_err(|_| {
        format!(
            "Unknown TTS provider id: {provider_id} (expected one of supertonic, kokoro, \
             chatterbox, chatterbox_remote, edge_tts, zipvoice)"
        )
    })?;
    Ok(match provider {
        TtsActiveProvider::Supertonic => SupertonicEngine::caps(),
        TtsActiveProvider::Kokoro => KokoroEngine::caps(),
        TtsActiveProvider::Chatterbox => ChatterboxEngine::caps(),
        TtsActiveProvider::ChatterboxRemote => ChatterboxRemoteProvider::caps(),
        TtsActiveProvider::EdgeTts => EdgeTtsProvider::caps(),
        TtsActiveProvider::Zipvoice => ZipvoiceEngine::caps(),
    })
}

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
    settings: &TtsSettings,
    super_tts_path: &Path,
    reference_audio: Option<&str>,
) -> Result<Box<dyn TtsProvider>, String> {
    let provider_config = settings.to_provider_config();
    let voice = settings.voice_index;
    let speed = settings.speed;
    let num_threads = settings.threads;

    match &provider_config {
        TtsProviderConfig::Supertonic => {
            log::info!("[TTS Factory] Initializing Supertonic engine");
            SupertonicEngine::new(super_tts_path, voice, speed, num_threads)
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
            speed: cb_speed,
            voice_id: _,
        } => {
            log::info!("[TTS Factory] Initializing Chatterbox engine");
            let chatterbox_path = model_dir(CHATTERBOX_MODEL_DIR);
            ChatterboxEngine::new(&chatterbox_path, language, *cb_speed, reference_audio)
                .map(|e| Box::new(e) as Box<dyn TtsProvider>)
                .map_err(|e| format!("Failed to create Chatterbox engine: {}", e))
        }
        TtsProviderConfig::ChatterboxRemote {
            endpoint,
            language,
            speed: remote_speed,
            remote_path,
            voice_id: _,
        } => {
            log::info!("[TTS Factory] Initializing ChatterboxRemote provider");
            ChatterboxRemoteProvider::new(endpoint, language, *remote_speed, remote_path)
                .map(|p| Box::new(p) as Box<dyn TtsProvider>)
                .map_err(|e| format!("Failed to create ChatterboxRemote provider: {}", e))
        }
        TtsProviderConfig::EdgeTts { voice: edge_voice } => {
            log::info!("[TTS Factory] Initializing EdgeTTS provider");
            Ok(Box::new(EdgeTtsProvider::new(edge_voice.as_deref())))
        }
        TtsProviderConfig::Zipvoice {
            voice_id,
            guidance_scale,
        } => {
            log::info!("[TTS Factory] Initializing ZipVoice engine");
            let zipvoice_path = model_dir(ZIPVOICE_MODEL_DIR);
            let voices_dir = zipvoice_path.join("voices");
            let initial_ref = match resolve_zipvoice_reference(&voices_dir, voice_id.as_deref()) {
                Ok(reference) => Some(reference),
                Err(e) => {
                    log::warn!("[TTS Factory] ZipVoice reference resolution failed: {}", e);
                    None
                }
            };
            ZipvoiceEngine::new(
                &zipvoice_path,
                speed,
                *guidance_scale,
                num_threads,
                initial_ref,
            )
            .map(|e| Box::new(e) as Box<dyn TtsProvider>)
            .map_err(|e| format!("Failed to create ZipVoice engine: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tts::{ProviderCaps, TtsVoiceSource};

    #[test]
    fn test_caps_for_id_matrix() {
        assert_eq!(
            caps_for_id("supertonic").expect("supertonic caps"),
            ProviderCaps {
                voices: TtsVoiceSource::Catalog,
                clone: false,
                speed_range: SupertonicEngine::SPEED_RANGE,
            }
        );
        assert_eq!(
            caps_for_id("kokoro").expect("kokoro caps"),
            caps_for_id("supertonic").expect("supertonic caps")
        );
        let chatterbox = caps_for_id("chatterbox").expect("chatterbox caps");
        assert_eq!(chatterbox.voices, TtsVoiceSource::Custom);
        assert!(chatterbox.clone);
        assert_eq!(
            caps_for_id("chatterbox_remote").expect("chatterbox_remote caps"),
            chatterbox
        );
        let edge = caps_for_id("edge_tts").expect("edge_tts caps");
        assert_eq!(edge.voices, TtsVoiceSource::Edge);
        assert!(!edge.clone);
        assert_ne!(
            edge.speed_range,
            SupertonicEngine::SPEED_RANGE,
            "edge_tts must keep its wider speed range"
        );
        let zipvoice = caps_for_id("zipvoice").expect("zipvoice caps");
        assert_eq!(zipvoice.voices, TtsVoiceSource::Custom);
        assert!(!zipvoice.clone);
    }

    #[test]
    fn test_caps_for_id_rejects_unknown() {
        let err = caps_for_id("no_such_engine").expect_err("unknown provider id must not resolve");
        assert!(err.contains("no_such_engine"), "error should name the id");
        assert!(caps_for_id("").is_err(), "empty id must not resolve");
        assert!(
            caps_for_id("Kokoro").is_err(),
            "matching must be exact, not case-insensitive"
        );
    }
}
