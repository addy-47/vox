# Deepgram Voice Agent API Integration

## Endpoints

```
# North America
wss://agent.deepgram.com/v1/agent/converse

# Europe (GA since Dec 2025)
wss://api.eu.deepgram.com/v1/agent/converse
```

## Authentication

- API key from [console.deepgram.com](https://console.deepgram.com)
- Header: `Authorization: Token YOUR_DEEPGRAM_API_KEY`

## Free Tier & Pricing

| Plan | Cost | Details |
|------|------|---------|
| Pay-As-You-Go | **$200 free credits** on signup | ~40+ hours free |
| Standard | **$0.075/min** ($4.50/hr) flat rate | Full Deepgram stack |
| Custom (BYO LLM) | **$0.050/min** ($3.00/hr) | Bring your own LLM |

**Key advantage**: Flat per-minute pricing — no token-based spike risk.

## Audio Specs

- **Input**: PCM16, flexible sample rate (8/16/24/44.1/48 kHz), default 16 kHz
- **Output**: Configurable (linear16, mulaw, alaw, mp3, opus, flac, aac)
- **Multiple codecs supported**: linear16, flac, opus, mulaw, alaw, speex, amr-nb/wb, g.729
- **Chunk size**: Binary WebSocket frames (raw PCM) — **no base64 overhead on send path**

## Protocol — Binary + JSON

Deepgram uses **binary WebSocket frames for audio** (not base64 JSON), which avoids base64 encoding overhead:

```
Client → Server:
  1. JSON settings            (STT model, LLM, TTS voice, audio params)
  2. Binary PCM audio         (raw PCM16 microphone stream)
  3. JSON control messages    (UpdateSpeak, UpdatePrompt, KeepAlive, etc.)

Server → Client:
  1. JSON: Welcome, SettingsApplied
  2. JSON: ConversationText, UserStartedSpeaking, AgentThinking
  3. JSON: AgentStartedSpeaking (includes latency metrics)
  4. Binary PCM audio         (TTS output — stream to speaker)
  5. JSON: AgentAudioDone, FunctionCallRequest
```

## BYO LLM Integration

Voice Agent can route through any OpenAI-compatible LLM endpoint — it can use Vox's existing `LlmProvider` infrastructure:

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

## Barge-In

- Native, model-driven — server sends `UserStartedSpeaking` event
- Server immediately stops TTS and starts processing new input
- No client-side cancellation needed for server
- Built-in turn-taking prediction using speech cadence

## Latency

| Metric | Value |
|--------|-------|
| End-to-end | **~1 second** typical |
| VAQI composite score | **71.5** (vs OpenAI 67.2, ElevenLabs 55.3) |

## Rust Integration

- No official Rust SDK for Voice Agent
- Must use raw `tokio-tungstenite`
- Reference: `github.com/deepgram-devs/deepgram-demos-rust`

## Limitations

- No voice cloning — use BYO TTS for custom voices
- Pipeline architecture adds ~1s latency vs native S2S
- Smallest language coverage for TTS output (7 languages)
- Per-minute billing — costs accrue during silence
