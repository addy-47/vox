# Test Revamp — Persistent Checklist

Source of truth for progress. Update per item, immediately, not at slice end.
Plan: `docs/plans/phase13/test-revamp-plan.md`
Findings: `docs/plans/phase13/test-findings.md`

**Scope:** 27 in-scope integration files / 121 test functions / 30 `src` unit-test modules.
**Excluded:** 12 remote files, 51 tests.

Legend: `[ ]` pending · `[x]` done · `[~]` folded into another binary · `[-]` pruned · `[!]` flagged (needs decision)

---

## Slice 0 — Standards (no test files)

- [x] `.agents/rules/testing-style-guide.md` — rewrite
- [x] `create-test` skill (`/root/.config/opencode/skills/create-test/SKILL.md`) — amend
- [x] `.agents/rules/test-engineer.md` — align
- [x] `docs/tests/integration-test-spec.md` §0.1 — remove sleep-helper endorsement
- [x] `app/src-tauri/scripts/test.sh` + `tests/tier-default.txt` — local entry point
- [x] Report Slice 0

## Slice 1 — Shared test support

- [x] `tests/common/harness.rs` — quiescence-based absence primitive
- [x] `tests/common/paths.rs` — audited, no duplicates or sleeps
- [x] `tests/common/audio.rs` — audited; added shared `rms`
- [x] `tests/common/scoring.rs` — audited, no duplicates or sleeps
- [x] `tests/common/http_server.rs` — audited; one intentional drip-pacing sleep
- [x] `tests/common/log_capture.rs` — audited, no duplicates or sleeps
- [x] `tests/common/archive.rs` — audited, no duplicates or sleeps
- [x] `tests/common/wire.rs` — audited, no duplicates or sleeps
- [x] `tests/common/mod.rs` — audited
- [x] Remove 10 silent `try_recv` drains
- [x] Remove 8 swallowed `let _ = ...join()`
- [x] Sleep inventory (65 sites) triaged — 12 in `tests/common/` (11 legitimate: backpressure, byte pacing, state polls; 1 collection quiet-window); 53 deferred to their owning seam slices
- [x] Remove duplicated `calculate_rms` (moved to `common::audio::rms`)
- [x] Report Slice 1

## Slice 2 — VAD segmentation

- [x] `tests/passive_streaming_test.rs` → `tests/vad_segmentation_test.rs` (1 fn, 3 scenarios: EN, HI, silence guard)
- [x] `src/services/vad/segmenter.rs` unit tests — 10 new state-machine tests on `process_continuous_segmentation` via a scripted Earshot detector; 12 legacy helper tests renamed + intent blocks
- [x] `src/services/vad/actor.rs` unit tests — 6 tests (1 suppression + 5 split `trim_window` branches), named + intent blocks
- [x] Binary: `vad_segmentation_test.rs` — passes in 3.9 s; `--lib` 149 passed, 1 skipped
- [x] Report Slice 2
- [!] F-14 — `VAD_MIN_UTTERANCE_SAMPLES` guard unreachable (ignored test marks it)

## Slice 3 — STT turn identity

- [x] `src/services/stt/actor.rs` — 5 unit tests: coalescing (non-partial passthrough, boundary stop), reset drain (recycle + drop, final preservation, shutdown)
- [x] Binary: `stt_turn_identity_test.rs` (new) — 12 subtests, recording provider, no models, 0.06 s
- [x] Added `stt_turn_identity_test` to `tests/tier-default.txt`
- [x] Report Slice 3
- [!] F-15 — coalescing recycles into the first partial's channel and leaks later buffers (ignored unit test)
- [!] F-16 — turn-identity contract absent from `docs/specs/` (spec gap)

## Slice 4 — Dictation PTT lifecycle

- [x] `tests/ptt_window_modular_test.rs` — folded into `dictation_ptt_test.rs` (S1–S3), file removed
- [x] `tests/ptt_window_realtime_offline_test.rs` — folded into `dictation_ptt_test.rs` (S4–S6), file removed
- [~] `tests/dictation_window_test.rs` — input half covered by S7–S17; file removed in Slice 5
- [x] Binary: `dictation_ptt_test.rs` — 17 scenarios, no model weights, 2.2 s, in the default tier
- [x] Report Slice 4
- [!] F-17 — ingestion-gate purge has no deterministic observable; the 150 ms sleep test is gone with `dictation_window_test.rs`

## Slice 5 — Dictation output

- [x] `tests/dictation_window_test.rs` — split and removed; output half in `dictation_output_test.rs`
- [x] Binary: `dictation_output_test.rs` — 9 subtests, no display needed, in the default tier
- [x] `src/services/dictation/output_router.rs` — 3 unit tests for the paste snippet
- [x] `src/services/translit.rs` — 3 unit tests (script detection, disabled path, non-Devanagari path)
- [x] `scripts/test.sh` — the models tier now also runs the default-tier binaries so model-dependent branches execute
- [!] Paste keystrokes, clipboard restore, Wayland path — not observable headlessly (F-3)
- [x] Report Slice 5
- [!] F-3 — the OS keystroke injection is unobserved; `create_input_adapter()` is not injectable
- [!] F-17 — the ingestion-gate purge lost its coverage with `dictation_window_test.rs`

## Slice 6 — Transcript ingestion

- [x] Binary: `transcript_ingestion_test.rs` — 12 subtests, recording LLM stub, no weights, in the default tier
- [x] `src/pipeline/assistant/mod.rs` — 7 unit tests for the TurnAccumulator continuation logic
- [x] Continuation merge (`RULE-EVT-08A`) — merge, late-arriving final, empty-turn promotion
- [x] Stale cancel — a cancelled token is re-armed by `next_turn`
- [x] Duplicate query filter — suppressed before dispatch, accumulator cleared, state back to Ready
- [~] `tests/transcript_to_llm_test.rs` — ingestion half absorbed; the real-generation and compaction-filler halves move to Slice 7
- [x] Report Slice 6
- [!] F-18 — a stalled LLM response holds the harness lock and blocks every later turn

## Slice 7 — Agentic tool loop

- [x] `tests/agentic_tool_runtime_test.rs` → `tests/agentic_tool_loop_test.rs` (6 → 8 subtests), file renamed
- [x] Non-terminal retrieval — memory RRF and the three-stage web search
- [x] Terminal tool — title update, accumulator parity, clause prefix drop
- [x] Wire mapping — NVIDIA preset and Ollama native request and tool-stream parsing
- [x] Tool filter taxonomy — title, memory and web-search switches
- [ ] Tool cap 5 (`MAX_TOOL_ITERATIONS`) — moved to Slice 12, the harness loop owns it
- [x] Report Slice 7

## Slice 8 — LLM → TTS chunking

- [x] `tests/llm_to_tts_test.rs` — renamed to a behaviour-first name, intent block added
- [x] `tests/chunking_determinism_test.rs` — folded and removed; the clause rules are pure, so they became 14 unit tests in `src/services/harness/stages/streaming/chunker.rs` (default tier) instead of an integration binary
- [x] Clause rules — determinism, emergency cap at 30 words, comma gate, compound seam at 22 words, abbreviation, decimal, mid-token period, question, short and long exclamation, newline, semicolon seam, flush, clear
- [x] Real generation — clauses reach the TTS, the pending job count matches, production commits the history
- [x] Report Slice 8
- [!] Deviation from the plan: the clause rules are component-level, so they live in the lib test binary

## Slice 9 — TTS catalogues and transitions

- [x] `tests/tts_provider_catalogue_test.rs` → `tests/tts_catalogue_test.rs` (9 subtests, honest header, intent blocks)
- [x] `tests/tts_transition_test.rs` → `tests/tts_catalogue/transition.rs`, a module of the same binary (2 subtests)
- [x] `tests/tts_voice_catalogue_test.rs` — folded and removed; the voice resolution rules are pure and became 6 unit tests in `src/services/tts/voice.rs` (default tier)
- [x] Report Slice 9

## Slice 10 — Playback, interrupt, barge-in

- [x] `tests/playback_interrupt_test.rs` → `tests/playback_test.rs` (6 subtests, honest header, intent blocks)
- [x] `tests/tts_to_playback_test.rs` → `tests/playback/tts_to_playback.rs`, a module of the same binary (2 subtests)
- [x] Report Slice 10

## Slice 11 — Audio capture and AEC

- [x] `tests/audio_capture_aec_test.rs` — 3 subtests, honest header, intent blocks
- [x] `tests/aec_pipeline_test.rs` → `tests/audio_capture/aec_pipeline.rs`, a module of the same binary (4 subtests)
- [x] `tests/aec_barge_in_integration_test.rs` → `tests/audio_capture/barge_in.rs`, a module of the same binary (5 subtests)
- [x] Shared `common::audio::erle_db` replaced the duplicated ERLE helpers
- [x] The wall-clock "< 15 ms" latency assertion is gone; the per-chunk deadline remains
- [x] Report Slice 11

## Slice 12 — Harness loop

- [x] Binary: `tests/harness_loop_test.rs` — 5 subtests, recording LLM stub, no weights, in the default tier
- [x] Tool budget — the loop stops after five tool rounds and runs one final tool-free pass
- [x] Cancelled turn — never reaches the model
- [x] Duplicate query — refused before the dispatch
- [x] Unmounted harness — reports an error
- [x] Finished turn — commits both messages to the history
- [ ] `src/harness/chassis.rs`, `src/harness/session.rs` — deferred to the final sweep
- [x] Report Slice 12

## Slice 13 — Session lifecycle

- [ ] `tests/session_lifecycle_test.rs` (8)
- [ ] Report Slice 13

## Slice 14 — Storage boundary

- [ ] `tests/database_persistence_boundary_test.rs` (6)
- [ ] Binary: `storage_test.rs`
- [ ] Report Slice 14

## Slice 15 — Settings

- [ ] `tests/settings_persistence_test.rs` (7)
- [ ] Binary: `settings_test.rs`
- [ ] Report Slice 15

## Slice 16 — Memory

- [ ] `tests/personal_memory_test.rs` (9)
- [ ] `tests/memory_ingestion_test.rs` (4)
- [ ] `tests/memory_compaction_test.rs` (6)
- [ ] Binary: `memory_test.rs`
- [ ] Report Slice 16

## Slice 17 — Notifications

- [ ] `tests/notifications_crud_test.rs` (5)
- [ ] Binary: `notifications_test.rs`
- [ ] Report Slice 17

## Slice 18 — Model manager and eviction

- [ ] `tests/model_manager_test.rs` (14)
- [ ] `tests/model_eviction_test.rs` (2)
- [ ] Report Slice 18

## Slice 19 — IPC registration

- [ ] `tests/ipc_command_registration_test.rs` (2)
- [ ] Report Slice 19

## Slice 20 — Realtime transport

- [ ] `tests/realtime_transport_test.rs` (6)
- [ ] Report Slice 20

## Slice 21 — Final sweep

- [ ] Remaining `src` unit-test modules get intent blocks (30 modules total)
- [ ] Prune / fold pass over the whole suite
- [ ] Feature-gate 12 remote files / 51 tests
- [ ] `cargo-mutants` run (one pass)
- [ ] Full non-remote suite run (one pass)
- [ ] Final report

---

## Totals

| Metric | Target | Current |
| :--- | :--- | :--- |
| Integration binaries | ~19 | 39 (27 non-remote) |
| Non-remote test functions | 121+ | 121 |
| Test functions with intent block | 121 | 0 |
| Raw sleeps in default tier | 0 | 65 |
| Silent drains | 0 | 10 |
| Swallowed joins | 0 | 8 |
| Orphan ignored tests | 0 | 4 |

## Slices 13 to 20 — Supporting seams

- [x] Session lifecycle — `session_lifecycle_test.rs`, 8 → 20 subtests
- [x] Storage boundary — `database_persistence_boundary_test.rs`, 6 → 15 subtests
- [x] Settings — `settings_persistence_test.rs`, 7 → 15 subtests
- [x] Memory — `personal_memory_test.rs` 9 → 17, `memory_ingestion_test.rs` 4 → 11, `memory_compaction_test.rs` 6 → 13
- [x] Notifications — `notifications_crud_test.rs`, 5 → 15 subtests
- [x] Model manager and eviction — `model_manager_test.rs` 14 → 15, `model_eviction_test.rs` 2 → 9
- [x] IPC registration — `ipc_command_registration_test.rs`, 2 → 7 subtests
- [x] Realtime transport — `realtime_transport_test.rs`, 6 → 10 subtests
- [x] Every scenario carries an intent block, a behaviour-first name and an honest header
- [x] Wall-clock watchdogs and sleep-based absence checks replaced by log or server acknowledgements
- [!] F-19 — packaged voice seeding fails on upgraded databases
- [!] F-20 — private-mode suppression has no acknowledgement channel
