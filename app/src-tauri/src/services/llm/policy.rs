use crate::core::settings::LlmSettings;
use crate::services::llm::types::{
    ConversationInput, GenerationOptions, GenerationPurpose, GenerationRequest, OutputConstraint,
};

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
        // Dynamic compaction budget: 25% of configured context window
        let compaction_max_tokens = (settings.ctx_size as f32 * 0.25) as u32;

        Self {
            conversation: GenerationDefaults {
                temperature: settings.chat_temperature,
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
