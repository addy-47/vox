use crate::{
    harness::{ChatMessage, ConversationHistoryStage, Role},
    services::memory::ml::estimate_tokens,
};

pub const SOFT_COMPACTION_THRESHOLD_PERCENT: usize = 65;
pub const CRITICAL_COMPACTION_THRESHOLD_PERCENT: usize = 85;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextStatus {
    Nominal,
    SoftWarning,
    Critical,
}

/// Plugin managing context window token budgeting, threshold evaluation, and FIFO degradation.
#[derive(Debug, Clone)]
pub struct ContextBudgetStage {
    max_context_tokens: usize,
    reserved_generation_tokens: usize,
}

impl ContextBudgetStage {
    pub fn new(max_context_tokens: usize, reserved_generation_tokens: usize) -> Self {
        Self {
            max_context_tokens,
            reserved_generation_tokens,
        }
    }

    pub fn max_context_tokens(&self) -> usize {
        self.max_context_tokens
    }

    pub fn reserved_generation_tokens(&self) -> usize {
        self.reserved_generation_tokens
    }

    pub fn usable_budget(&self) -> usize {
        self.max_context_tokens
            .saturating_sub(self.reserved_generation_tokens)
            .max(1)
    }

    pub fn calculate_tracked_tokens(&self, messages: &[ChatMessage]) -> usize {
        messages
            .iter()
            .map(|msg| {
                let tool_call_tokens: usize = msg
                    .tool_calls
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|tc| {
                        estimate_tokens(&tc.name) + estimate_tokens(&tc.arguments.to_string())
                    })
                    .sum();
                estimate_tokens(&msg.content) + tool_call_tokens
            })
            .sum()
    }

    /// Calculates tracked tokens across history and active tool schemas.
    pub fn calculate_tracked_tokens_with_extras(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[crate::services::llm::CanonicalToolDefinition]>,
    ) -> usize {
        let history_tokens: usize = self.calculate_tracked_tokens(messages);
        let tool_tokens: usize = match tools {
            Some(tool_list) => tool_list
                .iter()
                .map(|t| {
                    estimate_tokens(&t.name)
                        + estimate_tokens(&t.description)
                        + estimate_tokens(&t.parameters.to_string())
                })
                .sum(),
            None => 0,
        };

        history_tokens + tool_tokens
    }

    /// Estimates completion tokens for a step given preamble, thinking text, and canonical tool calls.
    pub fn estimate_step_completion_tokens(
        preamble_text: &str,
        thinking_text: &str,
        calls: &[crate::services::llm::CanonicalToolCall],
    ) -> u32 {
        let preamble_tok = estimate_tokens(preamble_text);
        let thinking_tok = estimate_tokens(thinking_text);
        let tool_calls_tok: usize = calls
            .iter()
            .map(|c| estimate_tokens(&c.name) + estimate_tokens(&c.arguments.to_string()))
            .sum();
        (preamble_tok + thinking_tok + tool_calls_tok) as u32
    }

    /// Estimates prompt tokens for a step given messages and active tool definitions.
    pub fn estimate_step_prompt_tokens(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[crate::services::llm::CanonicalToolDefinition]>,
    ) -> u32 {
        self.calculate_tracked_tokens_with_extras(messages, tools) as u32
    }

    /// Calculates the maximum allowed observation tokens for a tool execution (clamped to 30% of remaining usable context, max 2000, min 100).
    pub fn calculate_max_observation_tokens(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[crate::services::llm::CanonicalToolDefinition]>,
    ) -> usize {
        let tracked = self.calculate_tracked_tokens_with_extras(messages, tools);
        let remaining = self.usable_budget().saturating_sub(tracked);
        let cap = ((remaining as f32) * 0.30) as usize;
        cap.clamp(500, 2000)
    }

    pub fn evaluate_utilization(&self, tracked_tokens: usize) -> (f32, ContextStatus) {
        let usable = self.usable_budget() as f32;
        let utilization = tracked_tokens as f32 / usable;
        let percent = (utilization * 100.0) as usize;

        let status = if percent >= CRITICAL_COMPACTION_THRESHOLD_PERCENT {
            ContextStatus::Critical
        } else if percent >= SOFT_COMPACTION_THRESHOLD_PERCENT {
            ContextStatus::SoftWarning
        } else {
            ContextStatus::Nominal
        };

        (utilization, status)
    }

    pub fn execute_fifo_shift(&self, history: &mut ConversationHistoryStage) -> usize {
        let target_budget = (self.usable_budget() * SOFT_COMPACTION_THRESHOLD_PERCENT) / 100;
        let mut dropped = 0;

        while self.calculate_tracked_tokens(history.messages()) > target_budget {
            let msgs = history.messages();
            let user_indices: Vec<usize> = msgs
                .iter()
                .enumerate()
                .filter(|(_, m)| m.role == Role::User)
                .map(|(i, _)| i)
                .collect();

            // Never drop the active turn: we need at least 2 User turns to drop the older one.
            if user_indices.len() < 2 {
                break;
            }

            let start = user_indices[0];
            let end = user_indices[1];
            let unit_len = end - start;

            history.truncate_range(start..end);
            dropped += unit_len;
        }

        log::warn!(
            "[Harness::Budget] FIFO sliding window shift executed. Dropped {} messages across complete turn units. Remaining: {}",
            dropped,
            history.messages().len()
        );

        dropped
    }
}
