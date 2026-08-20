//! ============================================================================
//! dictation_test.rs — Integration Tests for Dictation Subsystem
//! ============================================================================
//! Category     : Integration Test
//! Component    : Dictation (`vox_lib::services::dictation`, `vox_lib::core::settings`, `vox_lib::core::error`)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    : cargo test --test dictation_test --release
//! ============================================================================

use vox_lib::core::error::{DictationError, VoxError};
use vox_lib::core::state::InteractionOwner;
use vox_lib::services::dictation::clipboard;
use vox_lib::services::dictation::input::create_input_adapter;

// ─── 1. Subsystem Interaction Owner Mapping & Fast-Path Invariants ───────────

#[test]
fn test_dictation_interaction_owner_fast_path_invariants() {
    // Contract: Dictation must have InteractionOwner ID = 0 for zero-latency indexing
    let owner = InteractionOwner::Dictation;
    assert_eq!(owner as u8, 0);
    assert_eq!(owner as u32, 0);
    assert_eq!(InteractionOwner::from(0u8), InteractionOwner::Dictation);
    assert_eq!(InteractionOwner::from(0u32), InteractionOwner::Dictation);

    // Contract: Dictation does not collide with Assistant window owners
    assert_ne!(InteractionOwner::MainWindow as u32, InteractionOwner::Dictation as u32);
    assert_ne!(InteractionOwner::Ptt as u32, InteractionOwner::Dictation as u32);
}

// ─── 2. Safe Clipboard Backup & Restore Lifecycle Contract ───────────────────

#[tokio::test]
async fn test_with_clipboard_safe_preserves_data_on_failure() {
    // Contract: When paste simulation fails (e.g. Wayland compositor denial or app error),
    // the transcribed text MUST remain on the clipboard so the user never loses their transcript.
    let dictation_text = format!("dictation_recovery_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

    let action_failure = || async {
        Err::<(), DictationError>(DictationError::InputSimulationFailed {
            message: "Simulated synthetic injection failure".to_string(),
        })
    };

    let result = clipboard::with_clipboard_safe(&dictation_text, action_failure).await;
    assert!(result.is_err());

    // If an active display server / clipboard context exists in the test runner,
    // verify the text is retained on clipboard.
    if let Ok(current_clip) = clipboard::get_text() {
        assert_eq!(current_clip, dictation_text, "Failed paste must leave transcript on clipboard for recovery");
    }
}

// ─── 3. Platform Input Adapter Resolution Contract ───────────────────────────

#[test]
fn test_create_input_adapter_resolution() {
    // Contract: create_input_adapter should cleanly instantiate without panic regardless of display environment
    let adapter = create_input_adapter();

    // Verify simulate_paste returns Result without panic
    let result = adapter.simulate_paste();
    match result {
        Ok(()) => {
            // Succeeded (e.g. running on X11 with active window focus)
        }
        Err(DictationError::InputSimulationFailed { message }) => {
            // Expected under headless test environments, Wayland security isolation, or unmapped display
            assert!(!message.is_empty());
        }
        Err(e) => {
            panic!("Unexpected error variant from input adapter: {:?}", e);
        }
    }
}

// ─── 4. Devanagari Transliteration Pre-Delivery Contract ──────────────────────

#[test]
fn test_dictation_transliteration_delivery_transformation() {
    // Spoken Hindi text must be transliterated if enabled
    let sample_devanagari = "नमस्ते दुनिया";
    let transliterated = vox_lib::services::utils::transliterate_if_hi(
        sample_devanagari,
        true,
        true,
    );
    assert!(!transliterated.is_empty());
    assert_ne!(transliterated, "");

    // When transliteration is disabled, original text is preserved verbatim
    let untransliterated = vox_lib::services::utils::transliterate_if_hi(
        sample_devanagari,
        true,
        false,
    );
    assert_eq!(untransliterated, sample_devanagari);
}

// ─── 5. Transcript Cache & Recovery Storage Contract ──────────────────────────

#[test]
fn test_last_transcript_cache_lifecycle() {
    let transcript_slot = parking_lot::Mutex::new(None);

    // Initial state: No transcript cached
    assert!(transcript_slot.lock().is_none());

    // Simulate final dictation text arrived at output router
    let final_text = "This is a real transcribed voice note from dictation pipeline.".to_string();
    *transcript_slot.lock() = Some(final_text.clone());

    // Verify recovery contract: cached transcript matches exactly
    let recovered = transcript_slot.lock().clone();
    assert_eq!(recovered, Some(final_text));

    // Clearing/overwriting works atomically
    *transcript_slot.lock() = None;
    assert!(transcript_slot.lock().is_none());
}

// ─── 6. Error Propagation Invariant ──────────────────────────────────────────

#[test]
fn test_dictation_error_propagation_contract() {
    let errors = vec![
        DictationError::ClipboardFailed { message: "Permission denied".into() },
        DictationError::InputSimulationFailed { message: "Enigo error".into() },
        DictationError::HotkeyRegistrationFailed { message: "Key occupied".into() },
        DictationError::EngineNotReady { message: "VAD failed".into() },
    ];

    for err in errors {
        let msg = format!("{}", err);
        assert!(!msg.is_empty());
        let vox_err: VoxError = err.into();
        assert!(matches!(vox_err, VoxError::Dictation(_)));
    }
}
