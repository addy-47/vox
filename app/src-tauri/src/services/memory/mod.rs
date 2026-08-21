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
///
/// Platform behavior:
/// - **Linux:**  `malloc_trim(0)` — returns free glibc arena pages immediately.
/// - **Windows:** `EmptyWorkingSet` — moves freed pages from the process working set
///   to the OS standby list, reducing physical RAM usage without extra allocations.
/// - **macOS:**  No stable public trim API. Apple's libmalloc returns pages to the kernel
///   autonomously under memory pressure. A no-op here is correct and safe.
///   See: https://github.com/apple-oss-distributions/libmalloc (pressure_relief is internal).
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
        // EmptyWorkingSet via raw FFI — zero new crate dependencies.
        // Declared here rather than globally to keep the unsafe surface minimal.
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
        // macOS: libmalloc has no stable public trim API (malloc_zone_pressure_relief is
        // an internal symbol and must not be called externally). The OS allocator releases
        // pages to the kernel on its own schedule when memory pressure is detected.
        // PLATFORM_LIMITATION: no explicit trim possible on macOS without private API.
        log::debug!(
            "[Heap] trim_heap no-op on macOS (called from {}). OS allocator self-manages.",
            caller
        );
        let _ = caller;
    }
}
