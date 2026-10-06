# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target. [Note: SKip this for tasks that lack accountability or can impact a future agent's work]
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
- **Design System & Performance Foundation:** Theme flip rebuilt on rAF token interpolation with scoped blur suspension and visibility-paused loops (Invariants 16–25), compact-viewport SSOT with the centered `Modal` primitive, and the stroke-style `VoxLogoLoader` replacing particle/orb loaders.
- **Frontend UX Audit → Implementation (Tier 1–3):** App-wide dual audit (`frontend-ux-audit-appwide.md`, **45,101 lines**) plus its follow-on sweeps closed the data-loss, silent-failure and dead-instrument findings across Memory, History, Home, Settings, Monitoring, tray and wizard — including notification fetch honesty, staging redraft, profiler truthfulness, engine restart visibility, and save-gated settings commit flow.
- **ML & Benchmark Gates:** Disfluency Gate 2 passed on reshaped data with in-training validation (`deberta-v3-base` 1.734% harmful span rate, 0.0% over-deletion); dictation-cleanup benchmark ledger codified in `dictation-cleanup-engine.md` §7 with DeBERTa/LFM2.5/Laya checkpoints synced to `submodules/vox-models/disfluency/`.
- **First-Run Wizard Hardening (2026-10-06):** `main` webview made lazy (removed from `tauri.conf.json`) so first run no longer double-windows; wizard white-flash root-caused and reveal delegated to post-paint `revealWizard()`; full de-generic icon pass onto Vox's own vocabulary; render/animation cost reduced (memoized rows, `scaleY` bars, stable callbacks); real error screen, honest completion status, `prefers-reduced-motion`, ~33 literals centralised and all copy rewritten from a first-run user's POV.
- **Regression Discipline:** 3 behavioural vitest suites added through real production seams (review model, notification logic, category tokens) with reported testability gaps instead of faked coverage; memory integration failure RCA'd to a vendor ignoring `response_format` and gated with `gate_on_structured_output_support`.
- **Personal Memory Lifecycle & Staging UI Polish (2026-10-06):** Replaced raw suggestion text with semantic token badges (accent/danger/amber pills) and bolder section headers; added scoped Lenis smooth scrolling to suggestion review; eliminated redundant copy in staging header and review footer; introduced dual-orb harmonic synthesis canvas; decoupled veil animation from consolidation start to post-decision commit; centered unextracted turn confirmation replacing action hub with dynamic enable-and-wait / proceed-anyway actions based on ingestion state.
- **Theme Flip Home/History Smoothness & LLM Catalog Warm Preload (2026-10-06):** Root-caused theme transition stalls on Home (TBT 205–255ms, worst frame 143–181ms) and History (TBT 77–102ms) versus clean 60fps on Memory: `AdvancedOrb` observed `style` on `:root`, calling `getComputedStyle()` twice per frame during token interpolation; `AmbientBackground` fired React re-renders via `MutationObserver(data-theme)`; `OrbitCarousel` ran concurrent rAF frame updates during flips. Shipped: parked `AdvancedOrb` style recalcs during flips with clean settlement subscription; removed React state from `AmbientBackground` update loop; paused `OrbitCarousel` during `getThemeTransitioning()`; eliminated cold fetch delay in Models card by caching remote models in `settingsStore` and prewarming catalog via `requestIdleCallback` on Settings mount.
- **Boot Loader Cross-Fade & Remote Models Deserialization (2026-10-06):** Smoothed initial boot transition by removing full-screen `scale: 0.98` composite churn in `App.tsx` and replacing the sudden unmount in `Home.tsx` orb loader with an `AnimatePresence` 500ms opacity cross-fade; fixed empty LLM catalog by enforcing tagged `kind: "server" | "cloud"` on provider payloads to satisfy Rust serde deserialization in `list_llm_models`.
- **Token-Driven Ambient Backdrop Gradient (2026-10-06):** Eliminated the synchronous backdrop snap on Home/History where `TitleBar`, `EdgeNav`, and `TopRightCluster` flashed during theme transitions: replaced static `[data-theme='light'] .amb-base` stylesheet rule with three interpolated gradient stop tokens (`--amb-stop-0`, `--amb-stop-1`, `--amb-stop-2`) registered in `FLIP_TOKEN_KEYS`, allowing `.amb-base` to fade smoothly across the 200ms transition in lockstep with foreground surfaces.
- **History Clock & Orbit Cards Theme Interpolation (2026-10-06):** Resolved stepped snap during theme flips in History by tokenizing `.clock-hub`, `.clock-pill`, and `.orbit-card-surface` gradients, borders, and drop/inset shadows into `FLIP_TOKEN_KEYS` (`--clock-hub-stop-0..2`, `--clock-pill-bg`, `--glass-surface`, `--glass-deep`, `--ghost`, and `--*-shadow/inset/border-alpha`); unified radial gradient geometries and enabled cross-alpha/rgba interpolation in `lerpToken` so surfaces fade synchronously with the ambient backdrop.
- **Settings Theme Toggle Decoupling & Responsive Chrome Placement (2026-10-06):** Decoupled theme flipping from `AppearanceCard` by removing redundant `SegmentedControl` theme switcher to prevent multi-card DOM recalc load; moved `ThemeToggleButton` to top chrome on Settings: positioned at top-left (`top-4 left-5 z-[60]`) on desktop/wide view (≥ 1024px) matching Home/History/Memory, and automatically repositioned to the left of `RestoreDefaultsButton` in `TopRightCluster` on compact view (< 1024px).
- **Orb Loader Transparency & Route Cross-Fade Transitions (2026-10-06):** Resolved square white box flash around the orb during Settings-to-Home navigation by removing opaque `bg-[rgb(var(--background))]` from `Home.tsx` orb loader and applying `bg-transparent pointer-events-none rounded-full`; added route-level hardware-accelerated cross-fade (`AnimatePresence mode="wait"` with 160ms cubic-bezier opacity transition) in `ResponsiveLayout.tsx` across `Outlet` changes to eliminate synchronous navigation frame drops; guarded `EdgeNav.tsx` active links to prevent redundant navigations.
- **History Viewport Measurement & Flash Elimination (2026-10-06):** Root-caused 120ms background flash of `HistoryListView` on History page mount: `dimensions` initialized to `{width: 0, height: 0}`, causing `isOrbitViewport` to evaluate `false` until a debounced `ResizeObserver` fired; resolved by initializing `dimensions` synchronously to `window.innerWidth/innerHeight` and measuring `containerRef` immediately in `useLayoutEffect`, rendering the orbit stage from frame 0 with zero intermediate list flash.
- **History Route Flicker Elimination & Session Cache (2026-10-06):** Eliminated full-UI flicker on History navigation caused by uncached async session fetching, an 85% opacity backdrop-blur-2xl overlay, and an exit delay in `ResponsiveLayout.tsx`: added in-memory session caching in `historyService.ts` and preloaded sessions on idle; initialized `useHistory` synchronously from cache; replaced full-screen frosted glass overlay with transparent loader; removed `mode="wait"` exit delay in `ResponsiveLayout.tsx` for synchronous lockstep route rendering.
- **Manual Delete Suggestion Insert for UI Review (2026-10-06):** Inserted `delete_block` pending revision (`rev_1791281300016_24ot`, `base_memory_version=17`) targeting `blk_1a10cff7815_555f` (rubber-duck block in About & Core Focus) in `~/.vox/data/db/vox.db` via `tursodb` for Google-Docs staging review visualization alongside 3 existing pending suggestions.
- **Manual Update Suggestion Insert for UI Review (2026-10-06):** Inserted `update_block` pending revision (`rev_1791284169973_ajg3`, `base_memory_version=17`) targeting `blk_1a10cff7815_2777` ('Answers ship it' block in Communication & Interaction) with appended clause '— unless it involves database migrations' in `~/.vox/data/db/vox.db` for word-diff staging review visualization alongside 4 existing pending suggestions.
- **Capability Probe Transparency Overhaul (2026-10-06):** Root-caused the fake-benchmark symptom to `useRemoteLlmProbing.ts` referencing undeclared `mountedRef`/`setRemoteModels` (ReferenceError strands every row in `testing` while stale cache renders as fresh success). Replaced the fabricated `get_weather` tool probe with a real `web_search` declaration via `ToolRegistry`+`canonical_tools_json`, per-transport detection reusing production parsers, and `(index,id)` tool-call accumulation (spec §4.1). Tri-state `Support` replaces bool flags; every sub-probe records a `ProbeCheck` (spec §6.4); embedded values come from manifest + new `catalog/gguf.rs` header parser under `DeclaredStatic` provenance (spec §6.1); cloud cache-key mismatch fixed; `session.rs` only disables tools on confirmed `No`; new read-only `get_model_capabilities_cache` stops Settings mount firing a 12s live probe; `responses.rs` now serialises/parses tools; removed redundant modal `testing` tile, vision-name filter, dual-cased IPC args, and 128k context fallback; spec + ipc-spec amended first. Unverified: user runs `cargo`/ `pnpm` checks manually.
- **Probe Follow-ups (2026-10-06):** Fixed boot panic from `spawn_catalog_sync` (`tokio::spawn` with no runtime in Tauri setup → `tauri::async_runtime::spawn`). Killed the false "no catalog entry" warning (baseline miss no longer recorded; UI warns only on `failed`, never `skipped`). Breadcrumb between catalog and cloud-providers list now renders only for `kind === "cloud"`. Metric tiles/cards restyled onto accent-derived tokens (`--accent` icons, `--notif-models` values, stronger selected states) replacing hardcoded emerald/sky/purple/indigo/blue; probe failures moved to a warning-triangle icon by the model name with details in a hover tooltip.
- **Probe Tooltip & Selection Polish (2026-10-06):** Removed the accent wash on selected catalog cards/rows (selection now border+ring+shadow only). Fixed warning tooltip layering: `z-50` in the caller's className was silently downgrading the Tooltip portal default `z-[9999]` via tailwind-merge, burying the popup under the modal overlay; dropped the override and enlarged the icon hit area (`p-1 -m-1`).
- **Corrupt Capability Cache Quarantine & Tooltip Overhaul (2026-10-06):** Implemented `quarantine_corrupt_cache` timestamped backup move in `probe.rs` to prevent data loss and rebuild cleanly; wired `cache_error` propagation into `ModelProbeResult` and `CapabilityCacheRead`; resolved TypeScript compile errors in `ResponsiveLayout.tsx` and `useRemoteLlmProbing.ts`; overhauled `probeWarningLabel` and `Tooltip` in `LlmCatalogView.tsx` from cramped 2-column 59px right-column layout to a stacked, comfortable 300px layout with accented monospace error blocks; purged stale `cache_read: failed` check from `model_capabilities.json`.



