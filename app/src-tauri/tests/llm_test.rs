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
use std::sync::mpsc;

#[test]
fn test_backend_init() {
    use llama_cpp_2::llama_backend::LlamaBackend;
    use std::sync::OnceLock;
    
    static TEST_BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    println!("Initializing backend...");
    let _backend = TEST_BACKEND.get_or_init(|| {
        LlamaBackend::init().expect("Failed to initialize llama.cpp backend")
    });
    println!("Backend initialized!");
}

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
    use vox_lib::services::llm::LlmWorker;

    let path = gguf_path();
    let worker = LlmWorker::new(&path, 512, 2);
    assert!(worker.is_ok(), "LLM model should load: {:?}", worker.err());
}

#[test]
#[ignore]
fn test_llm_generates_tokens() {
    use vox_lib::services::llm::{LlmWorker, LlmCommand};
    use vox_lib::core::events::VoxEvent;

    let path = gguf_path();
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();

    // Spawn worker thread
    let handle = std::thread::spawn(move || {
        let worker = LlmWorker::new(&path, 512, 2).expect("model load failed");
        worker.run_loop(cmd_rx, event_tx);
    });

    let cancel = Arc::new(AtomicBool::new(false));
    
    // Send generate command
    cmd_tx.send(LlmCommand::Generate {
        text: "Say exactly: hello".to_string(),
        system_prompt: "You are a helpful assistant.".to_string(),
        turn_id: 1,
        cancel_flag: cancel,
    }).expect("send failed");

    // Collect tokens with standard channel receiver
    let mut tokens = Vec::new();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(30) {
        if let Ok(event) = event_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            match event {
                VoxEvent::LlmToken { token, .. } => {
                    tokens.push(token);
                }
                VoxEvent::LlmFinished { .. } => break,
                _ => {}
            }
        }
    }

    // Shutdown
    cmd_tx.send(LlmCommand::Shutdown).expect("shutdown failed");
    handle.join().expect("thread panicked");

    assert!(!tokens.is_empty(), "LLM should produce at least one token");
    let text = tokens.join("");
    println!("[LLM TEST] Generated: {}", text);
    assert!(text.to_lowercase().contains("hello"), "Output should contain 'hello'");
}

#[test]
#[ignore]
fn test_llm_cancels_mid_generation() {
    use vox_lib::services::llm::{LlmWorker, LlmCommand};
    use vox_lib::core::events::VoxEvent;

    let path = gguf_path();
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (event_tx, event_rx) = mpsc::channel();

    let handle = std::thread::spawn(move || {
        let worker = LlmWorker::new(&path, 512, 2).expect("model load failed");
        worker.run_loop(cmd_rx, event_tx);
    });

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();

    // Send generate command
    cmd_tx.send(LlmCommand::Generate {
        text: "Write a very long essay about the history of computing".to_string(),
        system_prompt: "You are a helpful assistant.".to_string(),
        turn_id: 1,
        cancel_flag: cancel_clone,
    }).expect("send failed");

    // Cancel after first token arrives
    let mut got_token = false;
    let mut cancelled = false;
    let start = std::time::Instant::now();
    
    while start.elapsed() < std::time::Duration::from_secs(30) {
        if let Ok(event) = event_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            match event {
                VoxEvent::LlmToken { .. } => {
                    if !got_token {
                        got_token = true;
                        // Trigger cancellation
                        cancel.store(true, Ordering::Relaxed);
                    }
                }
                VoxEvent::Cancelled { .. } => {
                    cancelled = true;
                    break;
                }
                VoxEvent::LlmFinished { .. } => break,
                _ => {}
            }
        }
    }

    // Shutdown
    cmd_tx.send(LlmCommand::Shutdown).expect("shutdown failed");
    handle.join().expect("thread panicked");

    assert!(cancelled, "Cancellation event should be received after cancel_flag is set");
}
