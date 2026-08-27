---
trigger: manual
description: Vox Backend Code Style Guide and Engineering Standards for Rust (`app/src-tauri/src/`).
---

# Vox — Backend Code Style Guide & Engineering Standards

This document contains durable coding standards for the Vox native Rust backend (`app/src-tauri/src/`). **Agents doing write operations on Rust backend files must read this file before modifying code.**

---

## 1. Hardware Tiers & Feature Mapping

Architecture capabilities are gated by hardware tier. Vox must dynamically degrade or upgrade based on what the user's system supports. **Tier 2 is the recommended baseline.**

| Tier | Hardware | Pipeline Mode | Memory Ingestion | Memory Retrieval | Tool Calling |
| :--- | :------- | :-----------: | :--------------: | :--------------: | :----------: |
| **1A** | 8GB, CPU-only, no GPU | Modular (Local) | ❌ None (FIFO only) | ✅ Working Memory context window only | ❌ Unavailable |
| **1B** ⭐ | 8GB+, dedicated GPU | Modular (Local) | ✅ Full async ingestion | ✅ Full retrieval (episodic + semantic) | ⚠️ Depends on local LLM capability |
| **2A** ⭐ | Hybrid (Remote LLM + Local Audio) | Modular (Remote LLM) | ✅ Full async ingestion | ✅ Full retrieval | ⚠️ Depends on remote LLM capability |
| **2B** ⭐ default | Hybrid (Cloud LLM + Local Audio) | Modular (Cloud LLM) | ✅ Full async ingestion | ✅ Full retrieval | ✅ All cloud models support tool calling |
| **3** | Any (Realtime S2S) | Realtime (WebSocket) | ✅ Provider-managed | ✅ Via early tool calls in provider | ✅ Via early tool calls |

---

## 2. Module Organization & File Boundaries

- **Domain over type:** Group code by domain (`services/memory/nli.rs`), never by Rust construct (`models.rs`).
- **Single responsibility:** 1 responsibility per file. If a file cannot be described in 1 sentence, split it.
- **File size ceiling:** Flag and justify files exceeding ~600 lines.
- **`mod.rs` & `lib.rs`:** `mod.rs` is for module declarations, re-exports, and **subsystem-level constants**. Zero business logic. `lib.rs` is for module declarations + Tauri app setup only. Zero business logic.
- **Visibility:** Use `pub(crate)` over `pub` unless crossing the crate boundary (IPC handlers or integration tests).

---

## 3. Constant Hierarchy & Placement (CRITICAL)

Never scatter or bury magic numbers or configuration values across internal actor loops. All constants follow a strict 4-level hierarchy:

1. **Global Constants (`app/src-tauri/src/core/constants.rs`):**
   - App-wide constants shared across multiple subsystems (e.g., `SAMPLE_RATE`, `RING_BUFFER_SIZE`, `DB_FILENAME`, system event strings, global prompt templates).
2. **Settings Defaults (`app/src-tauri/src/core/defaults.rs`):**
   - Default values for user-configurable settings and catalog options (e.g., default STT provider, default TTS voice, default LLM temperature).
3. **Subsystem / Domain Constants (`app/src-tauri/src/services/<subsystem>/mod.rs` or domain `mod.rs`):**
   - Domain-specific thresholds, frame limits, buffer sizes, model filenames, and directory paths must be placed at the **top of that domain's `mod.rs`** (e.g. `MODEL_DIR_VAD`, `VAD_CHUNK_SIZE`, `INACTIVE_FRAMES_THRESHOLD` in `services/vad/mod.rs`; `CTX_FLOOR_NON_EMBEDDED`, `DEFAULT_CLOUD_MODEL_CTX` in `services/llm/mod.rs`).
   - Anyone inspecting a subsystem must immediately find its tuning parameters in `mod.rs` without searching through 10 internal worker files.
4. **Single-File Internal Constants (Top of `.rs` file):**
   - Constants purely local to a single struct or algorithm implementation (not shared across sibling modules) live at the very top of that specific file.

---

## 4. Function Standards & Code Cleanliness

- **Function line cap (soft):** No function exceeds 50 lines without documented justification.
- **Docstrings:** Exactly one `///` doc comment per function that states what it does, what it takes, and what it returns. Zero per-line comments inside function bodies. Runtime traces belong in `log::info!` / `log::warn!`.
- **No step-comment sequences:** If a function body needs numbered step comments (`// 1. do X`, `// 2. do Y`), each step must become a named private helper function.
- **No toggle functions:** A function named `engage()` must only engage. `if condition { engage } else { disengage }` in one function body is banned. Use discrete named functions.
- **Struct bundling for parameter lists (>5 arguments):** Any function or constructor taking more than 5 arguments must group related parameters into a dedicated typed config or handles struct (e.g. `VadActorConfig`, `VadActorHandles`, `PlaybackTelemetryHandles`).
- **Zero `#[allow(...)]` policy:** `#[allow(clippy::too_many_arguments)]`, `#[allow(dead_code)]`, `#[allow(unused_variables)]`, and all other lint suppressions are strictly banned.
- **Zero `_` prefixed masking:** Never prefix unused variables or fields with `_` to silence warnings. If an item is not needed, delete it.
  - *RAII drop guard exception:* `_` is strictly reserved for genuine RAII drop guards (`_stream: Option<cpal::Stream>`, `_log_guard: Option<WorkerGuard>`, `_thread_handle`) where holding the handle in memory is required to keep hardware streams or workers alive.

---

## 5. Error Handling & Resilience

- **No `unwrap()` in `src/`:** Banned except on poisoned `RwLock`/`Mutex` guards.
- **Propagation:** Use `?` with `.context("...")` (`anyhow`) in services and persistence.
- **IPC boundary:** Errors returned across Tauri IPC must be typed enums using `thiserror`.
- **No silent error swallowing:** `let _ = result` is banned. Every channel send or fallible call must either propagate with `?` or log warnings on error:
  ```rust
  if let Err(e) = tx.send(item) {
      log::warn!("[Domain::Subsystem] Channel send failed: {}", e);
  }
  ```
- **No fallback chains:** Avoid `if path A fails, try path B, try path C`. One deterministic path per operation. If the path fails, report the error.

---

## 6. Concurrency, Threading & Audio Hot Path

- **Actor-Engine Separation:** The actor owns the OS thread and state machine. The engine owns inference logic. They never merge into one struct or file.
- **Thread Placement:**
  - Inference (VAD/STT/LLM/TTS model execution) runs on dedicated OS threads, never on Tokio workers.
  - Tauri IPC and WebSocket I/O run on Tokio tasks, never on blocking OS threads.
  - Dedicated OS threads must use elevated thread priority (`thread_priority::ThreadPriority::Max`) where timing is critical.
- **Audio Hot Path is Sacred:** VAD → STT → LLM → TTS hot path must be zero allocations and zero lock acquisitions. Hot-path workers use snapshotted values.
- **Channels over Shared Mutexes:** Cross-thread communication uses Tokio/crossbeam channels or atomics. Avoid adding new `Arc<Mutex<T>>`.
- **Canonical Mutex Lock Order:** Strictly acquire `state.engine` before `state.realtime_engine`. Never reversed. Lock order inversion is a confirmed deadlock source.
- **No Polling where Events Suffice:** Subsystems must emit `VoxEvent` when state changes rather than having callers poll atomics on a timer.

---

## 7. Production Rust Best Practices

- **Structured Logging:** All logs must specify domain tags: `log::info!("[Domain::Subsystem] Action completed status=ok")`. Never use `println!` or `eprintln!` in `src/`.
- **Dropped Counter Telemetry:** High-throughput channel `try_send` calls must increment an atomic dropped-counter handle and log warnings if backpressure occurs.
- **Newtype Pattern:** Prefer lightweight typed wrappers or domain aliases over raw primitives for identifiers (e.g. `TurnId(u32)`).
- **Exhaustive Enums for State:** Model lifecycles using explicit state enums with transition functions rather than coordinating bags of loose booleans.

---

## 8. Documentation Standards

Root architecture and feature docs in `docs/*.md` follow a uniform frontmatter + "How to read" convention:

### 8.1 Required Frontmatter (YAML)
```yaml
---
title: "Doc Title"
audience: "Internal — <who this is for>"
last_updated: YYYY-MM-DD
owners: "backend-engineer role"
related_docs:
  - "docs/other.md — one-line relationship"
---
```

### 8.2 Required "How to read this doc" Section
Immediately after the title, include:
- **Audience:** who the doc is for.
- **Scope:** what it covers.
- **Convention:** how claims are cited (`path/file.rs` pointers; no invented code blocks).
- **Non-goals:** what it is explicitly NOT (with cross-links).
- **SSOT:** where the authoritative detail lives.

---

## 9. Testability Seams, Inversion of Control & Runtime Generics (MANDATORY)

Every backend actor, worker, pipeline domain, and router must be designed with explicit consideration of how it will be instantiated and tested in isolated unit and integration test harnesses:

1. **Generic Tauri Runtime (`AppHandle<R: tauri::Runtime>`)**:
   - Never bind actor functions, worker threads, domain routers, or lifecycle helpers to the concrete default Tauri runtime (`AppHandle` which defaults to `Wry`).
   - Always parameterize with `<R: tauri::Runtime>` (or `R: tauri::Runtime + 'static` for spawned threads):
     ```rust
     pub fn spawn_actor<R: tauri::Runtime + 'static>(app: AppHandle<R>, ...) -> Result<JoinHandle<()>, String>
     ```
   - This enables integration test suites to pass `tauri::test::mock_app().handle()` without requiring live OS webview windows or X11/Wayland event loops.

2. **Decoupled Ingestion & Dispatch Seams (No Isolated Module Statics)**:
   - Module-level statics (`static PTT_BUFFER: Mutex<...>`, `static IS_RECORDING: AtomicBool`) must never form isolated black boxes that upstream actors cannot feed or tests cannot observe.
   - Expose explicit ingress/egress seam functions (e.g. `ingest_audio(&[f32])`, `is_recording() -> bool`, `handle_ptt_stop_with_sender(...)`) so that upstream workers (like the VAD actor) can feed audio buffers and tests can drive turns without booting full audio hardware.

3. **Inversion of Control for Hardware Dependencies**:
   - High-level orchestrators that dispatch commands to downstream channels (`stt_tx`, `llm_tx`, `tts_tx`, `realtime_engine`) must support optional sender overrides or fallback gracefully when executing in headless test environments where hardware audio drivers (CPAL) are absent.

