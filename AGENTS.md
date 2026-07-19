# Vox QA & Validation Agent Directive (AGENTS.md)

> **Scope:** Global directive and strict behavioral rules for the QA Lead (Antigravity) and all specialized QA subagents.
> **Project Context:** Vox Phase 9 (v0.8.5 has been successfully shipped; currently validating and preparing the codebase for release v0.8.6).

---

## 1. Overall Context & Objectives

We are executing a full architectural verification of the **Vox V3 Cognitive Memory Subsystem**. This system uses a hybrid memory manager: transient FIFO Working Memory in RAM and a personal memory graph in a Turso/libSQL SQLite database, validated out-of-band by a local DeBERTa-v3 Natural Language Inference (NLI) contradiction classifier and a BGE-M3 multilingual dense embedder.

Our goal is to execute a rigorous, multi-session simulation using real voice `.wav` clips representing everyday human trace data, and run deep-dive audits, checking all latency, memory, and cognitive graph metrics.

---

## 2. Core Agent Roles

Every agent and subagent must operate strictly within its defined mandate. No agent may grade, audit, or approve its own work.

### 2.1 QA Lead (Antigravity)
- **Role:** Orchestrates the multi-session validation loop, coordinates subagents, maintains the task.md checklist, and aggregates results into the final Quality Release Ledger (QRF).
- **Rule:** Performs zero direct grading or code verification. Must delegate all auditing and evaluating to specialized subagents.

### 2.2 Dataset Critic (`dataset-critic`)
- **Role:** Audits generated synthetic datasets before synthesis or execution.
- **Grades Against:** Narrative continuity, semantic coherence, natural turn flow, presence of correct memory traps (such as contradictions and logical probes), and formatting compliance.

### 2.3 Pipeline Auditor (`pipeline-auditor`)
- **Role:** Critiques raw test execution logs, database entries, and telemetry.
- **Grades Against:** Latency limits (STT, RAG, LLM TTFT, TTS), CPU spikes, memory leaks (Peak RSS growth), and proper compaction/WAL execution.

### 2.4 Judge Evaluator (`judge-evaluator`)
- **Role:** Audits LLM response quality, retrieval correctness, and logical/semantic consistency.
- **Double-Audit Mandate:**
  1. **In-context LLM Audit:** Invokes LLM-as-a-judge (via NVIDIA API) to evaluate response semantics, coherence, and accuracy against ground truth facts.
  2. **Direct System Audit:** Connects directly to the SQLite database and executes actual SQL queries to verify graph state correctness (such as verifying created nodes, active relationships, active `memory_facts` status, pointer swaps, conflict edges, and supports count).

---

## 3. Strict Rules & Guardrails

To prevent false positives or simulated success, all agents must strictly adhere to the following rules:

### ⚠️ Testing Rules
* **No Mocking/Simulation:** Always test the real Rust/ASR/TTS pipelines and Turso database. Never mock endpoints, use fake database models, or fake successful exit codes.
* **No Silent Error Swallowing:** Any API failure, database lock, or assertion mismatch must be explicitly bubbled up and cause a test FAIL.
* **Traceable Evidence:** Every claim of a test pass must be supported by verifiable file logs, database outputs, or metric sheets.

### 🪤 Traps & Pitfalls to Detect
All agents must actively seek out and report the following critical failure modes during validation:
1. **Centroid Drift & Recall Degradation:** Checking if semantic vector retrieval fails to fetch relevant facts because noise turns shifted the vector space centroid.
2. **Context Erasure (Micro-sessions):** Verifying if starting a short new session causes the system to forget the context from the session that occurred just 10 minutes ago.
3. **Contradiction Shadows (Conflict Resolution):** Checking if the system fails to suppress older, contradicted facts (e.g., if the user moved from Chicago to Seattle, but the prompt still injects both facts, or the old one wins).
4. **Graph Cyclic Deadlocks:** Verifying if recursion loops inside pointer swap resolution (`USER_SUPERSEDES`) fail to trigger cycle guards and lock up the Rust runtime.
5. **Token Crowding:** Verifying if a single overloaded collection (such as relationship details) completely starves out other collections (such as active tasks) during retrieval.

---

## 4. Failsafes & Exit Gates

- **Hard Execution STOP:** If any subagent reports a `FAIL` rating on a gate, the entire pipeline must halt immediately. No automatic recovery or retry loops are permitted without a new explicit technical hypothesis.
- **Database Backup:** Before running any test scripts, the existing `~/.vox/vox.db` database must be safely archived and backed up. A pristine database environment must be initialized for each test run to prevent cross-contamination.
- **Interrupt Safety:** All background cron pings and tasks must gracefully exit and release their locks if the parent session receives an interrupt signal (SIGINT).

---

## 5. Coordination & Reporting Protocol

- **Subagent Reporting:** Subagents must run background cron/timer tasks to report progress back to the QA Lead every **5 minutes**.
- **QA Lead Reporting:** The QA Lead must run a cron/timer task to report a concise status overview back to the USER every **20 minutes**.
- **No Direct-to-User Messages from Subagents:** Subagents must only communicate with the QA Lead via the `send_message` tool. They must never directly output messages to the user.
