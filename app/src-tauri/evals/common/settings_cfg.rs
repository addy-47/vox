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

/// Builds eval settings: local defaults with the LLM pointed at the remote
/// server (gemma3:12b executor) and an eval-chosen context window.
///
/// `context_window` is a deliberate eval parameter (default 4096): the trip
/// mechanism is percentage-based, so a smaller window lets one 100-turn
/// session genuinely cross the 85% critical line through production math.
pub fn server_llm_settings(base_url: &str, model: &str, context_window: u32) -> VoxSettings {
    let mut settings = VoxSettings::default();
    settings.llm.active = LlmActiveProvider::Server;
    settings.llm.server.base_url = base_url.to_string();
    settings.llm.server.model = model.to_string();
    settings.llm.server.api_key = None;
    settings.llm.server.provider_name = Some("ollama".to_string());
    settings.llm.context_window = context_window;
    settings
}

/// Exposes the raw `LlmSettings` for provider construction.
pub fn llm_settings_of(settings: &VoxSettings) -> LlmSettings {
    settings.llm.clone()
}
