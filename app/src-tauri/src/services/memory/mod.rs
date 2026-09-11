use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Arc,
};

use crate::core::state::{AppState, InteractionState};

/// Shared runtime state for the memory subsystem.
pub struct MemoryAppState {
    pub graph_version: Arc<AtomicU64>,
    pub user_paused_ingestion: Arc<AtomicBool>,
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
    tokenizer::estimate_tokens,
    unload_all_onnx_models, unload_memory_pipeline_onnx_models,
};
pub use personal::{consolidate_personal_memory, export_personal_memory, import_personal_memory};

pub use crate::core::error::MemoryError;

pub const QUIET_INGESTION_DEBOUNCE_SECS: u64 = 30;
pub const COMPACTION_SENTINEL_TURN_ID: u32 = 999_999;

/// Spawns a background observer task that watches for sustained 30-second quiet periods in {Ready, Paused}
/// and executes an ingestion deduplication cycle on the database.
pub fn spawn_quiet_ingestion_observer(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut state_rx = state.pipeline.state_rx.clone();
        let db = state.db.clone();
        log::info!("[Memory::Ingestion] Quiet idle observer spawned.");

        loop {
            let current = *state_rx.borrow_and_update();
            if current == InteractionState::Ready || current == InteractionState::Paused {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(QUIET_INGESTION_DEBOUNCE_SECS)) => {
                        let latest = state.pipeline.state();
                        if latest == InteractionState::Ready || latest == InteractionState::Paused {
                            log::info!("[Memory::Ingestion] 30s sustained quiet state reached. Running ingestion deduplication cycle.");
                            if let Err(e) = ingestion::run_ingestion_cycle(&db).await {
                                log::warn!("[Memory::Ingestion] Background ingestion cycle error: {}", e);
                            }
                        }
                    }
                    res = state_rx.changed() => {
                        if res.is_err() {
                            break;
                        }
                    }
                }
            } else if state_rx.changed().await.is_err() {
                break;
            }
        }
    });
}
