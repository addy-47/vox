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
