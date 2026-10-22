# AGENTS.md — Vox Workspace Rules

---

## 1. MANDATORY: AGENTS.md Sync Hook

> 🛑 **POST-TASK HOOK — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Update Section 5.**
> After every completed task, update Section 5. **One bullet per work seam** — if the work you did belongs to the same lane (same feature, same subsystem, same logical thread) as an existing bullet, extend that bullet. Only work belonging to a genuinely different lane earns a new bullet.
> Do NOT write to `docs/`, `recent_work.md`, or any other file — Section 5 is the only target.
> [Skip for tasks with no lasting accountability: one-off lookups, read-only investigations, questions answered without touching files.]
>
> **Step 2 — Only when approaching 110 lines: Migrate Section 5.**
> After appending, count total lines. At or above **110 lines**:
> 1. Migrate Section 5 to `docs/plans/<current_phase>/recent_work.md` under `## Past Work (YYYY-MM-DD)`.
> 2. Replace Section 5 with a compact 3–5 bullet summary of only the highest-level milestones.

---

## 2. Key Paths

Only paths with non-obvious rules that agents cannot infer from the codebase index.

| Path | Rules |
| ---- | ----- |
| `app/src-tauri/src/` | No test logic. No benchmarks. Rust grammar order: `backend-style-guide.md §2.1`. |
| `app/src-tauri/tests/` | Named `<feature>_test.rs`. Public `vox_lib` API only. |
| `app/src-tauri/scripts/verify-targets.sh` | **The compile gate.** Always use this — never bare `cargo check` on a cross target. |
| `.agents/rules/` | Read the relevant rule file before acting in any named role. `android-style-guide.md` is mandatory for any change touching `cpal`, tray/menu, dictation, `utils/paths.rs`, model loading, `Cargo.toml` target tables, or any `cfg(`. |
| `docs/specs/` | Authoritative behavioral contracts — SSOT. Never contradict. |
| `docs/plans/phase13/` | Active phase. Plans and `recent_work.md` live here. |
| `temp/.env` / `temp/server.txt` | API keys and remote GPU server creds. Not versioned. |
| `~/.vox/vox.db` | Local Turso DB. Inspect via `tursodb ~/.vox/vox.db "<SQL>"`. |

**Remote GPU server:** creds in `temp/server.txt`. **Never kill running server processes.**

---

## 3. Execution & Testing Invariants

0. **Compile Gate:** Always:
   ```bash
   cd app/src-tauri && ./scripts/verify-targets.sh
   ```
1. **Sequential + Release:** Run benchmarks, evals, test suites, or any cargo command one at a time. Always `--release`.
2. **Test runner (`cargo-nextest`):**
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
   > ⏱️ Full suite baseline: ~45.5s (excluding cold compile). Set a timeout or cron — never leave a run unmonitored.
3. **External-dependency tests (`#[ignore]`):** Run only with explicit user approval:
   ```bash
   cargo nextest run -- --ignored
   ```

---

## 4. Invariants & Specs

### 4.1 Architectural Invariants (Non-Negotiable)

**DATABASE ENGINE (NON-NEGOTIABLE):** WE DO NOT USE SQLITE. Vox exclusively uses the **Turso Database Engine** (`turso` crate: pure-Rust, async-native, MVCC concurrent writers). Never assume SQLite, C toolchains, or rusqlite locking constraints. Reference: `docs/features/turso-database-engine.md`.

**Zero Backward Compatibility (ZBC):** Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers or legacy paths.

**Pre-exploration:** Before any codebase exploration, architecture lookup, graph query, or broad search — read `.agents/rules/codebase-memory-mcp.md` and follow its graph-first workflow.

**Local Turso CLI:**
```bash
tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
```

### 4.2 Code Modification Gate

> 🛑 **Before touching code:** Read the corresponding style guide and engineer rule file in `.agents/rules/` for the area you are working on.
>
> **Android gate:** If the change touches `cpal`, tray/menu, dictation, `utils/paths.rs`, model loading, `Cargo.toml` target tables, or any `cfg(` — also read [`android-style-guide.md`](file:///home/addy/projects/apps/vox/.agents/rules/android-style-guide.md). Android support is incomplete; confirmed traps there have caused false "done" reports.

### 4.3 Spec Alignment (Non-Drift Hook)

> 🛑 Every agent must adhere strictly to the approved specifications.
> 1. **Divergence / Additions:** If behavior, commands, or schemas diverge from or are absent from the relevant spec — STOP and ask for approval. If approved, update the spec FIRST (following `/create-spec`), then write code. Specs must never quietly drift from code.
> 2. **Legacy Code:** If existing code implements behavior not captured in the spec — confirm with the user before pruning the code or updating the spec.

### 4.4 Active Specifications Ledger

All authoritative specs live in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/) , read the all relevant specs for your task as mandatory.explorations 

---

## 5. Current Phase Summary

> 📖 **Phase 13 (active):** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase13/recent_work.md)
> Phase 12 Archive: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md)
- **Frontend hardening & settings hub re-layout:** vitest 162/162, 0 ESLint errors; warm-nav SWR, route-transition unmount/reveal, session-panel virtualisation; radial hub layout (12:00 Persona · 02:00 Memory · 04:00 Language · 06:00 Appearance · 08:00 Speech · 10:00 Interaction), unified `memory` settings domain, shared catalog reveal and modal shells; Speech card (stage tiles, 7 TTS engines, per-engine voice modals) and Language card (provider rows, inline credential inputs, LLM tuning popovers).
- **Backend pipeline & voice reliability:** clippy/lint-arch compile gate, VAD/ASR pipeline, SNR-adaptive onset gate, durable turn-identity gate on TTS playback/barge-in; dual-target compile gate clean.
- **Monitoring & telemetry:** `latest` snapshot null-safety guarded with typed empty fallback.
- **Remote Control v1 (in progress):** W0 (foundation), W1 (pure wire protocol, `RemoteLink` trait, conformance suite) and W2a (device identity, mode-0600 key store, Noise_XX handshake, pairing flow, trust store) landed and audited; 5 latent production defects fixed, architectural checks moved to `lint-arch.sh`, dual-target compile gate 0/0; W2b (WebSocket transport) is next.
- **Test suite audit and revamp (2026-10-22):** audit in `docs/tests/test-suite-audit-2026-10-22.md`; plan in `docs/plans/phase13/test-revamp-plan.md`, tracking in `test-revamp-checklist.md`, findings in `test-findings.md`. **Slice 0 done:** `testing-style-guide.md` rewritten (6-level taxonomy, one binary per seam with 10–20 subtests, per-test `PURPOSE/SCENARIO/ASSERTS/CLASS/REQUIRES/MUTANT` block, quiescence-based absence rule, no wall-clock latency, deliberate no-CI test policy), `create-test` amended to persist the trace and fold-or-prune, `test-engineer.md` aligned, spec §0.1 sleep-helper endorsement removed, `scripts/test.sh` + `tests/tier-default.txt` added (default tier: 184 passed, 1 skipped). **Slice 1 done:** `assert_channel_empty_after` (sleep + single `try_recv`) deleted; replaced with `assert_channel_empty` (never waits, requires a prior quiescence proof) and `drain_pending` (returns drained items); 15 call sites migrated, passive streaming now asserts after both producers are joined; 10 silent drains and 8 swallowed joins removed; duplicated `calculate_rms` moved to `common::audio::rms`; all 8 affected seam binaries green. **Slice 2 done:** `passive_streaming_test.rs` → `vad_segmentation_test.rs`; 10 deterministic state-machine tests in `segmenter.rs` drive the real `process_continuous_segmentation` with a scripted Earshot threshold (onset, plosive dropout, pre-roll order, stream==final audio, turn-id stability, barge-in, 800 ms hangover, silence, decay, teardown); `actor.rs` 5 split `trim_window` tests; 12 legacy helper tests renamed with intent blocks; `--lib` 149 passed / 1 skipped, seam 1 passed in 3.9 s; F-14 (minimum-utterance guard unreachable) confirmed and marked ignored. **Slice 3 done:** new `stt_turn_identity_test` (12 subtests, recording provider, no models, in the default tier) plus 5 unit tests in `stt/actor.rs`; covers final turn/owner, `+1` adoption without reset, turn-jump reset, same-turn idempotence, `u32` wrap, cancelled-current vs cancelled-stale finals, explicit reset, warmup, partial recycling, shutdown; F-15 (coalescing leaks pooled partial buffers) confirmed and marked ignored; F-16 (turn-identity contract has no spec) logged; default tier 214 passed / 3 skipped. **Slice 4 done:** `ptt_window_modular_test` and `ptt_window_realtime_offline_test` folded into new `dictation_ptt_test` (16 scenarios, raw STT sink, no weights) and removed; covers Assistant Modular/Realtime and Dictation start/stop/cancel/ghost-gate/passive/auto-stop/rejected-start; assertions moved from transcript text to the dispatch decision; F-17 (ingestion-gate purge has no deterministic observable) logged; default tier 215 passed / 3 skipped. **Slice 5 done:** `dictation_window_test.rs` split and removed — input half in `dictation_ptt_test` (17 scenarios, now including the cancel-flag reset), output half in new `dictation_output_test` (9 subtests: per-mode routing, typed headless clipboard failure, transcript preservation in last-transcript/in-memory history/JSONL history, blank and disabled transcripts, transliteration on/off); 3 paste-snippet unit tests in `output_router.rs`; 3 translit unit tests; `scripts/test.sh` models tier now also runs the default-tier binaries so model-dependent branches execute; F-3 partially resolved (OS keystroke injection still unobservable — `create_input_adapter()` is not injectable). **Slice 6 done:** new `transcript_ingestion_test` (12 subtests, recording LLM stub, no weights) covers store/dispatch, empty and non-Thinking drops, Realtime isolation, duplicate suppression, the RULE-EVT-08A merge, the late-arriving final and the empty-turn prefix promotion, stale cancel and transliteration; 7 TurnAccumulator unit tests in `pipeline/assistant/mod.rs`; F-4 and F-5 resolved, F-18 logged (a stalled LLM response holds the harness lock and blocks every later turn). Next: Slice 7 LLM to TTS chunking. No production code changed.
---

## 6. Backlog

See **[Phase 13 Backlog](file:///home/addy/projects/apps/vox/docs/plans/phase13/backlog.md)** for Cloud TTS quota management, Azure speech integration, and setup guide modal overhaul details.

---

## 7. TEMPORARY — Remote Control Module Map (delete when the feature lands)

> ⏳ Added by the Remote Control feature. **This section is deleted in the final wave (W8).** It is not a permanent rule of the codebase. Full rationale: [`docs/plans/phase13/remote-control-adr.md`](file:///home/addy/projects/apps/vox/docs/plans/phase13/remote-control-adr.md). Behavioral contract: [`docs/specs/remote-control-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/remote-control-spec.md).

### 7.1 Module ownership

Applies to `app/src-tauri/src/services/remote/`.

| Module | Owns | Must never contain |
| :--- | :--- | :--- |
| `protocol/` | wire message types, binary framing, capability bitmask, wire error codes | I/O, state, Tauri types |
| `link/` | `RemoteLink` trait, `LoopbackLink`, `WsLink`, link lifecycle | protocol semantics, app state |
| `security/` | device identity, key storage, Noise handshake, pairing, trust store | session state, IPC commands |
| `session/` | `RemoteLinkState`, `SessionRole`, session manager, heartbeat, snapshot | protocol encoding, Tauri commands |
| `control/` | routing table, arg specs, command gateway, redaction, event policy | transport, audio |
| `discovery/` | address hints, mDNS advertisement and browsing, dial candidates | session state |
| `audio/` | network capture, network sink, frame pacing | session state, protocol encoding |
| `testing/` | headless reference client | production logic |

### 7.2 Rules

1. **`protocol/` is pure.** No `tokio`, no `tauri`, no filesystem. It is the single source of truth for the wire contract; both roles and the test suite import it with no I/O.
2. **`RemoteLinkState` (`session/state.rs`) is the single source of truth for remote state.** No `is_connected`, `controller_attached`, `is_remote_connected` or equivalent boolean may shadow it anywhere in the codebase. This mirrors `.agents/rules/backend-style-guide.md` §5.
3. **Host and Controller are roles, not branches.** `session/manager.rs` implements the session once, parameterised by `SessionRole`. Never fork session logic per role.
4. **`control/routing.rs` is a static table.** Every IPC command and every `update_setting` domain/key must appear in it, including the `model_parameters` pseudo-domain. A test asserts completeness.
5. **The desktop-vs-remote distinction is an explicit `ControlOrigin` parameter** on the affected service functions. Never a thread-local, never a global flag.
6. **The backend owns remote state and pushes it.** The frontend may read it once at boot, never to drive it. There is exactly one `remote_state_changed` event.
7. **`ipc/` holds thin Tauri command wrappers only.** Domain logic belongs to the service module that owns the domain.
8. **New modules land in the tree above, not as flat files in `services/remote/`.** If a new capability does not fit one of the eight modules, it needs a new module with a stated ownership, not a ninth file at the root.
