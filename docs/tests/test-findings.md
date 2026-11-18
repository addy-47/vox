# Vox Test Findings — Production Defects and Unreachable Scenes

**Date:** 2026-11-18
**Role:** Senior Test Engineer
**Purpose:** record tests that fail, are skipped, or are blind because of a production defect or a production-unreachable seam. No review prose belongs here — see `test-review-report.md`.
**Suite status at time of writing:** `cargo nextest run --release --test-threads=1 --no-fail-fast` → **564 passed, 0 failed, 4 skipped** (188.292s). There are **no currently red tests**. The entries below are production defects masked by weak tests, or scenes that cannot run.

---

## 1. Production defects masked or made unreachable by tests

### TF-01 — Global compaction exclusion is not enforced (spec violation)
- **Test that should expose it:** `database_persistence_boundary_test::second_in_progress_compaction_is_rejected_by_partial_index` (`tests/database_persistence_boundary_test.rs:483`) and `memory_compaction_test::second_in_progress_compaction_on_a_session_is_rejected` (`tests/memory_compaction_test.rs:240`).
- **Symptom:** both tests insert two in-progress compactions for the **same** session and pass. A second in-progress compaction for a **different** session is never attempted.
- **Expected per spec:** `docs/specs/db-spec.md §5.4` — "Partial unique index `(status) WHERE status = 'in_progress'` enforces universal global mutual exclusion: at most one running compaction across the entire application and database at any time." §8 and `RULE-DB` verification criteria repeat this.
- **Actual:** `app/src-tauri/src/persistence/schema.rs:88` — `CREATE UNIQUE INDEX ... ON session_compactions(session_id) WHERE status = 'in_progress'`. This is per-session, so two sessions can compact concurrently.
- **Evidence command (for the fix cycle):**
  ```sql
  -- with two sessions and one active in_progress row for session A, insert for session B
  INSERT INTO session_compactions(...) VALUES (..., session_B, 'in_progress');
  -- spec says: UNIQUE constraint failed. Current schema: succeeds.
  ```
- **Suspected location:** `app/src-tauri/src/persistence/schema.rs:88`; index migration needs a new schema version (ZBC — no compatibility layer).
- **Owner for fix:** backend engineer / system architect (spec vs code decision).
- **Test action (COMPLETED):** both compaction tests now assert cross-session rejection and are **RED** in the CP3 suite:
  - `database_persistence_boundary_test::second_in_progress_compaction_is_rejected_by_partial_index` — panics at `tests/database_persistence_boundary_test.rs:531`.
  - `memory_compaction_test::second_in_progress_compaction_on_a_session_is_rejected` — panics at `tests/memory_compaction_test.rs:291`.
  - The assertions are authoritative. Do not weaken them. The suite stays red until the schema index is corrected.

### TF-02 — E2E teardown crash forces `process::exit(0)`
- **Tests:** `e2e_voice_pipeline_test.rs:240, 351, 516, 576` (all four full-loop scenes).
- **Symptom:** ORT FastGelu teardown throws. Each scene calls `std::process::exit(0)` to avoid the throw; this terminates the test process and bypasses all destructors and join guards.
- **Expected:** the rig unwinds cleanly with every worker joined via `.expect(...)` (style guide §5.3/§13).
- **Evidence:** `std::process::exit(0); //// ORT FastGelu teardown throws otherwise (F-004, see findings)`.
- **Suspected location:** ONNX Runtime teardown in the Kokoro TTS path; `tests/common/harness.rs::E2eVoiceRig::drop` cannot currently absorb it.
- **Owner for fix:** backend engineer (runtime teardown) + test engineer (rig teardown).
- **Test action:** remove the exits once teardown is fixed; until then these four scenes provide no teardown evidence.

### TF-03 — Mid-thought continuation E2E is ignored
- **Test:** `e2e_voice_pipeline_test.rs:364` `mid_thought_continuation_combines_both_bursts`.
- **Symptom (as recorded in the ignore reason):** cancelled sidecar inference is not aborted; the merged turn hits the 30 s idle timeout.
- **Expected:** `RULE-EVT-08A` — two bursts merge into one combined query and complete within the persist deadline.
- **Evidence:** `#[ignore = "PROD-BUG: LLM-CANCEL-STARVES — cancelled sidecar inference is not aborted; the merged turn hits the 30s idle timeout; see docs/tests/test-findings.md F-005"]`.
- **Suspected location:** sidecar cancel handling in `src/services/llm/sidecar.rs` / `harness/stages/streaming/router.rs`.
- **Owner for fix:** backend engineer.
- **Test action:** keep the assertion intact; add owner/expiry to the ignore marker; un-ignore after the fix.

### TF-04 — WsLink cannot pass the generic conformance suite
- **Test:** `remote_link_test.rs:588` `ws_link_passes_generic_conformance_suite` (ignored).
- **Symptom:** the generic suite assumes shared-socket control/audio semantics; `WsLink` splits control and audio onto separate link instances.
- **Expected:** one conformance contract applicable to both `LoopbackLink` and `WsLink`, or an explicit per-transport contract.
- **Evidence:** ignore reason `VERIFY: RC-SUITE-1 — generic suite assumes shared-socket control/audio semantics; WsLink splits control and audio onto separate link instances`.
- **Suspected location:** `tests/common/remote_conformance.rs` (suite assumptions) vs `src/services/remote/.../ws_link.rs`.
- **Owner for fix:** backend engineer + test engineer (contract decision).
- **Test action:** add owner/expiry; six `ws_link_*` subtests remain the valid coverage.

### TF-05 — Hardware cancel flag re-arm path is never exercised
- **Test:** `playback_test.rs:634` `hardware_cancel_flag_rearmed_after_barge_in_produces_audible_chunks`.
- **Symptom:** the test uses `playback_engine.cancel()`, which by contract must not set `state.pipeline.hardware_cancel_flag`. The flag is never `true`; `next_turn()` re-arms nothing. The engine under test also carries a disconnected `cancel_flag` atomic.
- **Expected:** `RULE-EVT-09 §4` — `cancel_active_turn()` sets the flag, `next_turn()` clears it, and the second turn produces chunks.
- **Evidence:** `atomics.rs:138` sets the flag only in `cancel_active_turn()`; `playback.rs:652` calls `playback_engine.cancel()` instead. Named MUTANT survives.
- **Suspected location:** test wiring at `tests/playback_test.rs:634-670` and `tests/common/harness.rs:118`.
- **Owner for fix:** test engineer (wire to production state; no production change known to be needed).
- **Status:** production behavior is **unverified** — the current test cannot detect permanent mute.

### TF-06 — Zero-audio fallback emission path is never exercised
- **Test:** `playback_test.rs:682` `zero_audio_fallback_emits_playback_finished_and_closes_quiescence_latch`.
- **Symptom:** the test injects `VoxEvent::PlaybackFinished` itself; no zero-sample TTS clause runs. `LlmFinished` with drained audio already resolves to `Ready`.
- **Expected:** `RULE-EVT-09 §4` — `tts/actor.rs` emits fallback `PlaybackFinished` only when `remaining_jobs == 0 && !playback.is_armed() && playback.occupied_len() == 0`.
- **Evidence:** `tests/playback_test.rs:701` `let _ = event_tx.send(VoxEvent::PlaybackFinished { turn_id });`.
- **Suspected location:** test only; production fallback code needs a real zero-audio verification.
- **Owner for fix:** test engineer.
- **Status:** production fallback behavior is **unverified**.

### TF-07 — Committed history in `llm_to_tts_test` is unverified
- **Test:** `llm_to_tts_test.rs:332` `harness_turn_streams_clauses_to_tts_and_commits_history`.
- **Symptom:** no history accessor or pending-job assertion exists; MUTANT "remove `push_assistant_turn`" survives.
- **Expected:** `RULE-HARN-11B` — the user query and assistant answer are committed to history at the step boundary.
- **Evidence:** body asserts only non-empty assistant response and non-empty clause list.
- **Owner for fix:** test engineer.

---

## 2. Tests skipped for external dependencies

These are environment-limited, not production defects. Per style guide §8 they still need owner, reason, and expiry.

| Test | Blocker | Owner / expiry |
| :--- | :--- | :--- |
| `realtime_transport_test.rs:777` `live_cloud_realtime_handshake` | live `GEMINI_API_KEY` or `DEEPGRAM_API_KEY` | missing — add |
| `memory_compaction_test.rs:750` `compaction_slice_100_turns_against_a_remote_ollama_server` | remote Ollama `gemma3:12b`; also hardcodes an IP URL | missing — add; move URL to config |

---

## 3. Findings register

| ID | Category | Production owner | Test action | Status |
| :--- | :--- | :--- | :--- | :--- |
| TF-01 | Spec/schema divergence | backend + architect | strengthen to global exclusion | OPEN |
| TF-02 | Runtime teardown | backend + test | remove `process::exit(0)` after fix | OPEN |
| TF-03 | Sidecar cancel starvation | backend | un-ignore after fix | OPEN |
| TF-04 | Link contract mismatch | backend + test | add owner/expiry, decide contract | OPEN |
| TF-05 | Unverified cancel re-arm | test | wire to production atomics | OPEN |
| TF-06 | Unverified TTS fallback | test | drive real zero-audio synthesis | OPEN |
| TF-07 | Unverified history commit | test | assert history accessor | OPEN |
| TF-08 | External cloud key | test | add owner/expiry | OPEN |
| TF-09 | External Ollama server | test | add owner/expiry, remove hardcoded URL | OPEN |

No finding in this file is fixed by weakening an assertion. Production fixes are owned by `backend-engineer`; test fixes by the test engineer in Checkpoint 3.
