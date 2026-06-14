# Gemini Multimodal Live API Integration

## Endpoints

```
# Google AI (free tier, API key auth)
wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent?key=YOUR_KEY

# Vertex AI (production, OAuth2 bearer token)
wss://{location}-aiplatform.googleapis.com/ws/google.cloud.aiplatform.v1.LlmBidiService/BidiGenerateContent
```

## Authentication

- **Free tier**: API key from [Google AI Studio](https://aistudio.google.com/app/apikey)
- **Production**: OAuth2 bearer token (Vertex AI)
- **Critical**: Enabling billing on a project removes the free tier entirely — use a separate project for testing

## Free Tier & Quotas

| Tier | RPM | RPD | TPM |
|------|-----|-----|-----|
| Free (Flash models) | ~10-15 | 1,500 | 250K |
| Paid — Tier 1 | $250/mo cap | — | 4M TPM |

## Pricing

| Model | Input Audio | Output Audio |
|-------|-------------|-------------|
| Gemini 3 Flash Live | $3.00/1M tokens (~$0.005/min) | $12.00/1M tokens (~$0.018/min) |

**Total: ~$0.036/min** — 6.4x cheaper than OpenAI Realtime.

## Audio Specs

- **Input**: 16 kHz PCM16 — **matches Vox natively, no resampling needed**
- **Output**: 24 kHz PCM16 — requires resampling to device playback rate
- **Chunk size**: 20-100ms recommended per WebSocket message
- **Encoding**: Base64 inside JSON text frames (Google AI) or binary frames (Vertex AI)

## Protocol — Two-Queue, Four-Task Architecture

The production code uses a **two-queue architecture** that is the most important pattern to replicate. Audio and control messages are split into separate queues to eliminate head-of-line blocking.

```
WebSocket from VAD ringbuf
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

Gemini Receive Loop (model → playback/frontend):
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

## Session Configuration

The exact configuration sent at connection time:

```json
{
  "tools": [
    { "googleSearchRetrieval": {} },
    { "functionDeclarations": [] }
  ],
  "responseModalities": ["AUDIO"],
  "speechConfig": {
    "voiceConfig": { "prebuiltVoiceConfig": { "voiceName": "Charon" } },
    "languageCode": "hi-IN"
  },
  "systemInstruction": {
    "parts": [{"text": "Dynamic prompt with language, grounding, history, and project context."}]
  },
  "temperature": 0.2,
  "inputAudioTranscription": {},
  "outputAudioTranscription": {},
  "thinkingConfig": { "thinkingBudget": 0 },
  "realtimeInputConfig": {
    "automaticActivityDetection": {
      "disabled": false,
      "startOfSpeechSensitivity": "START_SENSITIVITY_HIGH",
      "endOfSpeechSensitivity": "END_SENSITIVITY_HIGH",
      "prefixPaddingMs": 20,
      "silenceDurationMs": 100
    }
  },
  "sessionResumption": { "handle": null }
}
```

## Interruption Handling (Two-Phase)

**Phase 1 — Local Stop (immediate, 0ms):**
1. Stop/silence local playback immediately
2. Send `activityStart` event to Gemini (interrupts model generation)
3. If in PTT mode: sleep 50ms, then send `activityEnd`
4. Set `interrupt_active = true`

**Phase 2 — Server Confirmation (200-500ms later):**
1. On receiving `serverContent.interrupted`:
   - Set `interrupt_active = false`
   - Trigger sync, reset state
   - Forward `{"interrupted": true}` to frontend

## Turn Lifecycle

```
1. User speaks → VAD detects → audio chunks sent via audio_queue
2. Gemini streams input_transcription (interim) → forwarded to frontend
3. Gemini detects end-of-speech → processes with LLM
4. Gemini streams:
     a. output_transcription (interim) → forwarded to frontend
     b. model_turn audio (inline_data) → queued for playback
     c. model_turn text → accumulated into current_model_response
5. On turn_complete:
     a. Persist turn to history
     b. Send turn_complete event to frontend
     c. Increment completed_turns, reset turn state
     d. Log token usage
```

## Latency

| Metric | Value |
|--------|-------|
| TTFT (first audio token) | ~200-320ms (optimal), ~960ms (cold start) |
| Full A2A loop | ~770-1,415ms |
| Client-side mute on barge-in | **<50ms** |

## Limitations

- Preview on Google AI (GA only on Vertex AI) — breaking changes possible
- ~10 min WebSocket timeout — session resumption tokens valid 2h
- Post-barge-in freeze bug (workaround: nudge timer or activity_start signal)
- Mid-sentence truncation (server-side turnComplete fires prematurely)
- Compounding token billing (past audio re-billed every turn — enable contextWindowCompression)
