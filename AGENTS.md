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

- **Web Search Nexus 0.1.3 (published):** `nexuss 0.1.3` on crates.io, Vox on registry dep. DDG Lite-first, SearXNG + 7 gated verticals, pool 6, failure taxonomy + coverage/thin gates + quorum fix; 48-query evals + 5 QA rounds; gate clean, suite 147/147. Open: adv_02 live proof, one vertical grounding win.
- **Unified Config Decomposition & Model Provenance (ZBC):** Decomposed runtime settings into 3 atomic JSONC files (`settings.jsonc`, `providers.jsonc`, `agent.jsonc`) with isolated multi-server arrays and API keys; moved per-model runtime parameters to Turso SQLite (`models` table, schema v14) preserving parameters across model changes; dedicated `services/realtime/config.rs`; eliminated legacy compatibility aliases; verified dual-target gate at 0 warnings/errors.
- **Interaction Pipeline Cards & Persistent Providers Refactor:** Redesigned Level 1 stage cards ("Listening", "Reasoning", "Speaking") with clean active provider line with zero badge/pill containers; persistent ProvidersHeader with inlineCustomActions ([✓ Save] / [✕ Cancel]); integrated custom dialect health probing with non-hijacking fallback prompts; pure SVG vector logos for 35+ providers; 61/61 vitest tests passed; desktop clippy gate passed at 0 warnings/errors.
- **Provider Pill Horizontal Layout & Clearance Architecture:** Re-architected inline provider pills to horizontal layout placing title and subtitle directly next to 24px icon with right-aligned compact `+` connect action; reduced pill height to 46px eliminating vertical clipping; added bottom padding clearance for floating modal trigger; verified 61/61 vitest tests and 0-warning dual-target gate.
- **Brand SVG Normalization, Compact Provider Header & Realtime Desk Matching:** Switched ProvidersHeader "+ Custom" to compact "+" on mobile; installed official vector SVGs for OpenCode (square-within-square mark), Google Gemini (official 4-point spark vector), and Google Cloud Platform (official 4-color cloud vector); routed gTTS and Google Cloud TTS to GCP mark; matched RealtimeConfigDesk provider cards to LlmConfigDesk 46px horizontal layout with in-situ key editing; fixed compact catalog scroll lock by decoupling inner content grid and triggering Lenis resize; passed dual-target gate and 61/61 vitest suite.
- **Single-Selection Invariant, Custom Server Modal Flow & Realtime Modal Card Matching:** Enforced mutually exclusive single provider selection across embedded, server, and cloud tiers; refactored CloudProviderCard key input to clean border-b underline with transparent action icons; implemented in-modal custom server creation card with 2-second dialect probe, auto-name resolution, and save-or-discard error flow without closing modal; rendered saved custom server cards with delete action; upgraded RealtimeConfigDesk provider cards to 142px modal-style vertical cards with 36px icons, multiline descriptions, and in-situ key management; passed dual-target gate and 61/61 vitest suite.
- **Vertically Stacked Provider Cards & Minimal Custom Server Form:** Redesigned modal provider cards (`CloudProviderCard`, `LocalProviderCard`, `CustomProviderCreationCard`, `SavedCustomServerCard`, `TtsModalCard`) and `RealtimeConfigDesk` to vertically stacked, center-aligned 188px layout with 48px icon containers, 24px logos, top-right checkmarks, and centered copy; replaced `CustomServerFormCard` pill inputs and solid fill button with minimal `border-b` underline inputs and transparent action buttons; passed dual-target gate and 61/61 vitest suite.
- **Available Filter Invariant & Realtime Desk Scroll Clearance:** Enforced Available ("avl") filter contract to always show on-device card, custom creation skeleton card, saved user custom servers, connected cloud providers, and free providers (Edge TTS); decoupled custom server rendering from local tier selection; added scroll container with custom scrollbar, overscroll containment, and bottom padding clearance to RealtimeConfigDesk to prevent clipping under fixed card boundaries; passed 61/61 vitest tests and 0-warning dual-target gate.
- **Custom Server Health Probe & Warning Edit Action:** Added serde default to model in LlmRuntimeTarget and prioritized native dialect discovery (Ollama /api/version, vLLM, llama.cpp) over OpenAI-strict /models in health check; redesigned CustomServerFormCard warning UI to eliminate box container and borders; added '← Edit' action returning to input fields with preserved text without discarding entered credentials; passed 61/61 vitest tests and 0-warning dual-target gate.
- **Provider Flow Trace Logging B0 & Build Repair:** Added entry/outcome logs on `update_setting` rejections, `check_provider_health`/`list_llm_models`/`probe_model_capabilities` (kind, elapsed, no secrets), `to_provider_config` server-id resolution, and `list_remote_models` URL/transport/count; added `[Providers]` console tags to remote-model fetch and custom-server save/delete; fixed 5 pre-existing `tsc` errors blocking `pnpm build` (nullable `activeCloudProviderId`, dead `tagline||desc` fallback, unused modal props); desktop clippy gate, 61/61 vitest, and `pnpm build` green.
- **Provider Identity & Contract Rebuild (ZBC):** Backend now validates `llm`/`tts` server lists (URL normalize, duplicate id/URL reject), pruned reload-policy table to keys `mutation.rs` accepts, rejections name valid keys, empty-model DB upserts skipped, health verdict follows the real `/v1/models` check, `favorite_models` recreation dropped; frontend purged all phantom writes (`llm.server/cloud/embedded`, `stt.embedded`, `tts.speed`, `chatterbox_remote`, realtime subkeys) to `active`+`last_selected`+`servers`/`cloud_keys`/`api_keys` vehicles with id-only selection, per-model params via `model_parameters` with debounced hook, non-destructive partial-failure commit, typed `DraftKeyMap`, shared UI moved to `settings/providers/`; dual-target gate, 217/220 nextest (3 pre-existing session-lifecycle failures, confirmed on pristine tree), 63/63 vitest, `pnpm build` green.
- **Session Reactor Panic, Inventory Persistence & Banner Unification:** Made `resolve_llm_execution_params` async (removed `block_in_place`+`Handle::block_on` bridge that panicked the router thread) fixing the 3 `session_lifecycle_test` failures at the production root (220/220 nextest); added scoped `commitChanges({domain,key})` so server add/delete/edit persist immediately without sweeping unrelated drafts, with active-server repoint on delete; fixed voice compact/modal divergence (`stageVoice` now writes `tts.active_voice_id`, modal fallback removed, badge keys de-duplicated); unified all failure/transient banners on one wrapping `SettingsFooterBanner` (card, inline, modal) and fixed footer flex overflow; hover-reveal pencil+trash on cards/pills (always visible on touch); dashed Custom button grammar; redesigned titlebar model-updates popover (header+version, scrollable list, primary action, touch toggle); 64/64 vitest, `pnpm build` green.
- **Provider UX Round 2 (feedback):** Tick hides on hover revealing pencil+trash (touch keeps all visible); health errors moved from inline desk to card footer via `deskFooter` slot with clear-on-switch; save/select decoupled (creating no longer auto-selects); Custom button active state uses dashed accent instead of filled fill; provider selection re-asserts ModelsCard tab sync (bi-directional kept); `ipcErrorMessage` helper ends `[object Object]` banners; catalog reload narrowed to catalog-feeding keys (flicker); creation form uses light dashed grammar; committed-active server pins first post-save; 64/64 vitest, `pnpm build` green.
- **Custom In-Place Flow, Design Spec Popover & Error Token Normalization:** Positioned custom creation card at end of grid with in-place creation and editing to stop UI flicker and layout shuffle; removed redundant checkmarks across all provider cards relying on border highlight; tied connection failed banner to provider lifecycle with modal unmount cleanup; replaced mustard warnings with semantic `--error` red token; wrapped edit and trash buttons in Tooltip components with error color on delete; persisted inline cloud keys immediately via scoped `commitChanges`; guarded remote model and health probes behind configured API keys; aligned titlebar popovers to `design-spec.md` with subtle elevation, `rounded-xl`, and `rounded-md` badges; verified Deepgram brand color (`#13EF93`); 64/64 vitest tests and dual-target verify-targets compile gate passed with 0 warnings/errors.


---

## 6. Backlog

(none — cleared via §5 entry "Light/Dark Semantic Color Audit".)