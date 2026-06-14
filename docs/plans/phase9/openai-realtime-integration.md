# OpenAI Realtime API Integration

## Endpoints

```
# WebSocket (primary)
wss://api.openai.com/v1/realtime?model=gpt-realtime-2
```

## Authentication

- Standard OpenAI API key (`sk-...`) in WebSocket upgrade header
- No free tier for Realtime — valid payment method required

## Pricing

| Modality | Input | Output |
|----------|-------|--------|
| Audio | $32.00 / 1M tokens | $64.00 / 1M tokens |
| Text | $4.00 / 1M tokens | $24.00 / 1M tokens |

**Estimated total: ~$0.096/min** ($5.76/hr) for a balanced conversation.

## Audio Specs

- **Input**: **24 kHz PCM16** — Vox must resample from 16 kHz using `rubato`
- **Output**: 24 kHz PCM16 — match playback device or resample
- **Chunk size**: 100ms recommended

## Protocol — WebSocket Event Flow

```
Client → Server:
  session.update              → configure model, voice, VAD, tools
  input_audio_buffer.append   → base64 PCM16 chunk
  [VAD auto-detects]          → server triggers response
  [OR] input_audio_buffer.commit + response.create (manual mode)
  response.cancel             → barge-in / interrupt

Server → Client:
  session.created / updated
  input_audio_buffer.speech_started / speech_stopped
  response.audio.delta        → base64 PCM16 audio chunk (STREAM THESE)
  response.audio.done
  response.audio_transcript.delta / done
  error
```

## Barge-In

- Built-in: set `interrupt_response: true` in VAD config
- On `speech_started`: client stops playback immediately
- Three VAD modes: `server_vad` (default), `semantic_vad`, `null` (client-driven)

## Latency

| Metric | Value |
|--------|-------|
| End-to-end (P50) | **~232ms** (fastest of all providers) |

## Rust Integration

- Raw `tokio-tungstenite` + custom event type definitions
- Reference: `github.com/raja-patnaik/openai-realtime-rust`
- Alternative crate: `openai_dive` v1.4.3 (has `realtime` module)
- No official Rust SDK

## Limitations

- 60-minute session cap — must split long conversations
- No free tier — paid account required
- Voice locked — cannot change voice after first audio output
- 24 kHz input requires CPU overhead for upsampling
- Monolingual per-session
- Latest beta (`gpt-4o-realtime-preview`) shut down May 12, 2026 — must use `gpt-realtime-2`
