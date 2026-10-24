use std::path::Path;

use crate::{
    config::VoxSettings,
    core::{
        compute::{self, Execution, Workload},
        defaults::DEFAULT_LLM_CONTEXT_WINDOW,
    },
    services::llm::{
        provider::LlmProvider,
        sidecar::SidecarLlmProvider as EmbeddedProvider,
        transport::{ConnectionConfig, RemoteTransport},
        LlmProvidersConfig, LlmRuntimeTarget, LlmSettings,
    },
};

/// Maps a resolved LLM runtime target to the site that runs it.
fn execution_for(target: &LlmRuntimeTarget) -> Execution {
    match target {
        LlmRuntimeTarget::Embedded => Execution::Embedded,
        LlmRuntimeTarget::Server { .. }
        | LlmRuntimeTarget::Cloud { .. }
        | LlmRuntimeTarget::Unresolved { .. } => Execution::Remote,
    }
}

/// Creates a boxed LLM provider directly from `LlmSettings` and parameters.
pub fn create_llm_provider_from_llm_settings(
    llm_settings: &LlmSettings,
    providers: Option<&LlmProvidersConfig>,
    llm_path: &Path,
    ctx_size: u32,
) -> Result<Box<dyn LlmProvider>, String> {
    let provider_config = llm_settings.to_provider_config(providers);
    let effective_ctx = LlmSettings::effective_ctx_size(ctx_size);
    let n_threads = compute::resolve_threads(Workload::Llm, execution_for(&provider_config));

    match provider_config {
        LlmRuntimeTarget::Unresolved { provider_id } => Err(format!(
            "cannot create LLM provider: cloud provider '{}' is not in the declarative manifest (RULE-PROV-01)",
            provider_id
        )),
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
    )
}
