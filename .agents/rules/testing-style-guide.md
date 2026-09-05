---
trigger: manual
description: Vox Testing, Evaluation, and Benchmark Standards (`app/src-tauri/tests/`, `benches/`, `evals/`, `examples/`).
---

# Vox — Testing, Evaluation & Benchmark Standards

This document contains durable standards for designing, implementing, and running tests, evals, and performance benchmarks in Vox. **Agents authoring or running tests must read this file before acting.**

---

## 1. Hardware Tiers & Feature Mapping

Architecture capabilities and memory ceilings are gated by hardware tier. Tests and benchmarks must validate features within these physical constraints.

| Tier | Hardware | Pipeline Mode | Memory Ingestion | Memory Retrieval | Tool Calling |
| :--- | :------- | :-----------: | :--------------: | :--------------: | :----------: |
| **1A** | 8GB, CPU-only, no GPU | Modular (Local) | ❌ None (FIFO only) | ✅ Working Memory context window only | ❌ Unavailable |
| **1B** ⭐ | 8GB+, dedicated GPU | Modular (Local) | ✅ Full async ingestion | ✅ Full retrieval (episodic + semantic) | ⚠️ Depends on local LLM capability |
| **2A** ⭐ | Hybrid (Remote LLM + Local Audio) | Modular (Remote LLM) | ✅ Full async ingestion | ✅ Full retrieval | ⚠️ Depends on remote LLM capability |
| **2B** ⭐ default | Hybrid (Cloud LLM + Local Audio) | Modular (Cloud LLM) | ✅ Full async ingestion | ✅ Full retrieval | ✅ All cloud models support tool calling |
| **3** | Any (Realtime S2S) | Realtime (WebSocket) | ✅ Provider-managed | ✅ Via early tool calls in provider | ✅ Via early tool calls |

---

## 2. Testing Taxonomy & Scope

| Category | File Location | Command | Access Scope | Primary Output |
| :--- | :--- | :--- | :--- | :--- |
| **Unit Test** | Bottom of target `.rs` file in `#[cfg(test)] mod tests` | `cargo test --lib` | Private + public functions | Pass / Fail |
| **Integration Test** | `app/src-tauri/tests/<feature>_test.rs` | `cargo test --test <name>` | Public `vox_lib` API only | Structural & Lifecycle Correctness |
| **Evaluation (Eval)** | `app/src-tauri/evals/<capability>/` | `cargo run --release --example eval_<capability>` | Crate API + Models + Datasets | Statistical Accuracy + LLM Judge Score |
| **Performance Benchmark** | `app/src-tauri/benches/<feature>_bench.rs` | `cargo test --bench <name> --release` | Custom `fn main()` (`harness=false`) | Real Latency ($T_{\text{E2E}}$) & Throughput |
| **CLI Utility Tool** | `app/src-tauri/examples/<name>.rs` | `cargo run --example <name>` | Runnable dev tools | Standalone Utility CLI |

---

## 3. Testing Principles

A test earns its place by covering behavior that could fail in production in a way that would matter. The test taxonomy in §2 defines where each test lives; these principles define what makes each type worth having.

**Unit tests** earn their place by covering non-trivial algorithmic logic: state machine transitions, parsing edge cases, arithmetic boundaries, sanitization rules, error path behavior. A unit test that only verifies a struct's default values or a `From` implementation with no branching is measuring the compiler, not the code.

**Integration tests** earn their place by exercising real subsystem boundaries through the public `vox_lib` API: what happens when a dependency fails, when a buffer fills under load, when two components race on shared state, when an upstream producer emits an unexpected shape. An integration test that calls a leaf provider function directly, bypassing the actors and channels that connect it to the rest of the system in production, is not an integration test — it is a unit test with larger inputs.

**Performance benchmarks** earn their place by measuring real pipeline latency on real inputs: audio clips through the full STT stack, retrieval queries against a populated index, dispatch round-trips under concurrent load. A benchmark that measures isolated struct serialization in a tight loop produces numbers that do not map to user-observable latency.

---

## 4. Benchmark & Evaluation Execution Standards

The sequential execution and optimized build requirements in `AGENTS.md §2.1` apply to all Vox tests, benchmarks, and evaluations. Vox-specific elaboration:

- Benchmark probes run one model configuration at a time. Concurrent GGUF/ONNX inference causes CPU thread contention that invalidates per-model latency comparisons.
- Evaluation scripts and benchmarks use `cargo run --release` or `cargo test --release`. The `dev` profile omits SIMD vectorization, ONNX graph optimizations, and LTO — producing latency numbers up to 7× slower than production.
- Benchmarks record per-stage latency ($T_{\text{stt}}$, $T_{\text{llm}}$, $T_{\text{tts}}$, $T_{\text{e2e}}$), not only end-to-end. A passing E2E time with a regressed stage is a hidden performance bug.
- Benchmarks accept CLI arguments via `clap` (`--clip`, `--mode`, `--threshold`) so developers and CI can probe realistic workloads without recompiling.

**Ground truth verification standard for model output (STT / LLM / TTS):** Assert normalized string similarity ≥ 0.90 (Levenshtein) or WER ≤ 0.10 against clean, labelled ground truth fixtures. Asserting the presence of 1–2 keywords is not sufficient — it does not distinguish a correct transcript from a partially-correct one, and it will not catch a regression that changes meaning while preserving keywords.

---

## 5. Mandatory Header Format

Every file in `tests/`, `evals/`, `benches/`, and `examples/` must include this standard header:

```rust
//! ============================================================================
//! <filename> — <one-line description>
//! ============================================================================
//! Category     : [Integration Test | Evaluation | Benchmark | Utility Tool]
//! Component    : <target module or subsystem>
//! Prerequisites: <required models, env vars, or services>
//! Execution    : <exact cargo command>
//! Metrics      : <recorded operational/quality metrics>
//! ============================================================================
```

---

## 6. Documentation Standards

Root architecture and feature docs in `docs/*.md` follow a uniform frontmatter + "How to read" convention:

### 6.1 Required Frontmatter (YAML)
```yaml
---
title: "Doc Title"
audience: "Internal — <who this is for>"
last_updated: YYYY-MM-DD
owners: "test-engineer role"
related_docs:
  - "docs/other.md — one-line relationship"
---
```

### 6.2 Required "How to read this doc" Section
Immediately after the title, include:
- **Audience:** who the doc is for.
- **Scope:** what it covers.
- **Convention:** how claims are cited (`path/file.rs` pointers; no invented code blocks).
- **Non-goals:** what it is explicitly NOT (with cross-links).
- **SSOT:** where the authoritative detail lives.

---

## 7. Multi-Threaded, Async & Model Test Invariants (Mandatory)

### 7.1 Hard Timeout Enforcement per Test Function
- **Zero Unbounded Receives or Awaits:** NEVER use unbounded `.recv()` on channels or unbounded `.await` on tasks.
- **Top-Level Timeout Wrapper:** Every `#[tokio::test]` MUST be wrapped in a top-level `tokio::time::timeout(Duration::from_secs(N), ...)` to guarantee that deadlocks, missing events, or infinite loops terminate immediately with a clear panic rather than hanging the test suite indefinitely.
- **Synchronous Test Deadline:** In synchronous `#[test]` functions, bounded channel loops (`recv_timeout`) MUST have an explicit `Instant::now() + Duration::from_secs(N)` overall deadline that panics if exceeded.

### 7.2 Zero Silent Background Panics
- **Thread Handle Joins:** Every background worker thread spawned in test harnesses (`std::thread::spawn`) MUST have its `JoinHandle` saved, gracefully shut down, joined via `.join().expect("Worker thread panicked")`, and asserted for clean execution during test teardown.
- **No False Greens on Crashing Workers:** If a background actor or async task panics or aborts mid-test, the test must catch and report this failure rather than silently passing.

### 7.3 Single Worker Lifecycle per Model Provider (No Re-warming in Same Process)
- **Do NOT Re-warm C++/ONNX/GGUF Backends:** Initializing and tearing down Sherpa-ONNX, llama.cpp, or ONNX Runtime multiple times across isolated `#[test]` functions within the same process leads to singleton state corruption, thread pool races, and SIGSEGV panics.
- **Consolidated Matrix Tests:** Group test scenarios for a given provider (e.g., English speech, Hindi speech, empty guards) into a single session test function that initializes the worker once, runs the test matrix sequentially, and shuts down cleanly.

### 7.4 Correct Audio Resampling & Acoustic Tolerances
- **Resampling Discipline:** Downsampling/upsampling audio (e.g. 24kHz to 16kHz) MUST use valid resampling interpolation (or test against 24kHz golden fixtures directly). Never use naive index stride slicing which corrupts audio duration calculations.
- **Exact Golden Fixtures:** Reference clips and their expected durations must be verified against actual asset properties in `tests/assets/` and `test-clips/`.

---

## 8. Benchmark, Evaluation & Example Artifact Persistence Standards (Mandatory)

Test results, benchmark runs, and evaluation metrics must NEVER be discarded or lost solely to stdout.

### 8.1 Results Directory Layout
All benchmarks, evals, and CLI simulation tools must serialize structured JSON reports into their respective categorized directories:
- **Benchmarks (`benches/`):** `benches/results/<bench_name>/<run_id>/report.json` + `benches/results/<bench_name>/latest.json`
- **Evaluations (`evals/`):** `evals/results/<eval_name>/<run_id>/report.json` + `evals/results/<eval_name>/latest.json`
- **Simulation Examples (`examples/`):** `examples/results/<example_name>/<run_id>/report.json` + `examples/results/<example_name>/latest.json`

### 8.2 Run ID & Schema Rules
- **Unique Timestamped Run IDs:** Run IDs must follow the format `<YYYYMMDD_HHMMSS>_<short_uuid>` (e.g. `20260829_120530_a1b2c3d4`).
- **Standardized Metadata:** Every report artifact must record `run_id`, `timestamp_utc`, `system_info` (OS, CPU cores, physical RAM), per-stage/per-clip latency, RTF, throughput, accuracy similarity, and process RSS memory.
- **Latest Symlink / Mirror:** Each execution MUST overwrite `latest.json` in the tool's base results directory so downstream CI and developer inspection tools can immediately reference the latest run without scanning subdirectories.

### 8.3 Common Harness Decoupling (`benches/common/` & `tests/common/`)
- Shared audio parsing, Levenshtein scoring, streaming ring-buffer feeders, bounded channel drains, and report writers MUST live in decoupled modules (`benches/common/` and `tests/common/`), NEVER duplicated inside standalone benchmark or test files.

---

## 9. Debugging, Subtest Isolation & Blocker Protocol (Mandatory)

### 9.1 Single-Subtest Isolation First
- **Never Run the Entire Suite on Failure:** If any test or subtest fails or hangs, immediately narrow the execution filter to that single, specific test:
  ```bash
  cargo nextest run --test <test_binary> -E 'test(<specific_subtest_fn>)' --release --nocapture --test-threads=1
  ```
- **Zero Multi-Test Looping During Debugging:** Do not re-run all subtests or the full binary until the isolated subtest passes cleanly.

### 9.2 Zero Guessing — Line-by-Line Instrumented Tracing
- **Do Not Speculate or Guess Root Cause:** Never push speculative multi-line changes or run trial-and-error loops hoping an issue disappears.
- **Instrument Every Line:** When a test hangs or fails unpredictably, place explicit synchronous standard error markers (`eprintln!("[DEBUG checkpoint X]")`) before and after every single critical operation, mutex lock, channel send, thread spawn/join, and async boundary.
- **Isolate the Exact Line Number:** Determine precisely which line fails to complete within 1 iteration. Remove the debug markers once the root cause is confirmed and resolved.

### 9.3 The 2-Attempt Rule & Blocker Escalation
- **Strict 2-Attempt Limit:** If a test failure or hang persists after **2 targeted attempts** with no new confirmed hypothesis, **STOP IMMEDIATELY**.
- **Report, Never Work Around:**
  1. Halt execution immediately without making further trial edits.
  2. Report:
     - Exact test and line number where the failure occurs.
     - Observed symptoms and logs from instrumented checkpoints.
     - The two hypotheses already attempted and their results.
     - The specific dependency, deadlocking primitive, or architectural blocker identified.
     - What is required to unblock progress.
  3. Never silently bypass, stub with mock data, remove assertions, or delete tests to fake a green result.


