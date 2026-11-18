# Vox Mutation Report

**Status:** SMOKE TEST ONLY — not a full campaign
**Mode:** local  
**Mutant set:** `app/src-tauri/tests/assets/mutants.json`  
**Runner:** `app/src-tauri/scripts/mutation-runner.py`

| Mutant | Test | Killed | Seconds | Note |
| :--- | :--- | :---: | ---: | :--- |
| M01 — hardware_cancel_flag rearm latches true instead of clearing | `playback_test` | yes | 56.0 | expected-failure: next_turn can no longer re-arm the flag; the false assertion fails |
| M02 — quiescence law never returns Ready when llm_done | `playback_test` | NO | 56.6 | test passed under mutant |

**Mutation score:** 50.0% (1/2 killed; 1 survived).

## Surviving mutants — RCA

- **M02 — quiescence law never returns Ready when llm_done** (`playback_test::playback_finished_is_deferred_while_a_job_is_pending`)
  - **Why it survived:** the test sends `LlmFinished` with `pending_tts_jobs == 0` before the final `PlaybackFinished`. `on_llm_finished` already transitions `Thinking → Ready` on drained audio (`RULE-EVT-09 §3`), so the mutated branch in `on_playback_finished` is never the cause of the observed exit. The test name overstates what its body exercises.
  - **Verdict:** the test is weak for the `PlaybackFinished` quiescence branch, not the mutant. This is a real finding; see `test-review-report.md` CP4.5. Fix by driving `PlaybackFinished` with `llm_done` already latched by an epoch-guarded `LlmFinished` while audio is still active, so the branch under test is the only path to `Ready`.
  - **Status:** OPEN. Do not count this smoke run as campaign evidence.

All mutants were restored byte-for-byte after execution. A survivor means the test is weak or the mutant is wrong; triage before merge. RCA text is added manually after triage.
