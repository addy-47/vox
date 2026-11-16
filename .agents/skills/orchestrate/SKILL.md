---
name: orchestrate
description: Execute, steer, and monitor autonomous subagent loops, lifecycle state transitions, and milestone checkpoints for complex systems. Activate when spawning, managing, or coordinating subagents (Backend, Test, Review, QA) across implementation phases.
---

# Subagent Orchestration & Lifecycle Steering

This skill governs the **execution and real-time steering** of subagent loops. While `/create-loop` designs the skeleton (milestones, layers, and sign-off gates), `/orchestrate` governs the runtime mechanics: subagent spawning, boundary delegation, dynamic cron monitoring, tool economy enforcement, and adversarial sign-off gates. Mutation testing mechanics belong exclusively to the Test Engineer (`/mutate`).

---

## 1. The Orchestrator Mindset (Technical Lead / PM Standard)

The System Architect acts as a Principal Engineering Lead and Technical Project Manager:
- **Contract-Driven Delegation**: Lead through behavioral specifications, explicit file ownership, and verifiable completion criteria. Never micro-manage or dictate line-by-line syntax.
- **Context Boundary Discipline**:
  - **Shared / Forked Context (`fork: "<member>"` or subagent tools)**: Use for agents that need conversational context (e.g., continuing a multi-step design discussion or diagnosing a specific failure handoff).
  - **Clean-Room Independent Context (`Workspace: 'share'` or fresh subagent)**: Mandatory for adversarial reviewers and QA. Reviewers must have zero prior conversational bias and evaluate diffs strictly against specs and style guides.
- **Relentless Tool Economy**: The orchestrator is accountable for total wall-clock time and cargo lock contention. Enforce batched execution across all subagents.

---

## 2. The Anti-Spoon-Feeding Protocol

Never provide implementation code snippets, exact line-by-line edits, or verbose hand-holding in subagent briefs. Spoon-feeding causes context bloat and turns subagents into passive, non-thinking typists.

Every subagent brief or delegation prompt MUST provide **only the 4 Essential Anchors**:

1. **Role & Skill Gating**:
   - Explicit persona to load from `.agents/rules/<persona>.md`.
   - Explicit skill to exploit (e.g. `/implement`, `/create-test`, `/review`, `/rca`).
   - **Persistent Persona Invariant**: Re-use the existing persistent subagent for each role (single Backend Engineer, single Test Engineer) across all batches of a phase via `send_message`. Never spawn duplicate subagents per micro-task.
2. **Authoritative Context Pointers**:
   - Direct pointer to the authoritative specification (`docs/specs/<spec>.md`).
   - Direct pointer to the implementation plan (`docs/plans/<phase>/<plan>.md`).
3. **Explicit File Boundaries & Blast Radius**:
   - Strict whitelist of files the subagent owns and may modify.
   - Strict blacklist of files the subagent is **forbidden** from touching (e.g., Backend Engineer is strictly forbidden from modifying `tests/**`; Test Engineer is forbidden from modifying `src/**`).
4. **Binary Observable Completion Gate**:
   - The exact terminal command or test binary that defines done (e.g., `cargo test --test <binary> --release passes with 0 failures`).
   - Mandatory instruction: "Commit changes upon passing gate, report outcome, and stop."

### 2.1 Upstream Verification Handoff (Preventing Redundant Re-Compiles)

When the orchestrator hands off work between lifecycle roles (e.g., Backend Engineer $\to$ Test Engineer $\to$ Reviewer), it MUST explicitly summarize and relay upstream verification state:
- **Relay Commit Hash & Verified Status**: State the exact commit hash and what the upstream agent already validated (e.g. `"Backend Engineer verified verify-targets.sh desktop clean (0 warnings) and passed unit tests at commit abc1234"`).
- **Forbid Redundant Re-Runs**: Explicitly instruct the downstream agent: `"Upstream compilation and baseline tests are already verified at commit abc1234. Do NOT re-run full compiler checks or test suites before writing your code or tests."`
- **Pass File Manifest**: Provide the exact list of modified files so the downstream agent immediately knows the blast radius without blind searching.

---

## 3. Dynamic Schedule & Proactive Cron Check-Ins (`schedule` tool)

Do not leave subagents unmonitored, and **never use shell sleep or busy polling loops** to wait on subagents or cargo compilations. Instead, act like an engineering lead with scheduled check-ins calibrated to estimated task horizons:

### 3.1 Horizon-Calibrated Check-In Intervals

Before delegating, estimate the task duration and schedule an automated cron via the `schedule` tool:

| Estimated Duration | Cron Expression / Interval | Purpose |
| :--- | :--- | :--- |
| **Short (< 30 min)** | No cron; rely on native reactive wakeup on subagent completion. | Avoid unnecessary context switching. |
| **Medium (30–90 min)** | Recurring cron every 15–20 minutes (`*/20 * * * *`). | Interrogate progress, verify compile discipline, detect hung commands. |
| **Long (2–4 hours+)** | Recurring cron every 30–45 minutes (`*/30 * * * *`). | Audit completed batches, check resource usage, verify gate compliance. |

### 3.2 Interrogation on Cron Trigger

When the scheduled cron fires, inspect subagent progress and send a structured, adversarial check-in via `send_message` if needed:
- Has the subagent encountered a blocker or failing test?
- Is the subagent stuck in an interactive/hanging command (e.g. `cargo nextest list`)?
- Is the subagent adhering to the "Batch First, Compile Once" rule, or burning tokens on incremental recompilations?
- Has the subagent attempted two failed hypotheses without escalating?

Cancel the schedule immediately via `manage_task` once the phase gate is satisfied.

---

## 4. Subagent Execution Topologies

Do not restrict orchestration to a single rigid sequence. Select or compose the topology matching the domain's blast radius:

```
Pattern A (TDD / Red-Seam First):
  [Architecture/Spec] ──> [Minimal Stubs] ──> [Red Seam Tests] ──> [Green Implementation] ──> [Mutate & Review]

Pattern B (Sequential Implementation First):
  [Architecture/Spec] ──> [Atomic Production Batches] ──> [Black-Box Seam Tests] ──> [Adversarial Review]

Pattern C (Component Sharding across Git Worktrees):
  [Architecture/Spec] ──┬──> [Worktree A: Module 1 Implementation]
                        └──> [Worktree B: Module 2 Implementation] ──> [Merge & Integration Seam Gate]

Pattern D (Diagnostic / RCA Pipeline):
  [Incident/Bug] ──> [Diagnostic Probe Subagent] ──> [Root Cause Confirmed] ──> [Targeted Fix & Regression Test]
```

### 4.1 Topology Pattern A: Red-Seam First / TDD (Canonical for Behavioral Additions)
*Best for:* Well-defined behavioral additions, state-machine transitions, concurrency barriers, or persistence contracts.
*Lifecycle:*
1. **Architecture / Spec**: System Architect locks specifications and black-box seam intent blocks.
2. **Minimal Signatures**: Backend Engineer writes *only* public struct, enum, and method stubs so types compile.
3. **Test Engineer (RED)**: Authors black-box seam subtests against the stubs. Runs `cargo test` once to empirically prove all fail (Red).
4. **Backend Engineer (GREEN)**: Implements production logic until all seam tests pass (Green). **Forbidden from touching `tests/**`**.
5. **Mutation & Adversarial Review**: Test Engineer validates mutant kills; Reviewer executes final clean-room audit.

### 4.2 Topology Pattern B: Sequential Implementation First (Canonical for Foundational Overhauls)
*Best for:* Major architectural refactors, widespread interface replacements, or zero-backward-compatibility cleanups where the existing test suite cannot compile against legacy signatures until new production types land.
*Lifecycle:*
1. **Architecture / Spec**: System Architect locks specs, data schemas, and contracts.
2. **Backend Engineer**: Implements atomic production batches until clean compile.
3. **Test Engineer**: Authors black-box seam tests proving behavior against the new seam.
4. **Adversarial Review**: Clean-room audit, mutation verification, and final sign-off.

### 4.3 Topology Pattern C: Worktree Sharding & Offloaded Execution
*Best for:* Independent subsystem changes or heavy computational campaigns (e.g. mutation testing, benchmark suites).
- Uses `git worktree add` to spawn parallel authoring environments without git or cargo lock contention.
- Changes are merged into the main working tree upon passing their respective gates.

---

## 5. Tool Economy & Compilation Invariants

The orchestrator must strictly enforce the following execution rules across all spawned subagents:

### 5.1 Batch First, Compile Once, Test Once
- **Forbidden**: Running `cargo check`, `cargo build`, or `cargo test` after every small file edit or batch.
- **Mandatory**: The implementation subagent must write all modified files for the assigned phase first. Then, invoke the compiler **once** (`./scripts/verify-targets.sh` or `cargo test --no-run --release`). Fix any errors or warnings in a single sweep, recompile, and run the test binary **once**.

### 5.2 Hard Ban on Interactive / Hanging Commands
- 🛑 **NEVER RUN `cargo nextest list` UNDER ANY CIRCUMSTANCES.**
  It is an interactive command that hangs indefinitely in headless agent shells.
- 🛑 **NEVER RUN SINGLE SUBTESTS SEQUENTIALLY IN A LOOP.**
  Executing `cargo test --test <binary> <subtest>` 10 times consecutively burns 30+ minutes of redundant compilation. Always run the entire test binary in a single command.

---

## 6. Adversarial Reviewer as Independent Blocking Gate

The Reviewer subagent is the final quality gate before milestone completion:
- **Independent Context**: The Reviewer must be an independent subagent with fresh eyes, having zero prior conversational involvement with the implementation subagent.
- **Diff & Invariant Audit Only**: The Reviewer's input is `git diff origin/main...HEAD`. It audits code hygiene, architectural invariants, and test validity (`create-test` intent blocks, no false-greens, no white-box hacks).
- **No Redundant Re-Compiles**: If the Test Engineer has already verified compilation and passed all tests, the Reviewer MUST NOT re-run `./scripts/verify-targets.sh` or full test suites unless validating an identified defect.
