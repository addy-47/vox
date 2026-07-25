# AGENTS.md — Vox v6 Cognitive Memory Subsystem & Gate 1 Benchmarking Framework

## 1. Project Overview & Current State

Vox is a voice-native, agent-first AI platform built on a real-time, event-driven native pipeline. The memory system is governed by the **v6 Hybrid Cognitive Memory Subsystem Specification** (`docs/plans/v6-memory-architecture-spec.md`).

### Current Phase Focus: Gate 1 Local Model Benchmarking & Edge Creation Verification
* **Primary Target**: Validate local inference engines (`deberta-v3-xsmall` ONNX for Class A NLI, `LFM2.5-230M` GGUF for Class B LLM inter-collection edge creation) on real-world synthetic data simulating real-time code behavior as closely as possible.
* **Testing Scope**:
  1. Class A Intra-Collection NLI (`Constraints`, `Tasks`, `Goals`) using `deberta-v3-xsmall` (Candidate filter `same_collection_candidate_search = 0.65`, threshold `0.85`).
  2. Class B Inter-Collection LLM Edge Generation (`Skills`, `Preferences`, `Projects`, `Experiences`, `Relationships`) using `LFM2.5-230M` GGUF (Candidate filter `inter_collection_candidate_search = 0.75`).
  3. Class C (`Identity`, `Context`) & Class B Intra-Collection Strict Isolation Verification (Zero NLI/LLM passes).
  4. Automatic Deterministic Inverse Edge Mapping in SQLite runtime.

---

## 2. Agent Roles & Operational Rules

### MANDATORY SUBAGENT REUSE RULE
* **CRITICAL**: System Architect MUST re-use the three defined subagents (`ai_test_engineer`, `qa_reviewer`, `backend_engineer`) throughout the lifecycle.
* **DO NOT** spawn duplicate or new subagents for every turn. Use `send_message` with the existing conversation IDs so subagents maintain context!

### 2.1 System Architect (Parent / Lead Agent)
* **Role**: Strategy, intent alignment, system architecture, loop orchestration, gate governance, and report synthesis.
* **Constraints**:
  * **MUST NEVER** make direct source code edits in `app/src-tauri/src/` without approved plan.
  * **MUST NEVER** self-approve test results or benchmark metrics.
  * **MUST ALWAYS** delegate test execution to `ai_test_engineer`, audits to `qa_reviewer`, and backend code implementation to `backend_engineer`.

### 2.2 AI Test Engineer (`ai_test_engineer`)
* **Role**: Synthetic dataset generation (using NVIDIA API key `meta/llama-3.1-70b-instruct`), test script harness engineering, and executing local model benchmarks.
* **Constraints**:
  * Must utilize NVIDIA API strictly for dataset generation and baseline comparison.
  * Must run local benchmarks using ONNX Runtime for DeBERTa-v3-xsmall and llama.cpp/GGUF runtime for LFM2.5-230M.
  * Must capture real-time system metrics: execution latency per pair (ms), RAM allocation, CPU/GPU utilization, throughput, and error outputs.

### 2.3 QA Reviewer (`qa_reviewer`)
* **Role**: Independent audit of benchmark results, latency distributions, semantic edge precision, false positive rates, and spec compliance against `docs/plans/v6-memory-architecture-spec.md`.
* **Constraints**:
  * **MUST NEVER** accept `exit code 0` as proof of accuracy or performance.
  * **MUST NEVER** approve vanity counts (e.g. "20 edges created" without checking if edges match connection matrix).
  * Must audit Class A conflict auto-resolution (`Tasks`/`Goals` $\rightarrow$ `SUPERSEDES`, `Constraints` $\rightarrow$ `CONFLICTS`), Class B inter-collection relation validity, and inverse deterministic edge mapping.

### 2.4 Senior Backend Engineer (`backend_engineer`)
* **Role**: Implementation of Rust backend memory services in `app/src-tauri/src/services/memory/`.
* **Constraints**:
  * Must strictly adhere to `.agents/rules/backend-engineer.md`.
  * Must verify code using `cargo check` and `cargo clippy`. No unreviewed warnings.

---

## 3. Evaluation & Gate 1 Passing Criteria

### ❌ BANS (Never Accept As Gate Pass)
1. **Exit Code 0**: A Python or Rust harness finishing without an exception is NOT proof of model correctness or latency compliance.
2. **Mock Data Fallbacks**: Benchmarking must run on actual local weights (`LFM2.5-230M` GGUF and `deberta-v3-xsmall-nli`).
3. **Spec Violations**: Any LLM edge generated between forbidden collections (e.g., Class C or Class B intra-collection) is an instant Gate 1 failure.

### ✅ MANDATORY GATE 1 METRICS
* **Class A NLI Latency & Accuracy**:
  * Intra-pair inference latency: $< 20\text{ ms}$ on CPU/GPU.
  * Contradiction / Entailment classification accuracy: $\ge 90\%$.
  * Conflict resolution compliance: 100% (Tasks/Goals auto-resolve to `SUPERSEDES`; Constraints preserved as `CONFLICTS`).
* **Class B Inter-Collection LLM Latency & Precision**:
  * Inter-pair inference latency: $< 100\text{ ms}$ (LFM2.5-230M).
  * Edge generation precision: $\ge 85\%$ against gold reference.
  * Connection Policy Matrix compliance: 100% allowed pairs only.
  * Quantization Comparison: Benchmark `Q8_0` vs `Q4_K_M` / `Q4_0`.
* **Deterministic Inverse Mapping**: 100% forward edges auto-trigger runtime inverse edge creation.
* **Isolation Verification**: 0% false-positive NLI/LLM calls for Class C (`Identity`, `Context`) or Class B intra-collection.

---

## 4. Inference Provider & Model Governance

* **Synthetic Data Generation**: NVIDIA API (`meta/llama-3.1-70b-instruct`) using `NVIDIA_API_KEY` in `temp/.env`.
* **Local NLI Model**: `~/.vox/models/nli/deberta-v3-xsmall-nli/model_quantized.onnx`.
* **Local Class B LLM Model**: `~/.vox/models/llm/LFM2.5-230M-Q8_0.gguf` (and `Q4_K_M` / `Q4_0` variants).
* **Embedding Model**: `~/.vox/models/embedding/bge-m3/model_quantized.onnx` (1024-dim BGE-M3).

---

## 5. Key Specifications & Sources of Truth

* `docs/plans/v6-memory-architecture-spec.md` — Vox v6 Frozen Architecture Specification.
* `docs/features/memory-architecture.md` — Implemented Subsystem Ledger.
* `sandbox/v6_harness/` — Gate 1 Benchmarking Harnesses & Synthetic Datasets.
