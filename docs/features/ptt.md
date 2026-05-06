# 📄 `ptt-ux-hardening.md`

---

# 1. Purpose

Define a **Push-To-Talk (PTT) interaction mode** for Vox that:

* uses **explicit user control** (mic button)
* provides **live waveform feedback (no live text)**
* performs **incremental background STT**
* returns **instant full transcript on finalize**
* respects **real-time, low-latency constraints**
* avoids memory/CPU spikes on long recordings

---

# 2. Core Principle

> ❗ PTT = **capture-first UX, transcript-second UX**

* During recording → **waveform only**
* After stop → **final transcript appears instantly**
* Backend → **already preprocessed most audio**

---

# 3. Mode Separation (CRITICAL)

---

## 3.1 Modes

```ts
mode = "PASSIVE" | "PTT"
```

---

## 3.2 Rules

```text
PTT start:
  → disable VAD routing
  → pause passive pipeline

PTT end:
  → restore passive mode
```

---

## ❗ Must Ensure

* No double STT processing
* No conflicting events (`speech_start`, etc.)
* Clean isolation of buffers

---

# 4. PTT State Machine

---

## 4.1 States

```text
IDLE
RECORDING
PROCESSING
DISPLAY
```

---

## 4.2 Transitions

```text
Mic Click:
  IDLE → RECORDING

Mic Click (again):
  RECORDING → PROCESSING

Processing complete:
  PROCESSING → DISPLAY

Timeout / next interaction:
  DISPLAY → IDLE
```

---

## 4.3 Cancel (❌)

```text
During RECORDING:
  → discard buffers
  → stop STT
  → IDLE
```

---

# 5. UX Flow

---

## 5.1 Recording Phase

```text
User clicks mic
→ waveform appears
→ recording starts
→ NO transcript shown
```

---

## 5.2 Stop Phase

```text
User clicks mic again
→ waveform stops
→ small loader (200–400ms)
→ full transcript appears instantly
```

---

## 5.3 Display Phase

```text
→ transcript visible
→ stored in history (last 10)
→ read-only
```

---

# 6. Waveform (MANDATORY)

---

## 6.1 Implementation

Use ElevenLabs waveform component:

```bash
pnpm dlx @elevenlabs/cli@latest components add waveform
```

Reference:
https://ui.elevenlabs.io/docs/components/waveform

---

## 6.2 Requirements

* Driven by **raw audio amplitude (RMS)**
* Must be **real-time (frame-level updates)**
* Must NOT depend on STT

---

## 6.3 Behavior

```text
RECORDING:
  waveform active + animated

PROCESSING:
  waveform stops

IDLE:
  hidden
```

---

# 7. Audio Chunking Strategy (CRITICAL)

---

## 7.1 Chunk Config

```ts
chunk_size = 2–4 seconds
overlap = 200–300ms
max_duration = 60 seconds
max_inflight_chunks = 2–3
```

---

## 7.2 Flow

```text
Audio Stream
→ Chunker
→ STT Worker (async)
→ Result Buffer
```

---

## 7.3 Why

* Prevents long blocking decode
* Keeps latency bounded
* Protects memory

---

# 8. Background STT Processing

---

## 8.1 During Recording

```text
audio_chunk → STT → text_chunk → store
```

---

## 8.2 Data Structure

```ts
pttBuffer = Map<chunk_id, text>
```

---

## 8.3 Ordering (MANDATORY)

```ts
on finalize:
  sort chunk_id
  join text
```

---

## ❗ Prevent

* Out-of-order transcripts
* Missing segments

---

# 9. Finalization Logic

---

## 9.1 On Stop

```text
1. Stop recording
2. Flush remaining audio
3. Send final chunk to STT
4. Wait for last decode (~100–200ms)
5. Assemble transcript
6. Emit to UI
```

---

## 9.2 Constraint

```text
<300ms perceived delay
```

---

# 10. Memory Management

---

## 10.1 Required

```text
After chunk processed:
  → discard raw audio
  → keep only text
```

---

## 10.2 Prevent

* RAM spikes
* long-session accumulation

---

# 11. Interaction Model

---

## 11.1 Rules

```text
Mic Start:
  → ALWAYS new interaction

Mic Stop:
  → finalize interaction
```

---

## 11.2 No Continuity

Unlike passive mode:

* NO merging across sessions
* NO time-based grouping

---

# 12. History Integration

---

## 12.1 Storage

```ts
history.push({
  type: "ptt",
  text: finalTranscript
})
```

---

## 12.2 Rules

* max 10 entries
* read-only
* no partials

---

# 13. Error Handling

---

## 13.1 STT Failure

```text
→ show fallback text:
  "Transcription failed"
→ keep UI stable
```

---

## 13.2 Buffer Overflow

```text
→ drop oldest pending chunk
→ log warning
```

---

## 13.3 Timeout

```text
if processing >500ms:
  → show loader
```

---

# 14. Performance Constraints

---

## Must Ensure

* No blocking STT calls
* No large audio buffers
* Smooth waveform rendering
* Minimal re-renders

---

## Techniques

* async chunk processing
* bounded queues
* requestAnimationFrame for waveform

---

# 15. Backend Integration (IMPORTANT)

---

## 15.1 Reuse STT Worker

Add new commands:

```ts
PTT_PARTIAL(chunk_id, audio)
PTT_FINAL(chunk_id, audio)
```

---

## 15.2 Do NOT

* create separate STT engine
* duplicate pipelines

---

# 16. Event Flow (Frontend ↔ Backend)

---

## 16.1 From UI

```text
ptt_start
ptt_stop
ptt_cancel
```

---

## 16.2 From Backend

```text
ptt_chunk_result
ptt_final_result
ptt_error
```

---

# 17. Edge Cases

---

## 17.1 Rapid Toggle

```text
start → stop → start quickly
→ must reset buffers cleanly
```

---

## 17.2 Long Recording

```text
> max_duration
→ auto-stop
→ finalize
```

---

## 17.3 Slow STT

```text
recording continues
→ limit inflight chunks
→ avoid queue explosion
```

---

# 18. Final UX Summary

---

```text
User clicks mic
→ waveform appears

User speaks
→ system records + processes silently

User clicks mic again
→ waveform stops
→ short loader
→ full transcript appears instantly

Transcript saved
→ ready for next interaction
```

---

# 19. Final Principle

> PTT is NOT real-time UI.

It is:
→ **real-time capture**
→ **background processing**
→ **instant final output**

---

# 🧠 Evaluation

---

### 🐛 BUG (avoided)

Full audio decode at end
→ replaced with incremental chunking
**Confidence: 100%**

---

### ⚖️ TRADEOFF

No live transcript
→ cleaner UX vs less feedback
**Confidence: 85%**

---

### 💡 IMPROVEMENT

Waveform-first interaction
→ removes perceived latency
**Confidence: 95%**

---

# ✅ Ready for Implementation

---

If needed next:
→ I can convert this into **Antigravity step-by-step prompts**
→ or define **exact frontend + Rust event contracts**
