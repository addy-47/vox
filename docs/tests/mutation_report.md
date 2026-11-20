# Vox Mutation Report

**Status:** FULL CAMPAIGN
**Mode:** local  
**Mutant set:** `app/src-tauri/tests/assets/mutants.json`  
**Runner:** `app/src-tauri/scripts/mutation-runner.py`

| Mutant | Test | Killed | Seconds | Note |
| :--- | :--- | :---: | ---: | :--- |
| D5-M01 — Promote every fast bash command by replacing the completion receiver with a permanently pending one | `harness_loop_test` | yes | 60.9 | expected-failure: The fast printf command never reports completion, so the promotion branch returns a running_in_backgro |
| D5-M02 — Mark successful auto-promoted tool outcome as an error | `harness_loop_test` | yes | 59.1 | expected-failure: The promoted bash call is recorded with is_error=1 and a guardrail strike, failing the is_error==0 and |
| D5-M03 — Ignore run_in_background false and always apply the auto-promote window | `harness_loop_test` | yes | 58.7 | expected-failure: The foreground override promotes at 50ms instead of waiting for the 150ms command, so the elapsed-time |
| D5-M04 — Redirect promoted task stderr to null instead of the disk log | `harness_loop_test` | yes | 61.7 | expected-failure: The on-disk log contains STDOUT_LINE_1 but not STDERR_LINE_1, failing the stderr disk-stream assertion |
| D5-M05 — Return log head instead of tail for manage_task status | `harness_loop_test` | yes | 58.7 | expected-failure: status returns LINE_001 instead of LINE_120, failing the tail-content assertion. |
| D5-M06 — Kill only the parent shell PID instead of the whole process group | `harness_loop_test` | yes | 61.8 | expected-failure: The grandchild survives the kill, so the ESRCH grandchild-dead assertion fails. |
| D5-M07 — Suppress proactive turn dispatch in Ready state | `harness_loop_test` | yes | 63.8 | expected-failure: Ready TaskFinished returns without staging a proactive turn, so no TTS speech is emitted and the heard |
| D5-M08 — Bypass the five-minute quiet-window check in Ready | `harness_loop_test` | yes | 58.6 | expected-failure: A task finishing after six minutes of quiet still dispatches a turn, failing the zero-dispatch and zer |
| D5-M09 — Use inbox.push so TaskFinished releases the active speech hold | `harness_loop_test` | yes | 58.5 | expected-failure: The speech hold is cleared while the user is still speaking, failing the anti-clobber is_speech_held a |
| D5-M10 — Remove RULE-EVT-12 guard so Paused offloads to Sleeping while tasks run | `harness_loop_test` | yes | 58.9 | expected-failure: The idle monitor transitions Paused to Sleeping despite the running task, failing the sleep-inhibition |

**Mutation score:** 100.0% (10/10 killed; 0 survived; 0 skipped as unmutatable).

## Surviving mutants — RCA pending

None in this run.

All mutants were restored byte-for-byte after execution. A survivor means the test is weak or the mutant is wrong; triage before merge. RCA text is added manually after triage.
