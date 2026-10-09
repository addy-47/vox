pub struct MemoryAppState {
    pub scheduler_handle: parking_lot::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl Default for MemoryAppState {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryAppState {
    pub fn new() -> Self {
        Self {
            scheduler_handle: parking_lot::Mutex::new(None),
        }
    }
}

pub mod compaction;
pub mod ingestion;
pub mod ml;
pub mod personal;
pub mod scheduler;

pub use compaction::{
    run_compaction, strip_citation, AttributionFlag, CompactionResult, COMPACTION_SYSTEM_PROMPT,
};
pub use ingestion::{
    spawn_ingestion_sweep, drain_ingestion_queue, reconcile_crashed_queue_on_boot, run_ingestion_cycle, DedupDecision,
    DedupNearMiss, DedupStage, IngestionCycleSummary, QueueStatus, NEAR_MISS_REPORT_FLOOR,
};
pub(crate) use ml::trim_heap;
pub use ml::{
    embedder::{
        cosine_similarity, ensure_embedder_loaded, generate_embedding, generate_embeddings_batch,
        init_embedder, is_embedder_loaded, PRIMARY_EMBEDDING_MODEL_DIR,
        PRIMARY_EMBEDDING_MODEL_FILENAME,
    },
    estimate_tokens, unload_all_onnx_models, unload_memory_pipeline_onnx_models, warmup_tokenizer,
};
pub use personal::{
    apply_operations, batch_resolve_memory_revisions, consolidate_personal_memory,
    consolidate_personal_memory_with_telemetry, regenerate_personal_memory, resolve_operations,
    ConsolidationTelemetry, PersonalMemory, ResolvedOp,
};
pub use scheduler::{
    check_missed_consolidation_on_boot, spawn_consolidation_scheduler,
    start_consolidation_scheduler, stop_consolidation_scheduler,
};

pub use crate::{core::error::MemoryError, persistence::has_unfinished_items};

pub const COMPACTION_SENTINEL_TURN_ID: u32 = 999_999;
