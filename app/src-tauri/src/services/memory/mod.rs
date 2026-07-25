pub mod classifier;
pub mod deduplication;
pub mod embedder;
pub mod formatter;
pub mod ingestion;
pub mod llm_edge_classifier;
pub mod nli;
pub mod orchestrator;
pub mod retrieval;
pub mod tokenizer;
pub mod working_memory;

pub use crate::core::error::MemoryError;

pub use classifier::{
    classify_query, ensure_classifier_loaded, init_classifier, is_classifier_loaded,
};
pub use embedder::{
    cosine_similarity, ensure_embedder_loaded, generate_embedding, init_embedder,
    is_embedder_loaded,
};
pub use formatter::format_relative_timestamp;
pub use retrieval::{
    retrieve_personal_context, MemoryFact,
};
pub use tokenizer::estimate_tokens;
pub use working_memory::{
    ChatMessage, ConversationContext, ConversationManager, Role,
};
