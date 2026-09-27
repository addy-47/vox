# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target.
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **110 lines** (the warning threshold before the 175-line ceiling):
>
> 1. Migrate **only the delta**: append to `docs/plans/<current_phase>/recent_work.md` under a `## Past Work (YYYY-MM-DD)` heading just the Section 5 entries added since the last migration. Dedupe against the file first — never re-archive entries already present there (snapshotting the whole Section 5 duplicates history).
> 2. Replace Section 5 in `AGENTS.md` with a compact 3–5 bullet summary of only the highest-level milestones.
> 3. Keep the deep link at the top of Section 5: `📖 Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/<current_phase>/recent_work.md)`.
>
> **This is the complete hook. Nothing else is mandatory on every task.**

---

## 2. Workspace Directory Map

| Path                      | Purpose                                             | Rules                                                                                                 |
| ------------------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `app/src-tauri/src/`      | Purpose Rust source                                 | No test logic. No benchmarks.                                                                         |
| `app/src-tauri/tests/`    | Integration tests                                   | Named `<feature>_test.rs`. Tests public API only.                                                     |
| `app/src-tauri/benches/`  | Performance benchmarks                              | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`.                                   |
| `.agents/rules/`          | Role-specific agent instruction files               | Read relevant file before acting in that role.                                                        |
| `docs/plans/`             | Architecture specs and phase plans                  | Source of truth for specs. Do not contradict.                                                         |
| `docs/features/`          | Implemented feature ledgers                         | Update after completing features.                                                                     |
| `sandbox/`                | Scratch space for experiments, evaluations, scripts | Non-production code. Results in `sandbox/results/`. Datasets in `sandbox/datasets/`.                  |
| `temp/`                   | Ephemeral runtime files: logs, raw LLM outputs      | `temp/.env` (API keys). `temp/server.txt` (remote GPU server creds). Not versioned.                   |
| `submodules/`             | Git submodules                                      | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`, `nexus-rs`.       |
| `~/.vox/models/`          | Local model weights                                 | Canonical manifest: `~/.vox/models/models_manifest.json`.                                             |

**Remote GPU server:** `root@[IP_ADDRESS]` (creds in `temp/server.txt`). Ollama . **Never kill running server processes.**

---

## 3 Execution & Testing Invariants (All Agents)

0. **Command Context Gate:** Only use `cargo clippy --all-targets` this to verify syntax no other cmds. Test cmds must not be run without explicit user approval.

1. **Sequential Execution with Release flag:** Run benchmarks, evals ,test suites or any cargo command strictly one at a time to prevent CPU, memory, and I/O contention and always use the `--release` flag.
2. **Isolated Test Runner (`cargo-nextest`):** Use the cmd listed below and its variations . 
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
   > ⏱️ **Full Suite Baseline Run Time:** ~45.5s test execution without cold compilation , always use a timeout or cron to monitor test runs and avoid hangs.
4. **Test with External Dependencies(`#[ignore]`):** Cloud or remote server tests must be run manually only with explicit user approval: `cargo nextest -- --ignored`.
---

## 4. Invariants ,Rules and Specs [MUST FOLLOW]

### 4.1 Critical Architectural & Logical Invariants (Non-Negotiable Concepts)
**Zero Backward Compatibility (ZBC):** Backward compatibility is not a requirement unless explicitly stated. Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers, legacy paths, 
> 🛑 **MANDATORY CONTEXT GATE:
**Exploration hook:** Before any codebase exploration, architecture lookup, graph query, or broad search, read `.agents/rules/codebase-memory-mcp.md` and follow its graph-first workflow.
>
**Local Turso Database CLI (`tursodb`):** For inspecting or querying the local Turso SQLite database (`~/.vox/vox.db`), use the native `tursodb` CLI tool:
   ```bash
   tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
   ```

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
> - **ANY WRITE TASK whether its Backend/Frontend/Tests:** You MUST read the corresponding style guide and engineer rule file for the specific area you are working on located in .agents/rules/.

### 4.3 Specifications, Behavioral Contracts & Non-Drift Hook [MANDATORY]

> 🛑 **MANDATORY SPEC ALIGNMENT HOOK (NON-NEGOTIABLE):**
> Every agent working on Vox must adhere strictly to the approved specifications.
> 1. **Specification Divergence / Additions**: If an agent needs to implement, modify, or add behavior, commands, or schemas that diverge from or are not defined in the relevant spec, it MUST STOP and ask the user for approval. If approved, the agent MUST update the spec artifact FIRST before authoring or modifying code. Specifications must NEVER quietly drift from code.
> 2. **Code Divergence / Legacy Code**: If existing code implements nuances or legacy behaviors not defined in the spec, the agent MUST confirm with the user first before either pruning the code or updating the spec to capture the behavior.

#### Active Specifications Ledger
1. **[Event-Domain Architectural Specification (Ground Truth)](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)** — *Status: Approved SSOT*. Golden source of truth for pipeline behavior, state transitions, and 6-domain contracts.
2. **[Database & Persistence Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)** — *Status: Approved Target Spec*. Strict persistence boundary, native Turso engine invariants, and normalized v2 schema.
3. **[Minimal Cognitive Memory & Session Continuation Spec (v2)](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)** — *Status: Approved Target Spec*. 2-stage dedup, single evolving personal memory document, rolling working compaction, and deferred episodic tool retrieval.
4. **[IPC Command & Event Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md)** — *Status: Approved / Target Spec*. Grouped frontend-to-backend commands and backend-to-frontend IPC events.
5. **[LLM Agent Harness & Runtime Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** — *Status: Approved Target Spec*.  chassis,session lifecycle, decoupled LLM actor with duplex dialogue pipe, `InteractionState::Working` with audio intent gating and tools wiring.
6. **[Notification Center Behavioral & Interface Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/notifications-spec.md)** — *Status: Approved Target Spec*. Append storage with correlation key, task idempotency, and frontend stream rollup.
7. **[Agentic Tools & Tool Runtime Specification (v1)](file:///home/addy/projects/apps/vox/docs/specs/tools-spec.md )** — *Status: Approved Target Spec*. Dual classification (`Terminal  vs Non-Terminal`), capability gating, RRF hybrid retrieval, and scratchpad rollback.

---

## 5. Phase 12 — Recent Work Summary

> 📖 **Full History:** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md) | Phase 11 Archive: [phase11/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)

- **Memory stack (consolidation, review UI, evals):** Indexed-block consolidation (Schema v8, 4-prompt split), Google Docs-style suggestion review with batch resolution, `memory_pipeline_eval.rs` modularization (1,834→586 lines), candidate-facts staging UI and "Integrate Learned Facts" flow.
- **Agentic runtime, tools & test health:** Full tool taxonomy with reentrant loop and 122 integration tests (73% mutation kill), `nexuss` v0.1.1 publish with adaptive web-search fanout, 139-function test seam audit (9 added / 8 deleted / 5 mutants killed), session gating and provider wire-policy mapping.
- **Dictation usability overhaul (2026-09-27):** Evidence-backed RCA of 4 PTT bugs (false paste success + clipboard wipe on Wayland, no silence auto-stop by design, no live feedback path, misread toast-as-leak); single-handle clipboard + keeper thread with honest Wayland fallback; 1.2s VAD silence watchdog with regression tests; single replaceable OS-notification lifecycle card with throttled live partials; spec-first alignment of `dictation.md` and `notifications-spec.md`.
- **Perf, settings & platform hardening:** cgroup-free RAM measurement (~1.8 GB true footprint), native OS notifications replacing the 80MB GTK toast overlay, tap-to-talk toggle engine with debounce stack, Alt+V default with zero-config GNOME Wayland daemon, settings-desk rebuilds and ambient tuning.
- **Dictation review hardening (2026-09-27):** Scoped auto-stop + live partial streaming strictly to dictation PTT via explicit `stream_partials` flag on `StartWindowValidation` (assistant PTT behavior byte-identical); bundled `process_windowed_validation` params into `&VadActorChannels` (>5-arg rule); dropped unused `AppHandle` param from `dictation_listening`; fixed stable-fmt nits in new toast code. Noted large pre-existing uncommitted working-tree refactor (toast/notification arch) underneath — review fixes confined to new hunks only.
- **Dictation warning cleanup + ghost-paste guard (2026-09-27):** Collapsed dead `"trigger" | "toggle" | _` socket match arms (clippy now zero-warning); restored pre-refactor Idle guard in dictation `on_transcript_final` so transcripts arriving after the user disables dictation mid-transcription are dropped instead of ghost-pasted, with a 2s terminal card dismissing the stale Listening notification. Also repaired a stray double-comma import typo in `engine.rs` found via the resulting compile error.
- **Dictation notifications routed through front door (2026-09-27):** Eliminated all 10 direct `crate::toast::*` bypasses flagged against `notifications-spec.md` §11.1; new `services/notifications/lifecycle.rs` owns the single replaceable card (resident `--print-id`, 250ms throttle, terminal fallback through `notify()` with drawer elevation); `toast.rs` demoted to dumb dispatcher (`show/replaceable/update` primitives); `route_transcript` threads `db: &VoxDb`, STT worker feeds sync DB-free updater; deleted dead `ToastPayload` structs (backend + frontend); full-codebase audit confirmed zero remaining bypasses and zero legacy constructs; `clippy` zero-warning, `pnpm build` green.
- **Dictation→orb leak RCA + owner-trace logs (2026-09-27):** Traced Idle+Paste report (dictation text in clipboard AND main-window dialogue bubbles next to orb); root cause is owner-blind routing — `VoxEvent::TranscriptFinal/PttStart/PttStop` carry no owner so `router.rs` re-snapshots mutable global `state.owner` at arrival (TOCTOU over 5–40s turns), plus frontend `useTranscriptStream.ts`/`useSessionEvents.ts` ignore `payload.owner` (unlike `onStateChanged` which filters Dictation); clipboard copy is by-design Wayland Paste fallback. Added temporary `::Trace` logs (router owner+target, STT partial owner+window-exists, dictation/assistant final entry, frontend owner receipt); `clippy` clean. Crash check: 02:02 poweroff was clean `systemd-logind` shutdown, no kernel panic/OOPS — not VOX.
- **Dictation→orb leak fix (2026-09-27):** Repro logs proved backend correctly emitted `TranscriptFinal owner=Dictation target=tray (tray_exists=false)` yet Main still rendered it — root cause is Tauri `listen()` default `EventTarget::Any` (`match_any_or_filter` matches everything, `event.js:60-65`), so `emit_to("tray")` reaches Main's Any-listeners; removed per-partial trace spam, added silent owner guards (Main drops `owner===Dictation` in `useTranscriptStream`/`useSessionEvents`, Tray drops `owner===Assistant` incl. `state_changed`); backend `::Trace` owner logs kept; `clippy` clean.




