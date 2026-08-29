# LLM Provider Consolidation & Model-Agnostic Architecture Specification

---

## 1. Executive Summary & Purpose

This specification defines the architecture, wire protocol adapters, capability probing, and parameter handling for the Vox LLM subsystem.

### Core Objectives:
1. **100% Model-Agnostic Execution:** Eliminate all model-name heuristics, substring matching (`contains("qwen")`, `contains("llama")`), and hardcoded model catalogs. The model ID is treated as an opaque string identifier passed directly to the upstream endpoint.
2. **Endpoint-Driven Protocol Dispatch:** Route requests based on configured endpoint URLs and protocol types (`/v1/chat/completions`, `/v1/responses`, `/api/chat`, or local embedded), never by model name.
3. **Zero Synthetic Fallbacks & Truthful UI State:** If an endpoint does not expose a context window or hardware attributes, store `None` and treat the context limit as **Server-Managed**. Never artificially truncate tokens or synthesize fake default numbers (like 4096).
4. **Empirical Capability Discovery:** Discover live Tokens Per Second (TPS), Time To First Token (TTFT), and tool calling support empirically through active micro-probes and standard endpoint introspection (`/v1/models`, `/api/show`, GGUF headers).

---

## 2. Current State vs. Target Architecture

### 2.1 What Is Currently There (Current Flaws)
- **Brittle Model String Heuristics:** `capability_probe.rs` uses `heuristic_embedded_caps()` which pattern-matches strings like `"qwen"`, `"gemma"`, and `"llama"` to guess tool support and language capabilities.
- **Artificial Fallback Defaults:** When a context window is not returned, the system falls back to hardcoded numbers (e.g. 4096), risking premature truncation of large-context models.
- **Fragmented Protocol Handling:** Dispersed logic across `chat_completions.rs`, `responses.rs`, `ollama.rs`, and `openai_compat.rs` with inconsistent header injection and error wrapping.
- **Parameter Divergence Assumptions:** Complex code attempting to rewrite parameters (e.g. `max_tokens` vs `max_completion_tokens`) based on guessed model names rather than letting endpoints or gateways handle standard parameter mapping.

### 2.2 Desired Logic (Target Architecture)
- **Opaque Model Strings:** The model identifier is passed verbatim to the server.
- **Clean Protocol Separation:**
  - `ChatCompletionsAdapter` handles standard OpenAI `/v1/chat/completions` (SSE `data: {...}`).
  - `ResponsesAdapter` handles OpenAI `/v1/responses` (new Responses API stream format).
  - `OllamaAdapter` handles native Ollama `/api/chat` (NDJSON stream).
  - `EmbeddedProvider` handles in-process `llama.cpp` using direct GGUF header metadata.
- **Server-Managed Context Budgeting:** When `context_window` is `None`, the local pipeline passes full context to the server without artificial clamping. If the server rejects the request, the error is surfaced transparently.
- **Truthful Diagnostics:** UI displays `"Server / Provider Managed"` for unexposed limits and `"N/A"` for unexposed hardware stats.

---

## 3. Desired File & Module Structure

```
app/src-tauri/src/services/llm/
├── mod.rs                      # Module exports, constants, default timeout configuration
├── types.rs                    # GenerationRequest, LlmError, ProviderCapabilities, TokenStreamEvent
├── actor.rs                    # LlmActor managing request queue and cancellation signals
├── capability_probe.rs         # Empirical micro-prober (introspection + live TPS/TTFT measurements)
├── llama_cpp.rs                # C++ FFI bindings for in-process embedded GGUF inference
├── policy.rs                   # Context truncation policy (only active when limit is explicitly known)
└── providers/
    ├── mod.rs                  # LlmProvider trait definition & ProviderKind enum
    ├── embedded.rs             # EmbeddedProvider (in-process llama.cpp engine)
    ├── ollama.rs               # OllamaAdapter (native /api/chat NDJSON stream)
    ├── lm_studio.rs            # LmStudioAdapter (specialized local daemon introspection)
    ├── openai_compat.rs        # OpenAiCompatProvider (smart dispatcher & health checker)
    └── openai/
        ├── mod.rs              # Exports ChatCompletionsAdapter & ResponsesAdapter
        ├── chat_completions.rs # Standard OpenAI /v1/chat/completions SSE protocol adapter
        └── responses.rs        # OpenAI /v1/responses protocol adapter
```

---

## 4. Component Contracts & Refactoring Details

### 4.1 Protocol Adapters (`services/llm/providers/`)

```
                                  ┌───────────────────────────┐
                                  │    GenerationRequest      │
                                  │  • messages: Vec<Message> │
                                  │  • stream: true           │
                                  │  • temperature / options  │
                                  └─────────────┬─────────────┘
                                                │
                                                ▼
                        ┌───────────────────────────────────────────────┐
                        │            Endpoint-Driven Router             │
                        └───────┬───────────────┬───────────────┬───────┘
                                │               │               │
        ┌───────────────────────┘               │               └───────────────────────┐
        ▼                                       ▼                                       ▼
┌────────────────────────┐      ┌────────────────────────┐      ┌────────────────────────┐
│ ChatCompletionsAdapter │      │    ResponsesAdapter    │      │     OllamaAdapter      │
│  - POST /chat/complet  │      │  - POST /responses     │      │  - POST /api/chat      │
│  - SSE delta parser    │      │  - SSE response parser │      │  - NDJSON line parser  │
│  - messages array      │      │  - input array         │      │  - messages array      │
└────────────────────────┘      └────────────────────────┘      └────────────────────────┘
```

1. **`ChatCompletionsAdapter`:**
   - **Wire format:** `POST {base_url}/chat/completions`
   - **Payload:** `{"model": model, "messages": [...], "stream": true, "temperature": ..., "max_tokens": ...}`
   - **Stream parsing:** Reads `data: {"choices": [{"delta": {"content": "..."}}]}` SSE events.
   - **Cancellation:** Aborts HTTP stream immediately when `cancel_flag` is set.

2. **`ResponsesAdapter`:**
   - **Wire format:** `POST {base_url}/responses`
   - **Payload:** `{"model": model, "input": [...], "stream": true, "temperature": ...}`
   - **Stream parsing:** Reads Responses API event stream format (`response.output_item.added`, `response.text.delta`).

3. **`OllamaAdapter`:**
   - **Wire format:** `POST {base_url}/api/chat`
   - **Payload:** `{"model": model, "messages": [...], "stream": true, "options": {...}}`
   - **Stream parsing:** Reads NDJSON lines `{"message": {"content": "..."}, "done": false}`.

4. **`EmbeddedProvider`:**
   - **Engine:** Direct in-process `llama.cpp` context.
   - **Metadata:** Extracts `n_ctx_train` and context limits directly from GGUF binary header metadata.

---

### 4.2 Capability Probing (`capability_probe.rs`)

#### Probing Sequence:
1. **Introspection Query:**
   - For Ollama: `GET /api/show` with `{"name": model_id}` $	o$ extracts parameter size, family, quantization, and context length if available.
   - For LM Studio / OpenAI: `GET /v1/models` $	o$ extracts loaded model list.
   - For Embedded: Read GGUF header KV pairs.
2. **Empirical Micro-Probe (Live Benchmarking):**
   - Sends a small 1-token test prompt (`"Hi"`) with `stream: true`.
   - Records empirical `ttft_ms` (Time To First Token) and `tps` (Tokens Per Second).
3. **Empirical Tool-Calling Check:**
   - Sends a dummy tool schema definition to test if the endpoint accepts tool parameters or returns HTTP 400.
4. **Result Storage:**
   - Constructs `ModelCapabilities`:
     ```rust
     pub struct ModelCapabilities {
         pub model_id: String,
         pub provider_kind: String,
         pub supports_tools: bool,
         pub supports_latin: bool,
         pub supports_devanagari: bool,
         pub context_window: Option<u32>,   // None = Server-Managed (NO fallback)
         pub tps: Option<f32>,
         pub ttft_ms: Option<u32>,
         pub server_has_gpu: bool,
         pub is_gpu_accelerated: bool,
         pub gpu_status: String,
         pub vram_bytes: Option<u64>,        // None = N/A
         pub parameter_size: Option<String>, // None = Unknown
         pub quantization: Option<String>,   // None = Unknown
         pub family: Option<String>,         // None = Unknown
         pub tested_at_epoch: u64,
     }
     ```

---

### 4.3 Context Budgeting & Truncation Policy (`policy.rs`)

- **Explicit Limit (`context_window = Some(N)`):**
  - The context builder monitors token count (via ModernBERT/tiktoken estimate).
  - If token count approaches $N - 	ext{reserve}$, dynamic memory facts and older conversation turns are gracefully pruned before sending.
- **Server-Managed Limit (`context_window = None`):**
  - **No local artificial clamping or truncation is performed.**
  - Full context is dispatched to the upstream provider.
  - If the provider responds with an HTTP 400 Context Exceeded error, the error is caught and displayed cleanly in the UI.

---

## 5. Refactor Implementation Steps

| Step | Target File | Action |
| :--- | :--- | :--- |
| **1** | `services/llm/capability_probe.rs` | **Purge `heuristic_embedded_caps`:** Delete all model name substring matching. Rely purely on GGUF metadata or empirical probes. |
| **2** | `services/llm/capability_probe.rs` | **Remove Synthetic Fallbacks:** Remove arbitrary 4096 defaults; assign `context_window = None` whenever the server does not report a limit. |
| **3** | `services/llm/providers/openai/` | **Standardize Wire Adapters:** Ensure `chat_completions.rs` and `responses.rs` cleanly parse their respective protocols without model-name branches. |
| **4** | `services/llm/providers/openai_compat.rs` | **Clean Dispatcher:** Implement URL-based routing to select `ChatCompletionsAdapter` vs `ResponsesAdapter`. |
| **5** | `services/llm/policy.rs` | **Update Truncation Guard:** Guard context clamping to only trigger when `context_window` is `Some(N)`. |

---

## 6. Verification & Invariants

1. **Zero Hardcoded Model Checks:** A grep for model family names (`"qwen"`, `"llama"`, `"gemma"`, `"gpt-"`, `"o1"`, `"claude"`) in `services/llm/` must return 0 hits in routing or capability logic.
2. **Server-Managed Transparency:** When pointing to an endpoint that does not expose context limits, the UI must display `"Server / Provider Managed"` and zero tokens must be pruned locally.
3. **Endpoint Disambiguation:** Configuring a `/responses` endpoint routes exclusively to `ResponsesAdapter`; configuring standard `/v1` routes exclusively to `ChatCompletionsAdapter`.
4. **Compilation & Test Pass:** All tests pass via `cargo check` and `cargo nextest run --release --test-threads=1`.
