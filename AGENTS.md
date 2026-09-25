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

- **Agentic runtime & provider SSOT shipped:** Full tool taxonomy, reentrant harness loop, session gating, provider wire policy mapping, and 122 integration tests with 73% mutation kill rate.
- **Realtime, memory, and UX foundations:** Streaming TTS/metrics, realtime tool translation, session continuation, boundary compaction fixes, and deterministic frontend invariant/stress tooling.
- **Memory pipeline evaluation & semantic audit:** Built 14-case eval suite, executed operational baseline (Case 1), audited memory grounding overclaims, hardened extraction prompts, and stabilized `evals/memory_pipeline_eval.rs`.
- **Web search tool & `nexuss` published:** Built and optimized multi-engine search engine (`nexuss v0.1.1` on crates.io), wired into `web_search` tool with adaptive fanout quorum and hybrid ranking (2.16s latency), integrated via Cargo, and verified across all 6 Seam 21 tests and live Gemini eval.
- **Memory manual-test groundwork:** Audited the latest 14-case memory-pipeline run, recovered consolidated personal-fact anchors from the Turso sidecar, added full local `search_memory` candidate/filter/ranking/final-observation diagnostics, and added `docs/plans/phase12/search-memory-manual-test-queries.md`.
- **Eval DB swapped for UI testing:** Preserved the prior app database as `~/.vox/vox.db.bak-20260925-122700` and copied the 14-case eval database plus Turso sidecars into the main app location.
- **Remote CPU Ollama endpoint recorded:** Verified TCP/API access to `100.67.98.126:11435` and documented it in `temp/server.txt` for CPU-only memory-eval runs.
- **Provider-agnostic LLM runtime & cancellation fix:** Gated KV cache warmup to `ProviderKind::Embedded` only so remote HTTP endpoints never receive synthetic `[WARMUP]` requests or lock up the worker loop; connected cancellation cleanly; standardized `reasoning_off` on unknown presets (`reasoning_effort: "none"`); implemented deterministic protocol-based server discovery (`discovery.rs`) supporting Ollama, vLLM, llama.cpp, and LM Studio without hardcoded ports; updated `check_provider_health` to return discovered dialect to frontend and dynamically badge detected server runtime.
- **Settings panes & Working Memory desk redesign:** Added shared `SettingsTabPane`/preset primitives and rebuilt LLM, VAD, ASR, and TTS settings subtabs as centered full-bleed panes; replaced the WM budget desk with a 2-tab `WorkingMemoryConfigDesk` (Web Search toggle with animated CSS-3D globe, Content Share presets without fill bar), removed the Turso badge, and gated `working_memory.web_search_enabled` in spec + frontend copy/store.
- **Persona default preview, globe cleanup & engage flicker fix:** Reordered `PersonaCard` view tabs to place Preview first and set as default; removed orbiting satellite dot and radar sweep from `WebSearchGlobe`; eliminated engage transition flicker by preventing `waitForState` from broadcasting stale intermediate states over push IPC events.
- **Two-column desk layout for LLM settings with 2x2 grids & SVGs:** Rebuilt `SettingsTabPane` into the two-column config desk pattern (left title/description, right 2x2 preset grid); created `RemoteComputeGraphic` and `ManagedContextGraphic` custom SVGs for optionless remote/cloud tabs so visual structure is identical across all categories.
- **Settings desk layout refactor & transliteration vector toggle:** Enforced strict 65-35 column split in `SettingsTabPane`; pruned verbose descriptions in `settingsCopy.ts` down to single-sentence copy; eliminated redundant value badges duplicating active controls; locked compact preset button heights (32px) to eliminate vertical stretching; rebuilt optionless graphics (`RemoteComputeGraphic`, `ManagedContextGraphic`) into pure wireframe illustrations without pill containers; fixed broken voice & speed layouts in `TtsVoiceManager` by mounting controls in the 35% right slot; designed and wired interactive `TransliterationToggle` SVG (`अ` stationary when disabled, `अ → a` mapping vector when enabled).
- **Clock SVG schedule control in PersonalMemoryConfigDesk:** Replaced the Manual/Daily `SegmentedControl` pill toggle with a line-style clock SVG icon-as-toggle (same pattern as `WebSearchGlobe`) — muted with MANUAL badge when inactive, accent-colored with float animation and clickable time input badge (e.g. `09:00 AM`) when Daily is active.
- **Design spec & style guide hardened against faux-pill copy & boxed SVGs:** Updated `docs/specs/design-spec.md` (Sections 4.3, 5, 5.1, 5.2) and `.agents/rules/frontend-style-guide.md` (Section 5) enforcing strict invariant that pill styling (`rounded-full` + bg + border) is exclusively for interactive buttons/controls and strictly banned on static copy/labels; prohibited redundant title value badges; codified the frameless clean vector SVG pattern.
- **ModelStatusOverlay scoped to Home & Settings only:** Gated bottom-right idle `ModelStatusOverlay` to `isHome` and `isSettings` in `ResponsiveLayout`, preventing collision with memory graph legends on `/memory`.
- **ModelStatusOverlay polish & 2-step restore in TopRightCluster:** Removed the redundant `" · Voice"` text suffix from TTS in `ModelStatusOverlay`; increased all typography and icon dimensions by +1px for enhanced readability; moved `RestoreDefaultsButton` into `TopRightCluster` scoped to `/settings` only so `ModelStatusOverlay` retains an identical bottom-right position across views; refactored `RestoreDefaultsButton` from timer-based auto-revert into a 2-step inline confirmation with unboxed accent copy, red tick (✓), and grey cross (✕) controls.
- **MemoryLegendOverlay aligned with ModelStatusOverlay:** Matched `MemoryLegendOverlay` bottom-dock positioning (`bottom-4 right-4 z-40`), padding (`px-3 lg:px-4 py-2.5`), and `BottomDockFeather` in `Memory.tsx` to align exactly with `ModelStatusOverlay`; scaled legend typography to 12px font-mono, jewel dots to 2x2, and column gaps to `gap-x-6` for identical scale and visual weight across route transitions.
