//! ============================================================================
//! settings_ipc_propagation_test.rs — Settings IPC Mutation & Event Propagation Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : Core Settings (`vox_lib::core::settings`) & Events (`vox_lib::core::events`)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    : cargo test --test settings_ipc_propagation_test
//! Metrics      : Effective context window floors, dynamic compaction token curve, & non-blocking channel events
//! ============================================================================

use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::thread;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{LlmProviderConfig, LlmSettings, VoxSettings};
use vox_lib::services::llm::policy::calculate_compaction_max_tokens;
use vox_lib::services::llm::CTX_FLOOR_NON_EMBEDDED;

// ─── 1. Effective Context Window & Floor Calculations ─────────────────────────

#[test]
fn test_llm_effective_context_window_calculation() {
    let mut llm = LlmSettings {
        provider: LlmProviderConfig::Embedded,
        ctx_size: 2048,
        ..Default::default()
    };
    assert_eq!(llm.effective_ctx_size(), 2048);

    llm.ctx_size = 4096;
    assert_eq!(llm.effective_ctx_size(), 4096);

    // 2. OpenAiCompat provider (Server / Cloud) enforces minimum hard floor (8192)
    llm.provider = LlmProviderConfig::OpenAiCompat {
        base_url: "http://localhost:11434".to_string(),
        model: "llama3.2".to_string(),
        api_key: None,
        provider_name: Some("ollama".to_string()),
    };

    llm.ctx_size = 2048; // Below 8192 floor
    assert_eq!(
        llm.effective_ctx_size(),
        CTX_FLOOR_NON_EMBEDDED,
        "Non-embedded models with ctx_size < 8192 MUST enforce CTX_FLOOR_NON_EMBEDDED floor!"
    );

    llm.ctx_size = 16384; // Above 8192 floor
    assert_eq!(
        llm.effective_ctx_size(),
        16384,
        "Non-embedded models with ctx_size >= 8192 MUST use the higher configured value!"
    );
}

// ─── 2. Dynamic Compaction Max Tokens Scaling Curve Test ─────────────────────

#[test]
fn test_dynamic_compaction_max_tokens_formula_propagation() {
    // 8192 baseline context -> 15% of 8192 = 1228 tokens
    let tokens_8k = calculate_compaction_max_tokens(8192);
    assert_eq!(tokens_8k, 1228);

    // Small context (2048) -> 30% of 2048 = 614 tokens
    let tokens_2k = calculate_compaction_max_tokens(2048);
    assert_eq!(tokens_2k, 614);

    // Minimum output floor test (< 853 context -> forced min 256)
    let tokens_min = calculate_compaction_max_tokens(500);
    assert_eq!(tokens_min, 256);

    // Large context (128k) -> capped at hard max (16,384 tokens)
    let tokens_128k = calculate_compaction_max_tokens(128_000);
    assert_eq!(tokens_128k, 16_384);

    // Ultra-large context (1M) -> capped at hard max (16,384 tokens)
    let tokens_1m = calculate_compaction_max_tokens(1_000_000);
    assert_eq!(tokens_1m, 16_384);
}

// ─── 3. Concurrent RwLock Settings Access Contract ───────────────────────────

#[test]
fn test_settings_rwlock_concurrency_contract() {
    let settings = Arc::new(RwLock::new(VoxSettings::default()));
    let mut reader_handles = Vec::new();

    // 1. Spawn 10 concurrent reader threads checking default state
    for _ in 0..10 {
        let settings_clone = Arc::clone(&settings);
        reader_handles.push(thread::spawn(move || {
            let r = settings_clone.read().unwrap();
            assert_eq!(
                r.interaction.main_app_mode,
                vox_lib::core::settings::InteractionMode::Passive
            );
        }));
    }

    for h in reader_handles {
        h.join().unwrap();
    }

    // 2. Mutate settings on writer thread
    {
        let mut w = settings.write().unwrap();
        w.interaction.main_app_mode = vox_lib::core::settings::InteractionMode::PTT;
    }

    // 3. Spawn 10 concurrent reader threads checking updated state
    let mut updated_reader_handles = Vec::new();
    for _ in 0..10 {
        let settings_clone = Arc::clone(&settings);
        updated_reader_handles.push(thread::spawn(move || {
            let r = settings_clone.read().unwrap();
            assert_eq!(
                r.interaction.main_app_mode,
                vox_lib::core::settings::InteractionMode::PTT
            );
        }));
    }

    for h in updated_reader_handles {
        h.join().unwrap();
    }

    assert_eq!(
        settings.read().unwrap().interaction.main_app_mode,
        vox_lib::core::settings::InteractionMode::PTT
    );
}

// ─── 4. Non-Blocking Channel Event Propagation Test ───────────────────────────

#[test]
fn test_event_channel_propagation_non_blocking() {
    let (tx, rx) = mpsc::channel();

    // Emit sequence of events
    tx.send(VoxEvent::LlmToken {
        turn_id: 1,
        token: "Hello".to_string(),
    })
    .unwrap();

    tx.send(VoxEvent::TtsChunk {
        turn_id: 1,
        samples: vec![0.0f32; 100],
    })
    .unwrap();

    tx.send(VoxEvent::Cancelled { turn_id: 1 }).unwrap();
    tx.send(VoxEvent::Shutdown).unwrap();

    // Verify received in exact order without loss
    let ev1 = rx.recv().unwrap();
    assert!(matches!(ev1, VoxEvent::LlmToken { .. }));

    let ev2 = rx.recv().unwrap();
    assert!(matches!(ev2, VoxEvent::TtsChunk { .. }));

    let ev3 = rx.recv().unwrap();
    assert!(matches!(ev3, VoxEvent::Cancelled { turn_id: 1 }));

    let ev4 = rx.recv().unwrap();
    assert!(matches!(ev4, VoxEvent::Shutdown));
}

// ─── 5. LLM Provider No-Op Deduplication Contract ───────────────────────────

#[test]
fn test_llm_provider_deduplication_contract() {
    let p1 = LlmProviderConfig::Embedded;
    let p2 = LlmProviderConfig::Embedded;
    assert_eq!(
        p1, p2,
        "Identical Embedded provider configs MUST equal each other!"
    );

    let remote1 = LlmProviderConfig::OpenAiCompat {
        base_url: "http://localhost:11434".to_string(),
        model: "llama3.2".to_string(),
        api_key: None,
        provider_name: Some("ollama".to_string()),
    };
    let remote2 = LlmProviderConfig::OpenAiCompat {
        base_url: "http://localhost:11434".to_string(),
        model: "llama3.2".to_string(),
        api_key: None,
        provider_name: Some("ollama".to_string()),
    };
    assert_eq!(
        remote1, remote2,
        "Identical OpenAiCompat provider configs MUST equal each other!"
    );
}
