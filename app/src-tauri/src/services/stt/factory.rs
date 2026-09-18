use std::path::Path;

use crate::{
    core::settings::{SttProviderConfig, SttSettings},
    services::stt::{
        providers::{EmbeddedSttProvider, SttProvider},
        NEMOTRON_MODEL_DIR, QWEN_ASR_MODEL_DIR,
    },
};

/// Instantiates an `SttProvider` instance from the specified configuration and model path.
pub fn create_stt_provider(
    provider_config: &SttProviderConfig,
    model_path: &Path,
    num_threads: u32,
) -> anyhow::Result<Box<dyn SttProvider>> {
    match provider_config {
        SttProviderConfig::Embedded { model_type } => Ok(Box::new(EmbeddedSttProvider::new(
            model_path,
            model_type,
            num_threads,
        )?)),
        SttProviderConfig::Cloud { provider, .. } => {
            anyhow::bail!("Unknown cloud STT provider: \"{}\"", provider)
        }
    }
}

/// Resolves model directories and instantiates the active STT provider from settings.
pub fn create_stt_instance_from_settings(
    settings: &SttSettings,
    models_dir: &Path,
) -> Result<Box<dyn SttProvider>, String> {
    let provider_config = settings.to_provider_config();
    let num_threads = settings.embedded.threads;

    match provider_config {
        SttProviderConfig::Embedded { ref model_type } => {
            let path = match model_type.as_str() {
                "nvidia_nemotron" => models_dir.join(NEMOTRON_MODEL_DIR),
                _ => models_dir.join(QWEN_ASR_MODEL_DIR),
            };
            create_stt_provider(&provider_config, &path, num_threads)
                .map_err(|e| format!("STT provider creation failed: {}", e))
        }
        SttProviderConfig::Cloud { .. } => {
            let path = models_dir.join("stt");
            create_stt_provider(&provider_config, &path, num_threads)
                .map_err(|e| format!("STT provider creation failed: {}", e))
        }
    }
}
