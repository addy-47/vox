## Summary

What you're building is **not remote TTS**.

It's a **TTS Provider Architecture** where inference location is configurable:

```text
TTS
├─ Local Provider
│  └─ Supertonic
│
├─ Cloud Provider
│  └─ ElevenLabs
│
└─ Remote Provider
   └─ Chatterbox
```

Exactly like you already did for LLMs.

The difference is:

```text
LLM:
same API shape
text -> text

TTS:
different model architectures
text -> audio
different runtimes
different parameters
```

So TTS cannot be OpenAI-compatible the way LLMs are.

You need provider-specific implementations.

---

## Goal

Allow users to choose:

```text
TTS Backend

[✓] Local Supertonic
[ ] Remote Chatterbox
[ ] ElevenLabs
```

without changing anything else in Vox.

---

## Architecture

Keep the existing abstraction:

```rust
trait TtsProvider {
    fn synthesize(
        &self,
        text: String,
    ) -> AudioChunkStream;
}
```

Implement:

```rust
SupertonicProvider
ChatterboxRemoteProvider
ElevenLabsProvider
```

Pipeline doesn't care which one is active.

---

## Chatterbox Remote Design

### Local Vox

Responsibilities:

```text
Audio playback
Buffering
Streaming
Settings
Provider selection
```

No Chatterbox weights.

---

### Remote Server

Responsibilities:

```text
Download Chatterbox
Load model
Run inference
Return audio
```

Chatterbox Turbo is a 350M parameter model optimized for low-latency deployment and can be self-hosted behind an API. ([docs.clore.ai][1])

---

## API

### Request

```http
POST /tts
```

```json
{
  "text": "Hello world",
  "voice": "default",
  "exaggeration": 0.5
}
```

---

### Response

```text
audio/wav
```

or

```text
audio/pcm
```

streamed back.

---

## Vox Settings

```json
{
  "tts": {
    "provider": "remote_chatterbox",
    "endpoint": "http://192.168.1.50:8000",
    "voice": "default"
  }
}
```

---

## Why This Is Worth Building

Current Vox:

```text
Quality:
★★★★☆

Speed:
★★★★★
```

Supertonic already gives real-time performance and tiny memory usage. 

Chatterbox offers:

```text
Quality:
★★★★★

Prosody:
★★★★★

Emotion:
★★★★★
```

with emotion controls, expressive speech, voice cloning, and significantly more natural delivery. ([resemble.ai][2])

So this is not a performance feature.

It's a:

```text
Premium Voice Quality Feature
```

for users with:

```text
Gaming PC
Home server
Cloud GPU
```

---

## Recommendation

For v1:

```text
TtsProvider
├─ SupertonicProvider
└─ ChatterboxRemoteProvider
```

Only.

Do not build a generic remote inference framework yet.

Do not think about remote STT yet.

Do not think about model downloads yet.

Just:

```text
Local Vox
    ↓
HTTP
    ↓
Remote Chatterbox Server
    ↓
Audio Stream
```

One provider.

One model.

One use case.

Once that works, the same pattern can later be reused for Nemotron or other TTS engines.

[1]: https://docs.clore.ai/guides/audio-and-voice/chatterbox-tts?utm_source=chatgpt.com "Chatterbox Voice Cloning | Guides"
[2]: https://www.resemble.ai/chatterbox/?utm_source=chatgpt.com "Chatterbox - Free Open Source Text to Speech Model"
