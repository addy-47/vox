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
> 1. Append **only the delta**:  to `docs/plans/<current_phase>/recent_work.md` under a `## Past Work (YYYY-MM-DD)` .
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
> 1. **Specification Divergence / Additions**: If an agent needs to implement, modify, or add behavior, commands, or schemas that diverge from or are not defined in the relevant spec, it MUST STOP and ask the user for approval. If approved, the agent MUST update the spec artifact FIRST before authoring or modifying code. Specifications must NEVER quietly drift from code.
> 2. **Code Divergence / Legacy Code**: If existing code implements nuances or legacy behaviors not defined in the spec, the agent MUST confirm with the user first before either pruning the code or updating the spec to capture the behavior.

### 4.4 Active Specifications Ledger
Authoritative system specifications reside in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/). Key target specs include **[storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md)** (SSOT filesystem, 3-way config decomposition, and runtime paths), **[events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)** (SSOT pipeline & 6-domain contracts), **[dictation-spec.md](file:///home/addy/projects/apps/vox/docs/specs/dictation-spec.md)** (dictation & OS output), **[harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** (LLM runtime), **[db-spec.md](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)**, **[memory-spec.md](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)**, **[ownership-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md)**, etc.

---

## 5. Current Phase Summary

> 📖 **Phase 13 (active):** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase13/recent_work.md)
> Phase 12 Archive: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md

- **Phase 13 scope:** Android platform preparation (re-homed from Phase 12 on 2026-10-09) plus the critical on-device feature set — remote control, disfluency ML model, speaker lock. Feature designs stay in `docs/plans/wip/`; the Android platform plan and the on-device defect plan are promoted into `phase13/`.
- **Dual-Target Native Toolchain & Release APK (`arm64-v8a`):** Automated compile gate `verify-targets.sh` verifies both desktop and Android targets at 0 warnings / 0 errors. Native toolchain links verified for `turso`/`libsql`, `llama.cpp`, and `ort` NNAPI. Release APK builds signed and runs on POCO redwoodin / Android 14.
- **Backend & Network Invariants Shipped:** Batch 8 delivered resumable model downloads with RFC-compliant partial content range verification and SHA-256 prefix hashing. Replaced brittle total deadline with per-read idle deadline. Added disk preflight checks and declared `RECORD_AUDIO` / `MODIFY_AUDIO_SETTINGS` permissions.
- **Unified AppHeader & Layout Architecture:** Implemented unified `AppHeader` across `main`, `compact`, and `mobile` layouts, eliminating `TopRightCluster.tsx` and duplicate page headers. Standardized top-right action sequence (`[Context]` → `Theme` → `Notifications` → `Help`). Standardized centered desktop search bar for History and Memory, while compact/mobile search lives in in-view list controls. Home orb centered strictly for handheld devices (`isRealMobileDevice()`).
- **Android Notifications & Wizard IPC Wiring:** Connected native notification permissions via `getNotificationPermission` and `requestNotificationPermission` in `MobilePermissionStep.tsx` with 5-state handling. Fixed Android cross-compile issues in `services/notifications/transient.rs` (Channel builder, async handle cloning, and prompt rationale pattern matching). Both `verify-targets.sh` and `pnpm tsc --noEmit` pass with 0 errors.
- **Web Search Tool & Streaming Chunker Stabilization:** Resolved repetitive filler loops by serializing outbound `tool_calls` in `OllamaTransport`, gating repeat fillers with `has_played_filler`, and making `spoken_filler` optional in specifications and tool schemas (`web_search`, `search_memory`). Fixed mid-stream decimal splitting (e.g., `1.5`) via lookahead guard in `ClauseChunker`. Animated `AdvancedOrb` across `Working` state by forwarding audio telemetry, eliminating frozen UI during tool evaluation. Dual-target `verify-targets.sh` and `pnpm tsc --noEmit` verified at 0 warnings / 0 errors.
- **UI Android Polish & Swipe Optimization:** Implemented velocity-aware directional swipe gesture transitions in `ResponsiveLayout.tsx` with GPU-promoted horizontal spring slide animations. Elided redundant `backdrop-blur-xl` on mobile `EdgeNav` in favor of solid background, eliminating permanent compositor re-blurs. Removed redundant `backdrop-blur-2xl` on `>=96%` opaque search dropdowns in `HistorySearchBar.tsx` and `SearchBar.tsx`. Added eager touch-intent chunk preloading on `onPointerDown` in `EdgeNav.tsx`. Removed artificial 400ms loader floor on warm visits in `Memory.tsx`. Dual-target `verify-targets.sh`, AST invariant checker, `pnpm test`, and `pnpm build` pass with 0 errors.
- **🔴 Android runtime BLOCKED — `android-device-defect-fixes.md` is the active plan.** The first full on-device onboarding run completed (667 MB STT download verified) and exposed **7 defects**. **D1 is critical and self-inflicted:** `ndk-glue` is absent from the graph, so `ndk_context::android_context()` panics on every call and **no audio can start** — Engage hangs forever because Tauri drops the IPC responder on unwind. Begin Android work at [android-device-defect-fixes.md](file:///home/addy/projects/apps/vox/docs/plans/phase13/android-device-defect-fixes.md) D1, not at the platform plan.
- **Launcher icon shipped white; HMR could not work.** The adaptive-icon background had drifted to `#fff` in the *generated* copy (source was correctly `#090D16`) — fixed and verified via `aapt2`. Separately, a **release** APK can never hot-reload (it embeds `frontendDist` and never contacts `devUrl`); use `scripts/android-dev.sh debug` then `dev`. Both recorded in the style guide as Pitfalls 20–21.
- **Style guide now covers Pitfalls 1–22** (was 7), including the traps that cost the most time: a backend panic becomes a **silent frontend hang** on Tauri, and a `SAFETY` comment asserting an initializer must be checked against `Cargo.lock`, not intuition — the false `ndk-glue` claim survived a clean dual-target compile.
- **Android Defect Fixes Delivered (Batches 1–4, D1–D6):** Unblocked native audio via explicit JNI `initAndroidContext()` / `ndk_context` initialization in `MainActivity.onCreate` with fail-soft safety (D1). Bound frontend IPC promises with timeout and cancel affordance (D1.3). Wired notification launcher registration before `STARTED` and rendered `unavailable` permission states honestly (D2). Fixed accordion category sizing to strictly respect selected models (D4). Enforced truthful multi-model download completion, sequenced manifest fetch before runtime report, deleted fake interval simulation, surfaced failed transfers, and enabled retry (D5). Memoise `footerConfig` and scoped `MobileStepFooter` to `transition-colors` without scale flicker (D3). Redesigned `MobileReadyStep` with mobile-first card hierarchy and >=48px touch targets (D6). Dual-target compile gate and vitest suite pass at 0 warnings / 0 errors.
- **Mobile UI Refactors, Telemetry & Scrubber Stabilization:** Standardized ExpandableList modal dimensions, auto-close, icon-only mobile controls and single-line settings footer (Items 1-2). Wired responsive viewport subscriptions into AppHeader and decoupled EdgePanels into stage portal root (Items 3-7). Added loader floor guards and preloading for settings chunks (Items 4-5). Fixed multi-platform procfs/sysinfo CPU/RAM telemetry for Android and cross-targets (Item 9). Shipped transparent vector monochrome notification icon (Item 10). Enabled touch tap-to-toggle tooltips (Item 11). Equalized orb-to-status and orb-to-actions spacing on mobile (Item 12). Integrated `ChapterScrubber` on mobile/compact Home (Item 13). Dual-target compile gate and vitest pass at 0 warnings / 0 errors.
- **UI Proportions, Close Alignment & Tooltip Refinements:** Aligned EdgePanel minimal header close button with AppHeader trigger buttons across desktop and mobile. Enforced 40/60 vertical gap geometry for status capsule and engage controls on Home. Constrained compact ChapterScrubber to 50% orb height at the right viewport edge. Added hover tooltips to modal apply/restart and close triggers. Verified via `tsc`, vitest, and dual-target `verify-targets.sh`.
- **HMR Debug APK Installation & Scrubber Chronology:** Installed HMR-enabled debug APK on POCO device and confirmed live websocket connection to `http://tauri.localhost/@vite/client`. Reverted Home button offsets to baseline. Reversed ChapterScrubber order to show latest turns first with persistent top alignment. Attached mount telemetry logging. Dual-target `verify-targets.sh` passed at 0 warnings / 0 errors.