use super::estimate_tokens;
use super::NARRATIVE_CHAIN_SOFT_CAP_SHARE;
use crate::core::constants::{TRANSITION_MESSAGES_EN, TRANSITION_MESSAGES_HI};
use crate::services::llm::{LlmProvider, ProviderKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Conversational message participant roles.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
}

impl std::fmt::Display for Role {
    /// Formats the role enum into its canonical lowercase string identifier.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
        }
    }
}

/// A structured chat message within working memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub timestamp_ms: u64,
}

/// Context snapshot ready for LLM generation.
pub struct ConversationContext {
    pub messages: Vec<ChatMessage>,
    pub token_count: usize,
    pub kv_cache_index: usize,
}

/// Computes the current Unix timestamp in milliseconds.
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Orchestrates conversation turns, dynamic FIFO sliding window, and context compaction.
pub struct ConversationManager {
    messages: Vec<ChatMessage>,
    system_prompt: ChatMessage,

    total_token_count: usize,
    max_context_tokens: usize,
    reserved_generation_tokens: usize,
    critical_threshold: f32,
    soft_threshold: f32,

    kv_synced_index: usize,

    session_compaction_contexts: Vec<String>,
    latest_compaction_facts: HashMap<String, Vec<String>>,

    opportunistic_active: bool,
    opportunistic_cancel: Arc<AtomicBool>,
}

impl ConversationManager {
    /// Creates a new ConversationManager instance with the specified context capacity.
    pub fn new(max_context_tokens: usize) -> Self {
        let default_sys_prompt = ChatMessage {
            role: Role::System,
            content: crate::core::constants::SYSTEM_PROMPT_MODULAR.to_string(),
            timestamp_ms: current_timestamp_ms(),
        };

        let sys_tokens = estimate_tokens(&default_sys_prompt.content);

        Self {
            messages: vec![default_sys_prompt.clone()],
            system_prompt: default_sys_prompt,
            total_token_count: sys_tokens,
            max_context_tokens,
            reserved_generation_tokens: super::RESERVED_GENERATION_TOKENS,
            critical_threshold: super::CONTEXT_CRITICAL_THRESHOLD,
            soft_threshold: super::CONTEXT_SOFT_THRESHOLD,
            kv_synced_index: 0,
            session_compaction_contexts: Vec::new(),
            latest_compaction_facts: HashMap::new(),
            opportunistic_active: false,
            opportunistic_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns current total token count across active messages.
    pub fn total_token_count(&self) -> usize {
        self.total_token_count
    }

    /// Updates the maximum allowable context token budget.
    pub fn set_max_context_tokens(&mut self, max_tokens: usize) {
        if max_tokens > 0 {
            self.max_context_tokens = max_tokens;
            log::info!(
                "[WorkingMemory] Updated max_context_tokens to {}",
                max_tokens
            );
        }
    }

    /// Preloads active Identity facts into the base system prompt block.
    pub async fn load_identity_into_system_prompt(
        &mut self,
        conn: &turso::Connection,
    ) -> anyhow::Result<()> {
        let active_identities =
            crate::persistence::queries::fetch_all_active_identity(conn).await?;
        if !active_identities.is_empty() {
            let mut base_prompt = self.system_prompt.content.clone();
            if let Some(start_idx) = base_prompt.find("\n\n<user_profile>") {
                base_prompt.truncate(start_idx);
            } else if let Some(start_idx) = base_prompt.find("<user_profile>") {
                base_prompt.truncate(start_idx);
            }

            let identity_lines: Vec<String> = active_identities
                .iter()
                .map(|f| format!("- {}", f.fact))
                .collect();
            let user_profile_block = format!(
                "\n\n<user_profile>\n{}\n</user_profile>",
                identity_lines.join("\n")
            );
            let updated_content = format!("{}{}", base_prompt.trim_end(), user_profile_block);
            self.system_prompt.content = updated_content.clone();
            if !self.messages.is_empty() && self.messages[0].role == Role::System {
                self.messages[0].content = updated_content;
            }
            self.total_token_count = estimate_tokens(&self.system_prompt.content);
            log::info!(
                "[WorkingMemory] Successfully preloaded {} Identity facts into System Prompt.",
                active_identities.len()
            );
        }
        Ok(())
    }

    /// Resets conversational history and initializes a new session.
    pub fn new_session(&mut self, system_prompt: &str) {
        let sys_msg = ChatMessage {
            role: Role::System,
            content: system_prompt.to_string(),
            timestamp_ms: current_timestamp_ms(),
        };
        let sys_tokens = estimate_tokens(system_prompt);

        self.system_prompt = sys_msg.clone();
        self.messages = vec![sys_msg];
        self.total_token_count = sys_tokens;
        self.kv_synced_index = 0;
        self.session_compaction_contexts.clear();
        self.latest_compaction_facts.clear();
        self.cancel_opportunistic();
        log::info!("[WorkingMemory] New session started. System prompt set.");
    }

    /// Constructs a chronological narrative context chain from session compactions up to token cap.
    pub fn build_narrative_context_chain(&self, soft_cap_tokens: usize) -> String {
        let mut selected: Vec<&str> = Vec::new();
        let mut current_tokens = 0;

        for ctx in self.session_compaction_contexts.iter().rev() {
            let clean_ctx = ctx.trim();
            if clean_ctx.is_empty() {
                continue;
            }
            let ctx_tokens = estimate_tokens(clean_ctx);
            if selected.is_empty() || (current_tokens + ctx_tokens <= soft_cap_tokens) {
                selected.push(clean_ctx);
                current_tokens += ctx_tokens;
            } else {
                break;
            }
        }

        selected.reverse();
        selected.join(" ")
    }

    /// Replaces the active system prompt content and updates token calculations.
    pub fn update_system_prompt(&mut self, new_system_prompt: &str) {
        if self.system_prompt.content != new_system_prompt {
            let sys_tokens = estimate_tokens(new_system_prompt);
            let old_sys_tokens = estimate_tokens(&self.system_prompt.content);
            self.system_prompt.content = new_system_prompt.to_string();
            if !self.messages.is_empty() && self.messages[0].role == Role::System {
                self.messages[0].content = new_system_prompt.to_string();
                self.total_token_count =
                    self.total_token_count.saturating_sub(old_sys_tokens) + sys_tokens;
            }
            self.kv_synced_index = 0;
        }
    }

    /// Appends a new user turn to working memory.
    pub fn push_user_turn(&mut self, text: String) {
        let tokens = estimate_tokens(&text);
        let msg = ChatMessage {
            role: Role::User,
            content: text,
            timestamp_ms: current_timestamp_ms(),
        };
        self.messages.push(msg);
        self.total_token_count += tokens;
        log::debug!(
            "[WorkingMemory] User turn pushed. Total tokens: {} / {}",
            self.total_token_count,
            self.max_context_tokens
        );
    }

    /// Appends a new assistant turn to working memory.
    pub fn push_assistant_turn(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        let tokens = estimate_tokens(&text);
        let msg = ChatMessage {
            role: Role::Assistant,
            content: text,
            timestamp_ms: current_timestamp_ms(),
        };
        self.messages.push(msg);
        self.total_token_count += tokens;
        self.kv_synced_index = self.messages.len();
        log::debug!(
            "[WorkingMemory] Assistant turn pushed. Total tokens: {} / {}. KV index: {}",
            self.total_token_count,
            self.max_context_tokens,
            self.kv_synced_index
        );
    }

    /// Removes the latest user turn if cancelled by barge-in.
    pub fn pop_last_user_turn(&mut self) {
        if let Some(last) = self.messages.last() {
            if last.role == Role::User {
                let popped = self.messages.pop();
                if let Some(msg) = popped {
                    let tokens = estimate_tokens(&msg.content);
                    self.total_token_count = self.total_token_count.saturating_sub(tokens);
                    log::info!("[WorkingMemory] Popped pending user turn due to barge-in/cancel.");
                }
            }
        }
    }

    /// Computes percentage of usable context budget consumed by current conversation.
    pub fn context_utilization(&self) -> f32 {
        let usable_budget = self
            .max_context_tokens
            .saturating_sub(self.reserved_generation_tokens)
            .max(1);
        self.total_token_count as f32 / usable_budget as f32
    }

    /// Returns a slice view of active conversation messages.
    pub fn get_messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    /// Returns true if memory utilization has crossed the critical threshold.
    pub fn needs_threshold_maintenance(&self) -> bool {
        self.context_utilization() >= self.critical_threshold
    }

    /// Formats recent compaction narrative chain and facts into XML session history.
    fn build_session_history_xml(&self, soft_cap_tokens: usize) -> String {
        let narrative_chain = self.build_narrative_context_chain(soft_cap_tokens.max(50));
        let mut session_history = String::new();

        if !narrative_chain.is_empty() || !self.latest_compaction_facts.is_empty() {
            session_history.push_str("<session_history>\n");
            if !narrative_chain.is_empty() {
                session_history.push_str("  <narrative_chain>\n  ");
                session_history.push_str(&narrative_chain);
                session_history.push_str("\n  </narrative_chain>\n");
            }
            if !self.latest_compaction_facts.is_empty() {
                session_history.push_str("  <recent_compaction_facts>\n");
                for (col, facts) in &self.latest_compaction_facts {
                    if !facts.is_empty() {
                        session_history.push_str(&format!("    [{}]\n", col));
                        for f in facts {
                            session_history.push_str(&format!("    - {}\n", f));
                        }
                    }
                }
                session_history.push_str("  </recent_compaction_facts>\n");
            }
            session_history.push_str("</session_history>");
        }

        session_history
    }

    /// Consolidates session history XML into the root System Message.
    fn consolidate_system_message(&mut self, session_history: &str) {
        if session_history.is_empty()
            || self.messages.is_empty()
            || self.messages[0].role != Role::System
        {
            return;
        }

        let base_prompt = &self.system_prompt.content;
        let consolidated_prompt = if let Some(idx) = base_prompt.find("<user_profile>") {
            let (prefix, suffix) = base_prompt.split_at(idx);
            format!("{}\n{}\n\n{}", prefix.trim_end(), session_history, suffix)
        } else {
            format!("{}\n\n{}", base_prompt, session_history)
        };

        let old_sys_tokens = estimate_tokens(&self.messages[0].content);
        let new_sys_tokens = estimate_tokens(&consolidated_prompt);
        self.messages[0].content = consolidated_prompt;
        self.total_token_count =
            self.total_token_count.saturating_sub(old_sys_tokens) + new_sys_tokens;
    }

    /// Prepares context for LLM generation with threshold maintenance if required.
    pub fn build_context(
        &mut self,
        provider_kind: ProviderKind,
        is_devanagari: bool,
        llm_provider: Option<&dyn LlmProvider>,
        settings: Option<&crate::core::settings::LlmSettings>,
    ) -> (
        ConversationContext,
        Option<String>,
        HashMap<String, Vec<String>>,
    ) {
        self.cancel_opportunistic();

        let mut transition_speech = None;
        let mut personal_memory = HashMap::new();

        if self.needs_threshold_maintenance() {
            log::warn!(
                "[WorkingMemory] Critical threshold reached ({:.1}% utilization). Performing Maintenance...",
                self.context_utilization() * 100.0
            );

            let msg_set = if is_devanagari {
                TRANSITION_MESSAGES_HI
            } else {
                TRANSITION_MESSAGES_EN
            };
            let random_idx = (current_timestamp_ms() as usize) % msg_set.len();
            transition_speech = Some(msg_set[random_idx].to_string());

            let use_fifo = match provider_kind {
                ProviderKind::Embedded => self.max_context_tokens <= 4096,
                ProviderKind::OpenAiCompat => false,
            };

            if use_fifo || llm_provider.is_none() {
                self.perform_fifo_maintenance();
            } else if let Some(provider) = llm_provider {
                match self.perform_compaction_maintenance(provider, settings) {
                    Ok(updates) => {
                        personal_memory = updates;
                    }
                    Err(e) => {
                        log::error!(
                            "[WorkingMemory] LLM compaction failed: {}. Falling back to FIFO.",
                            e
                        );
                        self.perform_fifo_maintenance();
                    }
                }
            }
        }

        let soft_cap = ((self.max_context_tokens as f32) * NARRATIVE_CHAIN_SOFT_CAP_SHARE) as usize;
        let session_history = self.build_session_history_xml(soft_cap);
        self.consolidate_system_message(&session_history);

        let kv_idx = if provider_kind == ProviderKind::Embedded {
            self.kv_synced_index
        } else {
            0
        };

        let ctx = ConversationContext {
            messages: self.messages.clone(),
            token_count: self.total_token_count,
            kv_cache_index: kv_idx,
        };

        (ctx, transition_speech, personal_memory)
    }

    /// FIFO Sliding Window Shift: Drops oldest (User, Assistant) pairs until below soft threshold.
    fn perform_fifo_maintenance(&mut self) {
        log::info!("[WorkingMemory] Executing FIFO Sliding Window shift...");

        while self.messages.len() > 3 && self.context_utilization() > self.soft_threshold {
            let mut removed_tokens = 0;
            if self.messages.len() >= 3
                && self.messages[1].role == Role::User
                && self.messages[2].role == Role::Assistant
            {
                removed_tokens += estimate_tokens(&self.messages[1].content);
                removed_tokens += estimate_tokens(&self.messages[2].content);
                self.messages.remove(1);
                self.messages.remove(1);
            } else if self.messages.len() >= 2 {
                removed_tokens += estimate_tokens(&self.messages[1].content);
                self.messages.remove(1);
            } else {
                break;
            }
            self.total_token_count = self.total_token_count.saturating_sub(removed_tokens);
        }

        self.kv_synced_index = 0;
        log::info!(
            "[WorkingMemory] FIFO shift complete. Retained {} messages ({} tokens, utilization {:.1}%).",
            self.messages.len(),
            self.total_token_count,
            self.context_utilization() * 100.0
        );
    }

    /// Rebuilds message buffer and session compaction state from successful ingestion result.
    fn apply_compaction_result(
        &mut self,
        result: &crate::services::memory::ingestion::CompactionResult,
        last_user_turn: ChatMessage,
    ) -> HashMap<String, Vec<String>> {
        let context_summary = result.context_summary.trim().to_string();
        if !context_summary.is_empty() {
            self.session_compaction_contexts.push(context_summary);
        }

        let mut facts_9_col = result.personal_memory.clone();
        facts_9_col.remove("Context");
        facts_9_col.remove("Narrative");
        self.latest_compaction_facts = facts_9_col;

        let sys_tokens = estimate_tokens(&self.system_prompt.content);
        let user_tokens = estimate_tokens(&last_user_turn.content);

        self.messages = vec![self.system_prompt.clone(), last_user_turn];
        self.total_token_count = sys_tokens + user_tokens;
        self.kv_synced_index = 0;

        log::info!(
            "[WorkingMemory] Compaction complete. Context rebuilt with 2 items ({} tokens, utilization {:.1}%). Total session compactions: {}.",
            self.total_token_count,
            self.context_utilization() * 100.0,
            self.session_compaction_contexts.len()
        );

        result.diff_to_enqueue.clone()
    }

    /// LLM-driven Context Compaction: Delegates ingestion & prompt generation to `ingestion::run_compaction`.
    fn perform_compaction_maintenance(
        &mut self,
        provider: &dyn LlmProvider,
        settings: Option<&crate::core::settings::LlmSettings>,
    ) -> anyhow::Result<HashMap<String, Vec<String>>> {
        if self.messages.len() <= 3 {
            self.perform_fifo_maintenance();
            return Ok(HashMap::new());
        }

        let last_user_turn = self
            .messages
            .pop()
            .ok_or_else(|| anyhow::anyhow!("No user turn"))?;
        let history_slice = &self.messages[1..];

        match crate::services::memory::ingestion::run_compaction(
            provider,
            history_slice,
            settings,
        ) {
            Ok(result) => Ok(self.apply_compaction_result(&result, last_user_turn)),
            Err(e) => {
                log::warn!(
                    "[WorkingMemory] Ingestion compaction failed: {}. Falling back to FIFO shift.",
                    e
                );
                self.messages.push(last_user_turn);
                self.perform_fifo_maintenance();
                Ok(HashMap::new())
            }
        }
    }

    /// Attempts to initiate an opportunistic background compaction when between soft and critical thresholds.
    pub fn try_trigger_opportunistic(
        &mut self,
    ) -> Option<(usize, Vec<ChatMessage>, Arc<AtomicBool>)> {
        if self.context_utilization() > self.soft_threshold
            && self.context_utilization() < self.critical_threshold
            && !self.opportunistic_active
            && self.messages.len() > 3
        {
            self.opportunistic_active = true;
            self.opportunistic_cancel = Arc::new(AtomicBool::new(false));
            log::info!(
                "[WorkingMemory] Triggering Opportunistic Compaction candidate at {:.1}% utilization.",
                self.context_utilization() * 100.0
            );
            Some((
                self.messages.len(),
                self.messages.clone(),
                Arc::clone(&self.opportunistic_cancel),
            ))
        } else {
            None
        }
    }

    /// Commits opportunistic compaction results if no user turns were added during processing.
    pub fn commit_opportunistic(&mut self, snapshot_len: usize, summary_text: String) -> bool {
        if !self.opportunistic_active {
            log::info!("[WorkingMemory] Commit rejected: Opportunistic compaction was inactive.");
            return false;
        }
        if self.opportunistic_cancel.load(Ordering::Relaxed) {
            self.opportunistic_active = false;
            log::info!("[WorkingMemory] Commit rejected: Opportunistic compaction was cancelled.");
            return false;
        }
        if self.messages.len() != snapshot_len {
            self.opportunistic_active = false;
            log::info!(
                "[WorkingMemory] Commit rejected: Race detected (expected {} items, current has {}).",
                snapshot_len,
                self.messages.len()
            );
            return false;
        }

        let last_user_turn = self.messages.pop().unwrap();
        let summary_msg = ChatMessage {
            role: Role::System,
            content: format!("[Summary of prior context: {}]", summary_text),
            timestamp_ms: current_timestamp_ms(),
        };

        self.messages = vec![self.system_prompt.clone(), summary_msg, last_user_turn];

        let mut count = 0;
        for msg in &self.messages {
            count += estimate_tokens(&msg.content);
        }
        self.total_token_count = count;
        self.kv_synced_index = 0;
        self.opportunistic_active = false;

        log::info!(
            "[WorkingMemory] Opportunistic Compaction COMMITTED successfully! Utilization now {:.1}%.",
            self.context_utilization() * 100.0
        );

        true
    }

    /// Handles idle pipeline transitions.
    pub fn on_pipeline_idle(&mut self) {}

    /// Cancels active opportunistic compaction on speech detection.
    pub fn on_speech_start(&mut self) {
        self.cancel_opportunistic();
    }

    /// Aborts any running opportunistic compaction task.
    pub fn cancel_opportunistic(&mut self) {
        if self.opportunistic_active {
            self.opportunistic_cancel.store(true, Ordering::Relaxed);
            self.opportunistic_active = false;
            log::info!("[WorkingMemory] Opportunistic compaction cancelled.");
        }
    }

    /// Returns the most recent compaction summary string if present in history.
    pub fn latest_summary(&self) -> String {
        for msg in self.messages.iter().rev() {
            if msg.role == Role::System {
                if let Some(s) = msg.content.strip_prefix("[Compacted History Summary: ") {
                    return s.strip_suffix("]").unwrap_or(s).to_string();
                }
                if let Some(s) = msg.content.strip_prefix("[Summary of prior context: ") {
                    return s.strip_suffix("]").unwrap_or(s).to_string();
                }
            }
        }
        String::new()
    }
}
