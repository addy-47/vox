# Phase 12 — Recent Work & Agentic Runtime Ledger

This document tracks agentic turn execution, tool-calling architectures, MCP integrations, and test engineering for Phase 12.
For high-level workspace invariants and active guidelines, refer to [AGENTS.md](file:///home/addy/projects/apps/vox/AGENTS.md).

> 📖 **Phase 11 Archive:** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)

---

## Phase 11 Summary

> Compact summary of Phase 11 outcomes to provide context for Phase 12 work.

- **Full Keyboard Contract & Spatial Navigation:** Complete click-parity keyboard handling (`shortcuts.ts` SSOT), zone-bounded spatial navigation via `@noriginmedia/norigin-spatial-navigation`, context-aware drawer dispatcher (`PageDrawerContext.tsx`), and floating tooltip system (`@floating-ui/react`).
- **Memory Profiler Precision & Zero-Leak Soak:** Upgraded Linux kernel `/proc/<pid>/status` & `smaps_rollup` metrics (PSS, Anon Heap vs Shared isolation); verified 0-leak soak across 47 telemetry snapshots (164 constant DOM nodes, negative heap drift -0.465 MB/min).
- **Cloud Reasoning Multi-Provider Key Persistence:** Added persistent `cloud_keys: HashMap<String, String>` across 35+ providers, redesigned `LlmConfigDesk.tsx` 2-column grid, and clean underline tab system.

---

## Past Work (2026-09-21)

- **Phase 12 Initialized:** Transitioned from Phase 11 to Phase 12 ("Agentic Vox"). Scoped normalized LLM event stream, tool execution loop, duplex dialogue pipe extensions, MCP client integration, and initial tool capability (`generate_title`).
- **Spec Harmonization & Tool Taxonomy:** Unified tool architecture into `ToolFlow::Terminal` (single-pass with `spoken_response`, e.g. `respond_and_set_title`) and `ToolFlow::NonTerminal` (with `spoken_filler`, e.g. `search_memory`) across `events-spec.md`, `harness-spec.md`, `db-spec.md`, `ipc-spec.md`, and `memory-spec.md`.
- **Foundational Runtime & Types (Batches 0–2):** Expanded canonical tool types (`ToolFlow`, `CanonicalToolDefinition`, `CanonicalToolCall`), DB schema v5 `session_tool_calls` persistence, and implemented Invariant 14 synthesis guard latch (`turn_open` / `drained_while_open`) across pipeline atomics, playback, and transcript lifecycle.
- **Tool Runtime & Reentrant Loop (Batches 3–4):** Implemented isolated tool execution stage in `stages/tools/` (`ToolDefinition` trait, `ToolRegistry`, `ToolExecutor`, zero SQL outside `persistence/`), reentrant cognitive loop bounded by `MAX_TOOL_ITERATIONS = 5`, and single-pass terminal execution.
- **Provider & Capability Gating (Batches 5–7):** Wired session boot tool capability gating via `model_capabilities.json` cache with 4s probe fallback, persistence worker private-mode audit logging, and normalized streaming tool call delta fragments in `chat_completions.rs`.
- **Architectural Audit Pass:** Reviewed initial implementation against spec contracts; identified 15 findings and 6 residual gaps documented in `audit_review_report.md`.

---

## Past Work (2026-09-22)

- **Audit Remediation & Spec Harmonization (Batch 8):** Resolved all 15 audit findings + 6 residual gaps across Sub-batches 8.1–8.5:
  - Pass-level clause buffering with drop-all-prefix text to eliminate token leakage before tool execution.
  - Invariant 13 barge-in persistence with single-writer harness ownership (removed duplicate write in `interrupt.rs`).
  - Session-local first-turn title gate (`history.messages().len() <= 2`) replacing global turn counters.
  - Reentrant loop budget tracking with scratchpad & tool schema token counting, 5-pass bound + tool-free text fallback.
  - Non-blocking background model capability probing during session startup.
  - Hybrid RRF episodic memory retrieval ($k=60$) combining dense cosine embeddings with lexical keyword matching.
- **Harness Subsystem Flattening:** Flattened `orchestrator/` into clean top-level `services/harness/` files (`chassis.rs`, `loop.rs`, `steps.rs`, `mod.rs`), unified Step 1–7 sequential execution with explicit step banners, routed all non-terminal working transitions and `spoken_filler` audio through speech normalization, and updated `harness-spec.md` §9.1 and §9.2.
- **Modular Prompt Agentic Directives:** Updated `DEFAULT_SYSTEM_PROMPT_MODULAR` in `core/defaults.rs` with explicit directives instructing models when to invoke `respond_and_set_title` (first turn session naming) and `search_memory` (episodic context).
- **Phase 12 Target Integration Test Spec Authored (2026-09-22):** Defined testing boundaries across Seams 6, 8, 9, 11, 20 and established NEW Seam 21 (`agentic_tool_runtime_test.rs`) using `/create-test` standard; integrated directly into canonical `docs/specs/integration-test-spec.md`.
- **Phase 12 Integration Suite Execution & Verification (2026-09-22):** Authored and executed Seams 6, 8, 9, 11, 20 and NEW Seam 21 (`agentic_tool_runtime_test.rs`); resolved runtime `TOKIO_HANDLE` router thread propagation, mock engine reattachment after `EndSession`, canonical barge-in accumulator clearing (`events-spec §5 item 5`), and test stream handle wiring; verified all 122 integration tests passing (122 passed, 4 skipped in 48.94s); updated `docs/tests/integration_test_report.md`.
- **Agentic Tool & Audio Egress Evaluation Harness (2026-09-22):** Authored `evals/agentic_tool_eval.rs` and judge asset `evals/assets/prompts/judge_tool_eval.md`; wired directly to production `Harness::execute_turn` reentrant loop, capturing Turso SQLite `session_tool_calls`, intermediate stream tokens, and real TTS playback into `.wav` clips with markdown judge reports in `evals/results/agentic_tool_eval/<run_id>/`.
- **Phase 12 Mutation Testing Campaign (2026-09-22):** Executed 15 systematic mutations across `steps.rs`, `stages/tools/`, `interrupt.rs`, `session.rs`, and persistence worker against 4 integration suites (`agentic_tool_runtime_test`, `session_lifecycle_test`, `playback_interrupt_test`, `database_persistence_boundary_test`); confirmed 11 killed mutants (73.3%) and isolated 4 surviving mutants (tool flow validation, barge-in token cancel assertion, negative title validation, Invariant 13 partial-text persistence); verified all 23 integration tests passing; documented findings in `docs/tests/mutation_report.md`.
- **Provider-Model Catalog Spec v2 + Adapter Review (2026-09-22):** Renamed `model-capability-catalog-spec.md` to `provider-model-catalog-spec.md` via `git mv` (now owns provider wire contracts + model facts, banning URL-sniff dispatch and dual reasoning emission); audited full `services/llm/` ad-hoc branches and authored `docs/plans/phase12/llm_adapter_review_and_plan.md` (P0–P5 data-driven cleanup plan, testing/eval still paused).
- **Adapter Sourcing Correction (2026-09-22):** Answered community-catalog question with evidence: model facts already sync from `models.dev` (`catalog/sync.rs:9`); cloud wire params vendor from `modelparams.dev` JSON (verified live `nvidia/nemotron-3-super-120b-a12b` entry proves `reasoning_effort`+`max_tokens`, killing dual emission on NIM too); local rows come from official Ollama OpenAPI docs only; no Rust crate used (all surveyed crates hardcode mappings in code). Pinned sourcing rule into spec §2 + plan P0 (`source`+`checked` per row).
- **Provider JSON SSOT Implemented — P0 Done (2026-09-22):** Extended `baseline_providers.json` to 14 sourced rows (new `ollama_openai_compat` preset) with full wire policy, then reworked to plain mapping rows (no policy enums); `TransportType::OllamaNative` with explicit-match dispatch/health/list (URL-sniff deleted); drive-by compile repairs in `ollama.rs` stream loop + `chat_completions.rs` import.
- **Provider SSOT Rework — Enums Deleted, Vendors In (2026-09-22):** Replaced 5 policy enums with plain mapping rows (`token_limit` path, `reasoning_off{path,value}|null`, `tool_choice|nil`, `stream_usage`, `response_envelope`, `tool_stream`, `top_k_field|nil`); vendored `modelparams_vendor.json` (7 providers, 188 models) with build-failing cross-check test (caught groq `max_completion_tokens` + google native-vs-compat surface); serializers in `chat_completions.rs`/`ollama.rs` now read resolved `cfg.policy` (dual emission, unconditional `tool_choice`/`stream_options`/bare `top_k` gone); `ConnectionConfig` carries resolved policy; 58/58 lib tests green, clippy clean.
- **llm/ Defragmentation (2026-09-22):** Deleted `TokenLimitField` (400-flip wrote a field serializers no longer read; flip now toggles `policy.token_limit` path) and `CapabilitySource` (duplicated transport name; probe now matches on `transport`); stripped 14 `capability_source` keys from manifest; `modelparams_vendor.json` confirmed test-only fixture (runtime reads manifest; vendor exists so the cross-check test fails the build on drift); 58/58 lib tests green, clippy clean.
- **Seam 21 Wire Tests Fixed (2026-09-22):** Added 2 wire subtests to `agentic_tool_runtime_test.rs` driving real `RemoteTransport` against a std-only mock HTTP server (nvidia request bytes + chunked-SSE ToolCall assembly; ollama `think:false`/`num_predict` + NDJSON ToolCall); 5/5 green `--release`, red-proofed via wrong-value mutant; P5 partially done (live `#[ignore]` tests + eval rerun still open).
