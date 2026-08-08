//! ============================================================================
//! multi_provider_cancel_test.rs — Multi-Provider Cancel Flag Interruption Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : LLM Provider Abstraction (`vox_lib::services::llm::providers`)
//! Prerequisites: Compiles against `vox_lib` public API
//! Execution    : 
//!   - Default (Embedded Local) : cargo test --test multi_provider_cancel_test
//!   - Server & Cloud (Ignored) : cargo test --test multi_provider_cancel_test -- --ignored
//! Metrics      : Cancellation flag propagation & channel stream interruption
//! ============================================================================

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::events::VoxEvent;
use vox_lib::services::llm::providers::{LlmProvider, OpenAiCompatProvider};
use vox_lib::services::llm::types::{
    ConversationInput, GenerationOptions, GenerationPurpose, GenerationRequest, OutputConstraint,
};
use vox_lib::services::memory::{ChatMessage, Role};

fn create_test_request() -> GenerationRequest {
    GenerationRequest {
        input: ConversationInput {
            messages: vec![ChatMessage {
                role: Role::User,
                content: "Hello world, please generate a long response.".to_string(),
                timestamp_ms: 0,
            }],
        },
        options: GenerationOptions::default(),
        output: OutputConstraint::Text,
        purpose: GenerationPurpose::Conversation,
    }
}

// Helper to spawn a multi-connection mock HTTP server that handles probes and streams SSE chunks
fn spawn_slow_sse_server() -> String {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock server");
    let addr = listener.local_addr().unwrap();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                std::thread::spawn(move || {
                    let mut buf = [0u8; 2048];
                    if let Ok(n) = stream.read(&mut buf) {
                        let req_str = String::from_utf8_lossy(&buf[..n]);
                        if req_str.contains("GET ") {
                            // Respond 200 OK to health/probe GET checks
                            let resp = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
                            let _ = stream.write_all(resp.as_bytes());
                        } else if req_str.contains("POST ") {
                            // Respond SSE stream to POST generation requests
                            let header = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: keep-alive\r\n\r\n";
                            let _ = stream.write_all(header.as_bytes());

                            for i in 0..10 {
                                std::thread::sleep(Duration::from_millis(50));
                                let chunk = format!(
                                    "data: {{\x22choices\x22:[{{\x22delta\x22:{{\x22content\x22:\x22Token{}\x22}}}}]}}\r\n\r\n",
                                    i
                                );
                                if stream.write_all(chunk.as_bytes()).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
            }
        }
    });

    format!("http://127.0.0.1:{}", addr.port())
}

// ─── 1. Local Default Tests (Embedded / Mock SSE Stream) ──────────────────────

#[test]
fn test_local_provider_mid_stream_cancellation() {
    let url = spawn_slow_sse_server();
    let provider = OpenAiCompatProvider::new(&url, "mock-model", None, None);

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let request = create_test_request();

    let cancel_flag_clone = Arc::clone(&cancel_flag);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120)); // Cancel after 2-3 chunks
        cancel_flag_clone.store(true, Ordering::Relaxed);
    });

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let gen_res = rt.block_on(provider.generate(request, 101, &cancel_flag, &tx));
    assert!(gen_res.is_ok(), "Generation request returned error: {:?}", gen_res.err());

    let mut received_tokens = Vec::new();
    let mut saw_cancelled_event = false;

    while let Ok(event) = rx.recv_timeout(Duration::from_millis(300)) {
        match event {
            VoxEvent::LlmToken { turn_id, token } => {
                assert_eq!(turn_id, 101);
                received_tokens.push(token);
            }
            VoxEvent::Cancelled { turn_id } => {
                assert_eq!(turn_id, 101);
                saw_cancelled_event = true;
            }
            _ => {}
        }
    }

    assert!(
        saw_cancelled_event,
        "Mid-stream cancellation MUST emit VoxEvent::Cancelled!"
    );
    assert!(
        received_tokens.len() < 10,
        "Stream MUST be truncated when cancelled mid-stream!"
    );
}

#[test]
fn test_local_provider_pre_cancelled() {
    let url = spawn_slow_sse_server();
    let provider = OpenAiCompatProvider::new(&url, "mock-model", None, None);

    let cancel_flag = Arc::new(AtomicBool::new(true)); // Already cancelled
    let (tx, rx) = mpsc::channel();
    let request = create_test_request();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let gen_res = rt.block_on(provider.generate(request, 102, &cancel_flag, &tx));
    assert!(gen_res.is_ok(), "Pre-cancelled generation request returned error: {:?}", gen_res.err());

    let mut saw_cancelled_event = false;
    let mut token_count = 0;

    while let Ok(event) = rx.recv_timeout(Duration::from_millis(200)) {
        match event {
            VoxEvent::LlmToken { .. } => token_count += 1,
            VoxEvent::Cancelled { turn_id } => {
                assert_eq!(turn_id, 102);
                saw_cancelled_event = true;
            }
            _ => {}
        }
    }

    assert!(saw_cancelled_event, "Pre-cancelled request MUST emit VoxEvent::Cancelled!");
    assert_eq!(token_count, 0, "Pre-cancelled request MUST NOT emit any tokens!");
}

// ─── 2. Server & Cloud Tests (Ignored — Run via cargo test -- --ignored) ──────

#[test]
#[ignore = "Server provider test requires running local Ollama instance at localhost:11434"]
fn test_server_ollama_cancellation_ignored() {
    let provider = OpenAiCompatProvider::new("http://localhost:11434", "llama3.2:latest", None, Some("ollama"));
    let cancel_flag = Arc::new(AtomicBool::new(true));
    let (tx, rx) = mpsc::channel();
    let request = create_test_request();

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let _ = rt.block_on(provider.generate(request, 201, &cancel_flag, &tx));

    let mut cancelled = false;
    while let Ok(event) = rx.recv_timeout(Duration::from_millis(300)) {
        if let VoxEvent::Cancelled { turn_id } = event {
            assert_eq!(turn_id, 201);
            cancelled = true;
        }
    }
    assert!(cancelled);
}

#[test]
#[ignore = "Cloud provider test requires NVIDIA_API_KEY in temp/.env"]
fn test_cloud_nvidia_cancellation_ignored() {
    let env_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../temp/.env");
    if !env_path.exists() {
        return;
    }
    let dotenv_content = std::fs::read_to_string(env_path).unwrap_or_default();
    let api_key = dotenv_content
        .lines()
        .find(|l| l.starts_with("NVIDIA_API_KEY="))
        .and_then(|l| l.split('=').nth(1))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if let Some(key) = api_key {
        let provider = OpenAiCompatProvider::new(
            "https://integrate.api.nvidia.com/v1",
            "meta/llama-3.1-8b-instruct",
            Some(key),
            Some("nvidia"),
        );
        let cancel_flag = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::channel();
        let request = create_test_request();

        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let _ = rt.block_on(provider.generate(request, 301, &cancel_flag, &tx));

        let mut cancelled = false;
        while let Ok(event) = rx.recv_timeout(Duration::from_millis(500)) {
            if let VoxEvent::Cancelled { turn_id } = event {
                assert_eq!(turn_id, 301);
                cancelled = true;
            }
        }
        assert!(cancelled);
    }
}
