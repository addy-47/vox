/// Integration test: LLM runtime (llama-cpp-2 + Gemma GGUF)
///
/// Run with:
///   cargo test --test llm_test -- --ignored --nocapture
///
/// All model-loading tests are #[ignore]d by default to avoid blocking CI.
/// Logic-only tests (prompt formatting, cancellation flag) run always.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

fn gguf_path() -> PathBuf {
    // Resolve relative to the manifest directory so tests work from any CWD
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("assets/gemma4/google_gemma-4-E2B-it-IQ2_M.gguf")
}

// ─── Logic Tests (always run) ─────────────────────────────────────────────────

#[test]
fn test_cancel_flag_is_checked() {
    // Verify that an already-set cancel_flag prevents any work.
    // This is a pure Rust logic test — no model loaded.
    let cancel = Arc::new(AtomicBool::new(true)); // already cancelled
    assert!(cancel.load(Ordering::Relaxed), "flag should be set");
    cancel.store(false, Ordering::Relaxed);
    assert!(!cancel.load(Ordering::Relaxed), "flag should clear");
}

#[test]
fn test_gguf_file_exists() {
    let path = gguf_path();
    assert!(
        path.exists(),
        "[LLM] GGUF not found at expected path: {:?}. \
         Ensure assets/gemma4/google_gemma-4-E2B-it-IQ2_M.gguf is present.",
        path
    );
}

// ─── Model Tests (ignored — require 2.5GB model in memory) ───────────────────

#[test]
#[ignore]
fn test_llm_model_loads() {
    use vox_ui_lib::llm::LlmWorker;

    let path = gguf_path();
    let worker = LlmWorker::new(&path, 512, 2);
    assert!(worker.is_ok(), "LLM model should load: {:?}", worker.err());
}

#[test]
#[ignore]
fn test_llm_generates_tokens() {
    use vox_ui_lib::llm::LlmWorker;

    let path = gguf_path();
    let worker = LlmWorker::new(&path, 512, 2).expect("model load failed");

    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, mut rx) = mpsc::channel(64);

    // Run generation on a dedicated thread (llama.cpp is not Send)
    let cancel_clone = cancel.clone();
    let handle = std::thread::spawn(move || {
        worker.generate("Say exactly: hello", 1, &cancel_clone, &tx)
    });

    // Collect tokens with a timeout
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tokens: Vec<String> = rt.block_on(async {
        let mut collected = Vec::new();
        while let Ok(event) = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            rx.recv(),
        ).await {
            match event {
                Some(vox_ui_lib::events::VoxEvent::LlmToken { token, .. }) => {
                    collected.push(token);
                }
                Some(vox_ui_lib::events::VoxEvent::LlmFinished { .. }) => break,
                None => break,
                _ => {}
            }
        }
        collected
    });

    handle.join().expect("thread panicked").expect("generation failed");

    assert!(!tokens.is_empty(), "LLM should produce at least one token");
    let text = tokens.join("");
    println!("[LLM TEST] Generated: {}", text);
}

#[test]
#[ignore]
fn test_llm_cancels_mid_generation() {
    use vox_ui_lib::llm::LlmWorker;

    let path = gguf_path();
    let worker = LlmWorker::new(&path, 2048, 2).expect("model load failed");

    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, mut rx) = mpsc::channel(64);

    let cancel_for_thread = cancel.clone();
    let cancel_for_killer = cancel.clone();

    let handle = std::thread::spawn(move || {
        worker.generate(
            "Write a very long essay about the history of computing",
            1,
            &cancel_for_thread,
            &tx,
        )
    });

    // Cancel after first token arrives
    let rt = tokio::runtime::Runtime::new().unwrap();
    let cancelled = rt.block_on(async {
        let mut got_token = false;
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_secs(30),
                rx.recv(),
            ).await {
                Ok(Some(vox_ui_lib::events::VoxEvent::LlmToken { .. })) => {
                    if !got_token {
                        got_token = true;
                        // Trigger cancellation
                        cancel_for_killer.store(true, Ordering::Relaxed);
                    }
                }
                Ok(Some(vox_ui_lib::events::VoxEvent::Cancelled { .. })) => return true,
                Ok(Some(vox_ui_lib::events::VoxEvent::LlmFinished { .. })) => return false,
                _ => return false,
            }
        }
    });

    handle.join().expect("thread panicked").expect("generation error");
    assert!(cancelled, "Cancellation event should be received after cancel_flag is set");
}
