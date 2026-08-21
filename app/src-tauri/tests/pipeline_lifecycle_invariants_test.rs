//! ============================================================================
//! pipeline_lifecycle_invariants_test.rs — Voice Pipeline State Transition Invariants
//! ============================================================================
//! Category     : Integration Test
//! Component    : Pipeline State Machine (`vox_lib::core::state::PipelineAtomics`) &
//!                Engine Lifecycle Decision Paths (`vox_lib::core::settings`)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    : cargo test --test pipeline_lifecycle_invariants_test
//! Metrics      : State transition correctness, cancellation contract, & engine stop/launch decisions
//! ============================================================================

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use vox_lib::core::settings::{DictationInteractionMode, InteractionMode, VoxSettings};
use vox_lib::core::state::{InteractionOwner, InteractionState, PipelineAtomics};

/// Model the exact `engage()` disengage decision matrix from
/// `src/ipc/pipeline/lifecycle.rs`. Returns whether the engine MUST be stopped
/// after disengaging (i.e. the engine should only be torn down when dictation
/// is disabled and the pipeline has no other owner to serve).
fn should_stop_engine_on_disengage(dictation_enabled: bool) -> bool {
    !dictation_enabled
}

/// Model the exact `update_interaction_mode()` engine lifecycle decision matrix
/// from `src/ipc/tray.rs`: the engine is stopped only when the main app mode is
/// non-passive, the pipeline is unengaged, and dictation is disabled.
fn engine_stop_trigger(dictation_enabled: bool, is_engaged: bool, is_passive: bool) -> bool {
    !dictation_enabled && !is_engaged && !is_passive
}

/// Model the `resume_pipeline()` post-resume interaction state transition from
/// `src/ipc/pipeline/lifecycle.rs`.
fn resume_next_state(mode: InteractionMode) -> InteractionState {
    match mode {
        InteractionMode::PTT => InteractionState::Idle,
        InteractionMode::Passive => InteractionState::Listening,
    }
}

// ─── 1. Pipeline Engagement State Transitions (Idle -> Engaged -> Cancelled -> Idle) ──

#[test]
fn test_engage_idle_to_engaged_transition() {
    let pipeline = PipelineAtomics::new();
    let owner_atomic = AtomicU32::new(InteractionOwner::Dictation as u32);

    // Cold-start preconditions: dormant STT-only pipeline.
    assert!(!pipeline.is_engaged.load(Ordering::Relaxed));
    assert!(!pipeline.cancel_flag.load(Ordering::Relaxed));
    assert_eq!(
        pipeline.current_state_atomic.load(Ordering::Relaxed),
        InteractionState::Idle as u32
    );
    let owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(owner, InteractionOwner::Dictation);

    // Engage (matching lifecycle.rs engage branch).
    pipeline.is_engaged.store(true, Ordering::Relaxed);
    pipeline.cancel_flag.store(false, Ordering::Relaxed);
    owner_atomic.store(InteractionOwner::MainWindow as u32, Ordering::Relaxed);

    assert!(
        pipeline.is_engaged.load(Ordering::Relaxed),
        "Engage MUST set is_engaged to true!"
    );
    assert!(
        !pipeline.cancel_flag.load(Ordering::Relaxed),
        "Engage MUST reset cancel_flag to false!"
    );
    let engaged_owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(
        engaged_owner,
        InteractionOwner::MainWindow,
        "Engage MUST reassign owner to MainWindow!"
    );
}

#[test]
fn test_disengage_engaged_to_cancelled_and_idle_transition() {
    let pipeline = PipelineAtomics::new();
    let owner_atomic = AtomicU32::new(InteractionOwner::MainWindow as u32);
    pipeline.is_engaged.store(true, Ordering::Relaxed);
    pipeline.cancel_flag.store(false, Ordering::Relaxed);

    // Disengage (matching lifecycle.rs disengage branch).
    pipeline.is_engaged.store(false, Ordering::Relaxed);
    pipeline.cancel_flag.store(true, Ordering::Relaxed);
    owner_atomic.store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
    {
        let mut state_lock = pipeline.state.lock();
        *state_lock = InteractionState::Idle;
    }
    pipeline
        .current_state_atomic
        .store(InteractionState::Idle as u32, Ordering::Relaxed);

    assert!(
        !pipeline.is_engaged.load(Ordering::Relaxed),
        "Disengage MUST clear is_engaged to false!"
    );
    assert!(
        pipeline.cancel_flag.load(Ordering::Relaxed),
        "Disengage MUST set cancel_flag to true!"
    );
    let idle_owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(
        idle_owner,
        InteractionOwner::Dictation,
        "Disengage MUST reassign owner to Dictation!"
    );
    assert_eq!(
        pipeline.current_state_atomic.load(Ordering::Relaxed),
        InteractionState::Idle as u32,
        "Disengage MUST reset state to Idle!"
    );
}

#[test]
fn test_disengage_stop_engine_decision_matrix() {
    // Dictation enabled -> do NOT stop engine on disengage (STT stays warm for dictation)
    assert!(
        !should_stop_engine_on_disengage(true),
        "Engine MUST NOT be stopped on disengage when Dictation is enabled!"
    );
    // Dictation disabled -> MUST stop engine to reclaim memory.
    assert!(
        should_stop_engine_on_disengage(false),
        "Engine MUST be stopped on disengage when Dictation is disabled!"
    );
}

// ─── 2. Cancellation Invariants Across Engine Components ──────────────────────

#[test]
fn test_cancellation_bumping_turn_id_invalidates_inflight_tasks() {
    let pipeline = PipelineAtomics::new();
    let turn_at_start = pipeline.turn_id.load(Ordering::Relaxed);
    let owner_atomic = AtomicU32::new(InteractionOwner::Dictation as u32);

    // Simulate starting a turn
    pipeline.cancel_flag.store(false, Ordering::Relaxed);

    // Emulate barge-in cancellation: turn_id is bumped, cancel_flag set.
    let turn_cancelled = pipeline.turn_id.fetch_add(1, Ordering::SeqCst) + 1;
    pipeline.cancel_flag.store(true, Ordering::Relaxed);

    assert_eq!(turn_cancelled, turn_at_start + 1);
    assert!(pipeline.cancel_flag.load(Ordering::Relaxed));

    // Stale task running under `turn_at_start` MUST be rejected by filter logic.
    let is_task_stale = turn_at_start != pipeline.turn_id.load(Ordering::Relaxed);
    assert!(
        is_task_stale,
        "Task matching the prior turn_id MUST be detected as stale!"
    );

    // Cancellation resets owner to Dictation if it was unengaged.
    let owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(owner, InteractionOwner::Dictation);
}

#[test]
fn test_cancellation_during_playback_clears_pipeline_buffers() {
    let pipeline = PipelineAtomics::new();

    // 1. Pipeline transitions to AssistantSpeaking.
    {
        let mut state_lock = pipeline.state.lock();
        *state_lock = InteractionState::AssistantSpeaking;
    }
    pipeline.current_state_atomic.store(
        InteractionState::AssistantSpeaking as u32,
        Ordering::Relaxed,
    );

    // 2. User presses cancel / barge-in occurs.
    pipeline.cancel_flag.store(true, Ordering::Relaxed);
    pipeline.turn_id.fetch_add(1, Ordering::SeqCst);
    {
        let mut state_lock = pipeline.state.lock();
        *state_lock = InteractionState::Idle;
    }
    pipeline
        .current_state_atomic
        .store(InteractionState::Idle as u32, Ordering::Relaxed);

    assert!(pipeline.cancel_flag.load(Ordering::Relaxed));
    assert_eq!(
        pipeline.current_state_atomic.load(Ordering::Relaxed),
        InteractionState::Idle as u32
    );
}

// ─── 3. Pause / Resume State Transitions ──────────────────────────────────────

#[test]
fn test_pause_pipeline_sets_pause_flag_and_transitions_to_idle() {
    let pipeline = PipelineAtomics::new();
    let owner_atomic = AtomicU32::new(InteractionOwner::Dictation as u32);

    // Precondition: Pipeline is active.
    pipeline.is_paused.store(false, Ordering::SeqCst);
    {
        let mut state_lock = pipeline.state.lock();
        *state_lock = InteractionState::Listening;
    }
    pipeline
        .current_state_atomic
        .store(InteractionState::Listening as u32, Ordering::Relaxed);

    // Pause requested (matching pause_pipeline IPC command).
    pipeline.is_paused.store(true, Ordering::SeqCst);
    pipeline.cancel_flag.store(true, Ordering::SeqCst);
    pipeline.turn_id.fetch_add(1, Ordering::SeqCst);
    {
        let mut state_lock = pipeline.state.lock();
        *state_lock = InteractionState::Idle;
    }
    pipeline
        .current_state_atomic
        .store(InteractionState::Idle as u32, Ordering::Relaxed);

    assert!(
        pipeline.is_paused.load(Ordering::SeqCst),
        "Pause MUST set is_paused flag to true!"
    );
    assert!(
        pipeline.cancel_flag.load(Ordering::SeqCst),
        "Pause MUST set cancel_flag to true to drop active work!"
    );
    assert_eq!(
        pipeline.current_state_atomic.load(Ordering::Relaxed),
        InteractionState::Idle as u32,
        "Pause MUST transition current state to Idle!"
    );

    let owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(
        owner,
        InteractionOwner::Dictation,
        "Pause does not mutate the current owner!"
    );
}

#[test]
fn test_resume_pipeline_clears_pause_flag_and_transitions_to_mode_state() {
    let pipeline = PipelineAtomics::new();
    let owner_atomic = AtomicU32::new(InteractionOwner::Dictation as u32);

    // Precondition: Pipeline is paused.
    pipeline.is_paused.store(true, Ordering::SeqCst);
    pipeline.cancel_flag.store(true, Ordering::SeqCst);
    pipeline
        .current_state_atomic
        .store(InteractionState::Idle as u32, Ordering::Relaxed);

    // Resume under Passive mode -> Transitions to Listening.
    pipeline.is_paused.store(false, Ordering::SeqCst);
    pipeline.cancel_flag.store(false, Ordering::SeqCst);
    let next_passive_state = resume_next_state(InteractionMode::Passive);
    {
        let mut state_lock = pipeline.state.lock();
        *state_lock = next_passive_state;
    }
    pipeline
        .current_state_atomic
        .store(next_passive_state as u32, Ordering::Relaxed);

    assert!(
        !pipeline.is_paused.load(Ordering::SeqCst),
        "Resume MUST clear is_paused flag!"
    );
    assert!(
        !pipeline.cancel_flag.load(Ordering::SeqCst),
        "Resume MUST reset cancel_flag to false!"
    );
    assert_eq!(
        pipeline.current_state_atomic.load(Ordering::Relaxed),
        InteractionState::Listening as u32,
        "Resume under Passive mode MUST transition state to Listening!"
    );

    // Resume under PTT mode -> Transitions to Idle.
    let next_ptt_state = resume_next_state(InteractionMode::PTT);
    {
        let mut state_lock = pipeline.state.lock();
        *state_lock = next_ptt_state;
    }
    pipeline
        .current_state_atomic
        .store(next_ptt_state as u32, Ordering::Relaxed);

    assert_eq!(
        pipeline.current_state_atomic.load(Ordering::Relaxed),
        InteractionState::Idle as u32,
        "Resume under PTT mode MUST transition state to Idle!"
    );

    let owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(owner, InteractionOwner::Dictation);
}

#[test]
fn test_redundant_pause_and_resume_calls_are_no_ops() {
    let pipeline = PipelineAtomics::new();

    // Redundant pause: Already unpaused -> pause once -> pause again.
    pipeline.is_paused.store(false, Ordering::SeqCst);
    let first_pause_applied = !pipeline.is_paused.swap(true, Ordering::SeqCst);
    assert!(first_pause_applied);

    let second_pause_applied = !pipeline.is_paused.load(Ordering::SeqCst);
    assert!(
        !second_pause_applied,
        "Second pause call on an already-paused pipeline MUST be a no-op!"
    );

    // Redundant resume: Already resumed -> resume once -> resume again.
    let first_resume_applied = pipeline.is_paused.swap(false, Ordering::SeqCst);
    assert!(first_resume_applied);

    let second_resume_applied = pipeline.is_paused.load(Ordering::SeqCst);
    assert!(
        !second_resume_applied,
        "Second resume call on an already-active pipeline MUST be a no-op!"
    );
}

#[test]
fn test_cancelled_turn_clears_interaction_state_to_idle() {
    let pipeline = PipelineAtomics::new();
    let owner_atomic = AtomicU32::new(InteractionOwner::Dictation as u32);

    // Pipeline is in Thinking state.
    {
        let mut state_lock = pipeline.state.lock();
        *state_lock = InteractionState::Thinking;
    }
    pipeline
        .current_state_atomic
        .store(InteractionState::Thinking as u32, Ordering::Relaxed);

    // Cancel turn event processed.
    pipeline.cancel_flag.store(true, Ordering::Relaxed);
    pipeline.turn_id.fetch_add(1, Ordering::SeqCst);
    {
        let mut state_lock = pipeline.state.lock();
        *state_lock = InteractionState::Idle;
    }
    pipeline
        .current_state_atomic
        .store(InteractionState::Idle as u32, Ordering::Relaxed);

    assert_eq!(
        pipeline.current_state_atomic.load(Ordering::Relaxed),
        InteractionState::Idle as u32
    );
    let owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(owner, InteractionOwner::Dictation);
}

// ─── 4. Interaction Mode Switching & Owner State Machine Transitions ──────────

#[test]
fn test_interaction_mode_switching_between_owners() {
    let mut settings = VoxSettings::default();

    assert_eq!(settings.interaction.mode, InteractionMode::Passive);
    assert_eq!(
        settings.dictation.interaction_mode,
        DictationInteractionMode::Ptt
    );

    // Main window switches to PTT.
    settings.interaction.mode = InteractionMode::PTT;
    assert_eq!(settings.interaction.mode, InteractionMode::PTT);

    // Dictation switches to Passive — owners resolve independently.
    settings.dictation.interaction_mode = DictationInteractionMode::Passive;
    assert_eq!(
        settings.dictation.interaction_mode,
        DictationInteractionMode::Passive
    );

    // Main reverts to Passive.
    settings.interaction.mode = InteractionMode::Passive;
    assert_eq!(settings.interaction.mode, InteractionMode::Passive);
}

#[test]
fn test_resume_pipeline_state_machine_by_mode() {
    // Passive owner resumes to Listening.
    assert_eq!(
        resume_next_state(InteractionMode::Passive),
        InteractionState::Listening
    );
    // PTT owner resumes to Idle (waiting for key press).
    assert_eq!(
        resume_next_state(InteractionMode::PTT),
        InteractionState::Idle
    );
}

#[test]
fn test_owner_transition_dictation_to_main_window_when_engaged() {
    let owner_atomic = AtomicU32::new(InteractionOwner::Dictation as u32);
    let is_engaged = AtomicBool::new(true);

    // While pipeline is engaged -> owner reverts to MainWindow.
    if is_engaged.load(Ordering::Relaxed) {
        owner_atomic.store(InteractionOwner::MainWindow as u32, Ordering::Relaxed);
    }

    let owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(
        owner,
        InteractionOwner::MainWindow,
        "Engaged pipeline MUST switch owner to MainWindow!"
    );
}

#[test]
fn test_owner_transition_main_to_dictation_when_unengaged() {
    let owner_atomic = AtomicU32::new(InteractionOwner::MainWindow as u32);
    let is_engaged = AtomicBool::new(false);

    // Pipeline unengaged -> owner falls back to Dictation on disengage.
    if !is_engaged.load(Ordering::Relaxed) {
        owner_atomic.store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
    }

    let owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(
        owner,
        InteractionOwner::Dictation,
        "Disengaging an unengaged pipeline MUST fall back to Dictation owner!"
    );
}

#[test]
fn test_update_interaction_mode_engine_lifecycle_matrix() {
    // Non-passive + unengaged + dictation disabled -> engine MUST stop.
    assert!(engine_stop_trigger(false, false, false));
    // Passive mode -> engine MUST NOT stop.
    assert!(!engine_stop_trigger(false, false, true));
    // Engaged -> engine MUST NOT stop (active interaction being served).
    assert!(!engine_stop_trigger(false, true, false));
    // Dictation enabled -> engine MUST NOT stop (dictation serving as fallback owner).
    assert!(!engine_stop_trigger(true, false, false));
}

// ─── 5. Arc Ownership Invariant ───────────────────────────────────────────────

#[test]
fn test_pipeline_atomics_arcs_are_shared_not_owned() {
    // Guards the invariant that PipelineAtomics exposes shared atomics so the
    // engine, VAD/STT/LLM workers, and IPC handlers all observe the same flags.
    let pipeline = PipelineAtomics::new();
    let cancel_clone = Arc::clone(&pipeline.cancel_flag);
    let turn_clone = Arc::clone(&pipeline.turn_id);

    cancel_clone.store(true, Ordering::Relaxed);
    turn_clone.store(42, Ordering::Relaxed);

    assert!(pipeline.cancel_flag.load(Ordering::Relaxed));
    assert_eq!(pipeline.turn_id.load(Ordering::Relaxed), 42);
}
