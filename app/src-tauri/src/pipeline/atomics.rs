use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc,
    },
};

use crate::core::state::InteractionState;

/// Centralized atomic primitives, watch channels, and token lifecycles for pipeline execution.
pub struct PipelineAtomics {
    pub cancel_flag: Arc<AtomicBool>,
    pub turn_id: Arc<AtomicU32>,
    pub transcript_history: Arc<parking_lot::Mutex<VecDeque<String>>>,
    pub playback_underruns: Arc<AtomicU64>,
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    pub current_state_atomic: Arc<AtomicU32>,
    pub state_tx: tokio::sync::watch::Sender<InteractionState>,
    pub state_rx: tokio::sync::watch::Receiver<InteractionState>,
    pub dictation_state_atomic: Arc<AtomicU32>,
    pub dictation_state_tx: tokio::sync::watch::Sender<InteractionState>,
    pub dictation_state_rx: tokio::sync::watch::Receiver<InteractionState>,
    pub ingestion_gate: Arc<AtomicBool>,
    pub turn_token: Arc<parking_lot::Mutex<tokio_util::sync::CancellationToken>>,
    pub engine_shutdown: Arc<AtomicBool>,
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
        let (dictation_state_tx, dictation_state_rx) =
            tokio::sync::watch::channel(InteractionState::Idle);
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
            turn_id: Arc::new(AtomicU32::new(0)),
            transcript_history: Arc::new(parking_lot::Mutex::new(VecDeque::new())),
            playback_underruns: Arc::new(AtomicU64::new(0)),
            pending_synthesis_jobs: Arc::new(AtomicU32::new(0)),
            current_state_atomic: Arc::new(AtomicU32::new(InteractionState::Idle as u32)),
            state_tx,
            state_rx,
            dictation_state_atomic: Arc::new(AtomicU32::new(InteractionState::Idle as u32)),
            dictation_state_tx,
            dictation_state_rx,
            ingestion_gate: Arc::new(AtomicBool::new(false)),
            turn_token: Arc::new(parking_lot::Mutex::new(
                tokio_util::sync::CancellationToken::new(),
            )),
            engine_shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Recomputes the lock-free audio ingestion gate based on dual-track states.
    /// Invariant: Gate Is Open <=> (assistant in {Ready, Listening, Thinking, Speaking}) || (dictation in {Ready, Listening, Thinking})
    pub fn update_ingestion_gate(&self) {
        let a = InteractionState::from(self.current_state_atomic.load(Ordering::Relaxed));
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
        InteractionState::from(self.current_state_atomic.load(Ordering::SeqCst))
    }

    /// Updates internal interaction state atomics and notifies all observers.
    pub fn set_state(&self, new_state: InteractionState) {
        self.current_state_atomic
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
        if let Err(e) = self.dictation_state_tx.send(new_state) {
            log::warn!(
                "[Pipeline::State] Failed to broadcast dictation state: {}",
                e
            );
        }
    }

    /// Subscribes to the broadcast dictation state channel for multi-consumer fanout.
    pub fn subscribe_dictation_state(&self) -> tokio::sync::watch::Receiver<InteractionState> {
        self.dictation_state_tx.subscribe()
    }

    /// Returns a clone of the current turn's cancellation token.
    pub fn turn_token(&self) -> tokio_util::sync::CancellationToken {
        self.turn_token.lock().clone()
    }

    /// Cancels the active turn's token and returns a fresh cancellation token without allocating a new turn ID.
    /// Use this for session-level re-arming (e.g. on resume or test clip cancellation).
    pub fn rearm_turn_token(&self) -> tokio_util::sync::CancellationToken {
        let mut guard = self.turn_token.lock();
        guard.cancel();
        let new_token = tokio_util::sync::CancellationToken::new();
        *guard = new_token.clone();
        new_token
    }

    /// Atomically allocates the next monotonic turn ID.
    pub fn next_turn_id(&self) -> u32 {
        self.turn_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Returns the current turn ID without incrementing.
    pub fn peek_turn_id(&self) -> u32 {
        self.turn_id.load(Ordering::Relaxed)
    }

    /// Atomically increments turn_id and rotates the per-turn cancellation token.
    pub fn next_turn(&self) -> (u32, tokio_util::sync::CancellationToken) {
        let id = self.next_turn_id();
        let tok = self.rearm_turn_token();
        (id, tok)
    }
}
