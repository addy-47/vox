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

### 2.1 Standard Rust File Grammar Order (CRITICAL)

All Rust source files must strictly follow this top-to-bottom grammar ordering:
1. **Module Documentation & Imports:**
   - Crate/file doc comments (`//! ...`).
   - Grouped imports: `std::...`, external third-party crates, internal `crate::...`, `super::...`.
2. **File-Local Constants & Type Aliases:**
   - `const ...`, `pub type ...`.
3. **Data Structures (Structs & Enums):**
   - Public and internal `struct` and `enum` declarations with `#[derive(...)]`.
4. **Trait Implementations:**
   - Standard and custom trait impls (`impl Trait for Struct { ... }`).
5. **Main Inherent Implementations:**
   - `impl Struct { pub fn ... fn ... }` (constructors first, public methods, private methods).
6. **Helper Functions & Private Utilities:**
   - Free functions (`fn ...`).

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

## 7. State, Event, and Flag Discipline (CRITICAL)

### 7.1 Single Source of Truth (The Law of State)
- `InteractionState` (`Idle=0, Ready=1, Listening=2, Thinking=3, Speaking=4, Paused=5, Error=6`) is the **SOLE SOURCE OF TRUTH** for the assistant pipeline lifecycle.
- `DictationState` (`Idle=0, Recording=1, Transcribing=2, Error=3`) is the **SOLE SOURCE OF TRUTH** for dictation.

### 7.2 When Using a `bool` is Justified vs. Banned
- ❌ **STRICTLY BANNED (Synthetic Booleans & State Flag Bags):**
  - **Derived Lifecycle Flags:** Never create boolean atomics, struct fields, or query methods that duplicate, shadow, or approximate lifecycle state (e.g. `is_connected`, `is_idle`, `is_engaged`, `is_sleeping`, `is_paused`, `is_assistant`, `is_passive`, `is_private`, `is_recording`, `is_speech_detected`). Query the state enum (`state.pipeline.state() == InteractionState::...`) and settings directly.
  - **Model Readiness Bags:** Never model model availability or subsystem readiness as a flat bag of loose atomics (e.g. `is_stt_loaded`, `is_llm_loaded`, `is_tts_loaded`). Subsystem/engine availability must be derived from `Option<Engine>` / `Arc<RwLock<Option<...>>>` or explicit status enums.
  - **Ghost Flags:** Booleans that are written to but never read, or read without coordinated mutex guards leading to race conditions.
- ✅ **JUSTIFIED / PERMITTED:**
  - **Pure Binary Hardware / Signal Status:** A true, independent binary condition that is not a pipeline lifecycle phase (e.g. `mic_muted: bool`, `noise_gate_active: bool`, `VadBackend::is_above_noise_gate(&self) -> bool`).
  - **Static / Persistent Feature Configuration Flags:** Immutable or user-configured binary settings (e.g. `enable_vad: bool`, `echo_cancellation: bool`, `save_transcripts: bool`).
  - **Transient Flow Control within Single Function Scope:** A local variable tracking immediate iteration state (e.g. `let has_speech = ...;` or `let mut seen_first_token = false;`).
  - **Atomic Cancellation / Shutdown Tokens:** `tokio_util::sync::CancellationToken` or worker shutdown flags (`AtomicBool` for loop termination only).

### 7.3 State Transitions are the Sole Lifecycle Event Pump
- `transition(...)` broadcasts `IpcEvent::StateChanged` (`"state_changed"`).
- Submodules must **NEVER** manually emit ad-hoc custom lifecycle events (`speech_start`, `speech_end`, `playback_started`, `playback_finished`, `session_started`, `session_ended`, `ptt_status`).
- All cross-boundary IPC emissions must be dispatched strictly through `emit_ipc` or `emit_ipc_to` using canonical `IpcEvent` enum variants.

### 7.4 Centralized Monotonic Turn Generation
- Turn IDs must be monotonically allocated strictly at the turn boundary via `AppState::next_turn_id()`. Never fragment `fetch_add` across actors, reset to `0`, or pass dummy turn IDs.

### 7.5 Event Contracts
Events are registry-owned and must have exactly one canonical definition. Internal pipeline events belong in `core/events.rs` (`VoxEvent`); IPC events belong in the typed IPC event registry (`IpcEvent`); telemetry and other subsystem buses use their own dedicated enums (`TelemetryEvent`, `PersistenceEvent`, `MemoryWorkerEvent`). Never introduce raw event-name strings, ad-hoc event variants, or undocumented payloads at call sites. Every new event requires a registry entry, a strongly typed payload, an explicit producer, an explicit consumer or documented reason for being producer-only, and corresponding contract tests. If an event is not present in the canonical registry, it does not exist. Do not duplicate or rename an existing event to represent the same state; extend the existing contract instead. Commands are not events and must remain in their owning actor/service command enum.

---

## 8. Production Rust Best Practices

- **Structured Logging:** All logs must specify domain tags: `log::info!("[Domain::Subsystem] Action completed status=ok")`. Never use `println!` or `eprintln!` in `src/`.
- **Dropped Counter Telemetry:** High-throughput channel `try_send` calls must increment an atomic dropped-counter handle and log warnings if backpressure occurs.
- **Newtype Pattern:** Prefer lightweight typed wrappers or domain aliases over raw primitives for identifiers (e.g. `TurnId(u32)`).
- **Exhaustive Enums for State:** Model lifecycles using explicit state enums with transition functions rather than coordinating bags of loose booleans.

---

## 9. Documentation Standards

Root architecture and feature docs in `docs/*.md` follow a uniform frontmatter + "How to read" convention:

### 9.1 Required Frontmatter (YAML)
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

### 9.2 Required "How to read this doc" Section
Immediately after the title, include:
- **Audience:** who the doc is for.
- **Scope:** what it covers.
- **Convention:** how claims are cited (`path/file.rs` pointers; no invented code blocks).
- **Non-goals:** what it is explicitly NOT (with cross-links).
- **SSOT:** where the authoritative detail lives.

---

## 10. Testability Seams, Inversion of Control & Runtime Generics (MANDATORY)

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