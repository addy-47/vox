# Provider Quick Reference

## Sarvam AI (Not S2S — STT/TTS only)

Sarvam provides individual STT and TTS WebSocket APIs (not a combined S2S engine). Important for Hindi/Hinglish STT and TTS when Vox is operating in modular mode with cloud components.

| Feature | Detail |
|---------|--------|
| STT endpoint | `wss://api.sarvam.ai/speech-to-text/ws` |
| TTS endpoint | `wss://api.sarvam.ai/text-to-speech/ws` |
| Rust crate | `sarvam-rs` v0.2.0 (MIT, fully typed WebSocket streaming) |
| Free tier | ₹1,000 credits (~$12) |
| Pricing | STT: ₹30/hr (~$0.35), TTS: ₹30/10K chars (~$0.36) |
| Hindi support | Best-in-class for Indian languages |

## Open-Source Rust S2S Projects (Reference)

| Project | Language | Description |
|---------|----------|-------------|
| **Vona** | Rust | S2S runtime with protocol crates for OpenAI, Gemini, Deepgram, ElevenLabs |
| **nix-vox** | Rust | Local-first WebSocket endpoints for STT/TTS/converse |
| **Pipecat** | Python | Most mature OSS voice framework (68+ integrations) |
| **Dograh** | Python/TS | Production-ready voice agent platform |

## API Key & Quota Reference

| Provider | Get Key At | Free Tier | Est. Cost | Session Limit |
|----------|-----------|-----------|-----------|---------------|
| **Gemini** | [AI Studio](https://aistudio.google.com/app/apikey) | 10-15 RPM free | ~$0.036/min | ~10 min (resumable) |
| **OpenAI** | [platform.openai.com](https://platform.openai.com/api-keys) | None | ~$0.096/min | 60 min |
| **Deepgram** | [console.deepgram.com](https://console.deepgram.com) | $200 free credits | $0.075/min flat | Unlimited |
| **ElevenLabs** | [elevenlabs.io](https://elevenlabs.io/app/settings/api-keys) | 15 min/month | $0.08/min + LLM | 10 min default |

## Provider Selection Guide

```
User wants cloud realtime S2S:
    │
    ├── Primary recommendation: Gemini Live
    │   (free tier, native 16kHz input, cheapest at scale)
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
