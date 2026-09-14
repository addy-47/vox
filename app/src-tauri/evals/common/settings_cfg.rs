//! ============================================================================
//! settings_cfg.rs — Eval VoxSettings wired to the remote LLM server
//! ============================================================================
//! Category     : Evaluation (shared harness, not a runnable eval)
//! Component    : evals/common (vox_lib settings API)
//! Prerequisites: None
//! Execution    : Included via #[path] from evals/memory_*_eval.rs
//! Metrics      : N/A
//! ============================================================================

use vox_lib::core::settings::{LlmActiveProvider, LlmSettings, VoxSettings};

/// Builds eval settings: local defaults with the LLM pointed at a remote
/// OpenAI-compatible server and an eval-chosen context window.
///
/// `context_window` is a deliberate eval parameter (default 8192): the trip
/// mechanism is percentage-based, so the window sizes the slice that trips it.
/// `provider_name` selects the transport preset (`Some("ollama")` for Ollama
/// native, `None` for generic OpenAI chat-completions, e.g. Nvidia).
pub fn server_llm_settings(
    base_url: &str,
    model: &str,
    context_window: u32,
    api_key: Option<String>,
    provider_name: Option<String>,
) -> VoxSettings {
    let mut settings = VoxSettings::default();
    settings.llm.active = LlmActiveProvider::Server;
    settings.llm.server.base_url = base_url.to_string();
    settings.llm.server.model = model.to_string();
    settings.llm.server.api_key = api_key;
    settings.llm.server.provider_name = provider_name;
    settings.llm.context_window = context_window;
    settings
}

/// Exposes the raw `LlmSettings` for provider construction.
pub fn llm_settings_of(settings: &VoxSettings) -> LlmSettings {
    settings.llm.clone()
}
