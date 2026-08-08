pub mod classifiers;
pub mod deduplication;
pub mod embedder;
pub mod formatter;
pub mod ingestion;
pub mod pipeline;
pub mod retrieval;
pub mod scope_router;
pub mod tokenizer;
pub mod working_memory;

pub use crate::core::error::MemoryError;

pub use classifiers::inter_edge_classifier::{
    classify_edge, ensure_edge_classifier_loaded, init_edge_classifier, is_edge_classifier_loaded,
};
pub use classifiers::intra_edge_classifier::{
    classify_batch, ensure_nli_loaded, init_nli_engine, is_nli_loaded, relation_from_result,
    NliLabel, NliRelation, NLI_CONTRADICTION_THRESHOLD, NLI_ENTAILMENT_THRESHOLD, NLI_MODEL_DIR,
};
pub use classifiers::query_classifier::{
    classify_scope, ensure_scope_classifier_loaded, init_scope_classifier,
    is_scope_classifier_loaded,
};
pub use embedder::{
    cosine_similarity, ensure_embedder_loaded, generate_embedding, init_embedder,
    is_embedder_loaded,
};
pub use formatter::format_relative_timestamp;
pub use query_sieve::MemoryScope;
pub use retrieval::{retrieve_personal_context_v7, MemoryFact};
pub use tokenizer::estimate_tokens;
pub use working_memory::{ChatMessage, ConversationContext, ConversationManager, Role};
