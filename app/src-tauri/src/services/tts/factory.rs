use std::path::Path;

use turso::Connection;

use crate::{
    core::compute::{self, Execution, Workload},
    persistence::voices::get_voice,
    services::tts::{
        providers::TtsProvider, resolve_zipvoice_reference,
        voice::resolve_voice_index_for_provider, AzureTtsProvider, ChatterboxEngine,
        ChatterboxRemoteProvider, EdgeTtsProvider, GoogleTtsProvider, KokoroEngine, ProviderCaps,
        TtsActiveProvider, TtsEngine as SupertonicEngine, TtsProviderConfig, TtsProvidersConfig,
        TtsSettings, ZipvoiceEngine, CHATTERBOX_MODEL_DIR, KOKORO_MODEL_DIR,
        ZIPVOICE_GUIDANCE_SCALE, ZIPVOICE_MODEL_DIR,
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
             chatterbox, chatterbox_remote, edge_tts, zipvoice, azure_tts, google_tts)"
        )
    })?;
    Ok(match provider {
        TtsActiveProvider::Supertonic => SupertonicEngine::caps(),
        TtsActiveProvider::Kokoro => KokoroEngine::caps(),
        TtsActiveProvider::Chatterbox => ChatterboxEngine::caps(),
        TtsActiveProvider::ChatterboxRemote => ChatterboxRemoteProvider::caps(),
        TtsActiveProvider::EdgeTts => EdgeTtsProvider::caps(),
        TtsActiveProvider::Zipvoice => ZipvoiceEngine::caps(),
        TtsActiveProvider::AzureTts => AzureTtsProvider::caps(),
        TtsActiveProvider::GoogleTts => GoogleTtsProvider::caps(),
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

/// Maps a resolved TTS provider configuration to the site that runs it.
fn execution_for(config: &TtsProviderConfig) -> Execution {
    match config {
        TtsProviderConfig::Supertonic
        | TtsProviderConfig::Kokoro
        | TtsProviderConfig::Chatterbox { .. }
        | TtsProviderConfig::Zipvoice { .. } => Execution::Embedded,
        TtsProviderConfig::ChatterboxRemote { .. }
        | TtsProviderConfig::EdgeTts { .. }
        | TtsProviderConfig::AzureTts { .. }
        | TtsProviderConfig::GoogleTts { .. } => Execution::Remote,
    }
}

/// Creates a boxed TTS provider based on settings configuration.
pub fn create_tts_provider(
    settings: &TtsSettings,
    providers: Option<&TtsProvidersConfig>,
    super_tts_path: &Path,
    reference_audio: Option<&str>,
    speed: f32,
) -> Result<Box<dyn TtsProvider>, String> {
    let provider_config = settings.to_provider_config(providers, speed);
    let num_threads = compute::resolve_threads(Workload::Tts, execution_for(&provider_config));

    match &provider_config {
        TtsProviderConfig::Supertonic => {
            log::info!("[TTS Factory] Initializing Supertonic engine");
            let voice = resolve_voice_index_for_provider("supertonic", &settings.active_voice_id)
                .map_err(|e| format!("Failed to create Supertonic engine: {}", e))?;
            SupertonicEngine::new(super_tts_path, voice, speed, num_threads)
                .map(|e| Box::new(e) as Box<dyn TtsProvider>)
                .map_err(|e| format!("Failed to create Supertonic engine: {}", e))
        }
        TtsProviderConfig::Kokoro => {
            log::info!("[TTS Factory] Initializing Kokoro Multi-Lang engine");
            let kokoro_path = model_dir(KOKORO_MODEL_DIR);
            let voice = resolve_voice_index_for_provider("kokoro", &settings.active_voice_id)
                .map_err(|e| format!("Failed to create Kokoro engine: {}", e))?;
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
        TtsProviderConfig::AzureTts {
            region_or_endpoint,
            voice,
            key,
            resource_id,
        } => {
            log::info!("[TTS Factory] Initializing AzureTTS provider");
            AzureTtsProvider::new(region_or_endpoint, voice, key, resource_id.as_deref())
                .map(|p| Box::new(p) as Box<dyn TtsProvider>)
                .map_err(|e| format!("Failed to create AzureTTS provider: {}", e))
        }
        TtsProviderConfig::GoogleTts {
            voice,
            project_override,
        } => {
            log::info!("[TTS Factory] Initializing GoogleTTS provider");
            GoogleTtsProvider::new(voice, project_override.as_deref())
                .map(|p| Box::new(p) as Box<dyn TtsProvider>)
                .map_err(|e| format!("Failed to create GoogleTTS provider: {}", e))
        }
        TtsProviderConfig::Zipvoice {
            voice_id,
            guidance_scale: _,
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
                ZIPVOICE_GUIDANCE_SCALE,
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
                execution: crate::services::tts::TtsExecutionKind::Embedded,
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
            chatterbox.execution,
            crate::services::tts::TtsExecutionKind::Embedded
        );

        let chatterbox_remote = caps_for_id("chatterbox_remote").expect("chatterbox_remote caps");
        assert_eq!(chatterbox_remote.voices, TtsVoiceSource::Custom);
        assert!(chatterbox_remote.clone);
        assert_eq!(
            chatterbox_remote.execution,
            crate::services::tts::TtsExecutionKind::Remote
        );

        let edge = caps_for_id("edge_tts").expect("edge_tts caps");
        assert_eq!(edge.voices, TtsVoiceSource::Edge);
        assert!(!edge.clone);
        assert_eq!(
            edge.execution,
            crate::services::tts::TtsExecutionKind::Cloud
        );
        assert_ne!(
            edge.speed_range,
            SupertonicEngine::SPEED_RANGE,
            "edge_tts must keep its wider speed range"
        );
        let zipvoice = caps_for_id("zipvoice").expect("zipvoice caps");
        assert_eq!(zipvoice.voices, TtsVoiceSource::Custom);
        assert!(!zipvoice.clone);
        assert_eq!(
            zipvoice.execution,
            crate::services::tts::TtsExecutionKind::Embedded
        );

        let azure = caps_for_id("azure_tts").expect("azure_tts caps");
        assert_eq!(azure.voices, TtsVoiceSource::Cloud);
        assert!(!azure.clone);
        assert_eq!(
            azure.execution,
            crate::services::tts::TtsExecutionKind::Cloud
        );

        let google = caps_for_id("google_tts").expect("google_tts caps");
        assert_eq!(google.voices, TtsVoiceSource::Cloud);
        assert!(!google.clone);
        assert_eq!(
            google.execution,
            crate::services::tts::TtsExecutionKind::Cloud
        );
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
