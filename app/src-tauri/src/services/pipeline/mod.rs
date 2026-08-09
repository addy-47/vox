//! ============================================================================
//! src/services/pipeline/mod.rs — Pipeline Orchestrator module declarations, struct, & re-exports
//! ============================================================================

pub mod event_loop;
pub mod handlers;
pub mod llm_lifecycle;
pub mod tts_lifecycle;
pub mod types;

#[cfg(test)]
mod tests;

pub use types::*;

use crate::core::events::VoxEvent;
use crate::core::settings::VoxSettings;
use crossbeam_channel::Sender;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;
use std::sync::RwLock;

pub struct PipelineOrchestrator {
    pub(crate) cancel_flag: Arc<AtomicBool>,
    pub(crate) is_paused: Arc<AtomicBool>,
    pub(crate) _playback_active: Arc<AtomicBool>,
    pub(crate) tts_generating: Arc<AtomicBool>,
    pub(crate) turn_id: Arc<AtomicU32>,
    pub(crate) state: Arc<Mutex<crate::core::state::InteractionState>>,
    pub(crate) event_tx: std::sync::mpsc::Sender<VoxEvent>,
    pub(crate) settings: Arc<RwLock<VoxSettings>>,
    pub(crate) llm_path: PathBuf,
    pub(crate) super_tts_path: PathBuf,
    pub(crate) is_engaged: Arc<AtomicBool>,
    pub transcript_history: Arc<Mutex<std::collections::VecDeque<String>>>,
    pub conversation_id: Arc<std::sync::atomic::AtomicU64>,
    pub persist_tx: Option<Sender<crate::persistence::events::PersistenceEvent>>,
    pub dropped_persistence_events: Arc<std::sync::atomic::AtomicU64>,

    // Monitoring atomics
    pub latest_voice_latency_ms: Arc<std::sync::atomic::AtomicU32>,
    pub latest_tts_rtf: Arc<std::sync::atomic::AtomicU32>,
    pub latest_playback_start_ms: Arc<std::sync::atomic::AtomicU32>,

    // Lifecycle management
    pub(crate) llm_tx:
        Arc<Mutex<Option<std::sync::mpsc::Sender<crate::services::llm::LlmCommand>>>>,
    pub(crate) tts_tx:
        Arc<Mutex<Option<std::sync::mpsc::Sender<crate::services::tts::TtsCommand>>>>,
    pub llm_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    pub tts_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,

    // Residency Flags
    pub is_llm_loaded: Arc<AtomicBool>,
    pub is_tts_loaded: Arc<AtomicBool>,
    pub is_sleeping: Arc<AtomicBool>,

    // Working Memory
    pub conversation_manager: Arc<Mutex<crate::services::memory::ConversationManager>>,
}

impl PipelineOrchestrator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cancel_flag: Arc<AtomicBool>,
        is_paused: Arc<AtomicBool>,
        playback_active: Arc<AtomicBool>,
        tts_generating: Arc<AtomicBool>,
        turn_id: Arc<AtomicU32>,
        state: Arc<Mutex<crate::core::state::InteractionState>>,
        event_tx: std::sync::mpsc::Sender<VoxEvent>,
        settings: Arc<RwLock<VoxSettings>>,
        llm_path: PathBuf,
        super_tts_path: PathBuf,
        is_engaged: Arc<AtomicBool>,
        transcript_history: Arc<Mutex<std::collections::VecDeque<String>>>,
        conversation_id: Arc<std::sync::atomic::AtomicU64>,
        persist_tx: Option<Sender<crate::persistence::events::PersistenceEvent>>,
        dropped_persistence_events: Arc<std::sync::atomic::AtomicU64>,
        latest_voice_latency_ms: Arc<std::sync::atomic::AtomicU32>,
        latest_tts_rtf: Arc<std::sync::atomic::AtomicU32>,
        latest_playback_start_ms: Arc<std::sync::atomic::AtomicU32>,
        is_llm_loaded: Arc<AtomicBool>,
        is_tts_loaded: Arc<AtomicBool>,
        is_sleeping: Arc<AtomicBool>,
        conversation_manager: Arc<Mutex<crate::services::memory::ConversationManager>>,
    ) -> Self {
        Self {
            cancel_flag,
            is_paused,
            _playback_active: playback_active,
            tts_generating,
            turn_id,
            state,
            event_tx,
            settings,
            llm_path,
            super_tts_path,
            is_engaged,
            transcript_history,
            conversation_id,
            persist_tx,
            dropped_persistence_events,
            latest_voice_latency_ms,
            latest_tts_rtf,
            latest_playback_start_ms,
            llm_tx: Arc::new(Mutex::new(None)),
            tts_tx: Arc::new(Mutex::new(None)),
            llm_handle: Arc::new(Mutex::new(None)),
            tts_handle: Arc::new(Mutex::new(None)),
            is_llm_loaded,
            is_tts_loaded,
            is_sleeping,
            conversation_manager,
        }
    }
}
