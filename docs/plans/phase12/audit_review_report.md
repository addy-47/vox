# System Architect Audit & Review Report: Phase 12.1 (Agentic Vox)

> **Document Type:** Formal System Architecture Review & Spec Fidelity Audit  
> **Auditor:** System Architect  
> **Target Audience:** Backend Engineer / Diff Agent  
> **Authority Specifications:**  
> - `docs/specs/harness-spec.md` (Approved Target Spec, amended 2026-09-21)  
> - `docs/specs/tools-spec.md` (Approved Behavioral & Interface Spec)  
> - `docs/specs/db-spec.md` (v2 Normalized Database Spec)  
> - `docs/specs/events-spec.md` (Pipeline State & Event Spec)  
> - `docs/specs/notifications-spec.md` (Notification Subsystem Spec)  
> **Target Codebase:** `app/src-tauri/src/services/harness/`, `services/llm/`, `pipeline/assistant/`, `persistence/` (Batches 0–7)

---

## 1. Executive Summary & Calibration

This report synthesizes the first-pass investigation, second-pass read-only verification, and the latest `harness-spec.md` amendments (§6 drop-all-prefix & buffering, §9.1 named-step orchestrator directory structure, §9.2 refactor contract).

**Calibration:** Production voice pipeline. Every finding is audited against the fixed pipeline stage order, zero-unpunctuated audio leakage, monotonic turn lifecycles, and database parity.

### Overall Assessment
The initial Batch 0–7 implementation successfully authored the core types, Turso DB v5 migration (`session_tool_calls`), self-healing foreign key checks, and the `turn_open`/`drained_while_open` synthesis guard latch. However, **the implementation suffers from critical behavioral divergences and boundary leaks**:
1. **Audio Hot Path Violations:** Eager streaming dispatches raw text prefix tokens to TTS before tool calls are known; and tool handlers flush remaining text as `TurnResponse` before interim filler plays.
2. **Invariant 13 Barge-In Data Loss:** Cancellation unconditionally rolls back user queries and drops partial assistant speech.
3. **Capability Probe Invalidation:** Embedded models hardcode `supports_tools: true` despite having zero engine support; remote probe accepts error bodies with `"tool_calls"`.
4. **Architectural Boundary Erosion:** `PromptBuilderStage` was bypassed in favor of ad-hoc assembly in `loop_driver.rs`.
5. **State & Parity Disconnect:** Monotonic global turn counters are misused for session-local first-turn gates; Terminal tool execution fails to write to `TurnAccumulator`, leaving database `turns` blank.

---

## 2. Invariant Verification Ledger (`harness-spec.md §1.3`)

| # | Invariant | Spec Mandate | Code Reality | Status |
|---|---|---|---|:---:|
| 1 & 2 | **Single Orchestrator & Stage Encapsulation** | Orchestrator mediates data flow. Stages have narrow typed interfaces. `PromptBuilderStage` owns `build_generation_request`. | `PromptBuilderStage` was bypassed. Ad-hoc request assembly and tool injection were implemented directly in `loop_driver.rs:172-209`. | ❌ **BROKEN** |
| 3 | **Exclusive State Ownership** | Zero shared mutable state across stages. | History, Budget, Prompt, and Compaction maintain isolated ownership. | ✅ **HONORED** |
| 4 | **Sacred Audio Hot Path & Intent Isolation** | Zero audio leakage on tool proposal. Only `spoken_filler` or `spoken_response` is synthesized. | `router.rs:170-174` eagerly sends prefix tokens to TTS. `tools.rs:91-101` flushes prefix text as `AudioIntent::TurnResponse` before filler. | ❌ **BROKEN** |
| 5 | **Domain-Pure Layer Placement** | Text in harness, audio in TTS/playback. | Chunker and normalizer are contained within harness/streaming. | ✅ **HONORED** |
| 6 | **Lock Discipline Across Await** | No Mutex/RwLock guards held across `.await` points. | Orchestrator locks are scoped to synchronous blocks before async dispatches. | ✅ **HONORED** |
| 7 | **Decoupled Stateless Model Worker** | `LlmActor` is a stateless compute engine accepting `GenerationRequest` and emitting normalized events. | `LlmActor` forwards `LlmStreamEvent::ToolCall` -> `LlmResponse::ToolCall`. | ✅ **HONORED** |
| 8 | **Turn Completion Authority** | `VoxEvent::LlmFinished` emitted once after terminal model pass. Never on cancelled/aborted passes. | `router.rs:328` and `tools.rs:45` are mutually exclusive passes, but Terminal tool timing/accumulator synchronization is broken. | ⚠️ **TIMING DEFECT** |
| 9 | **Single-Turn Cancellation Token Discipline** | `CancellationToken` captured by value per turn and threaded throughout. | Threaded through loop, tools, and provider. | ✅ **HONORED** |
| 10 | **Autonomous Reactive Quiet Watcher** | Debounced 20s watcher monitors pipeline state transitions and prunes history. | Retained in `chassis.rs`. | ✅ **HONORED** |
| 11 | **Pipeline Turn Accumulator Boundary** | `StreamRouter` writes speakable text into `TurnAccumulator`. | Eager stream writes to accumulator; Terminal tool fails to write `spoken_response` to accumulator. | ⚠️ **PARTIAL** |
| 12 | **Ephemeral Scratchpad & Memory Parity** | Scratchpad dropped at commit/cancel. Committed history and SQLite `turns` maintain 100% parity. | Scratchpad drops cleanly. However, Terminal tool commits `spoken_response` to memory while SQLite `turns` gets empty accumulator text. | ❌ **BROKEN** |
| 13 | **Single-Writer Barge-In Persistence** | Cancelled branch inspects `partial_text`: if non-empty, commits `(query, partial)` to history + persistence; if empty, rolls back. | `finalize.rs:8-20` unconditionally rolls back user turn. `loop_driver.rs:268` discards `partial_text` with `..`. | ❌ **BROKEN** |
| 14 | **Synthesis Guard Latch (`turn_open` / `drained_while_open`)** | Router sets `turn_open` at onset, clears on terminal outcomes. Latches `drained_while_open` on premature playback drain. | Implemented in `atomics.rs:170-205`, `playback.rs:74`, `llm.rs:68-87`. Reset on interrupt, pause, end, error. | ✅ **HONORED** |
| 15 | **Self-Healing Foreign Keys** | `INSERT OR IGNORE INTO sessions` before writing `session_tool_calls` or updating titles. | Handled via `ensure_session_exists` in `sessions.rs:305-325` and `tool_calls.rs:26`. | ✅ **HONORED** |
| 16 | **Non-Blocking Discovery Probe (`session_starting`)** | Discovery probe runs asynchronously off router thread under `session_starting = true`. Dictation never blocked. | `session.rs:721` uses `tokio_handle.block_on` synchronously on the caller thread. No `session_starting` flag. | ❌ **BROKEN** |
| 17 | **Terminal Tool Single-Pass Resolution** | `spoken_response` dispatched directly as `AudioIntent::TurnResponse` with zero 2-pass latency. | Dispatched in `tools.rs:30-44`. | ✅ **HONORED** |
| 18 | **Zero Backward Compatibility (ZBC)** | Clean redesign, zero transitional wrappers or compatibility shims. | Clean types across codebase. | ✅ **HONORED** |

---

## 3. Systematic Item-by-Item Review (/feedback-review)

### Category A: Audio Stream & Intent Integrity

#### Item A.1: Drop-All-Prefix & Stream Buffering Requirement
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  In [`streaming/router.rs:107-109, 170-174`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/streaming/router.rs#L107-L109), incoming `LlmResponse::Token` is immediately chunked and dispatched to `TtsActor` as `AudioIntent::TurnResponse`. If the model emits "Let me check that..." followed by a tool call, the prefix has already entered the TTS queue. In [`orchestrator/tools.rs:91-101`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/tools.rs#L91-L101), the router flushes the remainder as `AudioIntent::TurnResponse` before calling `enter_non_terminal_phase` (which sends `AudioIntent::InterimFiller`).
- **Why it's wrong:**
  Per ratified decision and amended `harness-spec.md §6 Case B`: **drop all prefix**. Text preceding a tool call is untrusted planner scratchpad output and must never be spoken. Streaming must buffer clauses until pass completion, or cancel/recall dispatched clauses upon tool interception.
- **Action for Backend Engineer:** Implement pass-level clause buffering in `streaming/router.rs` (or discard un-synthesized items); discard prefix text completely in `tools.rs` for both Terminal and NonTerminal tools.

#### Item A.2: Terminal Tool TurnAccumulator & DB Persistence Disconnect (N1)
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  In [`orchestrator/tools.rs:38-46`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/tools.rs#L38-L46), `handle_terminal_tool` extracts `spoken_response` and passes it to `ctx.stream_stage.dispatch_spoken_response` (which creates a local chunker and does NOT update `ctx.stream_handles.accumulator`). It then calls `ctx.stream_stage.emit_finished`, triggering `VoxEvent::LlmFinished`.
  In [`pipeline/assistant/llm.rs:71-74`](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/llm.rs#L71-L74), `on_llm_finished` reads `full_text` from `state.pipeline_accumulator.lock()`. Because the accumulator was never updated with `spoken_response`, `full_text` contains only stale prefix tokens or is empty! `persist_assistant_turn` writes empty/garbage text to the database `turns` table, while in-memory `history` gets `spoken_response` via `harness.history.push_assistant_turn(final_response)`.
- **Why it's wrong:**
  Directly violates Invariant 12 (100% parity between active working memory and database-restored turns) and `tools-spec.md §4.1`.
- **Action for Backend Engineer:** Update `TurnAccumulator.assistant_response` with `spoken_response` inside `handle_terminal_tool` before emitting `LlmFinished`.

---

### Category B: Lifecycle & Concurrency Invariants

#### Item B.1: Invariant 13 Barge-In Data Loss
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  [`orchestrator/finalize.rs:8-20`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/finalize.rs#L8-L20) unconditionally rolls back the user turn via `harness.history.rollback_last_user_turn()`. [`orchestrator/loop_driver.rs:268-270`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/loop_driver.rs#L268-L270) ignores `partial_text` with `..`.
- **Why it's wrong:**
  Violates Invariant 13 and `harness-spec.md §6 Phase 7 Branch B`. If the assistant spoke 5 seconds of audio before barge-in, `partial_text` must be committed to history as an assistant turn and persisted via `PersistenceEvent::TurnCompleted`. It should only roll back if `partial_text.trim().is_empty()`.
- **Action for Backend Engineer:** Thread `partial_text` into `handle_turn_cancelled`, check `!partial.trim().is_empty()`, commit to history, and emit `PersistenceEvent::TurnCompleted`.

#### Item B.2: Barge-In Double-Writer Collision (N13)
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  [`pipeline/assistant/interrupt.rs:52-72`](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/interrupt.rs#L52-L72) constructs and sends `PersistenceEvent::TurnCompleted` directly from the interrupt handler whenever an interruption occurs.
- **Why it's wrong:**
  Violates Invariant 13: the Harness `Cancelled` branch is the **sole persistence writer** for interrupted turns. Having `interrupt.rs` emit `TurnCompleted` creates a race condition and duplicate turn writes in SQLite.
- **Action for Backend Engineer:** Remove `PersistenceEvent::TurnCompleted` dispatch from `interrupt.rs`; delegate exclusively to the Harness finalizer.

#### Item B.3: Session-Local Title Gate vs Global Monotonic Counter (N2)
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  In [`orchestrator/loop_driver.rs:188`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/loop_driver.rs#L188) and [`stages/tools/registry.rs:51`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/tools/registry.rs#L51):
  ```rust
  if name == "respond_and_set_title" && !(filter.turn_id == 1 && filter.title_is_unset) {
      continue;
  }
  ```
  `filter.turn_id` is the monotonically increasing global turn counter minted by `PipelineAtomics::next_turn()` (which is never reset to 0 across the lifetime of the application process).
- **Why it's wrong:**
  In the user's very first session, Turn 1 has `turn_id == 1`. In every subsequent session, Turn 1 has `turn_id > 1`. As a result, **no session after the first ever injects the `respond_and_set_title` tool schema**, leaving all subsequent sessions titled "Untitled Session".
- **Action for Backend Engineer:** Change gate to session-local criteria: `filter.is_first_turn` derived from `harness.history.messages().len() <= 2` (only root system prompt and initial user turn present) along with `!harness.title_set`.

#### Item B.4: Invariant 16 Synchronous Capability Probe Blocking Router
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  [`pipeline/assistant/session.rs:721`](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/session.rs#L721) uses `tokio_handle.block_on(...)` synchronously on the caller thread to await `probe_capabilities`. No `session_starting` flag exists.
- **Why it's wrong:**
  Violates Invariant 16 (`harness-spec.md §1.3.16, §8.1`). The discovery probe must run asynchronously off the router thread, keeping state in `Idle` until resolution completes.
- **Action for Backend Engineer:** Decouple probe into non-blocking async task with `session_starting = true` state guarding `Ready` transition.

---

### Category C: Subsystem Architecture & Modularity

#### Item C.1: Architectural Bypass of `PromptBuilderStage`
- **Verdict:** ✅ Confirmed Bug / Boundary Leak
- **Confidence:** 100%
- **What the code actually does:**
  [`stages/prompt.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/prompt.rs) was untouched. A duplicate `build_generation_request` function was written in [`orchestrator/loop_driver.rs:172-209`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/loop_driver.rs#L172-L209).
- **Why it's wrong:**
  Violates `harness-spec.md §4.3` ("Payload Assembly Authority: `PromptBuilderStage` owns `build_generation_request`") and §9.2. Loop drivers must not perform ad-hoc message slicing or tool schema formatting.
- **Action for Backend Engineer:** Move `build_generation_request` to `PromptBuilderStage` in `stages/prompt.rs`. `orchestrator/assemble.rs` will delegate directly to it.

#### Item C.2: Reentrant Context Budgeting Blind to Scratchpad
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  [`orchestrator/loop_driver.rs:111-162`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/loop_driver.rs#L111-L162) iterates through tool passes without invoking `ContextBudgetStage::evaluate_utilization`.
- **Why it's wrong:**
  Violates `harness-spec.md §6 Phase 2 Step 4 & Loop Cycle Invariant`. When a NonTerminal tool returns thousands of tokens into `scratchpad`, remaining context capacity must be calculated before assembling the next generation pass.
- **Action for Backend Engineer:** Wire `ContextBudgetStage::calculate_tracked_tokens` to include scratchpad messages on every loop cycle.

#### Item C.3: Dead Memory Retrieval Gating
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  In [`orchestrator/loop_driver.rs:190`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/loop_driver.rs#L190), `memory_retrieval_enabled: true` is hardcoded.
- **Why it's wrong:**
  Violates user settings configuration (`settings.working_memory`).
- **Action for Backend Engineer:** Read `memory_retrieval_enabled` from active settings.

#### Item C.4: `search_memory` is a Stub (N3)
- **Verdict:** ✅ Confirmed Bug (Blocking per User Ratification)
- **Confidence:** 100%
- **What the code actually does:**
  [`stages/tools/memory.rs:74-78`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/tools/memory.rs#L74-L78) returns a canned stub string: `"Memory search completed for query '...'. No relevant historical records found."`.
- **Why it's wrong:**
  Per user ratification (§5.4), `search_memory` is blocking for Phase 12.1. It must execute vector embedding + full-text search with RRF fusion ($k=60$) per `tools-spec.md §7.2` and exclude `personal` tagged facts.
- **Action for Backend Engineer:** Implement hybrid episodic retrieval using existing embedding and database vector tables.

---

### Category D: Provider & Persistence Contracts

#### Item D.1: Provider Capability Probe Invalidation
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  1. [`probe.rs:242`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/catalog/probe.rs#L242) hardcodes `supports_tools: true` for embedded models, despite `embedded/mod.rs` having zero tool parser or event support.
  2. `probe.rs:554-560` checks `body.contains("\"tool_calls\"")`. (Note: First-pass report noted false positives on HTTP 400; while `status.is_success()` guards against HTTP 400, any HTTP 200 conversational reply mentioning "tool_calls" triggers a false positive).
  3. `probe.rs` never consults `models_manifest.json` for embedded GGUFs as required by `tools-spec.md §3.1.3`.
- **Action for Backend Engineer:** Check `models_manifest.json` for embedded models; parse actual tool calls in probe response rather than substring matching.

#### Item D.2: Private-Mode Title Persistence Leak (N4)
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  [`stages/tools/title.rs:70-81`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/stages/tools/title.rs#L70-L81) calls `db.connect()` and `set_session_title` directly, bypassing the `persistence/worker.rs` private-mode filter.
- **Why it's wrong:**
  In Incognito/Private Mode, title updates hit disk, leaking session information.
- **Action for Backend Engineer:** Check `app_state.is_private_mode` before persisting title to SQLite.

#### Item D.3: Invalid Compaction `trigger_kind` (N5)
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  [`orchestrator/compaction.rs:63`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/compaction.rs#L63) sets `trigger_kind: "inline"`.
- **Why it's wrong:**
  `db-spec.md §2.4` restricts `trigger_kind` to `'critical' | 'soft' | 'manual' | 'auto' | 'boot_auto'`. `"inline"` is invalid and violates DB constraints.
- **Action for Backend Engineer:** Change `"inline"` to `"critical"`.

#### Item D.4: Missing Database Index (Finding 7)
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  [`persistence/schema.rs:141`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/schema.rs#L141) creates `idx_tool_calls_session_turn`, but omits `idx_tool_calls_created`.
- **Action for Backend Engineer:** Add `CREATE INDEX IF NOT EXISTS idx_tool_calls_created ON session_tool_calls(created_at DESC);`.

#### Item D.5: Lenient Malformed JSON Handling (N11)
- **Verdict:** ✅ Confirmed Bug
- **Confidence:** 100%
- **What the code actually does:**
  [`transport/chat_completions.rs:82-88`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/transport/chat_completions.rs#L82-L88) wraps unparseable arguments in `{"raw": buffer}` and emits a canonical tool call.
- **Why it's wrong:**
  `tools-spec.md §2.3` mandates that tool calls are emitted only when arguments validate as well-formed JSON.
- **Action for Backend Engineer:** Reject invalid JSON arguments; emit a sanitized `ToolResult { is_error: true }` back to the model.

---

## 4. Architectural Refactor Blueprint (`harness-spec.md §9.1 & §9.2`)

To eliminate code duplication, enforce single loop ownership, and establish clean file boundaries (<500 lines per file, <50 lines per function), the diff agent must execute the **§9.2 pure-move refactor contract** before fixing individual behavioral findings:

```
services/harness/orchestrator/
├── mod.rs        # Pure module declarations and re-exports. Zero business logic.
├── chassis.rs    # Harness struct, constructors, session state. No turn sequencing.
├── loop.rs       # SOLE sequencing owner: execute_turn() + reentrant budget→assemble→dispatch→stream→tools loop.
├── intake.rs     # Phase 1 & 2: Dedup gate, push_user_turn, initial budget check (runs ONCE per turn).
├── compaction.rs # Phase 3: Inline compaction maintenance (runs ONCE per turn, before loop).
├── assemble.rs   # Phase 4: Delegates to PromptBuilderStage::build_generation_request (sole assembly authority).
├── dispatch.rs   # Phase 5: Duplex LlmCommand::Generate dispatch over session pipe.
├── stream.rs     # Phase 6: Streaming adapter & outcome demuxing (Completed / ToolCall / Cancelled / Error).
├── tools.rs      # Phase 6 Case B: Terminal dispatch vs NonTerminal filler + scratchpad append.
├── phase.rs      # NonTerminalPhase trigger & enter_non_terminal_phase helper (Working + InterimFiller).
└── finalize.rs   # Phase 7: Single-writer commit / cancelled-partial / error branches.
```

---

## 5. Sequenced Remediation Plan for Backend Engineer

Follow this exact dependency order to avoid merge conflicts and compiler breaks:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ STEP 1: Pure-Move Orchestrator Refactor (§9.1 & §9.2)                       │
│ - Decompose into named steps + loop.rs                                      │
│ - Move build_generation_request to PromptBuilderStage                       │
│ - mod.rs to zero logic; cargo check verifies zero behavior change           │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
┌──────────────────────────────────────▼──────────────────────────────────────┐
│ STEP 2: Audio & Streaming Hot-Path Fixes                                    │
│ - Implement clause buffering in streaming/router.rs                         │
│ - Drop prefix text unconditionally on ToolCallReceived (tools.rs)           │
│ - Route spoken_response through TurnAccumulator before emit_finished (N1)   │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
┌──────────────────────────────────────▼──────────────────────────────────────┐
│ STEP 3: Invariant 13 & Barge-In Persistence                                 │
│ - Thread partial_text into finalize.rs; commit non-empty partials           │
│ - Remove duplicate TurnCompleted persistence write from interrupt.rs (N13)  │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
┌──────────────────────────────────────▼──────────────────────────────────────┐
│ STEP 4: Tool Gating & Budget Lifecycle                                      │
│ - Switch title gate from global turn_id==1 to session-local first-turn (N2) │
│ - Include scratchpad tokens in reentrant ContextBudgetStage evaluations     │
│ - Connect settings to ToolFilter.memory_retrieval_enabled                   │
│ - Fix compaction trigger_kind to "critical" (N5)                            │
│ - Add has_played_filler guard for compaction filler                         │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
┌──────────────────────────────────────▼──────────────────────────────────────┐
│ STEP 5: Retrieval Implementation & Persistence Hardening                    │
│ - Replace search_memory stub with hybrid RRF retrieval (N3)                 │
│ - Guard title persistence against Private Mode in title.rs (N4)             │
│ - Add idx_tool_calls_created index in schema.rs                             │
│ - Enforce strict JSON validation in chat_completions.rs (N11)               │
│ - Correct embedded capability probe to check models_manifest.json           │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Verification Checklist

Upon completion of remediation:
- [ ] `cargo check` and `cargo clippy --all-targets` pass with 0 errors and 0 warnings.
- [ ] Invariant 13: Simulated barge-in commits partial text to SQLite `turns` and history.
- [ ] Invariant 12: Terminal tool `respond_and_set_title` writes identical `spoken_response` to SQLite `turns` and working memory.
- [ ] Audio Stream: Turn 1 with tool call plays **only** filler or `spoken_response` — zero prefix text heard.
- [ ] Turn 2+: `respond_and_set_title` is omitted from schemas across all sessions, not just the first session.
- [ ] Full nextest suite runs single-threaded release mode without regression:
  ```bash
  RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
  ```

---

## 7. Addendum — Residual Gaps From Approval Review (2026-09-22)

> Second-pass items confirmed against live code but absent from §§2–6. Backend engineer must treat these as blocking alongside the Step 1→5 plan. No prior section is altered.

### G.1 Provider Transport Matrix Incomplete (blocking)

- **What the code actually does:** Only [`transport/chat_completions.rs:232`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/transport/chat_completions.rs#L232) reads `request.tools`. `transport/ollama.rs` (`build_request_body`, `stream_ollama`), `transport/responses.rs` (`build_request_body`, `stream_responses`), and `embedded/mod.rs:92-115` ignore `request.tools` entirely — schemas are silently dropped and no `LlmStreamEvent::ToolCall` is ever produced on those paths. The `harness-spec.md §8.4` textual `<tool_call>` ingress shim exists nowhere.
- **Why it matters:** Ollama, Responses-API, and local GGUF sessions run text-only while `supports_tools` may report `true`, with zero user-visible signal. Batch 7 ("provider normalization complete") is true only for OpenAI-compatible endpoints.
- **Action:** Either implement tool egress + ingress normalization per transport per the `tools-spec.md §2.2` matrix, or formally defer non-OpenAI tool support in the spec (force `supports_tools=false` on those paths with the §8.1 degraded notification). Do not leave silent drop.

### G.2 Tool-Path Cancellation Ignored (blocking)

- **What the code actually does:** [`orchestrator/tools.rs:25-81,84-154`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/tools.rs#L25-L81) never checks the turn `CancellationToken` before TTS dispatch or history commit. `run_with_guards` converts cancellation into an error `ToolResult`, but both handlers still dispatch audio and return `Completed`.
- **Why it matters:** Barge-in during the 10s tool window still speaks and commits a turn the user interrupted, violating Phase 7 Branch B.
- **Action:** Check cancellation after `execute_tool` returns in both handlers; on cancelled, discard audio/history side effects and return the `Cancelled` outcome path.

### G.3 Reentrant Loop Off-By-One + Final-Pass Handling

- **What the code actually does:** [`orchestrator/loop_driver.rs:111`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/harness/orchestrator/loop_driver.rs#L111) loops `while iteration <= MAX_TOOL_ITERATIONS` (6 LLM passes, not 5). The `tools=None` fallback pass in `handle_pass_outcome` still honors a hallucinated `ToolCallReceived`, then falls through to the `Exceeded max tool iterations` error abort.
- **Action:** Bound the loop to exactly 5 tool iterations + 1 final tool-free summarization pass; on the final pass, discard any tool call and commit text only, per `harness-spec.md §6 Case B` (note: §10.2 "hard fallback error" wording is superseded by §6 — spec editors to reconcile).

### G.4 Empty-Finish Rollback Missing

- **What the code actually does:** [`pipeline/assistant/llm.rs:75-83`](file:///home/addy/projects/apps/vox/app/src-tauri/src/pipeline/assistant/llm.rs#L75-L83) skips `TurnCompleted` persistence when the accumulator is empty but never rolls back the staged user turn from `stages/history.rs`.
- **Why it matters:** Phase 7 Branch C requires rollback + skip on empty; without it, a phantom user turn lingers in working memory and diverges from DB-restored sessions.
- **Action:** Roll back the staged user turn on the empty path.

### G.5 Notification Contract Clash (spec amendment)

- **Conflict:** `tools-spec.md §3.1.4` mandates `category: "system"` for the tool-unsupported warning; `notifications-spec.md §4.5` closed category set contains no `system`.
- **Ratified decision (2026-09-21):** Keep the implemented `session.rs:770-782` contract (Pipeline / Warning / Interactive Navigate to `settings/ai`, `group_key: "model_tool_unsupported"`); amend `tools-spec.md §3.1.4` to match instead of changing code.

### G.6 Budget & Private-Mode Precision Corrections

- **C.2 scope:** `ContextBudgetStage::calculate_tracked_tokens` must include **registered tool schemas** in addition to the scratchpad — the Phase 2 Step 4 formula counts system prompt, memory, summary, history, tool schemas, and scratchpad.
- **D.2 path:** the private-mode flag lives at `app_state.telemetry.is_private_mode` (`core/state.rs:162`, wired in `core/engine.rs:77`), not `app_state.is_private_mode`. Title tool (`stages/tools/title.rs:70-81`) must gate its direct `set_session_title` write on this flag per the ratified private-mode decision.



### Review: Dead Code Audit v2 (`audit_dead_code_v2.py` output)

Script raw numbers: clippy **0** · heuristic Rust **53** · knip **76 exports / 2 files / 82 types**. Every claim below was re-verified against source (module-specific import resolution, file-text analysis, backend grep), not taken from the script.

---

## Backend (Rust)

#### 50 × `test_*` functions (heuristic hits==1)
**Verdict:** ❌ False Positive (tool limitation) · **Confidence:** 100%
**What the code does:** All 50 carry `#[test]` / `#[tokio::test]`; the harness invokes them by symbol, so `rg` total==1 is expected for every unit test in the codebase.
**Why it's this way:** The hits==1 heuristic cannot see test-harness dispatch. clippy `-W dead_code` = 0 confirms.

#### `webview_created` — `window_customizer.rs:13`
**Verdict:** ❌ False Positive (framework trait dispatch) · **Confidence:** 100%
**What the code does:** `impl<R: Runtime> Plugin<R> for PinchZoomDisablePlugin`; Tauri calls it when each webview is created. Plugin is registered at `lib.rs:179` (`.plugin(window_customizer::PinchZoomDisablePlugin)`). Zero textual call sites is correct and irrelevant.

#### `update_personal_memory` — `services/harness/chassis.rs:178`
**Verdict:** ✅ **Confirmed dead** · **Confidence:** 95%
**What the code does:** `pub fn` wrapper → `self.prompt.set_personal_memory(...)`. Zero callers anywhere in the repo. Live personal-memory injection happens at prompt-stage construction (`chassis.rs:71`, `chassis.rs:120`) and session load (`persistence/sessions.rs:376`), never through this method. Clippy misses it because it's `pub` in a lib crate.
**Why it's this way:** Looks like a leftover mid-session-update API superseded by construction-time injection — no doc/comment confirms, but the two live call patterns are unambiguous.

#### `run_ingestion_cycle_with_embedder` — `services/memory/ingestion/mod.rs:61`
**Verdict:** ✅ **Confirmed dead** · **Confidence:** 98%
**What the code does:** Doc says "injected embedding function for deterministic testing" — but **no test or prod code calls it**. The real path `run_ingestion_cycle` is used (`services/memory/mod.rs:104`, `tests/memory_ingestion_test.rs:316`). The lower-level `run_stage2_cosine_dedup_with_embedder` *is* used by its own test (`stage2_embed.rs:290`) — only this cycle-level wrapper was never wired.

**Backend true-dead total: 2 functions.**

---

## Frontend

### A. True-dead symbols — safe to delete entirely (zero refs besides definition/barrel passthrough)

**Services / hooks / data (19):**
| Symbol | File |
|---|---|
| `cancelModelSetup` | `setupService.ts:93` (wizard has no cancel path) |
| `completeSetupWizard` | `setupService.ts:98` — **duplicate**; live copy is `settingsService.ts:155` (used by `CompletedStep`) |
| `showMainWindow` | `windowService.ts:6` |
| `checkSttProviderHealth` | `settingsService.ts:85` (LLM/TTS siblings used, STT orphaned) |
| `validateLlmTokenCap` | `settingsService.ts:132` (superseded by `probeModelCapabilities`) |
| `renameProject`, `deleteProject` | `projectService.ts:31,39` (`getProjects`/`createProject` used) |
| `onPersonalMemoryUpdated` | `eventsService.ts:263` — backend event variant exists (`events.rs:226`), never subscribed |
| `getStackIds` | `overlayStack.ts:76` (`getStackSize` used by `ResponsiveLayout`) |
| `SPATIAL_CONTAINERS` | `spatialNavigation.ts:7` (rest of module live) |
| `normalizeToInteractionModeLower` | `interactionMode.ts:26` (Upper variant used) |
| `MEMORY_CONFIG_DESK_COPY`, `HISTORY_SETTINGS_COPY` | `settingsCopy.ts:383,595` — **dead aliases** of `PERSONAL_MEMORY_*` / `WORKING_MEMORY_*` |
| `getShortcutsForRoute`, `shortcutSuffix` | `shortcuts.ts:96,102` |
| `getLatestSnapshot` | `useRuntimeSnapshot.ts:84` (hook itself used) |
| `OpenAiLogo`, `ElevenLabsLogo`, `CLOUD_PROVIDER_HOSTS` | `providersCopy.tsx` — see Ambiguous below |

**Dead components (2):**
- **`Badge` + `BadgeProps`** — `shared/ui/Badge.tsx` — **no JSX usage anywhere** (all `\bBadge\b` hits are comments/`TurnMetricsBadge`). Delete file + `shared/ui/index.ts:3` barrel line. *TurnMetricsBadge is separate and live.*
- **`ViewSelector` + `ViewSelectorProps`** — `history/ViewSelector.tsx:12` — never rendered; only `HistoryView` type is imported (`CentralClockNode.tsx:5`) and must stay. Barrel `history/index.ts:7` line dead.

**Dead orbit-memory helpers (zero in-file uses):** `ORBIT_Z_BACK_MAX`, `ORBIT_Z_CLOCK`, `ORBIT_Z_FRONT_MIN`, `ORBIT_GUIDE_OPACITY`, `ellipseAngleFromFraction`, `daysInMonthKey`, `timeToDialAngle`, `dayToDialAngle`, `dialDegrees`, `dialDotRadius`, `formatDuration`, `formatDayShortLabel` (all `orbitMath.ts`); `getThemeCollectionColors` (`memoryGraphTypes.ts:70`).

**Fully dead types (6):** `InteractionOwner`@`pipelineService`, `ContinueSessionResult`@`historyService:35` (**duplicate** of live `pipelineService:35`), `ModelMetadata`@`settingsStore:118`, `LocalSnapshot`@`monitoringService:45`, `EdgeTtsVoiceDto`@`voiceService:11`, `HelpTier`@`helpCopy.ts:25` (not even used in-file).

**Dead files (2):**
- `src/shared/hooks/useConversationList.ts` — 0 importers; its job lives in `useSessionPanel` / `ActiveSessionHeader` / `MemorySessionRail` (all call `getSessions`/`sortSessionsNewestFirst`/`onSessionsChanged` directly).
- `src/shared/components/help/index.ts` — dead barrel; children are imported directly / `lazy()`-loaded by `HelpPanel.tsx`.

### B. Dead re-export lines (symbol alive elsewhere — remove the line only)

- `historyService.ts:53` — `export { createSession, continueSession } from "./pipelineService"` — comment itself says "moved to pipelineService"; `VoiceSessionContext` imports from pipelineService (`:20-21`).
- `pipelineService.ts` "Backward Compatibility" block (§249): dead members = `listVoices`, `setupRemoteServer`, `type LocalSnapshot`, `type VoiceEntryDto`, `type EdgeTtsVoiceDto`, `type RemoteServerConfig` (+ their source-side twins flagged where no other importer exists). **Note:** `renameVoice`/`addVoiceFromFile`/etc. in the same block *are* imported via pipelineService — partial block only. This block is a ZBC violation by construction.
- `useHomePage.ts:20` — `export { toMood }` (the other two re-exports are used).
- `MemoryGraph.tsx:26` — `export { getCollectionColor, getCollectionIcon }`; consumers import from `memoryGraphTypes` directly (`MemoryNodeTooltip`, `MemorySessionRail`). `getCollectionIcon` import at `:21` is also body-unused.
- `SettingsContext.tsx:5` — re-exports of `ModelMetadata` + `VoiceProfile` have zero importers.
- `memory/index.ts` star-orphans: `ClusterBadgeData` has no importer at all → full chain dead (counts in A's type story).

### C. Export-keyword-only noise (symbol used in-file; NOT dead code — 39 exports + 71 types)

Knip's largest bucket. Examples: `checkUpdates` (used by `checkForUpdates`), `startSession`/`endSession`/`isVoxIpcError` (used inside `pipelineService`), all `ORBIT_RADIUS_*`/`formatDayLabel`-style orbitMath internals, `PANEL_EDGE_MAP`, `selectRolledUpNotifications`, `closeTopmost`, `KNOWN_CATEGORIES`, `SHORTCUTS`, `VOICE_INFO`, `REALTIME_SUBKEY_MAP`, `fuzzyMatch`, `listAudioDevices`, `GeminiLogo`/`DeepgramLogo` (in-file in `REALTIME_PROVIDERS`), `on` (workhorse of `eventsService`, called at `:209+`), and ~71 types (the entire `settingsStore` settings-shape cluster composing `VoxSettings`, `BadgeProps`-style props types, etc.). **Action if desired: drop the `export` keyword — do not delete the symbols.**

---

### Missed by the script (cross-boundary findings)

1. **Orphaned backend IPC handlers** once the dead FE wrappers go: `rename_project`, `delete_project` (`ipc/projects.rs:49,71`), `show_main_window`, `manage_models` action `"cancel"` — FE is their only client.
2. **`PersonalMemoryUpdated` event**: backend maps the name; no FE listener exists (dead `onPersonalMemoryUpdated`). Whether the backend ever *emits* it wasn't fully traced.
3. **Knip has no config file** (`knip.json` absent, no `knip` key in `package.json`) — defaults only; results happened to hold up under manual verification, but worth pinning entry patterns.

### Ambiguous — needs your call

- **`OpenAiLogo` / `ElevenLabsLogo` (and empty slots in `REALTIME_PROVIDERS`):** backend `RealtimeSettings` defines all 4 providers (`gemini_live`, `openai_realtime`, `deepgram_voice_agent`, `elevenlabs_convai`); FE carousel only lists gemini + deepgram. Dead code, or half-wired feature awaiting two `REALTIME_PROVIDERS` rows? **Cannot verdict without you.**

---

### Summary

**Confirmed dead (backend):** 2 — `chassis.rs::update_personal_memory`, `ingestion/mod.rs::run_ingestion_cycle_with_embedder`.
**Confirmed dead (frontend):** ~37 value symbols (19 services/hooks/data + 16 orbit helpers + 2 components incl. props), 6 types, 2 full files, plus ~6 dead re-export lines/partial blocks.
**False positives:** 51 Rust (50 tests + `webview_created`) — remove from any cleanup plan. Of knip's 82 types, ~71 are export-keyword noise, not dead code.
**Partial:** pipelineService compat re-export block (some members live, some dead); `MemoryGraph:26` line dead while both symbols live in `memoryGraphTypes`.
**Missed:** 4+ orphaned backend IPC commands/events; missing knip config.
**Ambiguous:** OpenAI/ElevenLabs realtime logos — delete or finish wiring?

**Overall assessment:** The script is directionally right on the frontend (knip) and badly wrong on the Rust heuristic (53 → **2** real). Manual verification was necessary and changed the verdict on 51/53 backend items and cleanly split knip's 76/82 into dead vs. export-keyword noise. True cleanup scope is modest and surgical; the only judgment call left is the OpenAI/ElevenLabs realtime pair. No implementation performed — awaiting your instruction on each bucket (delete / drop-export / wire / leave).