# AGENTS.md — Vox Cognitive Memory System & Evaluation Framework

## 1. Project Overview & Current State

Vox is a voice-native, agent-first AI platform built on a real-time, event-driven native pipeline. The system features a **v4 Hybrid Cognitive Memory Subsystem** comprising:
1. **Working Memory**: Transient RAM-based context window management with token-budgeted compaction.
2. **Personal Memory**: Long-term structured memory spanning 10 collections (`Identity`, `Constraints`, `Preferences`, `Relationships`, `Skills`, `Projects`, `Experiences`, `Context`, `Tasks`, `Goals`) with a Directed Relations Graph (`SUPPORTS`, `CONFLICTS`, `USER_SUPERSEDES`, `SIMILAR`, `MERGED`).

### Current Phase Focus: Compaction & Extraction Isolation
* **Primary Target**: Eliminate **Fact Duplication** and **Inconsistent Collection Placement** during session compactions.
* **Testing Scope**: Sandbox isolated testing using `sandbox/dataset_session1.json`, `sandbox/dataset_session2.json`, and `sandbox/dataset_session3.json`.

---

## 2. Agent Roles & Operational Rules

### 2.1 System Architect (Parent / Lead Agent)
* **Role**: Strategy, intent alignment, system architecture, root cause analysis, harness engineering, and specification drafting.
* **Constraints**:
  * **MUST NEVER** make direct source code fixes in the backend codebase (`app/src-tauri/src/`).
  * **MUST NEVER** review or self-approve test results.
  * **MUST ALWAYS** delegate code changes to `backend-engineer` and test execution/audits to `qa-engineer`.

### 2.2 QA Engineer (`.agents/rules/qa-engineer.md`)
* **Role**: Independently executes test suites, inspects raw outputs/logs, conducts semantic evaluations, and audits extraction quality.
* **Constraints**:
  * **MUST NEVER** confuse `exit code 0` or script completion with test success.
  * **MUST NEVER** use mock data or fake success fallbacks.
  * Must evaluate facts based on duplication percentage, collection placement consistency, and semantic accuracy.

### 2.3 Backend Engineer (`.agents/rules/backend-engineer.md`)
* **Role**: Implements surgical, minimal Rust and Python backend changes once specifications and plans are approved.
* **Constraints**:
  * **MUST NOT** refactor opportunistically.
  * Must verify all changes with `cargo check` / `cargo clippy` and unit/integration tests before reporting completion.

### 2.4 Context-Isolated Review Agents
* Fresh sub-agents spawned to perform un-biased reviews of test results or architectural plans without context contamination from long conversations.

---

## 3. Evaluation & Verification Guidelines (Banned vs. Required)

### ❌ BANS (Never Accept As Success)
* **Exit Code 0**: A script finishing without throwing an exception tells us nothing about memory accuracy.
* **Vanity Counts**: "50 facts extracted" or "12 edges created" is irrelevant if 30 facts are duplicates or placed in the wrong collections.
* **Self-Approval**: The agent that ran the test or wrote the code must never approve the output.
* **Unapproved Model/API Switches**: Switching between Gemini, NVIDIA, or Remote Ollama without explicit user notification is strictly forbidden.

### ✅ MANDATORY METRICS & EVALUATION CRITERIA
* **Fact Duplication Rate (%)**: Percentage of facts extracted during compaction that repeat already-known facts or duplicate each other within the same run.
* **Collection Placement Consistency**: Whether identical fact types (e.g. "User is allergic to peanuts") are consistently assigned to the same collection (e.g. `Constraints`) or jump between `Preferences`, `Identity`, etc.
* **False Positive NLI Error Rate**: Rate of incorrect `CONFLICTS` or `SUPPORTS` edges generated downstream due to duplicate/near-duplicate inputs.
* **Semantic Retrieval Precision**: Relevance of retrieved memory context relative to user queries.

---

## 4. Inference Provider & Environment Governance

* **Credentials**: Stored in `temp/.env` (`NVIDIA_API_KEY`, `GEMINI_API_KEY`).
* **Remote Inference**: Detailed in `temp/server.txt` (`hypr4@100.86.62.14`).
* **Policy**:
  1. Gemini API is used for setting initial baselines.
  2. NVIDIA API is preferred for major evaluation sweeps and LLM-as-a-judge scoring.
  3. Remote Server is used for local model testing when idle and verified.
  4. **Rule**: Always notify and receive approval before changing inference providers or model endpoints.

---

## 5. Key Documentation & Sources of Truth

* `docs/plans/personal-memory-spec.md` — Master Cognitive Memory Specification (v4).
* `app/src-tauri/src/services/memory/` — Memory subsystem implementation (Rust).
* `sandbox/` — Datasets and python test/audit tools.
* `.agents/rules/` — Sub-agent behavioral rules.
