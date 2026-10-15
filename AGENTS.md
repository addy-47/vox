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

- **Dual-Target Android Gate & Physical Hardware Pipeline:** Dual-target compile gate (`./scripts/verify-targets.sh`) compiles desktop and Android (`arm64-v8a`) at 0 warnings/errors. Full local pipeline proven on physical POCO device (Oboe, Silero VAD, Nemotron STT, NVIDIA NIM cloud LLM).
- **Core Seams & Transport Decoupling (A1, A2, A4):** Headless execution boundary with `IpcEmitter`; audio capture/sink traits decoupling hardware audio streams; frontend unified `Transport` abstraction with runtime carrier swapping; 61/61 vitest passed.
- **Unified Voice & Model Catalog Architecture:** Consolidated voices in Turso SQLite DB (v13 schema) with atomic SQL favorite toggling and persistent favorites; 16-bit PCM dynamic audio playback with peak normalization.
- **Model Hub Visual Hierarchy & Continuous Contour Architecture:** Engineered `UnifiedModelHubPanel` single-surface continuous SVG contour connecting active tabs to child card with zero dividing seams; standardized `CatalogHeader` grammar (Search left, Title centered without icon, Sort right, Filter on LLM); 3-tier compact model cards; clean dual-target verify-targets gate.
- **Unified Inference Stage UX & Providers Catalog Header Architecture:** Replaced uncanny embedded|server|cloud tabs with compact 3-stage switcher (STT | LLM | TTS) fitting 340px InteractionCard with zero overflow; created dedicated `ProvidersHeader` ([breadcrumb] [icon] [title] · gap · [+ Custom] [filter] [sort] [search]) with Coming Soon on STT and Step 2 custom server on LLM/TTS; purged hardcoded model names and faux pills; integrated clean frameless SVG `ProviderGuideView`; 61/61 vitest passed; 0-warning dual-target gate.
- **Web Search Nexus 0.1.3 (implemented, publish pending):** DDG Lite-first, SearXNG + 7 gated verticals, fetch pool 6 with strict domain caps, failure taxonomy + coverage/thin gates + quorum fix in output; 48-query evals + 5 QA rounds; dual-target gate clean, crate suite 147/147. Publish needs user go-ahead.
- **Unified Config Decomposition & Model Provenance (ZBC):** Decomposed runtime settings into 3 atomic JSONC files (`settings.jsonc`, `providers.jsonc`, `agent.jsonc`) with isolated multi-server arrays and API keys; moved per-model runtime parameters to Turso SQLite (`models` table, schema v14) preserving parameters across model changes; dedicated `services/realtime/config.rs`; eliminated legacy compatibility aliases; verified dual-target gate at 0 warnings/errors.
- **Settings Provenance Backend Review (`d692e001`):** Adversarial review of the config-decomposition commit. 30 findings (R-01..R-30) in root `settings-provenance-review.md`: confirmed defects include realtime api-key namespace mismatch (`"gemini"` vs spec `gemini_live`, R-05), one bad custom-server entry silently wipes all cloud keys (R-06), `LOWER(id)` provenance matching vs case-sensitive PK → duplicate rows (R-07), stale debounced-save snapshot able to overwrite immediate `settings.save()` (R-09), bookkeeping keys (`stt.last_selected`) falling through to a full engine restart (R-10). Also found no read IPC for `model_parameters` (`get_model_parameters` unregistered), key-registry drift across 3 tables with an in-commit test edit hiding it, and 3 hard-gate violations (banned `#[allow]`, `_settings`/`_state` masking). Desktop clippy gate re-verified 0 warnings; Android target not run (unverified). No backend code changed.
- **Custom Server Durability & Provenance Hardening (TICKET-BE-PROVIDERS-01 & R-01..R-08):** Added struct-level `#[serde(default)]` to `LlmCustomServer` and `TtsCustomServer` with resilient defaulted fields; purged `.or_else(|| provs.servers.first())` silent fallbacks; removed private IP leak in TTS server defaults; enriched `ModelCatalog` with SQLite `model_parameters`; resolved context window & threads SSOT across loader, prompt budgeter, compaction, and consolidation; dual-target verification passing at 0 warnings/errors on desktop and Android.
- **Interaction Pipeline Cards & Persistent Providers Refactor:** Redesigned Level 1 stage cards ("Listening", "Reasoning", "Speaking") with title right of icon, subtext below, and clean active provider line with zero badge/pill containers; eliminated custom server header layout shift by keeping ProvidersHeader persistent with inlineCustomActions ([✓ Save] / [✕ Cancel]); integrated custom dialect health probing with non-hijacking fallback prompts; scoped TTS expanded modal to 5 catalog entries with top-ranked custom creation card; pure zero-dependency SVG vector logos for 35+ providers; 61/61 vitest tests passed; desktop clippy gate passed at 0 warnings/errors.

---

## 6. Backlog

(none — cleared via §5 entry "Light/Dark Semantic Color Audit".)