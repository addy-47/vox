use std::sync::{
    atomic::{AtomicU32, Ordering::Relaxed},
    mpsc, Arc,
};

use crate::{
    core::{
        events::AudioIntent,
        state::{AppState, InteractionState},
    },
    pipeline::router::{transition, RoutingContext},
    services::tts::actor::TtsCommand,
};

/// Trigger that initiated an intermediate, non-terminal conversational phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonTerminalTrigger {
    Compaction,
    NonTerminalTool {
        tool_name: String,
        call_id: String,
    },
}

/// Description of a non-terminal operational phase (compaction, tool execution, etc.).
#[derive(Debug, Clone)]
pub struct NonTerminalPhase {
    pub trigger: NonTerminalTrigger,
    pub filler_phrase: Option<String>,
}

impl NonTerminalPhase {
    /// Constructs a non-terminal phase for inline memory compaction.
    pub fn compaction(filler_phrase: Option<String>) -> Self {
        Self {
            trigger: NonTerminalTrigger::Compaction,
            filler_phrase,
        }
    }

    /// Constructs a non-terminal phase for a non-terminal tool invocation.
    pub fn tool(
        tool_name: impl Into<String>,
        call_id: impl Into<String>,
        filler_phrase: Option<String>,
    ) -> Self {
        Self {
            trigger: NonTerminalTrigger::NonTerminalTool {
                tool_name: tool_name.into(),
                call_id: call_id.into(),
            },
            filler_phrase,
        }
    }
}

/// Bundled handles and contextual metadata for entering a non-terminal operational phase.
pub struct NonTerminalContext<'a, R: tauri::Runtime> {
    pub query: &'a str,
    pub turn_id: u32,
    pub routing_ctx: &'a RoutingContext,
    pub app: &'a tauri::AppHandle<R>,
    pub app_state: &'a Arc<AppState>,
    pub tts_tx: Option<&'a mpsc::Sender<TtsCommand>>,
    pub pending_synthesis_jobs: &'a Arc<AtomicU32>,
}

/// Uniformly transitions pipeline to Working and dispatches interim filler audio.
pub fn enter_non_terminal_phase<R: tauri::Runtime>(
    phase: &NonTerminalPhase,
    ctx: &NonTerminalContext<'_, R>,
) {
    transition(
        InteractionState::Working,
        ctx.routing_ctx,
        ctx.app,
        ctx.app_state,
    );

    let filler = match &phase.filler_phrase {
        Some(phrase) if !phrase.trim().is_empty() => phrase.clone(),
        _ => super::super::select_filler_phrase(ctx.query, ctx.turn_id).to_string(),
    };

    if let Some(tts_tx) = ctx.tts_tx {
        ctx.pending_synthesis_jobs.fetch_add(1, Relaxed);
        if let Err(e) = tts_tx.send(TtsCommand::Generate {
            turn_id: ctx.turn_id,
            text: filler,
            intent: AudioIntent::InterimFiller,
        }) {
            log::warn!("[Phase] Failed to dispatch interim filler TTS: {}", e);
            ctx.pending_synthesis_jobs.fetch_sub(1, Relaxed);
        }
    }
}
