---
trigger: manual
description: Rust backend coding standards, module boundaries, constant hierarchies, and concurrency invariants for Vox (`app/src-tauri/src/`).
---

# Vox — Backend Code Style Guide & Engineering Standards

Durable engineering standards for the Vox native Rust backend (`app/src-tauri/src/`). Read before creating or editing Rust backend source files.

---

## 1. Hardware Tiers & Capability Matrix

Vox degrades and scales across hardware capabilities. Tier 2B is the default production baseline.

| Tier | Hardware | Pipeline Mode | Ingestion | Retrieval | Tool Calling |
| :--- | :------- | :-----------: | :-------: | :-------: | :----------: |
| **1A** | 8GB, CPU-only | Modular (Local) | FIFO only | Context window | None |
| **1B** | 8GB+, dedicated GPU | Modular (Local) | Async background | Full (vector + BM25) | Local LLM dependent |
| **2A** | Hybrid (Remote LLM) | Modular (Remote LLM) | Async background | Full | Remote LLM dependent |
| **2B** (default) | Hybrid (Cloud LLM) | Modular (Cloud LLM) | Async background | Full | Full cloud tool calls |
| **3** | Realtime S2S | Realtime (WebSocket) | Provider-managed | Early tool calls | Full provider tools |

---

## 2. Module Organization & Grammar Order

- **Domain organization:** Group files by functional domain (`services/memory/nli.rs`), never by language construct (`models.rs`).
- **Single responsibility:** One distinct domain responsibility per file.
- **Module files:** `mod.rs` contains module declarations, re-exports, and domain constants only. No business logic. `lib.rs` contains Tauri plugin registrations and app setup only.
- **Visibility:** Use `pub(crate)` by default. Reserve `pub` strictly for IPC commands and integration test crate exports.

### 2.1 File Grammar Order

All Rust files strictly follow this top-to-bottom sequence:
1. **Imports:** Grouped: `std::`, external crates, internal `crate::`, `super::`. Use function-level doc comments (`///`); do not write crate header blocks (`//!`) in `lib.rs` or `main.rs`. A `mod.rs` MAY carry a `//!` module-ownership block — that is a *module* header, not a crate header, and it is the only way Rust lets a module document its own ownership contract.
2. **Constants & Type Aliases:** `const ...`, `pub(crate) type ...`.
3. **Data Structures:** `struct` and `enum` declarations with derived traits.
4. **Trait Implementations:** `impl Trait for Struct { ... }`.
5. **Inherent Implementations:** `impl Struct { ... }` (constructors first, public methods, then private methods).
6. **Helper Functions:** Private utility functions (`fn ...`).

---

## 3. Constant Placement Hierarchy

Avoid magic numbers. Place constants into one of three tiers:

1. **User Defaults ([`core/defaults.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/defaults.rs)):** User-facing default configurations and fallback parameters.
2. **Subsystem Constants (`services/<domain>/mod.rs`):** Shared sample rates, chunk sizes, buffer capacities, and model directories within a domain.
3. **File-Local Constants (Top of file):** Values private to a single file, declared directly below imports.

---

## 4. Authoritative Specifications (SSOT)

Align implementations with authoritative specifications in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/):
- **Lifecycle & State Machines:** [events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)
- **Filesystem & Config Paths:** [storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md)
- **Keystroke Injection & Dictation:** [dictation-spec.md](file:///home/addy/projects/apps/vox/docs/specs/dictation-spec.md)
- **LLM Harness & Streaming:** [harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)
- **Tauri IPC Command Contracts:** [ipc-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md)
- **Database Schemas & Migrations:** [db-spec.md](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)
- **Working & Personal Memory:** [memory-spec.md](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)
- **Ownership & Teardown Lifetimes:** [ownership-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md)
- **Provider & Model Catalog:** [provider-model-catalog-spec.md](file:///home/addy/projects/apps/vox/docs/specs/provider-model-catalog-spec.md)
- **Notifications & Telemetry:** [notifications-spec.md](file:///home/addy/projects/apps/vox/docs/specs/notifications-spec.md)

---

## 5. State Representation & Flag Discipline

- **Derive lifecycle from state enums:** Never introduce boolean flags that shadow state transitions (e.g. `is_connected`, `is_idle`, `is_recording`). Query the canonical state enum or settings directly.
- **Model readiness via Options:** Represent model availability via `Option<Engine>` or explicit status enums rather than loose boolean bags (`is_llm_loaded`).
- **Permitted booleans:** Hardware signal states (`mic_muted: bool`), persistent config flags (`echo_cancellation: bool`), local loop state (`let mut seen_token = false;`), and atomic cancellation tokens (`CancellationToken`).

---

## 6. Function Standards & Hygiene

- **Function size:** Keep functions under 50 lines. Extract complex sequences into named private helpers.
- **Discrete operations:** Avoid toggle functions (`fn toggle(...)`). Use distinct functions (`fn engage(...)`, `fn disengage(...)`).
- **Argument bundling:** Group parameter lists exceeding 5 arguments into dedicated typed config structs (`VadActorConfig`).
- **Lint policy:** Zero `#[allow(...)]` attributes. Delete unused variables and functions. Prefix with `_` strictly for genuine RAII drop guards (`_stream: Option<cpal::Stream>`).

---

## 7. Error Handling

- **No `unwrap()` in `src/`:** Allowed only on poisoned mutex locks.
- **Propagation:** Use `?` with context (`anyhow::Context`) in services.
- **IPC boundary:** Return typed `thiserror` error enums across Tauri IPC commands.
- **No silent error swallowing:** Channel sends and fallible calls must propagate errors or log warnings on failure.
- **Deterministic routing:** Avoid chained fallbacks (`try A, then B, then C`). Execute one deterministic path; report failure if it errors.

---

## 8. Concurrency & Hot Path

- **Actor-Engine separation:** The actor manages OS thread lifecycle and state; the engine executes inference logic. Never combine both into a single struct.
- **Thread placement:**
  - Dedicated OS threads for inference models (VAD, STT, LLM, TTS).
  - Tokio async workers for IPC, network I/O, and disk operations.
- **Audio hot path:** Zero allocations and zero lock acquisitions in the active audio loop. Workers operate on snapshotted settings.
- **Lock ordering:** Acquire `state.engine` before `state.realtime_engine`. Never invert lock order.
- **Event-driven communication:** Emit `VoxEvent` on state transitions instead of polling atomics.

---

## 9. Logging & Telemetry

- **Structured domain tags:** Format log messages with subsystem tags: `log::info!("[Domain::Subsystem] Operation completed status=ok")`. Never use `println!` in `src/`.
- **Dropped packet telemetry:** Channel sends encountering backpressure must increment atomic dropped counters and emit warnings.

---

## 10. Testability Seams & Runtime Generics

- **Generic Tauri Runtime (`AppHandle<R: tauri::Runtime>`):** Parameterize actor constructors and helper functions over `<R: tauri::Runtime>`. Avoid hardcoding the default `Wry` runtime to allow test execution with `tauri::test::mock_app()`.
- **Decoupled input seams:** Avoid isolated module-level statics. Expose entry functions (`ingest_audio(&[f32])`) allowing test harnesses to feed synthetic data without physical hardware.
- **Hardware decoupling:** High-level orchestrators must accept optional channel overrides for headless test environments where hardware audio devices are absent.