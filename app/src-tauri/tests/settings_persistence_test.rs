//! ============================================================================
//! settings_persistence_test.rs — settings mutations are applied, persisted to disk, reloaded field-for-field, and recovered from corrupt config files
//! ============================================================================
//! Category  : Integration (Seam 15 — settings persistence)
//! Component : src/config/, src/core/settings.rs, src/ipc/settings/mutation.rs, src/utils/paths.rs
//! Binary    : settings_persistence_test
//! Prereqs   : none (isolated temporary VOX_HOME per test)
//! Run       : cargo nextest run --test settings_persistence_test --release --test-threads=1
//! ============================================================================

mod common;

use std::{fs, time::Duration};

use common::paths::TempPathsGuard;
use vox_lib::{
    config::{
        apply_setting_mutation, apply_settings_patch_tree, build_settings_contract, classify_known,
        get_setting_reload_policy, is_secret_key, secret_keys, supported_keys_for_domain,
        SettingReloadPolicy, VoxSettings,
    },
    core::events::{PipelineMode, RemoteExecutionMode},
    pipeline::dictation::DictationOutputMode,
    services::vad::VadBackendOption,
};

/// PURPOSE:  Every live settings domain must accept its documented keys; a
///           key that silently stops resolving leaves the user unable to
///           change that setting from the UI.
/// SCENARIO: Apply one mutation per domain through the production
///           `apply_setting_mutation` entry point.
/// ASSERTS:  Every mutation returns `Ok(true)` for the live domains.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Remove the `"vad" => "threshold"` arm from
///           `apply_setting_mutation` — the VAD assertion would fail.
#[test]
fn setting_mutation_recognises_every_live_domain_key() {
    let _guard = TempPathsGuard::new();
    let mut settings = VoxSettings::default();

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
        apply_setting_mutation(
            &mut settings,
            "stt",
            "active",
            &serde_json::json!({
                "category": "cloud",
                "provider": "deepgram",
                "model": "nova-3"
            })
        )
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
        apply_setting_mutation(
            &mut settings,
            "llm",
            "active",
            &serde_json::json!({
                "category": "server",
                "provider": "ollama",
                "model": "qwen3.5:9b"
            })
        )
        .expect("llm.active mutation failed"),
        "llm.active should be recognized"
    );

    // TTS
    assert!(
        apply_setting_mutation(
            &mut settings,
            "tts",
            "active",
            &serde_json::json!({
                "category": "embedded",
                "model": "supertonic"
            })
        )
        .expect("tts.active mutation failed"),
        "tts.active should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "tts",
            "active_voice_id",
            &serde_json::json!("aria")
        )
        .expect("tts.active_voice_id mutation failed"),
        "tts.active_voice_id should be recognized"
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

    // Memory (merged scope)
    assert!(
        apply_setting_mutation(
            &mut settings,
            "memory",
            "private_mode",
            &serde_json::json!(true)
        )
        .expect("memory.private_mode mutation failed"),
        "memory.private_mode should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "memory",
            "max_context_share",
            &serde_json::json!(0.4)
        )
        .expect("memory.max_context_share mutation failed"),
        "memory.max_context_share should be recognized"
    );
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
    assert!(
        apply_setting_mutation(
            &mut settings,
            "memory",
            "semantic_similarity_cutoff",
            &serde_json::json!(0.55)
        )
        .expect("memory.semantic_similarity_cutoff mutation failed"),
        "memory.semantic_similarity_cutoff should be recognized"
    );

    // Interaction: web search toggle lives in the interaction scope.
    assert!(
        apply_setting_mutation(
            &mut settings,
            "interaction",
            "web_search_enabled",
            &serde_json::json!(false)
        )
        .expect("interaction.web_search_enabled mutation failed"),
        "interaction.web_search_enabled should be recognized"
    );
}

/// PURPOSE:  Zero backward compatibility: a retired settings key must not be
///           accepted, or a stale client would re-introduce a removed knob.
/// SCENARIO: Attempt mutations for the retired `tts.quality_steps`,
///           `working_memory.private_mode` and `personal_memory.top_k_facts`
///           keys.
/// ASSERTS:  Each retired key resolves without error but returns `false`.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Re-add a `"tts" => "quality_steps"` arm to
///           `apply_setting_mutation` — the assertion would fail.
#[test]
fn retired_setting_keys_are_rejected() {
    let _guard = TempPathsGuard::new();
    let mut settings = VoxSettings::default();

    // Diffusion steps are a fixed per-provider constant, not a setting.
    // A stale client must not be able to reintroduce the knob.
    assert!(
        !apply_setting_mutation(&mut settings, "tts", "quality_steps", &serde_json::json!(8))
            .expect("tts.quality_steps mutation should not error"),
        "tts.quality_steps must no longer be a recognized setting key"
    );
    assert!(
        !apply_setting_mutation(
            &mut settings,
            "working_memory",
            "private_mode",
            &serde_json::json!(true)
        )
        .expect("working_memory.private_mode must resolve without error"),
        "working_memory.private_mode must not be recognized after the scope merge"
    );
    assert!(
        !apply_setting_mutation(
            &mut settings,
            "personal_memory",
            "top_k_facts",
            &serde_json::json!(15)
        )
        .expect("personal_memory.top_k_facts must resolve without error"),
        "personal_memory.top_k_facts must not be recognized after the scope merge"
    );
}

/// PURPOSE:  A patch tree sent by the frontend on a retired scope must be
///           rejected outright; silently applying half of it would leave the
///           settings in a state the UI never asked for.
/// SCENARIO: Apply a patch tree targeting the retired `working_memory` scope.
/// ASSERTS:  `apply_settings_patch_tree` returns an error.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Make `apply_settings_patch_tree` ignore unknown scopes instead
///           of failing — the assertion would fail.
#[test]
fn patch_tree_rejects_retired_scope() {
    let _guard = TempPathsGuard::new();
    let mut settings = VoxSettings::default();

    assert!(
        apply_settings_patch_tree(
            &mut settings,
            &serde_json::json!({ "working_memory": { "private_mode": true } }),
        )
        .is_err(),
        "a patch on the retired working_memory scope must be rejected"
    );
}

/// PURPOSE:  Saving settings must produce the three config files the app
///           reads at boot; a missing `providers.jsonc` or `agent.jsonc` would
///           wipe the user's provider keys and persona on restart.
/// SCENARIO: Mutate settings, call `VoxSettings::save`, then inspect the
///           resulting files on disk.
/// ASSERTS:  `settings.json`, `providers.jsonc` and `agent.jsonc` all exist;
///           `providers.jsonc` is mode 0600; the raw settings JSON contains
///           the mutated values.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip writing `agent.jsonc` in `VoxSettings::save` — the
///           existence assertion would fail.
#[test]
fn settings_save_writes_config_files_with_owner_only_permissions() {
    let _guard = TempPathsGuard::new();
    let mut settings = VoxSettings::default();

    assert!(
        apply_setting_mutation(
            &mut settings,
            "tts",
            "active_voice_id",
            &serde_json::json!("aria")
        )
        .expect("tts.active_voice_id mutation failed"),
        "tts.active_voice_id should be recognized"
    );
    assert!(
        apply_setting_mutation(
            &mut settings,
            "llm",
            "active",
            &serde_json::json!({
                "category": "server",
                "provider": "ollama",
                "model": "qwen3.5:9b"
            })
        )
        .expect("llm.active mutation failed"),
        "llm.active should be recognized"
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

    let save_res = settings.save();
    assert!(save_res.is_ok(), "VoxSettings::save must return Ok(())");

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
    let json_val: serde_json::Value = vox_lib::utils::jsonc::from_jsonc_str(&raw_content)
        .expect("settings.jsonc must be valid JSONC");
    assert_eq!(
        json_val["tts"]["active_voice_id"], "aria",
        "Raw JSON must contain mutated tts.active_voice_id == aria"
    );
    assert_eq!(
        json_val["llm"]["active"]["category"], "server",
        "Raw JSON must contain mutated llm.active.category == server"
    );
    assert_eq!(
        json_val["appearance"]["accent_seed"], "#8B5CF6",
        "Raw JSON must contain mutated appearance.accent_seed"
    );

    // Verify providers.jsonc existence and 0600 permissions
    let providers_path = vox_lib::utils::paths::providers_path();
    assert!(
        providers_path.exists(),
        "providers.jsonc must exist at paths::providers_path() after save"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&providers_path).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "providers.jsonc must be chmod 0600"
        );
    }

    // Verify agent.jsonc exists
    let agent_path = vox_lib::utils::paths::agent_path();
    assert!(
        agent_path.exists(),
        "agent.jsonc must exist at paths::agent_path() after save"
    );
}

/// PURPOSE:  A saved setting must survive a process restart; a lossy
///           round-trip silently reverts the user's configuration.
/// SCENARIO: Mutate settings across every domain, save, reload from disk and
///           compare field by field.
/// ASSERTS:  Every reloaded field equals the mutated value.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Drop the `memory` section from the serialized settings — the
///           reloaded `memory.*` assertions would fail.
#[test]
fn settings_reload_preserves_every_mutated_field() {
    let _guard = TempPathsGuard::new();
    let mut settings = VoxSettings::default();

    assert!(apply_setting_mutation(
        &mut settings,
        "appearance",
        "theme",
        &serde_json::json!("dark")
    )
    .expect("appearance.theme mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "appearance",
        "accent_seed",
        &serde_json::json!("#8B5CF6")
    )
    .expect("appearance.accent_seed mutation failed"));
    assert!(
        apply_setting_mutation(&mut settings, "vad", "threshold", &serde_json::json!(0.72))
            .expect("vad.threshold mutation failed")
    );
    assert!(apply_setting_mutation(
        &mut settings,
        "vad",
        "ptt_noise_gate",
        &serde_json::json!(0.04)
    )
    .expect("vad.ptt_noise_gate mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "vad",
        "vad_backend",
        &serde_json::json!("earshot")
    )
    .expect("vad.vad_backend mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "stt",
        "active",
        &serde_json::json!({
            "category": "cloud",
            "provider": "deepgram",
            "model": "nova-3"
        })
    )
    .expect("stt.active mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "stt",
        "transliterate_enabled",
        &serde_json::json!(false)
    )
    .expect("stt.transliterate_enabled mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "llm",
        "active",
        &serde_json::json!({
            "category": "server",
            "provider": "ollama",
            "model": "qwen3.5:9b"
        })
    )
    .expect("llm.active mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "tts",
        "active",
        &serde_json::json!({
            "category": "embedded",
            "model": "supertonic"
        })
    )
    .expect("tts.active mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "tts",
        "active_voice_id",
        &serde_json::json!("aria")
    )
    .expect("tts.active_voice_id mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "interaction",
        "pipeline_mode",
        &serde_json::json!("realtime")
    )
    .expect("interaction.pipeline_mode mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "interaction",
        "web_search_enabled",
        &serde_json::json!(false)
    )
    .expect("interaction.web_search_enabled mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "dictation",
        "enabled",
        &serde_json::json!(true)
    )
    .expect("dictation.enabled mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "dictation",
        "output_mode",
        &serde_json::json!("tray")
    )
    .expect("dictation.output_mode mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "memory",
        "private_mode",
        &serde_json::json!(true)
    )
    .expect("memory.private_mode mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "memory",
        "max_context_share",
        &serde_json::json!(0.4)
    )
    .expect("memory.max_context_share mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "memory",
        "context_retrieval_enabled",
        &serde_json::json!(false)
    )
    .expect("memory.context_retrieval_enabled mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "memory",
        "top_k_facts",
        &serde_json::json!(15)
    )
    .expect("memory.top_k_facts mutation failed"));
    assert!(apply_setting_mutation(
        &mut settings,
        "memory",
        "semantic_similarity_cutoff",
        &serde_json::json!(0.55)
    )
    .expect("memory.semantic_similarity_cutoff mutation failed"));

    assert!(
        settings.save().is_ok(),
        "VoxSettings::save must return Ok(())"
    );

    let reloaded = VoxSettings::load();

    assert_eq!(reloaded.appearance.theme, "dark");
    assert_eq!(reloaded.appearance.accent_seed, "#8B5CF6");
    assert!((reloaded.vad.threshold - 0.72).abs() < 1e-5);
    assert!((reloaded.vad.ptt_noise_gate - 0.04).abs() < 1e-5);
    assert_eq!(reloaded.vad.vad_backend, VadBackendOption::Earshot);
    assert_eq!(reloaded.stt.active.category, "cloud");
    assert_eq!(reloaded.stt.active.provider.as_deref(), Some("deepgram"));
    assert_eq!(reloaded.stt.active.model, "nova-3");
    assert!(!reloaded.stt.transliterate_enabled);
    assert_eq!(reloaded.llm.active.category, "server");
    assert_eq!(reloaded.llm.active.provider.as_deref(), Some("ollama"));
    assert_eq!(reloaded.llm.active.model, "qwen3.5:9b");
    assert_eq!(reloaded.tts.active.category, "embedded");
    assert_eq!(reloaded.tts.active.model, "supertonic");
    assert_eq!(reloaded.tts.active_voice_id, "aria");
    assert_eq!(reloaded.interaction.pipeline_mode, PipelineMode::Realtime);
    assert!(!reloaded.interaction.web_search_enabled);
    assert!(reloaded.dictation.enabled);
    assert_eq!(reloaded.dictation.output_mode, DictationOutputMode::Tray);
    assert!(reloaded.memory.private_mode);
    assert!((reloaded.memory.max_context_share - 0.4).abs() < 1e-5);
    assert!(!reloaded.memory.context_retrieval_enabled);
    assert_eq!(reloaded.memory.top_k_facts, 15);
    assert!((reloaded.memory.semantic_similarity_cutoff - 0.55).abs() < 1e-5);
}

/// PURPOSE:  An unparsable settings file must not crash the app at boot; the
///           loader has to fall back to defaults so the user can still open
///           the settings UI and repair the file.
/// SCENARIO: Write truncated JSON to `settings.jsonc` and load.
/// ASSERTS:  `VoxSettings::load` returns settings equal to the defaults for
///           the inspected fields.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Make `VoxSettings::load` propagate the parse error — the test
///           would panic instead of returning defaults.
#[test]
fn malformed_settings_file_falls_back_to_defaults() {
    let _guard = TempPathsGuard::new();
    let settings_path = vox_lib::utils::paths::settings_path();

    fs::write(&settings_path, b"{\"corrupt_json_unclosed: true, ")
        .expect("Failed to write corrupt settings file");

    let recovered = VoxSettings::load();

    let defaults = VoxSettings::default();
    assert_eq!(recovered.appearance.theme, defaults.appearance.theme);
    assert_eq!(recovered.vad.threshold, defaults.vad.threshold);
    assert_eq!(recovered.llm.active, defaults.llm.active);
    assert_eq!(recovered.tts.active_voice_id, defaults.tts.active_voice_id);
    assert_eq!(recovered.memory.private_mode, defaults.memory.private_mode);
    assert_eq!(
        recovered.memory.context_retrieval_enabled,
        defaults.memory.context_retrieval_enabled
    );
}

/// PURPOSE:  The corrupt settings file must be preserved on disk, otherwise
///           the user's hand-edited configuration is destroyed with no way
///           to recover it.
/// SCENARIO: Load a malformed settings file and then look for the timestamped
///           backup in the config directory.
/// ASSERTS:  A `settings.corrupt.<ts>.jsonc` backup exists and contains the
///           original corrupt content.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Remove the corrupt-file backup from `VoxSettings::load` — the
///           backup assertion would fail.
#[test]
fn malformed_settings_file_is_backed_up_with_original_content() {
    let _guard = TempPathsGuard::new();
    let settings_path = vox_lib::utils::paths::settings_path();

    fs::write(&settings_path, b"{\"corrupt_json_unclosed: true, ")
        .expect("Failed to write corrupt settings file");

    let _ = VoxSettings::load();

    let parent = settings_path
        .parent()
        .expect("settings_path parent must exist");
    let mut found_corrupt_backup = false;
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.starts_with("settings.corrupt.")
                && (filename.ends_with(".jsonc") || filename.ends_with(".json"))
            {
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
        "A timestamped backup settings.corrupt.<ts>.jsonc must be created on corrupt parse"
    );
}

/// PURPOSE:  A settings file that only contains some sections must load the
///           present sections and default the missing ones; a partial file
///           must not wipe the whole configuration.
/// SCENARIO: Write a JSON file containing only `appearance` and `tts`, plus
///           an unknown section, and load it.
/// ASSERTS:  The present sections keep their values and the missing sections
///           equal the defaults.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Make the loader discard every section that is not fully
///           populated — the `appearance.theme` assertion would fail.
#[test]
fn partial_settings_file_keeps_valid_sections_and_defaults_the_rest() {
    let _guard = TempPathsGuard::new();
    let settings_path = vox_lib::utils::paths::settings_path();

    let partial_json = serde_json::json!({
        "appearance": {
            "theme": "nordic_frost",
            "accent_seed": "#00DBE9"
        },
        "tts": {
            "active": {
                "category": "embedded",
                "model": "kokoro"
            },
            "active_voice_id": "serene"
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

    let recovered = VoxSettings::load();

    assert_eq!(recovered.appearance.theme, "nordic_frost");
    assert_eq!(recovered.appearance.accent_seed, "#00DBE9");
    assert_eq!(recovered.tts.active_voice_id, "serene");
    assert_eq!(recovered.tts.active.model, "kokoro");

    let defaults = VoxSettings::default();
    assert_eq!(recovered.vad.threshold, defaults.vad.threshold);
    assert_eq!(recovered.llm.active, defaults.llm.active);
    assert_eq!(recovered.dictation.enabled, defaults.dictation.enabled);
}

/// PURPOSE:  The three config files are JSONC by contract; a loader that
///           rejects comments makes every commented config file unreadable
///           and the app silently resets its configuration.
/// SCENARIO: Write comment-bearing JSONC to `settings.jsonc`,
///           `providers.jsonc` and `agent.jsonc`, then load.
/// ASSERTS:  Values from all three files are parsed, including the provider
///           URL, model, cloud key and persona prompt.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Parse the config files with a strict JSON parser — the first
///           assertion would panic on the `//` comment.
#[test]
fn jsonc_comments_are_parsed_in_all_three_config_files() {
    let _guard = TempPathsGuard::new();
    let settings_path = vox_lib::utils::paths::settings_path();
    let providers_path = vox_lib::utils::paths::providers_path();
    let agent_path = vox_lib::utils::paths::agent_path();

    let settings_jsonc = r##"
    // Global hardware and audio configuration
    {
        // DSP noise gate threshold
        "vad": {
            "threshold": 0.65,
            "ptt_noise_gate": 0.02,
            "vad_backend": "earshot",
            "silence_duration_ms": 350,
            "speech_onset_ms": 32,
            "max_speech_duration_s": 25
        },
        "appearance": {
            "theme": "dark_modern",
            "accent_seed": "#10B981"
        }
    }
    "##;

    let providers_jsonc = r#"
    // Cloud and server model endpoints
    {
        /* Remote LLM provider configuration */
        "llm": {
            "servers": [
                {
                    "id": "ollama-server",
                    "name": "Ollama",
                    "url": "http://100.67.98.126:11435",
                    "model": "qwen3.5:9b",
                    "protocol": "ollama"
                }
            ],
            "cloud_keys": {
                "together": "test-key-12345"
            }
        }
    }
    "#;

    let agent_jsonc = r#"
    // Persona prompts
    {
        "persona": {
            "modular_prompt": "You are Vox, an AI assistant.",
            "realtime_prompt": "Be concise and accurate."
        }
    }
    "#;

    fs::write(&settings_path, settings_jsonc).expect("Failed to write settings.jsonc");
    fs::write(&providers_path, providers_jsonc).expect("Failed to write providers.jsonc");
    fs::write(&agent_path, agent_jsonc).expect("Failed to write agent.jsonc");

    let loaded = VoxSettings::load();

    assert!((loaded.vad.threshold - 0.65).abs() < 1e-5);
    assert_eq!(loaded.vad.vad_backend, VadBackendOption::Earshot);
    assert_eq!(loaded.appearance.theme, "dark_modern");
    assert_eq!(loaded.appearance.accent_seed, "#10B981");

    assert_eq!(
        loaded.providers.llm.servers[0].url,
        "http://100.67.98.126:11435"
    );
    assert_eq!(loaded.providers.llm.servers[0].model, "qwen3.5:9b");
    assert_eq!(
        loaded
            .providers
            .llm
            .cloud_keys
            .get("together")
            .map(|s| s.as_str()),
        Some("test-key-12345")
    );
    assert_eq!(
        loaded.persona.modular_prompt,
        "You are Vox, an AI assistant."
    );
}

/// PURPOSE:  A type error in one section of `settings.jsonc` must not reset
///           the whole file; only the broken section falls back to defaults.
/// SCENARIO: Write a settings file whose `vad.threshold` is a string while
///           the `appearance` section is valid, then load.
/// ASSERTS:  `vad.threshold` equals the production default and
///           `appearance` keeps its custom values.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Make the whole-file parse fail on the first type error — the
///           `appearance.theme` assertion would fail.
#[test]
fn settings_type_error_falls_back_to_default_section_only() {
    let _guard = TempPathsGuard::new();
    let settings_path = vox_lib::utils::paths::settings_path();

    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create config dir");
    }

    let corrupt_settings = r##"{
        "vad": {
            "threshold": "not_a_float",
            "silence_duration_ms": 500
        },
        "appearance": {
            "theme": "custom_purple",
            "accent_seed": "#8B5CF6"
        }
    }"##;

    fs::write(&settings_path, corrupt_settings).expect("Failed to write corrupt settings.jsonc");

    let loaded = VoxSettings::load();

    assert_eq!(loaded.appearance.theme, "custom_purple");
    assert_eq!(loaded.appearance.accent_seed, "#8B5CF6");

    assert_eq!(
        loaded.vad.threshold,
        vox_lib::core::defaults::DEFAULT_VAD_THRESHOLD
    );
}

/// PURPOSE:  A corrupt `settings.jsonc` must not take the sibling config
///           files down with it; otherwise one bad edit wipes the user's
///           provider keys and persona.
/// SCENARIO: Write a type-corrupt `settings.jsonc` next to valid
///           `providers.jsonc` and `agent.jsonc`, then load.
/// ASSERTS:  The provider cloud key and the persona prompt survive intact.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Skip loading `providers.jsonc` when `settings.jsonc` fails to
///           parse — the cloud-key assertion would fail.
#[test]
fn valid_sibling_config_files_survive_settings_corruption() {
    let _guard = TempPathsGuard::new();
    let settings_path = vox_lib::utils::paths::settings_path();
    let providers_path = vox_lib::utils::paths::providers_path();
    let agent_path = vox_lib::utils::paths::agent_path();

    let corrupt_settings = r##"{
        "vad": {
            "threshold": "not_a_float"
        }
    }"##;

    let valid_providers = r#"{
        "llm": {
            "cloud_keys": {
                "together": "my-secret-key-999"
            }
        }
    }"#;

    let valid_agent = r#"{
        "persona": {
            "modular_prompt": "Custom persona prompt for recovery test"
        }
    }"#;

    fs::write(&settings_path, corrupt_settings).expect("Failed to write corrupt settings.jsonc");
    fs::write(&providers_path, valid_providers).expect("Failed to write valid providers.jsonc");
    fs::write(&agent_path, valid_agent).expect("Failed to write valid agent.jsonc");

    let loaded = VoxSettings::load();

    assert_eq!(
        loaded
            .providers
            .llm
            .cloud_keys
            .get("together")
            .map(|s| s.as_str()),
        Some("my-secret-key-999"),
        "Valid providers.jsonc must be preserved during settings type-error recovery"
    );
    assert_eq!(
        loaded.persona.modular_prompt, "Custom persona prompt for recovery test",
        "Valid agent.jsonc must be preserved during settings type-error recovery"
    );
}

/// PURPOSE:  A settings key classified as `WorkerCommand` must have a
///           matching arm in the worker dispatch; without one the setting is
///           accepted but never reaches the worker that owns it.
/// SCENARIO: For every worker-command setting key, look up the reload policy
///           and the dispatch arm.
/// ASSERTS:  Each key has policy `WorkerCommand` and the dispatch has an arm
///           for it.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Remove the `"tts" => "active_voice_id"` arm from
///           `dispatch_worker_command` — the assertion would fail.
#[test]
fn worker_command_policy_keys_have_dispatch_arms() {
    use vox_lib::config::{
        dispatch_worker_command_has_arm, get_setting_reload_policy, SettingReloadPolicy,
    };

    let worker_keys = [
        ("tts", "active_voice_id"),
        ("tts", "voice_id"),
        ("tts", "voice"),
        ("vad", "threshold"),
        ("vad", "ptt_noise_gate"),
        ("vad", "silence_duration_ms"),
        ("vad", "speech_onset_ms"),
    ];

    for (domain, key) in worker_keys {
        assert_eq!(
            get_setting_reload_policy(domain, key),
            SettingReloadPolicy::WorkerCommand,
            "{}.{} is expected to have SettingReloadPolicy::WorkerCommand",
            domain,
            key
        );
        assert!(
            dispatch_worker_command_has_arm(domain, key),
            "{}.{} is classified WorkerCommand but dispatch_worker_command has no arm",
            domain,
            key
        );
    }
}

/// PURPOSE:  A setting that has no worker command must keep its declared
///           reload policy; misclassifying it as `WorkerCommand` would make
///           the app cycle the engine (or do nothing) on an unrelated save.
/// SCENARIO: Look up the reload policy for a non-worker VAD key and for the
///           provider API keys.
/// ASSERTS:  `vad.max_speech_duration_s` is not `WorkerCommand` and
///           `llm.cloud_keys` is `Hot`.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Classify `llm.cloud_keys` as `WorkerCommand` — the assertion
///           would fail.
#[test]
fn non_worker_keys_keep_their_reload_policy() {
    use vox_lib::config::{get_setting_reload_policy, SettingReloadPolicy};

    assert_ne!(
        get_setting_reload_policy("vad", "max_speech_duration_s"),
        SettingReloadPolicy::WorkerCommand,
        "vad.max_speech_duration_s must not be WorkerCommand until a VadCommand variant exists"
    );
    assert_eq!(
        get_setting_reload_policy("llm", "cloud_keys"),
        SettingReloadPolicy::Hot,
        "llm.cloud_keys must be Hot so saving a key does not cycle the engine"
    );
}

/// PURPOSE:  A patch tree with several keys must apply all of them and report
///           every key it changed; the frontend uses that list to refresh
///           only the affected panels.
/// SCENARIO: Apply a two-domain patch tree and inspect the returned change
///           list and the resulting values.
/// ASSERTS:  Two changes are reported, both `(domain, key)` pairs are present,
///           and both values are applied.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Return only the first changed key from
///           `apply_settings_patch_tree` — the `changed.len()` assertion
///           would fail.
#[test]
fn patch_tree_applies_multiple_keys_atomically() {
    let _guard = TempPathsGuard::new();
    let mut settings = VoxSettings::default();
    let patch = serde_json::json!({
        "appearance": { "theme": "dark" },
        "vad": { "threshold": 0.7 }
    });

    let changed = apply_settings_patch_tree(&mut settings, &patch).expect("patch should apply");

    assert_eq!(changed.len(), 2);
    assert!(changed.contains(&("appearance".to_string(), "theme".to_string())));
    assert!(changed.contains(&("vad".to_string(), "threshold".to_string())));
    assert_eq!(settings.appearance.theme, "dark");
    assert_eq!(settings.vad.threshold, 0.7);
}

/// PURPOSE:  A patch tree containing an unknown key must be rejected and must
///           leave the settings untouched; a partially applied patch would
///           leave the app in a state nobody requested.
/// SCENARIO: Apply a patch tree with an unknown `appearance.nope` key after a
///           successful patch.
/// ASSERTS:  The call errors and the previously applied value is unchanged.
/// CLASS:    error-path
/// REQUIRES: none
/// MUTANT:   Apply known keys and ignore unknown ones instead of failing — the
///           `is_err()` assertion would fail.
#[test]
fn patch_tree_failure_leaves_settings_untouched() {
    let _guard = TempPathsGuard::new();
    let mut settings = VoxSettings::default();
    let patch = serde_json::json!({
        "appearance": { "theme": "dark" },
        "vad": { "threshold": 0.7 }
    });
    apply_settings_patch_tree(&mut settings, &patch).expect("patch should apply");

    let before = settings.clone();
    let bad = serde_json::json!({ "appearance": { "nope": true } });

    assert!(apply_settings_patch_tree(&mut settings, &bad).is_err());
    assert_eq!(settings.appearance.theme, before.appearance.theme);
}

// ============================================================================
// Subtest 12: Remote Settings Domain Contract & Mutation
// ============================================================================

/// PURPOSE:  The `remote` domain must be registered in the settings contract and mutable via the production router.
/// SCENARIO: Query contract for remote domain; check reload policies; mutate keys; verify save/load roundtrip.
/// ASSERTS:  Contract contains accepts_controllers (Hot) and execution_mode (Restart); invalid mode rejected.
/// CLASS:    contract
/// REQUIRES: none
/// MUTANT:   Remove remote domain from supported_domains list in config/contract.rs.
#[test]
fn remote_domain_in_contract_and_mutable() {
    let _guard = TempPathsGuard::new();
    let domain = "remote";
    let key_accepts = "accepts_controllers";
    let key_mode = "execution_mode";

    // 1. Contract registration
    let contract = build_settings_contract();
    assert!(contract.supported_keys.contains_key(domain));
    let contract_keys = &contract.supported_keys[domain];
    assert!(contract_keys.contains(&key_accepts.to_string()));
    assert!(contract_keys.contains(&key_mode.to_string()));

    let policies = contract.reload_policies.get(domain).expect("policies");
    assert_eq!(policies.get(key_accepts).map(|s| s.as_str()), Some("hot"));
    assert_eq!(policies.get(key_mode).map(|s| s.as_str()), Some("restart"));

    assert_eq!(
        classify_known(domain, key_accepts),
        Some(SettingReloadPolicy::Hot)
    );
    assert_eq!(
        classify_known(domain, key_mode),
        Some(SettingReloadPolicy::Restart)
    );
    assert_eq!(
        get_setting_reload_policy(domain, key_accepts),
        SettingReloadPolicy::Hot
    );
    assert_eq!(
        get_setting_reload_policy(domain, key_mode),
        SettingReloadPolicy::Restart
    );

    // 2. Defaults
    let defaults = VoxSettings::default();
    assert!(!defaults.remote.accepts_controllers);
    assert_eq!(defaults.remote.execution_mode, RemoteExecutionMode::Local);

    // 3. Mutation router
    let mut settings = VoxSettings::default();
    assert!(
        apply_setting_mutation(&mut settings, domain, key_accepts, &serde_json::json!(true))
            .unwrap()
    );
    assert!(settings.remote.accepts_controllers);
    assert!(apply_setting_mutation(
        &mut settings,
        domain,
        key_mode,
        &serde_json::json!("remote")
    )
    .unwrap());
    assert_eq!(settings.remote.execution_mode, RemoteExecutionMode::Remote);

    // Reject unknown execution mode
    assert!(apply_setting_mutation(
        &mut settings,
        domain,
        key_mode,
        &serde_json::json!("invalid_mode")
    )
    .is_err());
    assert_eq!(settings.remote.execution_mode, RemoteExecutionMode::Remote);

    // 4. Persistence round-trip
    settings.save().expect("save");
    let reloaded = VoxSettings::load();
    assert!(reloaded.remote.accepts_controllers);
    assert_eq!(reloaded.remote.execution_mode, RemoteExecutionMode::Remote);

    // 5. Missing remote section falls back to defaults
    let settings_path = vox_lib::utils::paths::settings_path();
    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "appearance": { "theme": "nordic_frost" }
        }))
        .unwrap(),
    )
    .unwrap();
    let recovered = VoxSettings::load();
    assert_eq!(recovered.appearance.theme, "nordic_frost");
    assert!(!recovered.remote.accepts_controllers);
    assert_eq!(recovered.remote.execution_mode, RemoteExecutionMode::Local);
}

// ============================================================================
// Subtest 13: Azure Speech Key Secret Classification & Masking
// ============================================================================

/// PURPOSE:  Azure Speech key must be classified as a secret, masked in logs, and preserved in storage.
/// SCENARIO: Verify secret key registration; run update_setting for azure_tts while capturing logs.
/// ASSERTS:  Secret is registered; logs contain `[redacted]`, not the raw key; state holds the raw key.
/// CLASS:    suppression
/// REQUIRES: none
/// MUTANT:   Remove azure_tts from secret_keys() allow-list.
#[tokio::test]
async fn azure_speech_key_treated_as_secret_and_masked_in_logs() {
    tokio::time::timeout(Duration::from_secs(30), async {
        assert!(is_secret_key("tts", "azure_tts"));
        assert!(secret_keys()
            .iter()
            .any(|(d, k)| *d == "tts" && *k == "azure_tts"));
        for (d, k) in secret_keys() {
            assert!(supported_keys_for_domain(d).contains(k));
        }

        let _guard = TempPathsGuard::new();
        let (app, state) = common::harness::get_test_app_and_state().await;
        let mutating_app = app.clone();

        common::log_capture::install();
        let _ = common::log_capture::take();

        state
            .restart_runner
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let real_key = "azure-super-secret-key-123";
        let azure = vox_lib::services::tts::TtsAzureConfig {
            region_or_endpoint: "eastus".to_string(),
            voice: "en-US-JennyNeural".to_string(),
            key: real_key.to_string(),
            ..Default::default()
        };
        let value = serde_json::to_value(&azure).unwrap();

        let result = vox_lib::ipc::settings::update_setting(
            "tts".to_string(),
            "azure_tts".to_string(),
            value,
            mutating_app,
        )
        .await
        .unwrap();
        assert!(result.applied);

        let captured = common::log_capture::take_text();
        assert!(!captured.contains(real_key));
        assert!(captured.contains("[redacted]"));

        let stored = state
            .settings
            .read()
            .unwrap()
            .providers
            .tts
            .azure_tts
            .key
            .clone();
        assert_eq!(stored, real_key);

        state
            .restart_runner
            .store(false, std::sync::atomic::Ordering::SeqCst);
    })
    .await
    .expect("timeout");
}
