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
pub const EDGE_CLASSIFIER_MODEL_DIR: &str = "classifier/modernbert_edge_creation";
pub const EDGE_CLASSIFIER_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const MEMORY_SCOPE_MODEL_DIR: &str = "modernbert_memory_scope";
pub const CLASSIFIER_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const CLASSIFIER_TOKENIZER_FILENAME: &str = "tokenizer.json";

pub use crate::core::error::MemoryError;

pub use classifiers::inter_edge_classifier::{
    classify_edge, ensure_edge_classifier_loaded, init_edge_classifier, is_edge_classifier_loaded,
};
pub use classifiers::intra_edge_classifier::{
    classify_batch, ensure_nli_loaded, init_nli_engine, is_nli_loaded, relation_from_result,
    NliLabel, NliRelation,
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

/// Evicts the 3 memory pipeline worker ONNX models (MiniLM embedder, DeBERTa v3 NLI, ModernBERT Edge Classifier).
pub fn unload_memory_pipeline_onnx_models() {
    embedder::unload_embedder();
    classifiers::intra_edge_classifier::unload_nli_engine();
    classifiers::inter_edge_classifier::unload_edge_classifier();
    trim_heap("MemorySubsystem::unload_memory_pipeline_onnx_models");
    log::info!("[MemorySubsystem] Evicted 3 memory pipeline ONNX models from process memory.");
}

/// Evicts all ONNX models (memory pipeline + query scope classifier + transliteration engine).
pub fn unload_all_onnx_models() {
    unload_memory_pipeline_onnx_models();
    classifiers::query_classifier::unload_scope_classifier();
    crate::services::translit::unload_transliteration_engine();
    trim_heap("MemorySubsystem::unload_all_onnx_models");
    log::info!("[MemorySubsystem] Evicted all ONNX models from process memory.");
}

/// Releases physical memory pages back to the OS after model eviction.
pub(crate) fn trim_heap(caller: &str) {
    #[cfg(target_os = "linux")]
    {
        unsafe {
            libc::malloc_trim(0);
        }
        log::debug!("[Heap] malloc_trim(0) called from {}", caller);
    }

    #[cfg(target_os = "windows")]
    {
        extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn EmptyWorkingSet(hProcess: *mut std::ffi::c_void) -> i32;
        }
        let ok = unsafe { EmptyWorkingSet(GetCurrentProcess()) };
        if ok != 0 {
            log::debug!("[Heap] EmptyWorkingSet succeeded (called from {})", caller);
        } else {
            log::warn!(
                "[Heap] EmptyWorkingSet returned 0 (called from {}). Non-fatal.",
                caller
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        log::debug!(
            "[Heap] trim_heap no-op on macOS (called from {}). OS allocator self-manages.",
            caller
        );
    }
}
