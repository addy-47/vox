pub mod embedder;
pub mod tokenizer;

pub use embedder::{
    cosine_similarity, embedding_dim, ensure_embedder_loaded, generate_embedding,
    generate_embeddings_batch, init_embedder, is_embedder_loaded, l2_normalize_in_place,
    unload_embedder, TextEmbedder,
};
pub use tokenizer::{estimate_tokens, warmup_tokenizer};

use crate::services::translit::unload_transliteration_engine;

/// Evicts the memory pipeline embedder ONNX model from process memory.
pub fn unload_memory_pipeline_onnx_models() {
    embedder::unload_embedder();
    trim_heap("MemorySubsystem::unload_memory_pipeline_onnx_models");
    log::info!("[MemorySubsystem] Evicted memory pipeline ONNX model from process memory.");
}

/// Evicts all ONNX models (memory pipeline embedder + transliteration engine).
pub fn unload_all_onnx_models() {
    unload_memory_pipeline_onnx_models();
    unload_transliteration_engine();
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
