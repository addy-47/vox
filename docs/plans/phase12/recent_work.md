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
