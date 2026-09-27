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

- **RAM measurement fix (cgroup contamination removal):** Resolved 4.5–5.0 GB misreported memory in monitoring by eliminating host cgroup fallback; systemd user-slice `memory.current` (5.1 GB) was leaking desktop/IDE memory into Vox's metrics; anchored `resident_bytes()` strictly to measured process tree RSS (`scope.process.rss_bytes`), filtered non-Vox systemd scopes from `read_cgroup_path`, and prioritized `total_vox_ram_mb` over `cgroup_current_mb` in frontend profiler hooks; memory readout now accurately reflects actual ~1.8 GB launch footprint.
- **Personal memory suggestion review & candidate facts staging UI:** Built Google Docs-style suggestion diff review (`SuggestionCard`) with individual Accept (✓) and Reject (✕) toggles, atomic batch resolution (`resolve_memory_suggestions`), and active facts queue inspector (`LearnedFactsList`); preserved idle staging cards (Comment, Import, Edit); renamed consolidation trigger to "Integrate Learned Facts" and added header candidate facts toggle visible only when `unconsolidatedCount > 0`; updated Tauri IPC command and service to process batch decisions in a single transaction.
- **Indexed-block memory consolidation & eval modularization:** Replaced fragile string-matching delta patching with index-addressed content-element block edits (`[1] ## Heading`, `[2] - Bullet`) and atomic ops (`insert_after`, `replace`, `delete`); separated prompts into Cold Start (Prompt 1), Incremental Integration (Prompt 2), User Directives (Prompt 3), and Document Regeneration (Prompt 4); implemented Schema v8 (`target_index`, `content`) dropping `source_fact_ids`, `section`, and `target_text`; wired transactional arithmetic index re-anchoring ($\pm 1$) and direct fact transitions to `'consolidated'` (Invariants 5.3-A/B); exposed `regenerate_personal_memory` IPC command and frontend bindings; modularized `memory_pipeline_eval.rs` by extracting calibration, consolidation judge, preservation, and pipeline reporting into `evals/common/` (reducing line count from 1,834 to 586); all targets compile cleanly.
- **Agentic runtime, web search & tool ecosystem:** Shipped full tool taxonomy, reentrant cognitive loop, session gating, provider wire policy mapping, and 122 integration tests with 73% mutation kill rate; published `nexuss` (v0.1.1 on crates.io) and wired `web_search` with adaptive fanout quorum and hybrid ranking (2.16s latency).
- **Test suite seam audit + remediation (139 functions, 23 sprints):** Audited 62 unit and 77 integration tests against rubric; shipped `integration-test-spec.md` v2.5; codified Invariants 5.3-A/B; added 9 tests, deleted 8 dead tests, killed 5 seeded mutants, and resolved pre-existing MVCC test failure.
- **Settings desk overhaul, memory RSS RCA & ambient tuning:** Rebuilt settings tabs into 65-35 two-column desks with wireframe vector SVGs; measured 686 MB Vox RSS / 470 MB PSS in controlled idle benchmark; tuned ambient background ripple cycles and fixed pause state on speaking; confirmed active engine Kokoro v0.19 ONNX manifest consistency.
- **Global hotkey UI redesign, Alt+V default & dictation flow tracing:** Redesigned `DictationConfigDesk` into a balanced 2-column layout eliminating cramped horizontal overflow that obscured the Save button; added interactive `<kbd>` key badges, zero-jump recorder state, prominent full-width Save/Cancel buttons, and one-click default reset; changed default dictation hotkey from `Alt+Space` to `Alt+V` across backend defaults and frontend copy; identified root causes for silent hotkey failure under Wayland (`XGrabKey` isolation on native Wayland windows and inactive audio engine on cold PTT boot); wired auto on-demand audio engine launch and explicit `InteractionOwner::Dictation` store on hotkey press; instrumented comprehensive `[Dictation::Trace]` logs covering hotkey hooks, router dispatch, VAD speech validation, STT emission, transliteration, and clipboard/paste output routing.
- **Linux Wayland automated global hotkey daemon & dictation doc update:** Resolved background hotkey interception on GNOME Wayland by building a zero-configuration native integration; spawned local Unix domain socket listener at `~/.vox/vox.sock` and generated executable launcher `~/.vox/bin/vox-trigger`; wired automated GNOME media-keys registration and dynamic sync via `gsettings` (`org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/.../vox-dictation/`) mapping `<Alt>v` to the socket trigger; added `HotkeyAction::Toggle` resolving to `Press` (PTT start) or `Release` (PTT stop / transcribe / paste) based on active state, complementing Smart VAD silence auto-finalization; documented the complete Wayland compositor isolation RCA and socket daemon architecture in `docs/features/dictation.md`.
- **Dictation hotkey debounce & log noisiness remediation:** Eliminated rapid turn cascades and log flooding caused by GNOME compositor 30Hz keyboard auto-repeat and overlapping PTT triggers; added 400ms timestamp lock debounce in `vox-trigger`, 350ms throttle in Unix socket listener, and 400ms cooldown window in `hotkey.rs` worker; prohibited pipelined turn overlap while in `InteractionState::Thinking`; dropped noisy notifications for empty/non-speech transcripts and sub-100ms audio (<1600 samples); downgraded high-frequency routing and VAD trace logs from `INFO` to `DEBUG`, leaving clean single-line lifecycle milestones.




