use std::path::Path;

use crate::{
    config::VoxSettings,
    core::defaults::{DEFAULT_LLM_CONTEXT_WINDOW, DEFAULT_LLM_THREADS},
    services::llm::{
        embedded::EmbeddedProvider,
        provider::LlmProvider,
        transport::{ConnectionConfig, RemoteTransport},
        LlmProvidersConfig, LlmRuntimeTarget, LlmSettings,
    },
};

/// Creates a boxed LLM provider directly from `LlmSettings` and parameters.
pub fn create_llm_provider_from_llm_settings(
    llm_settings: &LlmSettings,
    providers: Option<&LlmProvidersConfig>,
    llm_path: &Path,
    ctx_size: u32,
    n_threads: u32,
) -> Result<Box<dyn LlmProvider>, String> {
    let provider_config = llm_settings.to_provider_config(providers);
    let effective_ctx = LlmSettings::effective_ctx_size(ctx_size);

    match provider_config {
        LlmRuntimeTarget::Embedded => EmbeddedProvider::new(llm_path, effective_ctx, n_threads)
            .map(|p| Box::new(p) as Box<dyn LlmProvider>)
            .map_err(|e| e.to_string()),
        LlmRuntimeTarget::Server {
            base_url,
            model,
            api_key,
            provider_name,
            ..
        }
        | LlmRuntimeTarget::Cloud {
            base_url,
            model,
            api_key,
            provider_name,
            ..
        } => {
            let conn_cfg = ConnectionConfig::new(
                &base_url,
                &model,
                api_key.as_deref(),
                provider_name.as_deref(),
            );
            let provider = RemoteTransport::new(conn_cfg);
            Ok(Box::new(provider) as Box<dyn LlmProvider>)
        }
    }
}

/// Creates a boxed LLM provider based on settings configuration.
pub fn create_llm_provider(
    settings: &VoxSettings,
    llm_path: &Path,
) -> Result<Box<dyn LlmProvider>, String> {
    create_llm_provider_from_llm_settings(
        &settings.llm,
        Some(&settings.providers.llm),
        llm_path,
        DEFAULT_LLM_CONTEXT_WINDOW,
        DEFAULT_LLM_THREADS,
    )
}
