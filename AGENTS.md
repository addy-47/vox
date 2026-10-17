# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target. [Note: SKip this for tasks that lack accountability or can impact a future agent's work]
>
> **Step 2 — Only when approaching 110 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **110 lines**)
> 1. Migrate the section 5 delta to `docs/plans/<current_phase>/recent_work.md` under a `## Past Work (YYYY-MM-DD)`.
> 2. Replace Section 5 in `AGENTS.md` with a compact 3–5 bullet summary of only the highest-level milestones.
> **This is the complete hook. Nothing else is mandatory on every task.**

---

## 2. Workspace Directory Map

| Path                      | Purpose                                             | Rules                                                                                                 |
| ------------------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `app/src-tauri/src/`      | Purpose Rust source                                 | No test logic. No benchmarks.                                                                         |
| `app/src-tauri/tests/`    | Integration tests                                   | Named `<feature>_test.rs`. Tests public API only.                                                     |
| `app/src-tauri/benches/`  | Performance benchmarks                              | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`.                                   |
| `app/src-tauri/scripts/`  | Build & verification scripts                        | `verify-targets.sh` is the compile gate (see §3.0). Env SSOT for the NDK toolchain.                 |
| `.agents/rules/`          | Role-specific agent instruction files               | Read relevant file before acting in that role. `android-style-guide.md` is mandatory for Android-touching work (see §4.2). |
| `docs/plans/`             | Architecture specs and phase plans                  | Source of truth for specs. Do not contradict. `phase13/` is active.                                  |
| `docs/features/`          | Implemented feature ledgers                         | Update after completing features.                                                                     |
| `sandbox/`                | Scratch space for experiments, evaluations, scripts | Non-production code. Results in `sandbox/results/`. Datasets in `sandbox/datasets/`.                  |
| `temp/`                   | Ephemeral runtime files: logs, raw LLM outputs      | `temp/.env` (API keys). `temp/server.txt` (remote GPU server creds). Not versioned.                   |
| `submodules/`             | Git submodules                                      | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`, `nexus-rs`.       |
| `~/.vox/models/`          | Local model weights                                 | Canonical manifest: `~/.vox/models/models_manifest.json`.                                             |
**Remote GPU server:** `root@[IP_ADDRESS]` (creds in `temp/server.txt`). Ollama . **Never kill running server processes.**

---

## 3 Execution & Testing Invariants (All Agents)

0. **Compile Gate — always use the script, never a bare `cargo check` on a cross target:**
   ```bash
   cd app/src-tauri && ./scripts/verify-targets.sh
   ```
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
> 🛑 **MANDATORY PRE-EXPLORATION HOOK:**
> - Before any codebase exploration, architecture lookup, graph query, or broad search, read `.agents/rules/codebase-memory-mcp.md` and follow its graph-first workflow.
>
**Local Turso Database CLI (`tursodb`):** For inspecting or querying the local Turso SQLite database (`~/.vox/vox.db`), use the native `tursodb` CLI tool:
   ```bash
   tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
   ```

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY PRE-MODIFICATION HOOK:**
> - You MUST read the corresponding style guide and engineer rule file for the specific area you are working on located in .agents/rules/.
> - **If the change touches anything that must compile on Android** — `cpal`, tray/menu, dictation, `utils/paths.rs`, model loading, Cargo.toml target tables, or any `cfg(` — you MUST also read [`.agents/rules/android-style-guide.md`](file:///home/addy/projects/apps/vox/.agents/rules/android-style-guide.md). Android support is **incomplete**; several confirmed traps there have already caused false "done" reports.

### 4.3 Specifications, Behavioral Contracts & Non-Drift Hook [MANDATORY]

> 🛑 **MANDATORY SPEC ALIGNMENT HOOK:**
> Every agent working on Vox must adhere strictly to the approved specifications.
> 1. **Specification Divergence / Additions**: If an agent needs to implement, modify, or add behavior, commands, or schemas that diverge from or are not defined in the relevant spec, it MUST STOP and ask the user for approval. If approved, the agent MUST update the spec artifact FIRST while adhering to the `/create-spec` skill ,before authoring or modifying code. Specifications must NEVER quietly drift from code.
> 2. **Code Divergence / Legacy Code**: If existing code implements nuances or legacy behaviors not defined in the spec, the agent MUST confirm with the user first before either pruning the code or updating the spec to capture the behavior.

### 4.4 Active Specifications Ledger
Authoritative system specifications reside in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/). Key target specs include **[storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md)** (SSOT filesystem, 3-way config decomposition, and runtime paths), **[events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)** (SSOT pipeline & 6-domain contracts), **[dictation-spec.md](file:///home/addy/projects/apps/vox/docs/specs/dictation-spec.md)** (dictation & OS output), **[harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** (LLM runtime), **[db-spec.md](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)**, **[memory-spec.md](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)**, **[ownership-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md)**, etc.

---

## 5. Current Phase Summary

> 📖 **Phase 13 (active):** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase13/recent_work.md)
> Phase 12 Archive: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md)
- **Catalog Modal Master Plan & Interaction Desk Refactor:** Completed Tickets 1-10 of Phase 13 catalog modal plan: unified backend dialect probing, `CatalogModalShell`, 3-stage responsive modal layouts, and 2-column `ProviderPillGrid` with in-situ key editing.
- **State & Catalog Store Decomposition (Tickets 1–6):** Pruned alias fallbacks across backend/frontend; added transactional IPC batch patch; separated `catalogStore.ts` and `engineStatusStore.ts`; resolved re-render loops via `useShallow`.
- **UI Scroll Stability & Custom Server Resolution:** Fixed scrollbar hover flicker with stable scroll containers; unified custom server resolution across draft/committed settings; prevented stale model leakage on provider switch.
- **Cloud TTS Streaming & Voice Preview:** Implemented Google Cloud TTS (streaming bidi gRPC) and Azure AI Speech (REST SSML 24kHz) preview synthesis; live voice catalog integration in Turso DB; dual-target compile gate passed cleanly.
- **Acoustic Echo Cancellation (AEC) & Full-Duplex Barge-In:** Integrated pure-Rust WebRTC AEC3 (`sonora = 0.2.0`) with lock-free playback reference tap and 2nd-order Butterworth anti-aliasing decimation filter; rewired live capture through AEC worker thread; deleted legacy speaker ducking; consolidated barge-in test suite achieving 32.58 dB ERLE.
- **Frontend test-suite rebuild (Phase 13, in progress):** Batch 0 done — `app/vitest.config.ts` (jsdom env, v8 coverage report-only, no thresholds), devDeps `@vitest/coverage-v8` + `@testing-library/user-event`, `test:unit`/`test:dom`/`test:coverage` scripts, shared harness `app/src/test/harness/transportStub.ts` + `settingsFixtures.ts`; full suite green under jsdom. Batch 1 done — deleted 5 fabricated sort tests + ProviderLogo tombstones (`activeCardPrimacy.regression.test.ts`); extracted primacy sort into `app/src/shared/lib/activeSelection.ts` (`promoteActive`), wired into `ModelModal.tsx`/`ProviderModal.tsx`, new `activeSelection.regression.test.ts` (15 tests, 2 mutants killed); `transport.test.ts` rewritten (10 tests, 4 mutants incl. timer-leak); `settingsCommit.regression.test.ts` rewritten on shared harness with correct atomic-rejection semantics (backend `apply_settings_patch_tree` is atomic — old test asserted mock echo); `ProviderLogo` `any`→`ReactElement`; `invariants.test.ts` deleted → `scripts/lint-arch.mjs` per-field Invariant 7, dead `ProviderCaps` fields (`voices`, `clone`) on pending-deletion allowlist awaiting user approval (spec-alignment question); `eslint.config.js` repaired (browser globals, `no-undef` off for TS, transport.ts exception) — repo still carries 89 pre-existing genuine lint errors (36 files, incl. conditional hooks in TitleBar/AppHeader) NOT introduced this session; out of scope, not fixed. Batch 2 done — Tier-1 seams validated via `/mutate`: `settingsFlags.regression.test.ts` (16; restart classification via real `get_settings_contract` loader; forward-wire pin), `overlayStack.regression.test.ts` (12; FILO/idempotence/outside-click), `catalogStore.regression.test.ts` (13; fetchKey cache/stale-clear/coalescing/cloud guard/failure settle), `pipelineService.regression.test.ts` (17; poll/deadline/InvalidState recovery/fast-path), `useVoiceSessionStream.regression.test.ts` (14; dictation isolation, 30ms throttle, turn boundaries, hydration window, unmount); all typecheck-clean. Batch 3 started, not complete — `notificationPanel.regression.test.tsx` written but FAILING (fixture shape vs real `NotificationRecord`: resolution comes from `metadata` JSON; store's `markRead` effect needs shared transport stub in beforeEach); file deleted to keep suite green — resume with transport stub + correct fixture.

---

## 6. Backlog

See **[Phase 13 Backlog](file:///home/addy/projects/apps/vox/docs/plans/phase13/backlog.md)** for Cloud TTS quota management, Azure speech integration, and setup guide modal overhaul details.
- **Frontend lint tooling audit (2026-10-17):** Enabled type-aware linting (`parserOptions.projectService: true`, typescript-eslint 8.59.1); added `no-floating-promises`, `no-misused-promises`, `no-unsafe-*`, `no-unnecessary-condition`, `prefer-nullish-coalescing`, `require-await`, `await-thenable`; expanded react-hooks to v7 `recommended-latest` (14 rules). Baseline: 88 errors / 0 warnings → 275 errors / 1027 warnings (all pre-existing defects, zero regressions). `lint:arch` and `tsc --noEmit` remain green. Validation test written to `src/test/lint-rules-validation.ts` (3 errors, 2 warnings proven). Full issue ticket at `.scratch/lint-audit/frontend-lint-issues.md`.