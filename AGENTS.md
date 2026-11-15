# AGENTS.md — Vox Workspace Rules

---

## 1. MANDATORY: Work Log Hook

> 🛑 **POST-TASK HOOK: Record Work in `work_log.md`**
> After every completed task, record your entry in `docs/plans/<current_phase>/work_log.md`.

---

## 2. Key Paths

Only paths with non-obvious rules that agents cannot infer from the codebase index.

| Path | Rules |
| ---- | ----- |
| `app/src-tauri/src/` | No test logic. No benchmarks. Rust grammar order: `backend-style-guide.md §2.1`. |
| `app/src-tauri/tests/` | Named `<feature>_test.rs`. Public `vox_lib` API only. |
| `app/src-tauri/scripts/verify-targets.sh` | **The compile gate.** Always use this — never bare `cargo check` on a cross target. |
| `.agents/rules/` | Read the relevant rule file before acting in any named role. `android-style-guide.md` is mandatory for any change touching `cpal`, tray/menu, dictation, `utils/paths.rs`, model loading, `Cargo.toml` target tables, or any `cfg(`. |
| `docs/specs/` | Authoritative behavioral contracts — SSOT. Never contradict. |
| `docs/plans/phase14/` | Active phase. Plans and `work_log.md` live here. |
| `temp/.env` / `temp/server.txt` | API keys and remote GPU server creds. Not versioned. |
| `~/.vox/vox.db` | Local Turso DB. Inspect via `tursodb ~/.vox/vox.db "<SQL>"`. |

**Remote GPU server:** creds in `temp/server.txt`. **Never kill running server processes.**

---

## 3. Execution & Testing Invariants

0. **Compile Gate:** Always use script below , NEVER any shallow or ad-hoc cargo checks WITHOUT --release TO VERIFY AS THEY WILL COMPILE IN DEBUG AND WILL NOT USE OUR CACHE.
   ```bash
   cd app/src-tauri && ./scripts/verify-targets.sh
   ```
1. **Sequential + Release:** Run benchmarks, evals, test suites, or any cargo command one at a time. Always `--release`.
2. **Test runner (`cargo-nextest`):**
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
   > ⏱️ Full suite baseline: ~45.5s (excluding cold compile). Set a timeout or cron — never leave a run unmonitored.
3. **External-dependency tests (`#[ignore]`):** Run only with explicit user approval:
   ```bash
   cargo nextest run -- --ignored
   ```
> **Note for every agent:** Polling cargo compilation commands is forbidden , always let the system inform you . 'cargo nextest list' should never be run as it hangs .

---

## 4. Invariants & Gates

### 4.1 Architectural Invariants (Non-Negotiable)

**DATABASE ENGINE (NON-NEGOTIABLE):** WE DO NOT USE SQLITE. Vox exclusively uses the **Turso Database Engine** (`turso` crate: pure-Rust, async-native, MVCC concurrent writers). Never assume SQLite, C toolchains, or rusqlite locking constraints. Reference: `docs/features/turso-database-engine.md`.

**Zero Backward Compatibility (ZBC):** Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers or legacy paths.

**Local Turso CLI:**
```bash
tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
```

### 4.2 Skill & Rule Gating (Mandatory Autonomous Skill Exploitation)

> 🛑 **Autonomous Skill & Rule Loading:** Vox provides 40+ specialized skills in `.agents/skills/` and `~/.gemini/config/skills/`. Agents MUST autonomously search, read, and enforce the matching skill and engineer rule files in `.agents/rules/` before acting in any role:
> - **Architecture & System Design:** Read `/architect` and enforce `architectural-invariants.md`.
> - **Writing / Modifying Tests:** Read `/create-test` and enforce `testing-style-guide.md`.
> - **Code Implementation & Refactoring:** Read `/implement` and the layer's rule file (`backend-style-guide.md`, `android-style-guide.md`, etc.).
> - **Adversarial Code & Test Review:** Read `/review`, `/review-ui`, and `testing-style-guide.md`.
> - **Debugging & Root Cause Analysis:** Read `/rca`.
> Never wait for explicit user prompting to load required skills. If a task involves a known engineering activity, exploit the corresponding skill immediately.

### 4.3 Code Style and Engineering Integrity (Non-Negotiable)

**CSG-01 — Code Hygiene:** No inline crate imports or direct crate:: path usage, excessive comments, magic numbers, or hardcoded strings and paths. Follow established repository conventions and use explicit configuration where appropriate.

**CSG-02 — Test Discipline:** Follow the test discipline and skill workflow defined in §4.2. Never write or modify tests without first reading `testing-style-guide.md` and `/create-test`.

**CSG-03 — Root Cause Over Bandaids (First-Step Diagnostic Gate):** Never patch symptoms while leaving the underlying problem unresolved. Before writing or proposing any code change, every agent MUST pause at the initial thinking step and answer these 5 mandatory diagnostic questions:
1. *What is the fundamental root cause, and is this failure caused upstream (e.g. invalid test asset, artificial delay, test harness pacing, missing prerequisite)?*
2. *Am I fixing the root cause, or patching a downstream symptom to make a test pass?*
3. *Is this solving an extremely rare synthetic edge case at the cost of production simplicity?*
4. *Is this problem already solved by established industry libraries, models, or standard protocols?*
5. *Am I hand-rolling heuristic math or complex state logic that should belong to an existing crate or neural model?*
If the failure is caused by an upstream or faulty test input, STOP immediately and report it. Never introduce ad hoc conditionals, magic multipliers, duplicate state buffers, or downstream hacks to appease a broken test. Identify the root cause and fix it at the correct architectural layer.

**CSG-04 — No Yes-Man Behavior:** Never agree with the user, orchestrator, or another agent merely to be compliant. Independently evaluate every proposed solution. Challenge incorrect assumptions, flawed designs, ambiguous requirements, and poor technical decisions. Explain why an approach is flawed and propose a better alternative. Prioritize technical correctness over agreement.

**CSG-05 — Architectural Integrity:** No hacks, architectural smells, hidden coupling, duplicated business logic, or bypassed abstractions for implementation convenience. Respect separation of concerns, module boundaries, ownership, and established contracts. Choose the smallest *sound* solution, not simply the smallest diff. Do not overengineer or introduce unnecessary abstractions either.

**CSG-06 — No Silent Compromises:** Never conceal design flaws, ignore architectural conflicts, or invent requirements to force an implementation through. Surface material risks and seek clarification when necessary. Do not claim completion without validating the implementation and reporting unresolved issues.

**Core principle:** Think critically before implementing. Diagnose before patching. Challenge before agreeing. Design before hacking. Engineering integrity takes precedence over convenience and blind compliance.

### 4.4 Subagent Orchestration & Execution Invariants (Non-Negotiable)

1. **Strictly Sequential Lifecycle (Zero Parallel Spawning):**
   Subagents MUST execute in a strict sequential dependency order. Never launch implementation, testing, or review subagents concurrently or in parallel. Every phase must pass its completion gate and compile cleanly before the next agent is spawned.

   System architects select one of two governing orchestration workflows based on the domain's blast radius and initial state:

   - **Workflow Option A: Sequential Implementation First**
     $$\text{Architecture / Spec} \longrightarrow \text{Atomic Implementation} \longrightarrow \text{Test Engineering} \longrightarrow \text{Adversarial Review}$$
     *Best for:* Major foundational refactors, widespread interface replacements, or zero-backward-compatibility cleanups where the existing test suite cannot compile against legacy signatures until new production types land.
     *Lifecycle:*
     1. Architecture / Spec: System Architect locks specs, data schemas, and contracts.
     2. Backend Engineer: Implements atomic production batches until clean compile.
     3. Test Engineer: Authors black-box seam tests proving behavior against the new seam.
     4. Review / QA: Adversarial audit, mutation testing, and final sign-off.

   - **Workflow Option B: Red-Seam First / Test-Driven Development (TDD)**
     $$\text{Architecture / Spec} \longrightarrow \text{Test Stubs (Red)} \longrightarrow \text{Atomic Implementation (Green)} \longrightarrow \text{Adversarial Review / Mutate}$$
     *Best for:* Well-defined behavioral additions, state-machine transitions, concurrency barriers, or persistence contracts where public contracts can be stubbed upfront.
     *Rationale:* Eliminates white-box implementation anchoring (`create-test` Phase 0) by writing assertions before production logic exists, preventing tests from being calibrated to developer shortcuts.
     *Lifecycle:*
     1. Architecture / Spec: System Architect locks specs and specifies exact black-box seam intent blocks.
     2. Minimal Signatures: Backend Engineer writes *only* public struct, enum, and method stubs to make types compile.
     3. Test Engineer (RED): Writes black-box seam subtests against the stubs. Runs `cargo nextest` to empirically prove tests fail (Red).
     4. Backend Engineer (GREEN): Implements production code until all seam tests pass (Green). **Forbidden from touching `tests/**`**.
     5. Review / QA: Adversarial audit, circuit breaker verification, and mutation proof.

2. **Mandatory Persona & Skill Gating for Every Subagent:**
   Every spawned subagent MUST autonomously read its assigned persona from `.agents/rules/` and load its respective skills before inspecting or editing files. Orchestrators must supply subagents with explicit boundaries, verbatim reviews, and clear batch criteria.

3. **Adversarial Reviewer as Final Blocking Gate:**
   The Review subagent MUST run strictly at the end, after all implementation and tests are complete. It must review BOTH production code AND test code against style guides and skills (per §4.2):
   - Review production code for concurrency leaks, single-writer invariants, and memory bugs.
   - Review tests for production wiring parity (no bypassing `spawn_router` with direct handler calls), zero bare sleeps, single seam binary discipline, and valid `ASSERTS:` claims.

4. **Context Handoff & Failure Escalation:**
   Parent orchestrators must supply subagents with explicit boundaries, verbatim reviews, and clear batch criteria. If a subagent encounters an upstream architectural defect or regression, it must stop and escalate rather than writing downstream mocks or bypassing gates.

---

## 5. Specifications & Spec Alignment (Non-Drift Gate)

All authoritative specifications live in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/). Reading the relevant spec is mandatory before designing or modifying code.

> 🛑 **Spec Alignment Invariant:** Every agent must adhere strictly to the approved specifications.
> 1. **Divergence / Additions:** If behavior, commands, or schemas diverge from or are absent from the relevant spec — STOP and ask for approval. If approved, update the spec FIRST (following `/create-spec`), then write code. Specs must never quietly drift from code.
> 2. **Legacy Code:** If existing code implements behavior not captured in the spec — confirm with the user before pruning the code or updating the spec.

---

## 6. Current Phase & Work Log

> 📖 **Phase 14 (active):** All completed work and previous agent activity are recorded in [`docs/plans/phase14/work_log.md`](file:///home/addy/projects/apps/vox/docs/plans/phase14/work_log.md).
> Read `work_log.md` to understand recent milestones, established architectural patterns, and verified seam implementations.

---

## 7. Backlog

See **[Phase 14 Backlog](file:///home/addy/projects/apps/vox/docs/plans/phase14/backlog.md)** for Cloud TTS quota management, Azure speech integration, and setup guide modal overhaul details.
- **Backend Ticket**: [Native Barcode Scanner Plugin](file:///home/addy/projects/apps/vox/.scratch/ticket_tauri_barcode_scanner.md) (`tauri-plugin-barcode-scanner`) for Android/iOS ML Kit camera integration.

---
