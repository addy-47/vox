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

- **TTS Stack, Dynamic Paste Loader & Audio Profiling (2026-09-28):** Model-agnostic TTS trait contract, bare-slug voice scoping, onset boundary clipping fixes via pre-roll + warmup silence, dynamic dlopen AT-SPI loader, and ZipVoice audition sweeps.
- **Tiered POSIX Storage & 3-Way JSONC Split (2026-09-29):** Implemented 6-tier POSIX layout in `paths.rs` (ZBC, no symlinks), decomposed settings into `settings.jsonc`, `providers.jsonc` (0600), and `agent.jsonc` with zero-dependency JSONC parser (`jsonc.rs`).
- **Domain Configuration & IPC Boundary Decoupling (2026-09-29):** Deleted monolithic `core/settings.rs` (~1500 lines), distributed sub-settings to subsystem owners (`services/`, `pipeline/`, `core/`), built modular `src/config/`, and turned settings IPC into lean transport adapters.
- **Settings & Storage Senior Review Remediation (2026-09-29):** Audited and resolved all 17 findings from `SETTINGS_REFACTOR_REVIEW.md`; aligned reload policies & dispatch tables, added synchronous exit flush, hardened JSONC BOM/trailing commas and 0600 file modes, aligned wiring defaults, pruned dead code, and synchronized specs.
- **Remediation Verification & Monolith Migration Regression (2026-09-29):** Confirmed 14/17 fixed, 2 partial, 1 regression. Pruning the legacy monolith branch in `VoxSettings::load()` orphaned the `paths.rs` `settings.json` migration: a monolith now parses silently as `SettingsConfigFile` (no `deny_unknown_fields`), so an upgrading user loses API keys, persona, and memory policy on first save. Requires decomposition at the migration site in `paths.rs` or full ZBC removal of the move.
- **ZBC Migration Purge, Schema Pruning & Settings Sync (2026-09-29):** Purged legacy monolith migration from `paths.rs` under ZBC, added ext4 fsync trade-off note in `persistence.rs`, swept stale config debris (`.tmp`/`.corrupt.*`), synchronized `reset_settings` backend & frontend contract with restart notification, pruned obsolete schema v1..v8 migrations in `schema.rs` to canonical v9, and replaced raw JSON blobs in `storage-spec.md` with concise domain scopes.
- **Dictation Gate 0 CPU Benchmark & Refiner Architecture (2026-09-29):** Empirically verified Gate 0 on client hardware (i5-1145G7): `lfm-230m` decodes at 10.4 tok/s (~96ms/tok, ~1.2–2.0s/sentence), proving autoregressive SLMs mathematically violate the 85ms P95 SLA on CPU. Formulated stitched non-autoregressive architecture (pruned 4-layer encoder tagger + deterministic ITN rule engine + micro-replacer) targeting ~12–18ms single-pass latency.
- **IPC Parameter Casing & LLM Provider Deserialization Fix (2026-09-29):** Fixed missing `providerId` in `get_provider_caps` and added camelCase/snake_case resilience for `probe_model_capabilities` in `settingsService.ts`; added `open_ai_compat` serde alias to `LlmProviderConfig::Server` with unit tests and widened frontend provider types; verified 0 errors across `clippy --all-targets --release` and `tsc --noEmit`.
- **`open_ai_compat` Variant Purge & Backend Style Guide Overhaul (2026-09-29):** Removed the `open_ai_compat` third provider kind from the entire stack (Rust enum, IPC serde alias, catalog CAP_KIND, probe, test fixtures, transport config); all remote OpenAI-compatible endpoints now classified as `server`. Updated `backend-style-guide.md`: purged phantom `constants.rs` reference, enforced 3-tier constant hierarchy, replaced duplicated invariants with spec-first pointers.
- **Semantic Structured Personal Memory Backend (2026-09-30):** Replaced positional Markdown memory (`document.rs`/`patch.rs`/`suggestions.rs`) with a canonical semantic JSON model (`model.rs`), ID-addressed operations (`operations.rs`), flat-grouped JSON prompts, and `personal_memory_revisions` (schema v10, ZBC drop of old table). Added `ConsolidationRequest`/`ConsolidateOutcome` with user-controlled `forced` gating, `suggestion_policy` setting with `auto_apply`, deterministic Markdown→JSON manual save, `markdown` wire field with `skip_serializing` on canonical `content`, and renamed fact→observation across Rust. Specs (`memory`/`db`/`ipc`) updated first per §4.3; frontend migration explicitly deferred.

---
