use crate::services::{harness::PromptTag, memory::ml::tokenizer::estimate_tokens};

/// Plugin managing system prompt assembly, user identity grounding, and system budget ceiling enforcement.
#[derive(Debug, Clone)]
pub struct PromptBuilderPlugin {
    base_system_prompt: String,
    personal_memory: Option<String>,
    max_context_tokens: usize,
    max_context_share: f32,
}

impl PromptBuilderPlugin {
    /// Creates a new `PromptBuilderPlugin` with base prompt, token ceiling, and configurable memory share.
    pub fn new(
        base_system_prompt: String,
        max_context_tokens: usize,
        max_context_share: f32,
    ) -> Self {
        Self {
            base_system_prompt,
            personal_memory: None,
            max_context_tokens,
            max_context_share,
        }
    }

    /// Sets or updates the base persona system prompt.
    pub fn set_base_system_prompt(&mut self, prompt: String) {
        self.base_system_prompt = prompt;
    }

    /// Sets or updates the active Personal Memory markdown document.
    pub fn set_personal_memory(&mut self, memory: Option<String>) {
        self.personal_memory = memory;
    }

    /// Updates the maximum context token ceiling.
    pub fn set_max_context_tokens(&mut self, max_tokens: usize) {
        self.max_context_tokens = max_tokens;
    }

    /// Updates the maximum context share allocated to personal memory.
    pub fn set_max_context_share(&mut self, share: f32) {
        self.max_context_share = share;
    }

    /// Assembles the finalized system prompt with grounded `<user_identity>` within budget ceiling.
    pub fn assemble(&self) -> String {
        let Some(ref mem) = self.personal_memory else {
            return self.base_system_prompt.clone();
        };

        let trimmed_mem = mem.trim();
        if trimmed_mem.is_empty() {
            return self.base_system_prompt.clone();
        }

        let ceiling = ((self.max_context_tokens as f32) * self.max_context_share) as usize;
        let base_tokens = estimate_tokens(&self.base_system_prompt);
        let mem_budget = ceiling.saturating_sub(base_tokens);

        let bounded_memory = self.bound_personal_memory(trimmed_mem, mem_budget);
        let wrapped_identity = PromptTag::UserIdentity.wrap(&bounded_memory);

        format!("{}\n\n{}", self.base_system_prompt.trim(), wrapped_identity)
    }

    /// Enforces the memory budget ceiling on the raw personal memory document.
    fn bound_personal_memory(&self, memory: &str, budget_tokens: usize) -> String {
        let mem_tokens = estimate_tokens(memory);
        if mem_tokens <= budget_tokens {
            return memory.to_string();
        }

        log::warn!(
            "[Harness::Prompt] Personal memory exceeds budget ceiling ({} / {} tokens). Truncating.",
            mem_tokens,
            budget_tokens
        );

        let char_limit = budget_tokens * 4;
        if memory.len() > char_limit {
            memory[..char_limit].to_string()
        } else {
            memory.to_string()
        }
    }
}
