# Domain 1 Implementation Plan: 3-Stream Demuxing, Preamble Preservation & Two-Key Latch

---

## 1. Target End-State

Vox transitions from its legacy v1 streaming pipeline to the v2 Cognitive Turn Architecture:
1. **3-Stream Demuxing:** The streaming router demuxes LLM responses into three isolated streams in a single pass:
   - `Thinking`: Quarantined in backend memory buffer and telemetry; never sent to TTS; zero unthrottled IPC events (complying with `events-spec.md RULE-EVT-02`); excluded from persistent conversation history.
   - `Token`: Eagerly chunked into speakable clauses with sub-800ms time-to-first-audio (TTFA).
   - `ToolCall`: Accumulated across the full generation pass to assemble complete multi-tool batches.
2. **Preamble Speech Preservation:** Spoken tokens emitted prior to tool calls (e.g. *"Let me search the files for that"*) are synthesized and committed to history as the assistant message initiating the tools. Chunker clearing (`acc.chunker.clear()`) is eliminated.
3. **Universal Tool Interface (ZBC):** `ToolFlow` (`Terminal` vs `NonTerminal`) and synthetic speech parameters (`spoken_filler`, `spoken_response`) are eliminated. Tools adhere to a single canonical contract returning `ToolResult`. `respond_and_set_title` is replaced by `set_title`.
4. **Two-Key Quiescence Latch:** Mid-stream clauses in `allow_tools == true` steps are dispatched as `AudioIntent::InterimFiller`. `turn_open` remains `true` across intermediate steps (`0..N`) and only clears upon `LlmFinished`. Transition from `Speaking` to `Ready` occurs if and only if BOTH `turn_open == false` AND output audio has completely drained (`pending_synthesis_jobs == 0` and sound card buffer underrun), regardless of whether draining chunks carried `InterimFiller`.
5. **Database Schema Cleanup:** Column `tool_kind` is cleanly removed from `session_tool_calls` via Turso migration v16.

---

## 2. Resolved Architectural Decisions (Clarification Ledger)

1. **Audio Intent Tagging During Streaming:**
   - *Question:* How can `router.rs` tag clauses when it does not know at $t=140\text{ms}$ whether tools will be proposed at $t=800\text{ms}$?
   - *Resolution:* All clauses in `allow_tools == true` steps are dispatched as `AudioIntent::InterimFiller`. If `allow_tools == false`, clauses are dispatched as `AudioIntent::TurnResponse`.
2. **0-Tool Turn Playback Transition to Ready:**
   - *Question:* If all clauses in a 0-tool turn were dispatched as `InterimFiller`, how does `playback.rs` transition to `Ready` without getting stuck?
   - *Resolution:* When `LlmFinished` arrives with 0 tools, `loop.rs` sets `turn_open = false` (**Key 1**) and enters `Speaking`. `on_playback_finished` checks `!state.pipeline.is_turn_open() && pending_synthesis_jobs == 0` (**Key 2**) **regardless of the chunk's `AudioIntent` tag**, transitioning deterministically to `Ready`.
3. **Thinking Token Telemetry vs. IPC:**
   - *Question:* Should thinking tokens emit `IpcEvent::LlmThinking` to the UI?
   - *Resolution:* No. `events-spec.md RULE-EVT-02` bans unthrottled streaming IPC. Thinking tokens are quarantined in backend memory buffer and log telemetry only.
4. **Database Migration Strategy:**
   - *Question:* Do we need a temporary table `session_tool_calls_v2` to drop `tool_kind`?
   - *Resolution:* No. Turso/libSQL (SQLite 3.35+) supports direct `ALTER TABLE session_tool_calls DROP COLUMN tool_kind;`. Verified in local Turso CLI.

---

## 3. Blast Radius and Dependency Clustering

The implementation is structured into 5 sequential atomic batches based on shared blast radius.

```
┌────────────────────────────────────────────────────────┐
│ Batch 1: Core Types, Enums & Database Schema (ZBC)     │
│ (provider.rs, actor.rs, schema.rs, tool_calls.rs)      │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Batch 2: Network Ingress Provider Stream Adapters      │
│ (chat_completions.rs, ollama.rs, sidecar.rs)           │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Batch 3: Universal Tool Cleanup & SetTitle Migration   │
│ (search_memory.rs, web_search.rs, set_title.rs)        │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Batch 4: Router Demuxing & Preamble Preservation       │
│ (router.rs, steps.rs)                                  │
└───────────────────────────┬────────────────────────────┘
                            │
                            ▼
┌────────────────────────────────────────────────────────┐
│ Batch 5: Two-Key Quiescence Latch & Playback Authority │
│ (loop.rs, playback.rs, llm.rs)                         │
└────────────────────────────────────────────────────────┘
```

---

## 4. Execution Batches

### Batch 1: Core Types, Enums & Database Schema (ZBC)
- **Scope & Files:**
  - `app/src-tauri/src/services/llm/provider.rs`:
    - Add `Thinking(String)` variant to `LlmStreamEvent`.
    - Remove `tool_flow` from `CanonicalToolDefinition`.
    - Delete `ToolFlow` enum (`Terminal`, `NonTerminal`).
  - `app/src-tauri/src/services/llm/actor.rs`:
    - Remove all references to `ToolFlow`.
  - `app/src-tauri/src/persistence/schema.rs`:
    - Bump `SCHEMA_VERSION` from 15 to 16.
    - Remove `tool_kind TEXT NOT NULL` from `session_tool_calls` table DDL.
    - Add migration block for `current_version < 16`: `ALTER TABLE session_tool_calls DROP COLUMN tool_kind;`.
  - `app/src-tauri/src/persistence/tool_calls.rs`:
    - Remove `tool_kind` parameter from `persist_tool_call` and SQL `INSERT`.
- **Dependencies:** None (starts immediately).
- **Build Health:** Mid-batch red expected until tool call sites in Batch 3 are updated, or Batch 1 can update signatures with empty placeholders to keep compile gate moving.
- **Done Criteria:** Structs and enums cleanly compile without `ToolFlow`; Turso schema test passes.

### Batch 2: Network Ingress Provider Stream Adapters
- **Scope & Files:**
  - `app/src-tauri/src/services/llm/providers/chat_completions.rs`:
    - Inspect streaming chunks for `reasoning_content` or `thought` fields.
    - Yield `LlmStreamEvent::Thinking(text)` when reasoning is present; yield `LlmStreamEvent::Token(text)` for normal content.
  - `app/src-tauri/src/services/llm/providers/ollama.rs`:
    - Parse thinking fields from Ollama streaming chunks (e.g. DeepSeek R1).
  - `app/src-tauri/src/services/sidecar/mod.rs` (or provider sidecar adapter):
    - Strip inline `<think>...</think>` tags using a state-machine parser to emit `Thinking` vs `Token`.
- **Dependencies:** Batch 1 (`LlmStreamEvent::Thinking`).
- **Build Health:** Green upon completion of Batch 2.
- **Done Criteria:** Provider streaming unit tests verify reasoning tokens yield `Thinking` variant and content tokens yield `Token` variant.

### Batch 3: Universal Tool Cleanup & SetTitle Migration
- **Scope & Files:**
  - `app/src-tauri/src/services/harness/stages/tools/search_memory.rs`:
    - Remove `spoken_filler` from JSON schema and parameter parsing.
    - Remove `ToolFlow` declaration.
  - `app/src-tauri/src/services/harness/stages/tools/web_search.rs`:
    - Remove `spoken_filler` from JSON schema and parameter parsing.
    - Remove `ToolFlow` declaration.
  - `app/src-tauri/src/services/harness/stages/tools/set_title.rs`:
    - Author clean `SetTitleTool` implementing `ToolDefinition`.
    - JSON schema accepts only `{"title": "..."}`.
  - `app/src-tauri/src/services/harness/stages/tools/respond_and_set_title.rs`:
    - Delete file entirely (ZBC).
  - `app/src-tauri/src/services/harness/stages/tools/mod.rs`:
    - Export `set_title::SetTitleTool`; remove `respond_and_set_title`.
    - Remove `flow` / `ToolFlow` from `ToolDefinition` trait.
- **Dependencies:** Batch 1 (`ToolFlow` removal).
- **Build Health:** Holds green compile gate with Batch 1 complete.
- **Done Criteria:** All built-in tools implement universal interface with zero speech parameters.

### Batch 4: Router Demuxing & Preamble Preservation
- **Scope & Files:**
  - `app/src-tauri/src/services/harness/stages/streaming/router.rs`:
    - Demux incoming `LlmStreamEvent`:
      - `Thinking(text)` $\to$ accumulate in `thinking_buffer` (isolated from chunker and TTS).
      - `Token(text)` $\to$ push to `ClauseChunker` and emit `IpcEvent::LlmToken`.
      - `ToolCall(call)` $\to$ append to `tool_calls` batch accumulator; do NOT abort stream loop.
    - Tag clauses pushed to TTS during streaming with `AudioIntent::InterimFiller` if `allow_tools == true`, or `AudioIntent::TurnResponse` if `allow_tools == false`.
    - Eliminate `acc.chunker.clear()` on tool calls.
    - Return `StreamPassOutcome::ToolCallsReceived { preamble_text, thinking_text, calls }` or `StreamPassOutcome::Completed { assistant_text, thinking_text }`.
  - `app/src-tauri/src/services/harness/stages/orchestration/steps.rs`:
    - Handle `StreamPassOutcome::ToolCallsReceived`: preserve `preamble_text` in history; transition state to `Working` with `ActivityEnvelope`.
    - Remove 5-step limit; enforce `MAX_TOOL_STEPS = 20`.
- **Dependencies:** Batches 1, 2, 3.
- **Build Health:** Holds green compile gate.
- **Done Criteria:** Preamble tokens are preserved; multi-tool batches accumulate until `Finished`.

### Batch 5: Two-Key Quiescence Latch & State Authority
- **Scope & Files:**
  - `app/src-tauri/src/services/harness/loop.rs`:
    - Harness coordinator exclusively drives `InteractionState` transitions.
    - Set `turn_open = false` upon cognitive turn completion and emit `VoxEvent::LlmFinished`.
  - `app/src-tauri/src/pipeline/assistant/llm.rs`:
    - In `on_llm_finished`: set `turn_open = false` (**Key 1**). If audio is playing or pending jobs > 0, transition to `Speaking`. If already drained, transition to `Ready`.
  - `app/src-tauri/src/pipeline/assistant/playback.rs`:
    - In `on_playback_started`: if `turn_open == true` and intent is `InterimFiller`, keep state in `Thinking`/`Working`. If `turn_open == false` (or intent is `TurnResponse`), transition to `Speaking`.
    - In `on_playback_finished`: if `turn_open == true`, return early. If `turn_open == false` and `pending_synthesis_jobs == 0`, transition to `Ready` (**Key 2**), regardless of the chunk's `AudioIntent` tag.
- **Dependencies:** Batch 4.
- **Build Health:** Green across `./scripts/verify-targets.sh`.
- **Done Criteria:** Pipeline transitions cleanly through `Thinking` $\to$ `Working` $\to$ `Speaking` $\to$ `Ready` on 0-tool and multi-tool turns.

### Batch 6: Production Bug Remediation & Test Suite Overhaul (Lead Audit Alignment)

This batch resolves the P0 production defects and the 5 testing violations flagged in the Lead Architect's audit.

#### Part A: P0 Production Bug Fixes (STATUS: COMPLETED & VERIFIED in commit `c111a988d`)

1. **Restore Migration Logic & Table Rebuild (`app/src-tauri/src/persistence/schema.rs`):**
   - Restored `retire_positional_personal_memory_model()` (70 lines of v9 migration code) and `const PERSONAL_MEMORY_SEMANTIC_SCHEMA_VERSION: u32 = 9`.
   - Restored error propagation `?` in `ensure_voice_metadata_columns()`.
   - Rebuilt `session_tool_calls` cleanly under `PRAGMA foreign_keys = OFF;`:
     ```sql
     CREATE TABLE IF NOT EXISTS session_tool_calls_v2 (
         id TEXT PRIMARY KEY,
         session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
         turn_id INTEGER NOT NULL,
         tool_name TEXT NOT NULL,
         arguments TEXT NOT NULL,
         result TEXT,
         is_error INTEGER NOT NULL DEFAULT 0,
         duration_ms INTEGER,
         created_at INTEGER NOT NULL
     );
     INSERT INTO session_tool_calls_v2 (id, session_id, turn_id, tool_name, arguments, result, is_error, duration_ms, created_at)
     SELECT id, session_id, turn_id, tool_name, arguments, result, is_error, duration_ms, created_at FROM session_tool_calls;
     DROP TABLE session_tool_calls;
     ALTER TABLE session_tool_calls_v2 RENAME TO session_tool_calls;
     CREATE INDEX IF NOT EXISTS idx_tool_calls_session_turn ON session_tool_calls(session_id, turn_id);
     CREATE INDEX IF NOT EXISTS idx_tool_calls_created ON session_tool_calls(created_at DESC);
     ```
     Re-enabled `PRAGMA foreign_keys = ON;` and propagated all errors with `?`.
   - Replaced residual `"respond_and_set_title"` strings in `database_persistence_boundary_test.rs` with `"set_title"`.

2. **Single-Writer Router Authority for `turn_open` (`loop.rs` & `llm.rs`):**
   - Removed all off-router `ctx.req.app_state.pipeline.clear_turn_open()` calls from the Tokio task in `loop.rs:294, 307, 311, 324, 336`.
   - Maintained `events-spec.md RULE-EVT-01`: `vox-router` is the sole writer of pipeline states and turn lifecycles.
   - Replaced `state.engine.try_lock().unwrap_or(false)` in `llm.rs:110` with `state.engine.blocking_lock()`.

3. **Fix 6-Second `Thinking` Delay on 0-Tool Turns (`playback.rs`):**
   - Updated `on_playback_started`:
     - If `current_state == InteractionState::Working && intent == AudioIntent::InterimFiller`: return early (remain in `Working`).
     - If `current_state == InteractionState::Thinking`: transition `Thinking -> InteractionState::Speaking` immediately when audio starts playing on speakers.
     - When tool calls arrive later, `steps.rs` transitions `Speaking -> Working`.

4. **Multi-byte UTF-8 & Case-Insensitive Safety (`think_tags.rs`):**
   - Replaced raw byte slicing with `find_case_insensitive` and `match_partial_suffix`.
   - Guarded all slice offsets with `is_char_boundary` to prevent panics on multi-byte CJK or emojis.
   - Handled `<think>`, `<THINK>`, and mixed-case `<Think>` tags safely. Added 4 unit tests covering ASCII, uppercase, and multi-byte UTF-8 split tags.

#### Part B: Test Suite Overhaul per `testing-style-guide.md` & `create-test`

1. **Shared Test Rig (`app/src-tauri/tests/common/harness.rs`):**
   - Move shared `struct Rig`, `ScriptedLlmWorker`, and test fixtures into `tests/common/harness.rs`.
   - **Production Wiring Parity (§6.1):** In `Rig`, spawn the real `spawn_router` OS thread loop with an authentic channel receiver.
   - Expose `rig.send_event(VoxEvent)` to route events (`PlaybackStarted`, `PlaybackFinished`) through `pipeline_tx` into the router thread.
   - Eliminate all direct calls to internal handlers (`on_llm_finished`, `on_playback_started`, `on_playback_finished`) in tests.

2. **Fold Seam Proliferation (§1 & §6):**
   - Fold `tests/harness_pipeline_test.rs` and `tests/harness_loop_test.rs` into a single canonical seam binary: `tests/harness_loop_test.rs`.
   - Structure `tests/harness_loop_test.rs` into 15 subtests:
     - Subtests 01-05: Existing turn loop lifecycle (`cancelled_turn_never_dispatches`, `duplicate_query_refused`, `finished_turn_commits_both_messages`, `unmounted_harness_reports_error`, `critical_threshold_plays_compaction_filler`).
     - Subtest 06: `the_tool_loop_stops_at_the_tool_budget` (asserts `requests.len() == 21` for `MAX_TOOL_STEPS = 20`).
     - Subtests 07-15: The 9 unique scene tests from `harness_pipeline_test.rs` (Scene 1: 0-tool demuxing and two-key latch; Scene 2: preamble preservation and concurrent tool execution; Scene 3: multi-tool batch accumulation and `set_title`; Scene 4: cancellation before speech, during tools, and with partial text).
   - Delete duplicate `tool_loop_reaches_max_steps_and_forces_tool_free_final_pass`.
   - Delete redundant binary `tests/harness_pipeline_test.rs`.
   - Update `app/src-tauri/tests/tier-default.txt`: ensure `harness_loop_test` is listed, remove `harness_pipeline_test`.

3. **Verify Unasserted Claims & Eliminate Bare Sleeps (§3 Rule 2, §5.4):**
   - In `stream_error_clears_turn_open_and_dispatches_error_event`, assert `VoxEvent::Error` is received on the event stream.
   - Replace any bare `std::thread::sleep` or `tokio::time::sleep` loops with channel-driven synchronization or bounded latch waits.

4. **Fix Test Function Naming (§4):**
   - In `services/llm/transport/think_tags.rs`, rename all unit tests starting with `test_*` to `<subject>_<scenario>_<outcome>` (e.g. `plain_tokens_without_tags_emit_as_token_events`).

5. **End-to-End Wire Tests for Reasoning Demuxing:**
   - In `tests/agentic_tool_loop_test.rs`, add mock SSE/NDJSON payloads containing `<think>` tags and `reasoning_content` deltas to verify `chat_completions.rs` and `ollama.rs` transport demuxing end-to-end.

#### Part C: Verification & Review Gates

- **Compile Gate:** `./scripts/verify-targets.sh` (desktop clippy `-D warnings`, android cross-check).
- **Test Gate:** `cargo nextest run --test harness_loop_test --test agentic_tool_loop_test --release --test-threads=1 --no-fail-fast`.
- **Review Gate:** Dedicated Adversarial Review subagent using `/review` and `testing-style-guide.md` auditing the full Batch 6 diff.

---

## 5. Risks and Technical Inferences

1. **Audio Ring Buffer Drain Latency:**
   - *Risk:* If `on_playback_finished` triggers while a trailing TTS chunk is being scheduled, premature transition to `Ready` could occur.
   - *Mitigation:* The latch requires BOTH `turn_open == false` AND `pending_synthesis_jobs == 0`. As long as chunks are in synthesis or in the channel, `pending_synthesis_jobs > 0` blocks premature exit.
2. **Provider Inline Thinking Variations:**
   - *Risk:* Different models use `<think>`, `thought`, or `reasoning_content`.
   - *Mitigation:* Network ingress stream adapters handle provider-specific formats at the boundary; `router.rs` receives only canonical `LlmStreamEvent::Thinking`.

---

## 6. Specification Traceability Matrix

| Spec Requirement | Plan Batch | Verification Checkpoint |
| :--- | :--- | :--- |
| `RULE-HARN-09` (3-Stream Demuxing & Thinking Isolation) | Batch 2 & Batch 4 | Thinking tokens in memory only; zero IPC; excluded from `assistant_text`. |
| `RULE-HARN-10A` (Preamble Speech Preservation) | Batch 4 | Preamble audio synthesizes; text committed to message 0 of tool turn. |
| `RULE-HARN-10B` (Stream Audio Intent & Two-Key Latch) | Batch 4 & Batch 5 | Mid-stream clauses tagged `InterimFiller`; Two-Key Latch governs `Ready`. |
| `RULE-HARN-11` (Universal Tool Contract & Zero Fillers) | Batch 1 & Batch 3 | No `ToolFlow`; no `spoken_filler`; `respond_and_set_title` deleted. |
| Turso Migration v16 | Batch 1 | `session_tool_calls` without `tool_kind`; `verify-targets.sh` clean. |
