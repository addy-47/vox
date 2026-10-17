---
trigger: manual
description: Testing taxonomy, evaluation benchmarks, async timeout invariants, and failure blocker protocols for Vox (`app/src-tauri/tests/`, `benches/`, `evals/`).
---

# Vox — Testing, Evaluation & Benchmark Standards

Durable standards for designing, implementing, and running tests, evals, and performance benchmarks in Vox. Read before authoring or modifying test files.

---

## 1. Testing Taxonomy & Execution Scope

| Category | File Location | Command | Access Scope | Primary Output |
| :--- | :--- | :--- | :--- | :--- |
| **Unit Test** | Target `.rs` in `#[cfg(test)] mod tests` | `cargo test --lib` | Private + public functions | Pass / Fail |
| **Integration Test** | `app/src-tauri/tests/<feature>_test.rs` | `cargo test --test <name>` | Public `vox_lib` API only | Structural Correctness |
| **Evaluation (Eval)** | `app/src-tauri/evals/<capability>/` | `cargo run --release --example eval_<name>` | Crate API + Models + Datasets | Accuracy & Quality Metrics |
| **Benchmark** | `app/src-tauri/benches/<feature>_bench.rs` | `cargo test --bench <name> --release` | Custom `main()` (`harness=false`) | Latency ($T_{\text{E2E}}$) & Throughput |
| **CLI Tool** | `app/src-tauri/examples/<name>.rs` | `cargo run --example <name>` | Runnable dev tools | Standalone Utility Output |

---

## 2. Core Testing Principles

- **Unit tests:** Exercise non-trivial algorithms, state machine transitions, arithmetic boundaries, and sanitization logic. Avoid tests that merely assert struct field assignment.
- **Integration tests:** Exercise authentic subsystem boundaries through `vox_lib`. An integration test must route events through actual actor channels rather than calling leaf functions directly.
- **Performance benchmarks:** Measure realistic pipelines on real audio clips or indexed datasets. Measure per-stage latency ($T_{\text{stt}}$, $T_{\text{llm}}$, $T_{\text{tts}}$) alongside total time ($T_{\text{e2e}}$).

---

## 3. Execution & Verification Standards

- **Sequential execution:** Run benchmark probes and tests one configuration at a time to prevent CPU/thread contention.
- **Mandatory release mode:** Always execute evaluations and benchmarks via `--release`. The debug profile produces latency figures up to 7× slower than production.
- **Ground truth verification:** Assert normalized Levenshtein similarity $\ge$ 0.90 or WER $\le$ 0.10 against labelled golden fixtures. Never assert single keyword presence.

---

## 4. Mandatory Test Header

Include this standard header at the top of every file in `tests/`, `evals/`, and `benches/`:

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

## 5. Multi-Threaded & Async Test Invariants

### 5.1 Hard Timeout Enforcement
- **Bounded async operations:** Every `#[tokio::test]` must be wrapped in a top-level `tokio::time::timeout(Duration::from_secs(N), ...)` to terminate deadlocks immediately.
- **Synchronous test deadlines:** Channel loops in synchronous `#[test]` functions must use bounded receives (`recv_timeout`) or check against an explicit `Instant` deadline.

### 5.2 Background Worker Teardown
- **Thread handle joins:** Save every background worker `JoinHandle` spawned during tests. Join all handles on teardown via `.join().expect("Worker thread panicked")`.
- **Catch worker panics:** Tests must actively verify that background threads completed cleanly without aborting.

### 5.3 Model Provider Lifecycle
- **Avoid re-warming native backends:** Initializing and tearing down Sherpa-ONNX, llama.cpp, or ONNX Runtime repeatedly within the same process causes memory corruption and segfaults. Group scenarios into a consolidated test that initializes the worker once.

### 5.4 Audio Resampling
- Resampling audio between sample rates (e.g. 24kHz to 16kHz) must use valid interpolation or test against native sample rate golden fixtures. Never use stride slicing.

---

## 6. Artifact Persistence Standards

- **Results directory layout:** Serialize structured JSON reports:
  - Benchmarks: `benches/results/<bench_name>/<run_id>/report.json` + `benches/results/<bench_name>/latest.json`
  - Evaluations: `evals/results/<eval_name>/<run_id>/report.json` + `evals/results/<eval_name>/latest.json`
- **Run ID format:** Use timestamped IDs: `<YYYYMMDD_HHMMSS>_<short_uuid>`.
- **Mirror latest:** Overwrite `latest.json` upon completion to allow immediate inspection without scanning directories.
- **Decoupled harnesses:** Common test audio parsers, scorers, and mock buffers live in `tests/common/` or `benches/common/`.

---

## 7. Debugging & Blocker Protocol

### 7.1 Single-Subtest Isolation
When any test fails, immediately isolate execution to that specific test function:
```bash
cargo nextest run --test <binary> -E 'test(<subtest_fn>)' --release --nocapture --test-threads=1
```
Never loop across the entire test suite while debugging a local failure.

### 7.2 Instrumented Tracing
When investigating a hang or race condition, place explicit standard error markers (`eprintln!("[DEBUG checkpoint X]")`) before and after mutex locks, channel operations, and async boundaries to pinpoint the exact failing line.

### 7.3 Strict 2-Attempt Rule
If a test failure persists after **2 targeted attempts** without a confirmed new hypothesis, stop immediately and report:
1. Exact test name and failing line number.
2. Observed logs and checkpoint outputs.
3. The two hypotheses attempted and their results.
4. The specific blocker, missing dependency, or architectural deadlock.
Never mock, stub, or remove assertions to manufacture a passing result.