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
    InteractionMode, LlmProviderConfig, LlmSettings, PipelineMode, VadBackendOption, VoxSettings,
};

// ─── 1. Default Settings Initialization Test ──────────────────────────────────

#[test]
fn test_settings_default_values_correctness() {
    let s = VoxSettings::default();

    // Default interaction modes
    assert_eq!(s.interaction.main_app_mode, InteractionMode::Passive);
    assert_eq!(s.interaction.tray_mode, InteractionMode::Passive);
    assert_eq!(s.interaction.pipeline_mode, PipelineMode::Modular);

    // Default VAD backend option
    assert_eq!(s.vad.vad_backend, VadBackendOption::TenVad);

    // Default LLM provider config is Embedded
    assert!(matches!(s.llm.provider, LlmProviderConfig::Embedded));
    assert_eq!(s.llm.ctx_size, 2048);
    assert_eq!(s.llm.threads, 4);

    // Default UI settings
    assert_eq!(s.ui.tray_enabled, true);
}

// ─── 2. Serde JSON In-Memory Roundtrip Test ──────────────────────────────────

#[test]
fn test_settings_serde_json_roundtrip() {
    let mut original = VoxSettings::default();

    // Mutate custom values
    original.interaction.main_app_mode = InteractionMode::PTT;
    original.interaction.tray_mode = InteractionMode::Passive;
    original.llm.ctx_size = 4096;
    original.llm.threads = 8;
    original.vad.threshold = 0.65;
    original.ui.theme = "cyberpunk".to_string();

    // Serialize to JSON
    let json_str = serde_json::to_string_pretty(&original).expect("Failed to serialize VoxSettings");
    assert!(!json_str.is_empty());

    // Deserialize back from JSON
    let deserialized: VoxSettings =
        serde_json::from_str(&json_str).expect("Failed to deserialize VoxSettings");

    // Assert exact equality
    assert_eq!(deserialized.interaction.main_app_mode, InteractionMode::PTT);
    assert_eq!(deserialized.interaction.tray_mode, InteractionMode::Passive);
    assert_eq!(deserialized.llm.ctx_size, 4096);
    assert_eq!(deserialized.llm.threads, 8);
    assert_eq!(deserialized.vad.threshold, 0.65);
    assert_eq!(deserialized.ui.theme, "cyberpunk");
}

// ─── 3. File System Disk Save / Load Roundtrip Test ──────────────────────────

#[test]
fn test_settings_disk_save_load_roundtrip_tempfile() {
    let temp_dir = std::env::temp_dir();
    let temp_file_path = temp_dir.join(format!("vox_test_settings_{}.json", std::process::id()));

    let mut settings = VoxSettings::default();
    settings.llm.model = "qwen_2_5_custom_q4".to_string();
    settings.llm.chat_temperature = 0.35;
    settings.ui.tray_enabled = false;

    // Atomic write simulation (tmp -> target)
    let tmp_path = temp_file_path.with_extension("tmp");
    let json_bytes = serde_json::to_string_pretty(&settings).unwrap();
    fs::write(&tmp_path, &json_bytes).unwrap();
    fs::rename(&tmp_path, &temp_file_path).unwrap();

    // Read back and parse
    let content = fs::read_to_string(&temp_file_path).unwrap();
    let loaded: VoxSettings = serde_json::from_str(&content).unwrap();

    assert_eq!(loaded.llm.model, "qwen_2_5_custom_q4");
    assert_eq!(loaded.llm.chat_temperature, 0.35);
    assert_eq!(loaded.ui.tray_enabled, false);

    // Clean up temp file
    let _ = fs::remove_file(&temp_file_path);
}

// ─── 4. Cross-Provider Setting Field Isolation Test ──────────────────────────

#[test]
fn test_llm_provider_switch_field_isolation() {
    let mut llm = LlmSettings::default();

    // 1. Initial embedded setup
    llm.provider = LlmProviderConfig::Embedded;
    llm.model = "llama_3_2_reasoning_q4".to_string();
    llm.ctx_size = 4096;

    let embedded_ctx = llm.effective_ctx_size();
    assert_eq!(embedded_ctx, 4096);

    // 2. Switch to OpenAiCompat (Ollama)
    llm.provider = LlmProviderConfig::OpenAiCompat {
        base_url: "http://100.86.62.14:11434".to_string(),
        model: "llama3.1:8b".to_string(),
        api_key: None,
        provider_name: Some("ollama".to_string()),
    };

    // Verify embedded ctx_size field remains 4096, but effective_ctx_size returns floor (8192)
    assert_eq!(llm.ctx_size, 4096);
    assert_eq!(llm.effective_ctx_size(), 8192);

    // 3. Switch back to Embedded
    llm.provider = LlmProviderConfig::Embedded;
    assert_eq!(llm.effective_ctx_size(), 4096);
    assert_eq!(llm.model, "llama_3_2_reasoning_q4");
}

// ─── 5. Corrupt & Partial JSON Resilience Test ────────────────────────────────

#[test]
fn test_corrupt_and_partial_json_resilience() {
    // Malformed JSON should return error on parse
    let corrupt_json = "{ \"ui\": { \"theme\": \"dark\", ";
    let result: Result<VoxSettings, _> = serde_json::from_str(corrupt_json);
    assert!(result.is_err());

    // Partial JSON with missing fields should leverage serde(default) without crashing
    let partial_json = r#"{
        "ui": {
            "theme": "nord"
        }
    }"#;

    let parsed: VoxSettings =
        serde_json::from_str(partial_json).expect("Partial JSON should leverage serde(default)");
    assert_eq!(parsed.ui.theme, "nord");
    assert_eq!(parsed.llm.ctx_size, 2048); // Restored default
    assert_eq!(parsed.interaction.main_app_mode, InteractionMode::Passive);
}
