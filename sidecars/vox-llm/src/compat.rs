pub mod services {
    pub mod harness {
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum Role {
            System,
            User,
            Assistant,
            Tool,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct ChatMessage {
            pub role: Role,
            pub content: String,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct ConversationContext {
            pub messages: Vec<ChatMessage>,
        }
    }

    pub mod llm {
        use super::harness::ConversationContext;
        use futures_util::future::BoxFuture;
        use llama_cpp_4::llama_backend::LlamaBackend;
        use serde::{Deserialize, Serialize};
        use std::sync::{mpsc, OnceLock};

        pub const DEFAULT_BATCH_CHUNK_SIZE: usize = 512;
        pub const DEFAULT_MAX_GENERATION_SAFETY_TOKENS: usize = 512;

        pub fn global_llama_backend() -> &'static LlamaBackend {
            static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
            BACKEND.get_or_init(|| {
                let mut b =
                    LlamaBackend::init().expect("Failed to initialize global llama.cpp backend");
                b.void_logs();
                b
            })
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
        #[serde(rename_all = "snake_case")]
        pub enum ReasoningMode {
            Enabled,
            #[default]
            Disabled,
        }

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
        pub struct GenerationOptions {
            pub temperature: Option<f32>,
            pub top_p: Option<f32>,
            pub top_k: Option<u32>,
            pub max_output_tokens: Option<u32>,
            pub stop: Vec<String>,
            pub seed: Option<u64>,
            pub reasoning: ReasoningMode,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum GenerationPurpose {
            Conversation,
            MemoryCompaction,
            StructuredExtraction,
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum OutputConstraint {
            Text,
            JsonObject,
            JsonSchema(String),
        }

        #[derive(Debug, Clone)]
        pub struct GenerationInput {
            pub messages: Vec<super::harness::ChatMessage>,
        }

        #[derive(Debug, Clone)]
        pub struct GenerationRequest {
            pub input: GenerationInput,
            pub options: GenerationOptions,
            pub purpose: GenerationPurpose,
            pub output: OutputConstraint,
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum LlmStreamEvent {
            Token(String),
            Reasoning(String),
            Finished,
            Error(String),
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum Support {
            Supported,
            Unsupported,
            Unknown,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct ProviderCapabilities {
            pub temperature: Support,
            pub top_p: Support,
            pub top_k: Support,
            pub max_output_tokens: Support,
            pub json_object: Support,
            pub json_schema: Support,
            pub streaming: Support,
            pub seed: Support,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum ProviderKind {
            Embedded,
            Server,
            Cloud,
        }

        #[derive(Debug, thiserror::Error)]
        pub enum LlmError {
            #[error("Engine error: {0}")]
            Engine(String),
            #[error("Context overflow: {0}")]
            ContextOverflow(String),
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        pub struct ModelCapabilities {
            pub model_id: String,
            pub provider_kind: String,
            pub supports_tools: Support,
            pub supports_latin: Support,
            pub supports_devanagari: Support,
            pub context_window: Option<u32>,
            pub max_output_tokens: Option<u32>,
            pub tps: Option<f32>,
            pub ttft_ms: Option<u32>,
            pub server_has_gpu: bool,
            pub is_gpu_accelerated: bool,
            pub gpu_status: String,
            pub vram_bytes: Option<u64>,
            pub parameter_size: Option<String>,
            pub quantization: Option<String>,
            pub family: Option<String>,
            pub tested_at_epoch: u64,
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct LlmModelInfo {
            pub id: String,
            pub name: String,
            pub size_bytes: Option<u64>,
            pub quantization: Option<String>,
            pub family: Option<String>,
            pub provider_kind: String,
            pub capabilities: Option<ModelCapabilities>,
        }

        pub trait LlmEngine: Send + Sync {
            fn generate(
                &self,
                context: &ConversationContext,
                turn_id: u32,
                options: &GenerationOptions,
                cancel: &tokio_util::sync::CancellationToken,
                tx: &mpsc::Sender<LlmStreamEvent>,
            ) -> anyhow::Result<()>;
        }

        pub trait LlmProvider: Send + Sync {
            fn generate<'a>(
                &'a self,
                request: GenerationRequest,
                turn_id: u32,
                cancel: &'a tokio_util::sync::CancellationToken,
                tx: &'a mpsc::Sender<LlmStreamEvent>,
            ) -> BoxFuture<'a, Result<(), LlmError>>;

            fn capabilities(&self) -> &ProviderCapabilities;
            fn health_check<'a>(&'a self) -> BoxFuture<'a, Result<(), LlmError>>;
            fn list_models<'a>(&'a self) -> BoxFuture<'a, Result<Vec<LlmModelInfo>, LlmError>>;
            fn kind(&self) -> ProviderKind;
        }
    }
}
