//! ============================================================================
//! tests/remote_voice_catalogue_test.rs — Voice Catalogue Provider Resolution
//! ============================================================================
//! Category     : Integration Test (Seam 7 — TTS voice catalogue)
//! Component    : src/services/tts/voice.rs
//! Prerequisites: None (no models, no services, no DB)
//! Execution    : cargo nextest run --test remote_voice_catalogue_test --release --nocapture --test-threads=1
//! Metrics      : (a) an unrecognised provider id produces an explicit error
//!                and never a catalogue index; (b) every provider the factory
//!                accepts is classified by the voice-profile lookup; (c) the
//!                Kokoro and Supertonic id sets are distinct and
//!                non-overlapping; (d) the provider-scoped default voice still
//!                resolves as specified.
//!
//! WHY THIS EXISTS
//! ---------------
//! The voice catalogue used to resolve a provider by matching raw provider-id
//! string literals, in parallel with the canonical provider enum the factory
//! parses. A typo in a provider id matched nothing, the catalogue came back
//! empty, and the caller fell back to catalogue index zero — which silently
//! gave the user a voice they never chose. A silent voice swap is the kind
//! of failure nobody reports: the assistant simply sounds different.
//!
//! SCOPE — what this can and cannot prove
//! -------------------------------------
//! The assertions are on the resolution contract, not on the audio. The
//! catalogue contents and the voice ids are unchanged by this work.
//! ============================================================================

mod common;

use std::collections::BTreeSet;

use vox_lib::services::tts::{
    caps_for_id, voice::resolve_voice_index_for_provider, TtsActiveProvider,
};

/// Every provider id the provider factory accepts.
///
/// The list mirrors the canonical enum: a provider the factory can build is a
/// provider the voice catalogue must classify.
const FACTORY_PROVIDER_IDS: [&str; 8] = [
    "supertonic",
    "kokoro",
    "chatterbox",
    "chatterbox_remote",
    "edge_tts",
    "zipvoice",
    "azure_tts",
    "google_tts",
];

// ============================================================================
// Test 1: An unrecognised provider id is an explicit error, not an index
// ============================================================================

#[test]
fn test_an_unrecognised_provider_id_yields_an_explicit_error() {
    let error = resolve_voice_index_for_provider("supertonnic", "james")
        .expect_err("a typo in a provider id must not resolve to a catalogue index");

    assert!(
        error.contains("supertonnic"),
        "the error must name the provider id, it said: {error}"
    );

    for unknown in ["", "Kokoro", "kokoro ", "kokoro2", "supertonic\n"] {
        assert!(
            resolve_voice_index_for_provider(unknown, "james").is_err(),
            "the provider id `{unknown}` is not a provider the factory accepts, so it must \
             not resolve to a catalogue index"
        );
    }
}

// ============================================================================
// Test 2: Every provider the factory accepts is classified
// ============================================================================

#[test]
fn test_every_provider_the_factory_accepts_resolves_to_its_own_voice_profile_set() {
    for provider_id in FACTORY_PROVIDER_IDS {
        // The factory must accept the id.
        caps_for_id(provider_id)
            .unwrap_or_else(|error| panic!("the factory must accept `{provider_id}`: {error}"));

        // The voice catalogue must resolve it — never to an error, and never
        // to another provider's catalogue.
        let resolved =
            resolve_voice_index_for_provider(provider_id, "james").unwrap_or_else(|error| {
                panic!("the voice catalogue must resolve `{provider_id}`: {error}")
            });
        assert!(
            resolved >= 0,
            "the resolved index for `{provider_id}` must not be negative, it was {resolved}"
        );
    }

    // The catalogue-bearing providers resolve to their own default voice.
    let kokoro =
        resolve_voice_index_for_provider("kokoro", "james").expect("Kokoro is a known provider id");
    let supertonic = resolve_voice_index_for_provider("supertonic", "james")
        .expect("Supertonic is a known provider id");
    assert_ne!(
        kokoro, supertonic,
        "Kokoro and Supertonic must not resolve to the same catalogue index"
    );
}

// ============================================================================
// Test 3: The Kokoro and Supertonic id sets are distinct
// ============================================================================

#[test]
fn test_the_kokoro_and_supertonic_voice_id_sets_do_not_overlap() {
    let manifest = vox_lib::services::tts::voice::get_manifest_voices();
    let kokoro: BTreeSet<String> = manifest
        .iter()
        .filter(|entry| entry.provider_id == "kokoro")
        .map(|entry| entry.id.clone())
        .collect();
    let supertonic: BTreeSet<String> = manifest
        .iter()
        .filter(|entry| entry.provider_id == "supertonic")
        .map(|entry| entry.id.clone())
        .collect();

    assert!(!kokoro.is_empty(), "the Kokoro voice set must not be empty");
    assert!(
        !supertonic.is_empty(),
        "the Supertonic voice set must not be empty"
    );

    let overlap: Vec<&String> = kokoro.intersection(&supertonic).collect();
    assert!(
        overlap.is_empty(),
        "the Kokoro and Supertonic voice id sets must not overlap, they share {overlap:?}"
    );
    assert!(
        !kokoro.contains("james"),
        "a Supertonic voice id must not be a Kokoro voice id"
    );
    assert!(
        !supertonic.contains("bella"),
        "a Kokoro voice id must not be a Supertonic voice id"
    );
}

#[test]
fn test_each_provider_resolves_a_voice_inside_its_own_catalogue() {
    let kokoro = vox_lib::services::tts::voice::get_voice_profiles();
    let supertonic = vox_lib::services::tts::voice::get_supertonic_voice_profiles();

    for profile in &kokoro {
        let name = profile.name.to_lowercase();
        let index = resolve_voice_index_for_provider("kokoro", &name)
            .expect("Kokoro is a known provider id");
        assert_eq!(
            kokoro[index as usize].name.to_lowercase(),
            name,
            "the Kokoro voice `{name}` must resolve inside the Kokoro catalogue"
        );
    }
    for profile in &supertonic {
        let name = profile.name.to_lowercase();
        let index = resolve_voice_index_for_provider("supertonic", &name)
            .expect("Supertonic is a known provider id");
        assert_eq!(
            supertonic[index as usize].name.to_lowercase(),
            name,
            "the Supertonic voice `{name}` must resolve inside the Supertonic catalogue"
        );
    }
}

// ============================================================================
// Test 4: The provider-scoped default voice still resolves as specified
// ============================================================================

#[test]
fn test_the_provider_scoped_default_voice_still_resolves_as_specified() {
    assert_eq!(
        vox_lib::services::tts::voice::default_voice_id_for_provider("kokoro"),
        Some("bella"),
        "Kokoro's default voice must be Bella"
    );
    assert_eq!(
        vox_lib::services::tts::voice::default_voice_id_for_provider("supertonic"),
        Some("james"),
        "Supertonic's default voice must be James"
    );
    assert_eq!(
        vox_lib::services::tts::voice::default_voice_id_for_provider("edge_tts"),
        None,
        "a provider without a bundled catalogue has no default catalogue voice"
    );

    // A foreign stored id resolves to the active provider's default, not to a
    // catalogue index.
    let kokoro =
        resolve_voice_index_for_provider("kokoro", "james").expect("Kokoro is a known provider id");
    let kokoro_profiles = vox_lib::services::tts::voice::get_voice_profiles();
    assert_eq!(
        kokoro_profiles[kokoro as usize].name, "Bella",
        "a Supertonic voice id under Kokoro must resolve to Kokoro's default"
    );

    let supertonic = resolve_voice_index_for_provider("supertonic", "bella")
        .expect("Supertonic is a known provider id");
    let supertonic_profiles = vox_lib::services::tts::voice::get_supertonic_voice_profiles();
    assert_eq!(
        supertonic_profiles[supertonic as usize].name, "James",
        "a Kokoro voice id under Supertonic must resolve to Supertonic's default"
    );
}

// ============================================================================
// Test 5: The canonical enum is the only provider vocabulary
// ============================================================================

#[test]
fn test_the_provider_ids_the_factory_accepts_match_the_canonical_enum() {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for provider_id in FACTORY_PROVIDER_IDS {
        let parsed: TtsActiveProvider =
            serde_json::from_value(serde_json::Value::String(provider_id.to_string()))
                .unwrap_or_else(|error| {
                    panic!("`{provider_id}` must be a canonical provider id: {error}")
                });
        assert!(
            seen.insert(format!("{parsed:?}")),
            "each provider id must map to its own canonical variant, `{provider_id}` does not"
        );
    }
    assert_eq!(
        seen.len(),
        FACTORY_PROVIDER_IDS.len(),
        "every provider id must map to a distinct canonical variant"
    );
}
