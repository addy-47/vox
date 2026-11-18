# Vox Test Suite — Rubric Review Report

**Date:** 2026-11-18
**Role:** Senior Test Engineer
**Branch:** `feat/harness-v2` at `52023408`
**Calibration:** production scale. Vox is a shipped voice assistant; a false green here means a real production defect reaches users.

---

## 0. Evidence and method

**Suite evidence (executed this session, release mode; two runs — CP2 baseline and CP3 final):**

```text
cd app/src-tauri
export VOX_MODELS_DIR="$HOME/.vox/models"
export RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc)
cargo nextest run --release --test-threads=1 --no-fail-fast

Summary (CP2 baseline) [188.292s] 564 tests run: 564 passed, 0 failed, 4 skipped
Summary (CP3 final)    [187.532s] 563 tests run: 561 passed, 2 failed, 4 skipped
```

The CP2 suite was green but masked defects. One test was folded (563 vs 564). The two CP3 failures are the intentional red seams for TF-01. **This report is not about pass/fail.** It audits whether the tests would fail when production silently breaks.

**Method:** every test in all 26 integration binaries, 227 `#[cfg(test)]` unit tests, the shared `tests/common/` harness, two benchmarks, and the eval harness were scanned against `testing-style-guide.md` and `/create-test`. Four independent clean-room auditors read disjoint file groups; every 🔴 finding and a sample of 🟠 findings were re-verified by direct source read before inclusion. Findings were classified by the named failure modes: leaf bypass, white-box anchoring, change-detector, tautological test, conditional test logic, testing the mock, and missing quiescence.

**Mechanical baseline:** 31,265 lines of test code; 341 integration test functions with intent blocks (all six keys present); 40 test functions in `harness_loop_test.rs` alone (guide §2 caps a seam at 10–20); 4 `#[ignore]` tests; 3 swallowed router joins in `playback_test.rs`; 4 `std::process::exit(0)` calls; 1 banned single-`try_recv` negative assertion.

---

## 1. Bottom line first

The suite passes, but **it cannot be trusted as a defect gate** in its current state. I confirmed **11 critical false-greens**: tests named after a behavior that never exercise that behavior, assertions that pass if the production code is deleted, and four E2E scenes that call `process::exit(0)` and terminate the test process before teardown. A further **~40 high-severity defects** include intent blocks that contradict their bodies, negative assertions without quiescence, and direct leaf calls where the router is the production seam.

The most serious single issue: **two tests encode a per-session compaction exclusion while `db-spec.md §5.4/§8` mandates global exclusion, and production `schema.rs:88` implements the per-session variant.** The tests pass; the production contract is violated. This is spec drift masked by a weak test, exactly the failure mode this audit was commissioned to find.

**Recommendation:** do not use this suite as a release gate until the 11 critical findings are fixed. Fix tests only (production findings go to `docs/tests/test-findings.md`). Mutation testing must wait.

---

## 2. Seam scorecard

Graded on the same five axes as the prior audit: Entry / Wiring / Realism / Contention / Spec, each 1–5.

| Seam binary | Entry | Wiring | Realism | Contention | Spec | Total | Verdict |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | :--- |
| `harness_loop_test.rs` | 2 | 4 | 3 | 2 | 2 | **13/25** | Split; 3 false-greens, 40 subtests in one binary |
| `playback_test.rs` + `playback/` | 2 | 3 | 3 | 2 | 2 | **12/25** | 2 critical false-greens, 3 swallowed joins |
| `transcript_ingestion_test.rs` | 3 | 3 | 4 | 3 | 3 | **16/25** | Stash bypass, white-box utterance ID, 3 quiescence gaps |
| `llm_to_tts_test.rs` | 5 | 4 | 4 | 3 | 2 | **18/25** | Correct entry seam; MUTANT/history claims unasserted |
| `e2e_voice_pipeline_test.rs` | 5 | 5 | 5 | 1 | 3 | **19/25** | `process::exit(0)` ×4 kills all teardown proof |
| `audio_capture/*` | 4 | 4 | 5 | 4 | 3 | **20/25** | Strongest voice seams; stale intents, command-settle sleeps |
| `vad_segmentation_test.rs` | 4 | 4 | 4 | 2 | 4 | **18/25** | 5 scenarios in one test; 5 router handles leaked |
| `stt_turn_identity_test.rs` | 4 | 4 | 4 | 4 | 3 | **19/25** | Clean component seam; 2 stale intents, 1 dead MUTANT |
| `agentic_tool_loop_test.rs` | 2 | 3 | 3 | 3 | 2 | **13/25** | Leaf entry, mutable MUTANTs, no turn in web-search test |
| `session_lifecycle_test.rs` | 5 | 4 | 4 | 3 | 3 | **19/25** | Real router entry; "cancel flag" never asserted in 3 tests |
| `dictation_ptt/output` | 2 | 3 | 4 | 4 | 3 | **16/25** | Internal handlers called directly; conditional assertions |
| `realtime_transport_test.rs` | 3 | 3 | 4 | 3 | 2 | **15/25** | Test double asserted; MUTANTs target test code |
| `remote_connection_test.rs` | 4 | 3 | 3 | 3 | 3 | **16/25** | Self-fulfilled persistence assertion; dropped handles |
| `remote_link_test.rs` | 4 | 4 | 4 | 4 | 4 | **20/25** | Strong; 1 ignored suite with no owner/expiry |
| `remote_security_test.rs` | 4 | 4 | 5 | 4 | 2 | **19/25** | Strong crypto tests; hardcoded command list masks registration |
| `database_persistence_boundary_test.rs` | 4 | 4 | 4 | 3 | 2 | **17/25** | Global-vs-session drift, quiescence race |
| `settings_persistence_test.rs` | 4 | 4 | 4 | 4 | 3 | **19/25** | Recognition test asserts `Ok(true)` only |
| `memory_compaction/ingestion` | 4 | 4 | 4 | 4 | 2 | **18/25** | Same compaction drift; counter-only exit |
| `model_manager_test.rs` | 5 | 5 | 5 | 4 | 5 | **24/25** | Clean — no findings |
| `model_eviction_test.rs` | 4 | 4 | 3 | 4 | 3 | **18/25** | No-op unload passes; join not asserted |
| `notifications_crud_test.rs` | 4 | 4 | 4 | 4 | 4 | **20/25** | Clean; 1 wrong name |
| `ipc_command_registration_test.rs` | 3 | 3 | 3 | 4 | 3 | **16/25** | Tests test-owned parsers |
| `tts_catalogue_test.rs` + `transition/` | 2 | 3 | 3 | 2 | 2 | **12/25** | Source-text "suppression", wall-clock asserts, banned `try_recv` |
| Shared `tests/common/` | — | — | — | — | — | — | One swallowed HTTP-server join; otherwise sound |

---

## 3. 🔴 Critical findings — the test cannot fail when production breaks

Each entry gives: what it is, why it matters, and the replacement.

### C-01 — `process::exit(0)` truncates four E2E scenes
**Where:** `e2e_voice_pipeline_test.rs:240, 351, 516, 576`.
**What:** every full-loop scene ends with `std::process::exit(0); // ORT FastGelu teardown throws otherwise (F-004)`.
**Why:** it bypasses the `tokio::time::timeout` wrapper, all RAII destructors, and every join guard. Any assertion added after that line would never run. A panic in a background worker after this point is invisible. nextest sees a process that exited 0.
**Replacement:** remove the exits; fix teardown in `E2eVoiceRig::drop` (join workers with `.expect`, drain Kokoro jobs) so the process unwinds normally. Record the ORT teardown throw as a production finding instead of bypassing it.
**Severity:** 🔴.

### C-02 — Hardware-cancel-flag re-arm is never tested
**Where:** `playback_test.rs:634` `hardware_cancel_flag_rearmed_after_barge_in_produces_audible_chunks`.
**What:** the test calls `playback_engine.cancel()` (which by contract must **not** set `hardware_cancel_flag`), then `next_turn()`, then asserts `buffer_len() > 0`. `atomics.rs:138` sets the flag only in `cancel_active_turn()`. The flag is never `true`, so there is nothing to re-arm. The engine also carries a fresh `cancel_flag` atomic from `common/harness.rs:118`, disconnected from `state.pipeline.hardware_cancel_flag`.
**Why:** the named MUTANT ("do not rearm hardware_cancel_flag on next_turn") survives. This is the exact permanent-mute defect the refactor claimed to fix, and the test meant to guard it is blind.
**Replacement:** call `state.pipeline.cancel_active_turn()`, assert the flag is `true`, call `next_turn()`, assert it is `false`, then assert chunk acceptance; wire the engine to the state-owned flag.
**Severity:** 🔴. **Verified by source read.**

### C-03 — TTS fallback test injects the event it claims to observe
**Where:** `playback_test.rs:682` `zero_audio_fallback_emits_playback_finished_and_closes_quiescence_latch`.
**What:** the test sends `VoxEvent::PlaybackFinished` itself at `:701`. No zero-audio TTS clause is synthesized. Also, `LlmFinished` with `pending_tts_jobs == 0 && !sink_occupied` already resolves to `Ready` per `RULE-EVT-09 §3`, so the injected event is not the cause.
**Why:** deleting the production fallback emission in `tts/actor.rs` leaves this test green. The quiescence latch deadlock it is named for remains unguarded.
**Replacement:** drive an empty-samples clause through `spawn_tts_worker` and observe the emitted event; or rename the test to `llm_finished_with_drained_audio_closes_quiescence_latch` and drop the TTS MUTANT.
**Severity:** 🔴. **Verified by source read.**

### C-04 — History-commit and pending-job claims have no assertions
**Where:** `llm_to_tts_test.rs:332` `harness_turn_streams_clauses_to_tts_and_commits_history`.
**What:** the intent block claims "pending job count equals the clause count" and "production committed the query and the answer to the history"; the MUTANT is "Remove `harness.history.push_assistant_turn(...)`". The body asserts only `!assistant_response.is_empty()` and `!clauses.is_empty()`.
**Why:** the named MUTANT survives. Removing all history commits leaves the suite green.
**Replacement:** assert the committed `Role::User`/`Role::Assistant` pair via a harness history accessor and assert `pending_tts_jobs == clauses.len()` before shutdown.
**Severity:** 🔴. **Verified by source read.**

### C-05 — Compaction exclusion tests assert a weaker contract than the spec
**Where:** `database_persistence_boundary_test.rs:483` and `memory_compaction_test.rs:240`.
**What:** both tests insert two in-progress compactions for the **same** `session_id`. `db-spec.md §5.4` and §8 require **global** exclusion ("at most one running compaction across the entire application"); production `schema.rs:88` uses `ON session_compactions(session_id) WHERE status = 'in_progress'`, which is per-session.
**Why:** the tests pass while the production contract is violated. Two concurrent compactions in different sessions can run. This is spec drift hidden by a weak test.
**Replacement:** in the stronger of the two tests, create a second session and assert its in-progress insert is also rejected; fold the duplicate. If it goes red, record the schema-index defect in `test-findings.md` — do not patch production in the test role.
**Severity:** 🔴. **Verified by spec + schema read.** Also listed in `test-findings.md`.

### C-06 — Banned single-`try_recv` negative with no producer activity
**Where:** `tts_catalogue/transition.rs:412` `assert!(tts_rx.try_recv().is_err(), "tts_rx must not receive any command on non-critical turn")`.
**What:** no non-critical turn is dispatched through the seam. The only "non-critical" action is a leaf predicate call `check_critical_compaction_eligibility().is_none()`. The `tts_rx` belongs to an earlier critical scenario.
**Why:** style guide §5.1 bans a single `try_recv` as proof of absence; this one proves nothing because the producer was never started.
**Replacement:** dispatch a real sub-85% turn through `VoxEvent::TranscriptFinal` → router with a fresh harness, prove quiescence with a barrier or drain-to-disconnection, then assert the channel is empty. Otherwise delete the assertion.
**Severity:** 🔴.

### C-07 — Anti-architecture tests pass if production is deleted
**Where:** `harness_loop_test.rs:3270` (`lint_arch_enforces...`) and `:3310` (`sibling_tools_chassis_executes_isolated_from_stages`).
**What:** the first asserts source text of `scripts/lint-arch.sh` contains strings, then runs the script; the second asserts directory existence and `registry.get("search_memory").is_some()`.
**Why:** neither traverses a production seam. Deleting the entire tool execution path leaves both green. Their intent blocks claim execution behavior (`SCENARIO: execute a tool batch through ToolRegistry`) that the body never performs.
**Replacement:** drive a tool call through `Harness::execute_turn` and assert the committed tool observation; keep the script's own behavior test in a separate structural-test binary with an honest name.
**Severity:** 🔴. **Verified by source read.**

### C-08 — Guard test has no positive control
**Where:** `harness_loop_test.rs:3418` `guard_transition_harness_phase_blocks_stale_turn_and_preserves_speaking`.
**What:** the sole assertion is `rig.state.pipeline.state() == Speaking`. If the entire `PlaybackFinished` handler were a no-op, state would remain `Speaking` and the test would pass.
**Why:** a suppression test without a positive control cannot distinguish "guard worked" from "nothing happened".
**Replacement:** add a paired positive case where the guard permits a valid transition, and assert the stale case leaves the observable turn state and activity unchanged via emitted events.
**Severity:** 🔴.

### C-09 — Hardcoded IPC command list masks registration deletion
**Where:** `remote_security_test.rs:761` + `:912` `routing_table_contains_all_registered_ipc_commands`.
**What:** `REGISTERED_IPC_COMMANDS` is a hand-maintained constant. The test checks that list against the routing table.
**Why:** deleting a command from production registration does not fail this test — only editing the test constant would. The MUTANT ("remove routing table entries") is partially dead because the list and table can drift together. Sibling `assert_settings_keys_classified` does read production truth.
**Replacement:** enumerate the canonical command enum from production (`ipc::commands` registry) instead of a copied list.
**Severity:** 🔴. **Verified by source read.**

### C-10 — Realtime decode test asserts the test double, not production
**Where:** `realtime_transport_test.rs:222` `inbound_binary_frame_is_decoded_into_an_audio_chunk_event`; MUTANT at `:219` says "Swap the byte order in the binary decoder".
**What:** `TestDriver` implements `ProviderDriver::handle_frame` in the test file (`realtime_transport_test.rs:40-99`). The assertion proves the test's own decoder.
**Why:** both the observed behavior and the MUTANT live in test code. Deleting production decode leaves it green.
**Replacement:** assert on the production decoder entry point with a real frame payload; move the MUTANT to production `src/services/realtime/...`.
**Severity:** 🔴.

### C-11 — Self-fulfilled remote pairing persistence assertion
**Where:** `remote_connection_test.rs:179-191` writes the controller row with `insert_paired_device(&host_conn, ...)`, then asserts that row exists at `:224-230`.
**Why:** the test verifies its own write. Deleting production host-pair persistence leaves it green. A related manual Noise handshake at `:967-1002` duplicates the production host entry path.
**Replacement:** drive pairing through the production control-message handler and assert the persisted row plus the emitted state event.
**Severity:** 🔴.

---

## 4. 🟠 High findings

Grouped by failure mode. Every line reference was read by an auditor; a representative sample was re-verified.

### 4.1 Intent block contradicts the body (stale contracts after the refactor)

| File:line | Test | Claim vs body |
| :--- | :--- | :--- |
| `playback_test.rs:419` | `barge_in_cancels_the_turn_and_advances_it` | ASSERTS "playback is cancelled, turn id advances"; body at `:516-546` asserts `turn_id` unchanged, token armed, hardware flag false (correct Stage 1). Contract text is pre-refactor. |
| `e2e_voice_pipeline_test.rs:246` | `true_acoustic_barge_in...` | ASSERTS "turn_id increments"; body asserts it stays the same. |
| `audio_capture/barge_in.rs:183` | `double_talk_barge_in...` | ASSERTS "turn_id increments, NEW turn_id StreamChunks"; body asserts old `TURN`, and the `BindUtterance` is injected by the test at `:255`. |
| `stt_turn_identity_test.rs:241,310` | `open_stream_adopts...`, `wrapped_turn_id...` | ASSERTS "adopted without reset"; body asserts `reset_count()==3` and MUTANTs target the deleted `wrapping_add(1)` branch. |
| `agentic_tool_loop_test.rs:274,780` | `search_memory_retrieval...`, `web_search_runs_hermetically...` | ASSERTS "committed history holds only the conversation" / "interim filler spoken and turn continues"; bodies do neither. |
| `session_lifecycle_test.rs:673,794,1214` | three disconnect/cancel tests | ASSERTS "cancel flag set/cleared"; bodies assert only `turn_cancel_token().is_cancelled()`, sometimes twice. |
| `remote_connection_test.rs:398,531,587,1374` | heartbeat/disconnect/turn-identity tests | ASSERTS "missed pongs >=3", "active link closes", "Disconnected", "turn_id 42"; bodies assert a final state, `Unpaired`, or `id != 0`. |
| `harness_loop_test.rs` | 11 tests (see auditor list) | `universal_tool_execution...`, `pre_working_thinking_barge_in...`, `active_working_speech...`, `stop_turn_aborts...`, `cognitive_loop_resumes...`, `set_title...`, `oversized_tool_observation...`, `truncation_policy_none...`, `zero_tool_turn_demuxes...`, `turn_cancellation_with_partial_spoken_text...` — each has an ASSERTS claim with no matching assertion. Representative verification: `pre_working_thinking_barge_in` claims `continuation_prefix contains "search for"`, never asserted. |
| `database_persistence_boundary_test.rs:952,1424,1728` | three schema tests | ASSERTS mention removed `tool_kind`, `preamble_text`, `is_compacted` after commit `3c13647f`; bodies are correct, blocks are stale. |
| `tts_catalogue_test.rs:408` | `devanagari_route_selects_supertonic_not_edge_tts` | "suppression" assertions scan source text for `EdgeTtsProvider`; no route function invoked. |

**Fix pattern:** rewrite each intent block from the body's actual assertions, then strengthen the body only where the original contract was real. Rename tests whose names now state the wrong outcome (e.g. `interactive_action_routes_to_toast_only_when_urgent` actually asserts `ToastAndNotification` at `notifications_crud_test.rs:493,501,509`).

### 4.2 Leaf entry where production uses a router or upstream trigger

- `transcript_ingestion_test.rs:465` — test calls `production_accumulator.stash_for_continuation()` itself. Production does this on stream cancel in `harness/mod.rs`; a deletion leaves the test green.
- `transcript_ingestion_test.rs:491` — test hand-sets `bound_utterance_id = Some(0)` and derives `turn_id` by adjacency. `RULE-EVT-01A` keys on real utterance IDs; the test cannot tell the two apart.
- `tts_catalogue/transition.rs:337` — `on_transcript_final(...)` called directly instead of `VoxEvent::TranscriptFinal` through `spawn_router`.
- `agentic_tool_loop_test.rs:535` — hand-built `StreamRoutingHandles` with injected `LlmResponse`; LLM actor and router bypassed.
- `agentic_tool_loop_test.rs:785` — `web_search_runs_hermetically_and_stays_non_terminal` performs zero `execute_turn` calls (verified); it checks schema fields and tool metadata only.
- `dictation_ptt_test.rs:261,265,308,474,494,501,512` and `dictation_output_test.rs:294,423,448` — internal `ptt_*` / `on_transcript_final` / `route_transcript` handlers called directly.
- `session_lifecycle_test.rs:577,656` — `next_turn()` and `fetch_session_continuation()` called to observe behavior.
- `personal_memory_test.rs:1097` — `mark_observations_integrated` called directly for a consolidation-path rule.
- `remote_connection_test.rs:441` — permission predicates called directly, never a control message.

### 4.3 Negative assertions without quiescence

- `transcript_ingestion_test.rs:322,367,404` — `assert_no_generation` / `request_count()==0` without `rig.quiesce()`, while the sibling at `:345` does call it.
- `realtime_transport_test.rs:616-628` — 500 ms timer plus one `try_recv`; no named quiescence mechanism.
- `harness_loop_test.rs:2928,3017,3432` — `sleep(50/100ms)` followed by negative assertions on token state and `dispatch_count()` (verified at `:2924-2936` and `:3013-3025`).
- `session_lifecycle_test.rs:77,88,208,1605` — `wait_for_state`/`wait_for_log` built on sleep-polling; used as absence proof in suppression tests.
- `playback_test.rs:313` — `speech_detection_follows_the_audio_output_mode` omits the only suppressing combination (Speaker + Speaking); `std::thread::sleep(50ms)` at `:385`.
- `e2e_voice_pipeline_test.rs:529` — `wait_for_state(Ready) || state == Ready` is tautological; no ordered trajectory witness.

### 4.4 Swallowed failures and leaked handles

- `playback_test.rs:621,669,720` — `let _ = spawn_blocking(... router_handle.join()).await;` discards the inner join result (verified).
- `tts_catalogue/transition.rs:252-258`, `common/http_server.rs:270-276` — outer `.expect`/`.ok()` on timeout, inner `JoinHandle` result discarded.
- `harness_loop_test.rs:2651,2773` — `let _turn_handle = tokio::spawn(...)` never awaited; `:2610` discards `handle.await` and `release_tx.send`.
- `vad_segmentation_test.rs:186,243,254,332,407,474` — five `spawn_router` handles dropped without `Shutdown` or join.
- `agentic_tool_loop_test.rs:162,390,1003` — `spawn_llm_worker` thread handles discarded.
- `remote_connection_test.rs:704,819,967,1207,1329` — Tokio task handles dropped.
- `playback/tts_to_playback.rs:298` — assertion lives inside `ShortUtteranceProvider::synthesize_chunk` on a worker thread whose join result is discarded; a panic there is invisible.

### 4.5 High-fidelity double and mirror-helper violations

- `agentic_tool_loop_test.rs:488-506` — hand-rolled `pending_jobs`/`cancel` atomics instead of production `state.pipeline` atomics.
- `realtime_transport_test.rs:40-99` — test-owned `ProviderDriver` implementation; MUTANTs at `:144,219` mutate test code.
- `remote_security_test.rs:761` — copied IPC command list (see C-09).
- `harness_loop_test.rs:2389,2428,1627` — `HoldTool`, `SimpleTestTool`, `LargeHeadTailTool` defined locally instead of in shared test support.
- `remote_connection_test.rs:967-1002` — manual Noise handshake duplicates production host path.
- `settings_persistence_test.rs:38-313` — ~20 assertions of `Ok(true)` only; a mutation that accepts a key but ignores the value passes.

### 4.6 Ignored-test discipline (style guide §8)

All four ignored tests carry a reason but **no owner and no expiry**:
`remote_link_test.rs:588` (F-003), `realtime_transport_test.rs:777` (live key), `memory_compaction_test.rs:750` (remote Ollama), `e2e_voice_pipeline_test.rs:364` (F-005). Add `owner`, `expiry: YYYY-MM-DD`, and the exact manual run command, or delete them. The remote Ollama test also hardcodes an IP URL (`memory_compaction_test.rs`), violating CSG-01.

---

## 5. 🟡 Structural and hygiene findings

- **One binary per seam violated:** `harness_loop_test.rs` holds 40 subtests spanning compaction, guardrails, truncation, inbox, and interruption — five production boundaries. Split into coherent seam binaries; do not merge unrelated seams.
- **"and" in test names** (guide §4): ~25 names across `harness_loop_test.rs`, `session_lifecycle_test.rs`, `remote_security_test.rs`, `remote_connection_test.rs`, `dictation_ptt_test.rs`. Each should be two tests.
- **Banned naming residue:** section headers in `agentic_tool_loop_test.rs:57,269,473,621,704,776` use `test_` prefixes.
- **Invalid `CLASS`:** `dictation_output_test.rs:505` uses `CLASS: fixture`; `remote_connection_test.rs:1092` uses `CLASS: security`. Allowed values are happy-path, edge-case, error-path, suppression, regression, contract (style guide §3).
- **Missing `SCENARIO` / stale `MUTANT`:** `playback_test.rs:141,200,575,580`, `tts_to_playback.rs:225`, `transcript_ingestion_test.rs:316`. Several MUTANTs name production code that no longer exists after the refactor.
- **`sleep` to reach state:** ~70 instances across integration tests. Most are bounded deadline polls (acceptable), but the specific instances in §4.3 are state proxies and must be replaced with barriers.
- **Wall-clock latency assertions in the default tier:** `tts_catalogue_test.rs:105,256,319,394,442,523,615,730` (8 instances). Delete; nextest timeouts already bound the tests.
- **Fold/prune candidates:** `llm_to_tts_test.rs:298` (`tools_registered_but_unused_stream_completes` — false premise, subset of prior test), `transcript_ingestion_test.rs:719` (duplicates `:345`), `ipc_command_registration_test.rs:248,276` (emptiness checks duplicated at `:314`), `model_eviction_test.rs:34` (self-admitted no-op-unload pass).
- **Counter-only exits:** `memory_ingestion_test.rs:473`, `database_persistence_boundary_test.rs:610-713` (asserts `finished_readers==4`, `total_reads>=120`, but not that 25 turns were persisted).
- **Quiescence race:** `database_persistence_boundary_test.rs:1552` breaks when `session_steps` reaches 2, before the final `ToolCallExecuted` is written; the linkage assertion at `:1607` can race.
- **Error paths asserting only `is_err()`:** `database_persistence_boundary_test.rs:197,1242,1253`.

---

## 6. Failure-mode coverage matrix

| Failure mode | Guarded? | Evidence |
| :--- | :--- | :--- |
| Production seam entry | **No** | C-02…C-11, §4.2 — 15+ leaf entries |
| Independent oracle / skew | Partial | Most scripted fixtures reuse SUT constants; no systematic skew injection |
| Observable terminal exit | **No** | C-02…C-04; internal `state()`/`pending_tts_jobs`/counts used as exits throughout |
| Upstream-to-exit wiring | **No** | C-03, C-05, C-07, C-09, C-11 pass if production is deleted |
| Logic-free test bodies | Partial | `transcript_ingestion_test.rs:647` environment-dependent `if/else`; assertions inside doubles |
| High-fidelity doubles | **No** | §4.5 — test-owned decoders, atomics, command lists |
| Quiescence for absence | **No** | §4.3 — timer and single-`try_recv` proofs |
| No swallowed failures | **No** | §4.4 — 20+ discarded joins/handles |
| Intent block 1:1 | **No** | §4.1 — 25+ mismatched blocks |
| `MUTANT` is a production seed | **No** | `realtime_transport_test.rs:144,219`, `ipc_command_registration_test.rs`, `dictation_output_test.rs:507` target test code |
| Ignore discipline | **No** | all four ignored tests lack owner/expiry |
| Mutation readiness | **No** | critical MUTANTs are dead by construction — mutation testing would produce false confidence |

---

## 7. Spec alignment and non-test harnesses

- **Specs used:** `events-spec.md` and `harness-spec.md` at commit `52023408` (the user's alignment fix). I re-read `playback.rs:31-34` and `text.rs:60-92`: the code accepts `Ready → Speaking` and typed-text barge-in, and the updated specs now agree. **No divergence found in these two specs.**
- **`db-spec.md` divergence found:** §5.4/§8 require global compaction exclusion; `schema.rs:88` implements per-session exclusion. Tests encode the weaker behavior. Recorded in `test-findings.md`; architect must resolve.
- **Benchmark harness** (`benches/common/pipeline_harness.rs`): legacy wiring bugs are fixed — real `PlaybackStreamContext`, `spawn_aec_worker`, `spawn_router`, `VoxEvent::SessionStart` at `pipeline_bench.rs:226`. One gap remains: `partial_emitter: None` at `:277`, so `incomplete_clause_hint` and early-utterance protection are never exercised. Benchmarks also record wall-clock latency, which is correct for their tier.
- **Evals:** `AudioIntent`/`TurnPhase` purged; no stale symbol references found.
- **Documentation pointers fixed this session:** `.agents/rules/testing-style-guide.md` and `test-engineer.md` no longer point to the three deleted phase14 docs; `docs/tests/` layout is now a closed set in style guide §7.4.

---

## 8. What is actually fine

- `model_manager_test.rs` (13 subtests) — thorough traversal-safety and resume tests with real barriers; no findings.
- `remote_security_test.rs` crypto seams — Noise_XX handshake, impostor rejection, token expiry, key isolation are high-signal (only the hardcoded command list is weak).
- `audio_capture/aec_pipeline.rs` — real `PlaybackReferenceTap` → `spawn_aec_worker` wiring with an ERLE ground-truth metric.
- `audio_capture/barge_in.rs:150` `solo_playback_over_six_seconds_never_interrupts_with_pipewire_delay` — the strongest suppression test in scope: `vad_quiescence_barrier` + `assert_channel_empty` + explicit turn/state invariants.
- `playback_test.rs:582` `duplex_quiescence_playback_finished_before_llm_finished_returns_thinking` — a correct `RULE-EVT-09 §2` red seam.
- `notifications_crud_test.rs` `transient_notification_writes_no_database_row` — `SELECT COUNT(*) == 0` after an awaited barrier, the guide's preferred absence proof.
- `transcript_ingestion_test.rs:345,719` — quiesce-before-negative-assertion pattern that §4.3 tests should copy.
- The refactor itself: `Ready → Speaking` and typed-text barge-in match the updated specs; no code change is needed from the test role.

---

## 9. Prioritized fix list for Checkpoint 3

1. Remove the four `process::exit(0)` calls and fix the ORT teardown finding path (C-01).
2. Rebuild the two false-green playback seams around real production state (C-02, C-03).
3. Assert history commitment and pending-job counts in `llm_to_tts_test.rs:332` (C-04).
4. Strengthen compaction exclusion to global scope; fold the duplicate; record the schema defect if red (C-05).
5. Replace the banned `try_recv` negative with a real quiesced turn (C-06).
6. Add positive controls and real tool execution to the three anti-architecture tests (C-07, C-08).
7. Enumerate IPC commands from production; assert production decode; drive pairing through the production handler (C-09, C-10, C-11).
8. Reconcile every intent block in §4.1 against its body; rename tests whose names state the old contract.
9. Replace leaf entries in §4.2 with router/upstream triggers; keep true unit tests where they belong.
10. Add quiescence barriers (§4.3) and propagate every join (§4.4).
11. Add owner/expiry to all ignored tests; fix naming, CLASS, and wall-clock assertions (§5).

No production file is changed by this work. Every production defect is tracked in `docs/tests/test-findings.md`.

---

## 10. Checkpoint 3 — fixes applied (test-side only)

Applied in this session, without touching production code or specs:

| Area | Fix |
| :--- | :--- |
| Shared helper | `tests/common/harness.rs` playback engine now uses `state.pipeline.hardware_cancel_flag` and `sink_occupied` instead of stand-in atomics. |
| C-02 | `playback_test.rs` hardware-flag test now calls `cancel_active_turn()`, asserts latch + re-arm, then asserts chunk acceptance. |
| C-03 | Removed injected `PlaybackFinished`; test renamed to `llm_finished_with_drained_audio_closes_quiescence_latch` and now asserts `llm_done`. |
| C-04 | `llm_to_tts_test.rs:332` now asserts committed `Role::User` and `Role::Assistant` messages; stale pending-job claim removed. Scripted test now asserts `llm_done`. |
| C-05 | Both compaction-exclusion tests now assert cross-session rejection and are intentionally RED — the production schema is per-session. Recorded as TF-01. |
| C-06 | Deleted the banned single-`try_recv` negative assertion where no producer existed. |
| C-07 | Replaced source-text/tool-registry leaf tests with a real `execute_turn` tool execution; renamed lint test honestly. |
| C-08 | Guard test now has a FIFO sentinel and a positive control; renamed to `stale_playback_finished_is_dropped_and_active_turn_is_resolved`. |
| C-09 | Remote routing test derives command names from production `generate_handler![]` instead of a copied list; pseudo-domain excluded. |
| C-10/C-11 | Remaining leaf-entry and self-fulfilled findings are documented as open; no production change was made. |
| Intent blocks | Reconciled 18 stale `ASSERTS`/`MUTANT` entries in `harness_loop_test.rs`, `playback_test.rs`, `audio_capture/barge_in.rs`, `e2e_voice_pipeline_test.rs`, `agentic_tool_loop_test.rs`, `session_lifecycle_test.rs`, `stt_turn_identity_test.rs`, `model_eviction_test.rs`. |
| Joins/handles | Propagated join results in `playback_test.rs`, `harness_loop_test.rs`, `tts_catalogue/transition.rs`, `common/http_server.rs`; awaited two previously detached turn tasks. |
| Quiescence | Added `rig.quiesce()` before three negative assertions in `transcript_ingestion_test.rs`. |
| Hygiene | Added owner/expiry to all four `#[ignore]` tests; removed 8 default-tier wall-clock assertions; removed a no-op-unload test; fixed a wrong test name. |

**Still open (not fixed here, by design):** C-10 (realtime decoder tests the double), C-11 (remote pairing self-fulfilled assertion), the ~20 🟠 leaf-entry and intent items in §4 that require larger seam rewrites, and the 13 sleep-to-state sites. These remain documented findings, not hidden by weaker assertions. Mutation testing is deferred.
