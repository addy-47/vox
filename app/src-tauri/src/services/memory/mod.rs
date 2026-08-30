pub mod compaction;
pub mod harness;
pub mod ingestion;
pub mod ml;
pub mod retrieval;

pub const RESERVED_GENERATION_TOKENS: usize = 512;
pub const CONTEXT_CRITICAL_THRESHOLD: f32 = 0.85;
pub const CONTEXT_SOFT_THRESHOLD: f32 = 0.65;
pub const SOFT_COMPACTION_DEBOUNCE_SECS: u64 = 20;

pub const COSINE_HARD_MATCH_THRESHOLD: f32 = 0.98;
pub const JACCARD_EXACT_MATCH_THRESHOLD: f32 = 1.0;
pub const SOFT_VECTOR_DEDUP_THRESHOLD: f32 = 0.95;
pub const SAME_COLLECTION_CANDIDATE_SEARCH: f32 = 0.60;
pub const INTER_COLLECTION_CANDIDATE_SEARCH: f32 = 0.40;
pub const SUBFLOOR_CANDIDATE_FLOOR: f32 = 0.25;

pub const NLI_CONTRADICTION_THRESHOLD: f32 = 0.85;
pub const NLI_ENTAILMENT_THRESHOLD: f32 = 0.85;
pub const NLI_CONTRADICTION_CONFIDENCE_THRESHOLD: f32 = 0.85;
pub const NLI_CONTRADICTION_MARGIN_THRESHOLD: f32 = 0.20;
pub const NLI_ENTAILMENT_CONFIDENCE_THRESHOLD: f32 = 0.85;

pub const EDGE_CLASSIFIER_THRESHOLD: f32 = 0.80;

pub const STAGE1_BATCH_CEILING: usize = 128;
pub const STAGE2_BATCH_SIZE: usize = 16;
pub const STAGE3_BATCH_SIZE: usize = 16;
pub const STAGE4_BATCH_SIZE: usize = 32;

pub const NARRATIVE_CHAIN_SOFT_CAP_SHARE: f32 = 0.05;
pub const EMBEDDING_DIM: usize = 384;
pub const PRIMARY_EMBEDDING_MODEL_DIR: &str = "minilm-l12-v2";
pub const PRIMARY_EMBEDDING_MODEL_FILENAME: &str = "model_int8.onnx";
pub const FALLBACK_EMBEDDING_MODEL_DIR: &str = "bge-m3";
pub const FALLBACK_EMBEDDING_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const EMBEDDING_TOKENIZER_FILENAME: &str = "tokenizer.json";
pub const NLI_MODEL_DIR: &str = "nli-deberta-v3-base";
pub const NLI_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const NLI_TOKENIZER_FILENAME: &str = "tokenizer.json";
pub const EDGE_CLASSIFIER_MODEL_DIR: &str = "classifier/modernbert_edge_creation";
pub const EDGE_CLASSIFIER_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const EDGE_CLASSIFIER_TOKENIZER_FILENAME: &str = "tokenizer.json";
pub const MEMORY_SCOPE_MODEL_DIR: &str = "modernbert_memory_scope";
pub const CLASSIFIER_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const CLASSIFIER_TOKENIZER_FILENAME: &str = "tokenizer.json";

pub use crate::core::error::MemoryError;

pub use compaction::{run_compaction, CompactionResult, COMPACTION_SYSTEM_PROMPT};
pub use harness::prompt_builder::format_relative_timestamp;
pub use harness::{
    prepare_turn_context, trigger_background_compaction, ChatMessage, ConversationContext,
    ConversationManager, PrepareTurnParams, Role,
};
pub use ml::edge_classifier::{
    classify_edge, ensure_edge_classifier_loaded, init_edge_classifier, is_edge_classifier_loaded,
};
pub use ml::embedder::{
    cosine_similarity, ensure_embedder_loaded, generate_embedding, init_embedder,
    is_embedder_loaded,
};
pub use ml::nli::{
    classify_batch, ensure_nli_loaded, init_nli_engine, is_nli_loaded, relation_from_result,
    NliLabel, NliRelation,
};
pub use ml::scope_classifier::{
    classify_scope, ensure_scope_classifier_loaded, init_scope_classifier,
    is_scope_classifier_loaded,
};
pub use ml::tokenizer::estimate_tokens;
pub(crate) use ml::trim_heap;
pub use ml::{unload_all_onnx_models, unload_memory_pipeline_onnx_models};
pub use query_sieve::MemoryScope;
pub use retrieval::{retrieve_turn_profile, MemoryFact, RetrievedProfile};
