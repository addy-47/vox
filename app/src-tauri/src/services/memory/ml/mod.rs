pub mod edge_classifier;
pub mod embedder;
pub mod nli;
pub mod scope_classifier;
pub mod tokenizer;

pub use edge_classifier::{
    classify_edge, ensure_edge_classifier_loaded, init_edge_classifier, is_edge_classifier_loaded,
    EdgeClassifierEngine,
};
pub use embedder::{
    cosine_similarity, embedding_dim, ensure_embedder_loaded, generate_embedding, init_embedder,
    is_embedder_loaded, l2_normalize_in_place, unload_embedder, TextEmbedder,
};
pub use nli::{
    classify_batch, ensure_nli_loaded, init_nli_engine, is_nli_loaded, relation_from_result,
    unload_nli_engine, NliEngine, NliLabel, NliRelation, NliResult,
};
pub use scope_classifier::{
    classify_scope, ensure_scope_classifier_loaded, init_scope_classifier,
    is_scope_classifier_loaded, unload_scope_classifier, QueryScopeClassifier,
};
pub use tokenizer::estimate_tokens;

/// Evicts the 3 memory pipeline worker ONNX models (MiniLM embedder, DeBERTa v3 NLI, ModernBERT Edge Classifier).
pub fn unload_memory_pipeline_onnx_models() {
    embedder::unload_embedder();
    nli::unload_nli_engine();
    edge_classifier::unload_edge_classifier();
    trim_heap("MemorySubsystem::unload_memory_pipeline_onnx_models");
    log::info!("[MemorySubsystem] Evicted 3 memory pipeline ONNX models from process memory.");
}

/// Evicts all ONNX models (memory pipeline + query scope classifier + transliteration engine).
pub fn unload_all_onnx_models() {
    unload_memory_pipeline_onnx_models();
    scope_classifier::unload_scope_classifier();
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
