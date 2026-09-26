//! ============================================================================
//! tests/common/wire.rs — Shared Provider-Wire Capture Harness
//! ============================================================================
//! Category     : Test Infrastructure (shared)
//! Component    : n/a — std-only HTTP/1.1 capture server
//! Prerequisites: None (loopback TCP only)
//! Execution    : n/a — invoked by test binaries
//! Metrics      : Captures the exact JSON request body a provider transport emits.
//! ============================================================================
//!
//! Serves exactly one canned HTTP response over loopback TCP and captures the
//! outbound JSON request body before responding.
//!
//! This exists because a `GenerationRequest` assembled by the Harness is otherwise
//! **unobservable** from an integration test: the duplex pipe's receiver is moved into
//! the LLM worker thread, so nothing outside the worker can inspect what was sent. That
//! made the "cognitive front door" seam (STT -> `prepare_turn` -> duplex pipe) unable to
//! assert its own headline behaviour — that the system prompt, personal memory, and
//! history actually reach the model.
//!
//! Pointing a real `RemoteTransport` at this server closes that gap without any
//! production change: the request bytes are produced by the real provider-mapping code
//! path, and the assertion is made against those bytes.
//!
//! Promoted to `tests/common/` per `.agents/rules/testing-style-guide.md` §8.3 — shared
//! wire infrastructure must live in a decoupled module, never duplicated per test file.
//!
//! Reference usage: `tests/agentic_tool_runtime_test.rs` (SSE + NDJSON tool-call
//! streams), `tests/transcript_to_llm_test.rs` (assembled request contract).

use parking_lot::Mutex;
use std::io::{Read, Write};
use std::sync::Arc;

/// Captured request bodies, keyed by call order.
pub type CapturedBody = Arc<Mutex<Option<serde_json::Value>>>;

/// Spawns a loopback HTTP/1.1 server that captures one JSON request body and replies
/// with `response_body` as `content_type`.
///
/// Returns the base URL (`http://127.0.0.1:<port>`) and a handle that must be joined so
/// a crashed server is reported rather than silently ignored.
pub fn spawn_mock_wire_server(
    response_body: Vec<u8>,
    content_type: &'static str,
    captured: CapturedBody,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock wire server must bind");
    let addr = listener
        .local_addr()
        .expect("mock wire server needs an addr");
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("mock wire server must accept");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .expect("mock read timeout");
        stream
            .set_write_timeout(Some(std::time::Duration::from_secs(10)))
            .expect("mock write timeout");
        let mut raw = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = stream.read(&mut buf).expect("mock wire server must read");
            if n == 0 {
                break;
            }
            raw.extend_from_slice(&buf[..n]);
            if let Some(end) = raw
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .map(|pos| pos + 4)
            {
                let head = String::from_utf8_lossy(&raw[..end]).to_lowercase();
                let len = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                while raw.len() < end + len {
                    let n = stream
                        .read(&mut buf)
                        .expect("mock wire server must read body");
                    if n == 0 {
                        break;
                    }
                    raw.extend_from_slice(&buf[..n]);
                }
                let have = raw.len().saturating_sub(end).min(len);
                if let Ok(body) = serde_json::from_slice::<serde_json::Value>(&raw[end..end + have])
                {
                    *captured.lock() = Some(body);
                }
                break;
            }
        }
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            content_type,
            response_body.len()
        );
        stream
            .write_all(head.as_bytes())
            .expect("mock must write head");
        stream
            .write_all(&response_body)
            .expect("mock must write body");
    });
    (format!("http://{}", addr), handle)
}

/// Builds a minimal non-streaming chat-completions response body carrying `content`.
pub fn canned_chat_completion(content: &str) -> Vec<u8> {
    format!(
        "{{\"choices\":[{{\"index\":0,\"message\":{{\"role\":\"assistant\",\"content\":{}}},\"finish_reason\":\"stop\"}}]}}",
        serde_json::json!(content)
    )
    .into_bytes()
}
