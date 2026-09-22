use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc,
    },
    time::Duration,
};

use crate::core::state::{AppState, InteractionState};

/// Shared runtime state for the memory subsystem.
pub struct MemoryAppState {
    pub graph_version: Arc<AtomicU64>,
    pub user_paused_ingestion: Arc<AtomicBool>,
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
            graph_version: Arc::new(AtomicU64::new(1)),
            user_paused_ingestion: Arc::new(AtomicBool::new(false)),
            scheduler_handle: parking_lot::Mutex::new(None),
        }
    }
}

pub mod compaction;
pub mod ingestion;
pub mod ml;
pub mod personal;
pub mod scheduler;

pub use compaction::{run_compaction, CompactionResult, COMPACTION_SYSTEM_PROMPT};
pub use ingestion::QueueStatus;
pub(crate) use ml::trim_heap;
pub use ml::{
    embedder::{
        cosine_similarity, ensure_embedder_loaded, generate_embedding, generate_embeddings_batch,
        init_embedder, is_embedder_loaded, PRIMARY_EMBEDDING_MODEL_DIR,
        PRIMARY_EMBEDDING_MODEL_FILENAME,
    },
    tokenizer::{estimate_tokens, warmup_tokenizer},
    unload_all_onnx_models, unload_memory_pipeline_onnx_models,
};
pub use personal::consolidate_personal_memory;
pub use scheduler::{
    check_missed_consolidation_on_boot, spawn_consolidation_scheduler,
    start_consolidation_scheduler, stop_consolidation_scheduler,
};

pub use crate::core::error::MemoryError;

pub const QUIET_INGESTION_DEBOUNCE_SECS: u64 = 30;
pub const COMPACTION_SENTINEL_TURN_ID: u32 = 999_999;

fn is_quiet_state(state: InteractionState) -> bool {
    matches!(
        state,
        InteractionState::Idle
            | InteractionState::Ready
            | InteractionState::Paused
            | InteractionState::Sleeping
    )
}

/// Spawns a background observer task that watches for sustained 30-second quiet periods in {Idle, Ready, Paused, Sleeping}
/// and executes an ingestion deduplication cycle on the database when pending items exist and pipeline processing is enabled.
pub fn spawn_quiet_ingestion_observer(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut state_rx = state.pipeline.state_rx.clone();
        let db = state.db.clone();
        log::info!("[Memory::Ingestion] Quiet idle observer spawned.");

        loop {
            let current = *state_rx.borrow_and_update();
            let is_enabled = state
                .settings
                .read()
                .map(|s| s.personal_memory.pipeline_processing_enabled)
                .unwrap_or(true);

            if is_enabled && is_quiet_state(current) {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(QUIET_INGESTION_DEBOUNCE_SECS)) => {
                        let latest = state.pipeline.state();
                        let still_enabled = state
                            .settings
                            .read()
                            .map(|s| s.personal_memory.pipeline_processing_enabled)
                            .unwrap_or(true);

                        if still_enabled && is_quiet_state(latest) {
                            match db.connect() {
                                Ok(conn) => {
                                    match crate::persistence::has_unfinished_items(&conn).await {
                                        Ok(true) => {
                                            log::info!("[Memory::Ingestion] 30s sustained quiet state reached. Running ingestion deduplication cycle.");
                                            if let Err(e) = ingestion::run_ingestion_cycle(&conn).await {
                                                log::warn!("[Memory::Ingestion] Background ingestion cycle error: {}", e);
                                            }
                                        }
                                        Ok(false) => {
                                            // Queue is quiescent; no deduplication needed.
                                        }
                                        Err(e) => {
                                            log::warn!("[Memory::Ingestion] Failed to check queue status: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("[Memory::Ingestion] Failed to vend connection for ingestion: {}", e);
                                }
                            }
                        }
                    }
                    res = state_rx.changed() => {
                        if res.is_err() {
                            break;
                        }
                    }
                }
            } else {
                tokio::select! {
                    res = state_rx.changed() => {
                        if res.is_err() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {}
                }
            }
        }
    });
}
