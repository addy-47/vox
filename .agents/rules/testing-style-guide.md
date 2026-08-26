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

## 3. Testing Principles (Zero Noise Policy)

1. **Never test trivial language invariants or compiler guarantees:**
   - **Banned:** Tests that solely construct a struct with default values and assert `field == expected`.
   - **Banned:** Tests that serialize/deserialize an enum and assert string equality (serde derive handles this).
   - **Banned:** Tests that assert enum discriminants or trivial `From` implementations with zero business logic.
   - **Banned:** Instantiating an ad-hoc local `Mutex` or fake struct in a test and claiming it tests a subsystem cache.
2. **Unit Tests (`#[cfg(test)] mod tests`)**:
   - Must test non-trivial algorithmic logic, state machine transitions, text sanitization, parsing, math, or error edge cases.
3. **Integration Tests (`app/src-tauri/tests/<feature>_test.rs`)**:
   - Must test subsystem interaction, lifecycle contracts, concurrency, and error recovery using public `vox_lib` APIs.
   - Must test real failure modes: what happens when a dependency fails, when state races occur, or when buffers overflow.
4. **Performance Benchmarks (`app/src-tauri/benches/<feature>_bench.rs`)**:
   - **Banned:** Micro-benchmarks measuring simple struct serde or isolated mutex locking in a tight loop.
   - Must execute real pipelines: ingest real inputs (e.g. WAV audio, text corpora), invoke actual ML inference or service dispatch, and record per-stage and end-to-end latency ($T_{\text{stt}}$, $T_{\text{dispatch}}$, $T_{\text{e2e}}$) and throughput.
   - Must support CLI arguments via `clap` (e.g. `--clip`, `--mode`) so developers and CI can test realistic workloads.

---

## 4. Benchmark & Latency Execution Rules (MANDATORY)

1. **NEVER RUN BENCHMARK PROBES IN PARALLEL:**
   - Running multiple GGUF or ONNX inference commands concurrently causes CPU thread contention and invalidates per-pair latency metrics.
   - Always execute benchmark probes **strictly sequentially, one model at a time**.
2. **NEVER RUN BENCHMARKS OR EVALUATION SCRIPTS IN DEBUG MODE:**
   - Debug builds (`dev` profile without `--release`) omit SIMD vectorization, ONNX graph optimizations, and LTO, producing invalid latency metrics (up to 7x slower).
   - Always execute evaluation scripts and benchmarks using `--release` mode.

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
