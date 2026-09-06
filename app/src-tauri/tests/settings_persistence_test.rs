//! ============================================================================
//! settings_persistence_test.rs — Settings Persistence Integration Test
//! ============================================================================
//! Category     : Integration Test (Seam 15)
//! Component    : core/settings.rs + ipc/settings/mutation.rs + utils/paths.rs
//! Prerequisites: Local temp directory isolation
//! Execution    : cargo nextest run --test settings_persistence_test --release --nocapture --test-threads=1
//! Metrics      : JSON file atomic write, round-trip serialization equality, malformed fallback
//! ============================================================================

mod common;

use std::{
    fs,
    time::{Duration, Instant},
};

use common::paths::TempPathsGuard;
use vox_lib::{
    core::settings::{
        AudioOutputMode, DictationOutputMode, LlmActiveProvider, PipelineMode, SttActiveProvider,
        TtsActiveProvider, VadBackendOption, VoxSettings,
    },
    ipc::settings::apply_setting_mutation,
};

// ============================================================================
// Subtest 1: test_settings_json_roundtrip_persistence
// ============================================================================
#[test]
fn test_settings_json_roundtrip_persistence() {
    let deadline = Instant::now() + Duration::from_secs(10);
    let _guard = TempPathsGuard::new();

    // 1. Start with system defaults
    let mut settings = VoxSettings::default();

    // 2. Mutate fields across multiple key domains via production `apply_setting_mutation`
    // Appearance
    assert!(
        apply_setting_mutation(
            &mut settings,
            "appearance",
            "theme",
            &serde_json::json!("dark")
        )
        .expect("appearance.theme mutation failed"),
        "appearance.theme should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "appearance",
            "accent_seed",
            &serde_json::json!("#8B5CF6")
        )
        .expect("appearance.accent_seed mutation failed"),
        "appearance.accent_seed should be recognized"
    );

    // Audio
    assert!(
        apply_setting_mutation(
            &mut settings,
            "audio",
            "output_mode",
            &serde_json::json!("Headset")
        )
        .expect("audio.output_mode mutation failed"),
        "audio.output_mode should be recognized"
    );

    // VAD
    assert!(
        apply_setting_mutation(&mut settings, "vad", "threshold", &serde_json::json!(0.72))
            .expect("vad.threshold mutation failed"),
        "vad.threshold should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "vad",
            "ptt_noise_gate",
            &serde_json::json!(0.04)
        )
        .expect("vad.ptt_noise_gate mutation failed"),
        "vad.ptt_noise_gate should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "vad",
            "vad_backend",
            &serde_json::json!("earshot")
        )
        .expect("vad.vad_backend mutation failed"),
        "vad.vad_backend should be recognized"
    );

    // STT
    assert!(
        apply_setting_mutation(&mut settings, "stt", "active", &serde_json::json!("cloud"))
            .expect("stt.active mutation failed"),
        "stt.active should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "stt",
            "transliterate_enabled",
            &serde_json::json!(false)
        )
        .expect("stt.transliterate_enabled mutation failed"),
        "stt.transliterate_enabled should be recognized"
    );

    // LLM
    assert!(
        apply_setting_mutation(&mut settings, "llm", "active", &serde_json::json!("server"))
            .expect("llm.active mutation failed"),
        "llm.active should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "llm",
            "temperature",
            &serde_json::json!(0.85)
        )
        .expect("llm.temperature mutation failed"),
        "llm.temperature should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "llm",
            "context_window",
            &serde_json::json!(16384)
        )
        .expect("llm.context_window mutation failed"),
        "llm.context_window should be recognized"
    );

    // TTS
    assert!(
        apply_setting_mutation(
            &mut settings,
            "tts",
            "active",
            &serde_json::json!("supertonic")
        )
        .expect("tts.active mutation failed"),
        "tts.active should be recognized"
    );
    assert!(
        apply_setting_mutation(&mut settings, "tts", "voice_index", &serde_json::json!(42))
            .expect("tts.voice_index mutation failed"),
        "tts.voice_index should be recognized"
    );
    assert!(
        apply_setting_mutation(&mut settings, "tts", "speed", &serde_json::json!(1.15))
            .expect("tts.speed mutation failed"),
        "tts.speed should be recognized"
    );
    assert!(
        apply_setting_mutation(&mut settings, "tts", "quality_steps", &serde_json::json!(8))
            .expect("tts.quality_steps mutation failed"),
        "tts.quality_steps should be recognized"
    );

    // Interaction & Dictation
    assert!(
        apply_setting_mutation(
            &mut settings,
            "interaction",
            "pipeline_mode",
            &serde_json::json!("realtime")
        )
        .expect("interaction.pipeline_mode mutation failed"),
        "interaction.pipeline_mode should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "dictation",
            "enabled",
            &serde_json::json!(true)
        )
        .expect("dictation.enabled mutation failed"),
        "dictation.enabled should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "dictation",
            "output_mode",
            &serde_json::json!("tray")
        )
        .expect("dictation.output_mode mutation failed"),
        "dictation.output_mode should be recognized"
    );

    // Memory
    assert!(
        apply_setting_mutation(
            &mut settings,
            "memory",
            "context_retrieval_enabled",
            &serde_json::json!(false)
        )
        .expect("memory.context_retrieval_enabled mutation failed"),
        "memory.context_retrieval_enabled should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "memory",
            "top_k_facts",
            &serde_json::json!(15)
        )
        .expect("memory.top_k_facts mutation failed"),
        "memory.top_k_facts should be recognized"
    );

    // 3. Persist mutated settings to disk via SUT: VoxSettings::save
    let save_res = settings.save();
    assert!(save_res.is_ok(), "VoxSettings::save must return Ok(())");

    // 4. Verify physical file properties
    let settings_path = vox_lib::utils::paths::settings_path();
    assert!(
        settings_path.exists(),
        "settings.json must exist at paths::settings_path() after save"
    );

    let raw_content =
        fs::read_to_string(&settings_path).expect("settings.json must be readable after save");
    assert!(
        !raw_content.is_empty(),
        "settings.json content must not be empty"
    );

    // Validate raw JSON schema contains mutated values
    let json_val: serde_json::Value =
        serde_json::from_str(&raw_content).expect("settings.json must be valid JSON");
    assert_eq!(
        json_val["tts"]["voice_index"], 42,
        "Raw JSON must contain mutated tts.voice_index == 42"
    );
    assert_eq!(
        json_val["llm"]["context_window"], 16384,
        "Raw JSON must contain mutated llm.context_window == 16384"
    );
    assert_eq!(
        json_val["appearance"]["accent_seed"], "#8B5CF6",
        "Raw JSON must contain mutated appearance.accent_seed"
    );

    // 5. Reload settings from disk via SUT: VoxSettings::load
    let reloaded = VoxSettings::load();

    // 6. Assert exact field-wise round-trip preservation across all domains
    assert_eq!(reloaded.appearance.theme, "dark");
    assert_eq!(reloaded.appearance.accent_seed, "#8B5CF6");
    assert_eq!(reloaded.audio.output_mode, AudioOutputMode::Headset);
    assert!((reloaded.vad.threshold - 0.72).abs() < 1e-5);
    assert!((reloaded.vad.ptt_noise_gate - 0.04).abs() < 1e-5);
    assert_eq!(reloaded.vad.vad_backend, VadBackendOption::Earshot);
    assert_eq!(reloaded.stt.active, SttActiveProvider::Cloud);
    assert!(!reloaded.stt.transliterate_enabled);
    assert_eq!(reloaded.llm.active, LlmActiveProvider::Server);
    assert!((reloaded.llm.temperature - 0.85).abs() < 1e-5);
    assert_eq!(reloaded.llm.context_window, 16384);
    assert_eq!(reloaded.tts.active, TtsActiveProvider::Supertonic);
    assert_eq!(reloaded.tts.voice_index, 42);
    assert!((reloaded.tts.speed - 1.15).abs() < 1e-5);
    assert_eq!(reloaded.tts.quality_steps, 8);
    assert_eq!(reloaded.interaction.pipeline_mode, PipelineMode::Realtime);
    assert!(reloaded.dictation.enabled);
    assert_eq!(reloaded.dictation.output_mode, DictationOutputMode::Tray);
    assert!(!reloaded.memory.context_retrieval_enabled);
    assert_eq!(reloaded.memory.top_k_facts, 15);

    assert!(
        Instant::now() < deadline,
        "test_settings_json_roundtrip_persistence exceeded 10s deadline"
    );
}

// ============================================================================
// Subtest 2: test_settings_malformed_fallback_to_default
// ============================================================================
#[test]
fn test_settings_malformed_fallback_to_default() {
    let deadline = Instant::now() + Duration::from_secs(10);
    let _guard = TempPathsGuard::new();

    let settings_path = vox_lib::utils::paths::settings_path();

    // Write completely malformed JSON to settings.json
    fs::write(&settings_path, b"{\"corrupt_json_unclosed: true, ")
        .expect("Failed to write corrupt settings file");

    // SUT: VoxSettings::load on malformed file
    let recovered = VoxSettings::load();

    // 1. Assert load does not panic and returns default settings
    let defaults = VoxSettings::default();
    assert_eq!(recovered.appearance.theme, defaults.appearance.theme);
    assert_eq!(recovered.audio.output_mode, defaults.audio.output_mode);
    assert_eq!(recovered.vad.threshold, defaults.vad.threshold);
    assert_eq!(recovered.llm.active, defaults.llm.active);
    assert_eq!(recovered.tts.voice_index, defaults.tts.voice_index);
    assert_eq!(
        recovered.memory.context_retrieval_enabled,
        defaults.memory.context_retrieval_enabled
    );

    // 2. Assert the corrupt file was backed up to settings.corrupt.<ts>.json
    let parent = settings_path
        .parent()
        .expect("settings_path parent must exist");
    let mut found_corrupt_backup = false;
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.starts_with("settings.corrupt.") && filename.ends_with(".json") {
                found_corrupt_backup = true;
                let backup_content =
                    fs::read_to_string(entry.path()).expect("Backup file must be readable");
                assert!(
                    backup_content.contains("corrupt_json_unclosed"),
                    "Backup file must contain original corrupt content"
                );
                break;
            }
        }
    }
    assert!(
        found_corrupt_backup,
        "A timestamped backup settings.corrupt.<ts>.json must be created on corrupt parse"
    );

    assert!(
        Instant::now() < deadline,
        "test_settings_malformed_fallback_to_default exceeded 10s deadline"
    );
}

// ============================================================================
// Subtest 3: test_settings_partial_section_recovery
// ============================================================================
#[test]
fn test_settings_partial_section_recovery() {
    let deadline = Instant::now() + Duration::from_secs(10);
    let _guard = TempPathsGuard::new();

    let settings_path = vox_lib::utils::paths::settings_path();

    // Create JSON with valid appearance and tts sections, but omitted/corrupted other fields
    let partial_json = serde_json::json!({
        "appearance": {
            "theme": "nordic_frost",
            "accent_seed": "#00DBE9"
        },
        "tts": {
            "active": "kokoro",
            "voice_index": 77,
            "quality_steps": 5,
            "speed": 1.05,
            "threads": 4
        },
        "unknown_junk_section": {
            "garbage": 12345
        }
    });

    fs::write(
        &settings_path,
        serde_json::to_string_pretty(&partial_json).unwrap(),
    )
    .expect("Failed to write partial settings file");

    // SUT: VoxSettings::load
    let recovered = VoxSettings::load();

    // Assert partial valid sections recovered
    assert_eq!(recovered.appearance.theme, "nordic_frost");
    assert_eq!(recovered.appearance.accent_seed, "#00DBE9");
    assert_eq!(recovered.tts.voice_index, 77);
    assert_eq!(recovered.tts.active, TtsActiveProvider::Kokoro);

    // Assert missing sections cleanly fell back to defaults without panic
    let defaults = VoxSettings::default();
    assert_eq!(recovered.vad.threshold, defaults.vad.threshold);
    assert_eq!(recovered.llm.active, defaults.llm.active);
    assert_eq!(recovered.dictation.enabled, defaults.dictation.enabled);

    assert!(
        Instant::now() < deadline,
        "test_settings_partial_section_recovery exceeded 10s deadline"
    );
}
