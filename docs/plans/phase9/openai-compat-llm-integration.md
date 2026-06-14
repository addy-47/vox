# OpenAI-Compat LLM Integration (v0.8.4)

> **Status**: Released in v0.8.4
> This document captures the architectural decisions and implementation details of the LLM Provider refactor.

## Objective

Refactor the current LLM implementation from a single embedded backend into a provider-based architecture.

This release is about creating the abstraction layer that allows Vox to support:
- Embedded local inference
- Remote inference servers
- Future cloud providers

without requiring future pipeline rewrites.

The goal is architectural decoupling.

## Problem

Current Vox treats the LLM as a concrete implementation inside the voice pipeline.

```
STT → LLM → TTS
```

This works for embedded inference but creates scaling problems:
- Every new backend requires special-case logic
- Cloud providers become difficult to integrate
- Remote servers require pipeline modifications
- Future STT/TTS provider support becomes inconsistent

The LLM should be treated as a provider, not an implementation.

## Architectural Direction

Move from:

```
Vox
 └─ Embedded LLM
```

to:

```
Vox
 └─ LLM Provider Layer
        ├─ Embedded
        └─ OpenAI-Compatible
```

The voice pipeline should not know where inference occurs.

The pipeline should only know:
- Generate
- Stream Tokens
- Cancel
- Health Check
- List Models

Everything else becomes provider responsibility.

## Provider Types (v0.8.4)

### Embedded

Current local implementation. Runs inside Vox process, uses local GGUF models, no network dependency. Existing functionality preserved.

### OpenAI-Compatible

Represents any server exposing OpenAI-compatible APIs. Examples: Ollama, LM Studio, vLLM, llama.cpp server, LocalAI, OpenWebUI backends, self-hosted inference servers.

These should all be treated as a single provider category because they expose largely compatible APIs and streaming behavior.

## Core Principle

The backend should care about:
- Protocol

not:
- Location

Examples: localhost, 192.168.1.20, gpu-server.local, mydomain.com, AWS — all of these are simply endpoints. The protocol remains the same.

## Required Capabilities

Every provider must support:
- **Generation**: Submit prompt and receive completion.
- **Streaming**: Receive tokens incrementally. Streaming must remain first-class because Vox is a real-time voice system.
- **Cancellation**: Barge-in must continue functioning identically across all providers.
- **Health Checks**: Determine provider availability before use.
- **Model Discovery**: Fetch available models dynamically. Users should not manually maintain model lists.

## Pipeline Impact

The voice pipeline remains unchanged.

```
Audio → STT → LLM Provider → TTS
```

Only the implementation behind the provider changes. This prevents ripple effects across VAD, STT, TTS, UI, Telemetry, and State management.

## Cloud Provider Integration (v0.8.4 shipped)

Cloud LLM providers ship via the `OpenAiCompatProvider`, extended with a `provider_name` parameter that dynamically maps URLs and injects required headers:

| Provider | Endpoint URL | Mechanism |
|----------|-------------|-----------|
| **OpenAI** | `api.openai.com` | Standard OpenAI-compatible REST |
| **Gemini** | `generativelanguage.googleapis.com/v1beta/openai` | OpenAI-compatible wrapper |
| **Anthropic** | `api.anthropic.com` | URL routing + `anthropic-version` header injection |

All three share a single `LlmProvider` trait implementation — no new structs required per provider.

### Provider Tree

```
LlmProvider trait:
  └─ EmbeddedProvider          (local GGUF via llama.cpp)
  └─ OpenAiCompatProvider      (handles ALL remote/cloud)
       ├─ OpenAI-compatible servers (Ollama, LM Studio, vLLM)
       ├─ OpenAI cloud          (provider_name: "openai")
       ├─ Gemini cloud          (provider_name: "gemini")
       └─ Anthropic cloud       (provider_name: "anthropic")
```
