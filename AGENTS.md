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
- **Catalog Modal Master Plan & Interaction Desk Refactor:** Completed Tickets 1-10 of phase 13 catalog modal plan: unified backend dialect probing, catalog validation, `CatalogModalShell`, 3-stage responsive modal layouts, and 2-column `ProviderPillGrid` with in-situ key editing.
- **IPC Transport & Voice Stream Consolidation:** Unified `useVoiceSessionStream.ts` with zero blocking pulse-convergence delays; added `invokeWithTimeout`/`invokeWithRetry` to `transport.ts`; consolidated readiness polling into `waitForPipelineState`.
- **Provider Modal UX & Interaction Desk Polish:** Cleaned up desktop facet rail, card affordances, dashed custom server creation card, keyboard navigation, and lightweight plain-language connection guides.
- **SSOT TTS Provider Contract, URL Normalization & NVIDIA Key Alias Resolution:** Backend contract emits `tts_providers: tts_models.clone()` from manifest SSOT; deduplicated `nvidia_nim` presets; `normalize_server_url` handles missing schemes and typos; bidirectional NVIDIA key alias resolution across store, active selection, and hooks. Both targets compile cleanly (0 errors/0 warnings), 94/94 vitest pass, Vite production build clean.
- **Provider Catalog UI, Scroll Stability & Custom Server Polish:** Fixed scrollbar flicker by stabilizing provider pill action slot dimensions; replaced header status text with responsive status dot; prevented auto-close on Apply & Restart; hooked automatic draft discard on modal close; enabled custom server model browsing breadcrumb; removed redundant active badge in model cards; updated OpenRouter brand mark; preserved Edge TTS in compact desk; corrected Chatterbox Remote server requirement flag.
- **Ticket 1 (Backend SSOT & Alias Purge):** Removed duplicate `nvidia_nim` preset from `baseline_providers.json`; purged `nvidia`/`nvidia_nim` alias fallbacks in `contract.rs`, `presets.rs`, and `config.rs`; updated agentic test preset arg to `nvidia`. Both targets compile clean (0 warnings/errors); 131 lib tests pass.
- **Ticket 2 (Transactional IPC Batch Patch & Event Engine):** Added `apply_settings_patch_tree` in `mutation.rs`; added `apply_settings_patch` IPC command with single write lock, aggregate reload policy, one restart, debounced save, and `SettingsUpdated` emit; added `EngineRestartState` IPC event variant/payload and emitted restart state from `request_engine_restart` and restart worker; registered command in `lib.rs`. Targets compile clean; settings persistence tests pass.
- **Ticket 3 (catalogStore & engineStatusStore):** Created `app/src/store/catalogStore.ts` (catalog slice facade mirroring settingsStore) and `app/src/store/engineStatusStore.ts` (listens to `engine-restart-state`); migrated `useRemoteLlmProbing.ts` to catalogStore. tsc and 94/94 vitest pass.
- **Ticket 4 (settingsStore decomposition & alias purge):** Removed `getCloudProviderAlias` and alias fallbacks in `activeSelection.ts`, `useProviderSelection.ts`, `settingsStore.ts`, and consumers; added `selectActiveLlmProvider`; replaced commit diff loop with `applySettingsPatch`; updated tests. tsc and 93/93 vitest pass.
- **Ticket 5 (Interaction Desk & ModelsCard convergence):** InteractionConfigDesk deleted custom provider calc and useRemoteLlmProbing; adopted selectActiveLlmProvider and reads remoteModels from catalogStore. ModelsCard adopted selectActiveLlmProvider and remains sole LLM fetch orchestrator. tsc and 93/93 vitest pass.
- **Ticket 6 (RemoteCatalogSection dedup & flash fix):** Replaced inline model card JSX with RemoteModelCard (dynamic capabilities metrics); gated list pinning on committedRemoteModelId only; subscribed capabilitiesCache to catalogStore. tsc and 93/93 vitest pass.
- **Provider Logo Regression Fix & Scroll Diagnostics:** Diagnosed Ticket 7 regression: backend ships brand marks as raw `data:image/svg+xml;base64` blobs and the bundled norm formula scanned `logo`, so base64 fragments randomly matched `go`/`openai`/`google` checks (OpenCode mark on NVIDIA, wrong clouds). Reworked `ProviderLogo`: `norm` now scans only id+name+dialect; concrete `data:`/`http(s)`/`/` logos render directly via a tightened `<img>` fast path (loose `includes(".")` removed); generic openai check suppressed when an ollama/lmstudio/vllm token is present; added `resolved()` DEV-only branch logs (`[ProviderLogo]` console.debug) plus a `[Lenis]` content-height change logger in `createContainedLenis` to pin hover-flicker sources; tsc, build, 94/94 vitest clean.
- **Catalog Store Independent Cutover & Render Loop Elimination:** Converted `catalogStore.ts` to full standalone store owning remote model fetching, capabilities caching, and probing; stripped duplicate catalog fetch loops from `settingsStore.ts`; wrapped `selectActiveLlmProvider` in `useShallow` across `InteractionConfigDesk` and `ModelsCard` to eliminate unmemoized object infinite re-render loop (`Maximum update depth exceeded`). Both targets compile clean (0 warnings/0 errors), 93/93 vitest pass.
- **Settings Store Decomposition, Infinite Spinner Fix & Compact Catalog Restoration:** Extracted 440+ LOC from `settingsStore.ts` into `types/settings.ts` and `store/settingsFlags.ts`; synced `loadModelCatalog` to populate both `catalogStore` and `settingsStore` resolving the infinite spinner in `ModelsCard`; restored compact inline model card with hover capabilities `<Tooltip>` in `RemoteCatalogSection` preventing giant cards in compact desk; locked pill card dimensions (`h-[46px] min-h-[46px] max-h-[46px] overflow-hidden`) and switched `providerGridRef` to native scroll in `InteractionConfigDesk` eliminating hover scrollbar flicker; both targets compile clean (0 warnings / 0 errors), 93/93 vitest pass, Vite production build clean.

---

## 6. Backlog

(none — cleared via §5 entry "Light/Dark Semantic Color Audit".)