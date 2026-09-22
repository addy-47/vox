# Provider & Model Catalog Specification (v2)

> Renamed from `model-capability-catalog-spec.md` via `git mv` (2026-09-22).
> Reason: the old name described only _model_ facts (context windows, tool flags).
> The eval failure came from _provider wire_ facts (which JSON key disables thinking,
> which token field each endpoint accepts, who supports `tool_choice`).
> This spec now owns **both contracts** and, critically, **how adapters must consume
> them without ad-hoc `if/else` logic**.

## Name & Concept

This specification governs **Provider Wire Contracts, Model Capability Discovery,
Baseline Synchronization, Setting Boundary Constraints, and Compaction Budget
Allocation**.

Two catalogs, two jobs:

| Catalog | Question it answers | Example |
|---|---|---|
| **Provider manifest** (`baseline_providers.json` + `ProviderPresetMeta`) | _How_ do I talk to this endpoint? Which keys, which transport, which shapes? | Ollama native wants `think:false` + `options.num_predict`; Ollama `/v1` wants `reasoning_effort:"none"` + `max_tokens`; OpenAI wants `max_completion_tokens`. |
| **Model catalog** (`baseline_catalog.json` + `ModelSpec`, synced from models.dev) | _What_ can this model do? How big, tools, structured? | `qwen3.5:9b` supports tools; `gpt-4o` context 128k. |

---

## Purpose

Users connect LLMs across local embedded engines, self-hosted servers, and cloud
providers. Models vary in context windows (8,192 to 1,000,000+ tokens) and output
limits (2,048 to 16,384+ tokens). Providers vary **independently** in wire format:
token-limit key, reasoning off-switch, `tool_choice` support, `stream_options`
support, tool streaming shape.

Hardcoding either axis into application code breaks as providers update offerings.
Trial-and-error HTTP 400 negotiation at runtime stalls voice turns. Scattered
`if provider == "ollama"` branches across transports caused the Phase 12 eval
failure (reasoning tokens ate the 120-token budget; `num_predict` sent to a
`max_tokens` endpoint; `tool_choice:auto` sent where unsupported).

This specification establishes a language-agnostic contract for:

1. Maintaining an updatable external baseline catalog of **model** specifications.
2. Maintaining a versioned, bundled **provider wire manifest** with full serialization policy.
3. Resolving capabilities through an empirical, provenance-tracked hierarchy.
4. Resolving the wire contract **once** at provider construction into an immutable
   `ResolvedEndpoint`; transports dispatch on it with **zero string sniffing**.
5. Dynamically bounding user-facing settings between invariant code floors and
   discovered capability ceilings.
6. Deterministically calculating compaction output token limits without magic numbers.

---

## Must Be True

### 1. External Baseline Catalog & Runtime Synchronization (models)

1. The system must maintain an offline-capable baseline catalog of model
   specifications (sourcing schemas comparable to `models.dev`).
2. The catalog must record for each known model:
   - Unique model identifier and display name.
   - Model family identifier (e.g. `llama-3.1`, `qwen2.5`, `gemma3`).
   - Published context window (maximum input tokens).
   - Published maximum output tokens.
   - Functional capabilities (tool calling, structured output).
3. The system must bundle a static offline snapshot so startup works with zero network.
4. When network is available, the system must periodically perform non-blocking
   background sync with conditional requests (`If-None-Match`/ETag):
   - HTTP 304 → zero disk writes, zero re-parsing.
   - HTTP 200 → atomically replace persistent cache, update in-memory registry.
   - Timeouts/DNS/5xx → fail silently, never interrupt turns or corrupt cache.
5. `sync.rs` remains the sole owner of model-catalog parsing and sync.
   Transports must never fetch model metadata directly.

### 2. Provider Wire Manifest (the clean solution — replaces ad-hoc branches)

1. `baseline_providers.json` is the SSOT for **how to serialize for each provider**.
   Every preset must declare the full wire policy (no implicit defaults that change
   meaning per transport):
   - `id`, `default_base_url`, `auth_scheme`, `display_label`.
   - `transport`: `ollama_native` | `chat_completions` | `responses`.
     The value is **authoritative**. Transports dispatch on it, never on URL suffixes.
   - `token_limit_field`: `max_tokens` | `max_completion_tokens` |
     `max_output_tokens` | `num_predict` (with `location`: top-level body vs
     `options.` object for native transports).
   - `reasoning_wire`: exactly one of:
     - `think_bool` (Ollama `/api/chat`: `think:false` when Disabled, omit when Enabled),
     - `reasoning_effort_none` (Ollama `/v1`, OpenAI reasoning models:
       `reasoning_effort:"none"` / `reasoning:{effort:"none"}` when Disabled, omit when Enabled),
     - `omit` (plain chat models: send nothing),
     - `warn_omit` (Responses API: no off-switch exists; log once, send nothing).
   - `tool_choice_policy`: `auto_when_tools` | `never` (Ollama native + `/v1`:
     `never` — the field is unsupported and must be omitted even when tools exist).
   - `stream_options_policy`: `include_usage` | `never` (gateways with 503s on the
     flag, e.g. Nvidia NIM class, use `never`).
   - `response_format_wire`: `chat_response_format` | `native_format` | `text_format`
     (maps `OutputConstraint` to the correct envelope per transport).
   - `tool_stream_shape`: `openai_delta` | `ollama_ndjson` | `openai_delta_with_xml_fallback`
     (declares which parser owns the stream).
   - `extra_body_passthrough`: explicit allow-list (e.g. `top_k` for vLLM class).
     Anything not listed is dropped, never forwarded speculatively.
2. `ProviderPresetMeta` (`catalog/types.rs`) must mirror every manifest field 1:1.
   Adding a wire knob without adding the struct field + JSON key + preset value is
   a spec violation.
3. Every preset row must carry `source` (upstream URL) + `source_checked` (date).
   Authoritative sources, in priority order:
   - Cloud endpoints: `modelparams.dev` per-model JSON (`/api/v1/models/{provider}/{model}.json`,
     schema-validated, MIT) — vendored and pinned, never live-fetched at runtime
     (offline-first startup is invariant).
   - Local endpoints (Ollama native + `/v1`, LM Studio, vLLM): official vendor docs
     (Ollama `docs/openapi.yaml`, `docs.ollama.com/api/openai-compatibility`) —
     modelparams.dev does not cover these.
   - Model facts only: `models.dev` (`models.json`, via `catalog/sync.rs`).
   A row without a source citation fails review. No Rust crate is used as a source:
   surveyed crates (`llmrust`, `multi-llm`, `llm-connector`, `aether-llm`) embed the
   same per-provider `if` mappings in code — depending on one would trade our
   branches for theirs plus a dependency tree.
3. `ConnectionConfig::new()` (`transport/config.rs`) resolves preset → config by
   **lookup only**. It must not infer transport from URL substrings, model-name
   substrings, or probe results. Unknown preset → explicit error, never silent
   `MaxTokens` fallback that changes wire meaning.
4. `RemoteTransport::dispatch_stream()` (`transport/mod.rs`) matches **only** on
   `config.transport`. URL-suffix conditions (`ends_with("/v1")`) are forbidden.
   An Ollama preset pointing at a `/v1` base URL must either be a distinct preset
   (`ollama_openai_compat`) or an explicit per-connection `transport` override —
   never an invisible branch.

### 3. Canonical Request Pipeline (single translation point per transport)

1. `GenerationRequest` (`provider.rs`: `input`, `options`, `output`, `purpose`,
   `tools`) is the only input transports accept. Transports must not read settings,
   history, or registry state.
2. Each transport exposes exactly two functions with no cross-imports:
   `build_request_body(config, request) -> Value` and `stream_*` parser emitting
   `LlmStreamEvent::{Token, ToolCall, Finished}`. Shared helpers live in one
   `transport/wire.rs` module (auth injection, URL join, SSE framing) — never
   copy-pasted per transport.
3. Canonical → wire mapping table (normative; manifest selects the row):

   | Canonical | ollama_native | ollama `/v1` | openai chat | responses |
   |---|---|---|---|---|
   | `max_output_tokens` | `options.num_predict` | `max_tokens` | `max_completion_tokens` (o-series) / `max_tokens` | `max_output_tokens` |
   | `ReasoningMode::Disabled` | `think:false` | `reasoning_effort:"none"` | `reasoning_effort:"none"` (reasoning models) else omit | warn + omit |
   | tools present | `tools[]`, no `tool_choice` | `tools[]`, no `tool_choice` | `tools[]` + `tool_choice:"auto"` | `tools[]` per Responses envelope |
   | `JsonSchema` | `format:<schema>` | `response_format:{json_schema}` | `response_format:{json_schema}` | `text:{format:{json_schema}}` |
   | stream usage | n/a (NDJSON) | `stream_options:{include_usage:true}` iff policy allows | same as `/v1` | n/a |
4. Dual-emitting two representations of one intent (`reasoning:{enabled:false}` **plus**
   `think:false` on the same body) is forbidden. One canonical intent → one wire key
   per manifest row. Dual emission is what made the eval failure invisible (one key
   worked natively, the other was silently ignored on `/v1`).
5. `top_k` and any other non-OpenAI key are sent **only** via `extra_body_passthrough`
   allow-list. Bare `top_k` at top level of a chat-completions body is forbidden.

### 4. Stream Parsing Contracts (one shape per transport, declared in manifest)

1. `openai_delta`: accumulate `choices[].delta.tool_calls[]` keyed by **`(index, id)`**,
   append `function.name` + `function.arguments` fragments, flush on
   `finish_reason=="tool_calls"` or `[DONE]`. Keying by index alone is forbidden
   (Ollama `/v1` reuses `index:0` across parallel calls).
2. `ollama_ndjson`: each NDJSON line is complete (`message.tool_calls[]` with full
   `arguments` objects, `message.content`, `message.thinking`, `done`). No cross-line
   accumulation. `thinking` must route to a thinking channel or drop per settings —
   never concatenate into speakable text.
3. `openai_delta_with_xml_fallback`: run the `openai_delta` parser first; if a turn
   yields `delta.content` containing `<tool_call>` with zero `delta.tool_calls`,
   run the XML fallback parser over the accumulated content buffer. The fallback is
   a declared manifest shape, not a sniff.
4. Debug `println!` of raw SSE lines and request bodies is forbidden in production
   transports. Wire observability goes through `log::debug!` behind a single
   `VoxSettings.llm.wire_debug` flag (off by default, never in hot path).

### 5. Negotiation Policy (bounded, cached, never per-request guessing)

1. The 400-fallback loop in `transport/mod.rs` (`flip_token_field`,
   `degrade_request_on_unsupported`) is retained **only** as a bounded last resort:
   max 3 attempts, then typed `LlmError`. It must never be the primary mechanism
   for known providers — correct manifest rows must succeed first try.
2. Successful negotiation mutates **cached** `active_token_limit_field`, never the
   bundled preset. Cache is per `RemoteTransport` instance and logged at `info`.
3. Downgrading `ReasoningMode::Disabled → Enabled` on 400 is allowed only when the
   provider error names the reasoning key. Silently enabling thinking to satisfy a
   400 without telling the caller is forbidden; the retry must be visible in logs.

### 6. Hierarchical Capability Resolution with Provenance (models, unchanged)

1. Every capability attribute (`context_window`, `max_output_tokens`) carries a
   provenance tier: `CatalogBaseline` > `FamilyBaseline`, with `ProbedServer`
   winning when empirically verified, `UserConfigured` winning when explicitly set,
   else `Unknown`. Precedence:
   $$\text{UserConfigured} > \text{ProbedServer} > \text{CatalogBaseline} > \text{FamilyBaseline} > \text{Unknown}$$
2. Embedded models: read from local manifests/headers. Ollama native: query
   `/api/show` + `/api/ps`. Cloud endpoints without metadata: catalog hypothesis +
   connectivity probe. Unknown stays `Unknown` — never fabricate defaults.
3. `probe.rs` owns all empirical probing. Transports own serialization. Probes call
   transports; transports never call probes (no cycle, no transport-internal 400
   scraping duplicated in probe code).

### 7. User Setting Boundaries (Code Floor vs. Probed Ceiling)

1. `context_window` floor 8,192 tokens (code invariant). Ceiling from resolved
   capability. `Unknown` → UI shows "Server-Managed / Unknown", rejects manual
   input until probed.
2. Conversational `max_output_tokens` governs dialogue turns only, never compaction.

### 8. Compaction Output Token Budgeting Contract

1. Compaction budget is decoupled from conversational settings:
   $$\text{slice} = \text{floor}(\text{context\_window} \times 0.15)$$
   $$\text{effective\_output\_tokens} = \min(\text{slice},\; \text{probed\_max\_output\_tokens})$$
2. `Unknown` probed max → use `slice`. Clamp down when slice exceeds capability.
3. Compaction input excludes base prompt, persona, personal memory (headroom below
   the 85% trigger by construction).

### 9. Testability Contract (prevents repeat of the mock-only Seam 21 gap)

1. Every manifest row needs a golden `build_request_body` unit test (canonical in →
   exact JSON out, asserting absent keys too: no `tool_choice` on Ollama rows, no
   `think` on `/v1` rows).
2. Every `tool_stream_shape` needs a golden SSE/NDJSON fixture replay test
   (including Ollama `index:0` duplication and `<tool_call>` XML fallback).
3. Live provider tests (`#[ignore]`, explicit approval only) hit real Nvidia NIM +
   Ollama endpoints and assert `session_tool_calls` persistence. Mock-stream tests
   cover harness routing only and must be named as such — they never count as
   transport coverage.

---

## Must Not Happen

1. **No URL sniffing for dispatch.** `base_url.ends_with("/v1")`,
   `contains("ollama")`, or model-name substring matches to pick serializers.
2. **No dual emission.** Two wire keys for one canonical intent on the same body.
3. **No unconditional `tool_choice`, `stream_options`, or `response_format`.**
   Every such key is gated by manifest policy.
4. **No index-only tool accumulation.** Parallel tool calls keyed by bare `index`.
5. **No `println!` wire dumps.** `log::debug!` behind `wire_debug` flag only.
6. **No hardcoded UI parameter arrays** (`[2048, 4096, 8192]`) for remote models.
7. **No invented values.** Unknown stays Unknown.
8. **No conversational bleed into compaction.** Conversational token settings never
   reach compaction requests.
9. **No unbounded negotiation loops.** Max 3 attempts, then typed error.
10. **No transport→probe imports.** Probes call transports, never the reverse.
11. **No mock-stream test counted as transport coverage.** Names must say `mock_`
    vs `live_` vs `golden_`.

---

## Out of Scope

1. **Inference hosting / proxying**: routing traffic, managing API keys.
2. **Conversation sampling policy**: temperature/top-p defaults per purpose live in
   the harness spec; this spec owns only their wire encoding.
3. **Memory dedup/storage**: governed by `memory-spec.md` / `db-spec.md`.
4. **Model weight distribution**: GGUF manifests, download manifests.

---

## Migration Notes (from v1 model-capability spec)

- Sections 1, 6, 7, 8 are carried over (reworded, provenance gains `UserConfigured`).
- Sections 2–5, 9 are **new**: they codify the provider wire manifest and pipeline
  rules that previously lived as scattered `if/else` across `transport/`.
- `baseline_providers.json` must grow from 7 display/routing fields to the full
  wire policy (§2.1) before any transport refactor lands (spec-first per AGENTS §4.3).
- `TransportType` gains `OllamaNative` as an explicit variant; `CapabilitySource`
  stays as discovery provenance only and loses all dispatch role.
