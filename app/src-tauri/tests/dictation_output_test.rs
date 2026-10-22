//! ============================================================================
//! dictation_output_test.rs — Dictation output routing seam
//! ============================================================================
//! Category  : Integration (Seam 5 — TranscriptFinal → output router → sink)
//! Component : pipeline/dictation/transcript.rs + services/dictation/output_router.rs
//!             + services/dictation/clipboard.rs + services/translit.rs
//! Binary    : dictation_output_test
//! Prereqs   : none. Headless is supported: an unavailable clipboard is a typed
//!             error, and the transcript must survive it.
//! Run       : cargo nextest run --test dictation_output_test --release --test-threads=1
//! ============================================================================
//!
//! WHAT THIS SEAM OWNS
//! The transcript must reach the user and must never be lost. A failed paste or a
//! failed clipboard write must not destroy the text: it stays in the last
//! transcript, in the in-memory history, and in the durable cache history.
//!
//! The OS paste itself cannot be simulated headlessly. The tests therefore assert
//! the *decision* and the *preservation* contract, not the keystrokes.
//!
//! WHY THE SCENARIOS SHARE ONE FUNCTION
//! `TempPathsGuard` and `paths::init_with_root` are process-global. Two test
//! functions in one binary would fight over the same VOX_HOME.

mod common;

use std::{
    fs,
    sync::{mpsc, Arc},
    thread::JoinHandle,
    time::Duration,
};

use tauri::AppHandle;
use vox_lib::{
    core::{
        error::DictationError,
        events::{InteractionOwner, VoxEvent},
        state::{AppState, InteractionState},
    },
    pipeline::dictation::{transition_dictation, DictationOutputMode},
    services::dictation::output_router::route_transcript,
};

type MockApp = AppHandle<tauri::test::MockRuntime>;

const DEADLINE: Duration = Duration::from_secs(10);

/// True when the process has no OS display server, so no clipboard can exist.
fn headless() -> bool {
    std::env::var("DISPLAY").is_err()
        && std::env::var("WAYLAND_DISPLAY").is_err()
        && std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.to_lowercase() != "wayland")
            .unwrap_or(true)
}

struct Rig {
    app: MockApp,
    state: Arc<AppState>,
    event_tx: mpsc::Sender<VoxEvent>,
    router_join: Option<JoinHandle<()>>,
    _paths: common::paths::TempPathsGuard,
}

impl Rig {
    async fn new() -> Self {
        let paths = common::paths::TempPathsGuard::new();
        let (app, state) = common::harness::get_test_app_and_state().await;

        let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
        *state.event_tx.lock() = Some(event_tx.clone());
        let router_join =
            vox_lib::pipeline::router::spawn_router(app.clone(), event_rx).expect("router");

        Self {
            app,
            state,
            event_tx,
            router_join: Some(router_join),
            _paths: paths,
        }
    }

    fn set_output_mode(&self, mode: DictationOutputMode) {
        let mut settings = self.state.settings.write().unwrap();
        settings.dictation.enabled = true;
        settings.dictation.output_mode = mode;
    }

    fn set_transliteration(&self, enabled: bool) {
        let mut settings = self.state.settings.write().unwrap();
        settings.stt.transliterate_enabled = enabled;
    }

    /// Puts the dictation track into Thinking so the transcript has a turn to land on.
    fn thinking(&self) {
        transition_dictation(InteractionState::Thinking, &self.app, &self.state);
    }

    /// Sends a TranscriptFinal through the production router and waits for the
    /// dictation state to return to Ready. That transition is the barrier.
    async fn deliver(&self, text: &str) {
        self.event_tx
            .send(VoxEvent::TranscriptFinal {
                turn_id: self.state.pipeline.peek_turn_id(),
                text: text.to_string(),
                owner: InteractionOwner::Dictation,
            })
            .expect("router must accept events");
        assert!(
            common::harness::wait_for_dictation_state(
                &self.state,
                InteractionState::Ready,
                DEADLINE
            )
            .await,
            "the transcript must return the dictation track to Ready"
        );
    }

    fn last_transcript(&self) -> Option<String> {
        self.state.dictation_last_transcript.lock().clone()
    }

    fn history_lines(&self) -> Vec<String> {
        let path = vox_lib::utils::paths::dictation_history_file();
        fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|l| l.to_string())
            .collect()
    }

    fn history_len(&self) -> usize {
        self.state.pipeline.transcript_history.lock().len()
    }

    async fn notification_titles(&self) -> Vec<String> {
        let conn = self.state.db.connect().expect("db connection");
        let mut rows = conn
            .query("SELECT title FROM notifications ORDER BY created_at", ())
            .await
            .expect("notification query");
        let mut titles = Vec::new();
        while let Some(row) = rows.next().await.expect("notification row") {
            let title: String = row.get(0).expect("title column");
            titles.push(title);
        }
        titles
    }

    fn close(mut self) {
        let _ = self.event_tx.send(VoxEvent::Shutdown);
        if let Some(join) = self.router_join.take() {
            join.join().expect("router thread panicked");
        }
    }
}

/// PURPOSE:  A finalized transcript must be stored for the next turn and must survive
///           every output mode.
/// SCENARIO: One transcript is delivered through the production router in each output mode.
/// ASSERTS:  `dictation_last_transcript` holds the text, the in-memory history grows, the
///           durable cache history gains one JSON line, and the dictation track returns to
///           Ready. Tray mode raises no "Dictation Copied" notification.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the `*state.dictation_last_transcript.lock() = ...` assignment.
#[tokio::test]
async fn transcript_is_stored_and_survives_every_output_mode() {
    let rig = Rig::new().await;

    // ---------------------------------------------------------------- S1
    // Tray mode: stored, no sink touched.
    {
        rig.set_output_mode(DictationOutputMode::Tray);
        rig.thinking();
        let baseline = rig.history_lines().len();
        rig.deliver("stored transcript for tray").await;

        assert_eq!(
            rig.last_transcript().as_deref(),
            Some("stored transcript for tray"),
            "the transcript must be stored"
        );
        assert_eq!(rig.history_len(), 1, "one in-memory history entry");
        assert_eq!(
            rig.history_lines().len(),
            baseline + 1,
            "one durable history line per turn"
        );
        assert!(rig.history_lines()[baseline].contains("stored transcript for tray"));
    }

    // ---------------------------------------------------------------- S2
    // Clipboard mode: stored even though the clipboard may be unavailable.
    {
        rig.set_output_mode(DictationOutputMode::Clipboard);
        rig.thinking();
        let baseline = rig.history_lines().len();
        rig.deliver("stored transcript for clipboard").await;

        assert_eq!(
            rig.last_transcript().as_deref(),
            Some("stored transcript for clipboard"),
            "a failed clipboard write must not lose the transcript"
        );
        assert_eq!(rig.history_len(), 2);
        assert_eq!(rig.history_lines().len(), baseline + 1);
    }

    // ---------------------------------------------------------------- S3
    // Paste mode: stored even though the paste cannot be simulated headlessly.
    {
        rig.set_output_mode(DictationOutputMode::Paste);
        rig.thinking();
        let baseline = rig.history_lines().len();
        rig.deliver("stored transcript for paste").await;

        assert_eq!(
            rig.last_transcript().as_deref(),
            Some("stored transcript for paste"),
            "a failed paste must not lose the transcript"
        );
        assert_eq!(rig.history_len(), 3);
        assert_eq!(rig.history_lines().len(), baseline + 1);
    }

    // ---------------------------------------------------------------- S4
    // Tray mode raises no clipboard notification.
    {
        let titles = rig.notification_titles().await;
        assert!(
            !titles.iter().any(|t| t == "Dictation Copied"),
            "the tray output mode must not raise a clipboard notification, got {:?}",
            titles
        );
    }

    rig.close();
}

/// PURPOSE:  An empty or blank transcript is not a turn and must not be stored.
/// ASSERTS:  The last transcript stays unset and no history line is written.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the `text.trim().is_empty()` guard.
#[tokio::test]
async fn blank_transcript_is_not_stored() {
    let rig = Rig::new().await;
    rig.set_output_mode(DictationOutputMode::Tray);

    rig.thinking();
    rig.deliver("").await;
    assert_eq!(
        rig.last_transcript(),
        None,
        "an empty transcript must not be stored"
    );
    assert_eq!(rig.history_len(), 0);
    assert_eq!(rig.history_lines().len(), 0);

    rig.thinking();
    rig.deliver("   ").await;
    assert_eq!(
        rig.last_transcript(),
        None,
        "a whitespace-only transcript must not be stored"
    );
    assert_eq!(rig.history_lines().len(), 0);
    rig.close();
}

/// PURPOSE:  While dictation is disabled the transcript must be dropped, not stored.
/// SCENARIO: `on_transcript_final` is called directly while the track is Idle, then the
///           track is re-enabled and a second transcript is delivered.
/// ASSERTS:  Only the second transcript is stored, and only it reaches the history.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove the `dictation_state == Idle` early return.
///
/// NOTE: the handler is called directly here. The router hop is asynchronous, and
/// re-enabling dictation before the router drained the queue would race the state
/// check. The router path is covered by every other scenario in this binary.
#[tokio::test]
async fn transcript_is_dropped_while_dictation_is_disabled() {
    let rig = Rig::new().await;
    rig.set_output_mode(DictationOutputMode::Tray);
    transition_dictation(InteractionState::Idle, &rig.app, &rig.state);

    vox_lib::pipeline::dictation::transcript::on_transcript_final(
        rig.state.pipeline.peek_turn_id(),
        "must not be stored".to_string(),
        &rig.app,
        &rig.state,
    );

    assert_eq!(
        rig.last_transcript(),
        None,
        "a transcript must not be stored while dictation is disabled"
    );
    assert_eq!(rig.history_len(), 0);
    assert_eq!(rig.history_lines().len(), 0);

    rig.thinking();
    rig.deliver("stored after re-enable").await;

    assert_eq!(
        rig.last_transcript().as_deref(),
        Some("stored after re-enable"),
        "the re-enabled transcript must be stored"
    );
    assert_eq!(rig.history_len(), 1);
    assert_eq!(rig.history_lines().len(), 1);
    assert!(rig.history_lines()[0].contains("stored after re-enable"));
    rig.close();
}

/// PURPOSE:  Devanagari must reach the user as transliterated text when the setting is on.
/// ASSERTS:  With the transliteration engine available the stored transcript contains no
///           Devanagari characters. Without the model the documented fallback applies and
///           the raw script is preserved.
/// CLASS:    happy-path
/// REQUIRES: the transliteration model for the transliterated branch; the fallback branch
///            runs without it.
/// MUTANT:   Pass `transliterate_enabled = false` to `transliterate_if_hi`.
///
/// NOTE: the transliteration engine is an ONNX model under `models/translit`. Run this
/// binary in the models tier to assert the transliterated branch.
#[tokio::test]
async fn devanagari_transcript_is_transliterated_when_enabled() {
    let rig = Rig::new().await;
    rig.set_output_mode(DictationOutputMode::Tray);
    rig.set_transliteration(true);
    rig.thinking();

    let engine_available = vox_lib::services::translit::init_transliteration_engine().is_ok();

    rig.deliver("नमस्ते वोक्स").await;

    let stored = rig.last_transcript().expect("transcript must be stored");
    if engine_available {
        assert!(
            !stored
                .chars()
                .any(|c| ('\u{0900}'..='\u{097F}').contains(&c)),
            "the stored transcript must be transliterated, got {:?}",
            stored
        );
    } else {
        assert!(
            stored
                .chars()
                .any(|c| ('\u{0900}'..='\u{097F}').contains(&c)),
            "without the model the raw script must be preserved, got {:?}",
            stored
        );
    }
    assert!(!stored.is_empty());
    rig.close();
}

/// PURPOSE:  With transliteration off the original script must be preserved.
/// ASSERTS:  The stored transcript still contains Devanagari.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Always transliterate regardless of the setting.
#[tokio::test]
async fn devanagari_transcript_is_preserved_when_transliteration_is_off() {
    let rig = Rig::new().await;
    rig.set_output_mode(DictationOutputMode::Tray);
    rig.set_transliteration(false);
    rig.thinking();

    rig.deliver("नमस्ते वोक्स").await;

    let stored = rig.last_transcript().expect("transcript must be stored");
    assert!(
        stored
            .chars()
            .any(|c| ('\u{0900}'..='\u{097F}').contains(&c)),
        "the original script must be preserved, got {:?}",
        stored
    );
    rig.close();
}

/// PURPOSE:  Every turn must get its own history entry, in order.
/// ASSERTS:  Two transcripts produce two in-memory entries and two JSON lines in order.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Clear the history deque before pushing.
#[tokio::test]
async fn every_transcript_gets_its_own_history_entry() {
    let rig = Rig::new().await;
    rig.set_output_mode(DictationOutputMode::Tray);

    rig.thinking();
    rig.deliver("first turn").await;
    rig.thinking();
    rig.deliver("second turn").await;

    assert_eq!(rig.history_len(), 2);
    let lines = rig.history_lines();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("first turn"));
    assert!(lines[1].contains("second turn"));
    rig.close();
}

/// PURPOSE:  The output router must accept the tray mode without touching a sink.
/// ASSERTS:  `route_transcript` returns Ok for the tray mode.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Make the tray arm return an error.
#[tokio::test]
async fn output_router_returns_ok_for_tray_mode() {
    let rig = Rig::new().await;
    let result = route_transcript(
        &rig.app,
        "tray text",
        DictationOutputMode::Tray,
        &rig.state.db,
    )
    .await;
    assert!(
        result.is_ok(),
        "tray mode must not fail: {:?}",
        result.err()
    );
    rig.close();
}

/// PURPOSE:  A missing display must produce a typed error, never a panic or a silent
///           success.
/// ASSERTS:  Headless: the clipboard arm returns `ClipboardFailed`. With a display it
///           returns Ok.
/// CLASS:    edge-case
/// REQUIRES: none (the headless branch is the normal CI case)
/// MUTANT:   Unwrap the clipboard result instead of propagating the error.
#[tokio::test]
async fn output_router_reports_a_typed_error_without_a_display() {
    let rig = Rig::new().await;
    let result = route_transcript(
        &rig.app,
        "clipboard text",
        DictationOutputMode::Clipboard,
        &rig.state.db,
    )
    .await;

    if headless() {
        let error = result.expect_err("a headless clipboard write must fail");
        assert!(
            matches!(error, DictationError::ClipboardFailed { .. }),
            "the failure must be a typed clipboard error, got {:?}",
            error
        );
    } else {
        assert!(result.is_ok(), "a display is available: {:?}", result.err());
    }
    rig.close();
}

/// PURPOSE:  The dictation track must return to Ready so the next hotkey works.
/// ASSERTS:  After the transcript the dictation state is Ready.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Remove the `transition_dictation(Ready, ...)` after the transcript.
#[tokio::test]
async fn dictation_returns_to_ready_after_the_transcript() {
    let rig = Rig::new().await;
    rig.set_output_mode(DictationOutputMode::Tray);
    rig.thinking();
    assert_eq!(
        rig.state.pipeline.dictation_state(),
        InteractionState::Thinking
    );

    rig.deliver("back to ready").await;

    assert_eq!(
        rig.state.pipeline.dictation_state(),
        InteractionState::Ready
    );
    rig.close();
}

/// PURPOSE:  Test fixtures for dictation history must read from tests/assets/dictation.jsonl
///           inside the isolated sandbox rather than host production storage.
/// SCENARIO: Initialize TempPathsGuard, seed dictation history fixture, read history lines
///           via paths::dictation_history_file().
/// ASSERTS:  History lines match the fixture contents and never touch ~/.vox.
/// CLASS:    fixture
/// REQUIRES: none
/// MUTANT:   Change get_dictation_fixture_path to return non-existent path.
#[tokio::test]
async fn dictation_history_reads_from_sandboxed_fixture() {
    let rig = Rig::new().await;
    rig._paths.seed_dictation_history();

    let lines = rig.history_lines();
    assert_eq!(
        lines.len(),
        2,
        "fixture must contain exactly 2 seeded history lines"
    );
    assert!(lines[0].contains("First dictation test entry"));
    assert!(lines[1].contains("Second dictation test entry"));
    rig.close();
}
