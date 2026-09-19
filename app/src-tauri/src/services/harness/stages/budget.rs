use crate::services::{
    harness::{ChatMessage, ConversationHistoryStage},
    memory::ml::tokenizer::estimate_tokens,
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

    pub fn set_max_context_tokens(&mut self, max_tokens: usize) {
        self.max_context_tokens = max_tokens;
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
            .map(|msg| estimate_tokens(&msg.content))
            .sum()
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
            if history.messages().len() <= 2 {
                break;
            }
            history.truncate_oldest_turns(2);
            dropped += 2;
        }

        log::warn!(
            "[Harness::Budget] FIFO sliding window shift executed. Dropped {} turns. Remaining: {}",
            dropped,
            history.messages().len()
        );

        dropped
    }
}
