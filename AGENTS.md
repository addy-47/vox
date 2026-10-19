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
- **Catalog Modal & State Decomposition:** Provider SSOT migration, catalog modal shell, responsive layouts; tsc/vitest clean.
- **Cloud TTS Streaming & Voice Preview:** Google Cloud TTS and Azure AI Speech; live voice catalog in Turso DB.
- **AEC & Incognito Persistence:** WebRTC AEC3, clear render queue, unified private mode SSOT.
- **Frontend test suite + lint remediation:** vitest 157/157, 0 ESLint errors, tsc/lint-arch clean.
- **Backend invariant gate + VAD/ASR + session panel perf:** clippy/lint-arch gate, VAD/ASR pipeline, session panel reconcile/virtualization/deferred prefetch.
- **Navigation perf (warm-nav SWR + motion):** `getSessions`/`getActiveObservations` stale-while-revalidate caches with forced `refreshSessions` on all `sessions_changed`/ingestion invalidation paths; route-nav RCA log dev-gated; route entrance motion restricted to mobile swipe; keydown listener keyed off `pathnameRef`; orb first-frame entrance bloom (design-spec §7.2); removed hot-path computed-style route tracing and deferred scroll reset to next frame so nav commits no longer force synchronous layout on every route change; route transitions immediately unmount the prior page under an opaque shell loader and reveal only after the destination mounts and required UI chunks preload; Settings chunk preparation no longer waits on remote-model IPC, and its chunk loaders live outside the page module to preserve Fast Refresh boundaries; theme flips reuse the current root-token snapshot and detailed tracing is opt-in. Current run: tsc/build, tests, and lint-arch pass; repository-wide ESLint still reports warnings.
- **Voice pipeline brittleness RCA + fixes (vox.log trace):** removed dynamic-onset-gate upper clamp that collapsed SNR margin at loud ambient floors and un-biased noise-floor EMA updates (`segmenter.rs`, unit-tested); durable turn-identity gate on `ingest_chunk_with_intent` + `begin_synthesis_turn` so a barge-in-superseded TTS job can no longer stream or play back late chunks after the shared `cancel_flag` is re-armed (`playback.rs`, `tts/actor.rs`); RULE-HARN-03 dedup moved pre-side-effect in `transcript.rs`/`text.rs` so duplicates no longer advance turn IDs or render orphan UI bubbles. Desktop verify-targets (lint-arch + clippy -D warnings) 0/0.
- **Settings header reset placement:** moved `RestoreDefaultsButton` to left slot in full (wide) layouts; kept right slot for compact/mobile. tsc clean.
---

## 6. Backlog

See **[Phase 13 Backlog](file:///home/addy/projects/apps/vox/docs/plans/phase13/backlog.md)** for Cloud TTS quota management, Azure speech integration, and setup guide modal overhaul details.
