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
- **File size ceiling:** Flag and justify files exceeding ~600 lines.
- **`mod.rs` & `lib.rs`:** `mod.rs` is for module declarations + re-exports only. Zero business logic. `lib.rs` is for module declarations + Tauri app setup only. Zero business logic.
- **Visibility:** Use `pub(crate)` over `pub` unless crossing the crate boundary (IPC or integration tests).

### 1.2 Function Standards
- **Function line cap (soft):** No function exceeds 50 lines without documented justification. Flag and review at review time.
- **No step-comment sequences:** If a function body needs numbered step comments (`// 1. do X`, `// 2. do Y`), each step must become a named private function. Step comments are compensating for missing abstraction.
- **No toggle functions:** A function named `engage()` must only engage. `if condition { engage } else { disengage }` in one function body is banned. Use two explicitly named functions.
- **Comment policy:** One `///` doc comment per function that states: what it does, what it takes, what it returns. No per-line comments inside function bodies. Runtime trace belongs in `log::info!` / `log::warn!`, not inline comments.
- **Struct bundling for parameter lists:** Any function or constructor taking more than 5 arguments must group related parameters into a dedicated config/handles struct (e.g. `PlaybackTelemetryHandles`, `VadActorConfig`, `AppStateTelemetryHandles`). `#[allow(clippy::too_many_arguments)]` is strictly banned.
- **No dead `_` prefixed variables or fields:** Never mask unused parameters, variables, or struct fields with a `_` prefix (e.g. `_playback_energy`, `_state`, `_last_user_turn`) to silence the compiler. If a parameter or struct field is not needed, delete it.
- **RAII drop guard exception for `_`:** The `_` prefix is strictly reserved for genuine RAII drop guards (`_stream: Option<cpal::Stream>`, `_log_guard: Option<WorkerGuard>`, `_thread_handle`) where holding the handle in memory is required to keep hardware audio streams or background logging workers alive, or for foreign trait callbacks with fixed signatures.

### 1.3 Error Handling
- **No `unwrap()` in `src/`:** Banned except on `RwLock`/`Mutex` guards (poisoned lock = unrecoverable).
- **Propagation:** Use `?` with `.context("...")` (`anyhow`) in services and persistence.
- **IPC boundary:** Errors returned across Tauri IPC must be typed enums using `thiserror`.
- **No silent error swallowing:** `let _ = result` is banned. Every channel send or fallible call must either propagate with `?` or log on error: `if let Err(e) = tx.send(...) { log::warn!("[Module] Channel send failed: {}", e); }`.
- **No backward-compatibility or fallback chains:** No `if path A fails, try path B, try path C`. One deterministic path per operation. If the path fails, return `Err(...)` or log a warning. Silent retry/fallback paths hide real failures.

### 1.4 Async & Concurrency
- **Non-blocking executor:** Never execute CPU-heavy work (inference, audio decode) on Tokio worker threads. Use `tokio::task::spawn_blocking`.
- **Channels over locks:** Use Tokio/crossbeam channels for inter-service communication. Avoid new `Arc<Mutex<T>>`.
- **Audio Hot Path:** VAD → STT → LLM → TTS hot path must be zero allocations and zero lock acquisitions. Use snapshotted values.
- **Canonical lock order:** When acquiring multiple `Mutex`/`RwLock` guards, always acquire in this order: `state.engine` → `state.realtime_engine`. Never reversed. Lock order inversion is a confirmed deadlock source.
- **No polling where events suffice:** If a subsystem knows when its state changes (e.g. `PlaybackEngine` knows when it starts/drains), it must emit a `VoxEvent` rather than relying on a caller to poll an atomic on a timer.

### 1.5 Routing Context
- **No inline settings re-reads per handler:** When an IPC handler needs `pipeline_mode`, `interaction_mode`, and `owner`, these must be resolved once into a `RoutingContext` struct, not duplicated across each handler with an inline `state.settings.read().unwrap()` block.

### 1.6 Linting & Zero-Suppression Policy
- `cargo check`: Mandatory zero errors.
- `cargo clippy --all-targets`: Mandatory zero warnings.
- **Zero `#[allow(...)]` policy:** `#[allow(clippy::too_many_arguments)]`, `#[allow(dead_code)]`, `#[allow(unused_variables)]`, and all other compiler lint suppressions are strictly banned across the entire codebase. Fix the signature or delete the dead code.
- `cargo fmt`: Must be run before committing.

### 1.5 Testing & Evaluation Taxonomy & Principles

#### Testing Principles (Zero Noise Policy)
1. **Never test trivial language invariants or compiler guarantees:**
   - Banned: Tests that solely construct a struct with default values and assert `field == expected`.
   - Banned: Tests that serialize/deserialize an enum and assert string equality (serde derive already handles this).
   - Banned: Tests that assert enum discriminants or `From` implementations with no business logic.
   - Banned: Instantiating an ad-hoc local `Mutex` or fake struct in a test and claiming it tests a subsystem cache.
2. **Unit Tests (`#[cfg(test)] mod tests`)**:
   - Must test non-trivial algorithmic logic, state transitions, parsing, math, or error edge cases.
   - Must validate deterministic transforms (e.g. text sanitization, transliteration logic, token accumulators).
3. **Integration Tests (`app/src-tauri/tests/<feature>_test.rs`)**:
   - Must test subsystem interaction, lifecycle contracts, concurrency, and error recovery using public `vox_lib` APIs.
   - Must test real failure modes: what happens when a dependency fails, when state races occur, or when buffers overflow.
4. **Performance Benchmarks (`app/src-tauri/benches/<feature>_bench.rs`)**:
   - Banned: Micro-benchmarks measuring simple struct serde or isolated mutex locking in a tight loop.
   - Must execute real pipelines: ingest real inputs (e.g. WAV audio, text corpora), invoke the actual ML inference or service dispatch, and record per-stage and end-to-end latency ($T_{\text{stt}}$, $T_{\text{dispatch}}$, $T_{\text{e2e}}$) and throughput.
   - Must support CLI arguments via `clap` (e.g. `--clip`, `--mode`) so developers and CI can test realistic workloads.
   - Must assert physical or state outcomes where possible (e.g., verifying clipboard contents, memory limits, output integrity).

| Category | File Location | Command | Access Scope | Primary Output |
|---|---|---|---|---|
| **Unit Test** | Bottom of target `.rs` file in `#[cfg(test)] mod tests` | `cargo test --lib` | Private + public functions | Pass / Fail |
| **Integration Test** | `app/src-tauri/tests/<feature>_test.rs` | `cargo test --test <name>` | Public `vox_lib` API only | Structural & Lifecycle Correctness |
| **Evaluation (Eval)** | `app/src-tauri/evals/<capability>/` | `cargo run --example eval_<capability>` | Crate API + Models + Datasets | Statistical Accuracy + LLM Judge Score |
| **Performance Benchmark** | `app/src-tauri/benches/<feature>_bench.rs` | `cargo test --bench <name>` | Custom `fn main()` (`harness=false`) | Real Latency ($T_{\text{E2E}}$) & Throughput |
| **CLI Utility Tool** | `app/src-tauri/examples/<name>.rs` | `cargo run --example <name>` | Runnable dev tools | Standalone Utility CLI |

#### 1.6 Evals Directory Structure Standard (`app/src-tauri/evals/`)
Every evaluation capability suite lives in its own dedicated subdirectory under `app/src-tauri/evals/<capability>/`:

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

---

## 4. Documentation Standards

Root architecture/reference docs in `docs/*.md` follow a **uniform frontmatter + "How to read"** convention so agents and contributors get accurate, scannable context fast. This is a house convention (composed from Diátaxis information-architecture, RFC-style YAML frontmatter, and ADR-style scope preambles) — not a third-party spec.

### 4.1 Required frontmatter (YAML, at file top)

```yaml
---
title: "Doc Title"
audience: "Internal — <who this is for>"
last_updated: YYYY-MM-DD
owners: "<role> role"
related_docs:
  - "docs/other.md — one-line relationship"
---
```

`design.md` already carries content YAML (tokens); merge these keys into its existing frontmatter rather than adding a second block.

### 4.2 Required "How to read this doc" section

Immediately after the title/intro, add a `## 0. How to read this doc` block with exactly these bullets:

- **Audience:** who the doc is for.
- **Scope:** what it covers.
- **Convention:** how claims are cited (e.g. `path/file.ts` pointers; no invented code blocks).
- **Non-goals:** what it is explicitly NOT, with cross-links (→ `docs/other.md`).
- **SSOT:** where the authoritative detail lives (so the doc never duplicates it).

Narrative docs (`vision.md`, `roadmap.md`, `decision-framework.md`) get the frontmatter but may omit §0 since they are self-evidently prose.

### 4.3 Single Source of Truth (SSOT) rule

Docs must **point, not copy**. The canonical homes are:

| Topic | Owner doc |
|---|---|
| Perf & memory optimizations | `docs/features/performance-memory-optimizations.md` |
| Design tokens / elevation / type | `docs/design.md` (+ `app/src/index.css`) |
| IPC event contract | `docs/backend.md` §8 |
| Settings reload policies | `docs/backend.md` §10 |
| Memory subsystem | `docs/features/memory-architecture.md` |
| Frontend architecture | `docs/frontend.md` |

When adding a root doc, copy the `frontend.md` header + §0 shape as the template. Keep code blocks out of architecture docs — cite files instead; schemas/types are linked, never pasted.