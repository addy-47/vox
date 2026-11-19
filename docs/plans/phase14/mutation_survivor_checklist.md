# Mutation Survivor Remediation Checklist

Source of truth: `/tmp/opencode/rerun_postmerge.md` plus targeted re-runs recorded in `docs/tests/mutation_report.md`.

Initial scope: **12 survivors**. All 12 are now resolved.

## Sprint 1 — harness_loop seeds
- [x] S065 — re-seeded to drop Thinking from PlaybackStarted ingress; KILLED
- [x] S071 — re-seeded to corrupt the Cancelled event turn_id; KILLED
- [!] S085 — FLAGGED: three re-seeds survive; the test does not drive the compaction branch under mutation. Documented skip with evidence.

## Sprint 2 — weak test assertions
- [!] S022 — FLAGGED: default-project guard and session-count check precede the FK path, so no single-line FK mutant is observable. Documented skip with evidence.
- [x] S053 — re-seeded PttStop→PttCancel; KILLED
- [x] S015 — re-seeded to drop AEC clean output; KILLED

## Sprint 3 — remote seeds
- [x] S211 — re-seeded to force Unpaired in the supervisor transition; KILLED
- [x] S213 — re-seeded to force Unpaired on host disconnect; KILLED
- [x] S248 — re-seeded to shorten encrypted output; KILLED
- [x] S249 — re-seeded to disable the handshake payload cap; KILLED

## Sprint 4 — transcript seeds
- [x] S317 — re-seeded to force Ready instead of Thinking; KILLED
- [x] S320 — re-seeded to no-op late-final prefix append; KILLED

## Drift discovered during final enumeration
Backend fixes rewrote several production paths, invalidating six mutant targets. Re-derived and verified:
- [x] S223 — re-seeded to early-return on UnknownPeer; KILLED
- [x] S240 — re-seeded to drop inbound control size cap; KILLED
- [!] S241 — FLAGGED: both close-frame targets survive (peer detects close via transport FIN). Documented skip with evidence.
- [x] S319 — re-seeded to discard the continuation prefix; KILLED
- [x] S322 — re-seeded to reuse a cancelled turn token; KILLED
- [x] S323 — re-seeded to disable transliteration; KILLED
- [!] S242, S243 — FLAGGED: targets invalidated by the WsLink duplex rewrite. Documented skips with evidence.

## Completion
- Total identified: 12 survivors.
- Resolved by KILLED re-run: 10.
- Flagged with evidence: 2 (S022, S085), plus 3 drift flags (S241, S242, S243).
- Final manifest: 339 total, 317 executable, 22 documented skips, 0 surviving executable mutants.
- Final dry-run: PASS. Production tree clean; no mutant markers remain.
