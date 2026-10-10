//! ============================================================================
//! tests/common/http_server.rs — Deliberately-Misbehaving HTTP Origin Server
//! ============================================================================
//! Category     : Shared Integration-Test Fixture
//! Component    : exercises `vox_lib::setup::model_manager` resumable transfer
//! Prerequisites: none (binds an ephemeral loopback port)
//! Execution    : consumed by `cargo nextest run --test model_manager_test`
//! Metrics      : received `Range` headers, served status codes, bytes served
//!
//! Hand-rolled on purpose (`docs/plans/phase13/...-plan.md` §8.9): the cases worth
//! testing are *wrong servers* — one that ignores `Range` and replies `200`, one
//! that replies `416`, one that drops the connection mid-body. Mocking crates make
//! deliberately-wrong servers awkward; a raw `TcpListener` makes them explicit.
//!
//! Every response carries `Connection: close` and exactly one request is served per
//! connection, which keeps framing trivial and keeps `reqwest`'s connection pool
//! from silently masking a second request.
//! ============================================================================

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot, Mutex},
    task::JoinHandle,
};

/// Largest request head we will buffer. Real reqwest requests against a byte-range
/// endpoint are well under 1 KiB; anything larger is a protocol surprise we fail loudly on.
const MAX_REQUEST_HEAD: usize = 16 * 1024;

// ============================================================================
// Observable record of what the server actually received and did
// ============================================================================

/// One parsed inbound request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    /// Raw `Range` header value, verbatim. `None` when the header was absent —
    /// which is the single most important thing these tests observe.
    pub range_header: Option<String>,
    /// Parsed start offset from `bytes=<start>-` / `bytes=<start>-<end>`. `None`
    /// when the header was absent or unparseable.
    pub range_start: Option<u64>,
}

/// Behaviour of the origin server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServeMode {
    /// RFC 9110-conformant byte-range origin.
    ///
    /// `Range: bytes=N-` present and well-formed -> `206 Partial Content` with the
    /// tail of the body. No `Range` header -> `200 OK` with the whole body.
    HonourRange,
    /// Deliberately wrong: **ignores** any `Range` header and replies `200 OK` with
    /// the *full* body. A client that appends instead of restarting duplicates bytes
    /// and corrupts the file silently.
    IgnoreRangeReturn200,
    /// Deliberately wrong in the other direction: answers every ranged request with
    /// `416 Range Not Satisfiable` and `Content-Range: bytes */<total>`.
    ///
    /// A request carrying **no** `Range` header still gets `200 OK` with the full
    /// body — that is what a real server does, and it is what lets a client recover
    /// by deleting its stale `.tmp` and retrying once from zero.
    RangeNotSatisfiable,
}

/// Transport-level knobs layered on top of [`ServeMode`].
#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub mode: ServeMode,
    /// Announce the full `Content-Length` but write only this many body bytes, then
    /// drop the connection. Simulates a dropped connection. `None` = send it all.
    pub truncate_after: Option<usize>,
    /// Write the body in slices of this size. `None` = one write.
    pub drip_chunk: Option<usize>,
    /// Pause between dripped slices, giving the client time to observe partial state.
    pub drip_delay: Duration,
}

impl ServeConfig {
    pub fn honour_range() -> Self {
        Self {
            mode: ServeMode::HonourRange,
            truncate_after: None,
            drip_chunk: None,
            drip_delay: Duration::ZERO,
        }
    }

    pub fn ignore_range_return200() -> Self {
        Self {
            mode: ServeMode::IgnoreRangeReturn200,
            ..Self::honour_range()
        }
    }

    pub fn range_not_satisfiable() -> Self {
        Self {
            mode: ServeMode::RangeNotSatisfiable,
            ..Self::honour_range()
        }
    }

    /// Announce `Content-Length` = full body but emit only `bytes` of it, then hang up.
    pub fn with_truncate_after(mut self, bytes: usize) -> Self {
        self.truncate_after = Some(bytes);
        self
    }

    /// Drip the body in `chunk`-byte slices, sleeping `delay` between them.
    pub fn with_drip(mut self, chunk: usize, delay: Duration) -> Self {
        self.drip_chunk = Some(chunk);
        self.drip_delay = delay;
        self
    }
}

/// State shared between the accept loop and the test.
#[derive(Default)]
struct ServerState {
    requests: Mutex<Vec<RecordedRequest>>,
    statuses: Mutex<Vec<u16>>,
    bytes_served: AtomicU64,
    /// We announced a longer body than we actually wrote, then closed.
    body_truncated: AtomicBool,
    /// The peer went away mid-body (normal when a download is cancelled).
    client_disconnected: AtomicBool,
}

// ============================================================================
// Server handle
// ============================================================================

/// A point-in-time snapshot of everything the server saw and did.
#[derive(Debug, Clone)]
pub struct Observation {
    pub requests: Vec<RecordedRequest>,
    pub statuses: Vec<u16>,
    pub bytes_served: u64,
    pub body_truncated: bool,
    pub client_disconnected: bool,
}

/// A running loopback origin server. Owns its accept task; drop or [`TestHttpServer::shutdown`]
/// to release it.
pub struct TestHttpServer {
    addr: SocketAddr,
    state: Arc<ServerState>,
    progress_rx: Mutex<mpsc::Receiver<u64>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl TestHttpServer {
    /// Binds `127.0.0.1:0` (so parallel tests never collide on a fixed port) and
    /// starts serving `body` under `config`.
    pub async fn start(body: Vec<u8>, config: ServeConfig) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind loopback test HTTP server");
        let addr = listener
            .local_addr()
            .expect("Failed to read assigned loopback port");

        let state = Arc::new(ServerState::default());
        let (progress_tx, progress_rx) = mpsc::channel::<u64>(256);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let task_state = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            accept_loop(listener, body, config, task_state, progress_tx, shutdown_rx).await;
        });

        Self {
            addr,
            state,
            progress_rx: Mutex::new(progress_rx),
            shutdown_tx: Some(shutdown_tx),
            handle: Some(handle),
        }
    }

    /// Base URL to hand to `ModelManager::setup_model`.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Every request received, in arrival order.
    pub async fn recorded_requests(&self) -> Vec<RecordedRequest> {
        self.state.requests.lock().await.clone()
    }

    /// The `Range` header values received, in arrival order (`None` = header absent).
    pub async fn range_headers(&self) -> Vec<Option<String>> {
        self.state
            .requests
            .lock()
            .await
            .iter()
            .map(|r| r.range_header.clone())
            .collect()
    }

    /// Snapshots everything observable. Take this *before* [`TestHttpServer::shutdown`],
    /// which consumes the handle.
    pub async fn observe(&self) -> Observation {
        Observation {
            requests: self.state.requests.lock().await.clone(),
            statuses: self.state.statuses.lock().await.clone(),
            bytes_served: self.bytes_served(),
            body_truncated: self.body_truncated(),
            client_disconnected: self.client_disconnected(),
        }
    }

    /// HTTP status codes this server sent back, in order.
    pub async fn served_statuses(&self) -> Vec<u16> {
        self.state.statuses.lock().await.clone()
    }

    /// Total response-body bytes actually written to the socket.
    pub fn bytes_served(&self) -> u64 {
        self.state.bytes_served.load(Ordering::SeqCst)
    }

    /// True if the server under-delivered relative to its own `Content-Length`.
    pub fn body_truncated(&self) -> bool {
        self.state.body_truncated.load(Ordering::SeqCst)
    }

    /// True if the peer hung up before the body was fully written.
    pub fn client_disconnected(&self) -> bool {
        self.state.client_disconnected.load(Ordering::SeqCst)
    }

    /// Awaits the next drip notification: the running total of body bytes written.
    ///
    /// This is how a test learns that a transfer is genuinely mid-flight and can
    /// therefore cancel it at a real mid-transfer instant rather than guessing with
    /// a sleep.
    pub async fn wait_for_progress(&self) -> Option<u64> {
        self.progress_rx.lock().await.recv().await
    }

    /// Signals the accept loop to stop and awaits the task, then hard-aborts on a
    /// short deadline so a wedged connection can never hang the test process.
    pub async fn shutdown(mut self) {
        self.stop().await;
    }

    async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            // `timeout` owns the handle; on expiry the task is dropped, which for a
            // `JoinHandle` aborts it. A wedged connection must never hang the suite.
            tokio::time::timeout(Duration::from_secs(5), handle)
                .await
                .ok();
        }
    }
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

// ============================================================================
// Server internals
// ============================================================================

async fn accept_loop(
    listener: TcpListener,
    body: Vec<u8>,
    config: ServeConfig,
    state: Arc<ServerState>,
    progress_tx: mpsc::Sender<u64>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => return,
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _peer)) => {
                        handle_connection(
                            stream,
                            body.clone(),
                            &config,
                            &state,
                            progress_tx.clone(),
                        )
                        .await;
                    }
                    Err(_) => return,
                }
            }
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    body: Vec<u8>,
    config: &ServeConfig,
    state: &Arc<ServerState>,
    progress_tx: mpsc::Sender<u64>,
) {
    let head = match read_request_head(&mut stream).await {
        Some(head) => head,
        None => return,
    };
    let request = parse_request_head(&head);
    state.requests.lock().await.push(request.clone());

    let total = body.len();
    let (status_line, extra_headers, payload): (&str, Vec<(&str, String)>, &[u8]) =
        match config.mode {
            ServeMode::IgnoreRangeReturn200 => {
                // Deliberately ignores the Range header entirely.
                state.statuses.lock().await.push(200);
                ("200 OK", Vec::new(), &body)
            }
            ServeMode::RangeNotSatisfiable => match request.range_start {
                Some(_) => {
                    state.statuses.lock().await.push(416);
                    (
                        "416 Range Not Satisfiable",
                        vec![("Content-Range", format!("bytes */{total}"))],
                        &body[0..0],
                    )
                }
                None => {
                    state.statuses.lock().await.push(200);
                    ("200 OK", Vec::new(), &body)
                }
            },
            ServeMode::HonourRange => match request.range_start {
                Some(start) => {
                    let start = start.min(total as u64) as usize;
                    let tail = &body[start..];
                    state.statuses.lock().await.push(206);
                    (
                        "206 Partial Content",
                        vec![(
                            "Content-Range",
                            format!("bytes {}-{}/{}", start, total.saturating_sub(1), total),
                        )],
                        tail,
                    )
                }
                None => {
                    state.statuses.lock().await.push(200);
                    ("200 OK", Vec::new(), &body)
                }
            },
        };

    // RFC 9110 §15.3.7: "A Content-Length header field present in a 206 response
    // indicates the number of octets in the content of this message, which is
    // usually not the complete length of the selected representation. Each
    // Content-Range header field includes information about the selected
    // representation's complete length." So a 206 advertises the LENGTH OF THE
    // RANGE, not the entity. §8.6 also forbids emitting a Content-Length "known to
    // be incorrect". Adopting the whole-entity length here was a fixture bug: a
    // ranged client waits for it, hits EOF mid-body, and reports a transport error
    // instead of exercising the resume logic at all.
    //
    // The one deliberate exception is `truncate_after`, which over-advertises the
    // FULL entity length and then hangs up. That lie is what makes a dropped
    // connection distinguishable from a short-but-complete body.
    let advertised_len = match config.truncate_after {
        Some(_) => total,
        None => payload.len(),
    };
    let mut head_out = format!(
        "HTTP/1.1 {status_line}\r\nContent-Length: {advertised_len}\r\nContent-Type: application/octet-stream\r\nAccept-Ranges: bytes\r\nConnection: close\r\n"
    );
    for (name, value) in extra_headers {
        head_out.push_str(&format!("{name}: {value}\r\n"));
    }
    head_out.push_str("\r\n");

    if stream.write_all(head_out.as_bytes()).await.is_err() {
        state.client_disconnected.store(true, Ordering::SeqCst);
        return;
    }

    let deliver = match config.truncate_after {
        Some(cap) => payload.len().min(cap),
        None => payload.len(),
    };
    let chunk_size = config.drip_chunk.unwrap_or(deliver).max(1);

    let mut written_total: u64 = 0;
    for slice in payload[..deliver].chunks(chunk_size) {
        if stream.write_all(slice).await.is_err() {
            state.client_disconnected.store(true, Ordering::SeqCst);
            break;
        }
        written_total += slice.len() as u64;
        state
            .bytes_served
            .fetch_add(slice.len() as u64, Ordering::SeqCst);
        // Bounded channel; a full buffer means the test stopped listening, which is
        // not an error for the server.
        let _ = progress_tx.send(written_total).await;
        if !config.drip_delay.is_zero() {
            tokio::time::sleep(config.drip_delay).await;
        }
    }

    // We advertised `advertised_len` but under-delivered, then hung up: this is the
    // dropped-connection case the client must recover from by keeping its `.tmp`.
    if deliver < advertised_len {
        state.body_truncated.store(true, Ordering::SeqCst);
    }

    let _ = stream.flush().await;
    drop(stream);
}

/// Reads until the `\r\n\r\n` terminator, or returns `None` if the peer sent
/// nothing / blew past [`MAX_REQUEST_HEAD`].
async fn read_request_head(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => n,
        };
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            return Some(buf);
        }
        if buf.len() > MAX_REQUEST_HEAD {
            panic!("Request head exceeded {MAX_REQUEST_HEAD} bytes; test server is not framing a valid request");
        }
    }
}

fn parse_request_head(raw: &[u8]) -> RecordedRequest {
    let text = String::from_utf8_lossy(raw);
    let mut lines = text.split("\r\n");

    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();

    let mut range_header = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("range") {
                range_header = Some(value.trim().to_string());
                break;
            }
        }
    }

    RecordedRequest {
        range_start: range_header.as_deref().and_then(parse_byte_range_start),
        method,
        path,
        range_header,
    }
}

/// Parses the start offset out of `bytes=<start>-` or `bytes=<start>-<end>`.
fn parse_byte_range_start(value: &str) -> Option<u64> {
    let spec = value.strip_prefix("bytes=")?;
    let first = spec.split(',').next()?.trim();
    let start = first.split('-').next()?.trim();
    start.parse::<u64>().ok()
}
