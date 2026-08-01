pub mod pipeline;
pub mod query_classifier;
pub mod scope_router;
pub mod deduplication;
pub mod embedder;
pub mod formatter;
pub mod ingestion;
pub mod edge_classifier;
pub mod nli;
pub mod retrieval;
pub mod tokenizer;
pub mod working_memory;

pub use crate::core::error::MemoryError;

pub use query_classifier::{
    classify_scope, ensure_scope_classifier_loaded, init_scope_classifier,
    is_scope_classifier_loaded,
};
pub use embedder::{
    cosine_similarity, ensure_embedder_loaded, generate_embedding, init_embedder,
    is_embedder_loaded,
};
pub use formatter::format_relative_timestamp;
pub use retrieval::{
    retrieve_personal_context_v7, MemoryFact,
};
pub use tokenizer::estimate_tokens;
pub use working_memory::{
    ChatMessage, ConversationContext, ConversationManager, Role,
};
pub use query_sieve::MemoryScope;

