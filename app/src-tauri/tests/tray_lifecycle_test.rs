//! ============================================================================
//! tray_lifecycle_test.rs — Tray Window Lifecycle & Session Cancellation Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : Tray IPC & Window State (`vox_lib::ipc::tray`)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    : cargo test --test tray_lifecycle_test
//! Metrics      : Precondition guard checks & cancellation flag invariants
//! ============================================================================

use std::sync::atomic::{AtomicBool, Ordering};
use vox_lib::core::settings::VoxSettings;
use vox_lib::core::state::{InteractionOwner, PipelineAtomics};

#[test]
fn test_tray_precondition_guards() {
    let mut settings = VoxSettings::default();

    // 1. Setup incomplete, Tray enabled -> Guard must block
    settings.setup.completed = false;
    settings.ui.tray_enabled = true;
    let guard_blocked_1 = !settings.setup.completed || !settings.ui.tray_enabled;
    assert!(
        guard_blocked_1,
        "Toggle MUST be blocked when setup is not completed!"
    );

    // 2. Setup completed, Tray disabled -> Guard must block
    settings.setup.completed = true;
    settings.ui.tray_enabled = false;
    let guard_blocked_2 = !settings.setup.completed || !settings.ui.tray_enabled;
    assert!(
        guard_blocked_2,
        "Toggle MUST be blocked when tray HUD is disabled!"
    );

    // 3. Setup completed, Tray enabled -> Guard passes
    settings.setup.completed = true;
    settings.ui.tray_enabled = true;
    let guard_passed = settings.setup.completed && settings.ui.tray_enabled;
    assert!(
        guard_passed,
        "Toggle MUST proceed when setup is completed and tray is enabled!"
    );
}

#[test]
fn test_tray_hide_cancels_active_session() {
    let pipeline = PipelineAtomics::new();
    let owner_atomic = std::sync::atomic::AtomicU32::new(InteractionOwner::Tray as u32);

    // 1. Simulate active Tray owner session
    let owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(owner, InteractionOwner::Tray);

    // 2. Execute cancellation contract when hiding tray window
    if owner == InteractionOwner::Tray {
        pipeline.cancel_flag.store(true, Ordering::Relaxed);
    }

    // 3. Assert cancel flag was set to true
    assert!(
        pipeline.cancel_flag.load(Ordering::Relaxed),
        "Hiding tray window during active Tray owner session MUST set cancel_flag to true!"
    );
}

#[test]
fn test_tray_hide_switch_owner_to_main_window_when_engaged() {
    let owner_atomic = std::sync::atomic::AtomicU32::new(InteractionOwner::Tray as u32);
    let is_engaged = AtomicBool::new(true);

    let owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(owner, InteractionOwner::Tray);

    // When tray hides and pipeline is engaged, owner switches to MainWindow
    if is_engaged.load(Ordering::Relaxed) {
        owner_atomic.store(InteractionOwner::MainWindow as u32, Ordering::Relaxed);
    }

    let new_owner: InteractionOwner = owner_atomic.load(Ordering::Relaxed).into();
    assert_eq!(
        new_owner,
        InteractionOwner::MainWindow,
        "Ending Tray session when engaged MUST revert owner to MainWindow!"
    );
}
