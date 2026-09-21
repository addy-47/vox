use std::sync::{atomic::AtomicU32, mpsc, Arc};

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    core::{
        events::VoxEvent,
        state::{AppState, InteractionOwner},
    },
    persistence::db::VoxDb,
    pipeline::{assistant::accumulator::TurnAccumulator, router::RoutingContext},
    services::{
        llm::{actor::LlmCommand, LlmProvider},
        tts::actor::TtsCommand,
    },
};

pub mod chassis;
pub mod compaction;
pub mod finalize;
pub mod loop_driver;
pub mod phase;
pub mod staging;
pub mod tools;

pub use chassis::Harness;
pub use finalize::handle_turn_cancelled;
pub use phase::{NonTerminalContext, NonTerminalPhase, NonTerminalTrigger};

use compaction::execute_inline_compaction;
use loop_driver::run_cognitive_loop;
use staging::{stage_turn_intake, IntakeResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineDomain {
    ModularAssistant,
    RealtimeS2S,
    Dictation,
}

/// Strongly typed terminal outcome of executing a conversational turn.
#[derive(Debug)]
pub enum TurnOutcome {
    Completed {
        turn_id: u32,
        assistant_response: String,
    },
    DuplicateIgnored {
        turn_id: u32,
    },
    Cancelled {
        turn_id: u32,
    },
    Error {
        turn_id: u32,
        message: String,
    },
}

/// Bundled parameters and subsystem handles for executing a turn through the harness.
pub struct TurnExecutionRequest<R: tauri::Runtime> {
    pub query: String,
    pub turn_id: u32,
    pub cancel: CancellationToken,
    pub owner: InteractionOwner,
    pub llm_tx: Option<mpsc::Sender<LlmCommand>>,
    pub tts_tx: Option<mpsc::Sender<TtsCommand>>,
    pub provider: Option<Arc<dyn LlmProvider>>,
    pub db: Arc<VoxDb>,
    pub pipeline_tx: Option<mpsc::Sender<VoxEvent>>,
    pub accumulator: Arc<Mutex<TurnAccumulator>>,
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    pub app: tauri::AppHandle<R>,
    pub routing_ctx: RoutingContext,
    pub app_state: Arc<AppState>,
}

/// Primary orchestrator entry point: coordinates the 7-phase conversational turn lifecycle.
pub async fn execute_turn<R: tauri::Runtime + 'static>(
    harness_arc: &Arc<Mutex<Option<Harness>>>,
    req: TurnExecutionRequest<R>,
) -> TurnOutcome {
    let turn_id = req.turn_id;

    // Phase 1 & 2: Intake deduplication and token budget evaluation
    let (can_compact, stream_stage) = match stage_turn_intake(harness_arc, &req) {
        IntakeResult::Proceed {
            can_compact,
            stream_stage,
        } => (can_compact, stream_stage),
        IntakeResult::Terminal(outcome) => return outcome,
    };

    // Phase 3: Generic Non-Terminal Phase (Inline compaction & maintenance)
    if can_compact {
        execute_inline_compaction(harness_arc, &req).await;
    }

    // Phase 4, 5, 6, 7: Reentrant generation, streaming, tool interception, and commit
    run_cognitive_loop(harness_arc, req, stream_stage, turn_id).await
}
