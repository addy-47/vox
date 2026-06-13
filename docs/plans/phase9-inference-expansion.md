# Phase 9 - Inference Expansion + Cloud Integration

##  Vox v0.8.4 — LLM Provider Architecture Refactor - Objective

Refactor the current LLM implementation from a single embedded backend into a provider-based architecture.

This release is not about adding cloud AI providers yet.

This release is about creating the abstraction layer that allows Vox to support:

* Embedded local inference
* Remote inference servers
* Future cloud providers

without requiring future pipeline rewrites.

The goal is architectural decoupling.

---

## Problem

Current Vox treats the LLM as a concrete implementation inside the voice pipeline.

```text
STT → LLM → TTS
```

This works for embedded inference but creates scaling problems:

* Every new backend requires special-case logic
* Cloud providers become difficult to integrate
* Remote servers require pipeline modifications
* Future STT/TTS provider support becomes inconsistent

The LLM should be treated as a provider, not an implementation.

---

## Architectural Direction

Move from:

```text
Vox
 └─ Embedded LLM
```

to:

```text
Vox
 └─ LLM Provider Layer
        ├─ Embedded
        └─ OpenAI-Compatible
```

The voice pipeline should not know where inference occurs.

The pipeline should only know:

```text
Generate
Stream Tokens
Cancel
Health Check
List Models
```

Everything else becomes provider responsibility.

---

## Provider Types (v0.8.4)

### Embedded

Current local implementation.

Characteristics:

* Runs inside Vox process
* Uses local GGUF models
* No network dependency
* Existing functionality preserved

### OpenAI-Compatible

Represents any server exposing OpenAI-compatible APIs.

Examples include:

* Ollama
* LM Studio
* vLLM
* llama.cpp server
* LocalAI
* OpenWebUI backends
* Self-hosted inference servers

These should all be treated as a single provider category because they expose largely compatible APIs and streaming behavior.

---

## Core Principle

The backend should care about:

```text
Protocol
```

not:

```text
Location
```

Examples:

```text
localhost
192.168.1.20
gpu-server.local
mydomain.com
AWS
```

All of these are simply endpoints.

The protocol remains the same.

---

## Required Capabilities

Every provider must support:

### Generation

Submit prompt and receive completion.

### Streaming

Receive tokens incrementally.

Streaming must remain first-class because Vox is a real-time voice system.

### Cancellation

Barge-in must continue functioning identically across all providers.

### Health Checks

Determine provider availability before use.

### Model Discovery

Fetch available models dynamically.

Users should not manually maintain model lists.

The provider reports available models.

---

## Pipeline Impact

The voice pipeline remains unchanged.

```text
Audio
 → STT
 → LLM Provider
 → TTS
```

Only the implementation behind the provider changes.

This prevents ripple effects across:

* VAD
* STT
* TTS
* UI
* Telemetry
* State management

---


## v0.8.5 — STT & TTS Provider Architecture (Brief)

The same trait-based provider pattern from v0.8.4 should be applied to STT and TTS,
enabling embedded + remote + future cloud providers for all three pipeline stages.
This is necessary groundwork — the cloud STT/TTS providers (e.g. Deepgram Aura,
Sarvam Bulbul) will be used as components within the pipeline that follows.

---

## v0.8.5 → v0.9.0 — Realtime Cloud S2S Providers

This is the primary deliverable for this phase. Vox will add a **new engine
abstraction** alongside the existing modular pipeline: the **Realtime Engine**,
which bypasses the STT → LLM → TTS chain entirely and instead runs a single
bidirectional WebSocket connection that accepts raw audio in and returns raw
audio out.

Only true **voice-in / voice-out (S2S)** providers are included — APIs where a
single connection handles the full voice conversation without the client stitching
STT, LLM, and TTS together.

---

## Realtime Engine Architecture

### Design Principle

The backend should care about:

```text
Protocol (WebSocket bidirectional audio streaming)
```

not about:

```text
How the provider internally does STT → LLM → TTS
```

All providers in this category accept PCM audio chunks over a WebSocket and
return PCM audio chunks over the same connection. The implementation differences
(sample rates, message formats, auth) are encapsulated behind a single trait.

### RealtimeVoiceProvider Trait

A new trait in `services/realtime/mod.rs`, separate from the existing
`LlmProvider` (which remains text-in/text-out for the modular pipeline):

```rust
pub trait RealtimeVoiceProvider: Send + Sync {
    /// Provider identifier for config & UI.
    fn kind(&self) -> RealtimeProviderKind;

    /// Declares audio format expectations for this provider.
    fn audio_config(&self) -> RealtimeAudioConfig;

    /// Connect to the S2S WebSocket endpoint.
    fn connect(&self) -> anyhow::Result<Box<dyn RealtimeSession>>;

    /// Quick reachability check (not a full connect).
    fn health_check(&self) -> bool;
}

pub trait RealtimeSession: Send {
    /// Send a PCM audio chunk to the provider.
    fn send_audio(&self, pcm: &[i16]) -> anyhow::Result<()>;

    /// Register a callback for incoming PCM audio from the provider.
    fn on_audio(&self, cb: Box<dyn Fn(&[i16]) + Send>);

    /// Register a callback for interruption/barge-in signals.
    fn on_interruption(&self, cb: Box<dyn Fn() + Send>);

    /// Cancel the current generation (barge-in).
    fn cancel(&self) -> anyhow::Result<()>;

    /// Close the session.
    fn disconnect(&self) -> anyhow::Result<()>;
}

pub struct RealtimeAudioConfig {
    pub input_sample_rate: u32,
    pub output_sample_rate: u32,
    pub requires_resampling: bool,
}
```

### Pipeline Routing

The pipeline (`services/pipeline.rs`) gains a mode check at the entry point:

```text
VAD detects speech
    │
    ├── Mode: modular ──→ STT → LLM → TTS (existing path)
    │
    └── Mode: realtime ──→ RealtimeVoiceProvider (new path)
                               │
                               ↓
                          Audio → Playback
```

The mode is set via settings and hot-swappable at runtime. When switching to
realtime mode, the existing modular pipeline is suspended and the VAD output
is routed to the WebSocket sender task instead of STT.

### Thread Model

The realtime engine introduces a **hybrid sync/async architecture** that differs
from the existing all-sync inference threads:

```text
┌─────────────────────────────────────────────────┐
│  SYNC OS THREADS (existing pattern)              │
│                                                   │
│  Audio Capture (cpal, Max priority)               │
│      │                                            │
│      ▼                                            │
│  VAD (Earshot)                                    │
│      │                                            │
│      ▼  PCM chunks                                │
│  Ring Buffer (Consumer)                           │
└──────┬──────────────────────────────────────────┘
       │  tokio::sync::mpsc::UnboundedSender
       │  (non-blocking, callable from sync code)
       ▼
┌─────────────────────────────────────────────────┐
│  TOKIO ASYNC TASK (new pattern)                  │
│                                                   │
│  Audio Bridge Task                                │
│      │  recv() from mpsc                          │
│      │  resample (rubato) if needed               │
│      │  base64-encode                             │
│      ▼                                            │
│  WebSocket Sender                                 │
│  ────────────────────→ Provider Cloud             │
│  WebSocket Receiver  ←────────────────────       │
│      │                                            │
│      │  base64-decode                             │
│      │  resample if needed                        │
│      ▼                                            │
│  Ring Buffer (Producer)                           │
└──────┬──────────────────────────────────────────┘
       │  lock-free atomic push
       ▼
┌─────────────────────────────────────────────────┐
│  SYNC OS THREAD (existing pattern)               │
│                                                   │
│  Playback (cpal, ringbuf Consumer)                │
│      ▼                                            │
│  Audio Output                                     │
└─────────────────────────────────────────────────┘
```

### Rust Crate Stack

```toml
# Core WebSocket + audio processing (required for all realtime providers)
tokio-tungstenite = { version = "0.29", features = ["rustls-tls-webpki-roots"] }
rubato = "3.0"                    # Sample rate conversion (16kHz↔24kHz etc.)
base64 = "0.22"                   # PCM audio base64 encoding

# Provider-specific (feature-gated, added per implementation)
gemini-live = { version = "0.1", optional = true }
openai_dive = { version = "1.4", optional = true }
```

---

## Technical Traps (Senior Architect's Warnings)

### ⚠️ A. Sample Rate Mismatch

Your local pipeline captures at 16 kHz (for VAD/STT). Each provider expects
different rates on the wire:

| Provider | Input Expected | Output Returns |
|----------|---------------|----------------|
| **Gemini Live** | **16 kHz** (match!) | 24 kHz (needs resampling to device rate) |
| **OpenAI Realtime** | **24 kHz** (must upsample) | 24 kHz |
| **Deepgram Voice Agent** | 16 kHz (default, flexible) | Configurable (default 24 kHz) |
| **ElevenLabs ConvAI** | **16 kHz** (match!) | **44.1 kHz** (needs heavy resampling) |

**The trap:** Sending 16 kHz audio to OpenAI's 24 kHz endpoint will produce
chipmunk-like distortion. Playing back 24 kHz audio at 48 kHz without
upsampling will sound slowed down. Vox must dynamically configure the
`rubato` resampler per active provider.

**The fix:** The `RealtimeAudioConfig` struct on each provider declares its
expected I/O sample rates. A shared `AudioResampler` utility wraps `rubato`
with per-provider configuration.

### ⚠️ B. Client-Side VAD (Cost Trap)

OpenAI and Gemini both offer "Server VAD" — you can open the microphone
permanently and let their servers figure out when you are talking.

**Do not do this.** Streaming continuous silence to the cloud drains bandwidth
and burns through API quotas because they charge for audio processing time.

**The fix:** Keep using Vox's local `Earshot` VAD. Only send audio over the
WebSocket when local VAD triggers `SpeechStart`. Send a flush/commit event
when local VAD triggers `SpeechEnd`. This is already how Vox's pipeline works
— it maps directly.

### ⚠️ C. Base64 JSON Overhead

Most of these APIs expect audio chunks as base64-encoded strings wrapped in
JSON payloads (e.g. `{"type": "input_audio_buffer.append", "audio": "//..."}`).

**The trap:** Allocating and encoding large JSON strings inside a hot audio
loop causes heap fragmentation and GC-like stuttering in Rust (repeated
allocations even without a GC).

**The fix:** Batch audio to **100ms chunks** before encoding. Do not send a
WebSocket message every 10ms. At 100ms granularity at 24 kHz:
- Raw PCM16: 4,800 bytes
- Base64: ~6,400 bytes
- Frequency: 10 messages/second
- This is trivially fast for `base64` crate — no SIMD optimization needed.

### ⚠️ D. WebSocket Binary vs Text Frames

- **Gemini Google AI endpoint**: JSON text frames only (base64 inside)
- **Gemini Vertex AI endpoint**: Binary frames for audio (handled by `gemini-live` crate)
- **OpenAI Realtime**: JSON text frames only
- **Deepgram Voice Agent**: Binary frames for audio + JSON for control messages
- **ElevenLabs ConvAI**: JSON text frames only

Vox's WebSocket layer must handle both frame types and route them correctly
per provider.

### ⚠️ E. Barge-In Must Be Local-First

Server-side barge-in has 200-500ms round-trip latency. By the time the server
signals interruption, the user has already heard 200-500ms of unwanted audio.

**The fix:** On local VAD speech detection during playback, Vox should:
1. Immediately mute/stop local audio playback (0ms — local)
2. Clear the playback ring buffer
3. Send cancel/interruption signal to the cloud provider
4. When the server confirmation arrives (200-500ms later), start new capture

This two-phase approach gives sub-100ms barge-in feel while keeping the
server in sync.

### ⚠️ F. Session Timeouts & Reconnection

| Provider | Max Session | Reconnect Strategy |
|----------|-------------|-------------------|
| Gemini Live | ~10 min (GoAway 60s before) | Session resumption token valid 2h |
| OpenAI Realtime | **60 min** | ~30s reconnect window |
| Deepgram Voice Agent | Not documented (practical: hours) | Reconnect from scratch |
| ElevenLabs ConvAI | 10 min default (configurable) | Signed URL valid 15 min; connection outlives |

**The fix:** Implement automatic reconnection with context restoration.
Gemini's session resumption tokens and the `gemini-live` crate's built-in
reconnection with exponential backoff should be the model.

---

## Provider: Google Gemini Multimodal Live API

### Endpoints

```text
# Google AI (free tier, API key auth)
wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent?key=YOUR_KEY

# Vertex AI (production, OAuth2 bearer token)
wss://{location}-aiplatform.googleapis.com/ws/google.cloud.aiplatform.v1.LlmBidiService/BidiGenerateContent
```

### Authentication
- **Free tier**: API key from [Google AI Studio](https://aistudio.google.com/app/apikey)
- **Production**: OAuth2 bearer token (Vertex AI) — use `gcloud auth print-access-token`
- **Client singleton pattern**: Initialize once per process (see production Python:
  `get_genai_client()` uses a module-level singleton that supports both `genai.Client(api_key=...)`
  and `genai.Client(vertexai=True, project=..., location=...)` depending on `USE_VERTEXAI` flag)
- **Best practice**: Ephemeral tokens generated server-side for client connections
- **Critical catch**: Enabling billing on a project **removes the free tier entirely** — use a separate project for testing

### Free Tier & Quotas
| Tier | RPM | RPD | TPM |
|------|-----|-----|-----|
| Free (Flash models) | ~10-15 | 1,500 | 250K |
| Paid — Tier 1 | $250/mo cap | — | 4M TPM |
| Paid — Tier 2 | $2,000/mo cap | — | 4M TPM |

### Pricing
| Model | Input Audio | Output Audio |
|-------|-------------|-------------|
| Gemini 3 Flash Live | $3.00/1M tokens (~$0.005/min) | $12.00/1M tokens (~$0.018/min) |
| Gemini 2.5 Flash Native Audio | $3.00/1M tokens | $12.00/1M tokens |

**Total: ~$0.036/min** — 6.4x cheaper than OpenAI Realtime.

### Audio Specs
- **Input**: 16 kHz PCM16 (`audio/pcm;rate=16000`) — **matches Vox natively, no resampling needed**
- **Output**: 24 kHz PCM16 — requires resampling to device playback rate
- **Chunk size**: 20-100ms recommended per WebSocket message. Production Python sends raw PCM
  blobs directly (no batching) — each VAD frame is forwarded immediately.
- **Encoding**: Base64 inside JSON text frames (Google AI) or binary frames (Vertex AI)

### Protocol — Production Architecture (from `gemini.py`)

#### Core Pattern: Two-Queue, Four-Task Architecture

The production Python code uses a **two-queue architecture** that is the most important
pattern to replicate in Rust. Audio and control messages are split into separate
queues to **eliminate head-of-line blocking** — a slow JSON control message must never
delay an audio chunk.

```text
WebSocket from browser (or VAD ringbuf in Vox)
    │
    ├── Binary (PCM audio)  →  audio_queue  →  Gemini Audio Send Task
    │                                              │
    │                                              ▼ send_realtime_input(audio=...)
    │
    └── JSON (text/control) →  control_queue →  Gemini Control Send Task
                                                   │
                                                   ▼
                                             send_realtime_input(text=...)
                                             send_realtime_input(activity_start=...)
                                             send_realtime_input(activity_end=...)

Gemini Receive Loop (model → browser/speaker):
    ├── model_turn.parts[].text            → forward as text events
    ├── model_turn.parts[].inline_data     → forward as binary audio to ringbuf
    ├── server_content.turn_complete       → trigger sync, reset turn state
    ├── server_content.input_transcription → forward STT transcript
    ├── server_content.output_transcription→ forward TTS transcript
    ├── server_content.interrupted         → reset interrupt state, flush
    ├── tool_call.function_calls           → execute tools, send responses
    ├── session_resumption_update          → store new handle, forward to frontend
    └── go_away                            → raise reconnect signal
```

#### Session Configuration (Literal from Production)

The production config sent at connection time includes every field shown below.
This is the exact configuration structure Vox's Rust implementation must replicate:

```json
{
  "tools": [
    {
      "googleSearchRetrieval": {}    // optional, for web_search
    },
    {
      "functionDeclarations": [
        // Tool definitions per application needs
      ]
    }
  ],
  "responseModalities": ["AUDIO"],
  "speechConfig": {
    "voiceConfig": {
      "prebuiltVoiceConfig": {
        "voiceName": "Charon"
      }
    },
    "languageCode": "hi-IN"
  },
  "systemInstruction": {
    "parts": [{"text": "Dynamic prompt built per-session with language, grounding, history, and project context."}]
  },
  "temperature": 0.2,
  "inputAudioTranscription": {},
  "outputAudioTranscription": {},
  "thinkingConfig": {
    "thinkingBudget": 0
  },
  "realtimeInputConfig": {
    "automaticActivityDetection": {
      "disabled": false,
      "startOfSpeechSensitivity": "START_SENSITIVITY_HIGH",
      "endOfSpeechSensitivity": "END_SENSITIVITY_HIGH",
      "prefixPaddingMs": 20,
      "silenceDurationMs": 100
    }
  },
  "sessionResumption": {
    "handle": "<resumption_handle_or_null>"
  }
}
```

Key details from production:
- **`responseModalities: ["AUDIO"]`** — text-only responses are not requested
- **`speechConfig.languageCode`** — BCP-47 code set per-session (e.g. `"hi-IN"`, `"en-US"`)
- **`temperature: 0.2`** — low temperature for consistent task-oriented responses
- **`inputAudioTranscription: {}`** — enables user speech transcription (empty = default params)
- **`outputAudioTranscription: {}`** — enables model TTS transcript (empty = default params)
- **`thinkingConfig: { thinkingBudget: 0 }`** — no reasoning tokens, pure response
- **VAD sensitivity**: `HIGH` for both start/end, `prefixPaddingMs: 20`, `silenceDurationMs: 100`
- **Session resumption**: handle passed from previous connection, null on first connect

#### Interruption Handling (Two-Phase, Production-Tested)

On user barge-in, the production code does **two things in sequence**:

**Phase 1 — Local Stop (immediate, 0ms):**
1. Stop/silence local playback immediately
2. Send `activity_start` event to Gemini (this natively interrupts model generation)
3. If in PTT mode: sleep 50ms, then send `activity_end` to close the open user turn
4. Set `interrupt_active = true` in session context

**Phase 2 — Server Confirmation (200-500ms later):**
1. On receiving `serverContent.interrupted` from Gemini:
   - Set `interrupt_active = false`
   - Trigger any pending sync
   - Reset `current_user_query` and `current_model_response`
   - Forward `{"interrupted": true}` to frontend

**During interrupt** (`interrupt_active == true`):
- All incoming `model_turn` content is silently dropped
- All incoming `input_transcription` / `output_transcription` is dropped
- Only tool calls and session events are processed

#### Turn Lifecycle

```text
1. User speaks → VAD detects → audio chunks sent via audio_queue
2. Gemini streams input_transcription (interim) → forwarded to frontend
3. Gemini detects end-of-speech → processes with LLM
4. Gemini streams:
     a. output_transcription (interim) → forwarded to frontend
     b. model_turn audio (inline_data) → queued for playback
     c. model_turn text → accumulated into current_model_response
5. On turn_complete:
     a. Trigger sync (persist turn to history)
     b. Send turn_complete event to frontend
     c. Increment completed_turns counter
     d. Reset current_user_query, current_model_response, turn_synced flags
     e. Log token usage (prompt_tokens + response_tokens)
```

The production code tracks two token counters per turn (`turn_prompt_tokens`,
`turn_response_tokens`) accumulated from `response.usage_metadata` and logs them
on `turn_complete`. This is useful for cost monitoring.

#### Tool Calling Pattern

The production code supports tool calling natively. Tools are defined in the
session config as `functionDeclarations`. When Gemini calls a tool:

```text
1. Receive tool_call with function_calls array
2. For each call:
     a. Match on call.name
     b. Execute tool (vector search, API call, etc.)
     c. Send response via genai_session.send_tool_response()
3. Tool responses can include status updates to frontend (e.g. "searching...")
```

Important: `interrupt_active` is reset to `false` on tool call receipt, since
tool calls are part of the model's turn and should not be suppressed.

#### Session Resumption

The production code handles session resumption to survive the ~10 min WebSocket
timeout:

```text
1. Receive session_resumption_update with new_handle
2. Forward handle to frontend for storage
3. On reconnection: pass handle in sessionResumption config
4. If resumption succeeds: context carries over seamlessly
5. If resumption fails: clear handle, start clean session
6. On reconnection loop exit (max 3 attempts): send fatal error to frontend
```

#### Reconnection Loop Pattern

The production code wraps the entire connection lifecycle in a reconnection loop:

```rust
// Pseudocode for Rust reconnection loop
let mut resume_handle: Option<String> = initial_handle;
let mut reconnect_attempts = 0;
const MAX_RECONNECT_ATTEMPTS: u32 = 3;

while browser_alive {
    let config = build_session_config(&resume_handle);
    
    match connect_and_run(config).await {
        Ok(()) => {
            reconnect_attempts = 0;  // reset on clean disconnect
            break;
        }
        Err(e) => {
            reconnect_attempts += 1;
            if reconnect_attempts >= MAX_RECONNECT_ATTEMPTS {
                send_error_to_frontend("Gemini connection lost permanently");
                break;
            }
            resume_handle = None;  // clear on failure
            sleep(Duration::from_secs(2 * reconnect_attempts)).await; // backoff
        }
    }
}
```

The inner `connect_and_run` function:
1. Establishes WebSocket with session config
2. Spawns 4 concurrent tasks: reader, audio_sender, control_sender, receiver
3. Uses `tokio::select!` or `FuturesUnordered` to wait for any task to complete
4. On GoAway: returns `Err` with reconnect signal (not a clean exit)
5. On browser disconnect: returns `Ok` (clean exit, break outer loop)
6. On Gemini receive error: cancels all sibling tasks, returns `Err`

#### GoAway Handling

When Gemini sends a `goAway` signal (60 seconds before termination):

1. Log the warning
2. Return an error from the receive loop to trigger reconnection
3. The resumption handle (previously received) enables seamless context transfer

#### Lifecycle / Cleanup Pattern

On session termination (browser disconnect or fatal error):

1. Cancel all 4 tasks if not already done
2. Final sync: persist any unsaved turn data
3. Log session summary (total turns, audio packets forwarded)
4. Close any external service connections

### Latency
| Metric | Value |
|--------|-------|
| TTFT (first audio token) | ~200-320ms (optimal), ~960ms (cold start) |
| Full A2A loop | ~770-1,415ms |
| Server VAD → interruption signal | 200-500ms |
| Client-side mute on barge-in | **<50ms** (Phase 1 local stop) |

### Rust Integration Architecture

**Recommended approach**: Raw `tokio-tungstenite` with custom protocol handling.
The `gemini-live` crate is a good reference but the production patterns (two-queue,
fine-grained turn lifecycle, tool calling) require direct control.

#### Rust Code Architecture: GeminiLiveProvider

```rust
// File: services/realtime/providers/gemini_live.rs

use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{StreamExt, SinkExt};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct GeminiLiveSession {
    // Two-queue architecture matching Python production
    audio_tx: mpsc::UnboundedSender<Vec<u8>>,      // PCM audio chunks
    control_tx: mpsc::UnboundedSender<ControlEvent>, // JSON control events
    
    // Shared session state for cross-task coordination
    state: Arc<Mutex<SessionState>>,
}

#[derive(Default)]
struct SessionState {
    current_user_query: String,
    current_model_response: String,
    turn_synced: bool,
    interrupt_active: bool,
    completed_turns: u32,
    turn_prompt_tokens: u32,
    turn_response_tokens: u32,
    resume_handle: Option<String>,
}

enum ControlEvent {
    Text(String),
    Interrupt,
    ActivityStart,
    ActivityEnd,
    AudioStreamEnd,
}

impl GeminiLiveSession {
    /// Connect and spawn the four production tasks.
    pub async fn connect(config: GeminiConfig) -> anyhow::Result<Self> {
        let (audio_tx, audio_rx) = mpsc::unbounded_channel();
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let state = Arc::new(Mutex::new(SessionState::default()));
        
        // Build session config JSON from GeminiConfig struct
        let setup_msg = build_setup_message(&config)?;
        
        // Connect WebSocket
        let (ws, _) = connect_async(&config.ws_url).await?;
        let (mut ws_write, mut ws_read) = ws.split();
        
        // Send setup
        ws_write.send(Message::Text(setup_msg.to_string())).await?;
        
        // Wait for setupComplete
        loop {
            match ws_read.next().await {
                Some(Ok(Message::Text(msg))) => {
                    let v: Value = serde_json::from_str(&msg)?;
                    if v.get("setupComplete").is_some() { break; }
                }
                _ => anyhow::bail!("Setup failed"),
            }
        }

        // Spawn Task 1: Read from VAD ringbuf → audio_queue
        tokio::spawn({
            let audio_tx = audio_tx.clone();
            async move { /* read from ringbuf, send to audio_tx */ }
        });

        // Spawn Task 2: Audio sender (audio_queue → Gemini)
        tokio::spawn({
            let mut ws_write = ws_write;
            async move {
                while let Some(chunk) = audio_rx.recv().await {
                    let msg = json!({
                        "realtimeInput": {
                            "audio": {
                                "data": base64_engine.encode(&chunk),
                                "mimeType": "audio/pcm;rate=16000"
                            }
                        }
                    });
                    ws_write.send(Message::Text(msg.to_string())).await.ok();
                }
            }
        });

        // Spawn Task 3: Control sender (control_queue → Gemini)
        tokio::spawn({
            let mut ws_write = ws_write;
            let state = state.clone();
            async move {
                while let Some(event) = control_rx.recv().await {
                    match event {
                        ControlEvent::Text(text) => {
                            let msg = json!({"realtimeInput": {"text": text}});
                            ws_write.send(Message::Text(msg.to_string())).await.ok();
                        }
                        ControlEvent::Interrupt => {
                            let mut s = state.lock().await;
                            s.interrupt_active = true;
                            // Send activity_start to interrupt model generation
                            let msg = json!({
                                "realtimeInput": {
                                    "activityStart": {}
                                }
                            });
                            ws_write.send(Message::Text(msg.to_string())).await.ok();
                            // In PTT: also send activity_end after brief delay
                            // send ActivityEnd after 50ms
                        }
                        ControlEvent::ActivityStart => {
                            let msg = json!({"realtimeInput": {"activityStart": {}}});
                            ws_write.send(Message::Text(msg.to_string())).await.ok();
                        }
                        ControlEvent::ActivityEnd => {
                            let msg = json!({"realtimeInput": {"activityEnd": {}}});
                            ws_write.send(Message::Text(msg.to_string())).await.ok();
                        }
                        ControlEvent::AudioStreamEnd => {
                            // Notify Gemini that user stopped speaking
                        }
                    }
                }
            }
        });

        // Spawn Task 4: Receive loop (Gemini → playback + frontend)
        tokio::spawn({
            let state = state.clone();
            let playback_tx = /* ringbuf producer */;
            let frontend_tx = /* event channel to UI */;
            async move {
                while let Some(Ok(msg)) = ws_read.next().await {
                    if let Message::Text(text) = msg {
                        let response: Value = serde_json::from_str(&text)?;
                        
                        // Handle GoAway
                        if response.get("goAway").is_some() {
                            anyhow::bail!("GoAway received — reconnect");
                        }
                        
                        // Handle usage metadata
                        if let Some(usage) = response.get("usageMetadata") {
                            // accumulate token counts
                        }
                        
                        // Handle server_content
                        if let Some(content) = response.get("serverContent") {
                            let mut s = state.lock().await;
                            
                            // Model turn: text + audio
                            if let Some(model_turn) = content.get("modelTurn") {
                                if s.interrupt_active { continue; }
                                
                                if let Some(parts) = model_turn.get("parts").and_then(|p| p.as_array()) {
                                    for part in parts {
                                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                            s.current_model_response.push_str(text);
                                            frontend_tx.send(Event::Text(text.to_string()));
                                        }
                                        if let Some(data) = part.get("inlineData") {
                                            if let Some(audio_b64) = data.get("data").and_then(|d| d.as_str()) {
                                                let pcm = base64_engine.decode(audio_b64)?;
                                                playback_tx.send(pcm); // to ringbuf
                                            }
                                        }
                                    }
                                }
                            }
                            
                            // Input transcription
                            if let Some(transcript) = content.get("inputTranscription") {
                                if !s.interrupt_active {
                                    // forward to frontend
                                }
                            }
                            
                            // Output transcription
                            if let Some(transcript) = content.get("outputTranscription") {
                                if !s.interrupt_active {
                                    s.current_model_response.push_str(/* text */);
                                    // forward to frontend
                                }
                            }
                            
                            // Turn complete
                            if content.get("turnComplete").is_some() {
                                s.interrupt_active = false;
                                // trigger sync
                                s.completed_turns += 1;
                                // reset turn state
                                s.current_user_query.clear();
                                s.current_model_response.clear();
                                s.turn_synced = false;
                                frontend_tx.send(Event::TurnComplete);
                            }
                            
                            // Interrupted
                            if content.get("interrupted").is_some() {
                                s.interrupt_active = false;
                                s.current_user_query.clear();
                                s.current_model_response.clear();
                                frontend_tx.send(Event::Interrupted);
                            }
                        }
                        
                        // Tool calls
                        if let Some(tool_call) = response.get("toolCall") {
                            // execute tools, send responses
                        }
                        
                        // Session resumption
                        if let Some(resumption) = response.get("sessionResumptionUpdate") {
                            if let Some(handle) = resumption.get("newHandle").and_then(|h| h.as_str()) {
                                s.resume_handle = Some(handle.to_string());
                                frontend_tx.send(Event::ResumptionHandle(handle.to_string()));
                            }
                        }
                    }
                }
                Ok::<(), anyhow::Error>(())
            }
        });

        Ok(Self { audio_tx, control_tx, state })
    }

    pub fn send_audio(&self, pcm: Vec<u8>) {
        self.audio_tx.send(pcm).ok();
    }

    pub fn interrupt(&self) {
        self.control_tx.send(ControlEvent::Interrupt).ok();
    }

    pub fn send_text(&self, text: String) {
        self.control_tx.send(ControlEvent::Text(text)).ok();
    }
}
```

#### System Prompt Building (Mapped from Production Python)

The production code builds a dynamic system instruction per-session. The Rust
implementation must replicate this composition:

```rust
fn build_system_prompt(config: &GeminiConfig) -> String {
    let mut parts = Vec::new();

    // 1. Base system prompt template
    parts.push(config.system_prompt_template.clone());

    // 2. Language instruction
    if let Some(lang) = &config.language_instruction {
        parts.push(lang.clone());
    }

    // 3. Grounding instructions (web search enabled/disabled, etc.)
    parts.push(config.grounding_instruction.clone());

    // 4. PRIOR SESSION CONTEXT — last N turns seeded from history
    if let Some(history) = &config.recent_turns {
        let history_block = history.iter().enumerate()
            .map(|(i, t)| format!("Turn {} User: {}\nTurn {} Assistant: {}", i+1, t.query, i+1, t.response))
            .collect::<Vec<_>>()
            .join("\n");
        parts.push(format!(
            "\n## PRIOR SESSION CONTEXT (MOST RECENT TURNS)\n\
            Use this for continuity; do not repeat it verbatim unless asked.\n{}",
            history_block
        ));
    }

    // 5. Project schema context (if available)
    if let Some(schema) = &config.project_schema {
        parts.push(format!(
            "\n## PROJECT SCHEMA CONTEXT\n\
            Current project state and collected information:\n{}",
            schema
        ));
    }

    parts.join("\n\n")
}
```

### Production-Tested Patterns Summary

| Pattern | Python (`gemini.py`) | Rust Equivalent |
|---------|---------------------|-----------------|
| Client singleton | `get_genai_client()` with global `_genai_client` | `OnceLock<Client>` or `lazy_static` |
| Audio/Control separation | `asyncio.Queue` × 2 | `tokio::sync::mpsc::unbounded_channel` × 2 |
| Audio sender | `send_audio_task()` | Tokio task consuming `audio_rx` |
| Control sender | `send_control_task()` | Tokio task consuming `control_rx` |
| Receiver | `receive_from_gemini()` | Tokio task with `ws_read.next()` |
| Browser reader | `read_from_browser()` | Reads from VAD ringbuf in Vox |
| WebSocket connect | `client.aio.live.connect()` | `tokio-tungstenite::connect_async()` |
| Base64 audio | `aio.send_realtime_input(audio=Blob(...))` | `base64_engine.encode()` in JSON |
| Send text | `aio.send_realtime_input(text=...)` | `realtimeInput.text` JSON message |
| Activity start | `aio.send_realtime_input(activity_start=...)` | `realtimeInput.activityStart` JSON |
| Activity end | `aio.send_realtime_input(activity_end=...)` | `realtimeInput.activityEnd` JSON |
| Tool response | `aio.send_tool_response()` | `toolCall.response` JSON message |
| Session config | Dict literal with all fields | `serde_json::json!({...})` macro |
| GoAway handling | Check `response.go_away` | Check `"goAway"` key in JSON |
| Resumption handle | `response.session_resumption_update.new_handle` | Same JSON field |
| Reconnection | Outer `while` + `asyncio.wait(FIRST_COMPLETED)` | `tokio::select!` with retry loop |
| Exponential backoff | `sleep(2 * attempt)` | `tokio::time::sleep()` |
| Turn lifecycle | Manual state machine in `session_context` | `Arc<Mutex<SessionState>>` |
| Cleanup | Cancel tasks, final sync, close services | Drop handler + cancel tokens |

### Limitations
- **Preview on Google AI** (GA only on Vertex AI) — breaking changes possible
- **~10 min WebSocket timeout** — session resumption tokens valid 2h
- Post-barge-in freeze bug (workaround: nudge timer or activity_start signal)
- Mid-sentence truncation (server-side `turnComplete` fires prematurely)
- Compounding token billing (past audio re-billed every turn — enable `contextWindowCompression`)

---

## Provider: OpenAI Realtime API

### Endpoints

```text
# WebSocket (primary)
wss://api.openai.com/v1/realtime?model=gpt-realtime-2

# WebRTC (browser/mobile)
POST https://api.openai.com/v1/realtime/client_secrets  → ephemeral token
```

### Authentication
- Standard OpenAI API key (`sk-...`) in WebSocket upgrade header:
  `Authorization: Bearer sk-xxxxxxxx`
- Ephemeral tokens for client-side via `POST /v1/realtime/client_secrets`
- **No free tier for Realtime** — a valid payment method is required
- Startup credits program: up to $100K for eligible startups

### Pricing
| Modality | Input | Output |
|----------|-------|--------|
| Audio | $32.00 / 1M tokens | $64.00 / 1M tokens |
| Text | $4.00 / 1M tokens | $24.00 / 1M tokens |

**Estimated total: ~$0.096/min** ($5.76/hr) for a balanced conversation. Costs grow with conversation length because every turn re-sends the full context.

### Audio Specs
- **Input**: **24 kHz PCM16** — Vox must resample from 16 kHz using `rubato`
- **Output**: 24 kHz PCM16 — match playback device or resample
- **⚠️ Critical**: 24 kHz only. The API rejects other sample rates.
- **Alternate formats**: G.711 μ-law, G.711 A-law
- **Chunk size**: Accepts chunks up to 15MB (but larger = higher latency; 100ms recommended)

### Protocol — WebSocket Event Flow

```
Client → Server:
  session.update           → configure model, voice, VAD, tools
  input_audio_buffer.append → base64 PCM16 chunk
  [VAD auto-detects]       → server triggers response
  [OR] input_audio_buffer.commit + response.create  (manual mode)
  response.cancel          → barge-in / interrupt

Server → Client:
  session.created / updated
  input_audio_buffer.speech_started / speech_stopped
  response.audio.delta     → base64 PCM16 audio chunk (STREAM THESE)
  response.audio.done
  response.audio_transcript.delta / done
  error
```

### Barge-In
- Built-in: set `interrupt_response: true` in VAD config
- On `speech_started`: client stops playback immediately
- Send `conversation.item.truncate` with the played duration for fine-grained control
- Three VAD modes: `server_vad` (default), `semantic_vad` (content-based), `null` (client-driven)

### Latency
| Metric | Value |
|--------|-------|
| End-to-end (P50) | **~232ms** (fastest of all providers) |
| WebSocket overhead | Minimal (JSON base64) |

### Rust Integration
- **Recommended approach**: Raw `tokio-tungstenite` + custom event type definitions
- **Reference implementation**: `github.com/raja-patnaik/openai-realtime-rust` — minimal CLI app with CPAL + tokio-tungstenite
- **Alternative crate**: `openai_dive` v1.4.3 (has `realtime` module with typed events)
- **No official Rust SDK** — OpenAI supports Python, Node, Go, Java only

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};
use http::Request;

let request = Request::builder()
    .uri("wss://api.openai.com/v1/realtime?model=gpt-realtime-2")
    .header("Authorization", "Bearer sk-...")
    .header("OpenAI-Beta", "realtime=v1")
    .body(())?;

let (ws, _) = connect_async(request).await?;
let (mut write, mut read) = ws.split();

// Send input audio buffer
let msg = serde_json::json!({
    "type": "input_audio_buffer.append",
    "audio": base64_engine.encode(&pcm_chunk),
});
write.send(Message::Text(msg.to_string())).await?;

// Receive audio deltas
while let Some(Ok(msg)) = read.next().await {
    if let Message::Text(text) = msg {
        let event: serde_json::Value = serde_json::from_str(&text)?;
        if event["type"] == "response.audio.delta" {
            let audio_b64 = event["delta"].as_str().unwrap();
            let pcm = base64_engine.decode(audio_b64)?;
            speaker_tx.send(pcm);
        }
    }
}
```

### Limitations
- **60-minute session cap** — must split long conversations
- **No free tier** — paid account required
- **Voice locked** — cannot change voice after first audio output in a session
- **24 kHz input** requires CPU overhead for upsampling from 16 kHz
- **Monolingual per-session** — language cannot be changed mid-conversation
- Latest beta (`gpt-4o-realtime-preview`) **shut down May 12, 2026** — must use `gpt-realtime-2`

---

## Provider: Deepgram Voice Agent API

### Endpoints

```text
# North America
wss://agent.deepgram.com/v1/agent/converse

# Europe (GA since Dec 2025)
wss://api.eu.deepgram.com/v1/agent/converse
```

### Authentication
- API key from [console.deepgram.com](https://console.deepgram.com)
- Header: `Authorization: Token YOUR_DEEPGRAM_API_KEY`
- JWT temporary tokens for client-side apps

### Free Tier & Pricing
| Plan | Cost | Details |
|------|------|---------|
| Pay-As-You-Go | **$200 free credits** on signup (no credit card, never expire) | ~40+ hours free |
| Standard | **$0.075/min** ($4.50/hr) flat rate | Full Deepgram stack |
| Custom (BYO LLM) | **$0.050/min** ($3.00/hr) | Bring your own LLM |
| Concurrency | 45 concurrent (NA/EU) | Pay-As-You-Go |

**Key advantage**: Flat per-minute pricing — no token-based spike risk. Costs are predictable regardless of conversation length.

### Audio Specs
- **Input**: PCM16, flexible sample rate (8/16/24/44.1/48 kHz), default 16 kHz
- **Output**: Configurable (linear16, mulaw, alaw, mp3, opus, flac, aac), default configurable sample rate
- **Multiple codecs supported**: linear16, flac, opus, mulaw, alaw, speex, amr-nb/wb, g.729
- **Chunk size**: Binary WebSocket frames (raw PCM) — **no base64 overhead on send path**

### Protocol — Binary + JSON

Deepgram uses **binary WebSocket frames for audio** (not base64 JSON), which
avoids the base64 encoding overhead:

```
Client → Server:
  1. JSON settings           (configure STT model, LLM, TTS voice, audio params)
  2. Binary PCM audio        (raw PCM16 microphone stream)
  3. JSON control messages   (UpdateSpeak, UpdatePrompt, KeepAlive, etc.)

Server → Client:
  1. JSON: Welcome, SettingsApplied
  2. JSON: ConversationText, UserStartedSpeaking, AgentThinking
  3. JSON: AgentStartedSpeaking (includes latency metrics:
        total_latency, tts_latency, ttt_latency)
  4. Binary PCM audio        (TTS output — stream to speaker)
  5. JSON: AgentAudioDone, FunctionCallRequest
```

### BYO LLM Integration

This is critical for Vox — the Voice Agent API can route through **any**
OpenAI-compatible LLM endpoint, which means it can use Vox's existing
`LlmProvider` infrastructure:

```json
{
  "types": {
    "think": {
      "provider": {
        "type": "open_ai",
        "endpoint": {
          "url": "http://localhost:8080/v1/chat/completions",
          "auth_header": "Bearer sk-..."
        }
      }
    }
  }
}
```

Supported managed LLMs: OpenAI GPT-5 series, Anthropic Claude, Google Gemini,
Groq, NVIDIA. BYO via any OpenAI-compatible endpoint.

### Barge-In
- **Native, model-driven** — server sends `UserStartedSpeaking` event
- Server immediately stops TTS and starts processing new input
- No client-side cancellation needed
- Built-in turn-taking prediction uses speech cadence, not just silence thresholds
- Rated highest in Voice Agent Quality Index (VAQI) for interruption handling

### Latency
| Metric | Value |
|--------|-------|
| STT (Nova-3 streaming) | ~150ms |
| TTS (Aura-2) first byte | **<200ms** |
| End-to-end | **~1 second** typical |
| VAQI composite score | **71.5** (vs OpenAI 67.2, ElevenLabs 55.3) |

Note: ~1s end-to-end exceeds Vox's sub-500ms target for the modular pipeline,
but the binary WebSocket transport (no base64) and flat pricing are advantages.

### Rust Integration
- **No official Rust SDK for Voice Agent** (the `deepgram` crate v0.10.0 only covers STT/TTS/Management, not Voice Agent)
- Must use raw `tokio-tungstenite`
- **Reference implementation**: `github.com/deepgram-devs/deepgram-demos-rust` — `voice-agent/src/main.rs` (675 lines)
- Uses CPAL for capture, rodio for playback, tokio-tungstenite for WebSocket

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};
use http::Request;

let request = Request::builder()
    .uri("wss://agent.deepgram.com/v1/agent/converse")
    .header("Authorization", "Token YOUR_DEEPGRAM_API_KEY")
    .body(())?;

let (ws, _) = connect_async(request).await?;
let (mut write, mut read) = ws.split();

// 1. Send settings JSON
let settings = serde_json::json!({/* agent config */});
write.send(Message::Text(settings.to_string())).await?;

// 2. Stream binary audio from mic
write.send(Message::Binary(pcm_bytes)).await?;

// 3. Receive audio + events
while let Some(Ok(msg)) = read.next().await {
    match msg {
        Message::Binary(audio) => { /* play audio */ }
        Message::Text(json) => { /* handle events */ }
        _ => {}
    }
}
```

### Languages
- STT (Nova-3): **45+ languages** including Hindi (in multilingual mode)
- TTS (Aura-2): **7 languages** (English, Spanish, Dutch, French, German, Italian, Japanese)
- For Hindi TTS output, use BYO TTS provider (e.g. ElevenLabs which supports 74 languages)

### Limitations
- **No voice cloning** on Deepgram side — use BYO TTS (ElevenLabs) for custom voices
- Pipeline architecture adds ~1s end-to-end latency vs native S2S
- Smallest language coverage for TTS output (7 languages)
- Per-minute connection billing — costs accrue even during silence

---

## Provider: ElevenLabs Conversational AI API

### Endpoints

```text
# US Primary
wss://api.elevenlabs.io/v1/convai/conversation?agent_id={agent_id}

# Regional endpoints
api.us.elevenlabs.io
api.eu.residency.elevenlabs.io   (EU data residency)
api.in.residency.elevenlabs.io   (India)
api.sg.residency.elevenlabs.io   (Singapore)
```

### Authentication
- API key in header: `xi-api-key: <key>`
- **Best practice**: Server generates signed URL via `GET /v1/convai/conversation/get-signed-url?agent_id=...` (15-min validity, can outlast once connected)
- Agent can be public (connect with agent_id only) or private (signed URL required)

### Free Tier & Pricing
| Plan | Monthly Cost | Included Minutes | Overage |
|------|-------------|-----------------|---------|
| Free | $0 | **15 min** | N/A (capped) |
| Starter | $6 | 75 min | $0.08/min |
| PAYG | Usage-based | N/A | $0.08/min |

LLM costs are **passed through separately** from agent call time. The total
per-minute cost depends on which LLM the agent is configured to use.

### Audio Specs
- **Input**: 16 kHz PCM16 (base64 inside JSON) — **matches Vox natively**
- **Output**: **44.1 kHz PCM16** (default) — requires downsampling to device rate (or configure to 24/16 kHz)
- **Alternative output formats**: `pcm_16000`, `pcm_22050`, `pcm_24000`, `pcm_44100`, `mp3_44100`, `ulaw_8000`
- **WebRTC path**: Hardcoded 48 kHz both directions

### Protocol — WebSocket Events

```json
Client → Server:
{
  "type": "conversation_initiation_client_data",
  "conversation_config_override": { /* optional overrides */ }
}

// Then stream audio:
{
  "type": "user_audio_chunk",
  "audio": "<base64 PCM16>"
}

Server → Client:
// Incoming audio:
{
  "type": "audio",
  "audio": "<base64 PCM44k>",
  "alignment": { ... }  // character-level timing
}

// Interruption:
{ "type": "interruption" }

// Transcription:
{ "type": "user_transcript", "text": "...", "is_final": true }

// Agent state:
{ "type": "agent_response", "message": "..." }
{ "type": "agent_response_complete" }
```

### Agent Configuration
The agent is **pre-configured server-side** (via ElevenLabs dashboard or REST API):
- System prompt (up to 2MB)
- LLM model (choose from OpenAI, Google, Anthropic, or custom OpenAI-compatible endpoint)
- Tools (webhooks, client-side, `end_call`, `transfer_to_number`)
- Knowledge base (RAG from documents)
- Voice (10,000+ options including cloned voices)
- Turn-taking: `turn_timeout` (1-30s), `turn_eagerness` (patient/normal/eager)
- Runtime overrides possible via `conversation_config_override`

### Barge-In
- Server sends `{"type": "interruption"}` when user speaks during agent output
- Client must stop audio playback and flush buffers immediately
- Agent automatically stops TTS and processes new input
- Turn-taking uses hybrid VAD + deep learning model (analyzes "um", "ah", prosody)
- **No exposed VAD sensitivity tuning** — platform-managed

### Latency
| Component | Latency |
|-----------|---------|
| TTS Flash v2.5 TTFA | **~135ms** (fastest in class for quality) |
| TTS Turbo v2.5 | ~250-300ms |
| Full stack (ASR→LLM→TTS) | ~400-600ms (VAQI benchmark: ~530ms) |
| Voice quality rating | ★★★★★ (best-in-class emotional range) |

### Rust Integration
- Raw `tokio-tungstenite` for WebSocket
- Optional: `elevenlabs-sdk` v0.1.0 for REST API calls (agent management, signed URL generation)
- 16kHz in / configurable out — use `rubato` to downsample 44.1kHz→device rate if needed
- **No known open-source Rust full ConvAI implementation** — Vox would be the first reference

### Voice Cloning
| Feature | Details |
|---------|---------|
| Instant Voice Cloning | 1-5 min audio, zero-shot, Free plan: 1 voice |
| Professional Voice Cloning | 30+ min audio, fine-tuned, Creator plan+ |
| Voice Library | 10,000+ pre-built voices (community-shared) |
| Voice Design | Text-prompt-based voice generation |

### Languages
- Eleven v3 Conversational TTS: **74 languages** including Hindi, Tamil, Telugu, Bengali, Marathi, Gujarati, Urdu, Punjabi
- Agent ASR (Scribe v2): 90+ languages
- **Hinglish mode**: `hinglish_mode: true` in agent config
- Language is fixed per session (cannot switch mid-conversation)

### Limitations
- **10-minute session** default (configurable, but no documented hard upper limit)
- Agent is pre-configured server-side — runtime overrides are limited
- No exposed VAD tuning — platform-managed
- WebSocket audio output at 44.1 kHz default — CPU overhead for downsampling
- Rust integration is raw WebSocket (no mature SDK for ConvAI specifically)
- Latency exceeds Gemini/OpenAI native S2S by ~200ms

---

## Provider Quick-Reference: Other Options

### Sarvam AI (Not S2S — STT/TTS only)

Sarvam provides **individual STT and TTS WebSocket APIs** (not a combined S2S
engine), so it is **not a RealtimeVoiceProvider**. However, it is an important
provider for **Hindi/Hinglish STT and TTS** when Vox is operating in modular
mode with cloud components:

| Feature | Detail |
|---------|--------|
| STT endpoint | `wss://api.sarvam.ai/speech-to-text/ws` |
| TTS endpoint | `wss://api.sarvam.ai/text-to-speech/ws` |
| Rust crate | `sarvam-rs` v0.2.0 (MIT, fully typed WebSocket streaming) |
| Free tier | ₹1,000 credits (~$12) |
| Pricing | STT: ₹30/hr (~$0.35), TTS: ₹30/10K chars (~$0.36) |
| Hindi support | **Best-in-class for Indian languages** — purpose-built, native code-mixed Hinglish |
| Languages | 22+ Indian languages + English |

### Open-Source Rust S2S Projects (For Reference)

If self-hosting a S2S engine becomes relevant in the future:

| Project | Language | Description |
|---------|----------|-------------|
| **Vona** | Rust | S2S runtime with protocol crates for OpenAI Realtime, Gemini Live, Deepgram, ElevenLabs. MIT. |
| **nix-vox** | Rust | Local-first WebSocket endpoints for STT/TTS/converse. Apache 2.0. |
| **Pipecat** | Python | Most mature OSS voice framework (68+ integrations), but Python sidecar required. |
| **Dograh** | Python/TS | Production-ready voice agent platform with visual workflow builder. |

---

## API Key Acquisition & Quota Reference

| Provider | Get Key At | Free Tier | Est. Cost | Concurrent Limit | Session Limit |
|----------|-----------|-----------|-----------|-----------------|---------------|
| **Gemini** | [aistudio.google.com/app/apikey](https://aistudio.google.com/app/apikey) | 10-15 RPM free | ~$0.036/min | 5,000/project | ~10 min (resumable) |
| **OpenAI** | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) | None (paid only) | ~$0.096/min | 100 (Tier 5) | 60 min |
| **Deepgram** | [console.deepgram.com](https://console.deepgram.com) | $200 free credits | $0.075/min flat | 45 concurrent | Unlimited (practical) |
| **ElevenLabs** | [elevenlabs.io/app/settings/api-keys](https://elevenlabs.io/app/settings/api-keys) | 15 min/month free | $0.08/min + LLM | 4 (Free), 30+ (Scale) | 10 min default |

---

## Implementation Roadmap

### Phase A — Realtime Engine Infrastructure (v0.8.5)

**Files**: `services/realtime/` (new module)

- [ ] Define `RealtimeVoiceProvider` trait and `RealtimeSession` trait in `services/realtime/mod.rs`
- [ ] Define `RealtimeAudioConfig` struct with per-provider sample rates
- [ ] Define `RealtimeProviderKind` enum
- [ ] Create thread bridge infrastructure:
  - `AudioBridge`: reads from VAD ring buffer → `tokio::sync::mpsc` → WebSocket sender
  - `PlaybackBridge`: WebSocket receiver → ring buffer → CPAL playback
- [ ] Add `rubato`-based `AudioResampler` utility for dynamic sample rate conversion
- [ ] Add pipeline routing logic in `services/pipeline.rs` (mode check: modular vs realtime)
- [ ] Add `PipelineMode` setting (`Modular | Realtime(RealtimeProviderKind)`)
- [ ] Add `tokio-tungstenite`, `rubato`, `base64` to `Cargo.toml`
- [ ] Basic end-to-end WebSocket loop test (no provider, echo server)

### Phase B — Gemini Live Provider (v0.8.6)

**Priority**: First — native 16 kHz input, free tier, production-tested patterns from `gemini.py`.

#### Architecture Setup
- [ ] Add `tokio-tungstenite = { version = "0.29", features = ["rustls-tls-webpki-roots"] }`, `base64 = "0.22"` to `Cargo.toml`
- [ ] Implement `GeminiLiveProvider` in `services/realtime/providers/gemini_live.rs`
- [ ] Define `GeminiConfig` struct with all session parameters:
  - `model: String`, `api_key: String`, `use_vertexai: bool`
  - `voice_name: String` (default `"Charon"`), `language_code: String` (BCP-47)
  - `system_prompt: String`, `temperature: f32` (default 0.2)
  - `enable_input_transcription: bool` (default true)
  - `enable_output_transcription: bool` (default true)
  - `resume_handle: Option<String>`
  - `use_web_search: bool`, `tools: Vec<ToolDefinition>`
  - Vad sensitivity: `start_sensitivity`, `end_sensitivity`, `prefix_padding_ms`, `silence_duration_ms`

#### Task 1: Session Config & Setup (Production Literal)
- [ ] Build the exact `setup` message JSON per production `gemini.py`:
  - `tools`: web_search tool + function declarations
  - `responseModalities: ["AUDIO"]`
  - `speechConfig.voiceConfig.prebuiltVoiceConfig.voiceName` + `languageCode` (BCP-47)
  - `systemInstruction` with multi-part dynamic prompt
  - `temperature: 0.2`
  - `inputAudioTranscription: {}` / `outputAudioTranscription: {}`
  - `thinkingConfig.thinkingBudget: 0`
  - `realtimeInputConfig.automaticActivityDetection` with VAD sensitivity
  - `sessionResumption.handle` (None/null on first connect, resumption token on reconnect)
- [ ] Send setup message on WebSocket connect, await `setupComplete` response
- [ ] Handle `setupComplete` timeout (5s) with error

#### Task 2: Two-Queue Architecture (Critical)
- [ ] Create **two independent `tokio::sync::mpsc::unbounded_channel` instances**:
  - `audio_tx` / `audio_rx`: raw PCM blob transport
  - `control_tx` / `control_rx`: JSON control events (text, interrupt, activity signals)
- [ ] Rationale: prevents head-of-line blocking — a stalled `realtimeInput.text` send must never delay `realtimeInput.audio`

#### Task 3: Audio Sender Task
- [ ] Spawn dedicated Tokio task: reads PCM chunks from `audio_rx`
- [ ] Base64-encode each chunk, wrap in `{"realtimeInput": {"audio": {"data": "...", "mimeType": "audio/pcm;rate=16000"}}}`
- [ ] Send as JSON text frame via WebSocket (`ws_write.send(Message::Text(...))`)
- [ ] Log every 100th packet for diagnostics (matches production `packet_count % 100`)

#### Task 4: Control Sender Task
- [ ] Spawn dedicated Tokio task: reads `ControlEvent` enums from `control_rx`
- [ ] Map each event to the correct Gemini wire format:
  - `ControlEvent::Text(t)` → `{"realtimeInput": {"text": t}}`
  - `ControlEvent::Interrupt` → set `interrupt_active = true`, send `{"realtimeInput": {"activityStart": {}}}`
  - `ControlEvent::ActivityStart` → `{"realtimeInput": {"activityStart": {}}}`
  - `ControlEvent::ActivityEnd` → `{"realtimeInput": {"activityEnd": {}}}`
- [ ] In PTT mode: send `ActivityEnd` after 50ms delay following interrupt (matches production)

#### Task 5: Receive Loop (Model → Playback + Frontend)
- [ ] Spawn dedicated Tokio task: read WebSocket messages from `ws_read.next()`
- [ ] Parse every server event into typed enums. Handle:
  - **`serverContent.modelTurn.parts`**: iterate parts, handle:
    - `part.text` → accumulate in `current_model_response`, forward to frontend as text event
    - `part.inlineData.data` → base64-decode, push to playback ringbuf
    - `part.thought` → skip silently
  - **`serverContent.turnComplete`**: trigger sync, increment `completed_turns`, reset turn state
  - **`serverContent.inputTranscription.text`**: forward to frontend as STT transcript
  - **`serverContent.outputTranscription.text`**: accumulate in `current_model_response`, forward to frontend
  - **`serverContent.interrupted`**: reset `interrupt_active`, send interrupted event to frontend
  - **`toolCall.functionCalls`**: execute tools, send responses via toolCall response messages
  - **`sessionResumptionUpdate.newHandle`**: store and forward to frontend
  - **`goAway`**: raise `ConnectionResetError` to trigger reconnection
  - **`usageMetadata`**: accumulate `promptTokenCount` + `responseTokenCount` per turn

#### Task 6: Session State Machine
- [ ] Implement `Arc<Mutex<SessionState>>` shared across all tasks:
  ```rust
  struct SessionState {
      current_user_query: String,       // accumulated from inputTranscription
      current_model_response: String,   // accumulated from modelTurn.text + outputTranscription
      turn_synced: bool,                // has current turn been persisted?
      interrupt_active: bool,           // suppress model/content during interrupt
      completed_turns: u32,             // turn counter for sequencing
      turn_prompt_tokens: u32,          // per-turn usage tracking
      turn_response_tokens: u32,
      resume_handle: Option<String>,    // session resumption token
  }
  ```
- [ ] On `turnComplete`: reset user_query, model_response, tokens, set `turn_synced = false`
- [ ] On `interrupt_active == true`: silently drop all `modelTurn` content and transcriptions
- [ ] On tool call receipt: reset `interrupt_active = false` (tool calls are valid model output)

#### Task 7: Barge-In (Two-Phase, Production-Tested)
- [ ] **Phase 1 — Local (immediate, <50ms)**:
  - VAD detects speech during playback → mute local audio output
  - Clear playback ring buffer
  - Send `ControlEvent::Interrupt` → control task sends `activityStart` to Gemini
- [ ] **Phase 2 — Server Confirmation (200-500ms later)**:
  - On receiving `serverContent.interrupted` → reset state, send `{"interrupted": true}` to frontend
- [ ] **Nudge timer**: If no audio from Gemini after 4s post-interrupt, send text nudge via control queue

#### Task 8: Reconnection Loop
- [ ] Wrap entire connection lifecycle in `while browser_alive` loop (matches production)
- [ ] On clean disconnect (browser closed): break loop, return Ok
- [ ] On Gemini error (GoAway, receive error, WS disconnect):
  - Increment `reconnect_attempts`, cap at 3
  - Clear `resume_handle` on failure
  - Exponential backoff: `sleep(Duration::from_secs(2 * reconnect_attempts))`
  - On max attempts: send fatal error to frontend, break loop
- [ ] When receive loop exits: cancel all sibling tasks (audio sender, control sender)

#### Task 9: System Prompt Builder
- [ ] Implement dynamic prompt builder matching production:
  - Base system prompt template
  - Language instruction (BCP-47 mapped)
  - Grounding instructions (web search enabled/disabled/vector)
  - Prior session context: last 3 chat turns seeded into system instruction
  - Project schema context (if available)
- [ ] All parts joined with `\n\n` separators

#### Task 10: Tool Calling
- [ ] Define `FunctionDeclaration` structs matching Gemini's schema
- [ ] On `toolCall.functionCalls`:
  - Match on `call.name`
  - Execute tool (search, API call, etc.)
  - Send result as `{"toolCall": {"functionResponses": [{"name": "...", "id": "...", "response": {...}}]}}`
- [ ] Send status updates to frontend before/during tool execution (e.g. `{"status": "searching", "query": "..."}`)

#### Task 11: Session Resumption
- [ ] On receiving `sessionResumptionUpdate` with `newHandle`:
  - Store in `SessionState.resume_handle`
  - Forward to frontend
- [ ] On reconnection: use stored handle in `sessionResumption.handle` config field
- [ ] On resumption failure: clear handle, start clean session

#### Task 12: Lifecycle & Cleanup
- [ ] Implement `Drop` for `GeminiLiveSession`:
  - Cancel all Tokio tasks
  - Final sync: persist any unsaved turn
  - Log session summary
- [ ] Track per-session telemetry:
  - `t_first_audio_in` → first audio packet received from VAD
  - `t_first_packet_out` → first response received from Gemini
  - `t_first_audio_out` → first audio chunk from Gemini
  - `t_first_text_out` → first text token from Gemini
  - Total audio packets forwarded
  - Total tokens consumed

#### Settings & UI
- [ ] Settings: API key input, provider selection, model selection (2.5 vs 3.1 Flash)
- [ ] Voice selection (available Gemini voices: Aoede, Charon, etc.)
- [ ] Language selector (BCP-47 codes)
- [ ] Web search toggle
- [ ] Vertex AI toggle + project ID + region fields
- [ ] Free-tier testing and validation

### Phase C — OpenAI Realtime Provider (v0.8.7)

**Priority**: Second — industry standard, but 24 kHz input requires resampling.

- [ ] Implement `OpenAIRealtimeProvider` in `services/realtime/providers/openai_realtime.rs`
- [ ] Define all 16+ server event types as serde structs (session.created, response.audio.delta, etc.)
- [ ] Wire up: VAD → resample (16kHz→24kHz via `rubato`) → base64 → WebSocket
- [ ] Wire up: WebSocket → resample (24kHz→device) → ringbuf → playback
- [ ] Implement barge-in: on `speech_started` → stop playback → send truncate
- [ ] Handle 60-min session cap: context transfer and reconnection
- [ ] Settings: API key, model selection (gpt-realtime-2, gpt-realtime-mini), voice selection

### Phase D — Deepgram Voice Agent Provider (v0.8.8)

**Priority**: Third — flat-rate pricing, BYO LLM aligns with existing architecture.

- [ ] Implement `DeepgramVoiceAgentProvider` in `services/realtime/providers/deepgram_voice_agent.rs`
- [ ] Handle binary WebSocket frames for audio (no base64 overhead on send)
- [ ] Implement settings JSON config (STT model, LLM choice, TTS voice)
- [ ] Wire BYO LLM: `think.provider.endpoint.url` → Vox's local or remote LLM
- [ ] Handle `UserStartedSpeaking` for barge-in
- [ ] Settings: API key, LLM provider selection, TTS voice

### Phase E — ElevenLabs Conversational AI Provider (v0.8.9)

**Priority**: Fourth — premium voice quality, but highest output sample rate.

- [ ] Implement `ElevenLabsConvaiProvider` in `services/realtime/providers/elevenlabs_convai.rs`
- [ ] Wire up: 16kHz input (native match) → base64 → WebSocket
- [ ] Wire up: WebSocket → resample (44.1kHz→device via `rubato`) → ringbuf → playback
- [ ] Implement signed URL generation endpoint (server-side token proxy)
- [ ] Handle `interruption` event for barge-in
- [ ] Settings: API key, agent_id, voice selection (via agent config)

### Phase F — Provider Selection & Stabilization (v0.9.0)

- [ ] Provider selection UI in Settings page (dropdown of available realtime providers)
- [ ] Health check badges (green/red indicator per provider)
- [ ] Fallback logic: if realtime provider is unhealthy, fall back to modular pipeline
- [ ] Rate limit monitoring and cost tracking (token/credit usage display)
- [ ] Graceful degradation: network loss → pause → resume with context restoration
- [ ] End-to-end latency telemetry per provider
- [ ] Documentation update: AGENTS.md, backend.md

---

## Success Criteria (v0.9.0)

### Functional

- [ ] Existing embedded LLM + modular pipeline remain fully functional
- [ ] Gemini Live provider works end-to-end (audio in → audio out)
- [ ] OpenAI Realtime provider works end-to-end (audio in → audio out)
- [ ] Deepgram Voice Agent provider works end-to-end
- [ ] ElevenLabs Conversational AI provider works end-to-end
- [ ] Barge-in works on all providers with <200ms local mute
- [ ] Dynamic provider switching at runtime
- [ ] Graceful fallback to modular pipeline on network loss
- [ ] Health check indicators for all configured providers

### Performance

- [ ] Local mute on barge-in: **<50ms**
- [ ] WebSocket audio send latency: **<10ms** (from ringbuf read to wire)
- [ ] Playback continuity: **no audible glitches** on reconnect
- [ ] CPU overhead from resampling: **<5%** on a modern CPU

### Architectural

- [ ] All realtime providers behind a single trait
- [ ] Pipeline routing mode check does not affect modular path latency
- [ ] Thread bridge does not block VAD or capture threads
- [ ] No provider-specific logic leaks into pipeline.rs or playback.rs

---

## Non-Goals (v0.9.0)

- **Hybrid S2S composition** (using individual cloud STT/LLM/TTS providers stitched
  together instead of a single S2S WebSocket) — this means Sarvam, Anthropic,
  OpenRouter as individual STT/LLM components are explicitly excluded from the
  Realtime Engine. They may be added as modular pipeline cloud providers in a
  future phase.
- **WebRTC support** — all providers use WebSocket initially. WebRTC may be
  added later if needed.
- **Multi-provider session** — cannot route audio through Gemini and ElevenLabs
  simultaneously. Single provider per session.
- **Video/multimodal** — audio-only sessions.
- **Telephony (PSTN/SIP)** — not needed for a desktop voice assistant.
- **Server-side agent management** — ElevenLabs agents are configured through
  their dashboard, not through Vox's UI.

---

## Provider Selection Guide

```text
User wants cloud realtime S2S:
    │
    ├── Primary recommendation: Gemini Live
    │   (free tier, native 16kHz input, cheapest at scale, best Rust crate)
    │
    ├── Lowest latency / industry standard: OpenAI Realtime
    │   (~232ms P50, paid only, 24kHz resampling required)
    │
    ├── Flat-rate pricing / BYO LLM: Deepgram Voice Agent
    │   ($0.075/min flat, $200 free credits, binary WS frames)
    │
    └── Best voice quality / custom voice: ElevenLabs ConvAI
        (premium TTS, voice cloning, 74 languages, 15 min/month free)
```

