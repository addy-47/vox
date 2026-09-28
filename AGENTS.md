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

> 📖 **MODEL-AGNOSTICISM DEFINITION OF DONE (Phase 12+):** Adding a model = edit the manifest + write the engine ([checklist](file:///home/addy/projects/apps/vox/docs/plans/phase12/model-registry-checklist.md)). No `app/src/**` edit, no capability table copy, no `as any`, no unconsumed capability field — Invariants 6 and 8 fail the build otherwise.

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
8. **[Interaction Track Ownership & Focus Lifecycle Specification (v1)](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md)** — *Status: Approved Target Spec*. Strict IPC-driven ownership model, zero hotkey owner mutation, state-check over settings-check invariant, and deterministic hotkey preemption gating.

---

## 5. Phase 12 — Recent Work Summary

> 📖 **Full History:** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md) | Phase 11 Archive: [phase11/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)

- **Agentic runtime & tool taxonomy:** Reentrant tool loop, 122 integration tests (73% mutation kill), `nexuss` v0.1.1 with adaptive fanout, and structured JSON provider wire policies.
- **Cognitive memory stack & eval modularization:** Indexed-block consolidation (Schema v8, 4-prompt split), suggestion review with atomic block ops, and Approach 4 JSON semantic personal memory spec/plan.
- **Platform ownership & dictation overhaul:** Owner-stamped `VoxEvent` pipeline eliminating TOCTOU races, deterministic hotkey preemption, resident notification lifecycle, and acoustic chronometer UI.
- **Model-agnostic TTS stack (Batches 0.5–4, 2026-09-28):** Native `ZipvoiceEngine`, 6-batch decoupling plan; `tts.quality_steps` deleted end-to-end; `fn caps()` trait contract with per-engine `SPEED_RANGE`; `caps_for_id` returns `Result`; `list_voices` provider scoping with `slug` column (schema v9); single-seed voice names; Invariants 6–7; fallback/dead-code deletions (`FALLBACK_CAPS`, `TtsProviderKind`, FE prefix filters, phantom union). Full detail in `recent_work.md` §2026-09-28.
- **Batches 7–10 sprint completion (2026-09-28):** Batch 7 deleted the realtime dual-shape (`Record<string,any>` config, 8-literal branch, legacy reads in 4 files; shape-driven `in`-narrowing, zero casts). Batch 8 audited all 27 settings structs field-by-field — deleted dead `auto_sleep_timeout`/`log_level`/`telemetry_enabled`/STT credential blobs/`resume_handle` (rehomed to constructor param, spec line fixed), killed 7 FE phantoms, exact mirror verified by script. Batch 9 unionized `output_mode`, centralized the `"default"` sentinel. Batch 10: model-registry checklist + DoD gate in §4.2. Full verification green: clippy clean, pnpm build clean, vitest 8/8, nextest 149/149 (4 ignored externals skipped). One real bug found by the suite (v9 migration assumed `voices` exists; guarded by sqlite_master check).

---

## 6. Backlog

- **[STT / RCA Needed] Nemotron-3.5 initial boundary clipping:** During TTS reference pack verification via `stt_bench`, Nemotron-3.5 dropped leading tokens on abrupt audio starts (e.g. dropped "The" on `voice_02.wav` and "I didn't" on `voice_05.wav`) where Qwen3-ASR detected them; requires RCA on VAD chunking and CTC prefix blank search window.
- **[TTS / RCA Needed] ZipVoice voice quality & acoustic profile:** Synthesized audio exhibits muffled frequency response due to reference audio cutoff (<4 kHz dominance) and ODE trajectory blur at >4 steps; requires testing 4-step distilled inference with high-frequency shelved/clean reference clips.
- **[Benchmark] Pipeline bench & Kokoro vs ZipVoice comparative report:** Execute `pipeline_bench` sequentially across Kokoro and ZipVoice baselines and produce comparative TTFA, RTF, E2E latency, and audio quality assessment report.

