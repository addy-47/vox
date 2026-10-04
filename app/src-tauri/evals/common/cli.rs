//! ============================================================================
//! evals/common/cli.rs — Shared clap argument groups
//! ============================================================================
//! Category     : Utility Module
//! Component    : evals harness
//! Prerequisites: see evals/README.md
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : see summary.md in the run directory
//! ============================================================================

use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct LlmServerArgs {
    #[arg(long, default_value = "http://127.0.0.1:11434/v1")]
    pub server_url: String,
    #[arg(long, default_value = "qwen3.5:9b")]
    pub server_model: String,
    #[arg(long, default_value = "")]
    pub server_api_key: String,
    #[arg(long, default_value = "ollama")]
    pub server_provider: String,
}

impl LlmServerArgs {
    pub fn key(&self) -> Option<&str> {
        let t = self.server_api_key.trim();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    }
    pub fn provider(&self) -> Option<&str> {
        let t = self.server_provider.trim();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct CorpusArgs {
    #[arg(long)]
    pub limit: Option<usize>,
    #[arg(long)]
    pub category: Option<String>,
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long, default_value_t = false)]
    pub resume: bool,
}

/// A single sample per query cannot distinguish a p50 from a p99.
#[derive(Args, Debug, Clone)]
pub struct RepetitionArgs {
    #[arg(long, default_value_t = 1)]
    pub repeats: u32,
}

impl RepetitionArgs {
    pub fn count(&self) -> u32 {
        self.repeats.max(1)
    }
}
