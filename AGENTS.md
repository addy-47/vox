# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target.
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **125 lines** (the warning threshold before the 175-line ceiling):
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
| `submodules/`             | Git submodules                                      | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`. Do not edit directly. |
| `~/.vox/models/`          | Local model weights                                 | Canonical manifest: `~/.vox/models/models_manifest.json`.                                             |

**Remote GPU server:** `root@[IP_ADDRESS]` (creds in `temp/server.txt`). Ollama . **Never kill running server processes.**

---

## 3 Execution & Testing Invariants (All Agents)

1. **Sequential Execution:** Run performance-sensitive tasks (benchmarks, evals, test suites) strictly one at a time to prevent CPU, memory, and I/O contention.
2. **Release / Optimized Mode:** Always run performance measurements and benchmarks under release mode (`--release`). Debug builds produce invalid metrics.
3. **Isolated Test Runner (`cargo-nextest`):** Always use `cargo-nextest run` with explicit thread pool allocation and single-thread isolation. Nextest defaults to fail-fast (cancels the suite on first failure). Use `--no-fail-fast` to execute the full suite without aborting on failure:
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
   _Isolate single test:_ `cargo nextest run -E 'test(<test_fn_name>)' --release --nocapture --test-threads=1`
   _Isolate test file:_ `cargo nextest run --test <test_file> --release --nocapture --test-threads=1`
   > ⏱️ **Full Suite Baseline Run Time:** ~45.5s test execution (~45s wall-clock with compilation cache hit; ~2m30s cold compile + run) across all 105 tests in 21 binaries (across Seams 1–20).
4. **External API Keys (`#[ignore]`):** Cloud provider tests (Nvidia, Gemini Live, Deepgram, OpenAI, ElevenLabs) must be marked `#[ignore]` and run manually only with explicit user approval: `cargo nextest -- --ignored`.
5. **Local Turso Database CLI (`tursodb`):** For inspecting or querying the local Turso SQLite database (`~/.vox/vox.db`), use the native `tursodb` CLI tool:
   ```bash
   tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
   ```

---

## 4. Invariants ,Rules and Specs [MUST FOLLOW]

### 4.1 Critical Architectural & Logical Invariants (Non-Negotiable Concepts)

0. **Zero Backward Compatibility (ZBC):** Backward compatibility is not a requirement unless explicitly stated. Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers, legacy paths, or transitional abstractions proactively..
1. **Registry-Owned Event Contracts:** `core/events.rs` is the SSOT for all cross-boundary events. Internal pipeline events belong to `VoxEvent`; IPC events belong to `IpcEvent` with strongly typed payloads. Raw string event literals are forbidden at emit and listen sites; frontend mirrors the registry via `IpcEventMap`.
2. **Sacred Audio Hot Path:** Zero allocations, zero locks (`Mutex`/`RwLock`), zero blocking I/O on the CPAL audio thread and VAD inference loop. Ring buffers are lock-free, pre-allocated, and single-consumer (`VadActor` only — never attach secondary readers).
3. **Strict Frontend Service Boundary:** React components and hooks must never directly call `invoke`/`listen`. All backend interactions route through strongly-typed singleton service modules in `src/services/`.
4. **Centralized Monotonic Turn Lifecycle:** Turn IDs come exclusively from `PipelineAtomics::next_turn()` at turn boundaries as a `(turn_id, token)` bundle. Never reset to 0, fragment across actors, or fabricate dummies; subsystems receive the bundle, never advance the counter.
5. **Covered in style guides (not repeated here):** thread placement (inference on OS threads, Tokio for I/O), React 19 memoization + atomic selectors, strict import hoisting (`backend-style-guide.md §2.1`).

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY CONTEXT GATE:**
>
> - **WRITE TASK (Backend Rust):** You MUST read `.agents/rules/backend-style-guide.md` AND `.agents/rules/backend-engineer.md` BEFORE modifying Rust backend code.
> - **WRITE TASK (Frontend React/TS):** You MUST read `.agents/rules/frontend-style-guide.md` AND `.agents/rules/frontend-engineer.md` BEFORE modifying frontend code.
> - **WRITE TASK (Tests/Benches/Evals):** You MUST read `.agents/rules/testing-style-guide.md` AND `.agents/rules/test-engineer.md` BEFORE authoring tests or benchmarks.
> - **READ-ONLY TASK (Auditing, answering questions, running tests/benchmarks, searching code):** DO NOT read code style files. Save context tokens.


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
5. **[LLM Agent Harness & Plugin Runtime Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** — *Status: Approved Target Spec*. Plugin chassis, 1:1 session lifecycle (`start_session(Option<sessionId>)`), decoupled LLM actor with duplex dialogue pipe, `InteractionState::Working` with audio intent gating, and clean-slate deletion of legacy harness code.
6. **[Notification Center Behavioral & Interface Specification (v2)](file:///home/addy/projects/apps/vox/docs/specs/notifications-spec.md)** — *Status: Proposed Target Spec*. Append storage with correlation key, task idempotency, and frontend stream rollup.

---

## 5. Phase 11 Test Suite & Verification Ledger

> 📖 **Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)** | Phase 10 Archive: [phase10/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase10/recent_work.md)

- **Phase 11 Seam Sprints Green (Seams 1–20):** All 20 integration test seams verified under isolated release mode with 100% mutation kills, eliminating mock orchestration and establishing production-faithful test contracts across audio, STT, LLM duplex dialogue, TTS pre-roll cushion, playback barge-in, Turso DB MVCC, and notifications.
- **Universal Compaction Contract & Cognitive Ladder:** Shipped strict 6-bucket JsonSchema SSOT with reasoning disabled across Ollama and remote transports; validated Rungs 1 and 2 (ingestion deduplication PASS); 2-stage dedup (Jaccard + MiniLM ONNX cosine) and transactional staging verified.
- **Harness Runtime Architecture & TTS Decoupling:** Refactored LLM Agent Harness to modular stage taxonomy under `services/harness/stages/` (`history`, `prompt`, `budget`, `compaction`, and `streaming/`). Decoupled NLP text chunking (`ClauseChunker`) from TTS actor into streaming stage; extracted TTS provider factory; pruned `services/tts/actor.rs` to synthesis-only with saturating decrement. Unified turn execution into `Harness::execute_turn` with direct token routing, 0-retry compaction fallback, and in-place prompt synchronization.
- **Core Pipeline Service Architecture Standardization (TTS, STT, VAD, LLM):** Standardized architecture across all 4 core pipeline services into cohesive pillars (declarative `mod.rs` with subsystem constants, dedicated OS thread `actor.rs`, settings-to-backend `factory.rs`, and provider modules). Relocated STT (`nemotron`, `qwen`, `embedded`) and VAD (`silero_onnx`, `ten_onnx`, `earshot_vad`) engines to `providers/`; extracted `provider.rs` and `factory.rs` for LLM; moved `VadCommand` to `vad/actor.rs`; consolidated `telemetry.rs` and `utils.rs` into `services/vad/segmenter.rs` to slim `vad/actor.rs` from 742 to 563 LOC; decoupled 120 lines of ad-hoc path/fallback logic from `core/engine.rs` to domain factories; verified 0 errors and 0 warnings on `cargo clippy --all-targets`.
- **Harness Production Hardening & Review Fixes:** Addressed 10-point system architect review: stream disconnect/error handling in `router.rs` triggers user turn rollback; deleted deferred tag demuxer and routed text straight to chunker/TTS; aborted background cancellation bridge tasks on all turn exit paths; offloaded sync Turso `db.connect()` AND full compaction inference via `tokio::task::spawn_blocking` preventing async reactor stall; guarded empty compaction results with FIFO shift fallback; enforced `saturating_add(1)` compaction window progression and non-empty context guard; sealed harness encapsulation by deleting `TurnPreparation`, `prepare_turn`, `clone_stream_stage`, `history()`, and `history_mut()` in compliance with §9.2.6 & ZBC; implemented `Drop for Harness` (`abort_watcher` + `session_cancel`); migrated `TurnExecutionRequest` to owned String and updated `llm_to_tts_test.rs` to unified `execute_turn`; preserved TTS actor telemetry logs; codified Compaction Parameter Isolation Invariant in specs; verified 0 dead code warnings via `audit_dead_code_v2.py` and 0 warnings/errors on `cargo clippy --all-targets`.
- **Dead Code Audit & Prune:** Ran `audit_dead_code_v2.py`; confirmed 0 Clippy `dead_code` warnings (compiler ground truth). Pruned 8 genuinely unreachable non-test functions: `with_timestamp` (harness/mod.rs), `rollback_user_turn` + `apply_compaction_summary` (orchestrator.rs), `set_reserved_generation_tokens` (budget.rs), `raw_database` (persistence/db.rs), `update_notification_status` + `notification_exists` + `find_active_notification_by_session` (persistence/notifications.rs). Restored `fetch_notification_by_id` (heuristic false-positive; has 3 real callers). Verified 0 errors, 0 warnings on `cargo clippy --all-targets`.
- **Text-as-Input & Temporary Chat:** Decommissioned `test_clip`/`test_clip_cancel`/`TestClipsPopover`/`FlaskConical` portal across backend (`pipeline/test.rs` deleted, `ipc/pipeline.rs` pruned) and frontend (`Home.tsx`, `useHomePage.ts`, `VoiceSessionContext.tsx`, `sessionStore.ts`). Shipped `VoxEvent::TextInput`, `set_playback_muted`, `set_mic_muted`, `set_session_private_mode` IPC commands with CPAL-layer zero-fill muting. Added `TextInputBar` component (underline input, mic/speaker mute toggles, send/discard). In `Home.tsx`: `Keyboard` icon toggle opens `TextInputBar` in engaged cluster; `RefreshCw` icon in idle cluster toggles ephemeral session with amber "Temporary" badge in status area. Build verified green (`pnpm build` exit 0).
- **Temporary Chat Relocation & Notification Panel Performance/Redesign:** Moved ephemeral session toggle from `Home.tsx` idle cluster to `TopRightCluster` (home-only, `TemporaryChatIcon` dashed-bubble SVG with in-bubble tick, accent-only active state); dormant `StatusCapsule` is replaced (not stacked) by a greyscaled capsule-styled Temporary badge until engagement. Fixed notification rail open lag: deferred `markAllRead` IPC + store churn past the 220ms slide, removed per-card drop shadows repainting over the live ambient field. Reimagined `NotificationItem` as calm industry-standard rows (hairline borders, severity as quiet token labels, category blurb on icon tooltip, hover-reveal dismiss, 11px type floor, hardcoded Critical/Warning moved to `notificationCopy`). `pnpm build` green, impeccable detector clean.
- **Kokoro Voice Truth, Paused Text Auto-Resume & Orb/Ambient Polish:** Proved `voices.bin` stride is 523,264B/voice from sherpa-onnx v1.13.6 reader + model metadata (`style_dim` 511,1,256, `n_speakers` 11, ordered speaker names); corrected 4 mislabeled ledger entries (sid 0 Alloy, 1 Bella, 2 Nicole, 3 Sarah, 4 Sky — user was hearing Sky labeled Alloy); replaced sid 10 (Lewis) in-place with downloaded `af_heart` tensor (padded trailing row, size preserved so manifest integrity passes; backup at `voices.bin.bak11`); default voice → Heart (10). Kokoro now uses shipped us-en lexicon, crate-default `silence_scale` 0.2, and sid clamping against file voice count. `TextInput` auto-resumes from Paused in router (specs updated first per §4.3; dedicated test authored, green, then removed per user request — manual verification via new `[Pipeline::Router] TextInput while Paused` log line). Status badges map 1:1 to pipeline state (dropped Recording/Processing overlays, fixed unmapped Sleeping); ambient background freezes in Speaking; orb motion dt-normalized with calm-state snap, wider speak/listen scale (1.10/0.92) and hotter Speaking base. Full `nextest --release` 107/107 green, `clippy` clean, `pnpm build` green, detector clean.
- **Orb Glide Refinement & Dialogue Auto-Scroll:** Replaced the calm-state snap with a state-dependent glide (slow breathe 1.14/0.90 speak/listen, quicker settle to calm) so pause/interrupt melts smoothly instead of cutting; added missing auto-scroll to the home dialogue rail (`useHomePage` effect on history/streaming text, near-bottom guard preserves manual scroll-back). `pnpm build` green.
- **LLM→Playback Stream Trace Artifact:** Traced the full Modular TurnResponse seam in `docs/plans/phase11/llm-to-playback-stream-trace.md` (43 numbered steps, 8 phases: dispatch → token streaming → stream routing → clause rules → synthesis → playback gating/drain → finalization → side exits, every claim cited to file:line, externals marked). Appendix inventories all per-turn log metrics (RTF, clause chars/words, preroll samples, played samples, underrun counter, context utilization) and records the gap: no TTFT/TPS/latency metrics on the remote-LLM path (only embedded-local path logs them). Docs-only, no code touched.
- **Chunking Algorithm Appendix:** Added Appendix B to the trace doc with the full `ClauseChunker` algorithm (state, (5,8,12)→(10,15,20)→(16,24,32) threshold ladder, scan order, period/abbreviation guards, prosody-morph rewrite, emergency fallback, extraction loop, lifecycle). Corrected chunker ownership: instance is a `TurnAccumulator` field (`accumulator.rs:8`), router drives it through the accumulator lock — not router-owned as first written. Docs-only.
- **Orb Spring Dynamics, Dynamic Stage & Anti-Stutter Transition:** Eliminated 5-FPS lag upon pause/interrupt/idle by maintaining 60 FPS through an active 1400ms state transition window (`isTransitioning`) and elevating `fpsIdle` from 5 to 24 FPS for fluid idle drift. Replaced 1st-order asymmetric exponential scaling with a 2nd-order critically damped spring follower ($\omega_n=3.6$, $\zeta=0.92$), ensuring continuous acceleration without initial jerks or abrupt snap-backs. Fixed WebGL frustum and DOM clipping by adjusting Three.js camera fit distance ($z \ge 7.3$, ~14% vertical breathing headroom) and converting `Home.tsx` Orb Stage from `overflow-hidden` to dynamic auto-expanding layout (`maxWidth/maxHeight` 580px → 600px/640px when engaged/speaking). Verified clean with `pnpm build` exit 0.
- **Orb Scale Restraint, Dialogue Read-More & ResizeObserver Auto-Scroll:** Tamed orb over-expansion from 1.14 down to subtle 1.06 for Speaking and 0.96 for Listening; increased Three.js camera distance to $z=8.5$ with $1.52\times$ fit cushion, providing a 30% safety boundary that eliminates WebGL canvas flat-edge clipping. Created reusable `DialogueBubble` clamping dialogue history and streaming transcripts to 4 lines with interactive `... Read more` / `Show less` toggle; fixed broken auto-scroll in `useHomePage.ts` and `Home.tsx` by replacing the faulty distance calculation with a content `ResizeObserver` that automatically tracks streaming tokens and user message submissions while respecting manual scroll-back. Verified clean with `pnpm build` exit 0.
- **Kokoro Prosody Optimization, Text Normalization, Sleeping State Auto-Wake & Dictation Memory Standby:** Shipped generic text normalizer with `num2words` crate verbalizing currencies, times, units, versions, and ordinals before TTS synthesis. Optimized `ClauseChunker` for Kokoro by eliminating StyleTTS2 period-to-comma pitch mutations, enforcing word thresholds on `!` and `?` to prevent 1-word audio fragmentation, and elevating phrasing floor. Fixed UI status badge in `voiceDisplay.ts` so `Sleeping` is no longer masked as `Paused`. Enabled `TextInput` auto-resume from `Sleeping` in `router.rs`. Grounded `<user_identity>` memory contract in `DEFAULT_SYSTEM_PROMPT_MODULAR`. Wired `restartEngine` in `SettingsCardWrapper.tsx` and hot-reload in `ipc/settings/core.rs` for `tts.active`, `stt.active`, and `llm.active`. Updated `session.rs` to maintain VAD+STT hot in memory when dictation is enabled while cooling down LLM/TTS and calling `trim_heap` to release Linux heap memory. Verified clean across chunker/normalizer unit tests, `clippy --all-targets` (0 warnings), and `pnpm build` (exit 0).