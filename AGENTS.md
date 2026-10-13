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

- **Phase 13 scope:** Android platform preparation + critical on-device features (remote control, disfluency ML, speaker lock). Feature specs in `docs/plans/wip/`; active plan in `phase13/`.
- **Android Dual-Target Gate & APK:** `verify-targets.sh` compiles desktop + `arm64-v8a` at 0 warnings/errors. Release APK signed and running on POCO redwoodin / Android 14. Style guide covers Pitfalls 1–22. Android defect fixes D1–D6 delivered (JNI init, IPC timeout, notifications, downloads, wizard UX).
- **Auto Audio Route, STT Warmup & Eager Streaming (2026-10-12):** Resolved barge-in interruption drop by adding cross-platform auto headphone detection (`detect_audio_output_mode()` in `route.rs` via CPAL device names, PipeWire/PulseAudio sink active ports, and Android `/sys/class/switch/h2w/state`), dynamically syncing route to VAD actor. Eliminated 3.7s lazy-load turn-1 delay by adding `SttCommand::Warmup` on modular session engage. Unblocked eager clause dispatch from streaming router to TTS to eliminate 6.2s buffering delay, aligning `harness-spec.md` with runtime expectations. All integration tests (`playback_interrupt_test`, `agentic_tool_runtime_test`) and `verify-targets.sh` compile and pass clean at 0 errors.
- **Dynamic Route Watcher & Model Probe Timeout (2026-10-12):** Added zero-polling background `AudioRouteWatcher` (`pactl subscribe` on Linux) to automatically detect headset connect/disconnect events at runtime and push `VadCommand::UpdateAudioMode` to `VadActor`. Bumped cold model probe timeout to 60s for large local LLMs (e.g. Gemma 4 12B). Fixed duplicate probe failure tooltip and restored "Apply & Restart" text label for non-compact desktop views. Removed obsolete persistent `output_mode` from settings. All gates pass at 0 errors.
- **Audio Domain Pruning & Wizard Simplification (2026-10-12):** Completely pruned the obsolete `audio` configuration domain from `settings.jsonc`, `storage-spec.md`, `VoxSettings`, `SettingsConfigFile`, and frontend stores. Simplified `AudioSetupStep` in the first-run wizard into a direct microphone level test using the OS default input without manual device selection. Dual-target `verify-targets.sh`, `settings_persistence_test`, `pnpm tsc`, and vitest all pass clean at 0 errors.
- **System Specifications Overhaul (2026-10-13):** Refactored all 10 core system specs (`db`, `storage`, `ownership`, `ipc`, `dictation`, `providers`, `memory`, `notifications`, `events`, `harness`) against HLD standards and `create-spec` skill. Stripped implementation code transcripts, raw grep inventories, and file trees. Restored all functional invariants, transaction/MVCC conflict retry rules, and timing contracts. Enriched with 14 Mermaid state machines, sequence diagrams, and flowcharts. Dual-target `verify-targets.sh` unaffected.
- **Settings UX & Card Polish (2026-10-13):** Redesigned TTS Voice Manager with standalone region pill filter and live search result feedback. Decoupled Server step 2 URL into IP/domain and port fields with an expandable Advanced options section, header sphere indicator, and dynamic dialect copy. Standardised persona prompt presentation between Modular and Realtime pipelines with default preview and corner pencil edit toggle. Added dark mode render diagnostics. Dual-target and vitest gates pass clean at 0 errors.
- **Config Desk, Voice Search & Personal Memory Fixes (2026-10-13):** Realigned Server Step 2 column ratio to standard 60/40 with concise copy, vertical stacking (Host, Port, Advanced), and inner scrolling. Resolved Voice Manager search overflow by scoping input `min-w-0` and slot width to 175px. Eliminated Personal Memory bottom border clipping and corner mismatch by removing hardcoded 128px workspace height, applying `overflow-hidden`, and aligning padding to match Working Memory card. Dual-target `verify-targets.sh`, `pnpm tsc`, and vitest all pass clean at 0 errors.
- **Unified Voice Catalog Backend (2026-10-13):** Replaced `tts.voice_index: i32` with canonical `tts.voice_id: String` (ZBC, manual `settings.jsonc` migration to `"bella"`). Added Turso schema v11 (`voice_favorites`, `voice_recents`, `remote_voice_cache`, voice metadata columns) and `VoiceRegistry` facade emitting `UnifiedVoiceDto` for the Select-a-Voice modal. New IPC (`list_unified_voices`, `toggle_voice_favorite`, `record_voice_used`, `set_active_voice`, `get_voice_preview_state`) with preview cache under `cache/tts/previews/`. Plan in `phase13/voice-catalog-plan.md`; specs (`db`, `storage`, `providers`, `ipc`) updated. Dual-target `verify-targets.sh` passes at 0 errors.
- **Voice Catalog Frontend (2026-10-13):** Pruned `TtsVoiceManager` voice tab (carousel, region buckets, Edge/custom branching removed; speed + compute kept). New `voices/` catalog (`VoiceCatalogView` inline + `ExpandableList` floating Maximize2 modal, `VoiceCatalogModalView` matching the Select-a-Voice design, memo `VoiceCard`) driven by `useVoiceCatalog` hook and extended `voiceService` (unified DTO, favorites, recents, preview state via asset URL). Preview play disabled when `has_preview == false` and session not idle. `settingsStore.tts.voice_id: string`. `pnpm tsc`, vitest 58/58, dual-target gate all green.
- **Unified TTS Voices Architecture & UI Redesign (2026-10-13):** Unified TTS voices in Turso DB into single `voices` table with v11 metadata schema; dropped redundant cache tables. Embedded declarative `voices_manifest.json` in `tts/` covering Kokoro, Supertonic, Zipvoice, Chatterbox. Consolidated IPC to canonical `list_voices`, background `sync_remote_voices`, and on-demand `synthesize_voice_preview` with LRU disk cache (`cache/tts/previews/`). Overhauled Select-a-Voice modal (`VoiceCatalogModalView`, `VoiceCard`, `CloneVoiceDrawer`) with Lenis smooth scroll, deduplicated facets, scrubber bar, custom prompt editor, and mic/WAV cloning. Dual-target `verify-targets.sh`, `agentic_tool_runtime_test`, `playback_interrupt_test`, `tts_transition_test`, `pnpm tsc`, and vitest 58/58 all pass at 0 errors.
- **Voice Schema Repair & Specification Normalization (2026-10-13):** Migrated user database to schema v12 by adding missing `provider_id` and metadata columns to `voices`, dropping legacy tables, and running `ensure_voice_metadata_columns` unconditionally on boot. Properly integrated voice catalog architecture across core specs (`db-spec.md` Section 5.12, `storage-spec.md` Section 5.1 RULE-STORAGE-09, `ipc-spec.md` Section 4.7, `providers-spec.md` Section 6) removing all appended end-of-file snippets. Dual-target `verify-targets.sh` passes at 0 errors.
- **Smooth Scroll Standard & LLM Catalog Lock Fix (2026-10-13):** Fixed catalog scroll lock (nested Modal body + inner scrollers fought; Lenis wrapper==content mis-measured on filter swaps; per-keystroke reset interrupted wheel-RAF; snap-* fought virtual scroll). Standardised on Lenis inertial glide via SSOT factory (`lenisScroll.ts`), hook (`useLenisScrollContainer` + ResizeObserver), and new declarative `SmoothScrollArea`; `Modal`/`Drawer` bodies yield to inner scrollers; snap banned on Lenis containers; resets only on settled filter state. Migrated all 4 ad-hoc Lenis copies; codified the standard in `design-spec.md`. `pnpm tsc` clean at 0 errors.
- **Memory Timeline UX Polish & Catalog Lenis Container Refactor (2026-10-13):** Removed redundant tooltip from session accordions in memory timeline. Allowed session titles to wrap when opened without truncation, and anchored session icon to top (`items-start`, `mt-0.5`) to eliminate layout jump. Decoupled scroll container from inner content grid in `RemoteCatalogSection` and `LocalCatalogSection` for Lenis height tracking, preventing scroll lock on "Show more". Configured `.custom-scrollbar` with visible `scrollbar-width` and accent thumb styling. `pnpm tsc` and vitest 58/58 pass clean at 0 errors.
- **Voice Catalog Rebuild & Invariant 6 Compliance (2026-10-13):** Rebuilt Select-a-Voice modal to the LLM catalog standard (search + count readout, mono filter chips, facet sidebar, compact two-row player strip with inline prompt editing, no faux-pill copy). Gated all clone entry points on backend `ProviderCaps.clone` (Chatterbox family only); fixed empty catalog on `chatterbox_remote` via backend scope mapping. Moved engine-to-catalog mapping into Rust (`resolve_voice_scope` + `get_voice_scope` IPC) after vitest Invariant 6 flagged frontend provider-id literals. Converted `CloneVoiceDrawer` to the shared `Modal` primitive; all voice scrollers on the canonical Lenis hook. Contract in `providers-spec.md` §6.5. `pnpm tsc`, vitest 58/58, dual-target `verify-targets.sh` all green at 0 errors.
- **Frontend UI Optimization Plan & Checklist (2026-10-14):** Formalised the approved implementation plan (`docs/plans/phase13/frontend-ui-approved-plan.md`) and pure checklist (`docs/plans/phase13/frontend-ui-checklist.md`) across 6 atomic batches from the read-only audit. Scope retains film/ambient grain and uncapped graph invariant while sequencing dead copy pruning (231 keys, 17.7%), overlay transitions (120ms coarse curve, tap tooltips), CSS motion migrations, Three.js chunk deferral, and panel decomposition.

- **Voice Catalog Favorite Toggle & Asset Protocol Scope (2026-10-14):** Fixed unliking failure caused by `initialFavoritesRef` snapshot desync in popular sorting by updating the snapshot set in `handleCardFavorite`. Resolved preview playback block (`asset protocol not configured to allow the path`) by configuring `app.asset_protocol_scope().allow_directory()` for both cache and root directories during startup. Both desktop and Android `verify-targets.sh`, `pnpm tsc`, and vitest 58/58 pass at 0 errors.