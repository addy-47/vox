use std::sync::{mpsc, Arc};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use super::{execute_turn, PipelineDomain, TurnExecutionRequest, TurnOutcome};
use crate::{
    core::{
        settings::{LlmProviderConfig, VoxSettings},
        state::AppState,
    },
    persistence::TurnRow,
    services::{
        harness::{
            stages::{
                budget::{ContextBudgetStage, ContextStatus},
                compaction::{CompactionStage, QuietCompactionWatcher},
                history::ConversationHistoryStage,
                prompt::PromptBuilderStage,
                streaming::StreamRoutingStage,
                tools::ToolRegistry,
            },
            ChatMessage,
        },
        llm::{
            actor::LlmCommand, ConversationInput, GenerationOptions, GenerationPurpose,
            GenerationRequest, OutputConstraint, ReasoningMode,
        },
    },
};

/// The session-scoped conversational orchestrator and stage chassis.
pub struct Harness {
    pub(crate) session_id: Option<i64>,
    pub(crate) domain: PipelineDomain,
    pub(crate) cancel_token: CancellationToken,
    pub(crate) llm_tx: Option<mpsc::Sender<LlmCommand>>,
    pub(crate) history: ConversationHistoryStage,
    pub(crate) prompt: PromptBuilderStage,
    pub(crate) budget: Option<ContextBudgetStage>,
    pub(crate) compaction: Option<CompactionStage>,
    pub(crate) stream: StreamRoutingStage,
    pub(crate) watcher: Option<QuietCompactionWatcher>,
    pub(crate) generation_options: GenerationOptions,
    pub(crate) tool_registry: ToolRegistry,
    pub(crate) supports_tools: bool,
    pub(crate) title_set: bool,
    pub(crate) memory_retrieval_enabled: bool,
    pub(crate) has_played_filler: bool,
}

impl Harness {
    /// Initializes a modular assistant session harness with full stage pipeline.
    pub fn new_modular(
        session_id: Option<i64>,
        base_prompt: String,
        personal_memory: Option<String>,
        settings: &VoxSettings,
        llm_tx: mpsc::Sender<LlmCommand>,
        supports_tools: bool,
    ) -> Self {
        let is_cloud = matches!(
            settings.llm.to_provider_config(),
            LlmProviderConfig::OpenAiCompat { .. }
        );
        let is_embedded = !is_cloud;
        let ctx_window = settings.llm.context_window as usize;
        let max_share = settings.working_memory.max_context_share;

        let mut prompt_stage = PromptBuilderStage::new(base_prompt, ctx_window, max_share);
        prompt_stage.set_personal_memory(personal_memory);
        let assembled_prompt = prompt_stage.assemble();

        let history_stage = ConversationHistoryStage::with_system_prompt(assembled_prompt);
        let budget_stage =
            ContextBudgetStage::new(ctx_window, settings.llm.max_output_tokens as usize);
        let compaction_stage = CompactionStage::new(
            ctx_window,
            is_embedded,
            settings.working_memory.auto_compaction,
        );
        let generation_options = GenerationOptions {
            temperature: Some(settings.llm.temperature),
            max_output_tokens: Some(settings.llm.max_output_tokens),
            reasoning: ReasoningMode::from_enabled(settings.llm.reasoning_enabled),
            ..Default::default()
        };

        Self {
            session_id,
            domain: PipelineDomain::ModularAssistant,
            cancel_token: CancellationToken::new(),
            llm_tx: Some(llm_tx),
            history: history_stage,
            prompt: prompt_stage,
            budget: Some(budget_stage),
            compaction: Some(compaction_stage),
            stream: StreamRoutingStage::new(),
            watcher: Some(QuietCompactionWatcher::new()),
            generation_options,
            tool_registry: ToolRegistry::with_default_tools(),
            supports_tools,
            title_set: false,
            memory_retrieval_enabled: settings.personal_memory.context_retrieval_enabled,
            has_played_filler: false,
        }
    }

    /// Initializes a realtime speech-to-speech session harness without modular stages.
    pub fn new_realtime(
        session_id: Option<i64>,
        base_prompt: String,
        personal_memory: Option<String>,
        settings: &VoxSettings,
    ) -> Self {
        let ctx_window = settings.llm.context_window as usize;
        let max_share = settings.working_memory.max_context_share;

        let mut prompt_stage = PromptBuilderStage::new(base_prompt, ctx_window, max_share);
        prompt_stage.set_personal_memory(personal_memory);
        let assembled_prompt = prompt_stage.assemble();

        let history_stage = ConversationHistoryStage::with_system_prompt(assembled_prompt);

        Self {
            session_id,
            domain: PipelineDomain::RealtimeS2S,
            cancel_token: CancellationToken::new(),
            llm_tx: None,
            history: history_stage,
            prompt: prompt_stage,
            budget: None,
            compaction: None,
            stream: StreamRoutingStage::new(),
            watcher: None,
            generation_options: GenerationOptions::default(),
            tool_registry: ToolRegistry::with_default_tools(),
            supports_tools: true,
            title_set: false,
            memory_retrieval_enabled: settings.personal_memory.context_retrieval_enabled,
            has_played_filler: false,
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
            tools: None,
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
        harness_lock: Arc<Mutex<Option<Harness>>>,
    ) {
        let tracked_turns = self.check_quiet_compaction_eligibility();
        if let Some(ref mut watcher) = self.watcher {
            watcher.on_turn_completed(state, harness_lock, tracked_turns);
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

    pub fn push_user_turn(&mut self, text: String) {
        self.history.push_user_turn(text);
    }

    pub fn push_assistant_turn(&mut self, text: String) {
        self.history.push_assistant_turn(text);
    }

    pub fn messages(&self) -> &[ChatMessage] {
        self.history.messages()
    }

    pub fn history(&self) -> &ConversationHistoryStage {
        &self.history
    }

    pub fn history_mut(&mut self) -> &mut ConversationHistoryStage {
        &mut self.history
    }

    pub fn supports_tools(&self) -> bool {
        self.supports_tools
    }

    pub fn title_set(&self) -> bool {
        self.title_set
    }

    pub fn check_critical_compaction_eligibility(&self) -> Option<Vec<ChatMessage>> {
        let budget = self.budget.as_ref()?;
        let compaction = self.compaction.as_ref()?;
        let tracked = budget.calculate_tracked_tokens(self.history.messages());
        let (_, status) = budget.evaluate_utilization(tracked);
        if status == ContextStatus::Critical
            && compaction.can_perform_inline_compaction(self.history.messages().len())
        {
            Some(self.history.messages().to_vec())
        } else {
            None
        }
    }

    pub fn seed_continuation(
        &mut self,
        session_context: Option<String>,
        turns: Vec<TurnRow>,
        title_set: bool,
    ) {
        self.title_set = title_set;
        if let Some(ref mut compaction) = self.compaction {
            compaction.set_session_context(session_context.clone());
        }
        if session_context.is_some() {
            self.prompt.set_session_context(session_context);
            self.history.reset(self.prompt.assemble());
        }
        for turn in turns {
            self.history.push_user_turn(turn.user_text);
            self.history.push_assistant_turn(turn.assistant_text);
        }
    }

    pub fn apply_session_context(&mut self, session_context: &str, active_query: &str) {
        self.prompt.set_session_context(Some(session_context.to_string()));
        if let Some(ref mut compaction) = self.compaction {
            compaction.apply_session_context(session_context);
        }
        let assembled_prompt = self.prompt.assemble();
        self.history.clear();
        self.history.reset(assembled_prompt);
        if !active_query.trim().is_empty() {
            self.history.push_user_turn(active_query.to_string());
        }
        log::info!(
            "[Harness::Chassis] History rebuilt with session context. Total turns: {}",
            self.history.messages().len()
        );
    }

    pub async fn execute_turn<R: tauri::Runtime + 'static>(
        harness_arc: &Arc<Mutex<Option<Harness>>>,
        req: TurnExecutionRequest<R>,
    ) -> TurnOutcome {
        execute_turn(harness_arc, req).await
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.abort_watcher();
        self.cancel_token.cancel();
    }
}
