# Vox — Backend Architecture

---

## 1. Overview

The Vox backend is a **real-time, event-driven audio processing system**.

It is responsible for:

* capturing audio input
* detecting speech boundaries
* transcribing speech
* generating responses
* producing audio output

---

## 2. Core Design Principles

---

### ⚡ Streaming First

The system must operate using **streaming pipelines**, not batch processing.

* partial data flows continuously
* downstream components begin processing immediately

A streaming pipeline allows overlapping execution across STT → LLM → TTS, significantly reducing latency ([LiveKit][1])

---

### ⚡ Event-Driven Architecture

The backend is not request-response based.

It operates on events:

```text
audio_chunk → vad_event → transcript_event → response_event → audio_output
```

Each stage emits signals consumed by downstream components.

---

### ⚡ Stateless Per Turn

* Each speech turn is isolated
* No persistent conversational state in the core loop
* Context (if any) is injected externally

---

### ⚡ Local-First Execution

* All processing must work offline
* External services are optional extensions

---

### ⚡ Low-Latency Constraint

Every stage must optimize for:

* time-to-first-result
* incremental output

---

## 3. Core Pipeline

---

### Primary Flow

```text
Audio Input
→ Voice Activity Detection (VAD)
→ Speech-to-Text (STT)
→ Language Model (LLM)
→ Text-to-Speech (TTS)
→ Audio Output
```

This cascading pipeline is the standard architecture for real-time voice systems ([getbluejay.ai][2])

---

### Streaming Behavior

Each stage operates incrementally:

* STT emits partial transcripts
* LLM begins processing before full sentence
* TTS starts synthesis before full response

This reduces perceived latency to sub-second range ([LiveKit][1])

---

## 4. Audio Ingestion Layer

---

### Responsibilities

* capture microphone input
* chunk audio into frames (e.g., 20–40 ms)
* stream audio continuously

---

### Requirements

* non-blocking audio capture
* consistent sampling rate
* minimal buffering

---

### Output

```text
audio_chunk (stream)

```
## 4.1 Audio Routing Layer

### Purpose

Handles dynamic routing between:

* physical microphone
* virtual audio devices (future: meeting mode)
* internal audio streams (TTS output)

---

### Responsibilities

* switch input sources dynamically
* route TTS output to:

  * speakers (default)
  * virtual mic (future)

---

### Design

* built using sounddevice / numpy buffers
* operates as a lightweight mixer
* supports multiple input/output channels

---

### Why Required

The system is not just capturing audio — it must:

* ingest from different sources
* output to different targets

This becomes critical for meeting mode and system-level integrations.


---

## 5. Voice Activity Detection (VAD)

---

### Responsibilities

* detect speech start
* detect speech end
* segment audio into turns

---

### Behavior

```text
silence → speech_start
speech → active_stream
silence_threshold → speech_end
```

---

### Constraints

* must be low-latency
* must avoid premature cutoff
* must tolerate pauses in natural speech

VAD-based segmentation is the most practical tradeoff between latency and accuracy in real-time systems ([Wikipedia][3])

---

## 6. Speech-to-Text (STT)

---

### Responsibilities

* convert audio stream → text
* emit partial transcripts

---

### Behavior

```text
audio_stream → partial_transcript → final_transcript
```

---

### Requirements

* streaming support (mandatory)
* low time-to-first-token
* continuous updates

---

### Output Events

```text
transcript_partial
transcript_final
```

---

## 7. Language Model (LLM)

---

### Responsibilities

* process transcript
* generate response text

---

### Behavior

```text
input_text → token_stream → response_text
```

---

### Requirements

* fast inference (small models)
* short responses preferred
* optional streaming tokens

---

### Notes

* no long-term memory in core loop
* no tool execution in base architecture

---

## 8. Text-to-Speech (TTS)

---

### Responsibilities

* convert text → audio
* stream output where possible

---

### Behavior

```text
text_stream → audio_chunks → playback
```

---

### Requirements

* fast startup time
* low compute usage
* streaming or chunked output

---

## 9. Audio Output Layer

---

### Responsibilities

* play generated audio
* manage playback lifecycle

---

### Requirements

* interruptible playback
* low latency output
* smooth streaming

---

### Critical Behavior

If user starts speaking during playback:

```text
interrupt → stop TTS → switch to listening
```

This enables natural interaction flow ([LiveKit][1])

---

## 10. Event Bus / Messaging Layer

---

### Purpose

Decouples components using events.

---

### Event Types

```text
speech_start
speech_end
audio_chunk
transcript_partial
transcript_final
response_partial
response_final
audio_output_start
audio_output_end
```

---

### Design

* lightweight in-process event system
* async communication between modules

---

## 11. Concurrency Model

### Core Requirement

No stage in the pipeline should block another.

---

### Execution Model

The system must run using parallel workers:

* Audio ingestion → continuous thread/process
* VAD → real-time processing
* STT → streaming worker
* LLM → async worker
* TTS → concurrent output worker

---

### Communication

* non-blocking queues
* event-driven messaging between components

---

### Why This Is Required

Python's GIL can block execution if:

* LLM inference runs on main thread
* audio capture timing is disrupted

This will break real-time performance.

---

### Anti-Pattern (STRICTLY FORBIDDEN)

STT → wait → LLM → wait → TTS

---

### Recommended Approaches

* multiprocessing (preferred)
  OR
* asyncio + thread pools (carefully managed)


## 12. Process Architecture

---

### Single Process (Default)

* all components run in one Python process
* lightweight threading / async model

---

### Optional (Future)

* split into multiple processes if needed
* IPC-based communication

---

## 13. State Management

### Core Principle

The system is **stateless at the logic level**, but **stateful at the buffer level**.

---

### Stateless (Logic)

* each interaction turn is independent
* no persistent conversation state in core loop

---

### Stateful (Buffers — REQUIRED)

The following must maintain short-term state:

* audio sliding window (for VAD stability)
* partial transcript buffer
* response token buffer

---

### Why This Matters

Without buffering:

* first syllables get clipped
* VAD becomes unstable
* transcript flickers

---

### External State (Optional)

* logs
* settings
* history (separate system)

## 14. Persistence Boundary

### Principle

The real-time pipeline MUST remain independent of storage.

---

### Rules

* no disk writes in critical path
* no blocking I/O during processing
* only final outputs may be persisted

---

### Storage Types

* config → JSON
* logs → file system
* history → SQLite 

---

### Separation

```text
Real-time pipeline (memory only)
        ↓
Async persistence layer
```

---

### Why This Is Critical

Mixing storage with pipeline will:

* increase latency
* break real-time behavior
* introduce blocking operations


---

## 15. Failure Handling

---

### Must Handle

* STT failure
* LLM timeout
* TTS errors
* audio device issues

---

### Behavior

* fail gracefully
* continue listening
* do not crash main loop

---

## 16. Performance Constraints

---

### Target Latency

| Stage             | Target     |
| ----------------- | ---------- |
| STT (first token) | 100–200 ms |
| LLM (first token) | 200–400 ms |
| TTS start         | 100–300 ms |

---

### System Goals

* < 1 second perceived response time
* continuous responsiveness

---

## 17. Extensibility Hooks

---

### Designed for future additions:

* memory systems
* tool calling
* agentic workflows
* external APIs

---

### Rule

New capabilities must integrate via:

* event system
* modular components

---

## 18. Final Principle

> The backend is a **real-time streaming engine**, not a request-response service.

It must:

* react continuously
* process incrementally
* remain lightweight

---

[1]: https://livekit.com/blog/voice-agent-architecture-stt-llm-tts-pipelines-explained?utm_source=chatgpt.com "Voice Agent Architecture: STT, LLM, and TTS Pipelines ..."
[2]: https://getbluejay.ai/resources/voice-ai-agent-architecture?utm_source=chatgpt.com "Voice AI Agent Architecture Patterns: How to Design ..."
[3]: https://fr.wikipedia.org/wiki/Gradium?utm_source=chatgpt.com "Gradium"
