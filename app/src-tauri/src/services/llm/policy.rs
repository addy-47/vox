use crate::core::settings::LlmSettings;
use crate::services::llm::{
    ConversationInput, GenerationOptions, GenerationPurpose, GenerationRequest, OutputConstraint,
};

/// Calculates dynamic max compaction output tokens based on context window size.
pub fn calculate_compaction_max_tokens(ctx_size: u32) -> u32 {
    let ctx = ctx_size as f32;
    let ratio = if ctx <= 8192.0 {
        let t = ((ctx - 2048.0) / (8192.0 - 2048.0)).clamp(0.0, 1.0);
        0.30 - t * (0.30 - 0.15)
    } else {
        let t = ((ctx - 8192.0) / (1_000_000.0 - 8192.0)).clamp(0.0, 1.0);
        0.15 - t * (0.15 - 0.10)
    };

    let raw = (ctx * ratio) as u32;
    raw.clamp(256, 16_384)
}

/// Policy defaults for a given generation purpose.
#[derive(Debug, Clone)]
pub struct GenerationDefaults {
    pub temperature: f32,
    pub max_output_tokens: u32,
    pub output: OutputConstraint,
}

/// Generation policy engine translating user/system settings into generation requests.
#[derive(Debug, Clone)]
pub struct GenerationPolicy {
    pub conversation: GenerationDefaults,
    pub compaction: GenerationDefaults,
}

impl GenerationPolicy {
    /// Constructs policy from current `LlmSettings`.
    pub fn from_settings(settings: &LlmSettings) -> Self {
        let eff_ctx = settings.effective_ctx_size();
        let compaction_max_tokens = calculate_compaction_max_tokens(eff_ctx);

        Self {
            conversation: GenerationDefaults {
                temperature: settings.temperature,
                max_output_tokens: settings.max_output_tokens,
                output: OutputConstraint::Text,
            },
            compaction: GenerationDefaults {
                temperature: settings.compaction_temperature,
                max_output_tokens: compaction_max_tokens,
                output: OutputConstraint::JsonObject,
            },
        }
    }

    /// Builds a provider-neutral `GenerationRequest` for a specified purpose.
    pub fn build_request(
        &self,
        purpose: GenerationPurpose,
        input: ConversationInput,
    ) -> GenerationRequest {
        let defaults = match purpose {
            GenerationPurpose::Conversation => &self.conversation,
            GenerationPurpose::MemoryCompaction | GenerationPurpose::StructuredExtraction => {
                &self.compaction
            }
        };

        GenerationRequest {
            input,
            options: GenerationOptions {
                temperature: Some(defaults.temperature),
                max_output_tokens: Some(defaults.max_output_tokens),
                ..Default::default()
            },
            output: defaults.output.clone(),
            purpose,
        }
    }
}
