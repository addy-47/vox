---
trigger: manual
description: Comprehensive Vox code style guide and engineering standards for Rust (backend) and TypeScript/React (frontend).
---

# Vox Code Style Guide & Engineering Standards

This document contains the durable coding standards for Vox. **Agents doing write operations must read this file before modifying code.**

---

## 1. Rust Backend Standards (`app/src-tauri/src/`)

### 1.1 Module Organization
- **Domain over type:** Group code by domain (`services/memory/nli.rs`), never by Rust construct (`models.rs`).
- **Single responsibility:** 1 responsibility per file. If a file cannot be described in 1 sentence, split it.
- **File size ceiling:** Flag and justify files exceeding ~600 lines. Current large files: `services/pipeline.rs` (1742), `ipc/pipeline.rs` (1327), `ipc/settings.rs` (1026), `core/settings.rs` (957).
- **`mod.rs` & `lib.rs`:** `mod.rs` is for module declarations + re-exports only. Zero business logic. `lib.rs` is for module declarations + Tauri app setup only. Zero business logic.
- **Visibility:** Use `pub(crate)` over `pub` unless crossing the crate boundary (IPC or integration tests).

### 1.2 Error Handling
- **No `unwrap()` in `src/`:** Banned except on `RwLock`/`Mutex` guards (poisoned lock = unrecoverable).
- **Propagation:** Use `?` with `.context("...")` (`anyhow`) in services and persistence.
- **IPC boundary:** Errors returned across Tauri IPC must be typed enums using `thiserror`.
- **No silent error swallowing:** Never `let _ = res`. Log discarded errors: `if let Err(e) = ... { tracing::warn!(...) }`.

### 1.3 Async & Concurrency
- **Non-blocking executor:** Never execute CPU-heavy work (inference, audio decode) on Tokio worker threads. Use `tokio::task::spawn_blocking`.
- **Channels over locks:** Use Tokio/crossbeam channels for inter-service communication. Avoid new `Arc<Mutex<T>>`.
- **Audio Hot Path:** VAD → STT → LLM → TTS hot path must be zero allocations and zero lock acquisitions. Use snapshotted values.

### 1.4 Linting & Verification
- `cargo check`: Mandatory zero errors.
- `cargo clippy --all-targets`: Mandatory zero warnings. Never suppress with `#[allow(...)]` without an explanatory comment.
- `cargo fmt`: Must be run before committing.

### 1.5 Testing Placement & Taxonomy
| Test Category | File Location | Command | Access Scope |
|---|---|---|---|
| **Unit Test** | Bottom of target `.rs` file in `#[cfg(test)] mod tests` | `cargo test --lib` | Private + public functions |
| **Integration Test** | `app/src-tauri/tests/<feature>_test.rs` | `cargo test --test <name>` | Public `vox_lib` API only |
| **Performance Benchmark** | `app/src-tauri/benches/<feature>_bench.rs` | `cargo test --bench <name>` | Custom `fn main()` (`harness=false`) |
| **CLI Utility Tool** | `app/src-tauri/examples/<name>.rs` | `cargo run --example <name>` | Runnable dev tools |

**Mandatory Header Format for `tests/`, `benches/`, `examples/`:**
```rust
//! ============================================================================
//! <filename> — <one-line description>
//! ============================================================================
//! Category     : [Integration Test | Benchmark | Utility Tool]
//! Component    : <target module or subsystem>
//! Prerequisites: <required models, env vars, or services>
//! Execution    : <exact cargo command>
//! ============================================================================
```

---

## 2. Frontend Standards (`app/src/`)

- **Package Manager:** Always use `pnpm`, never `npm` or `yarn`.
- **Type Safety:** Strict TypeScript. `any` is strictly prohibited — define interfaces or type aliases explicitly.
- **Service Layer:** Centralize API / IPC calls in `src/services/`. Components render layout and delegate data fetching/mutations.
- **State Management:** Local component state for UI transients. React Context only for low-frequency global state (theme, settings). Shared stateful logic extracted to custom hooks in `src/hooks/`.
- **Verification:** Run `pnpm lint` and `pnpm build` to verify frontend changes.

---

## 3. General & Infrastructure Standards

- **Constants:** No magic inline values. Constants go at top of file. Shared subsystem constants go in `core/constants.rs`.
- **Secrets & Credentials:** Sensitive values in `temp/.env` (never committed). GPU server credentials in `temp/server.txt`.
- **Dependencies:** Never add a new Rust crate (`Cargo.toml`) or npm package (`package.json`) without explicit approval.
- **Model Registration:** Every model added or updated in Vox MUST have an entry in `~/.vox/models/models_manifest.json` and the app catalog.