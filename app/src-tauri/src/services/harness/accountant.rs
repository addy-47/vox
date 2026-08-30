use super::buffer::{MessageBuffer, Role};
use crate::services::memory::ml::estimate_tokens;
use crate::services::memory::{
    CONTEXT_CRITICAL_THRESHOLD, CONTEXT_SOFT_THRESHOLD, RESERVED_GENERATION_TOKENS,
};

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
