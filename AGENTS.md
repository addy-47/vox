# AGENTS.md — Vox Workspace Rules

---

## 0. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target.
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **165 lines** (the warning threshold before the 175-line ceiling):
> 1. Write the **full, uncompacted** current Section 5 content to `docs/plans/<current_phase>/recent_work.md`.
> 2. Replace Section 5 in `AGENTS.md` with a compact 3–5 bullet summary of only the highest-level milestones.
> 3. Add a deep link at the top of Section 5: `📖 Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/<current_phase>/recent_work.md)`.
>
> **This is the complete hook. Nothing else is mandatory on every task.**

---

## 1. Project Context

Vox is a **realtime voice AI desktop app** (Tauri v2 / Rust / TypeScript). Constraint: 8GB RAM, CPU-first inference, sub-200ms perceived pipeline latency.

**Crate structure:** Single Rust library crate `vox_lib` at `app/src-tauri/`. `main.rs` is 1 line. `lib.rs` is module declarations + Tauri assembly only. All logic lives in modules.

---

## 2. Workspace Directory Map

| Path                      | Purpose                                             | Rules                                                                                                 |
| ------------------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `app/src-tauri/src/`      | Purpose Rust source                                 | No test logic. No benchmarks.                                                                         |
| `app/src-tauri/tests/`    | Integration tests (`cargo test --tests`)            | Named `<feature>_test.rs`. Tests public API only.                                                     |
| `app/src-tauri/benches/`  | Performance benchmarks (`cargo test --benches`)     | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`.                                   |
| `app/src-tauri/examples/` | Utility CLI tools (`cargo run --example <name>`)    | Standalone tools. No `#[test]`. No assertions.                                                        |
| `.agents/rules/`          | Role-specific agent instruction files               | Read relevant file before acting in that role.                                                        |
| `docs/plans/`             | Architecture specs and phase plans                  | Source of truth for specs. Do not contradict.                                                         |
| `docs/features/`          | Implemented feature ledgers                         | Update after completing features.                                                                     |
| `sandbox/`                | Scratch space for experiments, evaluations, scripts | Non-production code. Results in `sandbox/results/`. Datasets in `sandbox/datasets/`.                  |
| `temp/`                   | Ephemeral runtime files: logs, raw LLM outputs      | `temp/.env` (API keys). `temp/server.txt` (remote GPU server creds). Not versioned.                   |
| `submodules/`             | Git submodules                                      | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`. Do not edit directly. |
| `~/.vox/models/`          | Local model weights                                 | Canonical manifest: `~/.vox/models/models_manifest.json`.                                             |

**Remote GPU server:** `root@[IP_ADDRESS]` (creds in `temp/server.txt`). Ollama . **Never kill running server processes.**

---

## 3 Execution & Testing Invariants (All Agents)

1. **Sequential Execution:** Run performance-sensitive tasks (benchmarks, evals, test suites) strictly one at a time to prevent CPU, memory, and I/O contention.
2. **Release / Optimized Mode:** Always run performance measurements and benchmarks under release mode (`--release`). Debug builds produce invalid metrics.
3. **Isolated Test Runner (`cargo-nextest`):** Always use `cargo-nextest run` with explicit thread pool allocation and single-thread isolation:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1
   ```
   _Single test:_ `cargo nextest run --test <test_file> --release --nocapture --test-threads=1`  
4. **External API Keys (`#[ignore]`):** Cloud provider tests (Nvidia, Gemini Live, Deepgram, OpenAI, ElevenLabs) must be marked `#[ignore]` and run manually only with explicit user approval: `cargo nextest -- --ignored`.

---

## 4. Invariants and Workflow Gates

### 4.1 Critical Architectural & Logical Invariants (Non-Negotiable Concepts)

0. **Zero Backward Compatibility (ZBC):** Backward compatibility is not a requirement unless explicitly stated. Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers, legacy paths, or transitional abstractions proactively..
1. **Single Source of Truth for State:** `InteractionState` (`Idle, Ready, Listening, Thinking, Speaking, Paused, Error`) and `DictationState` (`Idle, Recording, Transcribing, Error`) are the sole sources of truth. Synthetic lifecycle booleans (`is_engaged`, `is_recording`, `is_connected`, `is_speaking`, `is_sleeping` are strictly banned across Rust and TypeScript.
2. **Registry-Owned Event Contracts:** `core/events.rs` is the SSOT for all cross-boundary events. Internal pipeline events belong to `VoxEvent`; IPC events belong to `IpcEvent` with strongly typed payloads. Raw string event literals are forbidden at emit and listen sites; frontend mirrors the registry via `IpcEventMap`.
3. **Sacred Audio Hot Path:** Zero dynamic memory allocations, zero lock acquisitions (`Mutex`/`RwLock`), and zero blocking I/O on the CPAL audio thread and VAD inference loop. Ring buffers must be lock-free and pre-allocated.
4. **Actor-Engine Separation & Thread Isolation:** CPU/GPU-heavy model inference (Whisper STT, ONNX VAD, Llama LLM, Chatterbox TTS) runs strictly on dedicated background OS threads (`std::thread`). Tokio runtime is reserved strictly for async I/O, IPC routing, and network WebSockets.
5. **Strict Frontend Service Boundary:** React components and hooks must never directly call `@tauri-apps/api/core` (`invoke`) or `@tauri-apps/api/event` (`listen`). All backend interactions must route through strongly-typed singleton service modules in `src/services/`.
6. **React 19 Context Memoization & Selector Discipline:** Provider values must be wrapped in `useMemo`. Zustand store state must be queried via fine-grained atomic selectors (`(s) => s.field`) rather than consuming entire store snapshots, preventing cascading render loops.
7. **Centralized Monotonic Turn Lifecycle:** Monotonic turn IDs must be generated exclusively at turn boundaries via `PipelineAtomics::next_turn()`, which atomically advances the turn ID, renews the `CancellationToken`, and returns `(turn_id, token)` as a bundle. Turn IDs must never be reset to 0, fragmented across parallel actors, or fabricated with dummy values. Subsystems receive `(turn_id, token)` at the turn boundary — they never own or directly advance the underlying `AtomicU32`.
8. **Single-Consumer Audio Stream Invariant:** Audio ring buffers and input channels must have exactly one consumer (`VadActor`). Never attach secondary or ad-hoc readers to production audio streams.

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
>
> - **WRITE TASK (Backend Rust):** You MUST read `.agents/rules/backend-style-guide.md` AND `.agents/rules/backend-engineer.md` BEFORE modifying Rust backend code.
> - **WRITE TASK (Frontend React/TS):** You MUST read `.agents/rules/frontend-style-guide.md` AND `.agents/rules/frontend-engineer.md` BEFORE modifying frontend code.
> - **WRITE TASK (Tests/Benches/Evals):** You MUST read `.agents/rules/testing-style-guide.md` AND `.agents/rules/test-engineer.md` BEFORE authoring tests or benchmarks.
> - **READ-ONLY TASK (Auditing, answering questions, running tests/benchmarks, searching code):** DO NOT read code style files. Save context tokens.

---

### 4.3 Agent Roles

| Role                 | Rule File                               | Scope                                                            |
| -------------------- | --------------------------------------- | ---------------------------------------------------------------- |
| System Architect     | `.agents/rules/system-architect.md`     | Strategy, gates, plan approval                                   |
| Backend Engineer     | `.agents/rules/backend-engineer.md`     | `app/src-tauri/src/` implementation                              |
| Frontend Engineer    | `.agents/rules/frontend-engineer.md`    | `app/src/` implementation                                        |
| QA Engineer          | `.agents/rules/qa-engineer.md`          | Test audit, benchmark validation                                 |
| ML Research Engineer | `.agents/rules/ml-research-engineer.md` | ML model research, evaluation, and fine-tuning dataset curation  |
| Test Engineer        | `.agents/rules/test-engineer.md`        | Test case design, benchmark validation, and performance analysis |

---

## 5. Phase 11 Test Suite & Verification Ledger

> 📖 **Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)** | Phase 10 Archive: [phase10/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase10/recent_work.md)

- **Phase 10 Concluded:** 4-domain pipeline refactor, realtime WebSocket driver decomposition, frontend Home controls alignment, hot-path zero-allocation buffer pools, and `services/audio/` architecture separation — all with zero clippy warnings.
- **Phase 11 Initialized (2026-09-02):** Established `docs/plans/phase11/` focused on full test suite engineering: Critical Isolated Unit Tests (UT), Integration Test Seams (IT via `integration_test_spec.md`), performance benchmarks, and pipeline evaluations.
- **2026-09-03 Boot Smoothness (3 rounds):** Fixed first-launch jitter, white pre-paint flash, accent color flash, and perceived boot latency via inline CSS loader, synchronous Rust window reveal, cached theme in localStorage, and deferred overlay initialization.
- **2026-09-03 Kokoro TTS UI wiring:** Added missing Kokoro select/verify branches (`TtsModelWorkspace`, `ModelsCard.isTtsVerified`), fixed multi-file download progress stuck at 1% via file→group weighted aggregation, replaced Get text button with Download icon (`SubModelCard`); verified voices (`voice_index` sid 0–9 shared list) and speed knob already map correctly, `quality_steps` correctly has no Kokoro UI (backend no-op).
- **2026-09-03 Test Clip Seam Refactor:** Refactored developer test clip feature into thin IPC (`ipc/pipeline/test.rs`) and modular execution (`pipeline/test.rs`); supports all model combinations (Modular local/cloud STT/LLM/TTS and Realtime S2S WebSocket), prevents DB writes by setting `is_private_mode`, and closes popover drawer immediately on clip selection in UI with zero clippy/test errors.
- **2026-09-03 Dynamic Model Hub plan:** Wrote reviewer-grade `DYNAMIC_MODEL_HUB_REFACTOR_PLAN.md` (root) — 22-file id-driven refactor (manifest `provider` SSOT, preview-selection settings, full-body Model|Settings toggle, caps-driven voice/speed); no code changed.
- **2026-09-03 Dynamic Model Hub plan v2 (id-driven, all categories):** Full-tree grep (16 group ids × 7 categories; 9 FE files + backend hits enumerated) proved `provider` key redundant — killed it. Plan rewritten: stable short ids (`kokoro_multi_lang_v1_1`→`kokoro` etc., zero settings migration), short `name` slugs with id/name boundary rule, 3-place manifest sync order (vox-models → ~/.vox → root mirror); no code changed.
- **2026-09-03 Dynamic Model Hub implemented:** Manifest v1.6.0 in all 3 places (stable ids, short names, `provider` keys deleted, `required` on `ten_vad`); backend `ProviderCaps` + `get_provider_caps` + UT (52/52 lib green, clippy clean); frontend id-direct selection, preview-driven settings, full-body Model|Settings toggle with `ProviderHeader`, caps-driven `TtsVoiceManager`/`LlmSettingsView`; `tsc` + `pnpm build` green.
- **2026-09-03 Settings visual harmony pass:** Merged provider context into the single card header row (deleted appended `ProviderHeader` box), unified all pane sub-tabs on the pill `SegmentedControl` language (`TtsVoiceManager`, `LlmSettingsView` + `LLM_SETTINGS_COPY`), stripped nested container boxes inside settings panes; `tsc` + `pnpm build` green, impeccable detect scan structurally clean.
- **2026-09-03 Crash-Safe Memory Compaction & Notification System:** Implemented DB Schema v2 with `session_compactions` ledger & `notifications` table; reorganized persistence into domain modules (`compactions.rs`, `sessions.rs`, `memory_*.rs`); built `CompactionCoordinator` with strict `Idle`/`Paused` state guard; aligned facade error routing (`CriticalCompaction` pipeline error) & soft compaction fact preservation; implemented boot reconciliation for uncompacted crash recovery; wired frontend stroke `Bell` with primary accent counter badge, interactive action cards (`[Compact Now]`, `[Dismiss]`, deep-linked session drawer), and `auto_compaction` setting toggle in HistoryCard; 54/54 tests green, clippy & `pnpm build` clean.
- **2026-09-03 IPC Consolidation, Persistence Domain Separation & UI Redesign:** Consolidated `ipc/memory/` into single `ipc/memory.rs` with zero raw SQL; created `persistence/graph.rs` and moved all query/mutation logic into domain modules; resolved GTK toast focus-stealing bug (`accept_focus(false)`); optimized bootup with dynamic light-mode CSS variables & cached `vox_setup_completed`; restored strict pinch-to-zoom prevention; unified Memory page top action bar & minimal stroke accent zoom stack; placed HistoryCard toggles side-by-side; redesigned Model Hub to use card gear buttons with in-body breadcrumbs while keeping card title untouched; `cargo clippy` and `pnpm build` clean.
- **2026-09-03 SubModelCard Dynamic Height & UI Harmonization:** Replaced fixed `SubModelCard` height with dynamic full-available-height grid layout (`h-full` / `grid-rows-1` / `auto-rows-full snap-y`), eliminating dead bottom gaps and peeking cards across VAD, ASR, TTS, and LLM workspaces; allowed descriptions to flex and use all available space cleanly (`flex-1 min-h-0 line-clamp-3`); trimmed overflowing HistoryCard toggle titles and sublabels; restored native WebKitGTK `destroy_zoom_gesture` pinch zoom elimination; `tsc` and `pnpm build` clean.