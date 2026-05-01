# Meeting Reply Mode — Vox Assistant (Native Architecture)

---

## 1. Goal

Enable Vox to act as a **silent meeting co-pilot** that:

* listens to meeting audio in real-time
* maintains rolling context
* generates intelligent replies
* injects speech into the meeting using TTS

---

## 2. Status

* **Not part of MVP (v1)**
* Planned for **v2**

---

## 3. Core Architectural Shift

Meeting mode is **not a separate system**.

It is an **extension of the core audio pipeline**:

```text
Meeting Audio → VAD → STT → LLM → TTS → Virtual Mic
```

---

## 4. Audio Routing Model (CRITICAL)

---

### Principle

Use **virtual audio devices (loopback routing)** instead of:

* packet interception
* process injection

---

### Why

Virtual audio cables act as an internal audio bridge:

* output of one app → input of another
* no quality loss
* near real-time transfer ([Virtual Audio Cable][1])

---

## 5. Native Audio Handling (Rust)

---

### Library

* `cpal` (cross-platform audio I/O)

---

### Capabilities

* enumerate devices
* create input/output streams
* real-time audio callbacks ([docs.rs][2])

---

### Important Detail

Audio streams run on **high-priority threads**, ensuring:

* low latency
* non-blocking capture

---

## 6. Audio Flow (Updated)

---

### 6.1 Incoming Audio (Meeting Capture)

```text
Meeting App Output
    ↓
Virtual Audio Device (Sink)
    ↓
Rust (cpal input stream)
    ↓
STT (Qwen3-ASR)
```

---

### Behavior

* Vox listens continuously
* STT emits `text_delta`
* context updated incrementally

---

---

### 6.2 Outgoing Audio (Reply Injection)

```text
LLM Response
    ↓
TTS (Chatterbox-Turbo)
    ↓
Rust Output Stream (cpal)
    ↓
Virtual Microphone Device
    ↓
Meeting App Input
```

---

### Behavior

* user triggers “Send to Meeting”
* audio streamed into meeting
* no blocking

---

## 7. Device Routing (User Setup)

---

### One-Time Setup

User configures:

* Meeting Speaker → Virtual Sink
* Meeting Microphone → Virtual Mic

---

### Concept

Virtual cable behaves like:

```text
App A (Zoom) → Virtual Output → Virtual Input → Vox
```

It routes audio internally without hardware dependency ([Wikipedia][3])

---

## 8. System Behavior

---

### Continuous Loop

```text
audio → STT → context → LLM → suggestion
```

---

### Reply Flow

```text
User clicks "Send"
→ LLM generates reply
→ TTS streams audio
→ injected into meeting
```

---

## 9. Context Management

---

### Problem

Meeting audio = long + continuous

→ LLM context will explode

---

### Solution

* rolling summarization
* sliding context window
* discard raw history

---

### Rule

```text
Keep:
- summary
- last few exchanges

Discard:
- full transcript
```

---

## 10. UI Behavior

---

### Modes

* **Idle**
* **Listening to Meeting**
* **Generating Reply**
* **Speaking into Meeting**

---

### Requirements

* clear state indicators
* live transcript preview
* single-click reply

---

## 11. Performance Constraints

---

### Critical Limits

* STT must remain lightweight
* LLM context ≤ 4096 tokens
* TTS must stream (<200ms start)

---

### Latency Expectation

* 1.5s – 3s response acceptable
* (higher than normal mode)

---

## 12. Reliability Considerations

---

### Must Handle

* missing virtual devices
* incorrect routing
* device switching

---

### Behavior

* auto-detect devices
* fallback to mic if needed
* show clear warnings

---

## 13. Privacy & Safety

---

### Requirement

User must be informed:

* meeting recording laws
* consent responsibility

---

### Controls

* pause listening
* disable mode instantly

---

## 14. Cross-Platform Complexity

---

### Windows

* VB-Cable / WASAPI loopback

---

### Linux

* PipeWire routing

---

### macOS

* CoreAudio virtual devices (manual setup)

---

## 15. Future Enhancements

---

* auto reply suggestions
* name detection
* full autonomous replies
* meeting summary
* smart interruption

---

## 16. Key Architectural Constraints

---

### Must NOT

* block main pipeline
* duplicate audio processing
* introduce extra buffering

---

### Must Ensure

* reuse core pipeline
* same event-driven flow
* minimal additional overhead

---

## 17. Final Principle

> Meeting mode is NOT a feature.

It is a **different audio routing layer on top of the same real-time engine**.

---

**Status:** Planned for v2
**Priority:** Medium-High
**Owner:** @adhbhut

[1]: https://vac.muzychenko.net/en/?utm_source=chatgpt.com "Virtual Audio Cable - connect audio applications, route and ..."
[2]: https://docs.rs/cpal/latest/cpal/?utm_source=chatgpt.com "cpal - Rust"
[3]: https://en.wikipedia.org/wiki/Virtual_Audio_Cable?utm_source=chatgpt.com "Virtual Audio Cable"
