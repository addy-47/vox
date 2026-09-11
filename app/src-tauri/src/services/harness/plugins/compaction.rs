use anyhow::Result;
use tokio_util::sync::CancellationToken;

use crate::{
    core::settings::LlmSettings,
    services::{
        harness::{ChatMessage, PromptTag, Role},
        llm::LlmProvider,
        memory::compaction::runner::{run_compaction, CompactionResult},
    },
};

pub const MIN_MESSAGES_FOR_COMPACTION: usize = 4;
pub const EMBEDDED_MODEL_MIN_CONTEXT_WINDOW: usize = 4096;

/// Plugin managing conversational memory compaction, rolling summary tracking, and quiet watcher timing.
#[derive(Debug, Clone)]
pub struct CompactionPlugin {
    rolling_summary: Option<String>,
    auto_compaction_enabled: bool,
    context_window: usize,
    is_embedded: bool,
}

impl CompactionPlugin {
    pub fn new(context_window: usize, is_embedded: bool, auto_compaction_enabled: bool) -> Self {
        Self {
            rolling_summary: None,
            auto_compaction_enabled,
            context_window,
            is_embedded,
        }
    }

    pub fn rolling_summary(&self) -> Option<&str> {
        self.rolling_summary.as_deref()
    }

    pub fn set_rolling_summary(&mut self, summary: Option<String>) {
        self.rolling_summary = summary;
    }

    pub fn auto_compaction_enabled(&self) -> bool {
        self.auto_compaction_enabled
    }

    pub fn set_auto_compaction_enabled(&mut self, enabled: bool) {
        self.auto_compaction_enabled = enabled;
    }

    pub fn can_perform_inline_compaction(&self, message_count: usize) -> bool {
        if self.is_embedded && self.context_window <= EMBEDDED_MODEL_MIN_CONTEXT_WINDOW {
            return false;
        }
        message_count >= MIN_MESSAGES_FOR_COMPACTION
    }

    pub async fn execute_compaction(
        &mut self,
        provider: &dyn LlmProvider,
        history_messages: &[ChatMessage],
        llm_settings: Option<&LlmSettings>,
        cancel: Option<&CancellationToken>,
    ) -> Result<CompactionResult> {
        log::info!(
            "[Harness::Compaction] Dispatching compaction across provider for {} messages",
            history_messages.len()
        );

        let result = run_compaction(provider, history_messages, llm_settings, cancel).await?;
        if !result.context_summary.trim().is_empty() {
            self.rolling_summary = Some(result.context_summary.clone());
        }

        Ok(result)
    }

    pub fn prune_history_with_summary(
        &self,
        system_prompt: &str,
        user_turn: &str,
    ) -> Vec<ChatMessage> {
        let mut pruned = Vec::with_capacity(3);
        pruned.push(ChatMessage::new(Role::System, system_prompt.to_string()));

        if let Some(ref summary) = self.rolling_summary {
            if !summary.trim().is_empty() {
                let wrapped = PromptTag::SessionContext.wrap(summary.trim());
                pruned.push(ChatMessage::new(Role::System, wrapped));
            }
        }

        pruned.push(ChatMessage::new(Role::User, user_turn.to_string()));
        pruned
    }
}
