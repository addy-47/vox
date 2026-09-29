---
trigger: manual
description: Vox Backend Code Style Guide and Engineering Standards for Rust (`app/src-tauri/src/`).
---

# Vox — Backend Code Style Guide & Engineering Standards

This document contains durable coding standards for the Vox native Rust backend (`app/src-tauri/src/`). **Agents doing write operations on Rust backend files must read this file before modifying code.**

---

## 1. Hardware Tiers & Feature Mapping

Architecture capabilities are gated by hardware tier. Vox dynamically degrades or upgrades based on what the user's system supports. **Tier 2 is the recommended baseline.**

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
- **`mod.rs` & `lib.rs`:** `mod.rs` is for module declarations, re-exports, and **subsystem-level shared constants**. Zero business logic. `lib.rs` is for module declarations + Tauri app setup only. Zero business logic.
- **Visibility:** Use `pub(crate)` over `pub` unless crossing the crate boundary. Use `pub` only for Tauri IPC command handlers and types that must be accessible from the integration test crate (`tests/`).

### 2.1 Standard Rust File Grammar Order (CRITICAL)

All Rust source files must strictly follow this top-to-bottom grammar ordering:
1. **Imports:**
   - Grouped imports: `std::...`, external third-party crates, internal `crate::...`, `super::...`.
   - Do not include crate/file header doc comment blocks (`//! ...`). Comments should strictly be concise doc comments directly on top of functions, types, and traits (`/// ...`).
2. **File-Local Constants & Type Aliases:**
   - `const ...`, `pub(crate) type ...`.
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

Never scatter or bury magic numbers or configuration values across internal actor loops. All constants follow a strict 3-tier hierarchy:

1. **User-Facing Settings Defaults ([`app/src-tauri/src/core/defaults.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/defaults.rs)):**
   - Default values for user-configurable settings, options catalog, fallback timeouts, and model parameters (e.g., `DEFAULT_UI_THEME`, `DEFAULT_VAD_THRESHOLD`, `DEFAULT_ASR_MODEL`, `DEFAULT_LLM_TEMPERATURE`, `DEFAULT_TTS_VOICE_INDEX`).
2. **Subsystem / Domain Shared Constants (`app/src-tauri/src/services/<domain>/mod.rs` or domain `mod.rs`):**
   - Shared thresholds, buffer sizes, sample rates, frame limits, model filenames, and directory paths used across multiple files within that subsystem (e.g., `TTS_SAMPLE_RATE`, `TTS_CHUNK_SIZE`, `ZIPVOICE_MODEL_DIR` in `services/tts/mod.rs`; `CTX_FLOOR_NON_EMBEDDED`, `DEFAULT_CLOUD_MODEL_CTX` in `services/llm/mod.rs`).
   - Anyone inspecting a subsystem must immediately find its tuning parameters in `mod.rs` without searching through internal worker files.
3. **File-Local Internal Constants (Top of `.rs` file):**
   - Constants used strictly by a particular file and only that file live directly at the top of that specific file beneath imports, following the standard grammar order.

---

## 4. Spec-First Invariant Grounding (Authoritative Source of Truth)

Do **not** invent or duplicate subsystem invariants in general style guides or actor implementations. Each domain in Vox is governed by an authoritative specification in `docs/specs/`. When designing, implementing, or refactoring code, always consult and align with the corresponding spec:

- **Pipeline Events, State Machines & Turn Lifecycle:** [events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)
  - Canonical `VoxEvent` registry, `InteractionState` and `DictationState` state machines, event-driven state transitions (IPC commands never mutate state directly), monotonic turn counter (`PipelineAtomics::next_turn()`), and 6-domain event contracts.
- **Tiered Storage, Paths & Configuration Persistence:** [storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md)
  - 6-tier POSIX layout (`config/`, `data/`, `models/`, `cache/`, `diagnostics/`, `run/`), 3-way configuration decomposition (`settings.jsonc`, `providers.jsonc` [0600], `agent.jsonc`), paths singletons, and atomic writes with fallback recovery.
- **Dictation & OS Keystroke Output:** [dictation-spec.md](file:///home/addy/projects/apps/vox/docs/specs/dictation-spec.md)
  - Dictation lifecycle, silence auto-stop, pre-roll audio buffering, AT-SPI/Wayland typing injection, and shortcut bindings.
- **LLM Harness & Streaming Inference:** [harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)
  - Execution contexts, token limits, streaming protocol, prompt assembly, memory injection, and tool-call contracts.
- **Tauri IPC Command Contracts & Error Enums:** [ipc-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md)
  - Tauri command signatures, typed `thiserror` boundaries, payload validation, and frontend IPC bridge invariants.
- **Database Architecture & Queries:** [db-spec.md](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)
  - Turso SQLite schemas, migrations, connection pooling, vector embeddings, and query constraints.
- **Working & Personal Memory Systems:** [memory-spec.md](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)
  - Dual-tier memory architecture, auto-compaction triggers, semantic similarity cutoffs, and consolidation pipelines.
- **Resource Ownership & Lifetimes:** [ownership-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md)
  - Actor ownership hierarchies, worker threads, `CancellationToken` hierarchies, drop guards, and graceful teardown semantics.
- **Provider & Model Catalog:** [provider-model-catalog-spec.md](file:///home/addy/projects/apps/vox/docs/specs/provider-model-catalog-spec.md)
  - Provider capabilities (`ProviderCaps`), voice profiles, model manifests, dynamic resolution, and fallback routing.
- **Notifications & Diagnostics:** [notifications-spec.md](file:///home/addy/projects/apps/vox/docs/specs/notifications-spec.md)
  - Desktop notifications, system tray states, toast centralization, and telemetry reporting.

---

## 5. State, Flag & Boolean Discipline

- ❌ **STRICTLY BANNED (Synthetic Booleans & State Flag Bags):**
  - **Derived Lifecycle Flags:** Never create boolean atomics, struct fields, or query methods that duplicate, shadow, or approximate lifecycle state (e.g. `is_connected`, `is_idle`, `is_engaged`, `is_sleeping`, `is_paused`, `is_assistant`, `is_passive`, `is_private`, `is_recording`, `is_speech_detected`). Query the state enum and settings directly.
  - **Model Readiness Bags:** Never model model availability or subsystem readiness as a flat bag of loose atomics (e.g. `is_stt_loaded`, `is_llm_loaded`, `is_tts_loaded`). Subsystem/engine availability must be derived from `Option<Engine>` / `Arc<RwLock<Option<...>>>` or explicit status enums.
  - **Ghost Flags:** Booleans that are written to but never read, or read without coordinated mutex guards leading to race conditions.
- ✅ **JUSTIFIED / PERMITTED:**
  - **Pure Binary Hardware / Signal Status:** A true, independent binary condition that is not a pipeline lifecycle phase (e.g. `mic_muted: bool`, `noise_gate_active: bool`).
  - **Static / Persistent Feature Configuration Flags:** Immutable or user-configured binary settings (e.g. `enable_vad: bool`, `echo_cancellation: bool`).
  - **Transient Flow Control within Single Function Scope:** A local variable tracking immediate iteration state (e.g. `let has_speech = ...;` or `let mut seen_first_token = false;`).
  - **Atomic Cancellation / Shutdown Tokens:** `tokio_util::sync::CancellationToken` or worker shutdown flags (`AtomicBool` for loop termination only).

---

## 6. Function Standards & Code Cleanliness

- **Function line cap (soft):** No function exceeds 50 lines without documented justification.
- **Docstrings:** Exactly one `///` doc comment per function that states what it does, what it takes, and what it returns. No narrative step-comments inside function bodies; runtime traces belong in `log::info!` / `log::warn!`. Exception: `// SAFETY:` blocks (required for `unsafe`) and `// INVARIANT:` comments explaining non-obvious preconditions are always permitted.
- **No step-comment sequences:** If a function body needs numbered step comments (`// 1. do X`, `// 2. do Y`), each step must become a named private helper function.
- **No toggle functions:** A function named `engage()` must only engage. `if condition { engage } else { disengage }` in one function body is banned. Use discrete named functions.
- **Struct bundling for parameter lists (>5 arguments):** Any function or constructor taking more than 5 arguments must group related parameters into a dedicated typed config or handles struct (e.g. `VadActorConfig`, `VadActorHandles`, `PlaybackTelemetryHandles`).
- **Zero `#[allow(...)]` policy:** `#[allow(clippy::too_many_arguments)]`, `#[allow(dead_code)]`, `#[allow(unused_variables)]`, and all other lint suppressions are strictly banned.
- **Zero `_` prefixed masking:** Never prefix unused variables or fields with `_` to silence warnings. If an item is not needed, delete it.
  - *RAII drop guard exception:* `_` is strictly reserved for genuine RAII drop guards (`_stream: Option<cpal::Stream>`, `_log_guard: Option<WorkerGuard>`, `_thread_handle`) where holding the handle in memory is required to keep hardware streams or workers alive.

---

## 7. Error Handling & Resilience

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

## 8. Concurrency, Threading & Audio Hot Path

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

## 9. Production Rust Best Practices

- **Structured Logging:** All logs must specify domain tags: `log::info!("[Domain::Subsystem] Action completed status=ok")`. Never use `println!` or `eprintln!` in `src/`.
- **Dropped Counter Telemetry:** High-throughput channel `try_send` calls must increment an atomic dropped-counter handle and log warnings if backpressure occurs.
- **Newtype Pattern:** Prefer lightweight typed wrappers or domain aliases over raw primitives for identifiers (e.g. `TurnId(u32)`).
- **Exhaustive Enums for State:** Model lifecycles using explicit state enums with transition functions rather than coordinating bags of loose booleans.

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