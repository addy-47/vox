# ElevenLabs Conversational AI API Integration

## Endpoints

```
# US Primary
wss://api.elevenlabs.io/v1/convai/conversation?agent_id={agent_id}

# Regional endpoints
api.us.elevenlabs.io
api.eu.residency.elevenlabs.io   (EU data residency)
api.in.residency.elevenlabs.io   (India)
api.sg.residency.elevenlabs.io   (Singapore)
```

## Authentication

- API key in header: `xi-api-key: <key>`
- Best practice: Server generates signed URL via `GET /v1/convai/conversation/get-signed-url?agent_id=...`

## Free Tier & Pricing

| Plan | Monthly Cost | Included Minutes | Overage |
|------|-------------|-----------------|---------|
| Free | $0 | **15 min** | N/A (capped) |
| Starter | $6 | 75 min | $0.08/min |
| PAYG | Usage-based | N/A | $0.08/min |

## Audio Specs

- **Input**: 16 kHz PCM16 (base64 inside JSON) — **matches Vox natively**
- **Output**: **44.1 kHz PCM16** (default) — requires downsampling to device rate
- **Alternative output formats**: `pcm_16000`, `pcm_22050`, `pcm_24000`, `pcm_44100`, `mp3_44100`, `ulaw_8000`

## Protocol — WebSocket Events

```json
Client → Server:
{ "type": "user_audio_chunk", "audio": "<base64 PCM16>" }

Server → Client:
{ "type": "audio", "audio": "<base64 PCM44k>", "alignment": { ... } }
{ "type": "interruption" }
{ "type": "user_transcript", "text": "...", "is_final": true }
{ "type": "agent_response", "message": "..." }
{ "type": "agent_response_complete" }
```

## Agent Configuration

The agent is pre-configured server-side (via ElevenLabs dashboard or REST API):
- System prompt (up to 2MB)
- LLM model (OpenAI, Google, Anthropic, or custom OpenAI-compatible)
- Tools (webhooks, client-side, end_call, transfer_to_number)
- Knowledge base (RAG from documents)
- Voice (10,000+ options including cloned voices)
- Turn-taking: `turn_timeout` (1-30s), `turn_eagerness` (patient/normal/eager)

## Barge-In

- Server sends `{"type": "interruption"}` when user speaks during agent output
- Client must stop audio playback and flush buffers immediately
- Turn-taking uses hybrid VAD + deep learning model

## Latency

| Component | Latency |
|-----------|---------|
| TTS Flash v2.5 TTFA | **~135ms** |
| Full stack (ASR→LLM→TTS) | ~400-600ms |
| Voice quality rating | ★★★★★ |

## Voice Cloning

| Feature | Details |
|---------|---------|
| Instant Voice Cloning | 1-5 min audio, zero-shot, Free plan: 1 voice |
| Professional Voice Cloning | 30+ min audio, fine-tuned, Creator plan+ |
| Voice Library | 10,000+ |

## Languages

- Eleven v3 TTS: **74 languages** including Hindi, Tamil, Telugu, Bengali, Marathi
- Hinglish mode: `hinglish_mode: true` in agent config

## Rust Integration

- Raw `tokio-tungstenite` for WebSocket
- Optional: `elevenlabs-sdk` v0.1.0 for REST API calls
- 16kHz in / configurable out — use `rubato` to downsample 44.1kHz→device rate

## Limitations

- 10-minute session default (configurable)
- Agent is pre-configured server-side
- No exposed VAD tuning — platform-managed
- 44.1 kHz output requires CPU overhead for downsampling
- No mature Rust SDK for ConvAI specifically
