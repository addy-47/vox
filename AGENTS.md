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
- **Catalog Modal & State Decomposition:** Tickets 1-10 done — unified backend dialect probing, `CatalogModalShell`, 3-stage responsive layouts, `ProviderPillGrid`; pruned alias fallbacks; transactional IPC batch patch; separated `catalogStore.ts`/`engineStatusStore.ts`; resolved re-render loops via `useShallow`; fixed scrollbar hover flicker; unified custom server resolution. Completed provider-list SSOT migration: `ProviderModal`, `RemoteCatalogSection`, `InteractionConfigDesk` now consume `useCatalogStore.cloudProviders/ttsProviders`; removed all legacy `CLOUD_PROVIDERS`/`TTS_MODAL_PROVIDERS` references; tsc --noEmit clean, vitest 148/148, lint:arch green.
- **Cloud TTS Streaming & Voice Preview:** Google Cloud TTS (bidi gRPC) and Azure AI Speech (REST SSML 24kHz); live voice catalog in Turso DB; dual-target compile gate clean.
- **Acoustic Echo Cancellation (AEC) & Incognito Persistence:** WebRTC AEC3 dynamic delay adaptation & complete queue drain; `ClearRenderQueue` on interruption and cancel; unified Incognito private mode SSOT across backend settings, header Ghost button, and state capsule badges; swept legacy `~/.vox/vox.db` debris.
- **Frontend test-suite rebuild (in progress):** Batch 0-2 complete — vitest config, shared harness (`transportStub.ts`, `settingsFixtures.ts`), `activeSelection.ts` (`promoteActive`), regression tests for settings flags, overlay stack, catalog store, pipeline service, voice session stream; all typecheck-clean. Batch 3 pending: `notificationPanel` test failing on `NotificationRecord` fixture shape — deleted to keep suite green; resume with correct transport stub + fixture.
- **Frontend lint tooling audit & defect remediation (complete):** Tickets 01–09 done; **0 ESLint errors** (275 → 0); vitest 148/148, tsc/lint-arch clean; CDP stress & soak suites expanded with mock IPC backend covering all panels, drawers, modals, canvas containment, focus trapping (`EdgePanel`), and deferred focus restoration (`Modal`/`Drawer`); 0 runtime exceptions, 0 leaked backdrops.
- **AGENTS.md & rules/ rewrite:** Rewrote all 12 rule files via `/writing-for-agents`; pruned dir map, hardened post-task hook (single-bullet-per-seam), removed duplicate frontmatter, deduplicated Android pitfalls, aligned testing references, and strictly demarcated persona roles vs style guides.
- **Backend Invariant Gate & Remediation:** Integrated `clippy.toml` (`disallowed_methods`) and `lint-arch.sh` into `verify-targets.sh`; completed Tickets 01–03 (named probe threads, fact text lookup, and TTS voice catalog SQL in persistence/); 0 SQL violations remaining outside persistence; dual-target compile gate clean.
- **VAD, ASR Pipeline & Mid-Thought Continuation:** Eliminated pre-roll turn-id race and Nemotron duplicate tail; un-stacked double hysteresis between Silero ONNX and `segmenter.rs` with SNR-adaptive leaky onset and cadence-aware hangover; added mid-thought continuation merging and late transcript capture in `events-spec.md`; 233/233 tests passed, dual-target compile gate clean.
- **Session Panel Remount/Lag & Voice Preview Lifecycle:** Fixed the Session Panel cascade remount (removed `keepMounted` hidden second instance; memoized `closeSessions` so streamed-token Home renders no longer defeat `SessionPanel`'s memo); fixed stale `<audio>` handler teardown in `useVoiceCatalog` that killed a just-started preview (the "click twice"/spurious `Audio playback error` symptoms); added DEV-gated `useRenderTrace` lifecycle logging across the session panel tree and gated the per-render `curatedProviders` log; fixed in-place mutation of the memoized provider list in `InteractionConfigDesk`; vitest 148/148, 0 new ESLint errors, tsc clean apart from one pre-existing `Reorder.Item layout={false}` error. Follow-up (lag + waveform): killed the panel re-render storm — `useSessionPanel.refresh` now reconciles sessions/projects so a no-op `sessions_changed` returns the *previous* array (React bails out, memoized rows hold identity), and every panel callback (`handleSelect` via a data ref, `renameProject`, `deleteProject`, `togglePin`, `moveSessionToProject`, `toggleProjectExpanded`) is referentially stable so `React.memo` on the ~42 rows actually bails; `SessionProjectsSection` skips no-op `projectIds` writes; made `useRenderTrace` opt-in (`?trace=1` or `localStorage["vox.trace"]="1"`) so DEV logging no longer inflates the lag, and added a gated `projects-prop CHANGED` diagnostic to pin parent-churn vs local-state churn; fixed the voice-catalog waveform horizontal overflow (bar count now follows the measured scrubber width, bars are shrinkable, and the list scroller is `overflow-x-hidden`); removed the now-dead `EdgePanel.keepMounted` prop; added `sessionPanelReconcile.regression.test.ts` (9 tests, mutation-verified against 3 seeded defects); vitest 157/157, 0 ESLint errors, tsc clean apart from the pre-existing error. RCA harness: `useRenderTrace` is **on by default in dev** (`?trace=0` or `localStorage["vox.trace"]="0"` to disable) and its new `watch` option diffs the identities React itself compares, logging `changed=[projects,onRename] dt=0.4ms` — the *cause* of each render; `useSessionPanel.refresh` logs its duration plus whether it changed anything; `EdgePanel` marks `edge-panel-open` and `SessionPanel` measures its mount against it.

---

## 6. Backlog

See **[Phase 13 Backlog](file:///home/addy/projects/apps/vox/docs/plans/phase13/backlog.md)** for Cloud TTS quota management, Azure speech integration, and setup guide modal overhaul details.