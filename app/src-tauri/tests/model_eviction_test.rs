//! ============================================================================
//! model_eviction_test.rs — Model Eviction & Zero Idle RAM Integration Test
//! ============================================================================
//! Category     : Integration Test (Seam 16)
//! Component    : services/memory/ml + services/tts/actor.rs + services/translit.rs
//! Prerequisites: Local ONNX and TTS models in ~/.vox/models/
//! Execution    : cargo nextest run --test model_eviction_test --release --nocapture --test-threads=1
//! Metrics      : ONNX runtime session drop, LazyLock RwLock reset to None, thread joins, idempotent eviction
//! ============================================================================

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, AtomicU32},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use vox_lib::{
    core::{events::VoxEvent, settings::VoxSettings},
    services::{
        memory::ml::{
            ensure_edge_classifier_loaded, ensure_embedder_loaded, ensure_nli_loaded,
            ensure_scope_classifier_loaded, generate_embedding, is_edge_classifier_loaded,
            is_embedder_loaded, is_nli_loaded, is_scope_classifier_loaded, unload_all_onnx_models,
            unload_memory_pipeline_onnx_models,
        },
        translit::{init_transliteration_engine, is_transliteration_engine_loaded},
        tts::actor::{cool_down_tts, warm_up_tts, TtsCommand, TtsWarmUpHandles},
    },
};

// ============================================================================
// Subtest 1: test_onnx_model_singleton_lifecycle_eviction
// ============================================================================
#[test]
fn test_onnx_model_singleton_lifecycle_eviction() {
    let deadline = Instant::now() + Duration::from_secs(60);
    vox_lib::utils::paths::init();

    // 0. Ensure starting clean
    unload_all_onnx_models();
    assert!(!is_embedder_loaded(), "Embedder must start unloaded");
    assert!(!is_nli_loaded(), "NLI must start unloaded");
    assert!(
        !is_edge_classifier_loaded(),
        "Edge classifier must start unloaded"
    );
    assert!(
        !is_scope_classifier_loaded(),
        "Scope classifier must start unloaded"
    );
    assert!(
        !is_transliteration_engine_loaded(),
        "Transliteration must start unloaded"
    );

    // 1. Lazy load all 4 memory ONNX models + transliteration engine
    let embedder_res = ensure_embedder_loaded(true).expect("ensure_embedder_loaded failed");
    assert!(embedder_res, "ensure_embedder_loaded must return true");
    assert!(is_embedder_loaded(), "Embedder must be loaded");

    let nli_res = ensure_nli_loaded("").expect("ensure_nli_loaded failed");
    assert!(nli_res, "ensure_nli_loaded must return true");
    assert!(is_nli_loaded(), "NLI engine must be loaded");

    ensure_edge_classifier_loaded().expect("ensure_edge_classifier_loaded failed");
    assert!(
        is_edge_classifier_loaded(),
        "Edge classifier must be loaded"
    );

    let scope_res =
        ensure_scope_classifier_loaded().expect("ensure_scope_classifier_loaded failed");
    assert!(scope_res, "ensure_scope_classifier_loaded must return true");
    assert!(
        is_scope_classifier_loaded(),
        "Scope classifier must be loaded"
    );

    init_transliteration_engine().expect("init_transliteration_engine failed");
    assert!(
        is_transliteration_engine_loaded(),
        "Transliteration must be loaded"
    );

    // Verify embedder actually produces embeddings while loaded
    let emb = generate_embedding("Systems engineering test query.")
        .expect("generate_embedding must succeed when loaded");
    assert!(emb.is_some(), "Active embedder must return Some(vector)");
    assert_eq!(emb.unwrap().len(), 384, "MiniLM dimension must be 384");

    // 2. Partial Eviction: unload_memory_pipeline_onnx_models (3 models)
    unload_memory_pipeline_onnx_models();
    assert!(
        !is_embedder_loaded(),
        "Embedder must be evicted after memory pipeline unload"
    );
    assert!(
        !is_nli_loaded(),
        "NLI must be evicted after memory pipeline unload"
    );
    assert!(
        !is_edge_classifier_loaded(),
        "Edge classifier must be evicted after memory pipeline unload"
    );
    // Scope and transliteration should still remain active
    assert!(
        is_scope_classifier_loaded(),
        "Scope classifier should remain loaded"
    );
    assert!(
        is_transliteration_engine_loaded(),
        "Transliteration should remain loaded"
    );

    // Calling generate_embedding when unloaded returns Ok(None) cleanly without SIGSEGV
    let post_evict_emb = generate_embedding("After eviction test.")
        .expect("generate_embedding must return Ok(None) when unloaded");
    assert!(
        post_evict_emb.is_none(),
        "Unloaded embedder must return None"
    );

    // 3. Full Eviction: unload_all_onnx_models (evicts everything + translit + triggers trim_heap)
    unload_all_onnx_models();
    assert!(!is_embedder_loaded(), "Embedder must be evicted");
    assert!(!is_nli_loaded(), "NLI must be evicted");
    assert!(
        !is_edge_classifier_loaded(),
        "Edge classifier must be evicted"
    );
    assert!(
        !is_scope_classifier_loaded(),
        "Scope classifier must be evicted"
    );
    assert!(
        !is_transliteration_engine_loaded(),
        "Transliteration must be evicted"
    );

    // 4. Idempotency test: calling unload a second time must not panic or error
    unload_all_onnx_models();
    assert!(!is_embedder_loaded());
    assert!(!is_nli_loaded());

    // 5. Reload verification: proving models can be reloaded into memory cleanly
    let reload_embedder =
        ensure_embedder_loaded(true).expect("Reloading embedder after unload must succeed");
    assert!(reload_embedder);
    assert!(is_embedder_loaded());

    // Teardown: leave clean
    unload_all_onnx_models();
    assert!(!is_embedder_loaded());

    assert!(
        Instant::now() < deadline,
        "test_onnx_model_singleton_lifecycle_eviction exceeded 60s deadline"
    );
}

// ============================================================================
// Subtest 2: test_tts_worker_cool_down_clears_handles_and_joins
// ============================================================================
#[tokio::test]
async fn test_tts_worker_cool_down_clears_handles_and_joins() {
    let test_timeout = Duration::from_secs(20);
    tokio::time::timeout(test_timeout, async {
        vox_lib::utils::paths::init();
        let supertonic_model_dir = common::paths::get_supertonic_model_dir();
        assert!(supertonic_model_dir.exists());

        let mut settings = VoxSettings::default();
        settings.tts.active = vox_lib::core::settings::TtsActiveProvider::Supertonic;

        let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
        let (playback_engine, _consumer) = common::harness::create_mock_playback_engine();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pending_jobs = Arc::new(AtomicU32::new(0));

        let mut tts_tx: Option<mpsc::Sender<TtsCommand>> = None;
        let mut tts_handle: Option<std::thread::JoinHandle<()>> = None;

        // 1. Warm up TTS worker
        let handles = TtsWarmUpHandles {
            tts_tx: &mut tts_tx,
            tts_handle: &mut tts_handle,
            cancel_flag,
            playback_engine,
            pending_synthesis_jobs: Some(pending_jobs),
            telemetry_rtf: None,
        };

        let warm_res = warm_up_tts(handles, &settings, &supertonic_model_dir, None, event_tx);
        assert!(warm_res.is_ok(), "warm_up_tts must return Ok(())");
        assert!(tts_tx.is_some(), "tts_tx must be populated after warm-up");
        assert!(
            tts_handle.is_some(),
            "tts_handle must be populated after warm-up"
        );

        // 2. Cool down TTS worker
        cool_down_tts(&mut tts_tx);

        // 3. Assert tts_tx was taken (set to None)
        assert!(
            tts_tx.is_none(),
            "cool_down_tts must take and reset tts_tx to None"
        );

        // 4. Assert worker thread exits cleanly and joins without panic
        if let Some(handle) = tts_handle.take() {
            let join_res = handle.join();
            assert!(
                join_res.is_ok(),
                "TTS worker thread must join cleanly on Shutdown command"
            );
        } else {
            panic!("tts_handle was None unexpectedly");
        }
    })
    .await
    .expect("test_tts_worker_cool_down_clears_handles_and_joins timed out");
}
