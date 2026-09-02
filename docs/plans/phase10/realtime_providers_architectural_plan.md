# Realtime Provider Refactor — Transport & Driver Architecture

> **Goal:** Eliminate ~1,100 LOC of copy-paste reconnection boilerplate shared between `gemini_live.rs` (1,061 LOC) and `deepgram_live.rs` (827 LOC) by extracting a shared `transport/` harness and leaving only thin `providers/` protocol drivers. Every file produced must be ≤ 300 LOC.

---

## Naming Decision: `OutboundCommand` (Not `ControlEvent`, Not `OutboundControlChannel`)

> [!IMPORTANT]
> **User asked: should the outbound control channel be named to mirror other commands in the system?**

**Yes — and the correct name is `OutboundCommand`.** Here is the reasoning:

The current files already have a private `ControlEvent` enum (`ActivityStart`, `ActivityEnd`, `Interrupt` in Gemini; `Interrupt` in Deepgram). The draft plan called it `OutboundMessage`. The right name, grounded in the existing codebase vocabulary, is `OutboundCommand`:

| Existing enum | Domain | Pattern |
|:---|:---|:---|
| `VadCommand` | `services/vad/` | Commands dispatched *into* the VAD actor |
| `TtsCommand` | `services/tts/` | Commands dispatched *into* the TTS actor |
| `LlmCommand` | `services/llm/` | Commands dispatched *into* the LLM actor |
| `SttCommand` | `services/stt/` | Commands dispatched *into* the STT actor |
| **`OutboundCommand`** | `services/realtime/transport/` | Commands dispatched *out to* the WebSocket wire |

`OutboundCommand` is:
- Directionally precise (`Outbound` = towards the server)
- Consistent with the `*Command` suffix that identifies the message type every actor in this codebase uses for typed channel communication
- Distinct from `RealtimeProviderEvent` (which flows *inbound*, from provider → actor)

The full proposed enum (lives in `transport/mod.rs`):

```rust
/// Typed wire commands dispatched outbound to the WebSocket connection writer.
pub enum OutboundCommand {
    Audio(Vec<i16>),         // raw PCM to be encoded provider-specifically
    ActivityStart,           // Gemini: activityStart frame; Deepgram: no-op (omitted at wire layer)
    ActivityEnd,             // Gemini: activityEnd frame; Deepgram: no-op
    Interrupt,               // Gemini: activityStart+activityEnd; Deepgram: {"type":"Clear"}
    KeepAlive,               // Gemini: no-op; Deepgram: {"type":"KeepAlive"} every 4s
}
```

Provider drivers implement a single method `encode(&self, cmd: OutboundCommand) -> Option<Message>` that maps the semantic command to the wire frame (or `None` to no-op it). The shared connection harness holds a `mpsc::Sender<OutboundCommand>` and calls `encode` before writing to the WebSocket. This gives you:
1. **Single FIFO queue**: `ActivityStart` is guaranteed to precede `Audio` on the wire — the race condition in the current dual-channel design is eliminated by construction.
2. **No provider coupling in session callers**: `commit_speech_turn` sends `[ActivityStart, Audio(pcm), ActivityEnd]`. Neither the harness nor the callers care which provider is underneath.

---

## Pitfalls & Risks Identified in Live Code

### 🔴 P0 — Critical (Will Break or Already Broken)

**P1: Wire-Ordering Race in `commit_speech_turn` (Both Files)**
- **Where:** `GeminiLiveSession::commit_speech_turn` (L829–845), `DeepgramVoiceAgentSession::commit_speech_turn` (L668–671)
- **Problem:** Gemini sends `ActivityStart` on `control_tx`, then PCM on `audio_tx`, then `ActivityEnd` on `control_tx`. These are two *separate* bounded mpsc channels. The reconnect-loop task drives a `tokio::select!` without bias — under backpressure (which is normal when the model is streaming audio responses), audio frames can be in `audio_rx` when the `select!` polls and `ActivityStart` hasn't been consumed yet. This is a real wire-ordering bug: audio arrives at the server before the activity framing envelope, which causes Gemini to discard or misinterpret the turn.
- **Fix:** Single `mpsc::Sender<OutboundCommand>` FIFO. No select, no race.

**P2: `ws_connected` AtomicBool — Banned Synthetic Flag (Both Files)**
- **Where:** `ws_connected: Arc<AtomicBool>` in both providers (Gemini L134, Deepgram L120). Style Guide §7.2 explicitly bans synthetic lifecycle booleans.
- **Problem:** It is read in the keepalive loop and written on disconnect. This is not a "pure binary hardware condition" — it shadows the session lifecycle. When the reconnect loop re-opens the socket, `ws_connected` is set back to `true` but the keepalive task has no way to know it was stopped and restarted; it just keeps looping.
- **Fix:** Eliminate `ws_connected`. The keepalive task is owned by the harness and shut down via `CancellationToken` on disconnect/reconnect. Reconnect respawns it.

**P3: Task Leak on Partial Connect Failure**
- **Where:** Both `connect()` methods spawn `audio_sender_task`, `control_sender_task`, and `keepalive_task` unconditionally before the reconnect harness loop is even started. If the reconnect harness itself fails to spawn (OOM), those tasks leak.
- **Fix:** In the new design, the harness controls all task lifetimes as a single `DuplexHarness` struct with a `JoinSet` or explicit abort handles — tasks are only created inside the harness, never before.

**P4: Receiver Task Duplicated Verbatim During Reconnect (~280 LOC)**
- **Where:** Gemini L439–515, Deepgram L352–393. The receiver task body inside the reconnect branch is an exact 100% copy of the initial receiver task (L291–368 for Gemini, L239–286 for Deepgram). This means a bug fixed in one is silently unfixed in the other path.
- **Fix:** A single `spawn_receiver_task(ws_read, ...)` free function, called from both initial connect and reconnect.

**P5: `SessionState` Duplicated Across Both Files With Diverging Invariants**
- **Where:** `SessionState` struct in Gemini (L773–803) and Deepgram (L614–643). Gemini's has `interrupt_active`, `resume_handle`, `model`; Deepgram's has `last_assistant_text`. Both have identical `current_or_new_turn_id` / `peek_or_current_turn_id` with identical `fetch_add` logic.
- **Problem:** `current_or_new_turn_id` directly calls `self.turn_id.fetch_add(1, Ordering::Relaxed)` — this bypasses the `PipelineAtomics::next_turn()` SSOT invariant (AGENTS.md §4.1, Invariant 7). Turn IDs must be allocated at turn boundaries via the canonical helper, not fragmented `fetch_add` calls inside session state.
- **Fix:** `SessionState` is not duplicated. Provider-specific fields are kept in the driver. Turn allocation is removed from session state entirely — providers emit `RealtimeProviderEvent` with the epoch-allocated turn ID handed to them at construction; the `server_turn_cursor` tracking stays in the driver's message handler.

**P6: `ControlEvent` Enum is Private and Unnamed Across Both Files**
- **Where:** `enum ControlEvent { ... }` defined at module bottom of each file (Gemini L767, Deepgram L610).
- **Problem:** Semantically identical concept, different variants, no shared abstraction. Callers (session `commit_speech_turn`, `cancel`) reach across to an opaque private enum — the coupling is invisible to the type system.
- **Fix:** `OutboundCommand` is public within the crate (`pub(crate)`), defined once in `transport/mod.rs`, used by both session implementations and the harness.

### 🟠 P1 — Real Cost (Correctness Under Load)

**P7: Unbounded WS Writer Channel (`unbounded_channel`)**
- **Where:** `tokio::sync::mpsc::unbounded_channel::<Message>()` in both write tasks (Gemini L273, Deepgram L221).
- **Problem:** Under backpressure (slow WebSocket write, network hiccup), audio and control frames accumulate in an unbounded heap queue. On an 8GB machine this is a confirmed OOM path for long sessions.
- **Fix:** Replace with a bounded `mpsc::channel(BRIDGE_CHANNEL_CAPACITY)` with non-blocking `try_send`. The `write_task` uses `recv()` — if the channel fills, `try_send` drops frames (with a logged counter) rather than buffering forever.

**P8: Gemini `audio_sender_task` Allocates Micro-Batches On Heap**
- **Where:** `pcm_batch.extend(extra)` loop (L160–167). Every `extend` is a heap reallocation.
- **Problem:** The intent is micro-batching to ≥ 960 samples. But `Vec::extend` can trigger multiple reallocs. On a hot audio path this is a minor but real allocation cost.
- **Fix:** Pre-allocate with `Vec::with_capacity(960)` and use `extend_from_slice`. The driver owns this logic; the harness just passes `OutboundCommand::Audio(pcm)` through.

**P9: Grammar Order Violations**
- Both files define `ControlEvent`, `SessionState`, `perform_handshake`, and session structs **after** the trait impls, which violates §2.1 (Standard Rust File Grammar Order). Enums and structs must precede impls.

**P10: `ws_connected` Keepalive Loop is a Polling Loop (Deepgram)**
- **Where:** Deepgram keepalive task (L194–219) polls `ws_connected.load(Ordering::SeqCst)` in a `while` loop with a `sleep`.
- **Problem:** §6 bans polling where events suffice. The keepalive should be driven by a `CancellationToken` that the harness cancels on disconnect.

**P11: Inline `serde_json::json!` 4-Level Deep in Task Bodies**
- **Where:** Gemini `audio_sender_task` (L176–184), `control_sender_task` (L208–258), `perform_handshake` (L613–680).
- **Problem:** Ad-hoc JSON mutation scattered inside closures. Fails style §3.4 ("Typed protocol structs"). A serialization bug in setup config requires hunting through a 600-line closure chain.
- **Fix:** Each driver's `protocol.rs` owns typed `#[derive(Serialize)]` structs. `perform_handshake` just serializes them.

### 🟡 P2 — Style / Minor

**P12:** `unwrap_or_default()` on JSON parse in handshake (Gemini L699, L713) silently returns `serde_json::Value::Null` on parse error, masking protocol deviations.
**P13:** `log::info!` in the Gemini audio sender task (`packet_count.is_multiple_of(LOG_INTERVAL_PACKETS)`) runs inside an async Tokio task on every 100th packet — `log::debug!` is the right level for this.
**P14:** `GeminiLiveProvider::new` takes 6 arguments — exactly at the 5-arg struct bundling threshold (style §4). Should be `GeminiProviderHandles { state_rx, turn_id, turn_token, turn_epoch }` + `GeminiRealtimeConfig`.

---

## Open Questions

> [!IMPORTANT]
> **Q1 — `commit_speech_turn` for Passive/Continuous Mode?**
> Currently `AudioBridge` calls `session.send_audio(&resampled)` for continuous streaming (passive mode). `commit_speech_turn` is used only in PTT mode. With the new `OutboundCommand::Audio`, passive mode just sends `OutboundCommand::Audio(chunk)` continuously. Gemini passive mode relies on the server's VAD (`automaticActivityDetection: { disabled: false }`), so no framing needed — audio flows raw. Confirm: should we retain `send_audio` as a distinct trait method, or collapse it into `commit_speech_turn` with a `PassiveAudio` variant? **Recommendation: retain `send_audio` as-is — it maps directly to `OutboundCommand::Audio`, zero ambiguity.**

> [!IMPORTANT]
> **Q2 — `ws_connected` Removal: Does Deepgram Keepalive Need a Session-Alive Signal?**
> If `ws_connected` is removed, the Deepgram keepalive task must be driven by a `CancellationToken`. The harness already holds a `CancellationToken` for the full session. Confirm: the keepalive token should be child of the session-level token so that disconnect() cancels both? **Yes — this is the safe design.**

> [!NOTE]
> **Q3 — `OutboundCommand::KeepAlive` encoding for Gemini:**
> Gemini does not use explicit keepalives. `encode(KeepAlive) -> None` for Gemini (the harness skips the write). For Deepgram it encodes to `{"type":"KeepAlive"}`. This is a clean no-op via `Option<Message>`.

---

## Proposed File Layout

```
services/realtime/
├── mod.rs                    [MODIFY] — Add OutboundCommand enum, keep existing traits & constants
├── actor.rs                  [MODIFY] — Minor: uses RealtimeSession unchanged
├── audio_bridge.rs           [KEEP]   — No changes needed
│
├── transport/                [NEW DIRECTORY]
│   ├── mod.rs                [NEW] — DuplexHarness, HarnessConfig, HarnessHandles structs
│   ├── connection.rs         [NEW] — Reconnect loop, task lifecycle, shutdown (<200 LOC)
│   └── health.rs             [NEW] — Generic TCP health check (~25 LOC)
│
└── providers/
    ├── mod.rs                [MODIFY] — Add gemini + deepgram factory submodule re-exports
    ├── gemini/               [NEW DIRECTORY — replaces gemini_live.rs]
    │   ├── mod.rs            [NEW] — GeminiLiveProvider: new(), RealtimeVoiceProvider impl, health_check
    │   ├── handshake.rs      [NEW] — perform_handshake async fn (<100 LOC)
    │   ├── protocol.rs       [NEW] — Typed Serde structs for Gemini JSON frames
    │   └── session.rs        [NEW] — GeminiLiveSession: RealtimeSession impl + message decoder
    └── deepgram/             [NEW DIRECTORY — replaces deepgram_live.rs]
        ├── mod.rs            [NEW] — DeepgramVoiceAgentProvider: new(), RealtimeVoiceProvider impl
        ├── handshake.rs      [NEW] — perform_handshake async fn (<100 LOC)
        ├── protocol.rs       [NEW] — Typed Serde structs for Deepgram JSON frames
        └── session.rs        [NEW] — DeepgramVoiceAgentSession: RealtimeSession impl + message decoder
```

**Target LOC budget per file:**

| File | Target |
|:---|:---:|
| `transport/mod.rs` | ≤ 80 |
| `transport/connection.rs` | ≤ 220 |
| `transport/health.rs` | ≤ 30 |
| `gemini/mod.rs` | ≤ 120 |
| `gemini/handshake.rs` | ≤ 120 |
| `gemini/protocol.rs` | ≤ 100 |
| `gemini/session.rs` | ≤ 200 |
| `deepgram/mod.rs` | ≤ 100 |
| `deepgram/handshake.rs` | ≤ 100 |
| `deepgram/protocol.rs` | ≤ 80 |
| `deepgram/session.rs` | ≤ 180 |

---

## Phased Delivery

### Phase 1 — Foundation (Non-Breaking, No Deletion)

> Establish the shared abstractions without deleting the old providers. At the end of Phase 1, the codebase compiles and passes all tests. The old providers still exist but are unused.

**Sprint 1.1 — Define `OutboundCommand` in `mod.rs`**

Files changing:
- `services/realtime/mod.rs` — add `OutboundCommand` enum below `RealtimeProviderEvent`

Thread context: None (data type declaration only).
Contract change: Additive only — no existing variant changed.

```rust
/// Typed commands dispatched outbound from session callers to the WebSocket connection writer.
pub enum OutboundCommand {
    /// Raw PCM audio samples to be encoded per-provider (base64 JSON for Gemini, binary for Deepgram).
    Audio(Vec<i16>),
    /// Explicit speech turn start marker (Gemini: activityStart frame; Deepgram: no-op).
    ActivityStart,
    /// Explicit speech turn end marker (Gemini: activityEnd frame; Deepgram: no-op).
    ActivityEnd,
    /// Barge-in / cancellation signal (Gemini: activityStart+activityEnd; Deepgram: Clear).
    Interrupt,
    /// Connection liveness ping (Gemini: no-op; Deepgram: KeepAlive JSON frame every 4 s).
    KeepAlive,
}
```

**Sprint 1.2 — Define `ProviderEncoder` Trait in `transport/mod.rs`**

New file: `services/realtime/transport/mod.rs`

Thread context: None (trait definition).

```rust
/// Encodes a semantic OutboundCommand into a provider-specific WebSocket wire frame.
pub trait ProviderEncoder: Send + Sync {
    fn encode(&self, cmd: OutboundCommand) -> Option<tokio_tungstenite::tungstenite::Message>;
}

/// Typed configuration bundle for the shared DuplexHarness.
pub struct HarnessConfig {
    pub outbound_capacity: usize,
    pub max_reconnect_attempts: usize,
    pub reconnect_base_delay_secs: u64,
    pub reconnect_factor_secs: u64,
}

/// Handles returned to the session struct for sending commands and triggering shutdown.
pub struct HarnessHandles {
    pub outbound_tx: tokio::sync::mpsc::Sender<OutboundCommand>,
    pub shutdown_tx: parking_lot::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub terminated: Arc<std::sync::atomic::AtomicBool>,
}
```

**Sprint 1.3 — Implement `transport/health.rs`**

New file: `services/realtime/transport/health.rs`

Extracts the identical TCP health check from both providers into one function:

```rust
/// Performs a synchronous TCP health check against the given socket address.
pub fn tcp_health_check(addr: std::net::SocketAddr, timeout: std::time::Duration) -> bool {
    std::net::TcpStream::connect_timeout(&addr, timeout).is_ok()
}

/// Resolves a host string to a socket address with an optional fallback.
pub fn resolve_or_fallback(host: &str, fallback: std::net::SocketAddr) -> std::net::SocketAddr {
    use std::net::ToSocketAddrs;
    host.to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .unwrap_or(fallback)
}
```

**Sprint 1.4 — Implement `transport/connection.rs` — Shared Reconnect Harness**

New file: `services/realtime/transport/connection.rs` (~200 LOC)

This is the core of the refactor. The harness:
1. Owns the outbound channel (`mpsc::Sender<OutboundCommand>`)
2. Owns the write task (bounded channel, not unbounded)
3. Owns the receiver task via `spawn_receiver_task`
4. Owns the reconnect loop with exponential backoff
5. Uses `CancellationToken` for shutdown (no polling `AtomicBool`)

Key design decisions captured from the live code analysis:
- **`ws_sender` is now `AtomicCell<Option<Sender<Message>>>`** or equivalently swapped under a brief `Mutex` lock on reconnect — same pattern as today but encapsulated.
- **`spawn_receiver_task`** is a free function called from both initial connect and each reconnect cycle — eliminates the 280-LOC duplication.
- **Keepalive** is an optional `tokio::time::interval` task, started if `HarnessConfig::keepalive_interval.is_some()`, cancelled via `CancellationToken` child on reconnect.

```rust
/// Spawns the shared duplex WebSocket harness for a realtime provider session.
pub fn spawn_harness<E: ProviderEncoder + 'static>(
    encoder: Arc<E>,
    ws_write: WsWriter,
    ws_read: WsReader,
    reconnect_fn: impl Fn() -> BoxFuture<'static, Result<(WsWriter, WsReader)>> + Send + 'static,
    provider_event_tx: Sender<RealtimeProviderEvent>,
    config: HarnessConfig,
    session_token: CancellationToken,
    keepalive_interval: Option<Duration>,
) -> HarnessHandles { ... }
```

---

### Phase 2 — Gemini Driver Extraction

> Delete `gemini_live.rs` and replace with the `providers/gemini/` sub-module. At the end of Phase 2, Gemini Live is fully on the new harness, all tests pass, 0 clippy warnings.

**Sprint 2.1 — `providers/gemini/protocol.rs`**

New file: `services/realtime/providers/gemini/protocol.rs`

Replace all `serde_json::json!({ ... })` macros with typed structs:

```rust
#[derive(Serialize)]
pub struct GeminiSetupFrame { pub setup: GeminiSetupPayload }
#[derive(Serialize)]
pub struct GeminiRealtimeInputFrame { ... }
#[derive(Deserialize)]
pub struct GeminiServerContent { ... }
// etc.
```

**Sprint 2.2 — `providers/gemini/handshake.rs`**

New file: `services/realtime/providers/gemini/handshake.rs`

Extract `perform_handshake` from `gemini_live.rs`. Uses typed structs from `protocol.rs`. Handles `sessionResumption` handle injection and `setupComplete` wait loop.

**Sprint 2.3 — `providers/gemini/session.rs`**

New file: `services/realtime/providers/gemini/session.rs`

`GeminiLiveSession` now holds:
- `outbound_tx: Sender<OutboundCommand>` (from harness `HarnessHandles`)
- `shutdown_tx: Mutex<Option<oneshot::Sender<()>>>`
- `terminated: Arc<AtomicBool>`

`commit_speech_turn` implementation:
```rust
fn commit_speech_turn(&self, pcm: &[i16]) -> Result<()> {
    self.outbound_tx.try_send(OutboundCommand::ActivityStart)?;
    if let Err(e) = self.outbound_tx.try_send(OutboundCommand::Audio(pcm.to_vec())) { ... }
    self.outbound_tx.try_send(OutboundCommand::ActivityEnd)?;
    Ok(())
}
```

Because these three sends target the **same** FIFO channel, wire ordering is guaranteed by construction. The encoder at the write end serializes them sequentially.

Also contains `GeminiEncoder: ProviderEncoder` and `decode_server_message` for inbound frame parsing. The `SessionState` fields specific to Gemini (`interrupt_active`, `resume_handle`, `model`) live in a local `GeminiSessionState` — not duplicated.

**Sprint 2.4 — `providers/gemini/mod.rs`**

New file: `services/realtime/providers/gemini/mod.rs`

`GeminiLiveProvider` implements `RealtimeVoiceProvider::connect()` by:
1. Calling `handshake::perform_handshake(...)`
2. Constructing `GeminiEncoder`
3. Calling `transport::connection::spawn_harness(encoder, ws_write, ws_read, reconnect_fn, ...)`
4. Returning `(Box::new(GeminiLiveSession { handles }), provider_event_rx)`

**Sprint 2.5 — Delete `gemini_live.rs`, Update `providers/mod.rs`**

Delete old file, update `providers/mod.rs` to re-export from `gemini/`.
Run: `cargo check --all-targets && cargo clippy --all-targets -- -D warnings`

---

### Phase 3 — Deepgram Driver Extraction

> Same pattern as Phase 2. Delete `deepgram_live.rs` and replace with `providers/deepgram/`. At end, all tests pass, 0 clippy warnings.

**Sprint 3.1 — `providers/deepgram/protocol.rs`**

Typed Serde structs for Deepgram Settings, KeepAlive, Clear, ConversationText, AgentAudioDone, UserStartedSpeaking.

**Sprint 3.2 — `providers/deepgram/handshake.rs`**

Extract `perform_handshake` from `deepgram_live.rs`. Handles Authorization header, Settings frame, Welcome+SettingsApplied wait loop.

**Sprint 3.3 — `providers/deepgram/session.rs`**

`DeepgramVoiceAgentSession` with `outbound_tx: Sender<OutboundCommand>`.

`DeepgramEncoder: ProviderEncoder` maps:
- `OutboundCommand::Audio(pcm)` → `Message::Binary(pcm_bytes)` (raw binary, no base64)
- `OutboundCommand::Interrupt` → `Message::Text({"type":"Clear"})`
- `OutboundCommand::KeepAlive` → `Message::Text({"type":"KeepAlive"})`
- `OutboundCommand::ActivityStart` / `ActivityEnd` → `None` (no-op for Deepgram)

Inbound `decode_server_message` handles `UserStartedSpeaking`, `ConversationText`, `AgentAudioDone`, `Error/Warning`, `FunctionCallRequest`.

**Sprint 3.4 — `providers/deepgram/mod.rs`**

`DeepgramVoiceAgentProvider` mirrors Gemini: `connect()` → handshake → harness → return session.

**Sprint 3.5 — Delete `deepgram_live.rs`, Verify**

Run: `cargo check --all-targets && cargo clippy --all-targets -- -D warnings`
Run: `cargo nextest run --release --test-threads=1`

---

### Phase 4 — Turn ID Fix & Cleanup

> Fix the P5 `fetch_add` bypass of `PipelineAtomics::next_turn()` SSOT and remove `ws_connected` synthetic flag. This is the most invariant-sensitive phase.

**Sprint 4.1 — Remove `fetch_add` From Session State**

Current `SessionState::current_or_new_turn_id()` calls `self.turn_id.fetch_add(1)` directly. This bypasses the `PipelineAtomics::next_turn()` SSOT.

Fix: Providers no longer own `turn_id: Arc<AtomicU32>`. Instead, the `server_turn_cursor: Option<u32>` is tracked per-session (in `GeminiSessionState`/`DeepgramSessionState`). When the server opens a new turn (first audio chunk or `ConversationText` for a new exchange), the cursor is set to the **current** `turn_id` value read via `Ordering::Relaxed` load (no increment — the increment was already done at `ptt_start` via `PipelineAtomics::next_turn()`). This preserves the server-cursor tracking without fragmenting `fetch_add`.

**Sprint 4.2 — Remove `ws_connected` AtomicBool**

The `ws_connected: Arc<AtomicBool>` is eliminated. The harness owns a `CancellationToken` hierarchy:
- `session_token` (from caller, controls entire session)
  - `connection_token` (child, cancelled and recreated on each reconnect)
    - `keepalive_token` (child, controls Deepgram keepalive task)

When the socket drops, `connection_token.cancel()` is called; the keepalive task exits. On reconnect, a new `connection_token` is derived from `session_token`.

**Sprint 4.3 — Final Lint & Test Gate**

```bash
cargo check --all-targets           # 0 errors
cargo clippy --all-targets -- -D warnings  # 0 warnings
cargo nextest run --release --test-threads=1  # all tests green
```

---

## Risk Register

| Risk | Severity | Mitigation |
|:---|:---:|:---|
| **Wire-ordering regression on Gemini PTT** | 🔴 | Single FIFO `OutboundCommand` channel eliminates this by construction. Integration test `realtime_ptt_test.rs` already exercises the commit-speech-turn path. |
| **Reconnect logic diverges from original behavior** | 🟠 | `spawn_receiver_task` is extracted as a named function, not inlined — the behavior is identical, just not duplicated. The reconnect loop logic in `connection.rs` is a direct mechanical extraction. |
| **`SessionState::fetch_add` removal changes turn ID sequence** | 🟠 | Providers no longer advance the global turn ID — they were not supposed to. The turn ID is read (not incremented) when a server turn opens. Existing tests verify turn ID monotonicity. |
| **Keepalive timing regression (Deepgram 4s interval)** | 🟡 | `WS_KEEPALIVE_INTERVAL = Duration::from_secs(4)` is already a constant in `mod.rs`. The harness uses `tokio::time::interval` with the same constant. |
| **`audio_bridge.rs` still uses `session.send_audio`** | 🟡 | `RealtimeSession::send_audio` is retained unchanged. The bridge calls it exactly as before. No contract change. |
| **Protocol struct rename breaks `provider_event_tx` type** | 🟡 | `RealtimeProviderEvent` type is unchanged — only the internal encoding of outbound frames changes. |

---

## Verification Plan

### Automated (After Each Phase)

```bash
# Syntax and type correctness
cargo check --all-targets

# Zero lint regressions
cargo clippy --all-targets -- -D warnings

# Full release test suite (40 tests, ~28s)
RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) \
  cargo nextest run --release --test-threads=1
```

### Manual (After Phase 3)

- [ ] Start Gemini Live session, speak a PTT turn → verify `ActivityStart` precedes audio on wire (Wireshark or provider log showing "turn committed" message before audio acknowledgment)
- [ ] Start Deepgram passive session, interrupt mid-response → verify `Clear` message is sent and `UserStartedSpeaking` triggers correct pipeline `Interrupted` event
- [ ] Simulate Gemini `goAway` by disconnecting network mid-session → verify reconnect loop triggers, session cache is not cleared, session resumes with stored handle

### Structure Check (After Phase 4)

```bash
# Verify no file exceeds 600 LOC ceiling
find app/src-tauri/src/services/realtime -name '*.rs' | xargs wc -l | sort -rn | head -20

# Verify old god files are gone
ls app/src-tauri/src/services/realtime/providers/
# Expected: gemini/  deepgram/  mod.rs  (no gemini_live.rs, no deepgram_live.rs)
```

---

## AGENTS.md Update (Post-Completion)

After all phases complete, append to Section 5:
> **Realtime Transport & Driver Refactor (Phase 10):** Eliminated 1,100 LOC of copy-paste reconnect boilerplate by extracting `services/realtime/transport/` shared harness (`connection.rs`, `health.rs`). Replaced `gemini_live.rs` (1,061 LOC) and `deepgram_live.rs` (827 LOC) with 11 focused files (each ≤ 300 LOC). Introduced `OutboundCommand` enum (single FIFO channel) eliminating the `ActivityStart`/Audio wire-ordering race. Removed banned `ws_connected: AtomicBool`, replaced with `CancellationToken` hierarchy. Fixed `fetch_add` bypass of `PipelineAtomics::next_turn()` SSOT in session state.
