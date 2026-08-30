# Adversarial Code Review — `services/llm/` & v1 Spec

**Calibration:** Reviewing this as **production** code for a shipped desktop voice assistant (8 GB RAM, CPU-first, sub-200 ms pipeline budget). Correctness of provider routing is user-visible: a wrong transport or a guessed context window produces silent failures or truncated answers. Critique is calibrated to that bar.

---

## Review Summary

The v1 spec describes a clean 4-adapter architecture (`ChatCompletionsAdapter`, `ResponsesAdapter`, `OllamaAdapter`, `LmStudioAdapter`) dispatched by endpoint URL. **The code does not implement this.** All live traffic goes through one monolithic `OpenAiCompatProvider` that internally guesses its backend (Ollama / LM Studio / Standard) by probing `/api/tags` and `/v1/models`, and guesses GPU/cloud/context from URL substrings, model names, and a TPS threshold. The four named adapters are **dead code** — never instantiated. On top of that, capability "discovery" fabricates values (hard-coded `context_window`, GPU-from-TPS, cloud-from-URL). The structure is not just over-engineered; it is *actively misleading* relative to its own spec.

---

## 🔴 Will Break

### 1. `ResponsesAdapter` parses the wrong wire shape (broken if ever wired)
`providers/openai/responses.rs:205-235` defines `ResponsesChunk { delta: Option<DeltaObj> }` / `DeltaObj { text }` and looks for `delta.text`. The real OpenAI Responses stream emits typed events: `{"type":"response.output_text.delta","delta":"<text>",...}`. There is **no `choices[].delta.content` and no `delta.text` wrapper** — the token is the bare `delta` string, and the event name is the `type` field inside the JSON. As written, the parser matches nothing → **zero tokens emitted**. It is also currently unreachable, so this is latent.
**Replacement:** typed-event parser keyed on `type`, extracting `delta` as a string (see spec §5.2).

### 2. Token-limit field sent to the wrong endpoint (heuristic-detection failure)
`openai_compat.rs:339-364` sends `max_completion_tokens` for `StandardOpenAi`, `num_predict` for Ollama, `max_tokens` for LM Studio — decided by `detect_backend_kind` (a probe of `/api/tags`). If an Ollama served behind auth returns 401 on `/api/tags` (or any non-Ollama server happens to 200 there), it is misclassified as `StandardOpenAi` and receives `max_completion_tokens`, which Ollama/llama.cpp/vLLM reject with HTTP 400 `unsupported_parameter`. **Hard failure on a legitimate config.**
**Replacement:** `token_limit_field` is an explicit per-connection config value; the transport serializes exactly that field (spec §4, §6.3).

### 3. Embedded context window is fabricated, not read from GGUF
`capability_probe.rs:123` sets `context_window: Some(DEFAULT_MAX_CONTEXT_TOKENS)` = 2048 for every embedded model. The spec (and correctness) require reading `n_ctx_train` / `general.context_length` from the GGUF header. The value 2048 is a constant guess that will wrongly clamp or misreport real models.
**Replacement:** read context from the GGUF header; feature capabilities for the curated embedded model set are known constants (spec §6.5, §8 `embedded.rs`).

### 4. Anthropic auth header chosen by name-sniffing breaks the OpenAI-compat shim
`openai_compat.rs:97-103` sends `x-api-key` + `anthropic-version` whenever `provider_name == "anthropic"`. But Anthropic's OpenAI-compat `/v1/` shim expects `Bearer`. A user pointing at that shim with `provider_name: anthropic` gets 401. Auth must come from an explicit scheme, not a substring (spec §5.3).

---

## 🟠 Real Cost at This Scale

### 5. Backend detection is a runtime-nesting hack on the hot path
`openai_compat.rs:112-167` (`detect_backend_kind`) runs **network probes** (`/api/tags`, `/v1/models`, each with a 2 s timeout) guarded by a `OnceLock`, and uses a `block_on` helper (`openai_compat.rs:608-636`) that spins up a *separate* tokio runtime mid-call. This runs inside the persistent LLM worker thread. It is fragile (runtime flavor branching), slow against a down endpoint, and — because it guesses — wrong per #2. Should not exist at all; config drives transport.

### 6. Capability probe results are disconnected from routing
Two disjoint structs: `ModelCapabilities` (probe output, `capability_probe.rs`) and `ProviderCapabilities` (trait return, `types.rs:62`), which hard-codes `Supported` for everything. `LlmProvider::capabilities()` returns the latter; the expensive probe is **display-only**. So real discovered limits never change request behavior — the "discovery" is cosmetic and the adapters can't downgrade on what was learned.

### 7. GPU / cloud / hardware status is invented
- `capability_probe.rs:415-438` `is_cloud_provider`: substring match on URL/name (`nvidia`, `groq`, `openrouter`, `mistral`, `openai`, `googleapis.com`, …). A self-hosted vLLM at a URL containing "openai" is mislabeled "cloud".
- `capability_probe.rs:306-335` `resolve_gpu_status`: builds a human string from those substrings.
- `capability_probe.rs:298-302`: `if tps > 20.0 { server_has_gpu = true }` — GPU inferred from a latency threshold. Wildly unreliable and contradicts the zero-guesswork mandate.
All three must be deleted; hardware is `None` unless Ollama `/api/ps` `size_vram` reports it (spec §6.4).

### 8. Dead-code monolith duplicates five parsers
`providers/ollama.rs`, `lm_studio.rs`, `openai/chat_completions.rs`, `openai/responses.rs` are never constructed (confirmed via grep: only `OpenAiCompatProvider` + `EmbeddedProvider` are wired in `actor.rs:84-100`). `OpenAiCompatProvider::generate` then re-implements Ollama NDJSON + LM Studio + StandardOpenAi branching inline, including a second SSE parser (`process_line`, `openai_compat.rs:644-712`). The result: the SSE line-buffering/parsing logic is copy-pasted across **five** files. Consolidate into `transport/` with one shared `sse.rs` (spec §8).

### 9. Fallback constants contradict the spec
`mod.rs:15-16` defines `CTX_FLOOR_NON_EMBEDDED: u32 = 8_192` and `DEFAULT_CLOUD_MODEL_CTX: u32 = 1_000_000`; `mod.rs:22` `DEFAULT_MAX_CONTEXT_TOKENS = 2048`. These are exactly the synthetic numbers the spec forbids. Any code path using `effective_ctx_size()` with a floor turns "server-managed" into "clamped to a guess." Verify and remove floors (spec §6.5).

### 10. `[WARMUP]` content-sniffing inside the provider
`openai_compat.rs:243-246` skips remote calls when the last user message equals `"[WARMUP]"`. Warmup is an actor/engine concern; leaking a magic string into the provider couples transport to app orchestration and is easy to break silently.

### 11. `parse_model_metadata` is misnamed
`openai_compat.rs:715-718` returns `(clean_name, None, None)` — it formats a display name and **discards** metadata, despite the name. Misleads readers into thinking capabilities are parsed. Rename to `display_name` or delete.

---

## 🟡 Stylistic / Optional
- `block_on` (`:608`) duplicates tokio runtime creation; not needed once detection is gone.
- `resolve_chat_url` (`capability_probe.rs:338`) and the inline URL resolution in `openai_compat.rs:340` / `chat_completions.rs:82` / `responses.rs:70` are three copies of the same normalization — collapse into one.
- `StreamExt` line-buffering loop is near-identical in 5 files — shared util.
- `provider_name` is still threaded through `OpenAiCompatProvider` for nothing once auth is config-driven.

---

## What's Actually Fine
- The Chat-Completions **stream parser** (`choices[].delta.content` + `data: [DONE]`) is correct where it exists (when not misrouted).
- `LlmProvider` trait shape (`generate` / `health_check` / `list_models` / `capabilities` / `kind`) is a sane boundary.
- Cancellation via `cancel_flag` polling + `LlmFinished` / `Cancelled` events is present and reasonable.
- Ollama `/api/show` parsing of `context_length` / `capabilities` (`capability_probe.rs:248-267`) is correct *when reached*.
- Error mapping to `LlmError::Provider { status, message }` preserves the raw message — good input for the §7 error contract.

---

## Bottom Line
**Not ready.** The architecture contradicts its own spec: the adapters it documents are dead, routing is a heuristic god-object, and "capability discovery" is mostly invented. The single most important fix is to **make the connection config explicit** (transport, auth, token-limit field, capability source) and delete the detection/guesswork — after which the dead adapters and duplicated parsers can collapse into one `transport/` layer. See the v2 spec below for the exact contracts.

---
---

# LLM Provider Consolidation & Model-Agnostic Architecture Specification (v2 — Correctness-First)

> **Status:** Supersedes the v1 "LLM Provider Consolidation" spec. v1 was shallow and described a clean adapter architecture that does not exist in the code. This v2 is correctness-first: explicit config drives everything, **empirical probing is required** where observation is possible, and only genuinely-unobservable properties are left unknown.
>
> **Facts basis:** OpenAI Chat Completions / Responses API Reference, Ollama API, Anthropic & Gemini OpenAI-compat shims, and the structure of LiteLLM / OpenWebUI / aider / OpenCode / Hermes (verified 2025–2026).

---

## 1. Concept & Purpose

**Concept:** The rules by which Vox connects to, dispatches requests to, and learns the *true* capabilities of any LLM endpoint — with **explicit config as the source of truth, empirical probing for what can be observed, and zero heuristic guessing.**

**Purpose:** Today the LLM layer is a single heuristic "god-object" (`OpenAiCompatProvider`) that infers backend type, GPU presence, cloud-ness, and token-limit field from URL substrings, model names, and TPS thresholds, and ships four unused adapter modules. This makes behavior unpredictable across the many endpoint shapes Vox must support. This spec makes correctness the only variable: what is not explicitly configured or **empirically observed** is unknown — but we observe as much as the wire honestly allows.

**Three ways of "knowing" — the core distinction:**
- **Curated catalog fact (ALLOWED, recommended):** a known property of a *specifically and explicitly identified* provider, stored as versioned data and selected by the user (e.g. "Groq is a hosted accelerated service; Gemini's OpenAI-compat path 404s `/responses`; OpenAI publishes context windows"). This is *educated* knowledge made rigorous: the subject is named, the fact is data, and it is chosen — not sniffed. This is the legitimate "map of cloud providers."
- **Empirical observation (REQUIRED where observable):** sending a real request and reading the real response — a tool-probe to see if `tool_calls` comes back, a multilingual streaming probe to see if Devanagari actually appears, measuring TPS/TTFT from a live stream, or reading Ollama's native capability endpoints. Legitimate discovery the spec mandates.
- **Runtime heuristic inference (FORBIDDEN):** deriving an attribute *at request time* from an unrelated signal — `base_url.contains("groq")`, `model.contains("llama")`, `tps > 20 ⇒ GPU`. The conclusion may sometimes be right, but the *mechanism* is fragile: it drifts, breaks on proxies/self-hosts, and is invisible in config. The gap between an "educated guess" and a "curated fact" is exactly this: the educated guess is *recomputed by sniffing*; the curated fact is *asserted once, explicitly, as data*.

---

## 2. Scope

**In scope:** connection configuration; request serialization & dispatch; capability **discovery (empirical) vs. guessing (forbidden)**; error interpretation; the refactored module tree.

**Out of scope:** tokenization / context budgeting & pruning policy (→ `memory_formatting_context_assembly_spec.md`; this spec only defines *what number, if any, the budgeter may use*); embedding models; realtime/speech-to-speech paths; UI rendering of capability strings.

---

## 3. Foundational Invariants

1. **Provider identity and its known attributes come from explicit selection, never from runtime sniffing.** A `provider_preset` (explicit catalog entry) or fully manual config supplies family, transport, token field, and auth scheme. No code infers these by parsing the base URL or model id at request time. (A curated catalog carrying known provider facts is allowed and recommended — see §4.1; runtime substring/heuristic inference is forbidden.)
2. **The model id is an opaque string**, forwarded verbatim. No `contains("gpt"|"qwen"|"llama"|"o1"|"claude"|"gemini")` branching in routing/capability logic.
3. **Empirical observation is required; runtime heuristic inference is forbidden.** A capability that can be observed by sending a request *must* be discovered that way. Known provider facts come from the explicit catalog (§4.1). A capability that cannot be cleanly observed (see §6) is `None` / "Unknown" / "Server-Managed" — **never** filled by a runtime sniff, fallback constant, or regex scrape of an error message.
4. **Context window is `None` (server-managed) unless a real source reports it** (GGUF header for embedded; Ollama `context_length` / `general.context_length`; a gateway field that explicitly returns it). No floor, no `DEFAULT_*` substitution.
5. **Errors are authoritative capability/limit signals.** When a provider rejects a request, the provider's response defines the boundary (esp. `context_length_exceeded`). The client never pre-empts the provider with a guessed clamp.
6. **Two transports, one default.** Chat Completions is the universal denominator and default. Responses is an explicitly opted-in transport. The wire parser matches the *actual* shape of the transport it is told to use.
7. **No orphaned code.** Every module is reachable from `create_llm_provider`.

---

## 4. Connection Configuration Contract (Single Source of Truth)

A connection is described by an explicit, user-authored (or import-defaulted) configuration object. This object — not runtime detection — drives all behavior.

**Required explicit fields:**
- `transport`: `chat_completions` | `responses`. Default `chat_completions`.
- `base_url`: endpoint root. No coercion/"magic default URL" keyed on provider name.
- `model`: opaque model id.
- `auth`: `bearer` | `anthropic_native` | `none` (see §5.3). Chosen by this field, never from URL/`provider_name`.
- `token_limit_field`: `max_tokens` | `max_completion_tokens` | `max_output_tokens` | `num_predict`. Declares the output-length field the endpoint expects.
- `capability_source`: `ollama_native` | `probed_generic`.
  - `ollama_native`: Ollama; read `context_length`, `capabilities[]`, `size_vram` from its native endpoints (§6.1).
  - `probed_generic`: generic OpenAI-compat; run the empirical probes (§6.2) — tools/devanagari/TPS are *observed*, only context window & hardware stay unknown.
- `provider_preset` (optional, **recommended**): an explicit selection from the **provider catalog** (§4.1) — e.g. `openai`, `openrouter`, `groq`, `together`, `deepseek`, `mistral`, `nvidia_nim`, `gemini`, `anthropic`, `ollama`, `lm_studio`, `vllm`, `self_hosted`. When set, it auto-fills `base_url`, `auth`, `transport`, `token_limit_field`, and any *published* `context_window` as defaults (still user-overridable). This is the legitimate "map of cloud providers": curated, versioned, explicitly chosen — **not** heuristic inference.

**Discovered fields (filled ONLY by empirical observation, authoritative endpoint, or the explicit catalog — never by runtime sniffing):**
- `context_window: Option<u32>` — from GGUF (embedded), Ollama `context_length`/`general.context_length`, or a gateway field. Else `None`.
- `supports_tools: Option<bool>` — from the tool-probe (§6.2) or Ollama `capabilities[]`. Else `None`.
- `supports_devanagari` / `supports_latin`: `Option<bool>` — from the multilingual streaming probe (§6.2).
- `tps` / `ttft_ms`: measured from the streaming probe.
- `hardware`: from Ollama `/api/ps` `size_vram` only. Else `None`.

**Must not happen:** a `provider_name`/URL substring overrides config; any discovered field is computed from a heuristic (cloud list, GPU substring, TPS threshold, hardcoded default); `context_window` is set to a constant when nothing reported it.

#### 4.1 Provider Catalog (the legitimate "cloud provider map")
A **static, versioned data table** (`catalog.rs`) mapping a provider identifier → its known attributes:
- `default_base_url` (e.g. `https://api.openai.com/v1`, `https://openrouter.ai/api/v1`).
- `auth_scheme` (resolves §5.3).
- `transport_support`: which transports the provider natively serves (`chat_completions` always; `responses` = yes/no — e.g. Gemini = no, OpenAI = yes).
- `default_token_limit_field` (e.g. OpenAI reasoning models → `max_completion_tokens`; most others → `max_tokens`).
- `published_context_window: Option<u32>` — only if the provider *publishes* it (e.g. OpenRouter `context_length`); else `None`.
- `display_label` — an informational string such as "Cloud / Server-Managed (accelerated)" for hosted providers. This is a **curated fact**, not a measured value.

Rules:
- The catalog is **data, not logic**. Selection is explicit (`provider_preset`); the runtime never re-derives these by sniffing URLs/model names.
- A `self_hosted` / `custom` entry exists for user-run endpoints → everything server-managed, `None`, no display acceleration claim.
- Catalog attributes are *defaults*; the user's explicit overrides win.
- The catalog is the **single source** for "is this a known cloud provider and what are its facts" — replacing the deleted `is_cloud_provider` / `resolve_gpu_status` substring functions.

---

## 5. Transport Layer Contract

### 5.1 Chat Completions — `POST {base_url}/chat/completions`
**Request (only fields with a value):** `model`, `messages` (full history array), `stream: true`; `temperature`/`top_p`/`top_k`/`stop`/`seed` forwarded if present; output-length field = configured `token_limit_field` (never both, never model-name swapped); `response_format` for JSON (`json_object` or `json_schema`); `stream_options:{include_usage:true}`.
**Stream response:** lines `data: {json}`; terminal `data: [DONE]`; text = `choices[].delta.content`; tool calls = `choices[].delta.tool_calls[]` (accumulate `function.arguments` by `index`/`id`); `finish_reason` (`stop`/`length`/`content_filter`/`tool_calls`); `usage` on final chunk with `completion_tokens_details.reasoning_tokens`.

### 5.2 Responses — `POST {base_url}/responses` (opt-in only)
**Request (only fields with a value):** `model`, `input` (string or item array — **never** a `messages` array), `instructions` (top-level system), `stream: true`; `max_output_tokens` (**not** `max_tokens`/`max_completion_tokens`); `tools`/`tool_choice`/`text.format` for JSON. (Reasoning-effort is intentionally **out of scope** for Vox — not sent, not modeled.)
**Multi-turn (recommendation — stateless full-history):** Each turn, the transport receives the **full conversation** in `request.input.messages` and flattens it into the `input` item array every turn (parity with Chat Completions sending the full `messages` array). The server is treated as stateless — no `previous_response_id`, no server-side conversation tracking, idempotent and reconnect-safe. **The conversation assembler must supply full history to the transport; the transport must NOT append-only the latest user turn.** (Alternative, not required: stateful `previous_response_id` + sending only the delta — only if payload size becomes a real problem; introduces response-id tracking and truncation handling, so rejected as default.)
**Stream response (parser MUST extract):** event name is the JSON `type` field (not the SSE `event:` line; tolerate it if present). Text deltas: `{"type":"response.output_text.delta","delta":"<text>",...}` — the token is the **bare `delta` string**; there is **no `choices[].delta.content` and no `delta.text` wrapper**. Terminal: `response.completed`/`incomplete`/`failed`. `response.output[]` items are typed (`message`, `function_call`, `reasoning`). Usage in `response.completed.response.usage` uses `input_tokens`/`output_tokens`/`total_tokens`. Terminal `data: [DONE]`.

**Must not happen:** Responses parser assumes Chat-Completions shape; Chat Completions sends `max_completion_tokens` to a `max_tokens` connection (and vice-versa) — declared field is authoritative; Responses selected for an endpoint that 404s it (Gemini OpenAI-compat supports only `/v1/chat/completions` + `/v1/embeddings`); transport appends only the latest user turn instead of flattening full history.

### 5.3 Authentication Header Contract
| Scheme (explicit) | Header(s) |
| --- | --- |
| `bearer` | `Authorization: Bearer <key>` — OpenAI, OpenRouter, Groq, Together, DeepSeek, Mistral, NVIDIA NIM, vLLM, LM Studio, llama.cpp, Ollama-shim, **and the Anthropic OpenAI-compat `/v1/` shim** |
| `anthropic_native` | `x-api-key: <key>` + `anthropic-version: 2023-06-01` — **only** the native Anthropic Messages path (a distinct connection, never inferred) |
| `none` | no auth |

**Must not happen:** auth chosen by sniffing `provider_name == "anthropic"` and sending `x-api-key` to an OpenAI-compat `/v1/` shim (breaks it); empty `Authorization` sent to a local server that rejects missing headers (send dummy non-empty Bearer when key empty).

---

## 6. Capability Discovery Contract (empirical, not guessed)

### 6.0 Principle
Discovery is **observation**, not assumption.
- **Authoritative endpoint metadata** (Ollama native) → read directly.
- **Empirical probes** (send a request, read the response) → required for everything observable on a generic endpoint: tool support, script support, TPS/TTFT.
- **Genuinely unobservable cleanly** → `None` / "Unknown". Only `context_window` (generic cloud) and `hardware` (non-Ollama) fall here, because no clean wire source exists and we do **not** scrape error strings to invent them.

### 6.1 Authoritative source — Ollama (`capability_source: ollama_native`)
- `GET /api/tags` → enumeration; `details.{family, families, parameter_size, quantization_level}`, optional `context_length`, `capabilities[]`.
- `POST /api/show` → `details`, `capabilities[]` (`completion`,`tools`,`thinking`,`vision`), `model_info["general.context_length"]` (integer), `parameters`, `template`.
- `GET /api/ps` → loaded models; `size_vram` (hardware), `context_length` (active `num_ctx`).
Stored as-is. These are the only non-`None` hardware/context values permitted.

### 6.2 Generic OpenAI-compat (`capability_source: probed_generic`)
Run the **empirical probes** (the existing `capability_probe.rs` logic, retained and required):
- **`supports_tools`** → the **tool-probe**: send a function schema with `tool_choice:"auto"`; if the response carries `tool_calls` (or legacy `function_call`), `supports_tools = true`, else `false`. This is observation, not guessing — keep it.
- **`supports_devanagari` / `supports_latin`** → the **multilingual streaming probe**: send a prompt asking for Devanagari+Latin; inspect the actual streamed characters. Observation — keep it.
- **`tps` / `ttft_ms`** → measured from the same streaming probe (real latency numbers). Keep.
- **`context_window`** → `None` (server-managed). Standard `/v1/models` does **not** return it; we do **not** scrape it from error messages. If the server later exposes it (gateway field), read it; otherwise unknown.
- **`hardware` / GPU (`vram_bytes`)** → `None` for all remote endpoints. VRAM cannot be measured on someone else's server, so it is never claimed. (Do **not** infer GPU from TPS or URL.) However, an explicitly-selected cloud `provider_preset` may carry a **display label** ("Cloud / Server-Managed (accelerated)") as a curated catalog fact (§4.1) — this is informational only and **must not** drive any routing, token, or budgeting decision. The distinction: we *assert* the hosted service is accelerated (educated, explicit), but we do **not** *measure* or *depend* on it.
- `GET /v1/models` → enumeration only (which model ids exist). Defensive parse of any extra fields; absence is normal.

### 6.3 Token-limit field negotiation (error-driven, not heuristic)
Send the configured `token_limit_field`. If the provider returns HTTP 400 `error.code == "unsupported_parameter"` naming the field, switch to the alternative for that connection and cache it. Driven by the provider's explicit rejection — never by model-name sniffing.

### 6.4 Embedded (in-process GGUF) — known constants OK
The curated embedded model set is fixed and known to us; its **feature capabilities may be hardcoded** (`supports_tools`, `supports_devanagari`, `supports_latin`, `supports_json` = known values) because they will not change. The one thing still read from the model, not hardcoded, is **`context_window`** — taken from the GGUF header (`n_ctx_train` / `general.context_length`), never from `DEFAULT_MAX_CONTEXT_TOKENS`.

### 6.5 Context window — the only "unknown by design"
`Some(N)` only when a clean source reports `N`: the GGUF header (embedded); Ollama `context_length`/`general.context_length`; a gateway field (e.g. OpenRouter `context_length`); or the explicit **provider catalog's `published_context_window`** (§4.1). Otherwise `None` (server-managed). **No floor constant, no `DEFAULT_*` substitution, no synthetic 1_000_000 / 8_192 / 2_048, no error-string regex scrape.**

**Must not happen:** `context_window` set from a constant for a remote endpoint; feature caps hardcoded for *remote* models without an authoritative/empirical source; GPU/cloud decided by TPS threshold or URL substring; token-ceiling scraped from an error message and stored as `context_window`.

---

## 7. Error Contract

Provider errors parsed from the **response body's `error` object**, not HTTP status alone.
- `error.type`: `invalid_request_error` (400), `authentication_error` (401), `permission_error` (403), `not_found_error` (404), `rate_limit_error` (429), `server_error` (500).
- `error.code` observed: `context_length_exceeded`, `unsupported_parameter`, `invalid_api_key`, `string_quadratic_overflow` (Gemini), `insufficient_quota`.
- **Context overflow** is the one stable detectable contract: HTTP 400 `invalid_request_error` + `code:"context_length_exceeded"`. This — not a client-side token count — triggers compaction upstream. If `context_window` is `Some(N)`, the budgeter may pre-trim; if `None`, it relies on this error and surfaces it cleanly.
- 429 may mean rate-limit *or* quota — distinguish by `type`/`code`, not status.
- **No error-message scraping:** a token ceiling inferred from a regex over the error *text* is **forbidden** (removed). The raw `message` is preserved for display; it is never parsed into a stored `context_window`.

**Must not happen:** silent client-side clamp from a guessed context window instead of honoring `context_length_exceeded`; an unrecognized `error.code` treated as fatal rather than a generic 4xx with raw message preserved.

---

## 8. Finalized Module Structure (where everything lives)

```
services/llm/
├── mod.rs                  # constants, LlmProvider trait, LlmEngine trait
├── types.rs                # GenerationRequest, GenerationOptions, OutputConstraint,
│                           #   ProviderCapabilities (static transport features),
│                           #   ModelCapabilities (empirical discovery result), LlmError
├── config.rs               # ConnectionConfig (§4) — SSOT for "what is explicit"
├── catalog.rs              # Provider catalog / presets (§4.1) — curated, versioned
│                           #   known-provider facts. DATA, not inference logic.
├── actor.rs                # LlmActor: request queue, cancellation, provider selection
├── policy.rs               # context budgeting — clamps ONLY when context_window = Some(N)
├── llama_cpp.rs            # LlmWorker: LOW-LEVEL in-process llama.cpp FFI engine.
│                           #   UNCHANGED location. Knows nothing about transports/config.
├── embedded.rs             # EmbeddedProvider: wraps LlmWorker, implements LlmProvider.
│                           #   Feature caps = known constants (§6.4); ctx from GGUF header.
├── probe.rs                # Empirical discovery (§6): Ollama-native + generic probes
│                           #   (tool-probe, multilingual probe, TPS/TTFT). No guessing.
└── transport/              # REMOTE providers only (OpenAI-compat family)
    ├── mod.rs              # Transport enum dispatch — picks serializer+parser from config
    ├── chat_completions.rs # §5.1 serialize + SSE parser (choices[].delta.content)
    ├── responses.rs        # §5.2 serialize + typed-event parser (type field, delta string)
    ├── ollama.rs           # Ollama /api/chat NDJSON (delegated when num_predict + ollama_native;
    │                       #   still a Chat-Completions-shaped stream)
    └── sse.rs              # SHARED line-buffer / SSE frame splitter (kills 5x duplication)
```

**`llama_cpp.rs` placement (explicit answer):** it stays at `services/llm/llama_cpp.rs` as the low-level `LlmWorker` FFI engine. It is **local/in-process**, not a "transport," so it lives *outside* `transport/`. `embedded.rs` is the thin `LlmProvider` wrapper over `LlmWorker`. `transport/` is exclusively for **remote** OpenAI-compat endpoints.

**Responsibility contracts:**
- `config.rs`: validates `transport`/`auth`/`token_limit_field`/`capability_source` are explicit; rejects implicit inference.
- `actor.rs` / `transport/mod.rs`: selects serializer+parser **from `config`**, not from an `/api/tags` probe. `detect_backend_kind` + its `OnceLock`+`block_on` hack are **deleted**.
- `probe.rs`: implements §6. For `ollama_native` reads the three Ollama endpoints; for `probed_generic` runs the empirical probes and returns `None` only for context window & hardware.
- `transport/sse.rs`: single shared streaming splitter used by all transports.
- `embedded.rs`: feature caps hardcoded (known); `context_window` from GGUF header.

**Deletion / purge:**
- Delete unused `providers/ollama.rs`, `providers/lm_studio.rs`, `providers/openai/chat_completions.rs`, `providers/openai/responses.rs`, `providers/openai_compat.rs` (dead; duplicate logic → `transport/`).
- Delete `OpenAiCompatProvider::detect_backend_kind`, `is_cloud_provider`, `resolve_gpu_status`, and the TPS→GPU heuristic in `probe.rs`.
- Delete `parse_token_ceiling_from_error` (regex error-scrape) and `validate_token_cap`'s ceiling-scrape path.
- Collapse `parse_model_metadata` (returns `None`s) into an honestly-named `display_name` or delete.
- Remove the `[WARMUP]` content-sniffing from the provider; warmup is actor/engine concern.
- Remove `CTX_FLOOR_NON_EMBEDDED`, `DEFAULT_CLOUD_MODEL_CTX` constants (floors forbidden).

---

## 9. Verification & Invariants
1. **Zero model-name heuristics:** grep `services/llm/` for `contains("qwen"|"llama"|"gpt"|"o1"|"claude"|"gemini"|"anthropic")` in routing/capability → 0 hits.
2. **Zero URL/name cloud-or-GPU inference:** no `is_cloud_provider`, `resolve_gpu_status`, TPS→GPU branch.
3. **Empirical probes present & used:** `supports_tools`/`supports_devanagari`/`tps` come from `probe.rs` observations, not constants; results stored in `ModelCapabilities` and consumed by UI + budgeter.
4. **Server-managed transparency:** generic endpoint reporting nothing → `context_window = None`, UI "Server-Managed", budgeter prunes nothing client-side.
5. **Explicit-config dispatch:** `create_llm_provider` builds transport purely from `ConnectionConfig`; no `/api/tags` probe to choose path.
6. **Responses parser correctness:** real `response.output_text.delta` (type-field, `delta` string) yields tokens; Chat-Completions fixture does not satisfy it and vice-versa.
7. **Multi-turn Responses:** fixture with full `input` history returns a coherent multi-turn answer; transport does not append-only the latest user turn.
8. **Token-field negotiation:** declared field sent; on `unsupported_parameter` 400 the connection flips and caches.
9. **No orphan modules:** every file under `services/llm/` reachable from `create_llm_provider` or a test.
10. **Compilation & tests:** `cargo check --all-targets` + `cargo nextest run --release --test-threads=1` green; no clippy warnings.

---

## 10. Open Questions
1. **`ModelCapabilities` vs `ProviderCapabilities` unification:** they are two real concerns (empirical discovery result vs. static transport feature support). Confirm the budgeter + UI read `ModelCapabilities.context_window` (the empirical/`None` one), and `ProviderCapabilities` stays for transport-level feature flags — they are not "disconnected," they answer different questions.
2. **Embedded tool-call format:** embedded `supports_tools = true` (hardcoded) — confirm the in-process engine actually emits the tool-call shape the rest of the pipeline expects, or scope it to `false` until verified.
3. **Multi-turn state:** confirmed default = stateless full-history flattening for both transports (§5.2). Stateful `previous_response_id` deferred unless payload size demands it.
