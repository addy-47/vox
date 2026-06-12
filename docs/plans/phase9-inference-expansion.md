# Phase 9 - Inference Expansion + Cloud Integration

##  Vox v0.8.4 — LLM Provider Architecture Refactor - Objective

Refactor the current LLM implementation from a single embedded backend into a provider-based architecture.

This release is not about adding cloud AI providers yet.

This release is about creating the abstraction layer that allows Vox to support:

* Embedded local inference
* Remote inference servers
* Future cloud providers

without requiring future pipeline rewrites.

The goal is architectural decoupling.

---

## Problem

Current Vox treats the LLM as a concrete implementation inside the voice pipeline.

```text
STT → LLM → TTS
```

This works for embedded inference but creates scaling problems:

* Every new backend requires special-case logic
* Cloud providers become difficult to integrate
* Remote servers require pipeline modifications
* Future STT/TTS provider support becomes inconsistent

The LLM should be treated as a provider, not an implementation.

---

## Architectural Direction

Move from:

```text
Vox
 └─ Embedded LLM
```

to:

```text
Vox
 └─ LLM Provider Layer
        ├─ Embedded
        └─ OpenAI-Compatible
```

The voice pipeline should not know where inference occurs.

The pipeline should only know:

```text
Generate
Stream Tokens
Cancel
Health Check
List Models
```

Everything else becomes provider responsibility.

---

## Provider Types (v0.8.4)

### Embedded

Current local implementation.

Characteristics:

* Runs inside Vox process
* Uses local GGUF models
* No network dependency
* Existing functionality preserved

### OpenAI-Compatible

Represents any server exposing OpenAI-compatible APIs.

Examples include:

* Ollama
* LM Studio
* vLLM
* llama.cpp server
* LocalAI
* OpenWebUI backends
* Self-hosted inference servers

These should all be treated as a single provider category because they expose largely compatible APIs and streaming behavior.

---

## Core Principle

The backend should care about:

```text
Protocol
```

not:

```text
Location
```

Examples:

```text
localhost
192.168.1.20
gpu-server.local
mydomain.com
AWS
```

All of these are simply endpoints.

The protocol remains the same.

---

## Required Capabilities

Every provider must support:

### Generation

Submit prompt and receive completion.

### Streaming

Receive tokens incrementally.

Streaming must remain first-class because Vox is a real-time voice system.

### Cancellation

Barge-in must continue functioning identically across all providers.

### Health Checks

Determine provider availability before use.

### Model Discovery

Fetch available models dynamically.

Users should not manually maintain model lists.

The provider reports available models.

---

## Pipeline Impact

The voice pipeline remains unchanged.

```text
Audio
 → STT
 → LLM Provider
 → TTS
```

Only the implementation behind the provider changes.

This prevents ripple effects across:

* VAD
* STT
* TTS
* UI
* Telemetry
* State management

---

## Future Roadmap Alignment

### v0.8.5

Apply identical provider architecture to STT & TTS

Goal:

```text
Embedded STT, TTS
Remote STT, TTS
Future Cloud STT, TTS
```

using the same design principles established in v0.8.4 but this would reuqire palnning as remote option may not be practical for stt & tts 


### v0.8.5 → v0.9.0

Introduce cloud providers.

Examples:

```text
OpenAI
Gemini
Anthropic
OpenRouter
Sarvam
ElevenLabs
```

These become provider implementations on top of the provider architecture created in v0.8.4.

---

## Long-Term Vision

The provider architecture is not an LLM feature.

It becomes a Vox-wide pattern.

Future Vox architecture:

```text
STT Provider
LLM Provider
TTS Provider
```

Each independently configurable.

---

## Future Engine Layer

After provider support exists for all three categories, Vox can evolve toward an engine abstraction.

Example:

### Modular Pipeline

```text
STT Provider
     ↓
LLM Provider
     ↓
TTS Provider
```

### Realtime Engine

```text
Audio
   ↓
Realtime Provider
   ↓
Audio
```

Examples:

* Gemini Live
* OpenAI Realtime
* ElevenLabs Conversational
* Sarvam Live

These bypass the traditional STT → LLM → TTS chain entirely.

The provider architecture established in v0.8.4 is the foundational step that enables this future evolution without requiring another major refactor.

---

## Success Criteria

### Functional

* Existing embedded LLM behavior remains unchanged
* Streaming remains operational
* Cancellation remains operational
* Dynamic model discovery works
* Remote OpenAI-compatible endpoints work

### Architectural

* Voice pipeline becomes provider-agnostic
* Future STT provider refactor can follow same pattern
* Future TTS provider refactor can follow same pattern
* Cloud integrations require provider additions, not pipeline rewrites
* Realtime engine architecture remains achievable in future phases

---

## Non-Goals

Not part of v0.8.4:

* Cloud provider integration
* STT provider refactor
* TTS provider refactor
* Realtime engine integration
* New models
* UI redesign

This release is purely an architectural foundation release focused on LLM decoupling and provider abstraction.

