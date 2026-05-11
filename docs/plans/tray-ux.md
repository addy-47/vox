Good — now the system is actually well-defined. No ambiguity left.

I’m going to give you a **production-grade UX + architecture plan** that aligns with your real-time constraints and avoids breaking the pipeline.

---

# 📄 `tray-ux-hardening.md`

---

# 1. Purpose

Harden the transcription tray into a **stable, low-latency, interruption-safe UX layer** that:

* survives noisy VAD boundaries
* supports real-time streaming feel
* maintains short-term continuity
* remains lightweight (no pipeline blocking)
* preserves Vox’s **ephemeral-first philosophy with bounded memory**

---

# 2. Core Architectural Principle

> ❗ **Decouple VAD sessions from UX interactions**

---

## ❌ Current (Incorrect)

```text
speech_start → new UI session
speech_end → destroy UI
```

---

## ✅ Required

```text
VAD session (noisy, low-level)
        ↓
Interaction session (stable, UX-level)
```

---

# 3. Interaction Model (CRITICAL)

---

## 3.1 Definitions

### VAD Session

* Triggered by:

  * `speech_start`
  * `speech_end`
* Unreliable for UX

---

### Interaction Session (NEW)

A **time-grouped logical unit of speech**

```ts
interaction = {
  id: number
  startTime: timestamp
  lastUpdateTime: timestamp
  committedText: string
  partialText: string
}
```

---

## 3.2 Continuity Rule

```text
If (speech_start - last_speech_end) < 1200ms
    → SAME interaction
Else
    → NEW interaction
```

---

## 3.3 Why This Matters

Prevents:

* UI resets on pauses
* broken sentences
* flicker

---

# 4. Visibility State Machine

---

## 4.1 States

```text
HIDDEN
APPEARING
ACTIVE
HOLD
FADING
```

---

## 4.2 Transitions

```text
speech_start:
  if HIDDEN → APPEARING → ACTIVE
  if HOLD/FADING → ACTIVE (cancel timers)

speech_end:
  → HOLD (start hold timer)

hold_timeout (3s):
  → FADING

fade_timeout (2s):
  → HIDDEN
```

---

## 4.3 Interrupt Handling (MANDATORY)

```text
speech_start during HOLD or FADING:
  → cancel all timers
  → immediately ACTIVE
```

---

## 4.4 Manual Close (❌ button)

```text
onClose:
  → terminate interaction
  → clear buffers
  → HIDDEN
```

---

# 5. Timing Configuration

---

```ts
continuity_window = 1200ms
hold_duration = 3000ms
fade_duration = 2000ms
cold_start_threshold = 60–90s
```

---

## ⚠️ Important

Cold start must be based on:

```text
last_speech_time
```

NOT UI visibility.

---

# 6. Transcript Pipeline (Frontend)

---

## 6.1 Data Model

```ts
currentInteraction = {
  id,
  committedText,
  displayText,
  targetText
}
```

---

## 6.2 Streams

```text
STT partial → targetText update
UI renderer → gradually updates displayText
```

---

## 6.3 Fake Streaming Engine

---

### Problem

STT emits chunked updates (~800ms) 

---

### Solution

Introduce **render buffer**

---

## 6.4 Rendering Algorithm

```ts
onNewTargetText(newText):
  diff = newText.slice(displayText.length)

  queue(diff)

animationLoop:
  append 1–3 chars per frame
```

---

## 6.5 Result

* smooth typing illusion
* no jump updates
* low CPU cost

---

# 7. Re-render Optimization

---

## 7.1 Rules

* NEVER replace full text
* ONLY append delta
* use refs instead of state where possible

---

## 7.2 Fix Current Issue

❌ Current:

```ts
setPartialText(text)
```

✅ Replace with:

```ts
updateTargetText(text)
renderEngine handles rest
```

---

## 7.3 Component Isolation

Split:

```text
TrayApp
 ├── Header (static)
 ├── TranscriptRenderer (high-frequency)
 └── Footer (low-frequency)
```

---

# 8. Interaction ID (CRITICAL FIX)

---

## ❌ Current

```ts
key = session_id
```

→ causes remounts

---

## ✅ Required

```ts
interaction_id (frontend-generated)
```

---

## Logic

```ts
if new interaction:
  interaction_id++
else:
  reuse existing
```

---

# 9. History System (Last 10)

---

## 9.1 Storage

```ts
history = CircularBuffer(10)
```

---

## 9.2 On Final Transcript

```ts
onFinal(text):
  if text not empty:
    history.push(text)
```

---

## 9.3 Rules

* store ONLY final text
* no partials
* no duplicates

---

## 9.4 Navigation

UI:

```text
← previous
→ next
```

Behavior:

* read-only
* does NOT affect current interaction

---

# 10. Long Audio Handling

---

## Problem

VAD splits long speech into multiple segments

---

## Solution

Interaction layer merges them:

```text
Segment A + pause + Segment B
→ same interaction
```

---

## Final Commit Strategy

```text
on transcript_final:
  append to committedText
  clear partial buffer
```

---

# 11. Hot / Cold Start Handling

---

## 11.1 Warm State

```text
recent speech < threshold
→ models active
```

---

## 11.2 Cold State

```text
no speech > 60–90s
→ unload STT / reduce memory
```

---

## ⚠️ Must NOT block pipeline

Unload must be:

* async
* outside real-time path 

---

# 12. Hotkey (Shift + V)

---

## Behavior

```text
toggle visibility
DOES NOT:
  reset interaction
  stop pipeline
```

---

## Edge Case

If hidden manually:

```text
speech_start:
  → still show tray (system priority)
```

---

# 13. Scrolling Behavior

---

## Requirements

* fixed container height
* vertical scroll
* auto-scroll ONLY when:

  * user is at bottom

---

## Prevent:

* scroll jump on updates
* forced scroll when user reading history

---

# 14. Failure & Edge Cases

---

## 14.1 Rapid Speech Toggle

```text
speech_start → speech_end → speech_start (fast)
```

Handled via:
→ continuity window

---

## 14.2 Empty Transcripts

```text
if finalText === "":
  ignore
```

---

## 14.3 STT Delay

If no partial arrives:

* keep cursor blinking
* DO NOT hide UI

---

## 14.4 Buffer Overflow (backend)

Already handled via ring buffer drop logging 

UI must:

* tolerate missing chunks
* not reset

---

# 15. Performance Constraints

---

## Must Ensure

* <1ms render updates
* no React reflow storms
* minimal allocations

---

## Techniques

* refs over state
* requestAnimationFrame loop
* memoized components

---

# 16. Final UX Behavior Summary

---

## Ideal Flow

```text
User speaks
→ tray appears instantly

Text streams smoothly
→ pauses briefly (UI stays)

User resumes
→ same text continues

User stops
→ tray holds → fades → disappears

Transcript saved in history
```

---

# 17. Final Principle

> Tray is NOT tied to speech events.
> Tray represents **user intent continuity**, not audio segmentation.

---

# 🧠 Final Evaluation

---

### 🐛 BUG (fixed)

VAD session driving UI lifecycle
→ replaced with interaction model
**Confidence: 100%**

---

### ⚖️ TRADEOFF

Ephemeral vs memory
→ bounded history (10) chosen
**Confidence: 90%**

---

### 💡 IMPROVEMENT

Fake streaming layer
→ drastically improves perceived latency
**Confidence: 95%**

---

If you want next:

I can convert this into:
→ **Antigravity prompt (step-by-step implementation plan)**
→ or **exact frontend state + hook design (no code, but near-code structure)**
