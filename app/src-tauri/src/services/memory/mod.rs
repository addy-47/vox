use std::sync::Arc;

use crate::core::state::AppState;

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

pub use compaction::{run_compaction, AttributionFlag, CompactionResult, COMPACTION_SYSTEM_PROMPT, strip_citation};
pub use ingestion::{
    drain_ingestion_queue, reconcile_crashed_queue_on_boot, run_ingestion_cycle,
    DedupDecision, DedupNearMiss, DedupStage, IngestionCycleSummary, QueueStatus,
    NEAR_MISS_REPORT_FLOOR,
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
    consolidate_personal_memory_with_telemetry, regenerate_personal_memory,
    resolve_operations, ConsolidationTelemetry, PersonalMemory, ResolvedOp,
};
pub use scheduler::{
    check_missed_consolidation_on_boot, spawn_consolidation_scheduler,
    start_consolidation_scheduler, stop_consolidation_scheduler,
};

pub use crate::{core::error::MemoryError, persistence::has_unfinished_items};

pub const COMPACTION_SENTINEL_TURN_ID: u32 = 999_999;

pub fn spawn_ingestion_sweep(
    state: Arc<AppState>,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
) {
    let is_enabled = state
        .settings
        .read()
        .map(|s| s.personal_memory.pipeline_processing_enabled)
        .unwrap_or(true);

    if !is_enabled {
        log::debug!(
            "[Memory::Ingestion] Ingestion sweep skipped: pipeline_processing_enabled is false"
        );
        return;
    }

    let token = cancel_token.unwrap_or_default();
    *state.ingestion_cancel.lock() = Some(token.clone());

    let db = state.db.clone();
    let state_arc = Arc::clone(&state);

    tauri::async_runtime::spawn(async move {
        let conn = match db.connect() {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "[Memory::Ingestion] Failed to vend connection for sweep: {}",
                    e
                );
                *state_arc.ingestion_cancel.lock() = None;
                return;
            }
        };

        match has_unfinished_items(&conn).await {
            Ok(true) => {
                log::info!(
                    "[Memory::Ingestion] Unfinished queue items found; starting ingestion sweep."
                );
                if let Err(e) = ingestion::drain_ingestion_queue(&conn, Some(&token)).await {
                    log::warn!("[Memory::Ingestion] Ingestion sweep error: {}", e);
                }
            }
            Ok(false) => {
                log::debug!("[Memory::Ingestion] Queue is quiescent; no sweep needed.");
            }
            Err(e) => {
                log::warn!("[Memory::Ingestion] Failed to check queue status: {}", e);
            }
        }

        *state_arc.ingestion_cancel.lock() = None;
    });
}
