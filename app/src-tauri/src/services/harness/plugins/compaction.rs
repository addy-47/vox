use anyhow::Result;
use tokio_util::sync::CancellationToken;
use turso::Connection;

use crate::{
    core::settings::LlmSettings,
    persistence::compactions::{
        commit_compaction_output, record_compaction_finish, record_compaction_start,
    },
    services::{
        harness::{ChatMessage, PromptTag, Role},
        llm::LlmProvider,
        memory::compaction::runner::{run_compaction, CompactionResult},
    },
};

pub const MIN_MESSAGES_FOR_COMPACTION: usize = 4;

/// Bundled parameters for executing and persisting compaction passes.
pub struct CompactionParams<'a> {
    pub session_id: i64,
    pub trigger_kind: &'a str,
    pub from_turn_id: u32,
    pub to_turn_id: u32,
    pub history_messages: &'a [ChatMessage],
    pub llm_settings: Option<&'a LlmSettings>,
    pub cancel: Option<&'a CancellationToken>,
}

/// Plugin managing conversational memory compaction, session context tracking, and quiet watcher timing.
#[derive(Debug, Clone)]
pub struct CompactionPlugin {
    session_context: Option<String>,
    auto_compaction_enabled: bool,
    context_window: usize,
    is_embedded: bool,
    last_compacted_to_turn: u32,
}

impl CompactionPlugin {
    pub fn new(context_window: usize, is_embedded: bool, auto_compaction_enabled: bool) -> Self {
        Self {
            session_context: None,
            auto_compaction_enabled,
            context_window,
            is_embedded,
            last_compacted_to_turn: 0,
        }
    }

    pub fn context_window(&self) -> usize {
        self.context_window
    }

    pub fn is_embedded(&self) -> bool {
        self.is_embedded
    }

    pub fn session_context(&self) -> Option<&str> {
        self.session_context.as_deref()
    }

    pub fn set_session_context(&mut self, context: Option<String>) {
        self.session_context = context;
    }

    pub fn auto_compaction_enabled(&self) -> bool {
        self.auto_compaction_enabled
    }

    pub fn from_turn_id(&self) -> u32 {
        self.last_compacted_to_turn + 1
    }

    pub fn set_last_compacted_to_turn(&mut self, turn_id: u32) {
        self.last_compacted_to_turn = turn_id;
    }

    pub fn can_perform_inline_compaction(&self, message_count: usize) -> bool {
        message_count >= MIN_MESSAGES_FOR_COMPACTION
    }

    pub fn apply_session_context(&mut self, context: &str) {
        if !context.trim().is_empty() {
            self.session_context = Some(context.to_string());
        }
    }

    /// Executes compaction across the LLM provider, records the run in the Turso DB ledger,
    /// commits extracted facts to the ingestion queue, and returns the result.
    pub async fn run_and_persist(
        provider: &dyn LlmProvider,
        conn: &Connection,
        params: CompactionParams<'_>,
    ) -> Result<CompactionResult> {
        log::info!(
            "[Harness::Compaction] Running {} compaction for session {} ({} messages)",
            params.trigger_kind,
            params.session_id,
            params.history_messages.len()
        );

        let run_id = if params.session_id > 0 {
            record_compaction_start(
                conn,
                params.session_id,
                params.trigger_kind,
                params.from_turn_id,
                params.to_turn_id,
            )
            .await
            .ok()
        } else {
            None
        };

        let result = match run_compaction(
            provider,
            params.history_messages,
            params.llm_settings,
            params.cancel,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                if let Some(id) = run_id {
                    let err_str = e.to_string();
                    let _ = record_compaction_finish(conn, id, "", "failed", Some(&err_str)).await;
                }
                return Err(e);
            }
        };

        if let Some(id) = run_id {
            if let Err(e) = commit_compaction_output(
                conn,
                id,
                &result.raw_json,
                &result.facts,
                params.session_id,
            )
            .await
            {
                log::warn!(
                    "[Harness::Compaction] Failed to commit compaction output to DB: {}",
                    e
                );
            } else {
                log::info!(
                    "[Harness::Compaction] Successfully committed compaction run {} with {} facts",
                    id,
                    result.facts.len()
                );
            }
        }

        Ok(result)
    }

    pub fn prune_history_with_summary(
        &self,
        system_prompt: &str,
        user_turn: &str,
    ) -> Vec<ChatMessage> {
        let full_system = if let Some(ref context) = self.session_context {
            if !context.trim().is_empty() {
                let wrapped = PromptTag::SessionContext.wrap(context.trim());
                format!("{}\n\n{}", system_prompt.trim(), wrapped)
            } else {
                system_prompt.to_string()
            }
        } else {
            system_prompt.to_string()
        };

        let mut pruned = Vec::with_capacity(2);
        pruned.push(ChatMessage::new(Role::System, full_system));

        if !user_turn.trim().is_empty() {
            pruned.push(ChatMessage::new(Role::User, user_turn.to_string()));
        }

        pruned
    }
}
