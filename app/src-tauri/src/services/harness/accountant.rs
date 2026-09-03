use super::buffer::{current_timestamp_ms, ChatMessage, MessageBuffer, Role};
use super::prompt_builder::{build_session_history_xml, consolidate_system_message};
use crate::services::memory::compaction::CompactionResult;
use crate::services::memory::ml::estimate_tokens;
use crate::services::memory::MemoryCollection;
use crate::services::memory::{
    CONTEXT_CRITICAL_THRESHOLD, CONTEXT_SOFT_THRESHOLD, RESERVED_GENERATION_TOKENS,
};
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

/// Manages context budgeting, sliding-window compaction, and background opportunistic compaction for Modular LLM.
pub struct ContextHarness {
    pub accountant: TokenAccountant,
    session_compaction_contexts: Vec<String>,
    latest_compaction_facts: HashMap<String, Vec<String>>,
    opportunistic_active: bool,
    opportunistic_cancel: CancellationToken,
}

/// Tracks token usage, enforces budget thresholds, and executes FIFO maintenance.
#[derive(Debug, Clone)]
pub struct TokenAccountant {
    total_token_count: usize,
    max_context_tokens: usize,
    reserved_generation_tokens: usize,
    critical_threshold: f32,
    soft_threshold: f32,
}

impl TokenAccountant {
    /// Creates a new token accountant with specified max context budget and initial token count.
    pub fn new(max_context_tokens: usize, initial_tokens: usize) -> Self {
        Self {
            total_token_count: initial_tokens,
            max_context_tokens,
            reserved_generation_tokens: RESERVED_GENERATION_TOKENS,
            critical_threshold: CONTEXT_CRITICAL_THRESHOLD,
            soft_threshold: CONTEXT_SOFT_THRESHOLD,
        }
    }

    /// Returns the current total token count across active messages.
    pub fn total_token_count(&self) -> usize {
        self.total_token_count
    }

    /// Sets the total token count directly.
    pub fn set_total_token_count(&mut self, count: usize) {
        self.total_token_count = count;
    }

    /// Increments the total token count by the specified amount.
    pub fn add_tokens(&mut self, tokens: usize) {
        self.total_token_count += tokens;
    }

    /// Decrements the total token count by the specified amount with saturation.
    pub fn sub_tokens(&mut self, tokens: usize) {
        self.total_token_count = self.total_token_count.saturating_sub(tokens);
    }

    /// Returns the maximum context token limit.
    pub fn max_context_tokens(&self) -> usize {
        self.max_context_tokens
    }

    /// Updates the maximum allowable context token budget.
    pub fn set_max_context_tokens(&mut self, max_tokens: usize) {
        if max_tokens > 0 {
            self.max_context_tokens = max_tokens;
            log::info!(
                "[TokenAccountant] Updated max_context_tokens to {}",
                max_tokens
            );
        }
    }

    /// Computes percentage of usable context budget consumed by active conversation.
    pub fn context_utilization(&self) -> f32 {
        let usable_budget = self
            .max_context_tokens
            .saturating_sub(self.reserved_generation_tokens)
            .max(1);
        self.total_token_count as f32 / usable_budget as f32
    }

    /// Returns true if memory utilization has crossed the critical threshold.
    pub fn needs_threshold_maintenance(&self) -> bool {
        self.context_utilization() >= self.critical_threshold
    }

    /// Returns true if utilization is between soft and critical thresholds for opportunistic compaction.
    pub fn is_in_soft_compaction_window(&self) -> bool {
        let util = self.context_utilization();
        util >= self.soft_threshold && util < self.critical_threshold
    }

    /// Drops oldest (User, Assistant) pairs until below soft threshold.
    pub fn perform_fifo_maintenance(&mut self, buffer: &mut MessageBuffer) {
        log::info!("[TokenAccountant] Executing FIFO Sliding Window shift...");

        while buffer.messages.len() > 3 && self.context_utilization() > self.soft_threshold {
            let mut removed_tokens = 0;
            if buffer.messages.len() >= 3
                && buffer.messages[1].role == Role::User
                && buffer.messages[2].role == Role::Assistant
            {
                removed_tokens += estimate_tokens(&buffer.messages[1].content);
                removed_tokens += estimate_tokens(&buffer.messages[2].content);
                buffer.messages.remove(1);
                buffer.messages.remove(1);
            } else if buffer.messages.len() >= 2 {
                removed_tokens += estimate_tokens(&buffer.messages[1].content);
                buffer.messages.remove(1);
            } else {
                break;
            }
            self.total_token_count = self.total_token_count.saturating_sub(removed_tokens);
        }

        buffer.kv_synced_index = 0;
        log::info!(
            "[TokenAccountant] FIFO shift complete. Retained {} messages ({} tokens, utilization {:.1}%).",
            buffer.messages.len(),
            self.total_token_count,
            self.context_utilization() * 100.0
        );
    }

    /// Constructs a chronological narrative context chain from session compactions up to token cap.
    pub fn build_narrative_context_chain(
        session_compaction_contexts: &[String],
        soft_cap_tokens: usize,
    ) -> String {
        let mut selected: Vec<&str> = Vec::new();
        let mut current_tokens = 0;

        for ctx in session_compaction_contexts.iter().rev() {
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
}

impl ContextHarness {
    /// Creates a new ContextHarness instance for modular LLM context management.
    pub fn new(max_context_tokens: usize) -> Self {
        Self {
            accountant: TokenAccountant::new(max_context_tokens, 0),
            session_compaction_contexts: Vec::new(),
            latest_compaction_facts: HashMap::new(),
            opportunistic_active: false,
            opportunistic_cancel: CancellationToken::new(),
        }
    }

    /// Recalculates total tokens across all messages in the buffer.
    pub fn sync_tokens_from_buffer(&mut self, buffer: &MessageBuffer) {
        let mut total = 0;
        for msg in &buffer.messages {
            total += estimate_tokens(&msg.content);
        }
        self.accountant.set_total_token_count(total);
    }

    /// Resets session context compaction state.
    pub fn reset(&mut self) {
        self.session_compaction_contexts.clear();
        self.latest_compaction_facts.clear();
        self.cancel_opportunistic();
    }

    /// Computes percentage of usable context budget consumed by active conversation.
    pub fn context_utilization(&self) -> f32 {
        self.accountant.context_utilization()
    }

    /// Returns true if memory utilization has crossed the critical threshold.
    pub fn needs_threshold_maintenance(&self) -> bool {
        self.accountant.needs_threshold_maintenance()
    }

    /// Formats recent compaction narrative chain and facts into XML session history.
    pub fn build_session_history_xml(&self, soft_cap_tokens: usize) -> String {
        let narrative_chain = TokenAccountant::build_narrative_context_chain(
            &self.session_compaction_contexts,
            soft_cap_tokens.max(50),
        );
        build_session_history_xml(&narrative_chain, &self.latest_compaction_facts)
    }

    /// Consolidates session history XML into the root System Message.
    pub fn consolidate_system_message(
        &mut self,
        buffer: &mut MessageBuffer,
        system_prompt: &ChatMessage,
        session_history: &str,
    ) {
        let mut total_tokens = self.accountant.total_token_count();
        consolidate_system_message(
            &mut buffer.messages,
            system_prompt,
            session_history,
            &mut total_tokens,
        );
        self.accountant.set_total_token_count(total_tokens);
    }

    /// FIFO Sliding Window Shift: Drops oldest (User, Assistant) pairs until below soft threshold.
    pub fn perform_fifo_maintenance(&mut self, buffer: &mut MessageBuffer) {
        self.accountant.perform_fifo_maintenance(buffer);
    }

    /// Rebuilds message buffer and session compaction state from successful compaction result.
    pub fn apply_compaction_result(
        &mut self,
        buffer: &mut MessageBuffer,
        system_prompt: &ChatMessage,
        result: &CompactionResult,
        last_user_turn: ChatMessage,
    ) -> HashMap<String, Vec<String>> {
        let context_summary = result.context_summary.trim().to_string();
        if !context_summary.is_empty() {
            self.session_compaction_contexts.push(context_summary);
        }

        let mut facts_9_col = result.personal_memory.clone();
        facts_9_col.remove(MemoryCollection::Narrative.as_str());
        facts_9_col.remove("Context");
        self.latest_compaction_facts = facts_9_col;

        let sys_tokens = estimate_tokens(&system_prompt.content);
        let user_tokens = estimate_tokens(&last_user_turn.content);

        buffer.messages = vec![system_prompt.clone(), last_user_turn];
        self.accountant
            .set_total_token_count(sys_tokens + user_tokens);
        buffer.kv_synced_index = 0;

        log::info!(
            "[ContextHarness] Compaction complete. Context rebuilt with 2 items ({} tokens, utilization {:.1}%). Total session compactions: {}.",
            self.accountant.total_token_count(),
            self.accountant.context_utilization() * 100.0,
            self.session_compaction_contexts.len()
        );

        result.diff_to_enqueue.clone()
    }

    /// Attempts to initiate an opportunistic background compaction when between soft and critical thresholds.
    pub fn try_trigger_opportunistic(
        &mut self,
        buffer: &MessageBuffer,
    ) -> Option<(usize, Vec<ChatMessage>, CancellationToken)> {
        if self.accountant.is_in_soft_compaction_window()
            && !self.opportunistic_active
            && buffer.messages.len() > 3
        {
            self.opportunistic_active = true;
            self.opportunistic_cancel = CancellationToken::new();
            log::info!(
                "[ContextHarness] Triggering Opportunistic Compaction candidate at {:.1}% utilization.",
                self.accountant.context_utilization() * 100.0
            );
            Some((
                buffer.messages.len(),
                buffer.messages.clone(),
                self.opportunistic_cancel.clone(),
            ))
        } else {
            None
        }
    }

    /// Commits opportunistic compaction results if no user turns were added during processing.
    pub fn commit_opportunistic(
        &mut self,
        buffer: &mut MessageBuffer,
        system_prompt: &ChatMessage,
        snapshot_len: usize,
        summary_text: String,
    ) -> bool {
        if !self.opportunistic_active {
            log::info!("[ContextHarness] Commit rejected: Opportunistic compaction was inactive.");
            return false;
        }
        if self.opportunistic_cancel.is_cancelled() {
            self.opportunistic_active = false;
            log::info!("[ContextHarness] Commit rejected: Opportunistic compaction was cancelled.");
            return false;
        }
        if buffer.messages.len() != snapshot_len {
            self.opportunistic_active = false;
            log::info!(
                "[ContextHarness] Commit rejected: Race detected (expected {} items, current has {}).",
                snapshot_len,
                buffer.messages.len()
            );
            return false;
        }

        let last_user_turn = match buffer.messages.pop() {
            Some(turn) => turn,
            None => return false,
        };

        let summary_msg = ChatMessage {
            role: Role::System,
            content: format!("[Summary of prior context: {}]", summary_text),
            timestamp_ms: current_timestamp_ms(),
        };

        buffer.messages = vec![system_prompt.clone(), summary_msg, last_user_turn];

        let mut count = 0;
        for msg in &buffer.messages {
            count += estimate_tokens(&msg.content);
        }
        self.accountant.set_total_token_count(count);
        buffer.kv_synced_index = 0;
        self.opportunistic_active = false;

        log::info!(
            "[ContextHarness] Opportunistic Compaction COMMITTED successfully! Utilization now {:.1}%.",
            self.accountant.context_utilization() * 100.0
        );

        true
    }

    /// Aborts any running opportunistic compaction task.
    pub fn cancel_opportunistic(&mut self) {
        if self.opportunistic_active {
            self.opportunistic_cancel.cancel();
            self.opportunistic_active = false;
            log::info!("[ContextHarness] Opportunistic compaction cancelled.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::buffer::{ChatMessage, MessageBuffer, Role};
    use super::*;

    fn sys_msg() -> ChatMessage {
        ChatMessage {
            role: Role::System,
            content: "System prompt base.".to_string(),
            timestamp_ms: 0,
        }
    }

    /// Tests context_utilization computes usable budget correctly and thresholds.
    #[test]
    fn test_token_accountant_utilization_and_thresholds() {
        let mut acc = TokenAccountant::new(4096, 0);
        assert!((acc.context_utilization() - 0.0).abs() < 1e-5);
        assert!(!acc.needs_threshold_maintenance());
        assert!(!acc.is_in_soft_compaction_window());

        let usable = 4096 - RESERVED_GENERATION_TOKENS;
        let crit_tokens = (usable as f32 * CONTEXT_CRITICAL_THRESHOLD).ceil() as usize + 1;
        acc.set_total_token_count(crit_tokens);
        assert!(acc.needs_threshold_maintenance());
        assert!(!acc.is_in_soft_compaction_window());

        let soft_tokens = (usable as f32 * CONTEXT_SOFT_THRESHOLD) as usize + 10;
        acc.set_total_token_count(soft_tokens);
        assert!(!acc.needs_threshold_maintenance());
        assert!(acc.is_in_soft_compaction_window());

        let below_soft = (usable as f32 * CONTEXT_SOFT_THRESHOLD) as usize - 10;
        acc.set_total_token_count(below_soft);
        assert!(!acc.is_in_soft_compaction_window());
        assert!(!acc.needs_threshold_maintenance());
    }

    /// Tests add/sub saturating arithmetic and set_max rejects zero.
    #[test]
    fn test_token_accountant_arithmetic_and_set_max() {
        let mut acc = TokenAccountant::new(4096, 100);
        acc.add_tokens(50);
        assert_eq!(acc.total_token_count(), 150);
        acc.sub_tokens(200);
        assert_eq!(acc.total_token_count(), 0);
        acc.sub_tokens(10);
        assert_eq!(acc.total_token_count(), 0);

        let before = acc.max_context_tokens();
        acc.set_max_context_tokens(0);
        assert_eq!(acc.max_context_tokens(), before);
        acc.set_max_context_tokens(8192);
        assert_eq!(acc.max_context_tokens(), 8192);
    }

    /// Tests perform_fifo_maintenance drops oldest pair below soft threshold.
    #[test]
    fn test_perform_fifo_maintenance_drops_oldest_pair() {
        let mut acc = TokenAccountant::new(100, 0);
        let mut buf = MessageBuffer::new(sys_msg());
        buf.push_user_turn("u1".to_string());
        buf.push_assistant_turn("a1".to_string());
        buf.push_user_turn("u2".to_string());
        buf.push_assistant_turn("a2".to_string());
        buf.push_user_turn("u3".to_string());
        let mut total = 0;
        for m in &buf.messages {
            total += estimate_tokens(&m.content);
        }
        acc.set_total_token_count(total * 2);
        let before_len = buf.messages.len();
        acc.perform_fifo_maintenance(&mut buf);
        assert!(buf.messages.len() < before_len);
        assert_eq!(buf.kv_synced_index, 0);
    }

    /// Tests build_narrative_context_chain respects soft cap and skips empty.
    #[test]
    fn test_build_narrative_chain_respects_cap() {
        let ctxs = vec![
            "        ".to_string(),
            "first context".to_string(),
            "second context which is longer".to_string(),
        ];
        let chain = TokenAccountant::build_narrative_context_chain(&ctxs, 5);
        assert!(chain.contains("second context"));
        let chain2 = TokenAccountant::build_narrative_context_chain(&ctxs, 1000);
        assert!(chain2.contains("first context"));
        assert!(chain2.contains("second context"));
        assert_eq!(
            TokenAccountant::build_narrative_context_chain(&[], 100),
            ""
        );
    }

    /// Tests ContextHarness opportunistic trigger and cancel/commit race guards.
    #[test]
    fn test_harness_opportunistic_trigger_and_race() {
        let mut harness = ContextHarness::new(100);
        let mut buf = MessageBuffer::new(sys_msg());
        buf.push_user_turn("u1".to_string());
        buf.push_assistant_turn("a1".to_string());
        buf.push_user_turn("u2".to_string());
        buf.push_assistant_turn("a2".to_string());
        buf.push_user_turn("u3".to_string());
        harness.sync_tokens_from_buffer(&buf);
        // Force into soft window without critical
        let usable = 100 - RESERVED_GENERATION_TOKENS.max(1);
        let target = (usable as f32 * 0.7) as usize;
        harness.accountant.set_total_token_count(target.max(10));
        // Ensure >3 messages
        assert!(buf.messages.len() > 3);

        let triggered = harness.try_trigger_opportunistic(&buf);
        if harness.accountant.is_in_soft_compaction_window() {
            assert!(triggered.is_some());
            let (snap_len, _msgs, _tok) = triggered.unwrap();
            // Second trigger should be None while active
            assert!(harness.try_trigger_opportunistic(&buf).is_none());
            // Race: buffer grew, commit must reject
            buf.push_user_turn("new turn".to_string());
            let ok = harness.commit_opportunistic(&mut buf, &sys_msg(), snap_len, "summary".to_string());
            assert!(!ok);
        }
        harness.cancel_opportunistic();
        assert!(!harness.opportunistic_active);
    }

    /// Tests MessageBuffer duplicate detection and pop/reset semantics.
    #[test]
    fn test_message_buffer_duplicate_and_pop() {
        let mut buf = MessageBuffer::new(sys_msg());
        assert!(!buf.is_duplicate_user_turn("hello"));
        buf.push_user_turn("hello".to_string());
        assert!(buf.is_duplicate_user_turn("hello"));
        assert!(!buf.is_duplicate_user_turn("world"));
        let popped = buf.pop_last_user_turn();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().content, "hello");
        assert!(buf.pop_last_user_turn().is_none());
        buf.push_user_turn("a".to_string());
        buf.push_assistant_turn("".to_string());
        assert_eq!(buf.messages.len(), 2);
        buf.push_assistant_turn("real".to_string());
        assert_eq!(buf.messages.len(), 3);
        buf.reset(sys_msg());
        assert_eq!(buf.messages.len(), 1);
        assert_eq!(buf.kv_synced_index, 0);
    }
}
