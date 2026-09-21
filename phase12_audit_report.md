# Architectural Audit & Spec-Fidelity Report: Phase 12.1 (Agentic Vox)

> **Document Type:** Formal Architectural Audit  
> **Target Subsystems:** `services/harness/`, `services/llm/`, `pipeline/assistant/`, `persistence/`  
> **Source of Truth:** `docs/specs/harness-spec.md`, `docs/specs/tools-spec.md`, `docs/specs/db-spec.md`  
> **Audit Focus:** Uncommitted changes across Batches 0–7 against approved specifications.

---

## 1. Executive Summary

A comprehensive, line-by-line inspection of the Phase 12.1 implementation reveals that while the modular decomposition of the orchestrator, synthesis guard latch, and basic tool execution traits were compiled cleanly, **critical architectural invariants and behavioral contracts were violated or bypassed in implementation**.

Key findings:
1. **Audio Hot-Path & Intent Violation**: Text generated before a tool proposal is flushed to TTS as `TurnResponse` before the tool filler plays, breaking state locking and intent isolation.
2. **Invariant 13 Violation (Barge-in)**: User turn rollback is hardcoded unconditionally on cancellation; partial assistant speech is discarded without persistence.
3. **Capability Probe Invalidation**: Embedded GGUF models hardcode tool support despite zero engine support; remote probe accepts error bodies containing `"tool_calls"` as positive capability.
4. **Stage Encapsulation Bypass**: `PromptBuilderStage` was bypassed; request assembly was copied into `loop_driver.rs`.
5. **Context Budget Blind Spot**: Ephemeral scratchpad tokens are omitted from in-turn context budgeting during reentrant tool passes.
6. **Dead Gating**: Memory retrieval gating is hardcoded to `true` in turn execution, ignoring user configuration.

---

## 2. Invariant Verification Ledger (`harness-spec.md §1.3`)

| Invariant | Spec Requirement | Actual Code Implementation | Status |
| :--- | :--- | :--- | :--- |
| **Invariant 1 & 2: Single Orchestrator & Stage Encapsulation** | Single orchestrator owns sequencing. Stages have typed interfaces and zero inter-stage communication. Stages own payload assembly. | `PromptBuilderStage` is untouched. Request payload assembly and dynamic tool schema injection were placed directly in `loop_driver.rs:172-209`. | ❌ **BROKEN** |
| **Invariant 4: Sacred Audio Hot Path & Audio Intent Isolation** | Zero allocations or locks on hot path. Non-terminal tool proposal must never leak text tokens into physical speech synthesis. Only model filler is spoken. | `tools.rs:91-101` flushes unpunctuated chunker remainder to TTS as `AudioIntent::TurnResponse` prior to entering `NonTerminalPhase` and playing filler. | ❌ **BROKEN** |
| **Invariant 8: Turn Completion Authority** | `VoxEvent::LlmFinished` emitted once after terminal model pass. Never emitted on cancelled/aborted passes. | `router.rs:328` emits `LlmFinished` at stream exhaustion. `tools.rs:45` also calls `emit_finished` on Terminal tools. | ⚠️ **FRAGILE** |
| **Invariant 12: Ephemeral Turn-Local Scratchpad** | Intermediate tool calls/results exist strictly in ephemeral scratchpad; dropped at commit/cancellation. Parity with DB. | `loop_driver.rs:107` scopes `scratchpad` to loop iteration function; dropped on return. | ✅ **HONORED** |
| **Invariant 13: Single-Writer Barge-In Persistence** | Cancelled branch inspects `partial_text`: if non-empty, commits `(query, partial)` to history + persistence; if empty, rolls back. | `finalize.rs:8-20` executes unconditional `history.rollback_last_user_turn()`. `loop_driver.rs:268` discards `partial_text` with `..`. | ❌ **BROKEN** |
| **Invariant 14: Turn Synthesis Guard Latch** | `turn_open` / `drained_while_open` prevents premature `Speaking -> Ready` transitions when audio drains before LLM finish. | Implemented in `atomics.rs:170-205`, `playback.rs:74`, and `llm.rs:68-87`. Cleared on all terminal outcomes. | ✅ **HONORED** |
| **Invariant 15: Self-Healing Foreign Keys** | `INSERT OR IGNORE INTO sessions` self-heal before writing `session_tool_calls` or updating titles. | Handled via `ensure_session_exists` in `sessions.rs:305-325` and `tool_calls.rs:26`. | ✅ **HONORED** |
| **Invariant 16: Non-Blocking Discovery Probe** | Probe runs asynchronously off the router thread under `session_starting = true` so dictation is never blocked. | `session.rs:721` uses `tokio_handle.block_on(...)` synchronously on the caller thread. No `session_starting` flag exists. | ❌ **BROKEN** |
| **Invariant 17: Terminal Tool Single-Pass Resolution** | `spoken_response` extracted from arguments and dispatched directly to TTS as `TurnResponse` without second LLM pass. | Implemented in `tools.rs:30-44`. | ✅ **HONORED** |

---

## 3. Deep-Dive Audit Findings

### Finding 1: Prefix Text Leakage & Audio Intent Race on Tool Proposal
- **Location:** [`services/harness/orchestrator/tools.rs:91-101`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/tools.rs#L91-L101)
- **Spec Reference:** `harness-spec.md §5.1, §6 Case B`; `tools-spec.md §1.2, §4.2`
- **Mechanism:**
  When a model streams text tokens before emitting a tool call (e.g. *"Checking that... "* followed by `search_memory`), `router.rs:107-109` pushes the tokens into `TurnAccumulator` and dispatches clauses to TTS as `AudioIntent::TurnResponse`.
  When `StreamPassOutcome::ToolCallReceived` returns to `tools.rs`, lines 91-101 take the chunker remainder and dispatch it again as `AudioIntent::TurnResponse`.
  Line 120 then invokes `enter_non_terminal_phase`, which dispatches the model's `spoken_filler` as `AudioIntent::InterimFiller`.
- **Failure Mode:**
  1. Playback receives `TurnResponse` audio packets, triggering the playback engine to transition state toward `Speaking`.
  2. Playback then receives `InterimFiller` audio packets, which expect `InteractionState::Working`.
  3. The user hears fragmented prefix speech, followed abruptly by filler audio.
- **Spec Mandate:** Any prefix text emitted before a tool call must be dropped from audio delivery; only `spoken_filler` or `spoken_response` may be synthesized.

---

### Finding 2: Invariant 13 Barge-In Data Loss
- **Location:** [`services/harness/orchestrator/finalize.rs:8-20`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/finalize.rs#L8-L20) & [`loop_driver.rs:268-270`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/loop_driver.rs#L268-L270)
- **Spec Reference:** `harness-spec.md §1.3.13, §6 Phase 7 Branch B`
- **Mechanism:**
  In `loop_driver.rs:268`:
  ```rust
  StreamPassOutcome::Cancelled { .. } => {
      LoopAction::Terminal(handle_turn_cancelled(ctx.harness_arc, turn_id))
  }
  ```
  In `finalize.rs:8-20`:
  ```rust
  pub fn handle_turn_cancelled(harness_arc: &Arc<Mutex<Option<Harness>>>, turn_id: u32) -> TurnOutcome {
      let mut guard = harness_arc.lock();
      if let Some(ref mut harness) = *guard {
          harness.history.rollback_last_user_turn();
      }
      TurnOutcome::Cancelled { turn_id }
  }
  ```
- **Failure Mode:**
  If the user barges in after the model has spoken several sentences, `partial_text` is discarded with `..`. The user's query is rolled back from `ConversationHistoryStage`, and no `PersistenceEvent::TurnCompleted` is dispatched. The turn vanishes from both memory and SQLite storage.

---

### Finding 3: Capability Probe Logic Invalidation
- **Location:** [`services/llm/catalog/probe.rs:242`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/catalog/probe.rs#L242) & [`probe.rs:554-562`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/catalog/probe.rs#L554-L562)
- **Spec Reference:** `tools-spec.md §3.1`
- **Mechanism:**
  1. **Embedded False Positive:**
     `probe_local_embedded` hardcodes:
     ```rust
     supports_tools: true
     ```
     However, [`services/llm/embedded/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/embedded/mod.rs) and `worker.rs` contain zero tool handling, zero JSON grammar enforcement, and zero tool-call event emission.
  2. **Remote Substring False Positive:**
     `empirical_tool_probe` checks:
     ```rust
     if let Ok(body) = resp.text().await {
         return body.contains("\"tool_calls\"") || body.contains("\"function_call\"");
     }
     ```
     If an endpoint rejects the request with HTTP 400 and an error payload such as `{"error": "tool_calls are not supported for this model"}`, `body.contains("\"tool_calls\"")` evaluates to `true`, incorrectly marking an unsupported model as tool-capable.
  3. **No Distinction of Error Types:**
     `tools-spec.md §3.1.3` requires recording persistent negative status on HTTP 400 ("Function calling not supported"), while treating transient network timeouts as session-scoped. The probe treats all failures identically as `false` with no distinction.

---

### Finding 4: Bypassing of `PromptBuilderStage`
- **Location:** [`services/harness/orchestrator/loop_driver.rs:172-209`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/loop_driver.rs#L172-L209) vs [`stages/prompt.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/prompt.rs)
- **Spec Reference:** `harness-spec.md §1.1.2, §4.3`
- **Mechanism:**
  `PromptBuilderStage` was left unmodified (73 lines). Instead of having `PromptBuilderStage` own `build_generation_request` as mandated by §4.3, a duplicate assembly function was written inside `loop_driver.rs`.
- **Consequence:**
  Violates Stage Encapsulation (`harness-spec.md §1.3.1, §1.3.2`). Egress generation requests and dynamic tool schemas are constructed ad-hoc inside the loop driver rather than within the dedicated prompt stage.

---

### Finding 5: Context Budget Blindness to Ephemeral Scratchpad
- **Location:** [`services/harness/orchestrator/loop_driver.rs:111-162`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/loop_driver.rs#L111-L162)
- **Spec Reference:** `harness-spec.md §6 Phase 2 Step 4 & Case B`
- **Mechanism:**
  In `execute_loop_iterations`, subsequent iterations re-enter generation by directly calling `build_generation_request` and `dispatch_llm_request`.
  `ContextBudgetStage::evaluate_utilization` is never called within the loop.
- **Failure Mode:**
  If a tool (e.g. `search_memory`) returns a substantial observation (e.g., 2,000–4,000 tokens), the loop does not verify remaining context headroom before dispatching the next pass. The request is sent directly to the model provider, risking context window overflow errors.

---

### Finding 6: Memory Retrieval Gating Hardcoded to `true`
- **Location:** [`services/harness/orchestrator/loop_driver.rs:187-191`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/loop_driver.rs#L187-L191)
- **Spec Reference:** `tools-spec.md §4.2, §7.2`
- **Mechanism:**
  ```rust
  let filter = ToolFilter {
      turn_id,
      title_is_unset: !harness.title_set,
      memory_retrieval_enabled: true, // Hardcoded
  };
  ```
- **Failure Mode:**
  `stages/tools/registry.rs:54` correctly implements the check `if name == "search_memory" && !filter.memory_retrieval_enabled`, but the caller always provides `true`, completely ignoring system settings for memory retrieval.

---

### Finding 7: Missing Database Index
- **Location:** [`persistence/schema.rs:141`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/schema.rs#L141)
- **Spec Reference:** `db-spec.md §2.10`
- **Mechanism:**
  The specification mandates two indexes for `session_tool_calls`:
  1. `idx_tool_calls_session_turn ON session_tool_calls(session_id, turn_id)` (Present)
  2. `idx_tool_calls_created ON session_tool_calls(created_at DESC)` (Missing)
- **Consequence:**
  Schema version 5 migration omitted the chronological index required for auditing and multi-turn rollback operations.

---

## 4. Summary of Code vs. Spec Divergence

```
                                  SPEC CONTRACT                            CODE IMPLEMENTATION
                     ┌──────────────────────────────────────┐    ┌──────────────────────────────────────┐
Audio on Tool Call:  │ Drop prefix text; play filler only.  │ != │ Flushes prefix text as TurnResponse! │
                     ├──────────────────────────────────────┤    ├──────────────────────────────────────┤
Barge-In Handling:   │ Persist partial if non-empty text.   │ != │ Hardcoded rollback; partial dropped. │
                     ├──────────────────────────────────────┤    ├──────────────────────────────────────┤
Capability Probe:    │ Empirical test; verify JSON tool call│ != │ Substring check; embedded = true.    │
                     ├──────────────────────────────────────┤    ├──────────────────────────────────────┤
Prompt Assembly:     │ Owned by PromptBuilderStage.         │ != │ Implemented inside loop_driver.rs.   │
                     ├──────────────────────────────────────┤    ├──────────────────────────────────────┤
Tool Budget Check:   │ Re-evaluates utilization with pad.   │ != │ Blind to scratchpad tokens in loop.  │
                     ├──────────────────────────────────────┤    ├──────────────────────────────────────┤
Retrieval Gating:    │ Gated by memory settings.            │ != │ Hardcoded memory_retrieval = true.   │
                     └──────────────────────────────────────┘    └──────────────────────────────────────┘
```

---

## 5. Second-Pass Review (2026-09-21) — Corrections, New Findings & Ratified Decisions

> **Method:** Read-only line-by-line re-verification of all uncommitted Batch 0–7 files against `harness-spec.md`, `tools-spec.md`, `db-spec.md`, `events-spec.md`, `memory-spec.md`, `ipc-spec.md`. No cargo commands run per instruction.

### 5.1 Corrections to First-Pass Report

1. **Finding 3 substring claim overstated:** `catalog/probe.rs:554-560` guards with `status.is_success()` before `body.contains("tool_calls")`. An HTTP 400 error body never reaches the substring check — it returns `false`. The real probe defects remain: embedded `supports_tools:true` with zero engine support (`embedded/mod.rs:91-115` ignores `request.tools`), no `models_manifest.json` check (violates `tools-spec.md §3.1.3`), no persistent-negative vs transient distinction, `pipeline/assistant/session.rs:721` blocks router via `block_on` with no `session_starting` flag (violates `harness-spec.md Inv.16`).
2. **Finding 1 spec nuance:** `harness-spec.md §6 Case B` literally mandates NonTerminal prefix flush as `TurnResponse` before filler; Terminal discards. The first-pass "spec mandates drop" summary elides this split. User ratified **drop-all-prefix** (see §5.3), so the spec itself needs amendment, not just a code fix.
3. **Invariant 8 FRAGILE overstated:** `streaming/router.rs:328` (`Completed` path) and `orchestrator/tools.rs:45` (Terminal path) are mutually exclusive passes — no double-emit in a single turn. The real defect is timing/content (see N1 below), not duplication.

### 5.2 New Blocking Findings (missed in first pass)

- **N1 Terminal persistence divergence (`orchestrator/tools.rs:38-45`, `pipeline/assistant/llm.rs:71-74`):** Terminal dispatches `spoken_response` via a fresh `ClauseChunker` without touching `TurnAccumulator`, then emits `LlmFinished`. `on_llm_finished` persists accumulator text (stale prefix, often empty) — history gets `spoken_response`, DB `turns` gets prefix/nothing. Violates Inv.12 parity and `tools-spec.md §4.1`. Terminal also never dispatches its own `TurnCompleted`.
- **N2 Title gate uses global turn ID (`loop_driver.rs:187-191`, `stages/tools/registry.rs:51`):** `filter.turn_id == 1` tests the global monotonic pipeline counter (`PipelineAtomics::next_turn`, never reset), not the session-local first turn. Every session after the first never injects `respond_and_set_title`. Ratified fix: session-local gate (history has only system+user && `!title_set`).
- **N3 `search_memory` is a stub (`stages/tools/memory.rs:74-78`):** returns canned "No relevant historical records found". Ignores embeddings, cosine/lexical, RRF `k=60`, `top_k_facts`/`semantic_similarity_cutoff` (`core/settings.rs:805-806`), and personal-exclusion scope (`tools-spec.md §7.2`, `memory-spec.md §6.1`). Ratified as **blocking**, not deferred.
- **N4 Private-mode title leak (`stages/tools/title.rs:70-81`):** writes via `db.connect()` + `set_session_title` directly, bypassing `persistence/worker.rs:105-108` private-mode skip. Session row + title hit disk in incognito. Ratified: title must respect private mode.
- **N5 Invalid `trigger_kind` (`orchestrator/compaction.rs:63`):** writes `"inline"`; `db-spec.md §2.4` allows only `critical/soft/manual/auto/boot_auto`. Breaks ledger filters. Must be `critical`.
- **N6 Missing `has_played_filler`:** zero occurrences in `src/`; `harness-spec.md §6 Phase 3` requires at-most-once compaction filler. Compaction filler + tool filler can double-play.
- **N7 Provider matrix incomplete:** only `transport/chat_completions.rs` implements tools. `transport/ollama.rs:27-84,98-216` drops `request.tools`/`tool_calls`; `transport/responses.rs:26-96` drops tools and `functionCall`; `embedded/mod.rs:92-115` drops `request.tools`; `harness-spec.md §8.4` `<tool_call>` textual shim missing everywhere.
- **N8 Eager-streaming makes Terminal discard impossible (`streaming/router.rs:170-174`):** `handle_token` dispatches prefix clauses to TTS before `ToolCallReceived` is known. Deleting the remainder in `tools.rs` cannot recall already-sent audio. Requires buffering or job cancellation, not just a flush/discard branch.
- **N9 Cancellation ignored in tool handlers (`orchestrator/tools.rs:25-81,84-154`):** no `cancel` check before TTS dispatch/history commit; barge-in during the 10s tool window still returns `Completed`.
- **N10 Loop bound off-by-one (`loop_driver.rs:111,150-156,164-168`):** `while iteration <= 5` allows 6 LLM passes; final `tools=None` pass still honors a hallucinated tool call, then aborts with `Error` instead of forced text summary.
- **N11 Lenient malformed JSON (`transport/chat_completions.rs:82-88`):** unparseable args coerced to `{"raw":buf}` instead of strict hold/error (violates `tools-spec.md §2.3`).
- **N12 Empty-finish no rollback (`pipeline/assistant/llm.rs:75-83`):** empty accumulator skips persist but never rolls back the staged user turn (spec Phase 7 Branch C requires rollback + skip).
- **N13 Barge-in double-writer (`pipeline/assistant/interrupt.rs:52-72`):** sends `TurnCompleted` unconditionally (even empty) — violates `events-spec.md §5.4` single-writer (Harness `Cancelled` branch sole owner).
- **N14 Notification spec clash:** `tools-spec.md §3.1.4` says `category:"system"`; `notifications-spec.md §4.5` closed set has no `system`. Ratified: keep `session.rs:770-782` Pipeline/Warning/Interactive, amend `tools-spec`.
- **N15 Style/boundary:** `orchestrator/mod.rs:80-102` holds `execute_turn` business logic (violates `backend-style-guide.md §2` mod.rs zero-logic). `PersistenceEvent::ToolCallExecuted` uses `tool_flow: ToolFlow` / `arguments: Value` / non-optional `result`/`duration_ms` vs `db-spec.md §1.4` (`tool_kind: String`, `result: Option`, `duration_ms: Option`) — language-agnostic deviation to note. `persist_tool_call` does heal+insert without `BEGIN CONCURRENT` (single-statement OK, two-statement race noted).

### 5.3 DB-Spec & Invariant Sweep (residual check)

- **Persistence boundary (§1.1):** CLEAN in production code. All SQL lives in `persistence/`; `title.rs`/`tool_calls.rs` use the facade. `services/memory/ingestion/mod.rs:121-254` raw SQL hits are `#[cfg(test)]`-only fixtures.
- **Schema (§2):** tables, FK cascades (`session_tool_calls` ON DELETE CASCADE verified in test), self-heal `ensure_session_exists` all correct. Only gap remains the missing `idx_tool_calls_created` (Finding 7) plus N5 invalid enum value.
- **Harness Inv.1-18:** 1,3,5,6,7,9,10,11 honored. 2 broken (F4), 4 broken (F1+N8), 8 timing-broken (N1), 12 parity-broken for Terminal (N1), 13 broken (F2+N13), 14 honored (latch set/cleared on all terminal outcomes; minor: Terminal/Completed paths rely on `LlmFinished` event to clear `turn_open` — dropped `LlmFinished` in `Paused` leaves latch set until next turn self-heals), 15 honored except `title_set` divergence (N2), 16 broken (F3), 17 single-pass honored except N1 timing, 18 honored.

### 5.4 User-Ratified Decisions (grill-me round, 2026-09-21)

1. Prefix audio: **drop all prefix** — amend `harness-spec.md §6 Case B`, fix `tools.rs` + `router.rs` buffering.
2. Title gate: **session-local** (history length + `title_set`), drop global `turn_id==1`.
3. Memory stub: **blocking** — implement real hybrid retrieval per `tools-spec.md §7.2`.
4. Private mode: **title respects private** — gate direct DB write.
5. Notification: **keep Pipeline/Warning/Interactive** — amend `tools-spec.md §3.1.4` category.

### 5.5 Fix Order (by blast radius)

Barge-in single-writer → Terminal accumulator/DB parity → title gate → private-mode guard → prompt-stage ownership → budget re-entry + token accounting → probe non-blocking + manifest + persistent negative → provider matrix (ollama/responses/embedded/tag-shim) → filler gating + `trigger_kind` → loop bound/cancel/malformed-JSON → missing index → spec amendments.
