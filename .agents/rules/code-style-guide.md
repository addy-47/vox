---
trigger: manual
description: Comprehensive Vox code style guide and engineering standards for Rust (backend) and TypeScript/React (frontend).
---

---
description: Vox Code Style Guide & Engineering Standardsfor with tier-mapping as well
---

This document contains the durable coding standards for Vox. **Agents doing write operations must read this file before modifying code.**

---

## The Hardware Mapping for Vox 

- Architecture decisons are decided based on feasibility with recommended tiers - hence vox must suport dynamic degrade and upgrade of architecture based on tier
where Tier 2 is recommended for users

* **Tier 1A: 8GB Pure Local (no gpu):** Working Memory FIFO variation only (Simple buffer to manage context window)

* **Tier 1B: [RECOMMENDED] Pure Local (with gpu):** Working Memory + Personal Memory + Semantic Memory.

* **Tier 2A: [RECOMMENDED/NO-COST] Hybrid Stack ( Remote LLM + Local Audio ):** Working Memory + Episodic + Semantic .

* **Tier 2B: [RECOMMENDED/DEFAULT] Hybrid Stack ( Cloud LLM + Local Audio ):** Working Memory + Episodic + Semantic. 

* **Tier 3: [BEST-PERFORMANCE] Realtime S2S (WebSocket):** Provider-managed Working Memory + Episodic & Semantic (managed via early tool calls) . 

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

### 1.5 Testing & Evaluation Taxonomy & Structure
| Category | File Location | Command | Access Scope | Primary Output |
|---|---|---|---|---|
| **Unit Test** | Bottom of target `.rs` file in `#[cfg(test)] mod tests` | `cargo test --lib` | Private + public functions | Pass / Fail |
| **Integration Test** | `app/src-tauri/tests/<feature>_test.rs` | `cargo test --test <name>` | Public `vox_lib` API only | Structural Correctness |
| **Evaluation (Eval)** | `app/src-tauri/evals/<capability>/` | `cargo run --example eval_<capability>` | Crate API + Models + Datasets | Statistical Accuracy + LLM Judge Score |
| **Performance Benchmark** | `app/src-tauri/benches/<feature>_bench.rs` | `cargo test --bench <name>` | Custom `fn main()` (`harness=false`) | Latency (ms/pair) & Throughput |
| **CLI Utility Tool** | `app/src-tauri/examples/<name>.rs` | `cargo run --example <name>` | Runnable dev tools | Standalone Utility CLI |

#### 1.6 Evals Directory Structure Standard (`app/src-tauri/evals/`)
Every evaluation capability suite lives in its own dedicated subdirectory under `app/src-tauri/evals/<capability>/`:
```
```

**Mandatory Header Format for `tests/`, `evals/`, `benches/`, `examples/`:**
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

## 2. Frontend Standards (`app/src/`)

- **Package Manager:** Always use `pnpm`, never `npm` or `yarn`.
- **Zero Hardcoded Text / Labels:** Banned inline hardcoded strings, labels, select options, or mock objects inside components/pages. All static content must live in `src/data/` (e.g., `appData.ts`, `settingsDomains.ts`).
- **Strict Service Layer Boundary:** Banned raw `@tauri-apps/api` invoke calls or direct fetches inside React components. All IPC/API calls MUST pass through dedicated service modules in `src/services/` (e.g. `pipelineService.ts`, `settingsService.ts`).
- **Page Responsibility (Layout Only):** Files in `src/pages/` MUST only define visual structure, routing, and layout composition. Heavy business logic, state sync, and data transformations belong in `src/services/`, `src/hooks/`, or `src/store/`.
- **Modular Component Subdirectories:** `src/shared/components/` must be structured into logical feature/domain subdirectories (e.g., `layout/`, `home/`, `history/`, `settings/`, `monitoring/`, `common/`). Banned flat, uncategorized component directories.
- **Shared state:** `src/context/` or Zustand `src/store/` for low-frequency global state. Never context for fast-changing animation values — those belong in local state or refs.
- **Reusable stateful logic:** `src/hooks/` when the same logic appears in 2+ components.
- **Component Consolidation & Deduplication:** Audit and merge components performing identical or near-identical visual/functional tasks into clean, configurable shared primitives.
- **Type Safety:** Strict TypeScript. `any` is strictly prohibited — define explicit interfaces/types for all props and service returns.
- **State Management:** Transient UI state in local component state/refs. Low-frequency app configuration in React Context or Zustand (`src/store/`). Shared stateful logic extracted to custom hooks (`src/hooks/`).
- **Verification:** Run `pnpm lint` and `pnpm build` after every modification. Zero warnings/errors permitted.


### Layout Rules

- Desktop: floating bottom `EdgeNav` capsule, monitoring as popover panel bottom-left.
- Mobile: monitoring moves to `/monitoring` route with solid background. Nav capsule gets a 4th tab.
- Viewport transitions are handled — mobile→desktop redirects from `/monitoring` to `/` and relaunches popover. Desktop→mobile closes popover and routes to `/monitoring`. Never break this.
- Mobile Orb scales to `min(92vw, 85vh)`. Desktop is `min(70vw, 65vh)`. Do not change these without design review.



---

## 3. General & Infrastructure Standards

- **Constants:** No magic inline values. Constants go at top of file. Shared subsystem constants go in `core/constants.rs`.
- **Secrets & Credentials:** Sensitive values in `temp/.env` (never committed). GPU server credentials in `temp/server.txt`.
- **Dependencies:** Never add a new Rust crate (`Cargo.toml`) or npm package (`package.json`) without explicit approval.
- **Model Registration:** Every model added or updated in Vox MUST have an entry in `~/.vox/models/models_manifest.json` and the app catalog.