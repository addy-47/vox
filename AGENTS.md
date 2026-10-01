# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target. [Note: SKip this for tasks that lack accountability or impact on future agent work]
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **110 lines** (the warning threshold before the 175-line ceiling)
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

### 4.3 Specifications, Behavioral Contracts & Non-Drift Hook [MANDATORY]

> 🛑 **MANDATORY SPEC ALIGNMENT HOOK:**
> Every agent working on Vox must adhere strictly to the approved specifications.
> 1. **Specification Divergence / Additions**: If an agent needs to implement, modify, or add behavior, commands, or schemas that diverge from or are not defined in the relevant spec, it MUST STOP and ask the user for approval. If approved, the agent MUST update the spec artifact FIRST before authoring or modifying code. Specifications must NEVER quietly drift from code.
> 2. **Code Divergence / Legacy Code**: If existing code implements nuances or legacy behaviors not defined in the spec, the agent MUST confirm with the user first before either pruning the code or updating the spec to capture the behavior.

### 4.4 Active Specifications Ledger
Authoritative system specifications reside in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/). Key target specs include **[storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md)** (SSOT filesystem, 3-way config decomposition, and runtime paths), **[events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)** (SSOT pipeline & 6-domain contracts), **[dictation-spec.md](file:///home/addy/projects/apps/vox/docs/specs/dictation-spec.md)** (dictation & OS output), **[harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** (LLM runtime), **[db-spec.md](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)**, **[memory-spec.md](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)**, **[ownership-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md)**, etc.

---

## 5. Phase 12 — Recent Work Summary

> 📖 **Full History:** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md) | Phase 11 Archive: [phase11/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)

- **TTS Stack, Storage Architecture & Backend Engine SSOT:** Trait-level TTS model contracts, 6-tier POSIX layout in `paths.rs` (ZBC), 3-way JSONC settings decomposition with 0600 provider keys, and backend-owned reload policy single-runner gate.
- **Frontend Impeccable Audit & Data-Integrity Remediation:** Shipped comprehensive 60-file UX/audit remediation (crossfade routes, WebGL gating, reduced motion, zero data loss browse-mode in version nav, restrained Docs suggestion styling, lag elimination).
- **Semantic Structured Personal Memory & Staging Mirror:** Canonical JSON model (`model.rs`), ID-addressed operations, Google Docs inline suggestion review with instant acceptance/auto-finalization, staged/pending filtering, and whole-memory synthesis.
- **Memory Ingestion Lifecycle & Evaluation Remediation (2026-10-01):** Replaced quiet-debounce observer with event-driven lifecycle (boot sweep, session-start cancellation, session-end sweep) and complete queue draining (eliminating Stage 2 batch starvation); refactored personal memory regeneration to re-synthesize strictly from already-integrated facts (`type = 'personal' AND status = 'integrated'`) without touching active facts; added re-synthesis action in `Memory.tsx`; calibrated multi-stage eval harness with explicit `[FACT-XX]` tags and OpenRouter fast judge routing; verified end-to-end via OpenCode QA Auditor subagent.
- **Frontend Audit Sweeps 2 & 3 — Data Integrity, Restraint & Honest Instrumentation (2026-10-01):** Sweep 2 kept: version nav is preview-only (the 500ms debounce that wrote to the DB on *browse* is gone; replaced by an explicit "Viewing revision N" + Restore bar); notification fetch failure renders an honest `role="alert"` + Retry instead of "all caught up"; suggestion diff is two-tone (red strike, accent insert, accent tick, red cross, rejected rows desaturated); the emerald `CommitSuccessView` takeover became a slim `CommitBanner`; staging-card lag fixed by removing `transition-all duration-500` + whole-card `opacity-50 pointer-events-none` on a `backdrop-blur` element and moving auto-finalize out of a `setState` updater; deleted `EdgeNav`'s one-frame spinner; `SearchBar` combobox role + lowercased index; `useVoxFootprint` NaN guard; `UnderlineInput` real `<label>`; `ViewSelector` adopted; History arrow-keys can no longer destroy an open transcript. Sweep 3 kept: onboarding-failure no longer inescapable-routes into first-run (3x backoff then an explicit unreachable screen); wizard webview `destroy()`d after setup + no double-mount (webview-label probe, `WizardStandIn`).

- **Navigation & Overlay Motion — Post-Regression Correction (2026-10-01):** **Four performance "optimizations" shipped in the audits were reverted after they made the app visibly worse, and are now recorded in `performance-memory-optimizations.md` as anti-patterns.** **(1)** `.no-blur-subtree, .no-blur-subtree * { backdrop-filter: none !important }` (Drawer/EdgePanel) is DELETED — the `*` descendant selector forces style invalidation + re-raster of every blurred descendant on toggle *and* untoggle, which on llvmpipe cost more than the blur it saved and produced a "blur snaps on a second later" artifact on every drawer open. **Never suppress backdrop-filter by descendant selector; if a blurred element is too expensive to move, promote the moving element instead.** **(2)** `<VoxOrb paused={isAnyOverlayOpen}>` is DELETED — it toggled the WebGL rAF loop on every panel open/close, so the orb cancelled mid-frame and visibly jumped. **(3)** The always-on profiler retention sampler is DELETED — walking the full process tree 2.5s after every route unmount regressed navigation; retention columns stay honestly "Not measured". **(4)** `AnimatePresence mode="wait"` around `<Routes>` is DELETED and route chunks are preloaded again (deferred via `requestIdleCallback` in `App.tsx`): `mode="wait"` holds the incoming route until the outgoing exit completes, producing exit -> blank stage -> enter, and combined with cold lazy chunks it exposed the full-stage Suspense fallback as a stall. **Navigation is now one thing: no exit animation, incoming page fades up over 200ms.**



---



