# LLM Adapter Review & Cleanup Plan (Phase 12)

> Status: review complete, spec-first approved. SSOT going forward:
> `docs/specs/provider-model-catalog-spec.md` (v2).
> Prior spec `model-capability-catalog-spec.md` renamed via `git mv`.
> Testing + eval remain paused until Batch P5 is green.

---

## Part 1 — Review: full `services/llm/` pass (2026-09-22)

Working tree at review time was dirty (`git status`: `M evals/agentic_tool_eval.rs`,
`M transport/chat_completions.rs`, `M transport/mod.rs`, `M transport/ollama.rs`,
`M harness-spec.md`, `M tools-spec.md`). Findings below are against the checked-out
files including those uncommitted edits.

### 1.1 Dispatch is URL sniffing, not data (`transport/mod.rs:129-132`)

```rust
if cfg.capability_source == CapabilitySource::OllamaNative
    && cfg.token_limit_field == TokenLimitField::NumPredict
    && !cfg.base_url.trim_end_matches('/').ends_with("/v1")
```

Three independent concepts (discovery provenance, token key, URL string) are ANDed
to guess the transport. The `ollama` preset ships `token_limit_field: num_predict`
(`baseline_providers.json:117`) with `default_transport: chat_completions`
(`:115`), so the same preset takes different code paths depending on whether the
user typed `/v1` at the end of the URL. That is exactly how the eval sent
`num_predict` semantics down a `max_tokens` endpoint. `health_check` (`mod.rs:247-253`)
and `list_models` (`mod.rs:276-309`) repeat the same suffix branching.

### 1.2 Dual reasoning emission (`transport/chat_completions.rs:193-199`)

Disabled reasoning emits **both** `reasoning:{enabled:false}` and `think:false` on
the same body. Ollama `/v1` honors neither (it wants `reasoning_effort:"none"`);
Ollama native honors only `think`. Dual emission made the bug invisible: the key
that worked natively was silently ignored on `/v1`, and thinking tokens ate the
120-token budget before any tool call.

### 1.3 Unconditional keys (`chat_completions.rs:113-116`, `:234-257`)

`stream_options:{include_usage:true}` and `tool_choice:"auto"` are sent for every
preset whenever tools exist. Ollama `/v1` documents `tool_choice` as unsupported;
some gateways 503 on `stream_options`. There is no manifest policy gating either key.

### 1.4 Bare `top_k` on chat bodies (`chat_completions.rs:177-179`)

`top_k` is emitted top-level on OpenAI-compat bodies. Upstream convention (vLLM,
most gateways) is `extra_body.top_k` or reject. No allow-list exists.

### 1.5 Index-only tool accumulation (`chat_completions.rs:315-321`)

```rust
let idx = tc.index.unwrap_or(0);
let pending = pending_tool_calls.entry(idx).or_default();
```

Ollama `/v1` reuses `index:0` across parallel tool calls (upstream `ollama#15457`).
Two parallel calls merge into one buffer and produce corrupt JSON. Must key by
`(index, id)`.

### 1.6 `println!` wire dumps in production path (`chat_completions.rs:297,348-349`)

Raw SSE lines and full request bodies go to stdout on every turn. No flag, no
sampling. Hot-path noise that also leaks prompts into logs.

### 1.7 Ollama stream body does not parse its lines (`ollama.rs:204-246`)

The `for line in lines` loop references `chunk.message` without ever parsing
`line` into `OllamaChatChunk` (contrast the `flush()` path at `:254-278`, which
does parse). As checked out, native streaming cannot yield tokens or tool calls
from the live body — only from the trailing flush. This file is also mid-edit
(uncommitted changes); whoever owns those edits should confirm intent before P3.

### 1.8 NDJSON decoded as SSE (`ollama.rs:177`)

Native Ollama streams `application/x-ndjson` (one JSON object per line, no `data:`
framing). Reusing `SseDecoder` works only by accident of line splitting; error
lines and `done` semantics differ. Needs its own framing or a documented reason.

### 1.9 Responses transport has no tools at all (`responses.rs:26-96`, `:111-237`)

`build_request_body` never serializes `request.tools`; the stream parser only
handles `response.output_text.delta`. Any preset routed to `responses` silently
drops agentic capability.

### 1.10 Manifest too thin to prevent any of this

`baseline_providers.json` carries 7 display/routing fields; `ProviderPresetMeta`
(`catalog/types.rs:63-74`) mirrors them 1:1. Nothing declares reasoning wire,
`tool_choice`, `stream_options`, response envelope, or stream shape — so every
decision above defaulted to an `if` in shared code. Unknown presets fall back
silently to `ChatCompletions + MaxTokens` (`config.rs:102-108`): a wrong-shaped
success instead of an explicit error.

### 1.11 Probe/transport layering is inverted in places

`probe.rs` owns empirical checks (correct), but the tool probe returns `false`
immediately for `OllamaNative` and substring-matches `"tool_calls"` in error
bodies for others — asserting wire behavior without using the real parsers from
§1.5–§1.8. Mocks in Seam 21 then bypassed the wire entirely, which is why 122
green tests hid a 100%-repro live failure.

### Root cause in one sentence

Wire policy lives in branches, not data: the manifest cannot express provider
differences, so each transport guesses — and guesses differently per endpoint
spelling.

---

## Part 2 — Clean plan (no new `if provider ==` branches)

Rule for all batches: **a new provider difference = a new manifest field + a new
golden test, never a new `if` in shared code.** `TransportType` gains an explicit
`OllamaNative` variant and is the single discriminator everywhere;
`CapabilitySource` and `TokenLimitField` were deleted outright (they duplicated
the transport name and the manifest `token_limit` path).

### Batch P0 — Manifest + types (spec §2)

> Sourcing rule (not hand-guessed): every preset row cites its upstream source.
> No Rust crate offers a downloadable wire-param catalog — `llmrust`, `multi-llm`,
> `llm-connector`, `aether-llm` all hardcode per-provider mappings in code (same
> shallowness, plus a dependency). So we **vendor data files**, we don't add deps:

| Rows | Source (vendored + pinned, not live-fetched) |
|---|---|
| Cloud wire params (openai, nvidia, groq, together, deepseek, mistral, gemini, openrouter, anthropic) | `modelparams.dev` static JSON (`/api/v1/models/{provider}/{model}.json` + `schema.json`, MIT). Verified live: `nvidia/nemotron-3-super-120b-a12b` declares exactly `max_tokens` + `reasoning_effort:none\|low\|high` — no `think`, no `reasoning.enabled`. That single entry already proves our dual emission wrong on NIM too. |
| Local wire params (ollama native + `/v1`, lm_studio, vllm) | Official Ollama `docs/openapi.yaml` + `docs.ollama.com/api/openai-compatibility` (native `think:bool\|low\|medium\|high\|max`, `options.num_predict`; `/v1` accepts `max_tokens` + `reasoning_effort`, rejects `tool_choice`). modelparams.dev covers none of these — hand-row from official docs only. |
| Model facts (context, tools, structured) | `models.dev` (`models.json`, already synced by `catalog/sync.rs:9`) — unchanged. |

- Extend `baseline_providers.json` to plain mapping rows per preset: identity
  (`id`, `name`, `base_url`, `auth`, `transport` incl. `ollama_native`) plus one
  exact wire path per canonical intent (`token_limit`, `reasoning_off{path,value}`
  or `null`, `tool_choice` or `null`, `stream_usage`, `response_envelope`,
  `tool_stream`, `top_k_field` or `null`) plus `catalog` slug, `context_window`,
  `source` + `checked`. No policy enums — paths and values ARE the policy.
- Each row gains `source` + `checked` (URL + date). A row without a source
  citation fails review — this is what makes the JSON non-shallow.
- Add `ollama_openai_compat` preset (base `http://…:11434/v1`, transport
  `chat_completions`, `max_tokens`, `reasoning_off={reasoning_effort,none}`,
  `tool_choice:null`). Existing `ollama` preset becomes native-only.
- Mirror every key in `ProviderPresetMeta`; vendor `modelparams_vendor.json`
  (modelparams.dev static JSON, MIT) and cross-check mapping paths in tests.
- **Done when:** `cargo test -p vox_lib catalog::` green on bundled JSON; spec §2
  table matches every preset row.
- ✅ **DONE 2026-09-22 (revised — enums deleted, vendors in):** 14 plain rows with
  `source`+`checked`; `modelparams_vendor.json` vendored (7 providers, 188 models)
  with a cross-check test that fails the build on unverified paths (already caught
  2 real errors: groq is `max_completion_tokens`, google vendor entries describe
  the native surface, not our compat endpoint); `TransportType::OllamaNative`;
  dispatch/health/list_models match on transport (URL sniff deleted); serializers
  in `chat_completions.rs`/`ollama.rs` read `cfg.policy` (dual reasoning emission,
  unconditional `tool_choice`/`stream_options`/bare `top_k` deleted); 58/58 lib
  tests green, `clippy --lib` clean. Drive-by repairs (were blocking compile):
  native stream loop now parses each NDJSON line (§1.7), `TokenLimitField` import
  restored in `chat_completions.rs`. Partial P1 completed as required dependency
  (explicit match dispatch + resolved `policy` on `ConnectionConfig`); remaining
  P1 (unknown-preset `Err`) still open.

### Batch P1 — Config + dispatch (spec §2.3–§2.4)

- `ConnectionConfig::new`: preset lookup only; unknown preset → `Err`, never silent
  `MaxTokens` fallback. Explicit per-connection `transport` override allowed (logged).
- `dispatch_stream`: match on `config.transport` only. Delete all
  `ends_with("/v1")` / `contains("ollama")` conditions in `mod.rs`, `health_check`,
  `list_models` (replace with per-transport `resolve_url` + `health_url` fns).
- **Done when:** `grep -rn 'ends_with("/v1")' services/llm/transport` empty (except
  inside `resolve_url` fns); routing unit test maps each preset to exactly one
  transport regardless of trailing-slash spelling.

### Batch P2 — Serializers, one per transport (spec §3)

- New `transport/wire.rs`: auth injection, URL join, `stream_options` gate — the
  only shared helpers. Move, don't duplicate.
- Each `build_request_body` implements its manifest row from the §3.3 table
  (verboten: dual emission, bare `top_k`, unconditional `tool_choice`).
- Golden tests per preset: canonical `GenerationRequest` in → exact JSON out,
  asserting **absent** keys (`no tool_choice` on Ollama rows, `no think` on `/v1`
  rows, `num_predict` nested under `options` on native rows).
- **Done when:** golden matrix green; eval's exact failing body (120-token
  `qwen3.5:9b` turn-1 with `respond_and_set_title`) serializes to
  `reasoning_effort:"none" + max_tokens + tools[]` with no `tool_choice`.

### Batch P3 — Parsers, one shape per transport (spec §4)

- `openai_delta`: key accumulation by `(index, id)`; flush on
  `finish_reason=="tool_calls"` or `[DONE]`.
- `openai_delta_with_xml_fallback`: declared Ollama-`/v1`-Qwen shape; XML fallback
  over accumulated `delta.content` only when zero `delta.tool_calls` seen.
- `ollama_ndjson`: fix §1.7 (parse every line), separate `thinking` channel, drop
  `SseDecoder` reuse or document framing equivalence with test.
- Responses: implement tool envelope or route presets away from `responses`
  until implemented — silent tool-drop is a spec violation (§4 + Must-Not #11).
- Replace `println!` dumps with `log::debug!` behind `wire_debug` flag (off default).
- **Done when:** fixture replays green, including `index:0`-duplicated parallel
  calls and `<tool_call>` XML content; live-body parse path covered, not just flush.

### Batch P4 — Negotiation bounds + observability (spec §5)

- Keep the 400 loop as last resort only: ≤3 attempts, cached field flip, named-key
  reasoning downgrade with visible log. Manifest-correct requests must succeed
  first try (assert in P2 goldens, not via 400s).
- Add `wire_debug` setting plumbing (settings → transport, default off).
- **Done when:** forced-400 test exhausts to typed `LlmError` in ≤3 attempts;
  success path makes exactly 1 HTTP call per turn in goldens.

### Batch P5 — Test repair, then eval unblock (spec §9)

- Rename existing Seam 21 mocks to `mock_*` (harness routing only — counts as zero
  transport coverage).
- Promote P2 goldens + P3 fixtures to CI (no network).
- Add `#[ignore]` live tests (explicit approval only, per AGENTS §3.4): Nvidia NIM
  + Ollama `qwen3.5:9b` turn-1 asserting `session_tool_calls` row + TTS handoff.
- Re-run `agentic_tool_eval --no-judge` rapid inspection, then full judged eval.
- ✅ **PARTIAL 2026-09-22:** 2 wire subtests added to `agentic_tool_runtime_test.rs`
  (nvidia preset: exact request bytes incl. `reasoning_effort:none`, absent `think`,
  plus chunked-SSE → single `ToolCall`; ollama native: `think:false`,
  `options.num_predict`, absent `tool_choice`, plus NDJSON → `ToolCall`) via a
  std-only mock HTTP server — no new deps, no network. 5/5 green `--release`;
  red-proofed with a wrong-value mutant (failed as required, reverted).
  Still open: `mock_*` rename of the 3 harness-routing subtests, `#[ignore]` live
  tests (NIM + Ollama `qwen3.5:9b`), eval rerun.
- **Done when:** CI suite green **and** live-ignored suite green on demand; eval
  judge passes turn-1 `respond_and_set_title` interception. Only then unpause
  testing/eval.

### Verification

```bash
cargo check
cargo clippy --all-targets   # 0 warnings (wire code is not hot path, but keep clean)
cargo nextest run --release --test-threads=1 --no-fail-fast   # CI set, no network
cargo nextest run --release --test agentic_tool_runtime_test -- --ignored  # live, on approval
```

### Risks

| Risk | Mitigation |
|---|---|
| Ollama version drift (`reasoning_effort` values) | Manifest pins per-preset strings; golden tests catch drift; 400 loop stays as backstop. |
| `qwen3.5` XML shape changes | XML fallback isolated in one parser with fixture; content path never touches speakable text. |
| Manifest growth → JSON fatigue | JSON stays bundled + schema-tested; `modelparams.dev` YAML catalog consulted at authoring time, not runtime. |
| Dirty tree (`M` files above) collides with P1–P3 | Commit or stash eval-debug edits first; `println!` dumps must not survive into P3. |
