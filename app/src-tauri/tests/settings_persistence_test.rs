//! ============================================================================
//! settings_persistence_test.rs — Settings Save/Load Roundtrip & Provider Isolation
//! ============================================================================
//! Category     : Integration Test
//! Component    : Core Settings (`vox_lib::core::settings`)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    : cargo test --test settings_persistence_test
//! Metrics      : JSON serialization roundtrip, disk file I/O, cross-provider field isolation, & corruption resilience
//! ============================================================================

use std::fs;
use vox_lib::core::settings::{
    DictationInteractionMode, DictationOutputMode, InteractionMode, LlmActiveProvider,
    LlmEmbeddedConfig, LlmRemoteConfig, LlmSettings, PipelineMode, VadBackendOption, VoxSettings,
};

// ─── 1. Default Settings Initialization Test ──────────────────────────────────

#[test]
fn test_settings_default_values_correctness() {
    let s = VoxSettings::default();

    // Default interaction modes
    assert_eq!(s.interaction.mode, InteractionMode::Passive);
    assert_eq!(s.interaction.pipeline_mode, PipelineMode::Modular);

    // Default dictation settings
    assert!(s.dictation.enabled);
    assert_eq!(s.dictation.interaction_mode, DictationInteractionMode::Ptt);
    assert_eq!(s.dictation.output_mode, DictationOutputMode::Paste);
    assert_eq!(s.dictation.hotkey, "Alt+Space");

    // Default VAD backend option
    assert_eq!(s.vad.backend, VadBackendOption::TenVad);

    // Default LLM provider config is Embedded
    assert_eq!(s.llm.active, LlmActiveProvider::Embedded);
    assert_eq!(s.llm.context_window, 2048);
    assert_eq!(s.llm.threads, 4);

    // Default Appearance settings
    assert_eq!(s.appearance.theme, "dark");
}

// ─── 2. Serde JSON In-Memory Roundtrip Test ──────────────────────────────────

#[test]
fn test_settings_serde_json_roundtrip() {
    let mut original = VoxSettings::default();

    // Mutate custom values
    original.interaction.mode = InteractionMode::PTT;
    original.dictation.interaction_mode = DictationInteractionMode::Passive;
    original.dictation.output_mode = DictationOutputMode::Clipboard;
    original.llm.context_window = 4096;
    original.llm.threads = 8;
    original.vad.threshold = 0.65;
    original.appearance.theme = "cyberpunk".to_string();

    // Serialize to JSON
    let json_str =
        serde_json::to_string_pretty(&original).expect("Failed to serialize VoxSettings");
    assert!(!json_str.is_empty());

    // Deserialize back from JSON
    let deserialized: VoxSettings =
        serde_json::from_str(&json_str).expect("Failed to deserialize VoxSettings");

    // Assert exact equality
    assert_eq!(deserialized.interaction.mode, InteractionMode::PTT);
    assert_eq!(
        deserialized.dictation.interaction_mode,
        DictationInteractionMode::Passive
    );
    assert_eq!(
        deserialized.dictation.output_mode,
        DictationOutputMode::Clipboard
    );
    assert_eq!(deserialized.llm.context_window, 4096);
    assert_eq!(deserialized.llm.threads, 8);
    assert_eq!(deserialized.vad.threshold, 0.65);
    assert_eq!(deserialized.appearance.theme, "cyberpunk");
}

// ─── 3. File System Disk Save / Load Roundtrip Test ──────────────────────────

#[test]
fn test_settings_disk_save_load_roundtrip_tempfile() {
    let temp_dir = std::env::temp_dir();
    let temp_file_path = temp_dir.join(format!("vox_test_settings_{}.json", std::process::id()));

    let mut settings = VoxSettings::default();
    settings.llm.embedded.model = "qwen_2_5_custom_q4".to_string();
    settings.llm.temperature = 0.35;
    settings.dictation.enabled = false;

    // Atomic write simulation (tmp -> target)
    let tmp_path = temp_file_path.with_extension("tmp");
    let json_bytes = serde_json::to_string_pretty(&settings).unwrap();
    fs::write(&tmp_path, &json_bytes).unwrap();
    fs::rename(&tmp_path, &temp_file_path).unwrap();

    // Read back and parse
    let content = fs::read_to_string(&temp_file_path).unwrap();
    let loaded: VoxSettings = serde_json::from_str(&content).unwrap();

    assert_eq!(loaded.llm.embedded.model, "qwen_2_5_custom_q4");
    assert_eq!(loaded.llm.temperature, 0.35);
    assert!(!loaded.dictation.enabled);

    // Clean up temp file
    let _ = fs::remove_file(&temp_file_path);
}

// ─── 4. Cross-Provider Setting Field Isolation Test ──────────────────────────

#[test]
fn test_llm_provider_switch_field_isolation() {
    let mut llm = LlmSettings {
        active: LlmActiveProvider::Embedded,
        embedded: LlmEmbeddedConfig {
            model: "llama_3_2_reasoning_q4".to_string(),
        },
        context_window: 4096,
        ..Default::default()
    };

    let embedded_ctx = llm.effective_ctx_size();
    assert_eq!(embedded_ctx, 4096);

    // 2. Switch to Server (Ollama)
    llm.active = LlmActiveProvider::Server;
    llm.server = LlmRemoteConfig {
        base_url: "http://100.86.62.14:11434".to_string(),
        model: "llama3.1:8b".to_string(),
        api_key: None,
        provider_name: Some("ollama".to_string()),
    };

    // Verify embedded config remains intact, but effective_ctx_size returns floor (8192)
    assert_eq!(llm.context_window, 4096);
    assert_eq!(llm.effective_ctx_size(), 8192);

    // 3. Switch back to Embedded
    llm.active = LlmActiveProvider::Embedded;
    assert_eq!(llm.effective_ctx_size(), 4096);
    assert_eq!(llm.active_model(), "llama_3_2_reasoning_q4");
    assert_eq!(llm.server.model, "llama3.1:8b");
}

// ─── 5. Corrupt & Partial JSON Resilience Test ────────────────────────────────

#[test]
fn test_corrupt_and_partial_json_resilience() {
    // Malformed JSON should return error on parse
    let corrupt_json = "{ \"appearance\": { \"theme\": \"dark\", ";
    let result: Result<VoxSettings, _> = serde_json::from_str(corrupt_json);
    assert!(result.is_err());

    // Partial JSON with missing fields should leverage serde(default) without crashing
    let partial_json = r#"{
        "appearance": {
            "theme": "nord"
        }
    }"#;

    let parsed: VoxSettings =
        serde_json::from_str(partial_json).expect("Partial JSON should leverage serde(default)");
    assert_eq!(parsed.appearance.theme, "nord");
    assert_eq!(parsed.llm.context_window, 2048); // Restored default
    assert_eq!(parsed.interaction.mode, InteractionMode::Passive);
    assert!(parsed.dictation.enabled);
}
