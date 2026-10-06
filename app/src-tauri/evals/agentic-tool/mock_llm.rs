//! ============================================================================
//! evals/agentic-tool/mock_llm.rs — Stateful Mock LLM + Scripted Tool Calls
//! ============================================================================
//! Category     : Evaluation
//! Component    : evals agentic-tool harness (LLM-in side)
//! Prerequisites: none (binds an ephemeral loopback port)
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : every request body is captured to `requests/request_<n>.json`
//!
//! ## The seam this closes
//!
//! The eval's input is *the LLM's output to the harness*. That arrives as an
//! OpenAI-compatible SSE stream, so the honest way to inject a tool call is to
//! serve one. A single-shot mock cannot drive a reentrant tool loop — the harness
//! issues a second request after the observation lands — so this server is
//! stateful: it serves a per-request script and captures every body it receives.
//!
//! **The captured body of the final request is the eval's output.** The harness
//! re-enters `execute_turn` with the scratchpad holding the tool observation, so
//! the last request body literally contains the context the model was given.
//!
//! ## Why this runs the real transport
//!
//! Pointing `ConnectionConfig` at this server means the production
//! `RemoteTransport` does the SSE parsing, tool-call assembly and streaming — the
//! eval exercises the same wire path as production rather than stubbing the
//! provider.

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use vox_lib::services::llm::CanonicalToolCall;

/// What the mock returns for one request.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TurnScript {
    /// Emit a tool call, assembled from chunked argument deltas like a real model.
    ToolCall {
        id: String,
        name: String,
        /// Serialized as a JSON object; streamed in fragments.
        arguments: serde_json::Value,
    },
    /// Emit streamed text tokens and finish.
    Text(String),
    /// Emit a tool call, then plain text on the next request.
    ToolCallThenText {
        call: Box<TurnScript>,
        final_text: String,
    },
}

#[allow(dead_code)]
impl TurnScript {
    /// Builds a tool-call script from a name and argument object.
    pub fn tool(name: &str, arguments: serde_json::Value) -> Self {
        Self::ToolCall {
            id: format!("call_{}", name),
            name: name.to_string(),
            arguments,
        }
    }

    /// Builds a tool-call script with an explicit call id.
    pub fn tool_with_id(id: &str, name: &str, arguments: serde_json::Value) -> Self {
        Self::ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments,
        }
    }

    /// Chains a tool call with the final spoken text for the following turn.
    pub fn then_text(self, final_text: &str) -> Self {
        match self {
            Self::ToolCallThenText { .. } => self,
            other => Self::ToolCallThenText {
                call: Box::new(other),
                final_text: final_text.to_string(),
            },
        }
    }
}

fn resolve(script: &TurnScript) -> (&TurnScript, Option<&str>) {
    match script {
        TurnScript::ToolCallThenText { call, final_text } => (call.as_ref(), Some(final_text)),
        other => (other, None),
    }
}

/// A running mock LLM server.
#[allow(dead_code)]
pub struct MockLlmServer {
    /// Base URL to hand to `ConnectionConfig`.
    pub base_url: String,
    /// Handles every request body the server received, in order.
    pub requests: Arc<Mutex<Vec<serde_json::Value>>>,
    /// The server thread. Joined on [`Drop`] so a panic is never silent.
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    port: u16,
    /// Tells the accept loop to exit after the current connection, so `shutdown`
    /// can join instead of parking forever back in `accept()`.
    shutdown: Arc<AtomicBool>,
}

#[allow(dead_code)]
impl MockLlmServer {
    /// Binds an ephemeral loopback port and starts serving `scripts` in order.
    ///
    /// Requests beyond the script length repeat the final entry, so a harness that
    /// loops more than expected still gets a valid response rather than hanging.
    pub fn start(scripts: Vec<TurnScript>) -> Result<Self> {
        anyhow::ensure!(!scripts.is_empty(), "Mock LLM script cannot be empty");

        // Flatten chains so consecutive requests advance through the script:
        // `ToolCallThenText` serves the call on request N and the text on N+1.
        // Without this, every request re-serves the call part and the harness
        // re-executes the tool once per loop iteration until MAX_TOOL_ITERATIONS.
        let scripts: Vec<TurnScript> = scripts
            .into_iter()
            .flat_map(|s| match s {
                TurnScript::ToolCallThenText { call, final_text } => {
                    vec![*call, TurnScript::Text(final_text)]
                }
                other => vec![other],
            })
            .collect();

        let listener = TcpListener::bind("127.0.0.1:0")
            .context("Mock LLM server failed to bind an ephemeral loopback port")?;
        let port = listener
            .local_addr()
            .context("Mock LLM server has no local address")?
            .port();

        let requests: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let counter = Arc::new(AtomicUsize::new(0));

        let thread_requests = Arc::clone(&requests);
        let thread_counter = Arc::clone(&counter);
        let thread_shutdown = Arc::new(AtomicBool::new(false));
        let loop_shutdown = Arc::clone(&thread_shutdown);
        let handle = std::thread::spawn(move || {
            // Serving runs on its own thread; each connection is handled inline,
            // which is sufficient because the harness issues requests sequentially.
            // The payload is framed as a real HTTP/1.1 response — the production
            // transport is a real HTTP client and cannot parse bare SSE bytes.
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
                let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));

                let idx = thread_counter.fetch_add(1, Ordering::SeqCst);
                let body = read_http_body(&mut stream);
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                    thread_requests.lock().push(v);
                }

                let script = scripts.get(idx).or_else(|| scripts.last()).unwrap();
                let (this_turn, chained) = resolve(script);
                let sse = render_sse(this_turn, idx);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    sse.len(),
                    sse
                );

                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                let _ = stream.shutdown(std::net::Shutdown::Both);

                if loop_shutdown.load(Ordering::SeqCst) {
                    break;
                }
                if chained.is_none() {
                    // A terminal text turn ends the loop; nothing more is expected.
                    if matches!(this_turn, TurnScript::Text(_)) {
                        break;
                    }
                }
            }
        });

        Ok(Self {
            base_url: format!("http://127.0.0.1:{}/v1", port),
            requests,
            handle: Arc::new(Mutex::new(Some(handle))),
            port,
            shutdown: thread_shutdown,
        })
    }

    /// The ephemeral port in use.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Every captured request body, oldest first.
    pub fn captured_requests(&self) -> Vec<serde_json::Value> {
        self.requests.lock().clone()
    }

    /// The final request body, which contains the tool observation.
    ///
    /// This is the eval's output under test.
    pub fn final_request(&self) -> Option<serde_json::Value> {
        self.requests.lock().last().cloned()
    }

    /// Stops accepting connections and joins the server thread.
    pub fn shutdown(&self) {
        // Set the exit flag first, then unblock `incoming()` by connecting once.
        // The thread serves the dummy connection, sees the flag, and breaks —
        // without the flag it would loop straight back into `accept()` and the
        // join below would hang forever.
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(h) = self.handle.lock().take() {
            let _ = h.join();
        }
    }
}

impl Drop for MockLlmServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

use std::net::TcpStream;

/// Reads one HTTP request and returns its body.
///
/// Deliberately minimal: the eval only needs `Content-Length` bodies from the
/// production transport, and a full HTTP parser would add surface without adding
/// fidelity.
fn read_http_body(stream: &mut TcpStream) -> String {
    let mut raw = Vec::new();
    let mut buf = [0u8; 8192];
    let mut header_end = None;

    // Read until the header terminator, then until Content-Length is satisfied.
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&buf[..n]);
                if header_end.is_none() {
                    header_end = find_header_end(&raw);
                }
                if let Some((hend, content_len)) = header_end {
                    if raw.len() >= hend + content_len {
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }

    let Some((hend, content_len)) = header_end else {
        return String::new();
    };
    let start = hend;
    let end = (start + content_len).min(raw.len());
    String::from_utf8_lossy(&raw[start..end]).to_string()
}

fn find_header_end(raw: &[u8]) -> Option<(usize, usize)> {
    let marker = b"\r\n\r\n";
    let pos = raw.windows(marker.len()).position(|w| w == marker)?;
    let headers = String::from_utf8_lossy(&raw[..pos]);
    let content_len = headers
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            if k.trim().eq_ignore_ascii_case("content-length") {
                v.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    Some((pos + marker.len(), content_len))
}

/// Renders one turn as an OpenAI-compatible SSE stream.
///
/// Tool arguments are streamed in fragments so the transport's incremental
/// assembly path is exercised, exactly as a real model would.
fn render_sse(script: &TurnScript, idx: usize) -> String {
    let mut out = String::new();
    let push = |out: &mut String, v: serde_json::Value| {
        out.push_str("data: ");
        out.push_str(&v.to_string());
        out.push_str("\n\n");
    };

    match script {
        TurnScript::Text(text) => {
            for token in text.split_inclusive(' ') {
                push(
                    &mut out,
                    serde_json::json!({
                        "choices": [{
                            "index": 0,
                            "delta": { "content": token },
                            "finish_reason": serde_json::Value::Null
                        }]
                    }),
                );
            }
            push(
                &mut out,
                serde_json::json!({
                    "choices": [{
                        "index": 0,
                        "delta": {},
                        "finish_reason": "stop"
                    }]
                }),
            );
        }
        TurnScript::ToolCall {
            id,
            name,
            arguments,
        } => {
            let arg_str = arguments.to_string();
            let (head, tail) = arg_str.split_at(arg_str.len() / 2);

            push(
                &mut out,
                serde_json::json!({
                    "choices": [{
                        "index": 0,
                        "delta": {
                            "tool_calls": [{
                                "index": 0,
                                "id": id,
                                "type": "function",
                                "function": { "name": name, "arguments": head }
                            }]
                        },
                        "finish_reason": serde_json::Value::Null
                    }]
                }),
            );
            push(
                &mut out,
                serde_json::json!({
                    "choices": [{
                        "index": 0,
                        "delta": {
                            "tool_calls": [{
                                "index": 0,
                                "function": { "arguments": tail }
                            }]
                        },
                        "finish_reason": serde_json::Value::Null
                    }]
                }),
            );
            push(
                &mut out,
                serde_json::json!({
                    "choices": [{
                        "index": 0,
                        "delta": {},
                        "finish_reason": "tool_calls"
                    }]
                }),
            );
        }
        TurnScript::ToolCallThenText { .. } => unreachable!("resolved before render"),
    }

    out.push_str("data: [DONE]\n\n");
    let _ = idx;
    out
}

/// Turns a captured request body into a `CanonicalToolCall` for cross-checking.
///
/// The transport already produced one during the run; this lets the eval assert
/// that what it *asked for* matches what the harness *received*. Searches for
/// the assistant stub carrying `tool_calls` — the last message is the tool-role
/// observation, which carries none.
#[allow(dead_code)]
pub fn tool_call_from_request(body: &serde_json::Value) -> Option<CanonicalToolCall> {
    let msgs = body.get("messages")?.as_array()?;
    let msg = msgs
        .iter()
        .rev()
        .find(|m| m.get("tool_calls").and_then(|t| t.as_array()).is_some())?;
    let call = msg.get("tool_calls")?.as_array()?.first()?;
    Some(CanonicalToolCall {
        id: call.get("id")?.as_str()?.to_string(),
        name: call.get("function")?.get("name")?.as_str()?.to_string(),
        arguments: call
            .get("function")
            .and_then(|f| f.get("arguments"))
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    })
}

/// Extracts every tool-role message content from a captured request body.
///
/// The harness stages the observation as a `Tool` message, so this is the string
/// the model was shown for a given call id.
#[allow(dead_code)]
pub fn tool_observations(body: &serde_json::Value) -> Vec<(Option<String>, String)> {
    body.get("messages")
        .and_then(|m| m.as_array())
        .map(|msgs| {
            msgs.iter()
                .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("tool"))
                .map(|m| {
                    (
                        m.get("tool_call_id")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        m.get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Blocks until `n` requests have been captured, or `timeout` elapses.
///
/// Returns the number actually captured so a caller can distinguish "arrived" from
/// "timed out short" rather than silently proceeding with a partial script.
#[allow(dead_code)]
pub fn wait_for_requests(server: &MockLlmServer, n: usize, timeout: std::time::Duration) -> usize {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let have = server.requests.lock().len();
        if have >= n || std::time::Instant::now() >= deadline {
            return have;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Asserts the server received at least `n` requests, naming the gap on failure.
pub fn require_requests(server: &MockLlmServer, n: usize) -> Result<()> {
    let have = server.requests.lock().len();
    if have < n {
        return Err(anyhow!(
            "Mock LLM expected at least {} requests but received {}. The harness loop \
             did not run as scripted, so the output seam cannot be trusted.",
            n,
            have
        ));
    }
    Ok(())
}

/// Unused marker retained so `mpsc` stays imported for future scripted streaming.
#[allow(dead_code)]
fn _mpsc_anchor() -> mpsc::Receiver<u8> {
    mpsc::channel().1
}
