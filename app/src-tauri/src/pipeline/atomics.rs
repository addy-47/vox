use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
};

use crate::core::{events::ActivityEnvelope, state::InteractionState};

/// Centralized atomic primitives, watch channels, and token lifecycles for pipeline execution.
pub struct PipelineAtomics {
    pub turn_id: Arc<AtomicU32>,
    pub utterance_id: Arc<AtomicU32>,
    pub transcript_history: Arc<parking_lot::Mutex<VecDeque<String>>>,
    pub playback_underruns: Arc<AtomicU64>,
    pub pending_tts_jobs: Arc<AtomicU32>,
    pub sink_occupied: Arc<AtomicBool>,
    pub assistant_state_atomic: Arc<AtomicU32>,
    pub state_tx: tokio::sync::watch::Sender<InteractionState>,
    pub state_rx: tokio::sync::watch::Receiver<InteractionState>,
    pub dictation_state_atomic: Arc<AtomicU32>,
    pub ingestion_gate: Arc<AtomicBool>,
    pub is_playback_muted: Arc<AtomicBool>,
    pub is_mic_muted: Arc<AtomicBool>,
    pub turn_cancel_token: Arc<parking_lot::Mutex<tokio_util::sync::CancellationToken>>,
    pub stream_cancel_token: Arc<parking_lot::Mutex<tokio_util::sync::CancellationToken>>,
    pub hardware_cancel_flag: Arc<AtomicBool>,
    pub llm_done: Arc<AtomicBool>,
    pub engine_shutdown: Arc<AtomicBool>,
    pub current_activity: Arc<parking_lot::RwLock<Option<ActivityEnvelope>>>,
}

impl Default for PipelineAtomics {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineAtomics {
    /// Initializes all pipeline atomics, watch channels, and cancellation tokens to idle defaults.
    pub fn new() -> Self {
        let (state_tx, state_rx) = tokio::sync::watch::channel(InteractionState::Idle);

        Self {
            turn_id: Arc::new(AtomicU32::new(0)),
            utterance_id: Arc::new(AtomicU32::new(0)),
            transcript_history: Arc::new(parking_lot::Mutex::new(VecDeque::new())),
            playback_underruns: Arc::new(AtomicU64::new(0)),
            pending_tts_jobs: Arc::new(AtomicU32::new(0)),
            sink_occupied: Arc::new(AtomicBool::new(false)),
            assistant_state_atomic: Arc::new(AtomicU32::new(InteractionState::Idle as u32)),
            state_tx,
            state_rx,
            dictation_state_atomic: Arc::new(AtomicU32::new(InteractionState::Idle as u32)),
            ingestion_gate: Arc::new(AtomicBool::new(false)),
            is_playback_muted: Arc::new(AtomicBool::new(false)),
            is_mic_muted: Arc::new(AtomicBool::new(false)),
            turn_cancel_token: Arc::new(parking_lot::Mutex::new(
                tokio_util::sync::CancellationToken::new(),
            )),
            stream_cancel_token: Arc::new(parking_lot::Mutex::new(
                tokio_util::sync::CancellationToken::new(),
            )),
            hardware_cancel_flag: Arc::new(AtomicBool::new(false)),
            llm_done: Arc::new(AtomicBool::new(false)),
            engine_shutdown: Arc::new(AtomicBool::new(false)),
            current_activity: Arc::new(parking_lot::RwLock::new(None)),
        }
    }

    /// Recomputes the lock-free audio ingestion gate based on dual-track states and mic mute.
    /// Invariant: Gate Is Open <=> !is_mic_muted && ((assistant in {Ready, Listening, Thinking, Speaking, Working}) || (dictation in {Ready, Listening, Thinking}))
    pub fn update_ingestion_gate(&self) {
        if self.is_mic_muted.load(Ordering::Relaxed) {
            self.ingestion_gate.store(false, Ordering::Relaxed);
            return;
        }

        let a = InteractionState::from(self.assistant_state_atomic.load(Ordering::Relaxed));
        let d = InteractionState::from(self.dictation_state_atomic.load(Ordering::Relaxed));
        let open = matches!(
            a,
            InteractionState::Ready
                | InteractionState::Listening
                | InteractionState::Thinking
                | InteractionState::Speaking
                | InteractionState::Working
        ) || matches!(
            d,
            InteractionState::Ready | InteractionState::Listening | InteractionState::Thinking
        );
        self.ingestion_gate.store(open, Ordering::Relaxed);
    }

    /// Returns the current interaction state derived from the canonical atomic state.
    pub fn state(&self) -> InteractionState {
        InteractionState::from(self.assistant_state_atomic.load(Ordering::SeqCst))
    }

    /// Updates internal interaction state atomics and notifies all observers.
    pub fn set_state(&self, new_state: InteractionState) {
        self.assistant_state_atomic
            .store(new_state as u32, Ordering::SeqCst);
        self.update_ingestion_gate();
        if let Err(e) = self.state_tx.send(new_state) {
            log::warn!(
                "[Pipeline::State] Failed to broadcast state to observers: {}",
                e
            );
        }
    }

    /// Subscribes to the broadcast interaction state channel for multi-consumer fanout.
    pub fn subscribe_state(&self) -> tokio::sync::watch::Receiver<InteractionState> {
        self.state_tx.subscribe()
    }

    /// Returns the current dictation state derived from the canonical atomic state.
    pub fn dictation_state(&self) -> InteractionState {
        InteractionState::from(self.dictation_state_atomic.load(Ordering::SeqCst))
    }

    /// Updates internal dictation state atomics and notifies all observers.
    pub fn set_dictation_state(&self, new_state: InteractionState) {
        self.dictation_state_atomic
            .store(new_state as u32, Ordering::SeqCst);
        self.update_ingestion_gate();
    }

    /// Returns a clone of the current turn's cancellation token.
    pub fn turn_cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.turn_cancel_token.lock().clone()
    }

    /// Cancels the active turn and latches the hardware cancel flag (RULE-EVT-09 §4).
    pub fn cancel_active_turn(&self) {
        self.hardware_cancel_flag.store(true, Ordering::Release);
        self.turn_cancel_token.lock().cancel();
    }

    /// Rearms the hardware flag and turns a fresh cancellation token without a new turn ID.
    pub fn rearm_turn_cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.rearm_hardware_cancel_flag();
        let mut guard = self.turn_cancel_token.lock();
        guard.cancel();
        let new_token = tokio_util::sync::CancellationToken::new();
        *guard = new_token.clone();
        new_token
    }

    /// Clears the hardware cancel flag on a false-barge-in return to an active state.
    pub fn rearm_hardware_cancel_flag(&self) {
        self.hardware_cancel_flag.store(false, Ordering::Release);
    }

    /// Returns a clone of the active stream cancellation token.
    pub fn stream_cancel_token(&self) -> tokio_util::sync::CancellationToken {
        self.stream_cancel_token.lock().clone()
    }

    /// Cancels the active LLM stream pass without cancelling the turn (RULE-EVT-08 §1b).
    pub fn cancel_active_stream(&self) {
        self.stream_cancel_token.lock().cancel();
    }

    /// Rearms the stream cancellation token and returns the fresh clone.
    pub fn rearm_stream_cancel_token(&self) -> tokio_util::sync::CancellationToken {
        let mut guard = self.stream_cancel_token.lock();
        guard.cancel();
        let new_token = tokio_util::sync::CancellationToken::new();
        *guard = new_token.clone();
        new_token
    }

    /// Sets `llm_done` for the active turn only; stale epochs are ignored (RULE-EVT-09 §4).
    pub fn mark_llm_finished(&self, turn_id: u32) -> bool {
        if self.peek_turn_id() != turn_id {
            return false;
        }
        self.llm_done.store(true, Ordering::Release);
        true
    }

    /// Returns true when the active turn's LLM generation has completed.
    pub fn is_llm_done(&self) -> bool {
        self.llm_done.load(Ordering::Acquire)
    }

    /// Atomically allocates the next monotonic turn ID.
    pub fn next_turn_id(&self) -> u32 {
        self.turn_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Returns the current turn ID without incrementing.
    pub fn peek_turn_id(&self) -> u32 {
        self.turn_id.load(Ordering::Relaxed)
    }

    /// Atomically allocates the next monotonic audio utterance ID.
    pub fn next_utterance_id(&self) -> u32 {
        self.utterance_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Returns the current utterance ID without incrementing.
    pub fn peek_utterance_id(&self) -> u32 {
        self.utterance_id.load(Ordering::Relaxed)
    }

    /// Allocates a new turn, clearing per-turn latches and rearming both tokens.
    pub fn next_turn(&self) -> (u32, tokio_util::sync::CancellationToken) {
        let id = self.next_turn_id();
        self.llm_done.store(false, Ordering::Release);
        self.rearm_hardware_cancel_flag();
        let tok = self.rearm_turn_cancel_token();
        self.rearm_stream_cancel_token();
        (id, tok)
    }

    /// Returns the currently active non-terminal activity envelope, if any.
    pub fn current_activity(&self) -> Option<ActivityEnvelope> {
        self.current_activity.read().clone()
    }

    /// Sets the currently active non-terminal activity envelope.
    pub fn set_current_activity(&self, activity: Option<&ActivityEnvelope>) {
        *self.current_activity.write() = activity.cloned();
    }
}
