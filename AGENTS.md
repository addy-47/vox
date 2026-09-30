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

- **TTS Stack & Realtime Streaming:** Trait-level TTS model contracts, bare-slug voice scoping, onset boundary clipping fixes via pre-roll + warmup silence, dynamic dlopen AT-SPI loader, and unified realtime PCM streaming across Kokoro, ZipVoice, Chatterbox, Supertonic, and EdgeTTS.
- **POSIX Tiered Storage & 3-Way JSONC Split:** 6-tier POSIX layout in `paths.rs` (ZBC), settings decomposed into `settings.jsonc`, `providers.jsonc` (0600), and `agent.jsonc` with zero-dependency JSONC parser, synchronous exit flush, and domain IPC decoupling.
- **Semantic Structured Personal Memory & Zero-Pollution Evals:** Canonical semantic JSON model, ID-addressed operations, schema v10 migrations, whole-memory synthesis prompts, zero-mock SQLite integration tests, and NVIDIA NIM 120B eval suite with isolated test DBs.
- **Personal Memory UI & Google Docs Diff Staging:** Real-time Google Docs inline diff review (word-level LCS diffs, inline strikethrough/insertions, per-suggestion decisions), unified paginated observations list with windowed infinite scroll, modular staging views, and high-contrast pending integration confirmation banners.
- **ZipVoice Guidance Scale & Text Normalization Remediation (2026-09-30):** Enforced internal canonical `ZIPVOICE_GUIDANCE_SCALE = 1.0` (single-pass flow-distilled synthesis without CFG 2x latency overhead); removed dead `MIN/MAX_ZIPVOICE_GUIDANCE_SCALE`; synchronized `ipc-spec.md` (§2.7) declaring guidance scale an internal constant; added unicode typographic text normalization (`normalize_zipvoice_text`) in `zipvoice.rs` preventing OOV tokenizer drops on quotes/dashes; aligned `providers.jsonc` and switched active TTS to `zipvoice` for testing.
- **Backend-Owned Reload Policy & Crash Diagnostics (2026-09-30):** `get_setting_reload_policy` is now the complete SSOT (73 accepted keys, all explicitly classified; `is_explicitly_classified` + `settings_persistence_test.rs` lock it to `apply_setting_mutation`); `Restart` is **executed** by `ipc/settings.rs` via a shared `restart_engine_inner`, coalesced by a `request_engine_restart` single-runner gate so a multi-key commit costs one rebuild; deleted both frontend hardcoded 7-key restart lists and the unreachable "Apply & Reload" footer; added `utils/crash.rs` SIGABRT/SEGV/BUS/ILL/FPE native backtrace capture (the `malloc(): invalid size` abort bypassed the panic hook entirely) plus `MALLOC_CHECK_=3` in debug builds; fixed the `ensure_modular_workers` double-`OfflineTts` TOCTOU with a RAII warm-up claim; implemented the previously silent `ZipvoiceEngine::set_voice`.
- **ZipVoice Streaming, Settings Reload Policy UI & Accent Token Remediation (2026-09-30):** Integrated incremental audio chunk streaming in `ZipVoice` via `playback_cb.ingest_chunk_with_intent`; patched interaction mode enum deserialization and case normalization across backend and frontend; purged amber/yellow colors and white-on-light text bugs across `SettingsCardWrapper` and `Settings.tsx` in favor of theme accent tokens (`var(--accent)`); eliminated manual save/discard footers on hot-synced changes (hot settings auto-save silently without footers); updated `ModelStatusOverlay` to resolve canonical saved settings (`settings ?? draftSettings`) displaying active provider model names on top with provider/parameters below.
- **Observations Scoping, Graph Fact Restoration & Token Normalization (2026-09-30):** Updated `fetch_all_observations` with optional `observation_type` filtering: memory graph queries all active session facts (`getActiveObservations`) so nodes render across all categories, while Personal Memory staging passes `"personal"` via `useObservationsList`; purged hardcoded green/emerald/rose badge colors in `LearnedFactsList` and `StagingHeader` in favor of theme tokens (`var(--accent)`, `var(--foreground-muted)`).
- **Observations Card Styling & Comment Box Antigravity Redesign (2026-09-30):** Removed card divider line; positioned observation date in accent color at bottom-right corner; aligned status text to bottom-left (only when 'All' filter is selected, plain borderless text); made card text container scrollable (`min-h-0 overflow-y-auto`) to isolate date from text overflows; stripped borders from margin and floating comment trigger icons; redesigned comment popover to clean Antigravity aesthetic with subtle radii and responsive buttons.
- **Suggestions Review Google Docs Redesign & Auto-Finalizing Integration (2026-09-30):** Seeded 5 quirky active personal facts in `vox.db`; redesigned `SuggestionsReviewView` to Google Docs aesthetic (clean inline strikethrough for removals, subtle green insertions, unbordered change labels `[Replace]`/`[Delete]`/`[New fact]`, floating tick & cross); added instant execution for Accept/Reject All and auto-finalization upon deciding the last suggestion; routed `isConsolidating` animation to `PixelSynthesisCanvas` on the right card while suggestion application animates the left card.
- **Staged/Pending Filter, LLM Hallucination Fix & Banner Remediation (2026-09-30):** Renamed `"active"` → `"staged"` throughout filter UI (`ObservationFilter` type, `useObservationsList`, `StagingHeader`, `LearnedFactsList`, `PersonalMemoryStagingCard`); added true `"pending"` filter backed by `fetch_pending_queue_observations` querying `memory_ingestion_queue WHERE status='pending'`; mapped `"staged"` → `"active"` and short-circuited `"pending"` in `ipc/memory.rs::get_observations`; added `HandleMap::section_handles()` method, injected `<valid_section_handles_for_creates>` into incremental integration user prompt, strengthened Rule 3 in `PERSONAL_INCREMENTAL_INTEGRATION_SYSTEM_PROMPT` to forbid inventing section handles; added fallback recovery in `resolve_operations` for unknown section handles (recovers as `CreateSection` instead of silently dropping); added `log::debug!` raw LLM response logging in `generation.rs`; fixed banner button WCAG contrast (white on lavender → `text-black`); updated copy strings (`"Integrate Staged Now"`, `"Ingestion Queue Not Empty"`, `"Wait for Extraction"`).

---


