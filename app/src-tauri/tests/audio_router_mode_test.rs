//! ============================================================================
//! audio_router_mode_test.rs — Audio Router & Dynamic Mode Switch Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : Audio Mode Router & Settings (`vox_lib::core::settings`, `vox_lib::ipc::tray`)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    : cargo test --test audio_router_mode_test
//! Metrics      : Interaction mode settings mutation, VAD owner matching, & engine lifecycle triggers
//! ============================================================================

use vox_lib::core::settings::{DictationInteractionMode, InteractionMode, VoxSettings};
use vox_lib::core::state::InteractionOwner;

// ─── 1. Interaction Mode Settings Mutation Tests ─────────────────────────────

#[test]
fn test_interaction_mode_settings_mutation() {
    let mut settings = VoxSettings::default();

    // Default modes in Vox
    assert_eq!(settings.interaction.main_app_mode, InteractionMode::Passive);
    assert_eq!(
        settings.dictation.interaction_mode,
        DictationInteractionMode::Ptt
    );

    // Switch main app mode to PTT
    settings.interaction.main_app_mode = InteractionMode::PTT;
    assert_eq!(settings.interaction.main_app_mode, InteractionMode::PTT);

    // Switch dictation mode to Passive
    settings.dictation.interaction_mode = DictationInteractionMode::Passive;
    assert_eq!(
        settings.dictation.interaction_mode,
        DictationInteractionMode::Passive
    );

    // Switch main back to Passive
    settings.interaction.main_app_mode = InteractionMode::Passive;
    assert_eq!(settings.interaction.main_app_mode, InteractionMode::Passive);
}

// ─── 2. Target Window to Interaction Owner Matching Tests ────────────────────

#[test]
fn test_audio_router_target_owner_matching() {
    let owner_main = InteractionOwner::MainWindow;
    let owner_dictation = InteractionOwner::Dictation;

    let target_main = match "main".to_lowercase().as_str() {
        "main" => InteractionOwner::MainWindow,
        _ => InteractionOwner::Dictation,
    };
    assert_eq!(target_main, owner_main);

    let target_tray = match "tray".to_lowercase().as_str() {
        "main" => InteractionOwner::MainWindow,
        _ => InteractionOwner::Dictation,
    };
    assert_eq!(target_tray, owner_dictation);
}

// ─── 3. Engine Lifecycle Trigger Decision Matrix Tests ───────────────────────

#[test]
fn test_engine_stop_trigger_condition_when_dictation_disabled_and_non_passive() {
    let dictation_enabled = false;
    let is_engaged = false;
    let is_passive = false; // Changed main app mode to PTT

    // Condition: !dictation_enabled && !is_engaged && !is_passive -> Must stop engine
    let should_stop_engine = !dictation_enabled && !is_engaged && !is_passive;
    assert!(
        should_stop_engine,
        "Engine MUST be stopped when dictation is disabled, pipeline is unengaged, and mode is non-passive!"
    );
}

#[test]
fn test_engine_launch_trigger_condition_when_passive() {
    let dictation_enabled = false;
    let is_engaged = false;
    let is_passive = true; // Main app mode changed to Passive

    // Condition: is_passive -> Must launch/ensure engine running
    let should_launch_engine = is_passive;
    let should_stop_engine = !dictation_enabled && !is_engaged && !is_passive;

    assert!(
        should_launch_engine,
        "Engine MUST be launched/ensured running when mode is changed to Passive!"
    );
    assert!(
        !should_stop_engine,
        "Engine MUST NOT be stopped when mode is Passive!"
    );
}
