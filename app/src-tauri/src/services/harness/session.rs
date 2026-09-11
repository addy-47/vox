use std::sync::{mpsc, Arc};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use super::{
    plugins::{
        budget::{ContextBudgetPlugin, ContextStatus},
        compaction::CompactionPlugin,
        history::ConversationHistoryPlugin,
        prompt::PromptBuilderPlugin,
        stream::{StreamRoutingHandles, StreamRoutingPlugin},
    },
    watcher::QuietCompactionWatcher,
    ChatMessage, Role,
};
use crate::{
    core::{
        settings::{LlmProviderConfig, VoxSettings},
        state::AppState,
    },
    persistence::TurnRow,
    services::{
        llm::{
            actor::LlmCommand, ConversationInput, GenerationOptions, GenerationPurpose,
            GenerationRequest, OutputConstraint,
        },
        memory::estimate_tokens,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineDomain {
    ModularAssistant,
    RealtimeS2S,
    Dictation,
}

#[derive(Debug)]
pub enum TurnPreparation {
    Ready(GenerationRequest),
    NeedsInlineCompaction {
        filler_phrase: &'static str,
        uncompacted_slice: Vec<ChatMessage>,
    },
    DuplicateTurnIgnored,
}

/// The session-scoped conversational orchestrator and plugin chassis.
pub struct HarnessSession {
    session_id: Option<i64>,
    domain: PipelineDomain,
    cancel_token: CancellationToken,
    llm_tx: Option<mpsc::Sender<LlmCommand>>,
    history: ConversationHistoryPlugin,
    prompt: PromptBuilderPlugin,
    budget: Option<ContextBudgetPlugin>,
    compaction: Option<CompactionPlugin>,
    stream: StreamRoutingPlugin,
    watcher: Option<QuietCompactionWatcher>,
    generation_options: GenerationOptions,
}

impl HarnessSession {
    pub fn new_modular(
        session_id: Option<i64>,
        base_prompt: String,
        personal_memory: Option<String>,
        settings: &VoxSettings,
        llm_tx: mpsc::Sender<LlmCommand>,
    ) -> Self {
        let is_cloud = matches!(
            settings.llm.to_provider_config(),
            LlmProviderConfig::OpenAiCompat { .. }
        );
        let is_embedded = !is_cloud;
        let ctx_window = settings.llm.context_window as usize;
        let max_share = settings.memory.max_context_share;

        let mut prompt_plugin = PromptBuilderPlugin::new(base_prompt, ctx_window, max_share);
        prompt_plugin.set_personal_memory(personal_memory);
        let assembled_prompt = prompt_plugin.assemble();

        let history_plugin = ConversationHistoryPlugin::with_system_prompt(assembled_prompt);
        let budget_plugin = ContextBudgetPlugin::new(ctx_window, is_cloud);
        let compaction_plugin =
            CompactionPlugin::new(ctx_window, is_embedded, settings.history.auto_compaction);
        let generation_options = GenerationOptions {
            temperature: Some(settings.llm.temperature),
            max_output_tokens: Some(settings.llm.max_output_tokens),
            ..Default::default()
        };

        Self {
            session_id,
            domain: PipelineDomain::ModularAssistant,
            cancel_token: CancellationToken::new(),
            llm_tx: Some(llm_tx),
            history: history_plugin,
            prompt: prompt_plugin,
            budget: Some(budget_plugin),
            compaction: Some(compaction_plugin),
            stream: StreamRoutingPlugin::new(),
            watcher: Some(QuietCompactionWatcher::new()),
            generation_options,
        }
    }

    pub fn new_realtime(session_id: Option<i64>, base_prompt: String) -> Self {
        let history_plugin = ConversationHistoryPlugin::with_system_prompt(base_prompt.clone());
        let prompt_plugin = PromptBuilderPlugin::new(base_prompt, 4096, 0.20);

        Self {
            session_id,
            domain: PipelineDomain::RealtimeS2S,
            cancel_token: CancellationToken::new(),
            llm_tx: None,
            history: history_plugin,
            prompt: prompt_plugin,
            budget: None,
            compaction: None,
            stream: StreamRoutingPlugin::new(),
            watcher: None,
            generation_options: GenerationOptions::default(),
        }
    }

    pub fn session_id(&self) -> Option<i64> {
        self.session_id
    }

    pub fn domain(&self) -> PipelineDomain {
        self.domain
    }

    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel_token
    }

    pub fn assembled_system_prompt(&self) -> String {
        self.prompt.assemble()
    }

    pub fn generation_options(&self) -> &GenerationOptions {
        &self.generation_options
    }

    pub fn from_turn_id(&self) -> u32 {
        self.compaction
            .as_ref()
            .map(|c| c.from_turn_id())
            .unwrap_or(0)
    }

    pub fn set_last_compacted_to_turn(&mut self, turn_id: u32) {
        if let Some(ref mut compaction) = self.compaction {
            compaction.set_last_compacted_to_turn(turn_id);
        }
    }

    pub fn update_personal_memory(&mut self, personal_memory: Option<String>) {
        self.prompt.set_personal_memory(personal_memory);
    }

    pub fn commit_turn(&mut self, assistant_text: String) {
        self.history.push_assistant_turn(assistant_text);
    }

    pub fn rollback_user_turn(&mut self) {
        self.history.rollback_last_user_turn();
    }

    pub fn fallback_fifo_shift(&mut self) {
        if let Some(ref budget) = self.budget {
            budget.execute_fifo_shift(&mut self.history);
        }
    }

    pub fn create_generation_request(&self) -> GenerationRequest {
        let input = ConversationInput {
            messages: self.history.messages().to_vec(),
        };
        GenerationRequest {
            input,
            options: self.generation_options.clone(),
            output: OutputConstraint::Text,
            purpose: GenerationPurpose::Conversation,
        }
    }

    pub fn check_quiet_compaction_eligibility(&self) -> Option<Vec<ChatMessage>> {
        let budget = self.budget.as_ref()?;
        let compaction = self.compaction.as_ref()?;
        if !compaction.auto_compaction_enabled() {
            return None;
        }
        let tracked = budget.calculate_tracked_tokens(self.history.messages());
        let (_, status) = budget.evaluate_utilization(tracked);
        if status == ContextStatus::SoftWarning && self.history.messages().len() >= 4 {
            Some(self.history.messages().to_vec())
        } else {
            None
        }
    }

    pub fn apply_quiet_compaction_summary(&mut self, session_context: &str) {
        self.apply_session_context(session_context, "");
    }

    pub fn apply_quiet_session_context(&mut self, session_context: &str) {
        self.apply_session_context(session_context, "");
    }

    pub fn on_turn_completed(
        &mut self,
        state: Arc<AppState>,
        harness_lock: Arc<Mutex<Option<HarnessSession>>>,
    ) {
        if let Some(ref mut watcher) = self.watcher {
            watcher.on_turn_completed(state, harness_lock);
        }
    }

    pub fn abort_watcher(&mut self) {
        if let Some(ref mut watcher) = self.watcher {
            watcher.abort();
        }
    }

    pub fn llm_tx(&self) -> Option<&mpsc::Sender<LlmCommand>> {
        self.llm_tx.as_ref()
    }

    pub fn prepare_turn(&mut self, query: &str, turn_id: u32) -> TurnPreparation {
        if self.history.is_duplicate_user_turn(query) {
            log::info!(
                "[Harness::Session] Dropping duplicate user turn: '{}'",
                query
            );
            return TurnPreparation::DuplicateTurnIgnored;
        }

        let assembled_system = self.prompt.assemble();
        if let Some(first) = self.history.messages().first() {
            if first.role == Role::System && first.content != assembled_system {
                self.history.reset(assembled_system);
            }
        }

        if let Some(ref budget) = self.budget {
            let mut tracked_tokens = budget.calculate_tracked_tokens(self.history.messages());
            let query_tokens = estimate_tokens(query);
            tracked_tokens += query_tokens;

            let (utilization, status) = budget.evaluate_utilization(tracked_tokens);
            log::info!(
                "[Harness::Session] Turn {} token utilization: {:.1}% ({:?})",
                turn_id,
                utilization * 100.0,
                status
            );

            if status == ContextStatus::Critical {
                let can_compact = self
                    .compaction
                    .as_ref()
                    .map(|c| c.can_perform_inline_compaction(self.history.messages().len()))
                    .unwrap_or(false);

                if can_compact {
                    let filler = super::select_filler_phrase(query, turn_id);
                    return TurnPreparation::NeedsInlineCompaction {
                        filler_phrase: filler,
                        uncompacted_slice: self.history.messages().to_vec(),
                    };
                }

                log::warn!(
                    "[Harness::Session] Inline compaction ineligible. Executing FIFO shift."
                );
                budget.execute_fifo_shift(&mut self.history);
            }
        }

        self.history.push_user_turn(query.to_string());

        let input = ConversationInput {
            messages: self.history.messages().to_vec(),
        };

        TurnPreparation::Ready(GenerationRequest {
            input,
            options: self.generation_options.clone(),
            output: OutputConstraint::Text,
            purpose: GenerationPurpose::Conversation,
        })
    }

    pub fn seed_continuation(&mut self, session_context: Option<String>, turns: Vec<TurnRow>) {
        if let Some(ref mut compaction) = self.compaction {
            compaction.set_session_context(session_context);
        }
        for turn in turns {
            self.history.push_user_turn(turn.user_text);
            self.history.push_assistant_turn(turn.assistant_text);
        }
    }

    pub fn apply_session_context(&mut self, session_context: &str, active_query: &str) {
        if let Some(ref mut compaction) = self.compaction {
            compaction.apply_session_context(session_context);
            let pruned_messages =
                compaction.prune_history_with_summary(&self.prompt.assemble(), active_query);
            self.history.clear();
            for msg in pruned_messages {
                if msg.role == Role::System {
                    self.history.reset(msg.content);
                } else if msg.role == Role::User {
                    self.history.push_user_turn(msg.content);
                } else {
                    self.history.push_assistant_turn(msg.content);
                }
            }
            log::info!(
                "[Harness::Session] History rebuilt with session context. Total turns: {}",
                self.history.messages().len()
            );
        }
    }

    pub fn apply_compaction_summary(&mut self, summary: &str, active_query: &str) {
        self.apply_session_context(summary, active_query);
    }

    /// Returns a cloned `StreamRoutingPlugin` for use in spawn_blocking contexts.
    pub(crate) fn clone_stream_plugin(&self) -> crate::services::harness::plugins::stream::StreamRoutingPlugin {
        self.stream.clone()
    }

    /// Routes a streaming LLM response through the stream plugin.
    pub fn route_stream<R: tauri::Runtime>(
        &self,
        handles: StreamRoutingHandles<R>,
        response_rx: std::sync::mpsc::Receiver<crate::services::llm::actor::LlmResponse>,
    ) -> Result<String, String> {
        self.stream.route_stream(handles, response_rx)
    }

    /// Returns a reference to the conversation history plugin.
    pub fn history(&self) -> &ConversationHistoryPlugin {
        &self.history
    }

    /// Returns a mutable reference to the conversation history plugin.
    /// Exposed for integration test seeding only — do not use in production pipeline code.
    pub fn history_mut(&mut self) -> &mut ConversationHistoryPlugin {
        &mut self.history
    }
}
