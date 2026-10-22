//! ============================================================================
//! tests/remote_tts_provider_test.rs — Default TTS Provider & Edge TTS Failure
//! ============================================================================
//! Category     : Integration Test (Seam 7 — TTS provider resolution)
//! Component    : services/tts/{config,factory,voice,providers/*}.rs
//! Prerequisites: Local Supertonic ONNX model in ~/.vox/models/tts/supertonic-3/
//! Execution    : cargo nextest run --test remote_tts_provider_test --release --nocapture --test-threads=1
//! Metrics      : (a) a fresh install resolves to `supertonic`; (b) the
//!                Supertonic path produces non-silent PCM; (c) the `edge_tts`
//!                path returns the explicit HTTP 403 availability error and
//!                produces no audio; (d) `edge_tts` is still in the provider
//!                catalogue and the voice catalog is unchanged; (e) the factory
//!                resolves exactly one provider; (f) the Devanagari route
//!                selects Supertonic, never the dead Edge TTS route;
//!                (g) the default voice is provider-aware; (h) the Devanagari
//!                route builds the Supertonic engine once and reuses it.
//!
//! WHY THIS EXISTS
//! ---------------
//! The Microsoft ReadAloud endpoint answers HTTP 403 for every client. It was
//! the default TTS provider, so a fresh install selected a provider that
//! cannot speak: the assistant produced an empty buffer and stayed silent.
//! That is the reason the remote-control host path was blocked — the host has
//! to be able to talk for the phone to hear it.
//!
//! The failure mode this file guards is the quiet one. A provider that fails
//! must say so with an actionable error, and must never hand the turn to a
//! different provider: a silent fallback gives the user a voice they did not
//! choose and hides the misconfiguration. The same rule applies to the
//! Devanagari route: it used to point at Edge TTS, which is now permanently
//! unavailable, so the route pointed at a dead end while claiming otherwise.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! The Supertonic assertions run the real ONNX model, so they are slow but
//! they are evidence. The Edge TTS assertion is deterministic and needs no
//! network: the upstream failure is a known constant, not a runtime probe.
//! ============================================================================

mod common;

use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32},
        mpsc, Arc,
    },
    time::Duration,
};

use ringbuf::traits::Consumer;
use vox_lib::{
    core::events::{AudioIntent, VoxEvent},
    services::tts::{
        providers::{
            devanagari::{
                live_devanagari_engine_count, DevanagariEngineCache, DEVANAGARI_ROUTE_PROVIDER,
            },
            edge_tts::EdgeTtsProvider,
            supertonic::TtsEngine as SupertonicEngine,
            SynthesisContext, TtsProvider,
        },
        voice::{
            default_voice_id_for_provider, get_supertonic_voice_profiles, get_voice_profiles,
            resolve_catalog_index, resolve_provider_voice_id, resolve_voice_index_for_provider,
        },
        TtsActiveProvider, TtsProviderConfig, TtsVoiceSource,
    },
};

/// The message that must surface when `edge_tts` is selected while the
/// upstream service is unavailable. It is duplicated here on purpose: the
/// test asserts the contract from the outside, not the constant it came from.
const EXPECTED_EDGE_ERROR: &str =
    "Edge TTS is currently unavailable (HTTP 403 from the upstream service)";

// ============================================================================
// Subtest 1: test_fresh_install_resolves_to_supertonic
// ============================================================================
/// Verifies:
///   - The default provider enum variant is `supertonic`.
///   - A fresh install with no stored settings resolves to the Supertonic
///     provider configuration.
///   - The default voice belongs to the default provider's catalogue.
#[test]
fn test_fresh_install_resolves_to_supertonic() {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);

    assert_eq!(
        TtsActiveProvider::default(),
        TtsActiveProvider::Supertonic,
        "The default TTS provider must be `supertonic` — the Edge ReadAloud endpoint \
         answers HTTP 403 for every client"
    );

    let settings = vox_lib::config::VoxSettings::default();
    assert_eq!(
        settings.tts.active.model, "supertonic",
        "A fresh install must select the Supertonic provider"
    );
    assert_eq!(
        settings.tts.last_selected.embedded.as_deref(),
        Some("supertonic"),
        "The embedded last-selected model must be Supertonic"
    );

    let provider_config = settings
        .tts
        .to_provider_config(None, vox_lib::core::defaults::DEFAULT_TTS_SPEED);
    assert_eq!(
        provider_config,
        TtsProviderConfig::Supertonic,
        "A fresh install must resolve to exactly one provider: Supertonic"
    );

    // The default voice must resolve inside the Supertonic catalogue.
    let index = vox_lib::services::tts::voice::resolve_catalog_index(
        &vox_lib::services::tts::voice::get_supertonic_voice_profiles(),
        &settings.tts.active_voice_id,
    );
    assert!(
        index >= 0,
        "The default voice id must resolve in the Supertonic catalogue"
    );

    assert!(
        deadline.elapsed() < Duration::from_secs(10),
        "test_fresh_install_resolves_to_supertonic exceeded 10s deadline"
    );
}

// ============================================================================
// Subtest 2: test_supertonic_path_produces_non_silent_samples
// ============================================================================
/// Verifies:
///   - With `supertonic` selected and its weights present, a synthesis request
///     returns non-silent PCM.
#[tokio::test]
async fn test_supertonic_path_produces_non_silent_samples() {
    tokio::time::timeout(Duration::from_secs(180), async {
        let _guard = common::paths::TempPathsGuard::new();
        vox_lib::utils::paths::init();

        let model_dir = common::paths::get_supertonic_model_dir();
        assert!(
            model_dir.exists(),
            "The Supertonic model directory must exist at {:?}",
            model_dir
        );

        // The default voice id resolves to the catalogue index used below.
        let settings = vox_lib::config::VoxSettings::default();
        let voice_index = vox_lib::services::tts::voice::resolve_catalog_index(
            &vox_lib::services::tts::voice::get_supertonic_voice_profiles(),
            &settings.tts.active_voice_id,
        );

        let provider = SupertonicEngine::new(&model_dir, voice_index, 1.0, 4)
            .expect("Failed to initialize the Supertonic engine");

        let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
        let (playback, consumer_arc) = common::harness::create_mock_playback_engine_with_handles(
            event_tx.clone(),
            Arc::new(AtomicU32::new(1)),
            Arc::new(AtomicU32::new(1)),
        );

        let cancel = Arc::new(AtomicBool::new(false));
        let ctx = SynthesisContext {
            turn_id: 401,
            intent: AudioIntent::TurnResponse,
            cancel,
            playback: &playback,
            event_tx,
            telemetry_rtf: None,
        };

        provider
            .synthesize_chunk("Vox is online and speaking.", &ctx)
            .expect("The Supertonic synthesis must succeed");

        let samples = {
            let mut consumer = consumer_arc.lock();
            let mut drained = Vec::new();
            while let Some(sample) = consumer.try_pop() {
                drained.push(sample);
            }
            drained
        };

        assert!(
            !samples.is_empty(),
            "The Supertonic path must produce audio samples"
        );
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        let rms = (sum_sq / samples.len() as f32).sqrt();
        assert!(
            rms > 0.001,
            "The Supertonic path must produce non-silent audio (RMS: {:.5})",
            rms
        );
    })
    .await
    .expect("test_supertonic_path_produces_non_silent_samples timed out");
}

// ============================================================================
// Subtest 3: test_edge_tts_path_returns_the_explicit_availability_error
// ============================================================================
/// Verifies:
///   - Selecting `edge_tts` returns an explicit error naming the upstream
///     HTTP 403 failure.
///   - The failed request produces no audio at all: no empty buffer, no
///     silence.
///   - The turn is reported to the pipeline as aborted.
#[test]
fn test_edge_tts_path_returns_the_explicit_availability_error() {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);

    let (event_tx, event_rx) = mpsc::channel::<VoxEvent>();
    let (playback, _consumer) = common::harness::create_mock_playback_engine_with_handles(
        event_tx.clone(),
        Arc::new(AtomicU32::new(1)),
        Arc::new(AtomicU32::new(1)),
    );

    let cancel = Arc::new(AtomicBool::new(false));
    let ctx = SynthesisContext {
        turn_id: 402,
        intent: AudioIntent::TurnResponse,
        cancel,
        playback: &playback,
        event_tx,
        telemetry_rtf: None,
    };

    let provider = EdgeTtsProvider::new(None);
    let result = provider.synthesize_chunk("This must not be spoken.", &ctx);

    let error = result.expect_err("The Edge TTS path must fail while the upstream is unavailable");
    let message = error.to_string();
    assert!(
        message.contains("HTTP 403"),
        "The error must name the upstream HTTP 403 failure, got: {}",
        message
    );
    assert!(
        message.contains("Edge TTS"),
        "The error must name the provider, got: {}",
        message
    );
    assert_eq!(
        EXPECTED_EDGE_ERROR, message,
        "The error message must match the contract in the provider specification"
    );

    assert_eq!(
        playback.occupied_len(),
        0,
        "A failed Edge TTS request must not leave an empty audio buffer"
    );

    let event = event_rx
        .recv_timeout(Duration::from_millis(500))
        .expect("The aborted turn must be reported to the pipeline");
    match event {
        VoxEvent::Error(err) => {
            assert!(
                err.message.contains("HTTP 403"),
                "The pipeline error must name the upstream failure, got: {}",
                err.message
            )
        }
        other => panic!("Expected VoxEvent::Error, got {:?}", other),
    }

    assert!(
        deadline.elapsed() < Duration::from_secs(15),
        "test_edge_tts_path_returns_the_explicit_availability_error exceeded 15s deadline"
    );
}

// ============================================================================
// Subtest 4: test_edge_tts_is_still_in_the_provider_catalogue
// ============================================================================
/// Verifies:
///   - `edge_tts` is still listed in the provider catalogue and is still
///     selectable.
///   - The voice catalogue is unchanged: the bundled manifest still carries
///     the same providers and the same number of voices.
#[test]
fn test_edge_tts_is_still_in_the_provider_catalogue() {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);

    let manifest = vox_lib::setup::manifest::VoxManifest::bundled();
    let tts_groups: Vec<_> = manifest
        .model_groups
        .iter()
        .filter(|g| g.category == "tts")
        .collect();

    assert!(
        tts_groups.iter().any(|g| g.id == "edge_tts"),
        "`edge_tts` must remain listed in the provider catalogue"
    );
    assert!(
        tts_groups.iter().any(|g| g.id == "supertonic"),
        "`supertonic` must be listed in the provider catalogue"
    );

    // The provider stays selectable: its capabilities still resolve.
    let caps = vox_lib::services::tts::caps_for_id("edge_tts").expect("edge_tts caps");
    assert_eq!(caps.voices, TtsVoiceSource::Edge);
    assert_eq!(
        caps.execution,
        vox_lib::services::tts::TtsExecutionKind::Cloud
    );

    // The voice catalogue is unchanged.
    let voices = vox_lib::services::tts::voice::get_manifest_voices();
    assert_eq!(
        voices.len(),
        32,
        "The bundled voice catalogue must be unchanged (32 manifest voices)"
    );
    for provider_id in ["kokoro", "supertonic", "zipvoice", "chatterbox"] {
        assert!(
            voices.iter().any(|v| v.provider_id == provider_id),
            "The voice catalogue must still carry `{}` voices",
            provider_id
        );
    }
    assert!(
        voices
            .iter()
            .any(|v| v.id == "james" && v.provider_id == "supertonic"),
        "The Supertonic voice catalogue must be unchanged"
    );

    assert!(
        deadline.elapsed() < Duration::from_secs(10),
        "test_edge_tts_is_still_in_the_provider_catalogue exceeded 10s deadline"
    );
}

// ============================================================================
// Subtest 5: test_provider_factory_resolves_exactly_one_provider
// ============================================================================
/// Verifies:
///   - The factory maps a Supertonic selection to the Supertonic provider and
///     an Edge TTS selection to the Edge TTS provider.
///   - Neither selection is rewritten to a different provider, so a failure can
///     never hand the turn to a voice the user did not choose.
#[test]
fn test_provider_factory_resolves_exactly_one_provider() {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);

    let model_dir = common::paths::get_supertonic_model_dir();

    // 1. The default selection resolves to Supertonic.
    let default_settings = vox_lib::config::VoxSettings::default();
    assert_eq!(
        default_settings.tts.to_provider_config(None, 1.0),
        TtsProviderConfig::Supertonic
    );

    // 2. An explicit Edge TTS selection resolves to the Edge TTS provider and
    //    to nothing else.
    let mut edge_settings = default_settings.clone();
    edge_settings.tts.active.category = "cloud".to_string();
    edge_settings.tts.active.model = "edge_tts".to_string();
    edge_settings.tts.active_voice_id = "en-US-AriaNeural".to_string();
    assert_eq!(
        edge_settings.tts.to_provider_config(None, 1.0),
        TtsProviderConfig::EdgeTts {
            voice: Some("en-US-AriaNeural".to_string())
        },
        "The Edge TTS selection must resolve to the Edge TTS provider, not to another one"
    );

    let edge_provider = vox_lib::services::tts::create_tts_provider(
        &edge_settings.tts,
        None,
        &model_dir,
        None,
        1.0,
    )
    .expect("The Edge TTS provider must still be constructible so it stays selectable");

    // The constructed provider must be the Edge TTS provider itself, not a
    // substitute. Its synthesis failure names Edge TTS, and no other provider
    // produces that error.
    let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
    let (playback, _consumer) = common::harness::create_mock_playback_engine_with_handles(
        event_tx.clone(),
        Arc::new(AtomicU32::new(1)),
        Arc::new(AtomicU32::new(1)),
    );
    let ctx = SynthesisContext {
        turn_id: 403,
        intent: AudioIntent::TurnResponse,
        cancel: Arc::new(AtomicBool::new(false)),
        playback: &playback,
        event_tx,
        telemetry_rtf: None,
    };
    let error = edge_provider
        .synthesize_chunk("This must not be spoken.", &ctx)
        .expect_err("The Edge TTS provider must fail while the upstream is unavailable");
    assert!(
        error.to_string().contains("Edge TTS"),
        "The factory must not substitute another provider for Edge TTS; got: {}",
        error
    );

    assert!(
        deadline.elapsed() < Duration::from_secs(20),
        "test_provider_factory_resolves_exactly_one_provider exceeded 20s deadline"
    );
}

// ============================================================================
// Subtest 6: test_devanagari_route_selects_supertonic_not_edge_tts
// ============================================================================
/// Verifies:
///   - The Devanagari route targets Supertonic, which renders Devanagari
///     natively.
///   - The dead Edge TTS route is gone from both engines that had it, and
///     neither log message mentions Edge TTS any more.
///   - The route names the missing Supertonic weights when they are absent,
///     instead of failing obscurely.
///   - The route really renders Devanagari when the weights are present.
#[test]
fn test_devanagari_route_selects_supertonic_not_edge_tts() {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);

    // 1. The route target is named and is not the dead provider.
    assert_eq!(
        DEVANAGARI_ROUTE_PROVIDER, "supertonic",
        "The Devanagari route must target the provider that renders Devanagari"
    );

    // 2. The dead Edge route is gone from both engines.
    let providers_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/services/tts/providers");
    for engine in ["kokoro.rs", "zipvoice.rs"] {
        let path = providers_dir.join(engine);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", path, e));
        for forbidden in ["EdgeTtsProvider", "EDGE_TTS_HINDI_VOICE", "Edge TTS"] {
            assert!(
                !source.contains(forbidden),
                "{} must not reference `{}`: the Edge TTS route is permanently \
                 unavailable (HTTP 403), so the reference is a dead end that \
                 also lies about what happens",
                engine,
                forbidden
            );
        }
        assert!(
            source.contains("DEVANAGARI_ROUTE_PROVIDER"),
            "{} must name the provider it routes to",
            engine
        );
    }

    assert!(
        deadline.elapsed() < Duration::from_secs(10),
        "test_devanagari_route_selects_supertonic_not_edge_tts exceeded 10s deadline"
    );
}

// ============================================================================
// Subtest 7: test_devanagari_route_reports_missing_supertonic_weights
// ============================================================================
/// Verifies:
///   - The route names the provider and the missing path when the Supertonic
///     weights are absent.
///   - The route renders Devanagari audio when they are present.
#[test]
fn test_devanagari_route_reports_missing_supertonic_weights() {
    let deadline = std::time::Instant::now() + Duration::from_secs(120);

    let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
    let (playback, consumer_arc) = common::harness::create_mock_playback_engine_with_handles(
        event_tx.clone(),
        Arc::new(AtomicU32::new(1)),
        Arc::new(AtomicU32::new(1)),
    );
    let ctx = SynthesisContext {
        turn_id: 404,
        intent: AudioIntent::TurnResponse,
        cancel: Arc::new(AtomicBool::new(false)),
        playback: &playback,
        event_tx,
        telemetry_rtf: None,
    };

    // 1. Absent weights: the error must name the provider and the path.
    let missing = PathBuf::from("/nonexistent/vox/models/tts/supertonic-3");
    let route = DevanagariEngineCache::new();
    let error = route
        .synthesize("आज का मौसम कैसा है?", &ctx, &missing, 1.0)
        .expect_err("The route must fail when the Supertonic weights are absent");
    let message = error.to_string();
    assert!(
        message.contains("supertonic"),
        "The error must name the provider, got: {}",
        message
    );
    assert!(
        message.contains("/nonexistent/vox/models/tts/supertonic-3"),
        "The error must name the missing weights path, got: {}",
        message
    );

    // 2. Present weights: the route renders the Devanagari clause.
    let model_dir = common::paths::get_supertonic_model_dir();
    assert!(
        model_dir.exists(),
        "The Supertonic model directory must exist at {:?}",
        model_dir
    );
    route
        .synthesize("आज का मौसम कैसा है?", &ctx, &model_dir, 1.0)
        .expect("The Devanagari route must render through Supertonic");

    let samples = {
        let mut consumer = consumer_arc.lock();
        let mut drained = Vec::new();
        while let Some(sample) = consumer.try_pop() {
            drained.push(sample);
        }
        drained
    };
    assert!(
        !samples.is_empty(),
        "The Devanagari route must produce audio samples"
    );
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    let rms = (sum_sq / samples.len() as f32).sqrt();
    assert!(
        rms > 0.001,
        "The Devanagari route must produce non-silent audio (RMS: {:.5})",
        rms
    );

    assert!(
        deadline.elapsed() < Duration::from_secs(120),
        "test_devanagari_route_reports_missing_supertonic_weights exceeded 120s deadline"
    );
}

// ============================================================================
// Subtest 8: test_default_voice_is_provider_aware
// ============================================================================
/// Verifies:
///   - Each provider has its own default voice.
///   - A stored voice id from another provider resolves to the active
///     provider's default, not to a catalogue index.
#[test]
fn test_default_voice_is_provider_aware() {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);

    // 1. The defaults are provider-scoped.
    assert_eq!(
        default_voice_id_for_provider("kokoro"),
        Some("bella"),
        "Kokoro's default voice must be Bella"
    );
    assert_eq!(
        default_voice_id_for_provider("supertonic"),
        Some("james"),
        "Supertonic's default voice must be James"
    );
    assert_eq!(
        default_voice_id_for_provider("edge_tts"),
        None,
        "A provider without a bundled catalogue has no default catalogue voice"
    );

    // 2. A stored id from another provider resolves to the provider's default.
    assert_eq!(
        resolve_provider_voice_id("kokoro", "james"),
        "bella",
        "A Supertonic voice id under Kokoro must resolve to Kokoro's default"
    );
    assert_eq!(
        resolve_provider_voice_id("supertonic", "bella"),
        "james",
        "A Kokoro voice id under Supertonic must resolve to Supertonic's default"
    );
    assert_eq!(
        resolve_provider_voice_id("kokoro", ""),
        "bella",
        "An unset voice must resolve to the provider's default"
    );
    assert_eq!(
        resolve_provider_voice_id("supertonic", ""),
        "james",
        "An unset voice must resolve to the provider's default"
    );

    // 3. A voice that belongs to the provider is kept.
    assert_eq!(resolve_provider_voice_id("kokoro", "nicole"), "nicole");
    assert_eq!(resolve_provider_voice_id("supertonic", "sophia"), "sophia");

    // 4. Neither resolution falls through to a catalogue index. The old
    //    behaviour resolved an unknown id to the first catalogue entry, which
    //    is "Alloy" for Kokoro — a voice the user never chose.
    let kokoro_profiles = get_voice_profiles();
    let silent_fallback = resolve_catalog_index(&kokoro_profiles, "james");
    let resolved =
        resolve_voice_index_for_provider("kokoro", "james").expect("Kokoro is a known provider id");
    assert_ne!(
        resolved, silent_fallback,
        "The Kokoro resolution must not fall through to the catalogue index {}",
        silent_fallback
    );
    assert_eq!(
        kokoro_profiles[resolved as usize].name, "Bella",
        "Kokoro must resolve to its own default voice"
    );

    let supertonic_profiles = get_supertonic_voice_profiles();
    let resolved = resolve_voice_index_for_provider("supertonic", "bella")
        .expect("Supertonic is a known provider id");
    assert_eq!(
        supertonic_profiles[resolved as usize].name, "James",
        "Supertonic must resolve to its own default voice"
    );
    assert_eq!(
        resolve_voice_index_for_provider("supertonic", "sophia")
            .expect("Supertonic is a known provider id"),
        5,
        "A voice that belongs to the provider must keep its own index"
    );

    assert!(
        deadline.elapsed() < Duration::from_secs(10),
        "test_default_voice_is_provider_aware exceeded 10s deadline"
    );
}

// ============================================================================
// Subtest 9: test_devanagari_route_reuses_one_cached_engine
// ============================================================================
/// Verifies:
///   - Two consecutive Devanagari clauses build the Supertonic engine once;
///     the second clause is served from the cache.
///   - The engine is released with the provider that owns the cache, so it
///     never outlives its provider.
///   - A new provider rebuilds the engine.
#[test]
fn test_devanagari_route_reuses_one_cached_engine() {
    let deadline = std::time::Instant::now() + Duration::from_secs(180);

    let (event_tx, _event_rx) = mpsc::channel::<VoxEvent>();
    let (playback, consumer_arc) = common::harness::create_mock_playback_engine_with_handles(
        event_tx.clone(),
        Arc::new(AtomicU32::new(1)),
        Arc::new(AtomicU32::new(1)),
    );
    let ctx = SynthesisContext {
        turn_id: 405,
        intent: AudioIntent::TurnResponse,
        cancel: Arc::new(AtomicBool::new(false)),
        playback: &playback,
        event_tx,
        telemetry_rtf: None,
    };

    let model_dir = common::paths::get_supertonic_model_dir();
    assert!(
        model_dir.exists(),
        "The Supertonic model directory must exist at {:?}",
        model_dir
    );

    let baseline = live_devanagari_engine_count();

    // Drains the mock playback buffer so the ring never fills between
    // clauses.
    let drain = || {
        let mut consumer = consumer_arc.lock();
        let mut drained = Vec::new();
        while let Some(sample) = consumer.try_pop() {
            drained.push(sample);
        }
        drained
    };

    // 1. The first clause builds the engine.
    let route = DevanagariEngineCache::new();
    route
        .synthesize("आज का मौसम कैसा है?", &ctx, &model_dir, 1.0)
        .expect("The Devanagari route must render through Supertonic");
    assert!(
        !drain().is_empty(),
        "The first Devanagari clause must produce audio samples"
    );
    assert_eq!(
        route.build_count(),
        1,
        "The first Devanagari clause must build the Supertonic engine"
    );
    assert_eq!(
        live_devanagari_engine_count(),
        baseline + 1,
        "The built engine must be held by the cache"
    );

    // 2. The second clause is served from the cache.
    route
        .synthesize("कल बारिश होगी।", &ctx, &model_dir, 1.0)
        .expect("The second Devanagari clause must render through the cached engine");
    assert!(
        !drain().is_empty(),
        "The second Devanagari clause must produce audio samples"
    );
    assert_eq!(
        route.build_count(),
        1,
        "The second Devanagari clause must not build a second engine"
    );
    assert_eq!(
        live_devanagari_engine_count(),
        baseline + 1,
        "The cache must still hold exactly one engine"
    );

    // 3. The engine is released with the provider that owns the cache.
    drop(route);
    assert_eq!(
        live_devanagari_engine_count(),
        baseline,
        "The engine must be released when its provider is dropped"
    );

    // 4. A new provider rebuilds the engine.
    let route = DevanagariEngineCache::new();
    route
        .synthesize("आज का मौसम कैसा है?", &ctx, &model_dir, 1.0)
        .expect("The Devanagari route must render through Supertonic");
    assert!(
        !drain().is_empty(),
        "The rebuilt route must produce audio samples"
    );
    assert_eq!(
        route.build_count(),
        1,
        "A new provider must rebuild the engine"
    );

    assert!(
        deadline.elapsed() < Duration::from_secs(180),
        "test_devanagari_route_reuses_one_cached_engine exceeded 180s deadline"
    );
}
