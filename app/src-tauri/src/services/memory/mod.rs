pub mod classifier;
pub mod embedder;
pub mod retrieval;
pub mod tokenizer;
pub mod working_memory;
pub mod nli;
pub mod personal_memory;

pub use classifier::{
    classify_query, ensure_classifier_loaded, init_classifier, is_classifier_loaded,
};
pub use embedder::{
    cosine_similarity, ensure_embedder_loaded, generate_embedding, init_embedder,
    is_embedder_loaded,
};
pub use retrieval::{
    format_retrieved_memories_for_prompt, retrieve_episodic_memories, RetrievedEpisode,
    retrieve_and_format_memory_context,
};
pub use tokenizer::estimate_tokens;
pub use working_memory::{
    ChatMessage, ConversationContext, ConversationManager, Role,
};
