//! ============================================================================
//! model_eviction_test.rs — ONNX singletons are loaded on demand and released by scope, and the TTS worker thread is joined on cool-down
//! ============================================================================
//! Category  : Integration (Seam 16 — model lifecycle)
//! Component : src/services/memory/ml/, src/services/translit.rs, src/services/tts/actor.rs
//! Binary    : model_eviction_test
//! Prereqs   : VOX_MODELS_DIR pointing at the model root (MiniLM embedder, translit engine, Supertonic TTS)
//! Run       : cargo nextest run --test model_eviction_test --release --test-threads=1
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32},
        mpsc, Arc,
    },
    time::Duration,
};

use vox_lib::{
    config::VoxSettings,
    core::events::VoxEvent,
    services::{
        memory::ml::{
            ensure_embedder_loaded, generate_embedding, is_embedder_loaded, unload_all_onnx_models,
            unload_memory_pipeline_onnx_models,
        },
        translit::{init_transliteration_engine, is_transliteration_engine_loaded},
        tts::actor::{cool_down_tts, warm_up_tts, TtsCommand, TtsWarmUpConfig, TtsWarmUpHandles},
    },
};

/// PURPOSE:  The ONNX singletons must start unloaded; a stale global that
///           reports "loaded" at boot would let the app believe it holds no
///           model weights while actually pinning RAM.
/// SCENARIO: Initialize paths, force a full unload, then inspect the loaded
///           flags for the embedder and the transliteration engine.
/// ASSERTS:  Both `is_embedder_loaded` and `is_transliteration_engine_loaded`
///           report false.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR
/// MUTANT:   Make `unload_all_onnx_models` a no-op — the flags would still read
///           false here, so the stronger mutant is to have `is_embedder_loaded`
///           return true unconditionally.
#[test]
fn onnx_singletons_start_unloaded_after_full_unload() {
    let _guard = common::paths::TempPathsGuard::new();

    unload_all_onnx_models();

    assert!(!is_embedder_loaded(), "Embedder must start unloaded");
    assert!(
        !is_transliteration_engine_loaded(),
        "Transliteration must start unloaded"
    );
}

/// PURPOSE:  A loaded embedder must actually produce a 384-dimensional vector;
///           a "loaded" flag that returns an empty vector would silently
///           disable every semantic memory lookup.
/// SCENARIO: Load the MiniLM embedder and embed one sentence.
/// ASSERTS:  `ensure_embedder_loaded` returns true, the flag reads loaded, and
///           the produced vector has exactly 384 dimensions.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR with the MiniLM embedding weights
/// MUTANT:   Return `Ok(Some(vec![0.0; 0]))` from the embedder — the dimension
///           assertion would catch it.
#[test]
fn loaded_embedder_produces_384_dimension_vector() {
    let _guard = common::paths::TempPathsGuard::new();
    unload_all_onnx_models();

    let loaded = ensure_embedder_loaded(true).expect("ensure_embedder_loaded failed");
    assert!(loaded, "ensure_embedder_loaded must return true");
    assert!(is_embedder_loaded(), "Embedder must be loaded");

    let emb = generate_embedding("Systems engineering test query.")
        .expect("generate_embedding must succeed when loaded");
    assert!(emb.is_some(), "Active embedder must return Some(vector)");
    assert_eq!(emb.unwrap().len(), 384, "MiniLM dimension must be 384");
}

/// PURPOSE:  `unload_memory_pipeline_onnx_models` must release the memory
///           pipeline weights without touching the transliteration engine;
///           over-eager unloading would make every transliteration request
///           re-load the engine.
/// SCENARIO: Load embedder and transliteration, unload only the memory
///           pipeline, then inspect both flags.
/// ASSERTS:  The embedder reports unloaded while the transliteration engine
///           still reports loaded.
/// CLASS:    edge-case
/// REQUIRES: VOX_MODELS_DIR
/// MUTANT:   Make `unload_memory_pipeline_onnx_models` also reset the
///           transliteration engine — the transliteration assertion would fail.
#[test]
fn memory_pipeline_unload_evicts_embedder_but_keeps_transliteration() {
    let _guard = common::paths::TempPathsGuard::new();
    unload_all_onnx_models();

    ensure_embedder_loaded(true).expect("ensure_embedder_loaded failed");
    init_transliteration_engine().expect("init_transliteration_engine failed");
    assert!(is_embedder_loaded());
    assert!(is_transliteration_engine_loaded());

    unload_memory_pipeline_onnx_models();

    assert!(
        !is_embedder_loaded(),
        "Embedder must be evicted after memory pipeline unload"
    );
    assert!(
        is_transliteration_engine_loaded(),
        "Transliteration should remain loaded"
    );
}

/// PURPOSE:  After the embedder is evicted, `generate_embedding` must return
///           `Ok(None)` instead of crashing on a dropped ONNX session; a
///           use-after-free here is a SIGSEGV in the running app.
/// SCENARIO: Evict the memory pipeline and then call the embedding API.
/// ASSERTS:  `generate_embedding` returns `Ok(None)` and does not panic.
/// CLASS:    edge-case
/// REQUIRES: VOX_MODELS_DIR
/// MUTANT:   Make the embedder accessor dereference the `None` session — the
///           test would abort instead of returning `Ok(None)`.
#[test]
fn evicted_embedder_returns_none_without_crashing() {
    let _guard = common::paths::TempPathsGuard::new();
    unload_all_onnx_models();

    ensure_embedder_loaded(true).expect("ensure_embedder_loaded failed");
    assert!(is_embedder_loaded());
    unload_memory_pipeline_onnx_models();
    assert!(!is_embedder_loaded());

    let post_evict_emb = generate_embedding("After eviction test.")
        .expect("generate_embedding must return Ok(None) when unloaded");
    assert!(
        post_evict_emb.is_none(),
        "Unloaded embedder must return None"
    );
}

/// PURPOSE:  The full ONNX unload must release every model backend, including
///           the transliteration engine; leaving it resident keeps the
///           zero-idle-RAM promise false.
/// SCENARIO: Load embedder and transliteration, then call the full unload.
/// ASSERTS:  Both loaded flags report false afterwards.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR
/// MUTANT:   Remove the transliteration reset from `unload_all_onnx_models` —
///           the transliteration assertion would fail.
#[test]
fn full_onnx_unload_evicts_embedder_and_transliteration() {
    let _guard = common::paths::TempPathsGuard::new();
    unload_all_onnx_models();

    ensure_embedder_loaded(true).expect("ensure_embedder_loaded failed");
    init_transliteration_engine().expect("init_transliteration_engine failed");
    assert!(is_embedder_loaded());
    assert!(is_transliteration_engine_loaded());

    unload_all_onnx_models();

    assert!(!is_embedder_loaded(), "Embedder must be evicted");
    assert!(
        !is_transliteration_engine_loaded(),
        "Transliteration must be evicted"
    );
}

/// PURPOSE:  Eviction runs on every idle cycle; a second unload must not
///           panic or error on an already-empty registry.
/// SCENARIO: Call the full unload twice in a row.
/// ASSERTS:  The second call returns normally and the embedder flag stays
///           false.
/// CLASS:    edge-case
/// REQUIRES: VOX_MODELS_DIR
/// MUTANT:   Make the second unload call `unwrap()` on an empty registry — the
///           test would panic.
#[test]
fn repeated_full_unload_is_idempotent() {
    let _guard = common::paths::TempPathsGuard::new();
    unload_all_onnx_models();

    unload_all_onnx_models();
    unload_all_onnx_models();

    assert!(!is_embedder_loaded());
    assert!(!is_transliteration_engine_loaded());
}

/// PURPOSE:  Eviction must not make the models permanently unloadable; the
///           app has to be able to reload them on the next request.
/// SCENARIO: Unload everything, then load the embedder again.
/// ASSERTS:  The reload returns true and the embedder flag reads loaded.
/// CLASS:    regression
/// REQUIRES: VOX_MODELS_DIR with the MiniLM embedding weights
/// MUTANT:   Have `unload_all_onnx_models` poison the registry so a later
///           `ensure_embedder_loaded` returns false — the assertion would fail.
#[test]
fn embedder_reloads_cleanly_after_full_unload() {
    let _guard = common::paths::TempPathsGuard::new();
    unload_all_onnx_models();

    ensure_embedder_loaded(true).expect("initial load must succeed");
    unload_all_onnx_models();
    assert!(!is_embedder_loaded());

    let reload_embedder =
        ensure_embedder_loaded(true).expect("Reloading embedder after unload must succeed");
    assert!(reload_embedder);
    assert!(is_embedder_loaded());
}

/// PURPOSE:  `warm_up_tts` must hand back a live worker channel and thread
///           handle; a warm-up that returns without a worker leaves the app
///           with no synthesis path.
/// SCENARIO: Warm up the TTS worker against the local Supertonic model with a
///           mock playback engine.
/// ASSERTS:  `warm_up_tts` returns Ok and both the command sender and the
///           worker join handle are populated.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR with the Supertonic TTS weights
/// MUTANT:   Return Ok from `warm_up_tts` without spawning the worker — the
///           handle assertions would fail.
#[tokio::test]
async fn tts_warmup_populates_worker_handles() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let _guard = common::paths::TempPathsGuard::new();
        let supertonic_model_dir = common::paths::get_supertonic_model_dir();
        assert!(supertonic_model_dir.exists());

        let mut settings = VoxSettings::default();
        settings.tts.active = vox_lib::config::ActiveSelection {
            category: "embedded".to_string(),
            provider: None,
            model: "supertonic".to_string(),
        };

        let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
        let (playback_engine, _consumer) = common::harness::create_mock_playback_engine();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pending_jobs = Arc::new(AtomicU32::new(0));

        let mut tts_tx: Option<mpsc::Sender<TtsCommand>> = None;
        let mut tts_handle: Option<std::thread::JoinHandle<()>> = None;

        let handles = TtsWarmUpHandles {
            tts_tx: &mut tts_tx,
            tts_handle: &mut tts_handle,
            cancel_flag,
            playback_engine,
            pending_synthesis_jobs: Some(pending_jobs),
            telemetry_rtf: None,
            turn_metrics: None,
        };

        let warm_res = warm_up_tts(
            handles,
            TtsWarmUpConfig {
                settings: &settings.tts,
                providers: None,
                super_tts_path: &supertonic_model_dir,
                reference_audio: None,
                speed: 1.0,
            },
            event_tx,
        );

        assert!(warm_res.is_ok(), "warm_up_tts must return Ok(())");
        assert!(tts_tx.is_some(), "tts_tx must be populated after warm-up");
        assert!(
            tts_handle.is_some(),
            "tts_handle must be populated after warm-up"
        );

        // Tear the worker down so the test leaves no thread behind.
        cool_down_tts(&mut tts_tx);
        if let Some(handle) = tts_handle.take() {
            handle.join().expect("TTS worker thread must join cleanly");
        }
    })
    .await
    .expect("tts_warmup_populates_worker_handles timed out");
}

/// PURPOSE:  `cool_down_tts` must take the command sender and let the worker
///           thread exit; a worker that never joins pins the TTS model in RAM
///           and keeps the ONNX session alive after eviction.
/// SCENARIO: Warm up the TTS worker, cool it down, then join the handle.
/// ASSERTS:  `tts_tx` is `None` after the cool-down and the worker thread
///           joins without panicking.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR with the Supertonic TTS weights
/// MUTANT:   Make `cool_down_tts` leave the sender in place — the `tts_tx`
///           assertion would fail; make the worker ignore `Shutdown` — the
///           join assertion would hang and trip the test timeout.
#[tokio::test]
async fn tts_cooldown_clears_sender_and_joins_worker() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let _guard = common::paths::TempPathsGuard::new();
        let supertonic_model_dir = common::paths::get_supertonic_model_dir();
        assert!(supertonic_model_dir.exists());

        let mut settings = VoxSettings::default();
        settings.tts.active = vox_lib::config::ActiveSelection {
            category: "embedded".to_string(),
            provider: None,
            model: "supertonic".to_string(),
        };

        let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
        let (playback_engine, _consumer) = common::harness::create_mock_playback_engine();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pending_jobs = Arc::new(AtomicU32::new(0));

        let mut tts_tx: Option<mpsc::Sender<TtsCommand>> = None;
        let mut tts_handle: Option<std::thread::JoinHandle<()>> = None;

        let handles = TtsWarmUpHandles {
            tts_tx: &mut tts_tx,
            tts_handle: &mut tts_handle,
            cancel_flag,
            playback_engine,
            pending_synthesis_jobs: Some(pending_jobs),
            telemetry_rtf: None,
            turn_metrics: None,
        };

        let warm_res = warm_up_tts(
            handles,
            TtsWarmUpConfig {
                settings: &settings.tts,
                providers: None,
                super_tts_path: &supertonic_model_dir,
                reference_audio: None,
                speed: 1.0,
            },
            event_tx,
        );
        assert!(warm_res.is_ok(), "warm_up_tts must return Ok(())");
        assert!(tts_handle.is_some(), "warm-up must produce a worker handle");

        cool_down_tts(&mut tts_tx);

        assert!(
            tts_tx.is_none(),
            "cool_down_tts must take and reset tts_tx to None"
        );

        let handle = tts_handle.take().expect("tts_handle must be present");
        let join_res = handle.join();
        assert!(
            join_res.is_ok(),
            "TTS worker thread must join cleanly on Shutdown command"
        );
    })
    .await
    .expect("tts_cooldown_clears_sender_and_joins_worker timed out");
}
