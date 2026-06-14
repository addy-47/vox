# Realtime Engine Architecture

> The foundational infrastructure for S2S (speech-to-speech) cloud providers.
> Defines the trait, threading model, pipeline routing, and common utilities.

## Design Principle

The backend should care about:
- **Protocol** (WebSocket bidirectional audio streaming)

not about:
- How the provider internally does STT → LLM → TTS

All providers in this category accept PCM audio chunks over a WebSocket and return PCM audio chunks over the same connection. The implementation differences (sample rates, message formats, auth) are encapsulated behind a single trait.

## RealtimeVoiceProvider Trait

A new trait in `services/realtime/mod.rs`, separate from the existing `LlmProvider` (which remains text-in/text-out for the modular pipeline):

```rust
pub trait RealtimeVoiceProvider: Send + Sync {
    fn kind(&self) -> RealtimeProviderKind;
    fn audio_config(&self) -> RealtimeAudioConfig;
    fn connect(&self) -> anyhow::Result<Box<dyn RealtimeSession>>;
    fn health_check(&self) -> bool;
}

pub trait RealtimeSession: Send {
    fn send_audio(&self, pcm: &[i16]) -> anyhow::Result<()>;
    fn on_audio(&self, cb: Box<dyn Fn(&[i16]) + Send>);
    fn on_interruption(&self, cb: Box<dyn Fn() + Send>);
    fn cancel(&self) -> anyhow::Result<()>;
    fn disconnect(&self) -> anyhow::Result<()>;
}

pub struct RealtimeAudioConfig {
    pub input_sample_rate: u32,
    pub output_sample_rate: u32,
    pub requires_resampling: bool,
}
```

## Pipeline Routing

The pipeline (`services/pipeline.rs`) gains a mode check at the entry point:

```
VAD detects speech
    │
    ├── Mode: modular ──→ STT → LLM → TTS (existing path)
    │
    └── Mode: realtime ──→ RealtimeVoiceProvider (new path)
                               │
                               ↓
                          Audio → Playback
```

The mode is set via settings and hot-swappable at runtime. When switching to realtime mode, the existing modular pipeline is suspended and the VAD output is routed to the WebSocket sender task instead of STT.

## Thread Model

The realtime engine introduces a **hybrid sync/async architecture** that differs from the existing all-sync inference threads:

```
  SYNC OS THREADS (existing pattern)

  Audio Capture (cpal, Max priority)
      │
      ▼
  VAD (Earshot)
      │
      ▼  PCM chunks
  Ring Buffer (Consumer)
      │
      │  tokio::sync::mpsc::UnboundedSender (non-blocking)
      ▼

  TOKIO ASYNC TASK (new pattern)

  Audio Bridge Task
      │  recv() from mpsc
      │  resample (rubato) if needed
      │  base64-encode
      ▼
  WebSocket Sender
  ────────────────────→ Provider Cloud
  WebSocket Receiver  ←────────────────────
      │
      │  base64-decode
      │  resample if needed
      ▼
  Ring Buffer (Producer)

      │  lock-free atomic push
      ▼

  SYNC OS THREAD (existing pattern)

  Playback (cpal, ringbuf Consumer)
      ▼
  Audio Output
```

### Rust Crate Stack

```toml
# Core WebSocket + audio processing (required for all realtime providers)
tokio-tungstenite = { version = "0.29", features = ["rustls-tls-webpki-roots"] }
rubato = "3.0"
base64 = "0.22"

# Provider-specific (feature-gated, added per implementation)
gemini-live = { version = "0.1", optional = true }
openai_dive = { version = "1.4", optional = true }
```

## Technical Traps

### ⚠️ A. Sample Rate Mismatch

Your local pipeline captures at 16 kHz (for VAD/STT). Each provider expects different rates:

| Provider | Input Expected | Output Returns |
|----------|---------------|----------------|
| **Gemini Live** | **16 kHz** (match!) | 24 kHz (needs resampling) |
| **OpenAI Realtime** | **24 kHz** (must upsample) | 24 kHz |
| **Deepgram Voice Agent** | 16 kHz (flexible) | Configurable |
| **ElevenLabs ConvAI** | **16 kHz** (match!) | **44.1 kHz** (needs resampling) |

**The trap:** Sending wrong sample rates produces distorted audio. Vox must dynamically configure the `rubato` resampler per active provider via `RealtimeAudioConfig`.

### ⚠️ B. Client-Side VAD (Cost Trap)

Do not use Server VAD — streaming silence to the cloud burns bandwidth and API quotas. Keep using Vox's local `Earshot` VAD. Only send audio over the WebSocket when local VAD triggers `SpeechStart`. Send a flush/commit event when local VAD triggers `SpeechEnd`.

### ⚠️ C. Base64 JSON Overhead

Most APIs expect audio chunks as base64-encoded strings wrapped in JSON payloads. **Batch audio to 100ms chunks** before encoding to avoid heap fragmentation.

### ⚠️ D. WebSocket Binary vs Text Frames

- **Gemini Google AI**: JSON text frames only (base64 inside)
- **Gemini Vertex AI**: Binary frames for audio
- **OpenAI Realtime**: JSON text frames only
- **Deepgram Voice Agent**: Binary frames for audio + JSON for control
- **ElevenLabs ConvAI**: JSON text frames only

Vox's WebSocket layer must handle both frame types and route them correctly per provider.

### ⚠️ E. Barge-In Must Be Local-First

On local VAD speech detection during playback:
1. Immediately mute/stop local audio playback (0ms — local)
2. Clear the playback ring buffer
3. Send cancel/interruption signal to the cloud provider
4. When server confirmation arrives (200-500ms later), start new capture

### ⚠️ F. Session Timeouts & Reconnection

| Provider | Max Session | Reconnect Strategy |
|----------|-------------|-------------------|
| Gemini Live | ~10 min (GoAway 60s before) | Session resumption token valid 2h |
| OpenAI Realtime | **60 min** | ~30s reconnect window |
| Deepgram Voice Agent | Not documented | Reconnect from scratch |
| ElevenLabs ConvAI | 10 min default | Signed URL valid 15 min |

Implement automatic reconnection with context restoration and exponential backoff.
