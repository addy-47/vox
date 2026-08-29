# Phase 10 — Backend Review Resolution Master Spec

> **Status:** ACTIVE Architectural Resolution & Backend Implementation Specification  
> **Location:** `docs/plans/phase10/backend_review_resolution_spec.md`  
> **Source Review:** `docs/backend-review/BACKEND_REVIEW.md` (and `docs/backend-review/sprint-01.md` through `sprint-11.md`)  
> **Sprint Checklist:** [`docs/plans/phase10/backend_review_sprints.md`](file:///home/addy/projects/apps/vox/docs/plans/phase10/backend_review_sprints.md)  
> **Methodology:** 3-Step Review-to-Spec Protocol:
> 1. `/create-sprints` — Scope Enumeration & Checklist Tracking
> 2. `/feedback-review` — Targeted Live Code & Intent Validation (No Unrelated Code Read)
> 3. `/grill-me` — Socratic Logic Exposition, Tradeoff Adjudication & Backend Engineer Action Requirements

---

## Module Index & Resolution Summary

| Module | Name | Scope | Total Items | 🔴 Critical | Status |
|:---|:---|:---|:---:|:---:|:---:|
| **Sprint 01** | Core + App Shell | `core/*`, `lib.rs`, `main.rs`, `tray.rs`, `window_main.rs`, `window_customizer.rs`, `wizard.rs` | 18 | 1 | **COMPLETED** |
| **Sprint 02** | IPC Layer | `ipc/*.rs` (voices, audio, setup, settings, memory, history) | 18 | 3 | **COMPLETED** |
| **Sprint 03** | Persistence | `persistence/*.rs` (worker, mutations, queries, db, voices) | 14 | 3 |  **COMPLETED** |
| **Sprint 04** | Audio + Translit + Utils | `services/audio/*`, `services/translit.rs`, `utils/*` | 12 | 2 |  **COMPLETED** |
| **Sprint 05** | Dictation + VAD | `services/dictation/*`, `services/vad/*` | 19 | 2 |  **COMPLETED** |
| **Sprint 06** | LLM + Providers | `services/llm/*` (actor, openai_compat, embedded, llama_cpp) | 16 | 3 |  **COMPLETED** |
| **Sprint 07** | Memory Pipeline | `services/memory/*` (working_memory, pipeline, classifiers) | 14 | 2 |  **COMPLETED** |
| **Sprint 08** | Pipeline Orchestration | `services/pipeline/*` (router, modular, realtime, context) | 20 | 3 | PENDING |
| **Sprint 09** | Realtime | `services/realtime/*` (deepgram, gemini, audio_bridge) | 23 | 1 | PENDING |
| **Sprint 10** | STT + TTS | `services/stt/*`, `services/tts/*` | 23 | 3 | PENDING |
| **Sprint 11** | Monitoring + Setup | `monitoring/*`, `setup/*` | 19 | 1 | PENDING |

---

## Sprint 01 — Core + App Shell Resolution Ledger

*(Entries populated as Socratic review and feedback validations progress)*

### Sprint 001: Resilient `settings.json` Deserialization & Anti-Wipe Protection

- **Review Point:** Any unparseable enum value in `settings.json` wipes the entire settings file (`core/settings.rs:937-951`)
- **Severity:** 🔴 WILL BREAK (Data Loss / Unsafe State Reset)
- **Target File:** [`app/src-tauri/src/core/settings.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/settings.rs#L933-L959)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `VoxSettings::load()` performs a strict monolithic `serde_json::from_str::<Self>(&content)`. If any enum variant or field fails deserialization (e.g., config schema evolution, unrecognized VAD/TTS provider, manual edit), `from_str` returns `Err`.
- **Failure Cascade:** Execution falls into the corruption block, renames `settings.json` to a static `settings.json.bak` (which overwrites any previous backup), falls back to `Self::default()`, and calls `settings.save()`, immediately overwriting the user configuration with factory defaults.
- **Verdict:** **Confirmed Critical Bug (🔴 WILL BREAK)**.

#### 2. Logic Explanation & Engineering Invariants
- Deserialization failure in a sub-struct (e.g. `vad_backend`, `tts_provider`) must never cause loss of unrelated configuration (API keys, history, hotkeys).
- Schema drift must degrade gracefully to field-level defaults.
- A backup must never overwrite a previous valid backup.

#### 3. Recommended Actions for Backend Engineer
1. **Field & Enum Resilience:**
   - Add `#[serde(default)]` to sub-structs and critical fields across `VoxSettings`.
   - Add fallback variants (e.g. `#[serde(other)] Unknown`) or custom deserializers to settings enums so unknown string values fallback to their default variant rather than failing outer parsing.
2. **Partial / Layered Recovery in `load()`:**
   - On top-level deserialization failure, attempt partial recovery using `serde_json::Value` to merge known valid fields over `VoxSettings::default()`.
3. **Safe Backup File Naming:**
   - If corrupted beyond repair, rename to a timestamped backup path `settings.corrupted.{unix_timestamp}.json` rather than static `settings.json.bak`.
4. **No Eager Overwrite:**
   - Do not call `settings.save()` on load failure; keep the in-memory fallback until the user explicitly commits a mutation.

### Sprint 002: VAD Backend Default & Documentation Alignment

- **Review Point:** `VadBackendOption` default contradicts its own doc — Earshot is never the default (`core/settings.rs:17-25`)
- **Severity:** 🟠 REAL COST AT SCALE (Documentation & Configuration Drift)
- **Target File:** [`app/src-tauri/src/core/settings.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/settings.rs#L17-L25)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** The doc comment on `VadBackendOption::Earshot` states `~20x faster than TenVAD. Default starting from Phase 8.`, but the `#[default]` attribute on the enum and `VadSettings::default()` explicitly set `vad_backend: VadBackendOption::TenVad`.
- **Verdict:** **Confirmed Documentation / SSOT Drift**.

#### 2. User Decision & Architecture Resolution
- **Decision:** Retain **`TenVad`** as the default VAD backend across Vox. `Earshot` remains the pure-Rust zero-ONNX alternative.
- **SSOT Invariant:** Defaults must be centralized in `core/defaults.rs` and reflected consistently in doc comments, struct defaults, and settings migration.

#### 3. Recommended Actions for Backend Engineer
1. **Doc Comment Correction:**
   - Update `VadBackendOption` doc comments in [`core/settings.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/settings.rs#L17-L25) to accurately document `TenVad` as the default engine and `Earshot` as the zero-dependency embedded alternative.
2. **Centralize Constant:**
   - Export `pub const DEFAULT_VAD_BACKEND: VadBackendOption = VadBackendOption::TenVad;` in [`core/defaults.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/defaults.rs) and reference it inside `VadSettings::default()`.

### Sprint 003: Elimination of `block_on` and Tokio Mutexes in Shell/Tray Contexts

- **Review Point:** `block_on` a tokio async `Mutex` from synchronous shell contexts (`tray.rs:87,93`)
- **Severity:** 🟠 REAL COST AT SCALE (Thread Starvation / Tokio Worker Blocking / Deadlock Risk)
- **Target Files:** [`app/src-tauri/src/tray.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/tray.rs#L82-L107), [`app/src-tauri/src/core/state.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/state.rs#L168-L171)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `AppState::hud_menu_item` and `AppState::hud_visible` are declared as `tokio::sync::Mutex`. In `tray.rs:87` and `tray.rs:93`, synchronous menu callback function `sync_live_menu_item` calls `tauri::async_runtime::block_on(state.hud_menu_item.lock())` and `tauri::async_runtime::block_on(state.hud_visible.lock())`.
- **Failure Mode:** Calling `block_on` inside an asynchronous Tokio executor context (or GTK event loop) risks blocking worker threads, thread exhaustion, or deadlocks if locks are contended.
- **Verdict:** **Confirmed Defect (🟠 REAL COST AT SCALE)**.

#### 2. Logic Explanation & Engineering Invariants
- `hud_visible` is a simple boolean flag that requires no async suspension.
- `hud_menu_item` holds a Tauri UI menu handle and is never held across `.await` points.
- Synchronous UI / shell contexts must never block asynchronous runtimes.

#### 3. Recommended Actions for Backend Engineer
1. **Migrate to Synchronous Mutexes / Atomics in `AppState`:**
   - Change `pub hud_visible: Mutex<bool>` $\to$ `pub hud_visible: Arc<AtomicBool>` (or `parking_lot::Mutex<bool>`).
   - Change `pub hud_menu_item: Mutex<Option<CheckMenuItem<Wry>>>` $\to$ `pub hud_menu_item: parking_lot::Mutex<Option<CheckMenuItem<Wry>>>`.
2. **Remove `block_on` in `tray.rs`:**
   - Replace `tauri::async_runtime::block_on(state.hud_menu_item.lock())` with direct `state.hud_menu_item.lock()`.
   - Replace `tauri::async_runtime::block_on(state.hud_visible.lock())` with `state.hud_visible.load(Ordering::Relaxed)`.
3. **Update IPC Callers:**
   - In `ipc/tray.rs` and `ipc/settings/mutation.rs`, replace `.lock().await` on these two fields with direct synchronous lock / atomic load.

### Sprint 004: Consolidation of Redundant `InteractionState` Storage

- **Review Point:** `InteractionState` has three redundant representations that can diverge (`core/state.rs:93-140`)
- **Severity:** 🟠 REAL COST AT SCALE (State Inconsistency & Unnecessary Locking)
- **Target File:** [`app/src-tauri/src/core/state.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/state.rs#L86-L142)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `PipelineAtomics` maintains three parallel state fields:
  1. `state: Arc<parking_lot::Mutex<InteractionState>>`
  2. `is_assistant_speaking: Arc<AtomicBool>`
  3. `current_state_atomic: Arc<AtomicU32>`
- **Analysis:** `current_state_atomic` was introduced for lock-free 10Hz monitoring and `is_assistant_speaking` for lock-free audio callback suppression. Maintaining a separate `parking_lot::Mutex<InteractionState>` alongside two atomics creates redundancy, potential drift if fields are mutated independently, and unnecessary memory allocation.
- **Verdict:** **Confirmed Architectural Redundancy (🟠 REAL COST AT SCALE)**.

#### 2. Logic Explanation & Engineering Invariants
- State transitions must be atomic and lock-free across both realtime audio threads and async IPC handlers.
- A single atomic field (`AtomicU32` holding the `InteractionState` discriminant) is sufficient for all readers and writers.

#### 3. Recommended Actions for Backend Engineer
1. **Single Source of Truth Atomic:**
   - Make `current_state_atomic: Arc<AtomicU32>` the canonical storage for interaction state.
   - Implement `TryFrom<u32>` / `From<u32>` and `From<InteractionState> for u32` for `InteractionState`.
2. **Deprecate `parking_lot::Mutex<InteractionState>`:**
   - Eliminate `pub state: Arc<parking_lot::Mutex<InteractionState>>` from `PipelineAtomics`.
   - Provide helper methods on `PipelineAtomics`:
     - `pub fn state(&self) -> InteractionState` (loads `AtomicU32` and casts to `InteractionState`).
     - `pub fn set_state(&self, s: InteractionState)` (stores `s as u32` in `AtomicU32` and updates `is_assistant_speaking`).
3. **Preserve Audio Callback Performance:**
   - Keep `is_assistant_speaking: Arc<AtomicBool>` updated inside `set_state()` so real-time audio threads retain zero-branch single-bool lock-free reads.

### Sprint 005: Bootstrap Logger Initialization Ordering

- **Review Point:** Bootstrap logs emitted before the logger is initialized are silently dropped (`lib.rs:79-121`)
- **Severity:** 🟠 REAL COST AT SCALE (Lost Diagnostic Logs / Observability Blind Spot)
- **Target File:** [`app/src-tauri/src/lib.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/lib.rs#L81-L122)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** In `lib.rs`, `paths::init()` is called at line 82, after which a background task for manifest caching (`tauri::async_runtime::spawn`) is spawned at line 86. This task uses `log::info!` and `log::warn!`. The logging system (`crate::utils::logging::init(...)`) is only initialized at line 121.
- **Failure Mode:** Any log emitted by early bootstrap tasks or async spawns before line 121 is silently dropped by the uninitialized log subscriber.
- **Verdict:** **Confirmed Defect (🟠 REAL COST AT SCALE)**.

#### 2. Logic Explanation & Engineering Invariants
- Logging must be fully initialized as early as possible in application lifecycle, immediately after paths resolution, before any asynchronous tasks, background workers, or subsystems are spawned.

#### 3. Recommended Actions for Backend Engineer
1. **Reorder Bootstrap Sequence in `lib.rs`:**
   - Move `let log_guard = crate::utils::logging::init(crate::utils::paths::get().logs.clone());` to immediately follow `crate::utils::paths::init(); crate::utils::paths::ensure_dirs().ok();`.
   - Ensure `tauri::async_runtime::spawn` (for manifest fetching) and all subsequent initialization stages occur *after* `logging::init`.

### Sprint 006: Non-Destructive Timestamped Settings Backup on Corruption

- **Review Point:** Corrupt-settings backup is lost on a second corruption (`core/settings.rs:943-950`)
- **Severity:** 🟠 REAL COST AT SCALE (Data Loss / Backup Overwrite)
- **Target File:** [`app/src-tauri/src/core/settings.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/settings.rs#L943-L951)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** When `VoxSettings::load()` detects unparseable content, it constructs a fixed path `let bak = path.with_extension("json.bak");` and renames the file to `bak`.
- **Failure Mode:** On Linux/macOS, `fs::rename` atomically overwrites any existing file at destination. If a corruption occurs again before the user recovers the previous `.bak`, the earlier recoverable backup is permanently destroyed. On Windows, `fs::rename` fails if the destination exists, leaving the corrupt file in place and skipping the backup altogether.
- **Verdict:** **Confirmed Defect (🟠 REAL COST AT SCALE)**.

#### 2. Logic Explanation & Engineering Invariants
- Backups of corrupt user configurations must be append-only / uniquely timestamped so prior recovery snapshots are never clobbered or silently dropped.

#### 3. Recommended Actions for Backend Engineer
1. **Timestamped Unique Backup Paths:**
   - In `VoxSettings::load()`, compute a timestamped backup path:
     ```rust
     let ts = std::time::SystemTime::now()
         .duration_since(std::time::UNIX_EPOCH)
         .map(|d| d.as_secs())
         .unwrap_or(0);
     let bak = path.with_file_name(format!("settings.corrupt.{}.json", ts));
     ```
2. **Copy Before Rename/Replace:**
   - Use `fs::copy` or `fs::rename` to the unique filename, ensuring errors are logged with the specific path.

### Sprint 007: Unification of Personal Memory Collection Taxonomy (SSOT)

- **Review Point:** Dead constants + a third source of truth for the collection→class taxonomy (`core/constants.rs:223,236`)
- **Severity:** 🟠 REAL COST AT SCALE (Dead Code & Multiple SSOTs for Taxonomy)
- **Target File:** [`app/src-tauri/src/core/constants.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/constants.rs#L222-L246)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `core/constants.rs` declares `PM_COLLECTIONS`, `PM_SPECIAL_STATE_COLLECTIONS`, and `PM_SEMANTIC_GRAPH_COLLECTIONS` as static string slices. However, `MemoryCollection` enum (lines 164–220) already defines all collections, their display strings, parsing, priorities, and `collection_type()` mapping.
- **Analysis:** `PM_COLLECTIONS` and `PM_SPECIAL_STATE_COLLECTIONS` have 0 usages outside `constants.rs`. `PM_SEMANTIC_GRAPH_COLLECTIONS` is used in only one file (`stage3_eval.rs:315`). This duplicates taxonomy definitions across an enum and loose string arrays.
- **Verdict:** **Confirmed SSOT Redundancy & Dead Code (🟠 REAL COST AT SCALE)**.

#### 2. Logic Explanation & Engineering Invariants
- `MemoryCollection` enum is the single source of truth for memory collection names, types, priorities, and validation.
- All collection iteration and classification must derive from `MemoryCollection::VARIANTS` or methods on `MemoryCollection`.

#### 3. Recommended Actions for Backend Engineer
1. **Remove Dead Arrays:**
   - Delete `pub const PM_COLLECTIONS` and `pub const PM_SPECIAL_STATE_COLLECTIONS` from `core/constants.rs`.
2. **Provide SSOT Iterators on `MemoryCollection`:**
   - Add `pub const ALL: [MemoryCollection; 6] = [...]` to `MemoryCollection`.
   - Provide helper `pub fn semantic_graph_collections()` or `pub const SEMANTIC_GRAPH` derived from the enum.
3. **Update Callers:**
   - In `services/memory/pipeline/stage3_eval.rs:315`, replace reference to `PM_SEMANTIC_GRAPH_COLLECTIONS` with `MemoryCollection::SEMANTIC_GRAPH`.

### Sprint 008: Race-Free Atomic Settings Persistence

- **Review Point:** `save()` is not safe against concurrent callers sharing one tmp path (`core/settings.rs:971-973`)
- **Severity:** 🟠 REAL COST AT SCALE (File Write Race & Atomic Rename Failure)
- **Target File:** [`app/src-tauri/src/core/settings.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/settings.rs#L961-L977)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `VoxSettings::save()` uses a static temporary path: `let tmp_path = path.with_extension("tmp");`.
- **Failure Mode:** When multiple threads/async tasks commit settings mutations concurrently, both write to the same `settings.tmp` file and race on `fs::rename(tmp_path, path)`. The losing thread fails with `NotFound` (`ENOENT`) because the temporary file was already renamed by the winning thread, or partially writes truncated content.
- **Verdict:** **Confirmed Defect (🟠 REAL COST AT SCALE)**.

#### 2. Logic Explanation & Engineering Invariants
- File system updates must be atomic and race-free.
- Temporary files used for atomic swap must reside on the same filesystem (same directory) and have unique, process- and thread-safe names.

#### 3. Recommended Actions for Backend Engineer
1. **Unique Temp File Construction:**
   - In `VoxSettings::save()`, generate a unique temporary file in the same parent directory:
     ```rust
     let unique_id = std::time::SystemTime::now()
         .duration_since(std::time::UNIX_EPOCH)
         .map(|d| d.as_nanos())
         .unwrap_or(0);
     let tmp_path = path.with_file_name(format!("settings.{}.tmp", unique_id));
     ```
2. **Clean Error Handling & Cleanup:**
   - Write to `tmp_path`, rename to `path`, and clean up `tmp_path` on write errors to avoid orphaned temporary files.

### Sprint 009: App Shell & Bootstrap Latency / Allocation Audit

- **Review Point:** Shell/bootstrap modules are not hot paths — no meaningful latency/alloc issues found
- **Severity:** ⚡ OPTIMIZATION (Performance Audit Baseline)
- **Target Files:** `core/*`, `lib.rs`, `main.rs`, `tray.rs`, `window_*.rs`, `wizard.rs`

#### 1. Feedback Review & Audit
- **Live Code Audit:** Audited boot flow, window customization, tray lifecycle, and state allocation. App initialization is executed once on startup; settings and configuration structures are small (<10 KB). No tight loops, unbuffered I/O on hot paths, or excessive allocations exist in the shell bootstrap.
- **Verdict:** **Audit Passed (No Optimization Action Required)**.

#### 2. Logic Explanation & Invariants
- Application startup time is dominated by local model weight probing / verification (which is already executed asynchronously in background tasks). Shell components remain lean and decoupled from inference pipelines.

### Sprint 010: Top-of-File Import Organization in `core/constants.rs`

- **Review Point:** `core/constants.rs:160` (misplaced `use serde::{Deserialize, Serialize}`)
- **Severity:** 🟡 STYLISTIC (Code Style & Organization)
- **Target File:** [`app/src-tauri/src/core/constants.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/constants.rs#L158-L162)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `use serde::{Deserialize, Serialize};` is declared midway through `constants.rs` on line 160, immediately following a long prompt string constant.
- **Verdict:** **Confirmed Style Issue (🟡 STYLISTIC)**.

#### 2. Recommended Actions for Backend Engineer
- Move `use serde::{Deserialize, Serialize};` to the top of `core/constants.rs` alongside standard imports.

### Sprint 011: Section Header Formatting in `core/constants.rs`

- **Review Point:** `core/constants.rs:68` (misplaced section banner on semicolon line)
- **Severity:** 🟡 STYLISTIC (Formatting & Readability)
- **Target File:** [`app/src-tauri/src/core/constants.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/constants.rs#L67-L71)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** Line 68 has `</memory_context>"; // ─── Transition Speech Assets...` where the comment banner was appended inline to the closing semicolon of `DEFAULT_SYSTEM_PROMPT`.
- **Verdict:** **Confirmed Formatting Flaw (🟡 STYLISTIC)**.

#### 2. Recommended Actions for Backend Engineer
- Split line 68 so the string literal ends cleanly with `";` on its own line and the section comment banner begins on the next line preceding `TRANSITION_MESSAGES_EN`.

### Sprint 012: Consistent Graph Relation Constants in `core/constants.rs`

- **Review Point:** `core/constants.rs:280` (`PM_RELATION_CONFLICTS` vs literal `"CONFLICTS_WITH"`)
- **Severity:** 🟡 STYLISTIC (Inconsistent Matching & Dead Alias)
- **Target File:** [`app/src-tauri/src/core/constants.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/constants.rs#L275-L285)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** In `inverse_edge_for_relation()`, line 280 matches `PM_RELATION_CONFLICTS | "CONFLICTS_WITH" => "conflicts_with"`. All other match arms strictly match their corresponding `PM_RELATION_*` constant.
- **Analysis:** `"CONFLICTS_WITH"` is an unreferenced legacy string alias with 0 callers in the entire repository.
- **Verdict:** **Confirmed Inconsistency (🟡 STYLISTIC)**.

#### 2. Recommended Actions for Backend Engineer
- Remove the hardcoded `"CONFLICTS_WITH"` literal from `inverse_edge_for_relation`, matching purely on `PM_RELATION_CONFLICTS` (or convert the relations to a strongly typed `MemoryRelation` enum).

### Sprint 013: Domain vs Root I/O Error Hierarchy in `core/error.rs`

- **Review Point:** `core/error.rs:29` + `:110` (dual `Io` variant conversion)
- **Severity:** 🟡 STYLISTIC (Error Coercion Hierarchy)
- **Target File:** [`app/src-tauri/src/core/error.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/error.rs#L28-L30)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** Both `VoxError::Io(#[from] std::io::Error)` and `PersistenceError::Io(#[from] std::io::Error)` derive `#[from] std::io::Error`.
- **Analysis:** Within persistence modules returning `Result<T, PersistenceError>`, `?` produces `PersistenceError::Io`. At higher levels returning `Result<T, VoxError>`, `?` produces `VoxError::Io`. While standard `thiserror` behavior, ensuring callers use domain errors preserves contextual stack traces.
- **Verdict:** **Benign / Idiomatic Hierarchy (🟡 STYLISTIC)**.

#### 2. Recommended Actions for Backend Engineer
- Retain domain error tagging in `PersistenceError::Io` for persistence operations to distinguish DB/file I/O from general system I/O.

### Sprint 014: Top-Level Tauri Builder Error Handling

- **Review Point:** `lib.rs:549` (Tauri builder `.expect(...)`)
- **Severity:** 🟡 STYLISTIC (Startup Panic vs Clean Exit)
- **Target File:** [`app/src-tauri/src/lib.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/lib.rs#L548-L551)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `lib.rs:549` uses `.build(tauri::generate_context!()).expect("error while building tauri application")`.
- **Analysis:** This is standard boilerplate in Tauri applications. If Tauri context assembly fails (e.g. invalid `tauri.conf.json` during development), the process panics with the message.
- **Verdict:** **Benign / Standard Tauri Pattern (🟡 STYLISTIC)**.

#### 2. Recommended Actions for Backend Engineer
- Maintain `.expect("error while building tauri application")` or replace with `match ... { Ok(app) => app.run(...), Err(e) => { log::error!("Fatal Tauri initialization error: {e}"); return Err(e.into()); } }` if `run()` is converted to return a `Result`.

### Sprint 015: Derive `Default` on `PinchZoomDisablePlugin`

- **Review Point:** `window_customizer.rs:6-10` (manual `impl Default` for unit struct)
- **Severity:** 🟡 STYLISTIC (Boilerplate Reduction)
- **Target File:** [`app/src-tauri/src/window_customizer.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/window_customizer.rs#L4-L10)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `PinchZoomDisablePlugin` is a unit struct with a manual `impl Default for PinchZoomDisablePlugin { fn default() -> Self { Self } }`.
- **Verdict:** **Confirmed Minor Boilerplate (🟡 STYLISTIC)**.

#### 2. Recommended Actions for Backend Engineer
- Replace manual `impl Default` with `#[derive(Default)]` directly on `pub struct PinchZoomDisablePlugin;`.


### Sprint 016: Safe WebKitGTK Pinch-Zoom Interception & Unsafe Teardown Cleanup

- **Review Point:** `window_customizer.rs:17-63` — correctness/safety of the unsafe GTK gesture teardown (❓ UNSURE / QUESTION)
- **Severity:** ❓ ARCHITECTURAL & SAFETY ADJUDICATION (Linux WebKit Stability)
- **Target File:** [`app/src-tauri/src/window_customizer.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/window_customizer.rs#L17-L65)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** On Linux, `PinchZoomDisablePlugin` uses `web_view.data::<gtk::GestureZoom>("wk-view-zoom-gesture")` with an `unsafe` call to `gobject_ffi::g_signal_handlers_destroy(data.as_ptr().cast())` relying on undocumented WebKitGTK private string symbols. It also installs a safe GTK signal listener `web_view.connect_notify(Some("zoom-level"), ...)` that resets any zoom deviation back to `1.0`.
- **Verdict:** The `unsafe` gesture destructor is brittle across WebKitGTK releases and distributions, whereas the safe `notify::zoom-level` signal listener reliably enforces `zoom_level = 1.0`.

#### 2. User Decision & Architecture Resolution
- **Decision:** Eliminate the fragile `unsafe` WebKitGTK internal gesture teardown and rely on the safe `web_view.connect_notify(Some("zoom-level"), ...)` handler.
- 🛑 **HOTFIX EXECUTION REQUIREMENT:** As adjudicated by the user, **this change must be executed via the `/hotfix` skill** with immediate manual verification on Linux (Wayland & X11) to confirm pinch-to-zoom prevention remains 100% effective with zero regression.

#### 3. Recommended Actions for Backend Engineer
1. **Remove `unsafe` Block:**
   - Remove `destroy_zoom_gesture` and `gobject_ffi::g_signal_handlers_destroy`.
2. **Harden Safe Signal Listener:**
   - Retain `web_view.connect_notify(Some("zoom-level"), |web_view, _| { if (web_view.zoom_level() - 1.0).abs() > 0.001 { web_view.set_zoom_level(1.0); } })`.

---

### Sprint 017: Bidirectional Semantic Graph Edge Inversion

- **Review Point:** `core/constants.rs:276-285` — is `inverse_edge_for_relation` a true two-way inverse? (❓ UNSURE / QUESTION)
- **Severity:** ❓ ARCHITECTURAL ADJUDICATION (Graph Integrity & Taxonomy Symmetry)
- **Target Files:** [`app/src-tauri/src/core/constants.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/constants.rs#L275-L285), [`services/memory/pipeline/stage3_eval.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/pipeline/stage3_eval.rs#L192-L210)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `inverse_edge_for_relation` mapped uppercase labels (`"SUPPORTS"`) to lowercase inverse labels (`"supported_by"`), but fell through to `"related_to"` when passed a lowercase inverse label (`"supported_by"`).
- **Verdict:** Asymmetry creates silent edge corruption if inverse relations are ever queried or evaluated in reverse.

#### 2. User Decision & Architecture Resolution
- **Decision:** Implement **symmetric, bidirectional edge inversion**.

#### 3. Recommended Actions for Backend Engineer
1. **Bidirectional Inversion Function:**
   - Update `inverse_edge_for_relation(relation: &str) -> &'static str`:
     - `"SUPPORTS"` <-> `"supported_by"`
     - `"CONFLICTS"` | `"CONFLICTS_WITH"` <-> `"conflicts_with"`
     - `"SUPERSEDES"` <-> `"superseded_by"`
     - `"SHAPES"` <-> `"shaped_by"`
     - `"DEPENDS_ON"` <-> `"dependency_of"`
     - Default -> `"related_to"`

---

### Sprint 018: Cohesive Telemetry State Encapsulation in `AppState`

- **Review Point:** `core/state.rs:144-228` vs `:231-256` — intentional duplication of telemetry atomics? (❓ UNSURE / QUESTION)
- **Severity:** ❓ ARCHITECTURAL ADJUDICATION (State Consolidation & Lifecycle Architecture)
- **Target File:** [`app/src-tauri/src/core/state.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/state.rs#L164-L265)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** 20 telemetry `Arc<AtomicU32>` and `Arc<AtomicBool>` fields were declared flat in `AppState` and mirrored identically in `AppStateTelemetryHandles`, requiring a 20-line manual destructuring and clone in `AppState::new`.
- **Verdict:** Duplication and boilerplate during bootstrap.

#### 2. User Decision & Architecture Resolution
- **Decision:** Encapsulate all telemetry handles inside `telemetry: Arc<TelemetryState>` within `AppState`.

#### 3. Recommended Actions for Backend Engineer
1. **Consolidate Telemetry Struct:**
   - Rename `AppStateTelemetryHandles` -> `TelemetryState` and declare `pub telemetry: Arc<TelemetryState>` inside `AppState`.
2. **Eliminate 20 Flat Duplicate Fields:**
   - Remove flat `latest_*` atomics from top-level `AppState`. Access them uniformly via `state.telemetry.latest_*`.
3. **Simplify Constructor:**
   - `AppState::new` takes `telemetry: Arc<TelemetryState>` directly with zero field-by-field cloning.

---

## Module 02: IPC Layer (`sprint-02.md`)

### Sprint 019: Unbounded In-App PCM Audio Payload Rejection

- **Review Point:** `voices.rs:123` — Unbounded `pcm_f32: Vec<f32>` from untrusted IPC -> OOM
- **Severity:** 🔴 WILL BREAK (C100 — Direct OOM Vulnerability on 8 GB Constraint)
- **Target File:** [`app/src-tauri/src/ipc/voices.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/voices.rs#L120-L140)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `add_voice_from_recording` accepts `pcm_f32: Vec<f32>` from the frontend IPC. It only verifies `duration < 1.0` and has no upper bound. A malicious or malfunctioning frontend can pass hundreds of millions of samples, causing multi-gigabyte heap allocations and instant OOM crash.
- **Verdict:** **Confirmed Vulnerability (🔴 WILL BREAK)**.

#### 2. Recommended Actions for Backend Engineer
1. **Define Max Audio Buffer Constant:**
   - Add `const MAX_VOICE_RECORDING_SAMPLES: usize = 1_600_000; // ~100s at 16kHz (~6.4 MB)`
2. **Enforce Upper Bound Check:**
   - In `add_voice_from_recording`, reject payloads immediately if `pcm_f32.len() > MAX_VOICE_RECORDING_SAMPLES`:
     ```rust
     if pcm_f32.len() > MAX_VOICE_RECORDING_SAMPLES {
         return Err(format!("Voice recording payload exceeds maximum allowed size ({} samples).", MAX_VOICE_RECORDING_SAMPLES));
     }
     ```

---

### Sprint 020: Path Traversal Vulnerability in Test Clip Path Resolver

- **Review Point:** `pipeline/test_clip.rs:59-83` — Path traversal in `resolve_clip_path` (arbitrary file read)
- **Severity:** 🔴 WILL BREAK (C100 — Arbitrary Filesystem Read / Information Leak)
- **Target File:** [`app/src-tauri/src/ipc/pipeline/test_clip.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/pipeline/test_clip.rs#L58-L84)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `resolve_clip_path(clip_id: &str)` directly appends `clip_id` or `format!("{}.wav", clip_id)` to candidate directories via `dir.join(&filename)`. A string containing `..` or leading path separators (`../../../../etc/passwd`) escapes candidate directories and allows arbitrary file reads into the STT engine.
- **Verdict:** **Confirmed Vulnerability (🔴 WILL BREAK)**.

#### 2. Recommended Actions for Backend Engineer
1. **Sanitize `clip_id` Input:**
   - Reject any `clip_id` containing path separators (`/`, `\`) or directory traversal components (`..`).
2. **Canonicalization & Boundary Verification:**
   - In `resolve_clip_path`:
     ```rust
     if clip_id.contains('/') || clip_id.contains('\') || clip_id.contains("..") {
         return Err("Invalid clip ID: directory traversal not permitted".into());
     }
     let filename = if clip_id.ends_with(".wav") { clip_id.to_string() } else { format!("{clip_id}.wav") };
     for dir in &candidate_dirs {
         let candidate = dir.join(&filename);
         if let Ok(canon) = candidate.canonicalize() {
             if let Ok(canon_dir) = dir.canonicalize() {
                 if canon.starts_with(&canon_dir) && canon.exists() {
                     return Ok(canon);
                 }
             }
         }
     }
     ```

---

### Sprint 021: Zero Sample Rate Guard in WAV Resampler

- **Review Point:** `pipeline/test_clip.rs:13` — `source_rate == 0` -> `inf` -> `usize::MAX` -> panic/OOM
- **Severity:** 🔴 WILL BREAK (C80 — Arithmetic Overflow Panic / OOM on Malformed Header)
- **Target File:** [`app/src-tauri/src/ipc/pipeline/test_clip.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/pipeline/test_clip.rs#L8-L56)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `resample_to_16k` calculates `let ratio = 16000.0 / source_rate as f64;`. If `source_rate == 0` (e.g. malformed or zeroed WAV header), `ratio` becomes `f64::INFINITY`, and `let target_len = (samples.len() as f64 * ratio).round() as usize;` saturates to `usize::MAX`, triggering a panic on `Vec::with_capacity(target_len)`.
- **Verdict:** **Confirmed Vulnerability (🔴 WILL BREAK)**.

#### 2. Recommended Actions for Backend Engineer
1. **Validate Sample Rate at Decode Entry:**
   - In `decode_wav_to_mono_f32`, immediately check `spec.sample_rate`:
     ```rust
     if spec.sample_rate == 0 {
         return Err(format!("Invalid WAV '{}': sample rate cannot be 0", path.display()));
     }
     ```
2. **Defensive Guard in `resample_to_16k`:**
   - Return empty or error if `source_rate == 0`.

---

### Sprint 022: Offload Blocking CPAL Audio Device Enumeration to Worker Threads

- **Review Point:** `audio.rs:13,42` — Blocking cpal enumeration on the async command executor
- **Severity:** 🟠 REAL COST AT SCALE (C80 — UI Freeze & Audio Driver Stalls under Sub-200ms Budget)
- **Target File:** [`app/src-tauri/src/ipc/audio.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/audio.rs#L12-L68)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `list_input_devices` and `list_output_devices` are `async fn` IPC endpoints, but invoke synchronous CPAL host/device/config queries (`host.input_devices()`, `device.supported_input_configs()`) directly on the async runtime thread. Under Linux ALSA/PulseAudio/PipeWire, querying audio driver formats can block for several hundred milliseconds, stalling Tauri's async event reactor.
- **Verdict:** **Confirmed Latency Hazard (🟠 REAL COST AT SCALE)**.

#### 2. Recommended Actions for Backend Engineer
1. **Wrap Enumeration in `spawn_blocking`:**
   - In `list_input_devices` and `list_output_devices`, move CPAL device iteration into `tokio::task::spawn_blocking(move || { ... }).await.map_err(...)`.

---

### Sprint 023: Audio File Path Validation in Voice Cloning

- **Review Point:** `voices.rs:71` — Arbitrary `file_path` from IPC read via `add_voice_from_file`
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Arbitrary Local File Ingestion & Symlink Traversal)
- **Target File:** [`app/src-tauri/src/ipc/voices.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/voices.rs#L70-L88)

#### 1. Feedback Review & Root Cause
- **Live Code Audit:** `add_voice_from_file` accepts `file_path: String` from frontend IPC and forwards it directly to `convert_and_validate_audio`. There is no check verifying that `file_path` points to a regular file (versus a device, pipe, or dangling symlink) before handing it to the audio processing pipeline.
- **Verdict:** **Confirmed Boundary Gap (🟠 REAL COST AT SCALE)**.

#### 2. Recommended Actions for Backend Engineer
1. **Enforce Regular File Check & Path Canonicalization:**
   - In `add_voice_from_file`, verify path exists, is a regular file (not directory/device), and has a valid audio extension (`.wav`, `.mp3`, `.m4a`, `.ogg`, `.flac`) before spawning the processing task:
     ```rust
     let path = std::path::Path::new(&file_path);
     if !path.is_file() {
         return Err("Selected path is not a valid regular file".to_string());
     }
     ```

### Sprint 024: Range & Bounds Enforcement on Untrusted Numeric Settings

- **Review Point:** `settings/mutation.rs:456,459,480,546,552,…` — Silent integer truncation on untrusted numeric settings
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Silent Configuration Corruption / Runaway Allocations)
- **Target File:** [`app/src-tauri/src/ipc/settings/mutation.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/settings/mutation.rs#L450-L565)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `apply_setting_mutation` casts untrusted `value.as_u64()` / `as_i64()` using raw `as u32` / `as i32` without bounds checking for several numeric settings (`max_output_tokens`, `threads`, `voice_index`, `quality_steps`). Values like `0` or `u64::MAX` are accepted without error, silently producing zeroed or wrapped configuration values.
- **Why it's this way:** Missing defensive validation bounds in early prototype implementation.

#### 2. Recommended Actions for Backend Engineer
1. **Enforce Strict Bounds Checking in `apply_setting_mutation`:**
   - `("llm", "max_output_tokens")`: Validate `1..=32768`.
   - `("llm", "threads")`: Validate `1..=64`.
   - `("tts", "quality_steps")`: Validate `1..=20`.
   - `("tts", "voice_index")`: Validate `0..=1000`.
   - Return descriptive `Err(format!("{key} must be between X and Y"))` on invalid inputs.

---

### Sprint 025: Mutual Exclusion Guard for Remote Server Setup

- **Review Point:** `settings/health.rs:509-537` — `setup_remote_server` has no mutual-exclusion guard
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Race Condition / Overlapping SSH Processes)
- **Target File:** [`app/src-tauri/src/ipc/settings/health.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/settings/health.rs#L507-L537)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `setup_remote_server` immediately spawns `run_remote_ssh_task` into `tauri::async_runtime::spawn` without checking any in-progress state flag (unlike `start_model_setup` which gates on `state.setup_running`). Repeated frontend calls spawn concurrent SSH sessions competing on the same remote host and emitting conflicting progress events.
- **Why it's this way:** Omission of a concurrency guard during remote setup handler implementation.

#### 2. Recommended Actions for Backend Engineer
1. **Introduce Atomic Concurrency Guard:**
   - Add `remote_setup_running: Arc<AtomicBool>` in `AppState` or check and set an atomic guard at the entry of `setup_remote_server`.
   - If already running, return `Err("Remote setup is already in progress".to_string())`.
   - Ensure the flag is cleared on task completion, failure, or cancellation.

---

### Sprint 026: Non-Blocking Embedding Generation in Fact Mutation

- **Review Point:** `memory/mutations.rs:25-30` — `edit_fact_content` loads the full embedder model per edit
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Memory Footprint & Async Reactor Latency)
- **Target File:** [`app/src-tauri/src/ipc/memory/mutations.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/memory/mutations.rs#L20-L33)

#### 1. Feedback Review & Root Cause
- **Verdict:** ⚠️ **Partial / Known Tradeoff** (Confidence: 90%)
- **What the code actually does:** `edit_fact_content` invokes `ensure_embedder_loaded(true)` followed by `generate_embedding(trimmed)`. If the embedder is already resident, `ensure_embedder_loaded` returns `Ok(true)` immediately via a fast `RwLock::read` check with zero disk I/O. However, calling ONNX model initialization and inference directly on the async IPC task blocks the Tokio executor during cold loads.
- **Why it's this way:** Vector similarity search requires up-to-date embeddings when fact text changes.

#### 2. Recommended Actions for Backend Engineer
1. **Check Subsystem State & Offload Inference:**
   - Verify `state.settings.read().memory.enabled` before triggering embedding update.
   - Offload synchronous ONNX embedding inference to `tokio::task::spawn_blocking` to preserve sub-200ms latency responsiveness across the IPC layer.

---

### Sprint 027: Payload Bound for Memory Queue Item Retries

- **Review Point:** `memory/ingestion.rs:201-233` — Unbounded `item_ids: Vec<i64>` from frontend
- **Severity:** 🟠 REAL COST AT SCALE (C60 — Unbounded SQL Query Construction & Large Allocation)
- **Target File:** [`app/src-tauri/src/ipc/memory/ingestion.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/memory/ingestion.rs#L200-L235)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `retry_failed_queue_items` accepts `item_ids: Vec<i64>` from untrusted IPC and dynamically constructs a SQL string with `IN (?,?,...)` placeholders without bounding `item_ids.len()`. A massive vector payload generates large string buffers and Turso value arrays.
- **Why it's this way:** Missing upper-bound check on batch collection size.

#### 2. Recommended Actions for Backend Engineer
1. **Enforce Maximum Batch Size:**
   - In `retry_failed_queue_items`, reject payloads where `item_ids.len() > 1000`:
     ```rust
     if item_ids.len() > 1000 {
         return Err("Too many items in retry batch. Maximum allowed is 1000.".to_string());
     }
     ```

---

### Sprint 028: Realistic Reachability Verification for EdgeTTS Health

- **Review Point:** `settings/health.rs:166` — `check_tts_provider_health` EdgeTTS arm always returns `Ok(true)`
- **Severity:** 🟠 REAL COST AT SCALE (C60 — Misleading Status / Offline Silence)
- **Target File:** [`app/src-tauri/src/ipc/settings/health.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/settings/health.rs#L150-L168)

#### 1. Feedback Review & Root Cause
- **Verdict:** ❌ **False Positive (Known Tradeoff) with Recommended Enhancement** (Confidence: 95%)
- **What the code actually does:** `check_tts_provider_health` returns `Ok(true)` for `TtsProviderConfig::EdgeTts` because EdgeTTS requires no local model weights or daemon process. However, if the machine is offline, the UI indicates TTS is healthy even though synthesis requests will fail.
- **Why it's this way:** Intent was to indicate zero-setup readiness since EdgeTTS does not require downloading GBs of model weights.

#### 2. Recommended Actions for Backend Engineer
1. **Lightweight Connectivity Probe:**
   - Add a lightweight reachability probe (HTTP HEAD request with a short 2-second timeout to the EdgeTTS endpoint or network check) to report true availability without blocking the UI.

### Sprint 029: Concurrency Guard for Background Optional Model Downloads

- **Review Point:** `setup.rs:336` — `download_optional_model` has no `setup_running` guard
- **Severity:** 🟠 REAL COST AT SCALE (C60 — Concurrent Download Collisions & Race Conditions)
- **Target File:** [`app/src-tauri/src/ipc/setup.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/setup.rs#L334-L374)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `download_optional_model` spawns a background download task without verifying or acquiring `state.setup_running`. If the user initiates base setup while an optional download is active, or clicks optional download multiple times, concurrent `ModelManager` tasks race on temporary destination files.
- **Why it's this way:** Background downloads were added as an independent endpoint without shared lifecycle locking.

#### 2. Recommended Actions for Backend Engineer
1. **Acquire Setup Guard:**
   - Check and set `state.setup_running` or acquire a model group lock before spawning the download task.
   - Return `Err("Model download or setup is already in progress".to_string())` if locked.

---

### Sprint 030: Memory Bounding on Ephemeral Transcript History Text

- **Review Point:** `history.rs:17-36` — Unbounded `text` length into `transcript_history`
- **Severity:** 🟠 REAL COST AT SCALE (C60 — Memory Leak & Unbounded Heap Growth)
- **Target File:** [`app/src-tauri/src/ipc/history.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/history.rs#L15-L36)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `commit_session_to_history` caps the *count* of entries via `settings.history.tray_history_limit`, but does not bound the byte size of individual `text` payloads. Large strings pushed repeatedly consume unbounded RAM.
- **Why it's this way:** Early code assumed natural turn lengths from the audio pipeline without untrusted IPC defense.

#### 2. Recommended Actions for Backend Engineer
1. **Cap Individual String Length:**
   - In `commit_session_to_history`, enforce `const MAX_HISTORY_TEXT_CHARS: usize = 10_000;`. Truncate or reject text exceeding this threshold.

---

### Sprint 031: Filter Scoping & Pagination for Memory Graph Edges

- **Review Point:** `memory/graph.rs:174-179` — Full edge set fetched every topology request
- **Severity:** ⚡ OPTIMIZATION (C60 — Unbounded Query Growth on Memory Graph Polling)
- **Target File:** [`app/src-tauri/src/ipc/memory/graph.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/memory/graph.rs#L173-L191)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** `get_memory_graph_topology` executes `SELECT id, from_id, to_id, relation, created_at FROM memory_relations ORDER BY id ASC` fetching all relation rows regardless of the active node filter.
- **Why it's this way:** Simplified graph payload construction in early development.

#### 2. Recommended Actions for Backend Engineer
1. **Scope Edges by Active Node IDs:**
   - When a collection filter is supplied, filter relations where `from_id IN (...) AND to_id IN (...)` (or join with filtered facts) to avoid unbounded payloads.

---

### Sprint 032: Zero-Copy Return for Session History Commit

- **Review Point:** `history.rs:12,34` — Full `Vec` clone on every history read/commit
- **Severity:** ⚡ OPTIMIZATION (C60 — Redundant Full-History Clones on Hot Turn Path)
- **Target File:** [`app/src-tauri/src/ipc/history.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/history.rs#L15-L36)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** `commit_session_to_history` clones the entire `VecDeque<String>` to return `Vec<String>` on every turn commit, even when the caller only needs write confirmation.
- **Why it's this way:** Combined mutation + read pattern inherited from early IPC prototypes.

#### 2. Recommended Actions for Backend Engineer
1. **Return Unit Acknowledgement:**
   - Change `commit_session_to_history` signature to return `Result<(), String>` instead of `Result<Vec<String>, String>`. Callers that require the full history can read it via `get_transcript_history`.

---

### Sprint 033: Audio Device Format Query Caching

- **Review Point:** `audio.rs:18-37,47-66` — Re-enumerate all device configs per call, no cache
- **Severity:** ⚡ OPTIMIZATION (C60 — Redundant OS Audio Hardware Iteration)
- **Target File:** [`app/src-tauri/src/ipc/audio.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/audio.rs#L18-L67)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** `list_input_devices` and `list_output_devices` re-probe hardware device formats via `supported_input_configs()` / `supported_output_configs()` on every settings view open.
- **Why it's this way:** Stateless enumeration without caching.

#### 2. Recommended Actions for Backend Engineer
1. **Add Short TTL Caching / Debounce:**
   - Cache device list with a short TTL (e.g. 5–10s) or refresh only when settings modal mounts.

### Sprint 034: Zero-Copy Borrow for Audio Resampling

- **Review Point:** `pipeline/test_clip.rs:11` — Needless `to_vec` on already-16k audio
- **Severity:** ⚡ OPTIMIZATION (C60 — Redundant Buffer Allocation on 16kHz Test Clips)
- **Target File:** [`app/src-tauri/src/ipc/pipeline/test_clip.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/pipeline/test_clip.rs#L8-L25)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** `resample_to_16k` clones `samples.to_vec()` when `source_rate == 16000`, copying the entire vector even when no resampling is necessary.
- **Why it's this way:** Simplified return type `Vec<f32>` instead of `Cow<'a, [f32]>`.

#### 2. Recommended Actions for Backend Engineer
1. **Adopt Copy-on-Write / Sliced Processing:**
   - Return `std::borrow::Cow<'a, [f32]>` or pass slices directly to STT ingestion when `source_rate == 16000`.

---

### Sprint 035: Domain-Segmented Setting Mutation Dispatch

- **Review Point:** `settings/mutation.rs:325-771` — Giant flat `match` in `apply_setting_mutation`
- **Severity:** 🟡 STYLISTIC (C60 — Maintainability & Auditability)
- **Target File:** [`app/src-tauri/src/ipc/settings/mutation.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/settings/mutation.rs#L325-L771)

#### 1. Feedback Review & Root Cause
- **Verdict:** ❌ **False Positive (Deliberate Design) with Maintainability Refactor** (Confidence: 95%)
- **What the code actually does:** `apply_setting_mutation` uses a single comprehensive `match (domain, key)` pattern. It is correct, strongly typed, and covers all configuration keys, but spans ~440 lines.
- **Why it's this way:** Single centralized mutation entry point.

#### 2. Recommended Actions for Backend Engineer
1. **Decompose by Domain:**
   - Split into domain helper functions (`apply_appearance_mutation`, `apply_audio_mutation`, `apply_llm_mutation`, `apply_tts_mutation`, `apply_memory_mutation`, etc.) for cleaner auditing and maintainability.

---

### Sprint 036: RAII Transaction Guards for Memory Graph Mutations

- **Review Point:** `memory/mutations.rs & conflicts.rs` — Manual `BEGIN/COMMIT/ROLLBACK` string statements
- **Severity:** 🟡 STYLISTIC (C60 — Transaction Robustness & Connection Cleanliness)
- **Target Files:** [`app/src-tauri/src/ipc/memory/conflicts.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/memory/conflicts.rs#L85-L128), [`app/src-tauri/src/ipc/memory/mutations.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/memory/mutations.rs#L48-L100)

#### 1. Feedback Review & Root Cause
- **Verdict:** ❌ **False Positive (Deliberate Design) with Robustness Enhancement** (Confidence: 90%)
- **What the code actually does:** Statements are wrapped with explicit rollback blocks (`if let Err(err) = result { conn.execute("ROLLBACK;", ...); }`).
- **Why it's this way:** Turso LibSQL async driver transactions executed via raw statements.

#### 2. Recommended Actions for Backend Engineer
1. **Enforce Transaction Helper / RAII:**
   - Encapsulate transaction execution inside a scoped closure/helper that guarantees automatic rollback on drop or unexpected error paths.

---

### Sprint 037: SSH Connection Parameter Validation & Trust Boundary

- **Review Point:** `settings/health.rs:361-505` (`run_remote_ssh_task`) — Intended trust boundary for SSH arguments (❓ UNSURE / QUESTION)
- **Severity:** ❓ ARCHITECTURAL & SECURITY ADJUDICATION
- **Target File:** [`app/src-tauri/src/ipc/settings/health.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/settings/health.rs#L360-L400)

#### 1. Feedback Review & Root Cause
- **Verdict:** ⚠️ **Partial / Security Hardening Opportunity** (Confidence: 100%)
- **What the code actually does:** `run_remote_ssh_task` passes `identity_key_path`, `connection_string`, and port directly as process args to `Command::new("ssh")`. While safe from shell injection (piped args, not `sh -c`), invalid or non-existent key paths fail without user-friendly feedback.
- **Why it's this way:** Assumed UI supplies valid settings.

#### 2. User Decision & Architecture Resolution
- **Decision:** Validate SSH identity key paths and sanitize connection arguments before spawning the SSH child process.

#### 3. Recommended Actions for Backend Engineer
1. **Validate Identity Key File:**
   - If `identity_key_path` is provided, verify `Path::new(path).is_file()`; return an error if the key file is missing or unreadable.
2. **Sanitize Host & Port:**
   - Validate `ssh_port > 0`; ensure `connection_string` does not contain control characters or disallowed flags.

---

### Sprint 038: State Machine Cleanup on Test Clip Cancellation and Failure

- **Review Point:** `pipeline/test_clip.rs:96-99` — Engine owner/engaged state restoration after test clip (❓ UNSURE / QUESTION)
- **Severity:** ❓ ARCHITECTURAL ADJUDICATION (Pipeline State Machine Safety)
- **Target File:** [`app/src-tauri/src/ipc/pipeline/test_clip.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/pipeline/test_clip.rs#L85-L144)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug / State Machine Gap** (Confidence: 100%)
- **What the code actually does:** `test_clip` sets `owner = InteractionOwner::Assistant` and `is_engaged = true`. `test_clip_cancel` clears `is_engaged` and sets `cancel_flag`, but leaves `owner` set to `Assistant`, preventing subsequent PTT or passive interactions from acquiring ownership.
- **Why it's this way:** Omission of owner release on cancellation path.

#### 2. User Decision & Architecture Resolution
- **Decision:** Explicitly reset `state.owner` to `InteractionOwner::None` upon test clip cancellation and error exit.

#### 3. Recommended Actions for Backend Engineer
1. **Reset Owner on Cancel & Error:**
   - In `test_clip_cancel`, store `InteractionOwner::None as u32` into `state.owner`.
   - On injection failure inside `test_clip`, reset `state.owner` and `state.pipeline.is_engaged = false`.

---

---

### Sprint 039: Health Check Reachability for EdgeTTS Provider

- **Review Point:** `settings/health.rs:166` (`check_tts_provider_health` EdgeTTS arm always returns `Ok(true)`)
- **Severity:** ❓ UNSURE / QUESTION (Deliberate Fallback vs Live DNS Probe)
- **Target File:** [`app/src-tauri/src/ipc/settings/health.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/ipc/settings/health.rs#L160-L175)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Inaccuracy** (Confidence: 95%)
- **What the code actually does:** `check_tts_provider_health` for EdgeTTS returns a hardcoded `Ok(true)` without executing a live network probe.
- **Why it's this way:** Assumed cloud endpoint availability without requiring local API keys.

#### 2. Recommended Actions for Backend Engineer
1. **Lightweight Network Ping:**
   - Implement a lightweight TCP/DNS reachability check against the EdgeTTS endpoint before returning health status.

## Module 03: Persistence Subsystem (`sprint-03.md`)

### Sprint 040: Turn Persistence Event Wiring & Emission

- **Review Point:** `worker.rs:182-232` — `TurnCompleted` is never emitted, so turns are never persisted
- **Severity:** 🔴 WILL BREAK (C100 — Core Conversation History Persistence Data-Loss)
- **Target Files:** [`app/src-tauri/src/persistence/worker.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/worker.rs#L182-L232), [`app/src-tauri/src/services/pipeline/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `PersistenceEvent::TurnCompleted` is the sole writer of the `turns` table (`worker.rs:205`). However, there are zero emit sites for `TurnCompleted` anywhere in the codebase. As a result, `turn_count` remains 0 and `SessionEnded` deletes empty sessions (`DELETE FROM sessions WHERE turn_count = 0`), leaving the history UI permanently empty.
- **Why it's this way:** The event definition existed in `events.rs` and `worker.rs`, but was not wired into the pipeline completion dispatcher during the pipeline modular refactor.

#### 2. Recommended Actions for Backend Engineer
1. **Emit `TurnCompleted` on Pipeline Turn Completion:**
   - In the pipeline post-playback / stream-drain completion handler (e.g. `services/pipeline/modular/context.rs` / `realtime/session.rs`), emit:
     ```rust
     let _ = tx.send(PersistenceEvent::TurnCompleted {
         conversation_id,
         turn_id,
         user_text,
         assistant_text,
         stt_latency_ms,
         ttft_ms,
     });
     ```
2. **Add Integration Test:**
   - Author a test asserting that sending `TurnCompleted` records a row in `turns` and increments `sessions.turn_count`.

---

### Sprint 041: Memory Worker Lifecycle & Event Producer Wiring

- **Review Point:** `memory_worker.rs` — Memory worker event producers missing (`SessionEnd`, `PersonalFactsReady`, `PipelineIdle`, `PipelineActive`, `ActiveSessionChanged`)
- **Severity:** 🔴 WILL BREAK (C100 — Background Memory Consolidation & Resource Management Dormant)
- **Target Files:** [`app/src-tauri/src/persistence/memory_worker.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/memory_worker.rs#L150-L195), [`app/src-tauri/src/services/pipeline/router.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/router.rs)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `MemoryWorkerEvent` variants (`SessionEnd`, `PersonalFactsReady`, `PipelineIdle`, `PipelineActive`, `ActiveSessionChanged`) are handled inside `memory_worker.rs`, but only `Shutdown` is ever sent from `lib.rs`. `session_end_consolidation` is never called, and `state.is_idle` remains `true` permanently, causing background sweeps to run during active speech while preventing ONNX model unloads.
- **Why it's this way:** Event producers in the central router/pipeline were omitted during decoupling.

#### 2. Recommended Actions for Backend Engineer
1. **Wire Pipeline Event Producers:**
   - In `services/pipeline/router.rs` and session lifecycle handlers, dispatch:
     - `MemoryWorkerEvent::SessionEnd { session_id, summary }` on session termination.
     - `MemoryWorkerEvent::PipelineActive` on VAD speech start / turn start.
     - `MemoryWorkerEvent::PipelineIdle` when audio stream returns to idle.
     - `MemoryWorkerEvent::ActiveSessionChanged { session_id }` on new session initialization.

---

### Sprint 042: Harmonize Personal Memory Queue Status Constants

- **Review Point:** `worker.rs:270` & `mutations.rs:85` — `pending` and `staged` personal-memory-queue statuses are orphaned -> silent memory loss
- **Severity:** 🔴 WILL BREAK (C100 — Silent Memory Loss on Paused / Resumed Pipelines)
- **Target Files:** [`app/src-tauri/src/persistence/mutations.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/mutations.rs#L83-L89), [`app/src-tauri/src/persistence/worker.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/worker.rs#L265-L275)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `mutations.rs:85` sets `status = 'pending'`, and filters `WHERE (status = 'staged' OR status = 'paused')`. However, the four-stage memory pipeline strictly claims `staged_pending` (`core/constants.rs:294`). Queue items updated to `pending` are never claimed by Stage 1 (`stage1_dedup.rs`), permanently losing paused memory facts.
- **Why it's this way:** Legacy status token names (`pending`, `staged`) were not updated when the 4-stage pipeline standard (`staged_pending`) was introduced.

#### 2. Recommended Actions for Backend Engineer
1. **Align Queue Status Updates with SSOT Constants:**
   - Update `mutations.rs:84-88` and `worker.rs:270` to use `PM_QUEUE_STATUS_STAGED_PENDING`:
     ```rust
     conn.execute(
         "UPDATE personal_memory_queue 
          SET status = 'staged_pending' 
          WHERE session_id = ? AND (status = 'staged_pending' OR status = 'paused')",
         (session_id.to_string(),),
     ).await?;
     ```

---

### Sprint 043: Atomic Transaction Enclosure for `supersede_user_fact`

- **Review Point:** `mutations.rs:115-173` — `supersede_user_fact` is not atomic (partial-commit leaves dangling fact/vector/relation)
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Corrupted Knowledge Graph & Orphaned Facts)
- **Target File:** [`app/src-tauri/src/persistence/mutations.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/mutations.rs#L115-L173)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `supersede_user_fact` runs 4 consecutive statements without transaction scoping. An error during vector insertion or relation creation leaves a new active fact with no vector embedding and without marking the prior fact as superseded, corrupting the semantic graph.
- **Why it's this way:** Multi-statement mutation lacked explicit `BEGIN / COMMIT / ROLLBACK` framing.

#### 2. Recommended Actions for Backend Engineer
1. **Wrap in Transaction:**
   - Enclose lines 126–171 inside `conn.execute("BEGIN TRANSACTION;", ()).await?;` and `conn.execute("COMMIT;", ()).await?;` with automatic rollback on error.

---

### Sprint 044: True Read-Only Database Connection Mode

- **Review Point:** `db.rs:52-54` — `VoxDb::open_readonly` opens a fully *writable* connection
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Lock Contention & Transient Writer Connection Overhead)
- **Target File:** [`app/src-tauri/src/persistence/db.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/db.rs#L50-L55)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug / Design Discrepancy** (Confidence: 100%)
- **What the code actually does:** `VoxDb::open_readonly` directly calls `Self::open(path).await`, opening a full read-write connection and repeatedly executing `PRAGMA journal_mode = WAL;`. This adds overhead and can create transient lock contention with background writers on an 8 GB budget.
- **Why it's this way:** Placeholder helper implementation.

#### 2. Recommended Actions for Backend Engineer
1. **Implement Explicit Read-Only Connection:**
   - Configure read-only connection params without re-issuing writer pragmas (`journal_mode=WAL`), or document/standardize connection pooling across reader paths.

### Sprint 045: Non-Blocking Realtime Dispatch on Persistence Channels

- **Review Point:** `worker.rs:21` & `memory_worker.rs:49` — Bounded producer channels can block the realtime pipeline
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Pipeline Jitter & Sub-200ms Latency Budget Risk)
- **Target Files:** [`app/src-tauri/src/persistence/worker.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/worker.rs#L15-L35), [`app/src-tauri/src/persistence/memory_worker.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/memory_worker.rs#L40-L60)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latency Hazard** (Confidence: 90%)
- **What the code actually does:** `spawn_persistence_worker` uses `bounded(128)` and `spawn_memory_worker` uses `bounded(32)`. If a background worker is blocked on a SQLite checkpoint or busy timeout, callers invoking `tx.send(...)` from audio or realtime pipeline threads will block synchronously, violating the sub-200ms latency requirement.
- **Why it's this way:** Standard crossbeam bounded channel instantiation.

#### 2. Recommended Actions for Backend Engineer
1. **Use Non-Blocking `try_send` on Realtime Paths:**
   - On realtime pipeline threads, replace blocking `tx.send(ev)` with `tx.try_send(ev)` and log a warning if the channel is full.

---

### Sprint 046: Dead Code Removal for `update_preview_wav`

- **Review Point:** `voices.rs:126-133` — `update_preview_wav` is dead code
- **Severity:** 🟠 REAL COST AT SCALE / DEAD CODE (C100 — Unused Function Surface)
- **Target File:** [`app/src-tauri/src/persistence/voices.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/voices.rs#L125-L135)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Code** (Confidence: 100%)
- **What the code actually does:** `update_preview_wav` has zero callers across the entire codebase.
- **Why it's this way:** Leftover persistence helper from earlier voice manager revisions.

#### 2. Recommended Actions for Backend Engineer
1. **Remove Dead Helper:**
   - Delete `pub async fn update_preview_wav` from `persistence/voices.rs`.

---

### Sprint 047: Dimension & Length Validation in `decode_f32_blob`

- **Review Point:** `mod.rs:44-49` — `decode_f32_blob` silently truncates mis-sized blobs
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Silent Data Corruption / Zero-Vector Generation)
- **Target File:** [`app/src-tauri/src/persistence/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/mod.rs#L43-L50)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `decode_f32_blob` relies on `chunks_exact(4)` and `unwrap_or_default()` without verifying that `bytes.len() % 4 == 0`. Misaligned or corrupted vector blobs silently produce truncated vectors or zero floats.
- **Why it's this way:** Incomplete error handling in primitive byte serializer.

#### 2. Recommended Actions for Backend Engineer
1. **Validate Byte Alignment:**
   - Check `bytes.len() % 4 == 0` and log or return a validation error on misaligned byte slices instead of silently defaulting to `0.0`.

---

### Sprint 048: Vector Blob Re-use in KNN Search

- **Review Point:** `queries.rs:175-177,244-246,315-317` — Re-encode `query_embedding` inside inner loop
- **Severity:** ⚡ OPTIMIZATION (C60 — Redundant Vector Cloning in Vector Retrieval)
- **Target File:** [`app/src-tauri/src/persistence/queries.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/queries.rs#L170-L195)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** `vec_knn_search` clones `query_blob` multiple times for each parameter slot in the UNION query.
- **Why it's this way:** Simplified query parameter assembly.

#### 2. Recommended Actions for Backend Engineer
1. **Avoid Redundant Clones:**
   - Assemble `params` using references or pass single pre-allocated blob buffers into SQLite bindings.

---

### Sprint 049: Consolidated Session Cleanup Query

- **Review Point:** `worker.rs:164-169` — `SessionEnded` runs two queries when one conditional update works
- **Severity:** ⚡ OPTIMIZATION (C60 — Unnecessary WAL Writes on Empty Sessions)
- **Target File:** [`app/src-tauri/src/persistence/worker.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/worker.rs#L164-L181)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** `SessionEnded` executes an `UPDATE` on `sessions` followed immediately by a `DELETE FROM sessions WHERE id = ? AND turn_count = 0`. For empty sessions, the initial `UPDATE` is unnecessary disk/WAL I/O.
- **Why it's this way:** Sequential processing without short-circuiting.

#### 2. Recommended Actions for Backend Engineer
1. **Conditional Update / Early Cleanup:**
   - Execute the cleanup check first, or perform conditional `UPDATE sessions SET ended_at = ? WHERE id = ? AND turn_count > 0`.

### Sprint 050: Batched DDL Schema Migration Execution

- **Review Point:** `schema.rs:43-162` — Redundant column existence checks per table
- **Severity:** ⚡ OPTIMIZATION (C60 — Startup Initialization Round-Trips)
- **Target File:** [`app/src-tauri/src/persistence/schema.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/schema.rs#L40-L115)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** `init_schema` iterates over an array of `CREATE TABLE / INDEX IF NOT EXISTS` statements, executing them sequentially over individual async calls.
- **Why it's this way:** Declarative startup schema definitions.

#### 2. Recommended Actions for Backend Engineer
1. **Batch Execution:**
   - Execute all DDL migration statements inside a single transactional block or batch execution call on initial DB setup.

---

### Sprint 051: Structured Schema Versioning Table

- **Review Point:** `schema.rs:43-162` — Long string of schema migration blocks
- **Severity:** 🟡 STYLISTIC (C60 — Schema Migration Maintainability)
- **Target File:** [`app/src-tauri/src/persistence/schema.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/schema.rs#L40-L115)

#### 1. Feedback Review & Root Cause
- **Verdict:** ❌ **False Positive (Deliberate Design) with Refactor** (Confidence: 95%)
- **What the code actually does:** Migration statements are maintained in an array of idempotent SQL statements executed at startup.
- **Why it's this way:** Lightweight startup schema initialization without third-party migration crates.

#### 2. Recommended Actions for Backend Engineer
1. **Adopt Schema Version Tracking:**
   - Track `PRAGMA user_version` or a `schema_migrations` table so incremental column/index modifications are executed once instead of checked on every launch.

---

### Sprint 052: Single Source of Truth for Similarity Threshold Constants

- **Review Point:** `mutations.rs:40-42` — Hardcoded similarity thresholds
- **Severity:** 🟡 STYLISTIC (C60 — Configuration SSOT)
- **Target Files:** [`app/src-tauri/src/persistence/mutations.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/mutations.rs), [`app/src-tauri/src/core/constants.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/core/constants.rs)

#### 1. Feedback Review & Root Cause
- **Verdict:** ❌ **False Positive (Deliberate Design) with SSOT Centralization** (Confidence: 95%)
- **What the code actually does:** Thresholds are passed as functional arguments in `queries.rs` (`threshold: f32`).
- **Why it's this way:** Parameterized query interface.

#### 2. Recommended Actions for Backend Engineer
1. **Centralize Defaults in `core/constants.rs`:**
   - Define canonical defaults (e.g. `PM_SIMILARITY_THRESHOLD_DEFAULT: f32 = 0.70;`) to avoid magic numbers.

---

### Sprint 053: Standardized Persistence Subsystem Log Prefixes

- **Review Point:** `worker.rs:30,34` — Inconsistent logging prefixes
- **Severity:** 🟡 STYLISTIC (C60 — Diagnostic Uniformity)
- **Target Files:** [`app/src-tauri/src/persistence/worker.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/worker.rs), [`app/src-tauri/src/persistence/memory_worker.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/persistence/memory_worker.rs)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Stylistic Polish** (Confidence: 100%)
- **What the code actually does:** Log prefixes vary between `[Persistence::Worker]`, `[Persistence::MemoryWorker]`, and `[Persistence::Schema]`.
- **Why it's this way:** Individual file authoring differences.

#### 2. Recommended Actions for Backend Engineer
1. **Standardize Prefixes:**
   - Ensure all persistence log statements use structured prefixes (`[Persistence::Worker]`, `[Persistence::MemoryWorker]`, `[Persistence::Schema]`).

---

## Module 04: Audio, Transliteration & Utilities Subsystem (`sprint-04.md`)

### Sprint 054: Defensive Tensor Output Extraction in Transliteration Engine

- **Review Point:** `translit.rs:129-224` — Translit ONNX outputs indexed by hardcoded tensor names (panic on model mismatch)
- **Severity:** 🔴 WILL BREAK (C80 — Runtime Panic on ONNX Tensor Name / Shape Mismatch)
- **Target File:** [`app/src-tauri/src/services/translit.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/translit.rs#L120-L180)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `transliterate()` indexes `decoder_outputs["logits"]`, `["h"]`, and `["c"]` using `Index<&str>` on `SessionOutputs`. If a model output name differs or a tensor rank mismatches, `ort` panics the calling thread. In addition, hidden state loops hardcode `256` dimensions without shape validation.
- **Why it's this way:** Initial implementation assumed fixed ONNX model exports.

#### 2. Recommended Actions for Backend Engineer
1. **Defensive Tensor Resolution & Shape Guards:**
   - Use `.get("logits").or_else(...)` returning `Result` instead of indexing with `["logits"]`.
   - Inspect tensor shape dynamically via `view.shape()` before indexing; return `Err(...)` (triggering raw text fallback) rather than panicking on dimension mismatch.

---

### Sprint 055: Poison-Safe `state.settings` Reads in Audio Engine

- **Review Point:** `services/audio/engine.rs:56, 87, 214, 287` — `state.settings.read().unwrap()` poisons -> engine-thread panic
- **Severity:** 🔴 WILL BREAK (C100 — Unhandled Poison Panics Crashing Audio Engine Startup)
- **Target File:** [`app/src-tauri/src/services/audio/engine.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/engine.rs#L50-L95)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `create_stt_instance` and `create_vad_instance` call `state.settings.read().unwrap()`. If any background thread panics while holding the settings lock, subsequent audio engine calls panic immediately.
- **Why it's this way:** Standard `unwrap()` on lock acquisition.

#### 2. Recommended Actions for Backend Engineer
1. **Eliminate `unwrap()` on Locks:**
   - Replace with `state.settings.read().map_err(|e| format!("Settings lock poisoned: {}", e))?` across all 4 sites in `engine.rs`.

---

### Sprint 056: Dead Code Removal for `PlaybackEngine::is_idle`

- **Review Point:** `services/audio/playback.rs:139-141` — `PlaybackEngine::is_idle()` is dead code
- **Severity:** 🟠 REAL COST AT SCALE / DEAD CODE (C100 — Unused Public Method)
- **Target File:** [`app/src-tauri/src/services/audio/playback.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/playback.rs#L138-L142)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Code** (Confidence: 100%)
- **What the code actually does:** `pub fn is_idle(&self) -> bool` has zero callers in the workspace.
- **Why it's this way:** Leftover method from previous playback polling architecture.

#### 2. Recommended Actions for Backend Engineer
1. **Remove Method:**
   - Delete `PlaybackEngine::is_idle` from `playback.rs`.

---

### Sprint 057: Dead Code Removal for `AudioError` Re-export

- **Review Point:** `services/audio/mod.rs:26` — `AudioError` re-export is unused
- **Severity:** 🟠 REAL COST AT SCALE / DEAD CODE (C100 — Unused Module Re-export)
- **Target File:** [`app/src-tauri/src/services/audio/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/mod.rs#L25-L27)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Re-export** (Confidence: 100%)
- **What the code actually does:** `pub use crate::core::error::AudioError;` is never imported via `services::audio::AudioError`.
- **Why it's this way:** Redundant re-export.

#### 2. Recommended Actions for Backend Engineer
1. **Remove Re-export:**
   - Delete `pub use crate::core::error::AudioError;` from `services/audio/mod.rs`.

---

### Sprint 058: Forwarder Task Lifecycle & JoinHandle Management

- **Review Point:** `services/audio/engine.rs:153-169, 285` — Detached `spawn_event_forwarder` task (JoinHandle dropped)
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Teardown Race Condition / Unobserved Panics)
- **Target File:** [`app/src-tauri/src/services/audio/engine.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/engine.rs#L152-L170)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Teardown Gap** (Confidence: 100%)
- **What the code actually does:** `spawn_event_forwarder` drops the `JoinHandle` immediately, leaving the task detached without explicit cancellation or join capability during engine restart/teardown.
- **Why it's this way:** Fire-and-forget forwarding task design.

#### 2. Recommended Actions for Backend Engineer
1. **Store JoinHandle in `VoxEngine`:**
   - Store `forwarder_handle: JoinHandle<()>` in `VoxEngine` struct and abort/await it in `stop_audio_engine`.

### Sprint 059: Single-Source Buffer Occupancy in Audio Playback

- **Review Point:** `playback.rs:109,326-329,343` — Playback `buffer_samples` atomic is double-counted / racy vs. actual ring occupancy
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Redundant Bookkeeping & Potential Telemetry Drift)
- **Target File:** [`app/src-tauri/src/services/audio/playback.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/playback.rs#L100-L130)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Concurrency Inefficiency** (Confidence: 95%)
- **What the code actually does:** `buffer_samples` atomic is incremented on ingest, decremented on drain via `fetch_sub` clamped by `min()`, and reset on empty. The underlying `HeapRb` consumer already maintains authoritative lock-free occupancy via `consumer.occupied_len()`.
- **Why it's this way:** Duplicate tracking added for external metrics before direct query methods were exposed.

#### 2. Recommended Actions for Backend Engineer
1. **Derive Occupancy Directly from Ring Buffer:**
   - In `PlaybackEngine::buffer_len()`, query `self.consumer.occupied_len()` directly.
   - Eliminate redundant `fetch_sub` and atomic drift risks in the audio callback.

---

### Sprint 060: Lock-Free Read & Eager Initialization for Transliteration

- **Review Point:** `translit.rs:273-301` — `transliterate()` lazy-init re-acquires global `RwLock` write lock on hot path
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Pipeline Latency Spike on First Utterance)
- **Target File:** [`app/src-tauri/src/services/translit.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/translit.rs#L270-L310)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latency Hazard** (Confidence: 95%)
- **What the code actually does:** `transliterate()` drops a read lock and acquires a write lock to load models and parse JSON files on-demand if not initialized, causing latency spikes on Hindi speech paths.
- **Why it's this way:** Fallback lazy-init mechanism.

#### 2. Recommended Actions for Backend Engineer
1. **Eager Preloading & Lock Elimination:**
   - Ensure `init_transliteration_engine()` is called eagerly during app setup.
   - Wrap transliteration engine in `OnceLock` / `ArcSwap` to eliminate read/write lock contention on transcription hot paths.

---

### Sprint 061: Zero-Copy Audio Decoding in `decode_bytes_to_24khz_mono`

- **Review Point:** `decode.rs:37` — `decode_bytes_to_24khz_mono` needlessly clones entire input
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Redundant Memory Allocations on Audio Payloads)
- **Target File:** [`app/src-tauri/src/services/audio/decode.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/decode.rs#L35-L45)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Allocation Waste** (Confidence: 100%)
- **What the code actually does:** `let cursor = std::io::Cursor::new(bytes.to_vec());` duplicates the entire audio byte buffer into a new `Vec<u8>`.
- **Why it's this way:** Quick conversion to owned Cursor.

#### 2. Recommended Actions for Backend Engineer
1. **Pass Borrowed Slice Directly:**
   - Use `std::io::Cursor::new(bytes)` directly with `Box::new(cursor)` since `Cursor<&[u8]>` implements `MediaSource + Read + Seek`.

---

### Sprint 062: Updated Tracing Directives for Active Crates

- **Review Point:** `logging.rs:20-22` — `DEFAULT_DIRECTIVES` uses legacy crate names
- **Severity:** 🟠 REAL COST AT SCALE / STYLISTIC (C80 — Diagnostic Log Noise)
- **Target File:** [`app/src-tauri/src/utils/logging.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/utils/logging.rs#L18-L23)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Configuration Drift** (Confidence: 100%)
- **What the code actually does:** Log filter string lists legacy crate aliases (`onnxruntime`, `ort_sys`, `onnx`).
- **Why it's this way:** Historical filter strings carried forward.

#### 2. Recommended Actions for Backend Engineer
1. **Update Directives:**
   - Standardize default directives to active stack: `"info,ort=warn,sherpa_onnx=warn,turso=warn,cpal=warn"`.

---

### Sprint 063: Batch Frame Ingestion & Vectorized Playback Ramp

- **Review Point:** `playback.rs:288-292` — Per-sample volume multiplication vs. SIMD block scaling
- **Severity:** ⚡ OPTIMIZATION (C60 — CPAL Audio Callback Optimization)
- **Target File:** [`app/src-tauri/src/services/audio/playback.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/playback.rs#L285-L325)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** Drains samples one-by-one with `try_pop()` and per-sample floating point arithmetic.
- **Why it's this way:** Simple scalar processing loop.

#### 2. Recommended Actions for Backend Engineer
1. **Slice-Based Draining:**
   - Pop slices in blocks when volume is stable (`target_volume == current_volume`) to enable compiler auto-vectorization and reduce per-sample branch overhead.

### Sprint 064: Scratch Buffer Reuse in Audio Playback Upsampling

- **Review Point:** `services/audio/playback.rs:16-34, 98` — Per-chunk allocation in `upsample_2x` on playback ingest path
- **Severity:** ⚡ OPTIMIZATION (C60 — Elimination of Repeated Heap Allocs on TTS Playback Path)
- **Target File:** [`app/src-tauri/src/services/audio/playback.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/playback.rs#L15-L45)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** `upsample_2x` allocates a new `Vec<f32>` of size `input.len() * 2` on every single TTS playback chunk.
- **Why it's this way:** Simple stateless helper.

#### 2. Recommended Actions for Backend Engineer
1. **Reuse Scratch Buffer:**
   - Maintain a reusable scratch vector or write upsampled sample pairs directly into the ring buffer producer.

---

### Sprint 065: Capacity Pre-Allocation in Audio Decoding Loop

- **Review Point:** `services/audio/decode.rs:150-213` — `append_samples_as_f32_mono` grows `raw_samples` unreserved
- **Severity:** ⚡ OPTIMIZATION (C60 — Buffer Reallocation Reduction in Audio Decode)
- **Target File:** [`app/src-tauri/src/services/audio/decode.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/decode.rs#L150-L190)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** `append_samples_as_f32_mono` pushes decoded frame samples into `raw_samples` without reserving buffer capacity upfront.
- **Why it's this way:** Incremental sample accumulation.

#### 2. Recommended Actions for Backend Engineer
1. **Pre-Reserve Buffer Capacity:**
   - Call `raw_samples.reserve(buf.frames())` before decoding packet frames.

---

### Sprint 066: Linear Single-Pass Forward Parser for JSON Repair

- **Review Point:** `utils/json.rs:22-101` — `fix_missing_commas_in_json` is O(n²) and mangles substring keys
- **Severity:** ⚡ OPTIMIZATION (C60 — Performance & Robustness in LLM Output Sanitization)
- **Target File:** [`app/src-tauri/src/utils/json.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/utils/json.rs#L20-L85)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization & Hardening** (Confidence: 95%)
- **What the code actually does:** `fix_missing_commas_in_json` collects all characters into `Vec<char>` and executes reverse scans (`output.chars().rev()`) whenever a key substring is found, causing quadratic scanning and potential comma misplacement in string values.
- **Why it's this way:** Ad-hoc regex-free JSON repair utility.

#### 2. Recommended Actions for Backend Engineer
1. **Single-Pass Forward Scanner:**
   - Track previous non-whitespace token state across a single linear forward pass instead of running backward searches.

---

### Sprint 067: Loop Invariant Hoisting in Linear Audio Resampling

- **Review Point:** `services/audio/decode.rs:216-231` — `resample_linear` (decode) recomputes `input.len() - 1` and floors each sample
- **Severity:** ⚡ OPTIMIZATION (C40 — Minor Decode Arithmetic Polish)
- **Target File:** [`app/src-tauri/src/services/audio/decode.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/decode.rs#L215-L235)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** Evaluates `input.len() - 1` on every output sample interpolation.
- **Why it's this way:** Simple scalar indexing.

#### 2. Recommended Actions for Backend Engineer
1. **Hoist Bound Calculations:**
   - Precompute `let max_idx = input.len().saturating_sub(1);` outside the sample loop.

---

### Sprint 068: Verification of Symphonia Trait Imports

- **Review Point:** `services/audio/decode.rs:9` — Possibly-unused `use symphonia_core::audio::Audio;` import
- **Severity:** 🟡 STYLISTIC (C40 — Clean Imports & Dead Trait Purge)
- **Target File:** [`app/src-tauri/src/services/audio/decode.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/decode.rs#L1-L20)

#### 1. Feedback Review & Root Cause
- **Verdict:** ❌ **False Positive / Trait Method Requirement** (Confidence: 90%)
- **What the code actually does:** `use symphonia_core::audio::Audio;` brings `.spec()` and `.frames()` trait methods into scope for `AudioBuffer<T>`.
- **Why it's this way:** Required trait method in scope for symphonia audio buffer manipulation.

#### 2. Recommended Actions for Backend Engineer
1. **Retain Import with Comment:**
   - Retain `use symphonia_core::audio::Audio;` if required by the compiler, or delete if `cargo clippy` indicates it is unused.

## Module 05: Dictation & Voice Activity Detection (VAD) (`sprint-05.md`)

### Sprint 069: Single-Source Realtime Audio Forwarding in VAD Actor

- **Review Point:** `services/vad/actor.rs:188-200, 318-326` — Duplicate audio frames sent to realtime server in passive/PTT-realtime modes
- **Severity:** 🔴 WILL BREAK (C100 — Realtime Stream Audio Duplication & Transcript Corruption)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L185-L330)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `stream_passive_realtime` forwards all audio chunks to `realtime_tx`. In addition, during active speech, `accumulate_speech_frames` also forwards the exact same chunk to `realtime_tx`, resulting in back-to-back 2x duplicated speech audio delivered to Gemini Live / realtime sessions.
- **Why it's this way:** Redundant forwarding hooks placed in both streaming and accumulation paths.

#### 2. Recommended Actions for Backend Engineer
1. **Consolidate Realtime Forwarding:**
   - Remove duplicate `realtime_tx.send` block from `accumulate_speech_frames`, making `stream_passive_realtime` the single source of audio chunks dispatched to the realtime channel.

---

### Sprint 070: Panic-Resilient Supervision for VAD Actor OS Thread

- **Review Point:** `services/audio/engine.rs:247-283` & `services/vad/actor.rs:462-567` — VAD actor thread is unsupervised
- **Severity:** 🔴 WILL BREAK (C80 — Silent Total VAD Failure on Thread Panic with Stuck `is_loaded` State)
- **Target Files:** [`app/src-tauri/src/services/audio/engine.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/audio/engine.rs#L245-L285), [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L460-L570)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** The VAD actor executes inside a raw `std::thread::spawn` closure calling FFI functions (`Sherpa-ONNX` / `earshot`). An unhandled panic terminates the thread while leaving `is_vad_loaded` stuck at `true` in `AppState`.
- **Why it's this way:** Lack of panic boundary on long-running OS thread.

#### 2. Recommended Actions for Backend Engineer
1. **Enclose in `catch_unwind` & Cleanup:**
   - Wrap the actor loop inside `std::panic::catch_unwind` and guarantee `handles.is_loaded.store(false, Ordering::Relaxed)` is executed on both normal exit and panic.

---

### Sprint 071: Consistent PTT Guarding on Realtime Audio Channels

- **Review Point:** `services/vad/actor.rs:318-326` — `accumulate_speech_frames` forwards to `realtime_tx` without `is_ptt` guard
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Realtime Audio Routing Inconsistency)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L315-L330)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** Realtime forwarding in `accumulate_speech_frames` lacked the `!state.realtime_is_ptt` guard present in `stream_passive_realtime`.
- **Why it's this way:** Inconsistent conditional checks between streaming helper functions.

#### 2. Recommended Actions for Backend Engineer
1. **Unify Policy:**
   - Enforce single-source forwarding (per Sprint 073) with unified PTT mode checks.

---

### Sprint 072: Disambiguated Audio Ownership in PTT Realtime Mode

> [!IMPORTANT]
> 🚩 **FLAGGED FOR REVIEW / BACKEND ENGINEER DISCUSSION:**
> The user noted doubts regarding PTT realtime audio ownership and routing boundaries. Needs dedicated alignment with the backend engineer during implementation to ensure exact interaction between `realtime::ptt::ingest_audio` and local scoring.

- **Review Point:** `services/vad/actor.rs:506-544` — PTT + realtime path double-handles audio
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Duplicate Audio Processing in PTT Sessions)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L505-L545)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Architectural Gap** (Confidence: 95%)
- **What the code actually does:** In `InteractionMode::PTT`, audio is routed both to `realtime::ptt::ingest_audio` and passed to local `process_speech_frame`.
- **Why it's this way:** Hybrid routing path during pipeline modularization.

#### 2. Recommended Actions for Backend Engineer
1. **Establish Single Ingest Path:**
   - Route PTT audio exclusively to `realtime::ptt::ingest_audio` when in realtime mode, bypassing redundant local speech scoring.

---

### Sprint 073: Earshot VAD Debouncer Simplification & Alignment

- **Review Point:** `services/vad/earshot_vad.rs:53-98` — `EarshotVadEngine` internal debounce state is dead
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Dead State Tracking in VAD Hot Path)
- **Target File:** [`app/src-tauri/src/services/vad/earshot_vad.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/earshot_vad.rs#L50-L100)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Logic** (Confidence: 100%)
- **What the code actually does:** `EarshotVadEngine::predict()` updates internal active/inactive frame counters but returns the raw per-frame boolean `is_active`, completely bypassing its own debouncing logic.
- **Why it's this way:** Outer `VadActor` handles speech start/end debouncing.

#### 2. Recommended Actions for Backend Engineer
1. **Remove Redundant Internal Debouncing:**
   - Clean up unused `active_frames` / `inactive_frames` / `is_speech` fields in `EarshotVadEngine`, relying on `VadActor`'s debouncer as the single source of truth.

### Sprint 074: In-Memory Threshold Updates in TenVAD Engine

- **Review Point:** `ten_onnx.rs:57-60` — `Ten` backend reloads ONNX model from disk on every threshold update
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Multi-Hundred Millisecond Audio Thread Block on Threshold Drag)
- **Target File:** [`app/src-tauri/src/services/vad/ten_onnx.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/ten_onnx.rs#L25-L65)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latency Hazard** (Confidence: 100%)
- **What the code actually does:** `update_detector` re-instantiates `VoiceActivityDetector` from disk files synchronously on the VAD actor thread whenever a threshold parameter is changed.
- **Why it's this way:** Simple detector replacement logic.

#### 2. Recommended Actions for Backend Engineer
1. **Debounce / In-Memory Threshold Updates:**
   - Debounce threshold configuration updates from the frontend/settings layer and apply updates without blocking the realtime audio thread.

---

### Sprint 075: Ring Buffer Replacement for VAD PreRollBuffer

- **Review Point:** `actor.rs:586-592` — `PreRollBuffer::push` does O(n) `drain(0..excess)` on every chunk once full
- **Severity:** 🟠 REAL COST AT SCALE (C100 — High-Frequency Memory Shifting in Audio Thread)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L580-L605)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Inefficiency** (Confidence: 100%)
- **What the code actually does:** `PreRollBuffer::push` calls `self.buffer.drain(0..excess)` every 256 samples once full, shifting ~8,000 float elements in memory ~62.5 times per second (~500,000 element shifts/sec).
- **Why it's this way:** Naive vector buffer truncation.

#### 2. Recommended Actions for Backend Engineer
1. **Circular Buffer Implementation:**
   - Replace `PreRollBuffer` internal `Vec<f32>` with a fixed-capacity ringbuffer / circular deque to eliminate linear array shifts.

---

### Sprint 076: Bounded Allocation Strategy for VAD Partial Transcripts

- **Review Point:** `actor.rs:328-343,296-303` — Large clones per partial: `VAD_MAX_PARTIAL_WINDOW_SAMPLES = 240000` copied every ~0.8s
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Heavy Heap Allocation Churn on Speech Hot Path)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L325-L345)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Allocation Hazard** (Confidence: 100%)
- **What the code actually does:** Emits `to_vec()` clones of up to 240,000 samples (~960 KB) on every partial interval (~0.8s) during continuous speech.
- **Why it's this way:** Standard owned payload dispatch over `mpsc` channel.

#### 2. Recommended Actions for Backend Engineer
1. **Zero-Copy / Chunked Dispatch:**
   - Pass reference-counted slices (`Arc<[f32]>`) or chunked audio envelopes to the STT worker instead of large linear clones.

---

### Sprint 077: Serialized Execution for Dictation Hotkey Events

- **Review Point:** `hotkey.rs:38-55` — Hotkey press/release spawn independent async tasks racing on shared `AppState`
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Race Conditions in Rapid PTT/Dictation Taps)
- **Target File:** [`app/src-tauri/src/services/dictation/hotkey.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/dictation/hotkey.rs#L35-L55)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Synchronization Gap** (Confidence: 95%)
- **What the code actually does:** `ShortcutState::Pressed` and `ShortcutState::Released` spawn separate uncoordinated Tokio tasks, allowing release logic to race ahead of press logic.
- **Why it's this way:** Uncoordinated async task spawning from synchronous callback.

#### 2. Recommended Actions for Backend Engineer
1. **Sequential Channel Queue:**
   - Feed hotkey press/release events into a sequential mpsc channel / state queue to guarantee FIFO execution ordering.

---

### Sprint 078: Explicit Shortcut Unregistration on Hotkey Rebind

- **Review Point:** `hotkey.rs:34-56` — Hotkey re-registration has no visible unregister path
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Duplicate Hotkey Handler Registration on Settings Updates)
- **Target File:** [`app/src-tauri/src/services/dictation/hotkey.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/dictation/hotkey.rs#L30-L60)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Lifecycle Gap** (Confidence: 95%)
- **What the code actually does:** `register_global_hotkey` registers shortcuts without explicitly unregistering previously active hotkeys on re-binding.
- **Why it's this way:** Setup helper invoked during initial bootstrap.

#### 2. Recommended Actions for Backend Engineer
1. **Clean Unregister on Rebind:**
   - Call `app.global_shortcut().unregister(...)` or `unregister_all()` before binding new shortcut definitions.

### Sprint 079: Reusable Scratch Buffer for Audio Sample Conversion

- **Review Point:** `services/vad/actor.rs:191-194, 319-322` — Per-chunk `Vec<i16>` allocation in audio hot path
- **Severity:** ⚡ OPTIMIZATION (C100 — Allocation Reduction on Realtime VAD Hot Path)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L185-L200)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization** (Confidence: 100%)
- **What the code actually does:** Allocates a fresh `Vec<i16>` of 256 samples on every chunk (~62.5/sec) during realtime streaming.
- **Why it's this way:** Simple per-chunk format mapping.

#### 2. Recommended Actions for Backend Engineer
1. **Reuse Pre-Allocated Scratch Buffer:**
   - Store a reusable `Vec<i16>` scratch buffer inside `VadActorState` and call `.clear()` before filling.

---

### Sprint 080: Audio Telemetry Invariant During Playback Ducking

- **Review Point:** `services/vad/actor.rs:479-489` — `emit_audio_telemetry` computes filter-bank + RMS on every chunk even when frames are suppressed
- **Severity:** ⚡ OPTIMIZATION / DESIGN (C80 — Visualizer Telemetry Continuity)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L475-L505)

#### 1. Feedback Review & Root Cause
- **Verdict:** ❌ **False Positive (Deliberate UI Telemetry Design)** (Confidence: 100%)
- **What the code actually does:** Computes RMS energy and filter bank telemetry before checking playback suppression.
- **Why it's this way:** Live audio waveform visualizations in the UI remain active during speaker ducking so the user sees mic activity even when VAD speech processing is paused.

#### 2. Recommended Actions for Backend Engineer
1. **Retain Continuous Telemetry:**
   - Keep telemetry calculation prior to suppression checks to maintain smooth UI visualizer frames.

---

### Sprint 081: Single-Pass Command Queue Drain in VAD Actor

- **Review Point:** `services/vad/actor.rs:109-114` — Redundant second `try_recv()` in `process_vad_commands` can silently drop command
- **Severity:** 🟡 STYLISTIC / BUG (C100 — Channel Command Drain Race Fix)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L100-L115)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Logic Flaw** (Confidence: 100%)
- **What the code actually does:** Runs a second `try_recv()` specifically to check for `Disconnected`, which will silently drop any command arriving in that microsecond window if `Ok(cmd)` is returned.
- **Why it's this way:** Ad-hoc disconnection detection after loop exit.

#### 2. Recommended Actions for Backend Engineer
1. **Unify Channel Drain Loop:**
   - Handle `Ok`, `Err(Empty)`, and `Err(Disconnected)` in a single unified `loop`/`match` construct.

---

### Sprint 082: Asynchronous Delay for OS Clipboard Paste Safety

- **Review Point:** `services/dictation/clipboard.rs:64` — `with_clipboard_safe` 350ms sleep on success path
- **Severity:** 🟡 STYLISTIC (C100 — OS Paste Race Guard)
- **Target File:** [`app/src-tauri/src/services/dictation/clipboard.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/dictation/clipboard.rs#L50-L75)

#### 1. Feedback Review & Root Cause
- **Verdict:** ❌ **False Positive (Deliberate OS Interaction Guard)** (Confidence: 100%)
- **What the code actually does:** `tokio::time::sleep(Duration::from_millis(350))` delays clipboard restoration after sending synthetic `Ctrl+V`.
- **Why it's this way:** OS input queues are asynchronous; without this delay, the target application often receives the restored original clipboard text instead of the freshly injected transcription.

#### 2. Recommended Actions for Backend Engineer
1. **Preserve Asynchronous Paste Delay:**
   - Maintain the non-blocking async delay or make it configurable via user settings.

---


---

### Sprint 083: Earshot VAD Noise Gate Multiplier & Calibration Alignment

- **Review Point:** `actor.rs:614-620` (`is_above_noise_gate` Earshot ×1.5 multiplier and `+0.15` threshold offset)
- **Severity:** ❓ UNSURE / QUESTION (Intentional Engine Sensitivity Calibration)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L610-L622)

#### 1. Feedback Review & Root Cause
- **Verdict:** ❌ **False Positive (Deliberate Acoustic Calibration)** (Confidence: 100%)
- **What the code actually does:** Multiplies noise gate threshold by 1.5 for Earshot and applies +0.15 offset.
- **Why it's this way:** Pure-Rust Earshot energy modeling operates on a different dynamic range scale than Sherpa Ten-VAD ML probabilities; calibration offsets equalize false-positive triggering rates in real-world ambient room noise.

#### 2. Recommended Actions for Backend Engineer
1. **Preserve Calibration Constant:**
   - Retain calibrated multiplier and document dynamic range rationale in `core/constants.rs`.

---

### Sprint 084: Audio Mode & Interaction Owner Synchronization

- **Review Point:** `actor.rs:100` (`UpdateMode`/`UpdateAudioMode` command consistency with `owner_atomic`)
- **Severity:** ❓ UNSURE / QUESTION (Deterministic Mode Synchronization)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L95-L115)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Architectural Contract** (Confidence: 100%)
- **What the code actually does:** `process_vad_commands` synchronizes runtime mode transitions across `InteractionOwner` atomics and audio routing flags.
- **Why it's this way:** Prevents desynchronization when switching between PTT, Passive, and Dictation modes.

#### 2. Recommended Actions for Backend Engineer
1. **Atomic Synchronization Guard:**
   - Guarantee `state.mode` and `owner_atomic` are updated synchronously during mode switch commands.


## Module 06: LLM & Providers (`sprint-06.md`)

---

> ⚠️ **BACKEND ENGINEER REVIEW FLAG (Sprints 085, 086, 088):**
> *This entire LLM provider architecture subsystem requires a dedicated, standalone review and discussion with the backend engineer to align on exact requirements — specifically whether to consolidate all remote/local endpoints into the unified `OpenAiCompatProvider` or retain and wire dedicated standalone adapters (`LmStudioAdapter`, `OllamaAdapter`, `ChatCompletionsAdapter`, `ResponsesAdapter`).*

### Sprint 085: OpenAiCompat Provider Cloud Route & Backend Classification

- **Review Point:** `openai_compat.rs:115-170` — `OpenAiCompatProvider` misclassifies authenticated OpenAI as LM Studio → wrong chat URL → 404
- **Severity:** 🔴 WILL BREAK (C80 — Cloud OpenAI Auth Breakdown)
- **Target File:** [`app/src-tauri/src/services/llm/providers/openai_compat.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/providers/openai_compat.rs#L115-L170)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** `detect_backend_kind` checks if `GET {base}/v1/models` succeeds with `200 OK`. When configured for cloud OpenAI with an API key (`https://api.openai.com`), this endpoint returns `200`, causing the engine to classify cloud OpenAI as `LocalBackendKind::LmStudio` and subsequently issue requests to `https://api.openai.com/api/v1/chat` (HTTP 404).
- **Why it's this way:** Naive endpoint probe assuming `/v1/models` is unique to LM Studio.

#### 2. Recommended Actions for Backend Engineer
1. **Explicit Cloud Provider Bypass:**
   - Check `CapabilityProbeEngine::is_cloud_provider(&self.base_url, name)` or early-return `LocalBackendKind::StandardOpenAi` for known cloud providers (`openai`, `anthropic`, `gemini`, `nvidia`, `groq`, `together`, `deepseek`) before probing local endpoints.

---

### Sprint 086: LM Studio OpenAI-Compatible Chat Endpoint & URL Normalization

- **Review Point:** `openai_compat.rs:315` & `lm_studio.rs:55` — LM Studio backend is targeted with a wrong URL through the active provider
- **Severity:** 🔴 WILL BREAK (C80 — LM Studio Connection Failure & Path Duplication)
- **Target File:** [`app/src-tauri/src/services/llm/providers/openai_compat.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/providers/openai_compat.rs#L310-L335)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Bug** (Confidence: 100%)
- **What the code actually does:** For `LocalBackendKind::LmStudio`, `generate` targets `{base}/api/v1/chat` instead of LM Studio's standard OpenAI-compatible endpoint `{base}/v1/chat/completions`. Additionally, if the user configures a URL with `/v1`, paths are duplicated (e.g., `http://localhost:1234/v1/v1/models`).
- **Why it's this way:** Divergence between dead `LmStudioAdapter` (which had correct endpoints) and `OpenAiCompatProvider`.

#### 2. Recommended Actions for Backend Engineer
1. **Correct LM Studio Target URL & Strip Trailing Suffixes:**
   - Standardize LM Studio chat endpoint to `{base}/v1/chat/completions`.
   - Strip trailing `/v1` or `/v1/` from `base_url` during initialization to prevent path doubling.

---

### Sprint 087: Embedded KV-Cache Prefix Reuse Re-Engagement

- **Review Point:** `embedded.rs:46` & `llama_cpp.rs:390-393` — Embedded KV-cache prefix reuse is permanently disengaged (`kv_cache_index` hardwired to 0)
- **Severity:** 🔴 WILL BREAK / 🟠 REAL COST AT SCALE (C100 — Severe CPU Latency Degradation on Multi-Turn Dialogue)
- **Target File:** [`app/src-tauri/src/services/llm/providers/embedded.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/providers/embedded.rs#L40-L55) and [`app/src-tauri/src/services/llm/llama_cpp.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/llama_cpp.rs#L385-L415)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Critical Bottleneck** (Confidence: 100%)
- **What the code actually does:** `EmbeddedProvider::generate` hardcodes `kv_cache_index: 0` in `ConversationContext`. In `LlmWorker::generate`, `if conv_ctx.kv_cache_index == 0 { *cache_lock = None; }` unconditionally purges the KV cache at the start of every turn.
- **Why it's this way:** Incomplete wiring between upstream working memory context builder and embedded generation request.

#### 2. Recommended Actions for Backend Engineer
1. **Thread KV Cache State or Compare System Prompt Prefix:**
   - Allow `LlmWorker` to retain `CacheState` when `system_prompt` matches and consecutive turns are appended, only resetting when system prompt changes, session resets, or barge-in occurs.

---

### Sprint 088: Provider Adapter Consolidation & Dead Code Removal

- **Review Point:** `services/llm/providers/{lm_studio,ollama,openai}.rs` — Four dedicated provider adapters are entirely dead code
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Codebase Bloat & Maintenance Hazard)
- **Target File:** [`app/src-tauri/src/services/llm/providers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/providers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Code** (Confidence: 100%)
- **What the code actually does:** `LmStudioAdapter`, `OllamaAdapter`, `ChatCompletionsAdapter`, and `ResponsesAdapter` are exported in `mod.rs` but never instantiated anywhere in `vox_lib`; `actor.rs` solely creates `EmbeddedProvider` or `OpenAiCompatProvider`.
- **Why it's this way:** Incomplete refactor towards the unified `OpenAiCompatProvider`.

#### 2. Recommended Actions for Backend Engineer
1. **Purge Unused Adapter Files:**
   - Remove dead adapter files (`lm_studio.rs`, `ollama.rs`, `openai.rs`) once `OpenAiCompatProvider` correctly handles OpenAI, LM Studio, Ollama, and Cloud endpoints.

---

### Sprint 089: Cancellable Prefill Phase in Embedded LLM Worker

- **Review Point:** `llama_cpp.rs:462-477, 434` — Embedded prefill phase cannot be cancelled
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Barge-In Latency Hazard During Long Context Prefill)
- **Target File:** [`app/src-tauri/src/services/llm/llama_cpp.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/llama_cpp.rs#L460-L480)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latency Hazard** (Confidence: 100%)
- **What the code actually does:** `LlmWorker::generate` only checks `cancel_flag` inside the token generation loop; the multi-chunk prefill `decode` loop runs synchronously to completion without checking for user cancellation.
- **Why it's this way:** Cancel checks were only placed in the streaming token loop.

#### 2. Recommended Actions for Backend Engineer
1. **Check Cancellation on Every Chunk Decode:**
   - Add `if cancel_flag.load(Ordering::Relaxed) { *cache_lock = None; return Ok(()); }` inside the prefill chunk loop.


### Sprint 090: Accurate Context Window Extraction in Capability Probes

- **Review Point:** `capability_probe.rs:125, 509, 654` — `capability_probe` returns a hardcoded/wrong context window
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Model Context Capacity Inaccuracy)
- **Target File:** [`app/src-tauri/src/services/llm/capability_probe.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/capability_probe.rs#L120-L140)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Configuration Inaccuracy** (Confidence: 95%)
- **What the code actually does:** `probe_local_embedded` hardcodes `context_window: Some(4096)` regardless of the actual user settings or GGUF metadata, and remote non-Ollama probes leave `context_window: None`.
- **Why it's this way:** Placeholder default when constructing static capability descriptors.

#### 2. Recommended Actions for Backend Engineer
1. **Dynamic Context Size Propagation:**
   - Use the active `settings.llm.context_window` or model GGUF metadata for embedded probes.
   - For remote providers, extract `context_length` from `/v1/models` or provider metadata endpoints when available.

---

### Sprint 091: Lifetime Invariant Documentation & Safe Context Encapsulation

- **Review Point:** `llama_cpp.rs:352` — Unsound `LlamaContext<'static>` lifetime transmute
- **Severity:** 🟠 REAL COST AT SCALE (C60 — Latent Soundness / Borrow Erasure)
- **Target File:** [`app/src-tauri/src/services/llm/llama_cpp.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/llama_cpp.rs#L345-L360)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latent Soundness Hazard** (Confidence: 85%)
- **What the code actually does:** `init_context` uses `unsafe { std::mem::transmute(ctx) }` to cast `LlamaContext` to `'static` for storage in a `Mutex`. While safe in the current static lifetime topology, it bypasses the compiler's borrow guarantees.
- **Why it's this way:** Storing borrowed context alongside `LlamaModel` in a single struct without self-referential crate abstractions.

#### 2. Recommended Actions for Backend Engineer
1. **Document Safety Invariants & Pin Struct:**
   - Add explicit `# Safety` invariants documenting that `LlamaWorker` is immovable and model outlives context, or encapsulate via self-referential pattern (`ouroboros` or separate RAII scope).

---

### Sprint 092: Streaming-Safe HTTP Timeout Configuration

- **Review Point:** `openai_compat.rs:70-72` — Reqwest client 180 s *total* timeout can abort long remote generations
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Long-Form Streaming Abort Risk)
- **Target File:** [`app/src-tauri/src/services/llm/providers/openai_compat.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/providers/openai_compat.rs#L65-L75)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Streaming Abort Risk** (Confidence: 95%)
- **What the code actually does:** Sets a global 180s total request timeout on the `reqwest::Client`, which applies to the entire duration of long SSE generation streams.
- **Why it's this way:** Using total request timeout rather than connect/read timeouts for streaming connections.

#### 2. Recommended Actions for Backend Engineer
1. **Configure Per-Chunk Streaming Watchdog:**
   - Keep `connect_timeout` (e.g., 10s) on `ClientBuilder`, but set global `timeout` to `None` for streaming requests, relying on per-chunk idle timeouts and explicit cancellation signals.

---

### Sprint 093: Resilient Fallback for Memory-Locked Model Loading

- **Review Point:** `llama_cpp.rs:299` — `with_use_mlock(true)` can fail model load on constrained RAM and is all-or-nothing
- **Severity:** 🟠 REAL COST AT SCALE (C60 — Hard Model Load Failure Under RAM Pressure)
- **Target File:** [`app/src-tauri/src/services/llm/llama_cpp.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/llama_cpp.rs#L295-L310)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Load Resilience Defect** (Confidence: 90%)
- **What the code actually does:** `LlamaModelParams` is constructed with `.with_use_mlock(true)`. If physical RAM limits or OS permissions prevent mlock, model loading fails immediately.
- **Why it's this way:** Optimization to prevent OS paging without a retry mechanism.

#### 2. Recommended Actions for Backend Engineer
1. **Graceful Fallback on Mlock Failure:**
   - Attempt `with_use_mlock(true)` first; if loading fails with a memory locking error, retry with `with_use_mlock(false)` and emit a warning log.

---

### Sprint 094: Panic-Free Context Window Validation

- **Review Point:** `llama_cpp.rs:341` — `NonZeroU32::new(self.ctx_size).unwrap()` panics if context window is configured as 0
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Unchecked Panic on Zero Context Size)
- **Target File:** [`app/src-tauri/src/services/llm/llama_cpp.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/llama_cpp.rs#L340-L348)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Panic Vulnerability** (Confidence: 100%)
- **What the code actually does:** Directly calls `.unwrap()` on `NonZeroU32::new(self.ctx_size)`. If settings contain `0`, the actor thread panics during context initialization.
- **Why it's this way:** Unchecked assumption that `self.ctx_size > 0`.

#### 2. Recommended Actions for Backend Engineer
1. **Defensive Clamping:**
   - Replace with `NonZeroU32::new(self.ctx_size.max(512)).unwrap()` to enforce a safe minimum context window.


### Sprint 095: Static Lazy Compilation for Error Token Regexes

- **Review Point:** `capability_probe.rs:471-495` — Regex recompiled on every `parse_token_ceiling_from_error` call
- **Severity:** ⚡ OPTIMIZATION (C100 — Eliminate Redundant Regex Compilations)
- **Target File:** [`app/src-tauri/src/services/llm/capability_probe.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/capability_probe.rs#L470-L495)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization Opportunity** (Confidence: 100%)
- **What the code actually does:** Compiles 4 regex patterns dynamically inside a loop on every call to `parse_token_ceiling_from_error`.
- **Why it's this way:** Inline pattern instantiation without static lazy caching.

#### 2. Recommended Actions for Backend Engineer
1. **Cache Regexes in Static LazyLock:**
   - Hoist the regex array to `static RE_PATTERNS: std::sync::LazyLock<[regex::Regex; 4]>` or `OnceLock`.

---

### Sprint 096: Zero-Allocation Streaming Multi-Lingual Capability Probing

- **Review Point:** `capability_probe.rs:541, 577, 589-594` — Capability probe buffers the full streaming reply in memory before measuring
- **Severity:** ⚡ OPTIMIZATION (C100 — Eliminate Redundant String Buffer Accumulation)
- **Target File:** [`app/src-tauri/src/services/llm/capability_probe.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/capability_probe.rs#L540-L595)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Minor Optimization** (Confidence: 100%)
- **What the code actually does:** Accumulates all streamed text into `full_text: String` purely to perform a single Devanagari character set scan at stream completion.
- **Why it's this way:** Post-stream character validation pattern.

#### 2. Recommended Actions for Backend Engineer
1. **Incremental Stream Character Check:**
   - Test incoming chunk strings incrementally: `if !supports_devanagari && text.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c)) { supports_devanagari = true; }`, eliminating the need to buffer the full response string.

---

### Sprint 097: Micro-Batch Sizing for Embedded GGUF Decoding

- **Review Point:** `llama_cpp.rs:344-345` — `with_n_batch(self.ctx_size)` / `with_n_ubatch(self.ctx_size)` oversize the decode batch
- **Severity:** ⚡ OPTIMIZATION (C100 — Align Decode Batch with Chunk Sizing)
- **Target File:** [`app/src-tauri/src/services/llm/llama_cpp.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/llama_cpp.rs#L340-L350)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Optimization Opportunity** (Confidence: 100%)
- **What the code actually does:** Sets `n_batch` and `n_ubatch` equal to full `ctx_size` (e.g. 4096), allocating oversized internal scratch buffers in llama.cpp.
- **Why it's this way:** Initial copy of context parameter fields.

#### 2. Recommended Actions for Backend Engineer
1. **Align Batch with DEFAULT_BATCH_CHUNK_SIZE:**
   - Configure `.with_n_batch(512).with_n_ubatch(512)` (matching `DEFAULT_BATCH_CHUNK_SIZE`) while keeping `n_ctx` at `self.ctx_size`.

---

### Sprint 098: Accurate Model Readiness Signals for Remote Providers

- **Review Point:** `actor.rs:30-34` — `actor.rs` emits `EVENT_MODEL_READY` for remote providers before any model is loaded/verified
- **Severity:** ⚡ OPTIMIZATION (C100 — IPC State Machine Accuracy)
- **Target File:** [`app/src-tauri/src/services/llm/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/llm/actor.rs#L25-L35)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed State Machine Inaccuracy** (Confidence: 100%)
- **What the code actually does:** `spawn_llm_worker` emits `EVENT_MODEL_READY` unconditionally, even for remote providers where no HTTP connection or model probe has yet verified reachability.
- **Why it's this way:** Blanket event emission at worker thread startup.

#### 2. Recommended Actions for Backend Engineer
1. **Condition Ready Event on Provider Kind & Probe:**
   - For remote providers, defer `EVENT_MODEL_READY` until health check / backend detection succeeds or emit a provider-configured event.


## Module 07: Memory Subsystem (`sprint-07.md`)

---

### Sprint 099: Deterministic XML System Prompt History Assembly

- **Review Point:** `working_memory.rs:373-395, 451-453` — System prompt duplicates `<session_history>` on every `build_context()` call
- **Severity:** 🔴 WILL BREAK (C90 — Context Window Overflow from Accumulating Duplicate History Blocks)
- **Target File:** [`app/src-tauri/src/services/memory/working_memory.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L370-L395)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Context Growth Hazard** (Confidence: 95%)
- **What the code actually does:** `consolidate_system_message` prepends `session_history` before `<user_profile>` on `self.messages[0].content`. If `self.messages[0]` or the system prompt is reused without stripping previous blocks, multiple `<session_history>` blocks can accumulate, inflating prompt token count.
- **Why it's this way:** Incremental string assembly without a clean single-source template rebuild.

#### 2. Recommended Actions for Backend Engineer
1. **Deterministic System Message Rebuild:**
   - Always assemble `self.messages[0]` from the pristine base system template, stripping any existing `<session_history>` blocks before inserting exactly one fresh `<session_history>` block per turn.

---

### Sprint 100: Resilient Queue Recovery for Transient Processing Statuses

- **Review Point:** `stage1_dedup.rs:53`, `stage2_embed.rs:41`, `stage3_eval.rs:289`, `stage4_commit.rs:53` — Items claimed into transient `processing_*` status are orphaned on restart → facts silently lost
- **Severity:** 🔴 WILL BREAK / DATA LOSS (C80 — Permanent Fact Dropping on App Crash/Restart)
- **Target File:** [`app/src-tauri/src/services/memory/pipeline/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/pipeline/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Data Loss Hazard** (Confidence: 95%)
- **What the code actually does:** Pipeline stages atomically flip queue items from input status (`staged_pending`, `deduped`, `embedded`, `evaluated`) to `processing_*`. Stage selectors only query clean status tokens. If an app crash occurs mid-stage, items remain stuck in `processing_*` indefinitely and are never re-evaluated or committed.
- **Why it's this way:** Lack of a crash-recovery lease sweeper or startup status reconciler.

#### 2. Recommended Actions for Backend Engineer
1. **Startup & Lease Expiry Status Reaper:**
   - Add a recovery sweep on memory engine startup: `UPDATE personal_memory_queue SET status = 'staged_pending' WHERE status LIKE 'processing_%' AND claimed_at < ?;`.

---

### Sprint 101: Discard Empty Facts in Stage 1 Deduplication

- **Review Point:** `stage1_dedup.rs:115-132` & `stage4_commit.rs:79-100` — Empty facts are committed as `superseded` memory_facts instead of dropped
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Database Pollution & Wasteful Storage)
- **Target File:** [`app/src-tauri/src/services/memory/pipeline/stage1_dedup.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/pipeline/stage1_dedup.rs#L115-L135) and [`app/src-tauri/src/services/memory/pipeline/stage4_commit.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/pipeline/stage4_commit.rs#L75-L105)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Database Pollution** (Confidence: 90%)
- **What the code actually does:** When a queue item contains an empty fact after `trim()`, Stage 1 routes it to `superseded_ids`, causing Stage 4 to write an empty `superseded` row into `memory_facts`.
- **Why it's this way:** Routing all non-deduped items through the superseded path rather than deleting.

#### 2. Recommended Actions for Backend Engineer
1. **Direct Queue Deletion for Empty Facts:**
   - Delete empty queue items directly in Stage 1 and do not insert placeholder rows in `memory_facts`.

---

### Sprint 102: Explicit Compaction Sentinel Turn ID Constant

- **Review Point:** `ingestion.rs:72` — Compaction generation requested with `max_tokens = 999_999` (clarified as `turn_id`)
- **Severity:** 🟠 REAL COST AT SCALE (C70 — Design Clarity & Magic Constant Elimination)
- **Target File:** [`app/src-tauri/src/services/memory/ingestion.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/ingestion.rs#L65-L75)

#### 1. Feedback Review & Root Cause
- **Verdict:** ⚖️ **Clarified Code Intention (Architectural Polish)** (Confidence: 100%)
- **What the code actually does:** `999_999` is passed as the `turn_id` argument to `provider.generate` (not `max_tokens`), while `policy.build_request(GenerationPurpose::MemoryCompaction, ...)` enforces proper bounded token caps.
- **Why it's this way:** Hardcoded sentinel turn ID for background compaction tasks.

#### 2. Recommended Actions for Backend Engineer
1. **Extract Named Constant:**
   - Define `pub const COMPACTION_SENTINEL_TURN_ID: u32 = 999_999;` in `core/constants.rs` to replace magic literal.

---

### Sprint 103: Domain-Specific Tokenizer Constants & Fail-Loud Classifier Initialization

- **Review Point:** `inter_edge_classifier.rs:24` & `intra_edge_classifier.rs:57` — Edge / NLI classifiers reuse `EMBEDDING_TOKENIZER_FILENAME` ("tokenizer.json"); silent no-op if the classifier model dir lacks that file
- **Severity:** 🟠 REAL COST AT SCALE (C70 — Silent Graph Retrieval Degradation)
- **Target File:** [`app/src-tauri/src/services/memory/classifiers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/classifiers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Silent Degraded State Hazard** (Confidence: 90%)
- **What the code actually does:** `init_edge_classifier` and `init_intra_classifier` return `Ok(false)` when tokenizer or model files are missing, silently disabling graph edge creation without alerting upstream telemetry or health checks.
- **Why it's this way:** Soft degradation design that hides missing ONNX model assets.

#### 2. Recommended Actions for Backend Engineer
1. **Dedicated Tokenizer File Constants & Diagnostic Telemetry:**
   - Define dedicated tokenizer constants (`EDGE_CLASSIFIER_TOKENIZER_FILENAME`, `NLI_TOKENIZER_FILENAME`) and surface explicit warning events when classifier models cannot be initialized.


### Sprint 104: True Pipeline Stage Error Counting & Observability

- **Review Point:** `stage1_dedup.rs:360`, `stage2_embed.rs:227`, `stage3_eval.rs:451`, `stage4_commit.rs:220` — Pipeline `error_count` is hard-coded to 0 in every stage's metrics
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Blind Metrics Masking Ingest Failures)
- **Target File:** [`app/src-tauri/src/services/memory/pipeline/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/pipeline/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Observability Defect** (Confidence: 100%)
- **What the code actually does:** All 4 pipeline stages emit `PipelineStageMetrics` with a hardcoded `error_count: 0`, even when items fail embedding or evaluation and are marked failed via `mark_job_failed`.
- **Why it's this way:** Hardcoded placeholder in metrics record constructor.

#### 2. Recommended Actions for Backend Engineer
1. **Accumulate & Emit Real Error Count:**
   - Track failed item attempts in a local `error_count` counter per batch and record the true count in `PipelineStageMetrics`.

---

### Sprint 105: Dynamic Embedding Dimension & Mismatch Prevention

- **Review Point:** `embedder.rs:207` & `mod.rs:37-41` — Embedding dimension mismatch risk with the BGE-M3 fallback
- **Severity:** 🟠 REAL COST AT SCALE (C60 — Latent Dimensionality Inconsistency on Empty Inputs)
- **Target File:** [`app/src-tauri/src/services/memory/embedder.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/embedder.rs#L205-L215)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latent Dimensionality Inconsistency** (Confidence: 90%)
- **What the code actually does:** For zero-length token sequences, `embedder.rs` returns `vec![0.0f32; EMBEDDING_DIM]`, where `EMBEDDING_DIM` is fixed to 384 (MiniLM). If a 1024-dim model (e.g., BGE-M3) is active, this generates a mismatched 384-vector.
- **Why it's this way:** Constant-sized fallback vector instead of reading active engine dimensions.

#### 2. Recommended Actions for Backend Engineer
1. **Derive Dimension from Active Engine:**
   - Store `dim` on `TextEmbedder` and use `vec![0.0f32; embedder.dim]` or return `Ok(None)` for empty inputs.

---

### Sprint 106: Batched Embedding Generation in Stage 2

- **Review Point:** `stage2_embed.rs:211-216` — Stage 2 embeds items one-by-one (no batching) under the 8 GB / sub-200 ms budget
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Sequential ONNX Inference Latency Overhead)
- **Target File:** [`app/src-tauri/src/services/memory/pipeline/stage2_embed.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/pipeline/stage2_embed.rs#L210-L220)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latency Overhead** (Confidence: 95%)
- **What the code actually does:** Loops over claimed items one-by-one, executing individual ONNX session forward passes per fact instead of batching the 16 facts together.
- **Why it's this way:** Initial scalar loop implementation.

#### 2. Recommended Actions for Backend Engineer
1. **Expose Batched Embedding API:**
   - Implement `generate_embeddings_batch(&[&str]) -> Result<Vec<Vec<f32>>>` constructing an `(N, max_seq)` input tensor to embed the full batch in a single forward pass.

---

### Sprint 107: Concurrent Multi-Item Evaluation in Stage 3

- **Review Point:** `stage3_eval.rs:337-349, 437` — `spawn_blocking` runs two independent CPU sub-branches but each item is still serial across the batch
- **Severity:** ⚡ OPTIMIZATION (C80 — CPU Core Under-Utilization)
- **Target File:** [`app/src-tauri/src/services/memory/pipeline/stage3_eval.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/pipeline/stage3_eval.rs#L335-L355)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Throughput Optimization** (Confidence: 95%)
- **What the code actually does:** Parallelizes NLI and ModernBERT sub-branches for a single item, but processes items within a batch serially one after another.
- **Why it's this way:** Conservative serial batch processing.

#### 2. Recommended Actions for Backend Engineer
1. **Batch Concurrency:**
   - Process multiple items concurrently across `spawn_blocking` tasks (or execute batched ONNX passes) to saturate multi-threaded worker pools.

---

### Sprint 108: Short-Circuit Non-Vectorized Collections in Stage 3

- **Review Point:** `stage3_eval.rs:305-313` & `stage2_embed.rs:57-64` — Narrative facts are embedded then thrown away / queried with empty vectors
- **Severity:** ⚡ OPTIMIZATION (C90 — Eliminate Redundant SQL Candidate Searches)
- **Target File:** [`app/src-tauri/src/services/memory/pipeline/stage3_eval.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/pipeline/stage3_eval.rs#L305-L335)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Wasteful Query Optimization** (Confidence: 100%)
- **What the code actually does:** When an item has an empty vector (such as narrative facts), Stage 3 still invokes `fetch_intra_collection_candidates` and `fetch_inter_collection_candidates`, issuing empty SQL queries.
- **Why it's this way:** Lack of an early return guard for non-vectorized facts.

#### 2. Recommended Actions for Backend Engineer
1. **Short-Circuit on Empty Vector:**
   - Check `if item.vector.is_empty() { return advance_item_to_evaluated(conn, item.id).await; }` to bypass candidate queries and NLI evaluation.


> ⚠️ **BACKEND ENGINEER REVIEW FLAG (Sprints 109, 110):**
> *This memory formatting and user profile assembly subsystem requires a dedicated discussion and alignment with the backend engineer — specifically to decide on the single source of truth for `<user_profile>` XML/markdown context assembly across retrieval (`retrieval.rs`), working memory (`working_memory.rs`), and the formatter module (`formatter.rs`), and whether to purge or consolidate divergent formatting primitives (`format_user_profile_context`).*

### Sprint 109: Dead Memory Formatting & Math Utilities Purge

- **Review Point:** `formatter.rs:56`, `mod.rs:68`, `embedder.rs:265` — Dead code: `format_user_profile_context`, `format_relative_timestamp`, `cosine_similarity`
- **Severity:** 🟡 STYLISTIC (C100 — Eliminate Unused Functions & Stale Exports)
- **Target File:** [`app/src-tauri/src/services/memory/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Code** (Confidence: 100%)
- **What the code actually does:** `format_user_profile_context` in `formatter.rs` is unused outside tests, `format_relative_timestamp` is exported without callers, and `cosine_similarity` in `embedder.rs` has no internal Rust callers (Turso uses vector distance functions in SQL).
- **Why it's this way:** Residue from previous formatting prototypes.

#### 2. Recommended Actions for Backend Engineer
1. **Purge Uncalled Exports:**
   - Remove `format_user_profile_context` and unused timestamp formatters, or unify with the active context assembler.

---

### Sprint 110: Unified `<user_profile>` Context Assembler

- **Review Point:** `formatter.rs:56` vs `retrieval.rs:241-244` and `working_memory.rs:109-145` — Two divergent `<user_profile>` assemblers can drift
- **Severity:** 🟡 STYLISTIC (C95 — Eliminate Prompt Formatting Drift)
- **Target File:** [`app/src-tauri/src/services/memory/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Prompt Consistency Risk** (Confidence: 95%)
- **What the code actually does:** `retrieval.rs` builds `<user_profile>` and `<semantic_graph>` XML blocks, `working_memory.rs` performs string surgery to insert them into the system prompt, and `formatter.rs` formats alternative `[Identity]` text headers.
- **Why it's this way:** Split formatting implementations across retrieval and working memory modules.

#### 2. Recommended Actions for Backend Engineer
1. **Consolidate XML Formatting SSOT:**
   - Establish a single canonical context formatting module (e.g. `formatter.rs`) utilized uniformly across retrieval and prompt construction.

---

### Sprint 111: Clean Jaccard Direct Check in Stage 1

- **Review Point:** `stage1_dedup.rs:144` — `is_exact_duplicate` called with a hard-coded `0.0` cosine in Stage 1
- **Severity:** 🟡 STYLISTIC (C100 — Misleading Literal Elimination)
- **Target File:** [`app/src-tauri/src/services/memory/pipeline/stage1_dedup.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/pipeline/stage1_dedup.rs#L140-L155)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Code Polish** (Confidence: 100%)
- **What the code actually does:** Stage 1 only has raw text (no vector embeddings), so it passes `0.0` as the cosine parameter to `is_exact_duplicate(0.0, jacc_sim)`.
- **Why it's this way:** Reusing a dual-metric helper where one metric is not yet computed.

#### 2. Recommended Actions for Backend Engineer
1. **Direct Jaccard Threshold Assertion:**
   - Call `jaccard_similarity(trimmed_fact, cand_fact) >= JACCARD_EXACT_MATCH_THRESHOLD` directly instead of passing dummy zeros to multi-metric functions.

---

### Sprint 112: Panic-Free Tokio Runtime Fallback in Compaction

- **Review Point:** `ingestion.rs:84, 95` — `expect()` panics on tokio runtime construction in the compaction path
- **Severity:** 🟡 STYLISTIC (C100 — Robust Fallback on Runtime Resource Exhaustion)
- **Target File:** [`app/src-tauri/src/services/memory/ingestion.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/ingestion.rs#L75-L100)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Panic Vulnerability** (Confidence: 100%)
- **What the code actually does:** Uses `.expect(...)` when constructing local Tokio runtime threads for background compaction generation.
- **Why it's this way:** Standard runtime construction boilerplate.

#### 2. Recommended Actions for Backend Engineer
1. **Propagate Errors Gracefully:**
   - Replace `.expect(...)` with `?` or map error into `Err(anyhow!(...))` so `perform_compaction_maintenance` can safely fall back to FIFO context compaction.


## Module 08: Pipeline Orchestration (`sprint-08.md`)

---

### Sprint 113: Gate Realtime PTT Audio Streaming & Eliminate Duplicate Ingestion

- **Review Point:** `vad/actor.rs:318-326`, `realtime/ptt.rs:213-218`, `realtime/ptt.rs:279-291` — Realtime PTT double-delivers (and triple-counts) the utterance to the cloud provider
- **Severity:** 🔴 WILL BREAK (C100 — Audio Duplication & Triple Delivery in Cloud PTT)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L315-L330) and [`app/src-tauri/src/services/pipeline/realtime/ptt.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/realtime/ptt.rs#L210-L295)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Critical Functional Bug** (Confidence: 100%)
- **What the code actually does:** In `accumulate_speech_frames` (vad/actor.rs:318), speech audio is streamed chunk-by-chunk to `state.realtime_tx` without checking `!state.realtime_is_ptt`. Then when PTT stops, `handle_ptt_stop` re-pushes the entire recorded buffer to the cloud engine, while `on_speech_end` appends `utterance_buffer` into `REALTIME_PTT_BUFFER` on top of `ingest_audio`. The provider receives duplicated audio, causing 2x token usage and garbled duplicate responses.
- **Why it's this way:** Missing PTT exclusion guard in `accumulate_speech_frames` and redundant append in `on_speech_end`.

#### 2. Recommended Actions for Backend Engineer
1. **Enforce PTT Exclusivity in Accumulate Frames:**
   - In `accumulate_speech_frames`, check `if !state.realtime_is_ptt` before streaming audio chunks over `state.realtime_tx`.
2. **Eliminate Redundant Buffer Append:**
   - In `realtime/ptt.rs:on_speech_end`, do not append audio into `REALTIME_PTT_BUFFER` (audio is already ingested chunk-by-chunk via `ingest_audio`).

---

### Sprint 114: Cancel Audio Playback Engine on Modular Passive Barge-In

- **Review Point:** `modular/passive.rs:156-174` — Modular passive barge-in does not actually stop playback
- **Severity:** 🔴 WILL BREAK (C100 — Audio Continues Playing While UI Enters Listening)
- **Target File:** [`app/src-tauri/src/services/pipeline/modular/passive.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/modular/passive.rs#L155-L175)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Critical Barge-In Failure** (Confidence: 100%)
- **What the code actually does:** `on_speech_start` sets `cancel_flag = true`, clears the sentence chunker, and transitions to `Listening`, but never calls `engine.playback_engine.cancel()`. Queued audio in CPAL ringbuffers continues playing aloud while the assistant is supposed to be listening.
- **Why it's this way:** Incomplete barge-in handler omitting playback engine flush.

#### 2. Recommended Actions for Backend Engineer
1. **Cancel Playback Engine on Speech Start:**
   - In `on_speech_start`, acquire `state.engine` lock and call `engine.playback_engine.cancel()`.

---

> ⚠️ **BACKEND ENGINEER REVIEW FLAG (Sprints 115, 121, 126 — Turn ID Sync Architecture):**
> *The entire Turn ID synchronization and propagation subsystem across VAD (`vad/actor.rs`), Pipeline Orchestration (`realtime/ptt.rs`, `modular/context.rs`), Cloud Realtime Providers (`gemini_live.rs`, `deepgram_live.rs`), and IPC requires a dedicated architecture review and discussion with the backend engineer — specifically to establish a single authoritative turn ID minting lifecycle, enforce turn ID propagation into context building and tracing, and eliminate provider `turn_id: 0` synthetic resets.*

### Sprint 115: Unified Turn ID Flow Across Realtime PTT & VAD

- **Review Point:** `realtime/ptt.rs:167, 192`, `vad/actor.rs:227` — Realtime PTT turn-id incoherence (three/four different ids per single interaction)
- **Severity:** 🔴 WILL BREAK (C100 — Turn ID Misalignment Across Events & Persistence)
- **Target File:** [`app/src-tauri/src/services/pipeline/realtime/ptt.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/realtime/ptt.rs#L165-L200) and [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L220-L235)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed State Incoherence Bug** (Confidence: 100%)
- **What the code actually does:** `handle_ptt_start` increments `pipeline.turn_id`. When speech is subsequently detected, `vad/actor.rs` increments the shared `turn_id_atomic` again, emitting `SpeechStart` with a different ID, while `handle_ptt_stop` reads the post-incremented ID and cloud providers return `turn_id: 0`.
- **Why it's this way:** Multiple independent producers incrementing the turn counter for a single interaction turn.

#### 2. Recommended Actions for Backend Engineer
1. **Single Turn ID Minting Authority:**
   - In PTT mode, let `handle_ptt_start` mint the turn ID; suppress redundant VAD atomic increment when PTT is active.

---

### Sprint 116: Pipeline State Recovery on Generation Cancellation

- **Review Point:** `dictation.rs:229-241`, `modular/passive.rs:397-421`, `modular/ptt.rs:451-473`, `realtime/passive.rs:280-298`, `realtime/ptt.rs:388-411` — `VoxEvent::Cancelled` is ignored by every domain handler; pipeline state can hang
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Pipeline Stuck in Thinking/Speaking on Interrupt)
- **Target File:** [`app/src-tauri/src/services/pipeline/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed State Machine Stall Risk** (Confidence: 100%)
- **What the code actually does:** `handle_event` across all pipeline domain dispatchers falls through to `_ => {}` on `VoxEvent::Cancelled`, failing to transition state back to `Ready` or `Idle` if cancellation happens during LLM inference or audio generation.
- **Why it's this way:** Unhandled cancellation event variant in match blocks.

#### 2. Recommended Actions for Backend Engineer
1. **Handle Cancelled Event:**
   - Add `VoxEvent::Cancelled { turn_id } => on_cancelled(turn_id, app, state)` transitioning interaction state back to `Ready` and emitting `EVENT_INTERACTION_STATE_CHANGED`.

---

### Sprint 117: Purge Dead Public Buffer & Recording Accessors

- **Review Point:** `dictation.rs:23, 28`, `modular/ptt.rs:33, 38`, `realtime/ptt.rs:34, 39` — Six dead public accessors (`is_recording` / `get_buffer_len`)
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Dead Code Purge)
- **Target File:** [`app/src-tauri/src/services/pipeline/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Code** (Confidence: 100%)
- **What the code actually does:** Defines `pub fn is_recording()` and `pub fn get_buffer_len()` across three pipeline modules without any call sites across the codebase.
- **Why it's this way:** Leftover inspection helpers from earlier development phases.

#### 2. Recommended Actions for Backend Engineer
1. **Purge Uncalled Accessors:**
   - Delete the 6 unused accessor functions to keep module interfaces minimal.


### Sprint 118: Read SPEECH_DETECTED for Ghost Audio Rejection in Modular PTT

- **Review Point:** `modular/ptt.rs:18, 146, 181-185` — `SPEECH_DETECTED` in `modular/ptt.rs` is write-only (dead logic)
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Ghost Audio Filtering Inconsistency)
- **Target File:** [`app/src-tauri/src/services/pipeline/modular/ptt.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/modular/ptt.rs#L180-L208)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Ghost-Audio Inconsistency** (Confidence: 100%)
- **What the code actually does:** `SPEECH_DETECTED` is stored on speech detection in `modular/ptt.rs`, but `handle_ptt_stop` never loads it, checking only `if audio.is_empty()`. If a user taps the PTT key in silence, ambient silence is dispatched to STT, unlike `realtime/ptt.rs` which gates on `SPEECH_DETECTED`.
- **Why it's this way:** Incomplete parity with realtime PTT ghost-audio filter.

#### 2. Recommended Actions for Backend Engineer
1. **Gate Modular PTT on Detected Speech:**
   - In `modular/ptt.rs:handle_ptt_stop`, check `if !SPEECH_DETECTED.load(Ordering::Relaxed) { PTT_BUFFER.lock().clear(); return Ok(()); }` to discard silence audio safely.

---

### Sprint 119: Contention-Resilient Audio Engine Sender Acquisition

- **Review Point:** `modular/ptt.rs:187`, `modular/passive.rs:242-249`, `modular/ptt.rs:242-249` — `try_lock` failures silently drop the user's turn / response
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Silent Dropped Turns on Lock Contention)
- **Target File:** [`app/src-tauri/src/services/pipeline/modular/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/modular/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Drop Turn Vulnerability** (Confidence: 95%)
- **What the code actually does:** `on_transcript_final` uses `try_lock` to acquire `state.engine`. If the lock is briefly busy, it returns `(None, None)` without retrying or erroring, resulting in no assistant response generated for a valid finalized user utterance.
- **Why it's this way:** Non-blocking lock call without fallback or retry.

#### 2. Recommended Actions for Backend Engineer
1. **Cache Senders or Await Lock:**
   - Clone `tts_tx`/`llm_tx` once at session start or acquire `state.engine.lock()` with a short bounded timeout so turns are never dropped under transient contention.

---

### Sprint 120: Poison-Resilient Settings Reads on Router Thread

- **Review Point:** `dictation.rs:160, 178`, `modular/passive.rs:196, 213, 238`, `modular/ptt.rs:237, 254, 279`, `realtime/passive.rs:73, 136` — `RwLock::read().unwrap()` / `lock().unwrap()` can panic on poison and are used per-event
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Router Worker Thread Crash on Poisoned Lock)
- **Target File:** [`app/src-tauri/src/services/pipeline/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Actor Crash Vulnerability** (Confidence: 95%)
- **What the code actually does:** Calls `.unwrap()` on `state.settings.read()` across multiple event handlers. If another thread panics while holding the settings lock, subsequent events will panic the dedicated `vox-router` thread, terminating all pipeline event routing permanently.
- **Why it's this way:** Standard unwrap pattern on synchronized state locks.

#### 2. Recommended Actions for Backend Engineer
1. **Use Poison-Safe Lock Guards:**
   - Replace `.read().unwrap()` with `.read().unwrap_or_else(|p| p.into_inner())` or graceful error handling across router handlers.

---

### Sprint 121: Realtime Provider Turn ID Threading

- **Review Point:** `gemini_live.rs` & `deepgram_live.rs` — Realtime providers hardcode `turn_id: 0` for all response events
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Session Memory Turn ID Incoherence)
- **Target File:** [`app/src-tauri/src/services/realtime/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Correlation Defect** (Confidence: 100%)
- **What the code actually does:** Realtime cloud providers emit `TranscriptFinal`, `LlmToken`, `LlmFinished`, and `Error` with a fixed `turn_id: 0`, which pipeline handlers forward verbatim into `conversation_manager` and the UI.
- **Why it's this way:** Hardcoded zero in WebSocket message event builders.

#### 2. Recommended Actions for Backend Engineer
1. **Pass Turn ID to Provider Sessions:**
   - Thread the active session `turn_id` into the realtime session / engine so all emitted tokens and transcript events carry the correct turn ID.

---

### Sprint 122: Lightweight Routing Context Snapshots

- **Review Point:** `router.rs:16` & `mod.rs:48` — Full `VoxSettings` clone on *every* routed event
- **Severity:** ⚡ OPTIMIZATION (C100 — Eliminate Redundant Settings Deep Copies on Router Hot Path)
- **Target File:** [`app/src-tauri/src/services/pipeline/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/mod.rs#L45-L65)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Hot-Path Allocation Optimization** (Confidence: 100%)
- **What the code actually does:** `RoutingContext::from_app_state` reads and clones the entire settings struct for every single event on the router thread, even though only 3 scalar enums (`PipelineMode`, `InteractionMode`, `InteractionOwner`) are required for dispatch.
- **Why it's this way:** Full state snapshot convenience helper.

#### 2. Recommended Actions for Backend Engineer
1. **Extract Minimal Scalar Fields:**
   - Read only the required enum fields without deep-cloning full nested configuration structs.


### Sprint 123: Database Connection Reuse in Context Construction

- **Review Point:** `modular/context.rs:77, 113` — Two DB opens per modular turn
- **Severity:** ⚡ OPTIMIZATION (C100 — Connection Churn & Latency Reduction)
- **Target File:** [`app/src-tauri/src/services/pipeline/modular/context.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/modular/context.rs#L70-L130)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latency & FD Optimization** (Confidence: 100%)
- **What the code actually does:** `build_generation_request` opens a read-only Turso database connection for personal context retrieval, and then spawns a background task that opens the database *again* to enqueue extracted personal memory facts.
- **Why it's this way:** Independent asynchronous query and mutation blocks.

#### 2. Recommended Actions for Backend Engineer
1. **Reuse Connection Pool / Shared Handle:**
   - Pass the existing connection or a shared DB handle into the spawned persistence task instead of opening a second file descriptor per user turn.

---

### Sprint 124: Cache TTS Senders to Eliminate Per-Chunk Lock Contention

- **Review Point:** `modular/passive.rs:296` and `modular/ptt.rs:337` — `try_lock` per TTS chunk in token/finished handlers
- **Severity:** ⚡ OPTIMIZATION (C100 — Lock Contention Elimination on Streaming Audio Chunks)
- **Target File:** [`app/src-tauri/src/services/pipeline/modular/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/modular/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Hot-Path Contention Optimization** (Confidence: 100%)
- **What the code actually does:** Re-acquires `state.engine.try_lock()` on every single clause/token emitted during LLM streaming. Under lock contention with audio playback callbacks, clauses are silently dropped without being synthesized by TTS.
- **Why it's this way:** Inline engine lock acquisition inside high-frequency token callbacks.

#### 2. Recommended Actions for Backend Engineer
1. **Capture Sender at Turn Inception:**
   - Capture `tts_tx` once at turn start or retain an active channel sender reference to avoid locking the entire engine struct per token.

---

### Sprint 125: Pre-Allocate Static IPC Status Payloads

- **Review Point:** `dictation.rs:48`, `modular/passive.rs:202`, `realtime/ptt.rs:174` — Per-event `serde_json::json!({...})` allocations
- **Severity:** ⚡ OPTIMIZATION (C100 — Minor Allocation Optimization)
- **Target File:** [`app/src-tauri/src/services/pipeline/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Minor Allocation Optimization** (Confidence: 100%)
- **What the code actually does:** Dynamically constructs `serde_json::Value` objects for static status transitions (`STATUS_RECORDING`, `STATUS_PROCESSING`, `IDLE`).
- **Why it's this way:** Inline json macro ergonomics.

#### 2. Recommended Actions for Backend Engineer
1. **Use Typed IPC Payloads / Static Consts:**
   - Pre-serialize static status strings or emit strongly typed structs to eliminate runtime map allocations on the router thread.

---

### Sprint 126: Clean Unused Turn ID Parameter in Context Builder *(Grouped with Sprint 115 Review Flag)*

- **Review Point:** `modular/context.rs:67` — `build_generation_request(..., _turn_id: u32, ...)` unused parameter
- **Severity:** 🟡 STYLISTIC (C100 — Dead Signature Parameter Removal / Turn ID Sync)
- **Target File:** [`app/src-tauri/src/services/pipeline/modular/context.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/modular/context.rs#L60-L70)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Parameter** (Confidence: 100%)
- **What the code actually does:** `_turn_id: u32` is accepted and passed from callers but never used inside `build_generation_request`.
- **Why it's this way:** Leftover parameter from earlier turn-indexed prompt templates; tied to global Turn ID synchronization (see Sprints 115 & 121 Review Flag).

#### 2. Recommended Actions for Backend Engineer
1. **Incorporate or Remove Parameter:**
   - Either utilize `turn_id` for structured context tracing / retrieval telemetry or drop it from the function signature and call sites as aligned during the Turn ID architecture review.

---

### Sprint 127: User Turn Ingestion Alignment Across Realtime Handlers

- **Review Point:** `realtime/passive.rs:215` & `realtime/ptt.rs` — Ingest user turns to ConversationManager without duplicate accumulation
- **Severity:** 🟡 STYLISTIC (C90 — Memory Ingestion Deduplication)
- **Target File:** [`app/src-tauri/src/services/pipeline/realtime/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/realtime/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Conversation Manager Alignment Risk** (Confidence: 95%)
- **What the code actually does:** `on_transcript_final` blindly invokes `cm.push_user_turn(text)` whenever a final transcript event arrives from the cloud provider, risking duplicate turns if the provider emits multiple final segments.
- **Why it's this way:** Simple push without checking last turn identity.

#### 2. Recommended Actions for Backend Engineer
1. **Deduplicate Successive Final Transcripts:**
   - Check against the last recorded user turn before pushing to prevent duplicate conversation history entries.


### Sprint 128: Clean Pipeline Module Re-Imports

- **Review Point:** `mod.rs:34-37` re-imports (`use crate::core::settings::...`, `use crate::core::state::...`, `use tauri::{AppHandle, Emitter}`)
- **Severity:** 🟡 STYLISTIC (C100 — Idiomatic Module Organization)
- **Target File:** [`app/src-tauri/src/services/pipeline/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/mod.rs#L30-L45)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Idiomatic Style** (Confidence: 100%)
- **What the code actually does:** Re-imports required core state, settings enums, and Tauri emitter primitives for use within `RoutingContext` and `transition` helpers in `mod.rs`.
- **Why it's this way:** Standard Rust module scoping.

#### 2. Recommended Actions for Backend Engineer
1. **Retain Clean Scoped Imports:**
   - Keep current clean module-level imports as idiomatic Rust; no changes required.

---

### Sprint 129: Clean Async Closure Ergonomics in Dictation Handler

- **Review Point:** `dictation.rs:185` spawns an async task for `output_router::route_transcript`
- **Severity:** 🟡 STYLISTIC (C100 — Closure Capture Ergonomics)
- **Target File:** [`app/src-tauri/src/services/pipeline/dictation.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/dictation.rs#L175-L195)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Minor Style Polish** (Confidence: 100%)
- **What the code actually does:** Clones `app` and `processed_text` into temporary local bindings before moving them into `tauri::async_runtime::spawn`.
- **Why it's this way:** Explicit pre-spawn clone bindings.

#### 2. Recommended Actions for Backend Engineer
1. **Streamline Closure Capture:**
   - Move `.clone()` directly into the async move closure to eliminate redundant intermediate variable declarations.

---

### Sprint 130: Verify `pop_last_user_turn` Safety on Initial Barge-In

- **Review Point:** `modular/passive.rs:166` — Safety of `pop_last_user_turn` during assistant greeting / first-turn interrupt
- **Severity:** ❓ UNSURE / QUESTION (Memory Rollback Safety Analysis)
- **Target File:** [`app/src-tauri/src/services/memory/working_memory.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/memory/working_memory.rs#L310-L325)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Safe Implementation** (Confidence: 100%)
- **What the code actually does:** `pop_last_user_turn` explicitly inspects `if last.role == Role::User` before popping the message from history and decrementing token count. If the assistant is speaking a greeting or following an assistant turn, the last message role is `Assistant` or `System`, causing `pop_last_user_turn` to safely no-op without corrupting conversation memory.
- **Why it's this way:** Guarded role verification inside `WorkingMemory`.

#### 2. Recommended Actions for Backend Engineer
1. **Preserve Guarded Pop:**
   - Retain `if last.role == Role::User` guard in `pop_last_user_turn` and add unit test verifying greeting interrupt safety.

---

### Sprint 131: Clarify Public Accessors Purpose & Purge Dead APIs

- **Review Point:** `is_recording()` / `get_buffer_len()` across pipeline modules (F5 resolution)
- **Severity:** ❓ UNSURE / QUESTION (Public Interface Scope Clarification)
- **Target File:** [`app/src-tauri/src/services/pipeline/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead API Stubs** (Confidence: 100%)
- **What the code actually does:** Defines standalone `is_recording()` and `get_buffer_len()` functions that are not bound to Tauri IPC commands or referenced anywhere in Rust.
- **Why it's this way:** Temporary inspection helpers left over from prototyping.

#### 2. Recommended Actions for Backend Engineer
1. **Purge Dead Interfaces:**
   - Remove these dead functions as documented in Sprint 117 to maintain a minimal surface area.

---

### Sprint 132: Confirm Ghost Audio Protection Requirement in Modular PTT

- **Review Point:** `SPEECH_DETECTED` intentionality in `modular/ptt.rs` (F6 resolution)
- **Severity:** ❓ UNSURE / QUESTION (Behavioral Parity Across Pipeline Modes)
- **Target File:** [`app/src-tauri/src/services/pipeline/modular/ptt.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/pipeline/modular/ptt.rs#L180-L208)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Behavioral Parity Requirement** (Confidence: 100%)
- **What the code actually does:** Realtime PTT gates audio dispatch on `SPEECH_DETECTED.load(Ordering::Relaxed)` to discard accidental silence presses, whereas Modular PTT previously sent empty silence to STT.
- **Why it's this way:** Incomplete parity during modular pipeline refactoring.

#### 2. Recommended Actions for Backend Engineer
1. **Enforce Ghost Audio Rejection:**
   - Implement `SPEECH_DETECTED` gating in `modular/ptt.rs:handle_ptt_stop` to match realtime PTT behavior exactly.


## Module 09: Realtime Providers (`sprint-09.md`)

---

### Sprint 133: Explicit Runtime Handle Injection in Realtime Connect

- **Review Point:** `deepgram_live.rs:75` and `gemini_live.rs:74` — `Handle::current()` + `block_in_place` panic risk if ever called off-runtime
- **Severity:** 🔴 WILL BREAK (C80 — Latent Panic Vulnerability Off-Runtime)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latent Panic Vulnerability** (Confidence: 80%)
- **What the code actually does:** `connect()` in `deepgram_live.rs` and `gemini_live.rs` invokes `tokio::runtime::Handle::current()` and `block_in_place`. If called from any non-worker thread or synchronous test harness, `Handle::current()` panics immediately.
- **Why it's this way:** Implicit runtime acquisition instead of using injected handle parameters.

#### 2. Recommended Actions for Backend Engineer
1. **Pass Runtime Handle in Trait:**
   - Pass `handle: &tokio::runtime::Handle` explicitly through `RealtimeSessionProvider::connect` or make `connect` an async trait method.

---

### Sprint 134: Terminated Session Guard Against Unbounded Audio Channel Growth

- **Review Point:** `deepgram_live.rs:605-608`, `gemini_live.rs:726-728` — Unbounded `audio_tx`/`control_tx` growth after permanent disconnect (8GB leak)
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Memory Leak on Permanent Reconnect Failure)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Memory Leak on Permanent Disconnect** (Confidence: 100%)
- **What the code actually does:** On max reconnect attempts exceeded, the reconnect orchestrator calls `audio_sender_task.abort()`, but the session's unbounded `audio_tx` channel remains open and accepts incoming mic PCM without error, causing unbounded memory growth under active audio ingestion.
- **Why it's this way:** Aborting the consumer task without closing the producer channel or setting a termination flag.

#### 2. Recommended Actions for Backend Engineer
1. **Enforce Session Termination Flag:**
   - Set an `Arc<AtomicBool>` `terminated` flag on permanent disconnection; in `send_audio()`, return `Err` if terminated so `AudioBridge` breaks its forwarder loop.

---

### Sprint 135: Non-Blocking Realtime Handshake Execution

- **Review Point:** `deepgram_live.rs:85-89` and `gemini_live.rs:96-108` — Handshake blocks a tokio worker thread for up to 5s
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Worker Thread Starvation During Connect)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Worker Starvation Latency Defect** (Confidence: 100%)
- **What the code actually does:** `connect()` performs the WebSocket TLS handshake synchronously inside `block_in_place`, blocking a Tokio worker thread for up to the 5s timeout.
- **Why it's this way:** Synchronous wrapper around async handshake routine.

#### 2. Recommended Actions for Backend Engineer
1. **Offload Handshake to Blocking Pool / Async Connect:**
   - Execute the handshake via `tokio::task::spawn_blocking` or native async connect to keep runtime worker threads free for audio streaming.

---

### Sprint 136: Active Ping/Pong Keepalive & Silent Drop Detection

- **Review Point:** `deepgram_live.rs:174-199` and `gemini_live.rs` — No real silent-disconnect detection
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Silent Connection Stall Without Reconnect)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Silent Disconnect Vulnerability** (Confidence: 80%)
- **What the code actually does:** Deepgram sends JSON KeepAlive without verifying responses; Gemini has no heartbeat task. If intermediate NAT routers silently terminate the TCP connection, the read loop remains hung indefinitely without triggering a reconnect.
- **Why it's this way:** Missing protocol-level WS ping frames and activity staleness tracking.

#### 2. Recommended Actions for Backend Engineer
1. **Implement WebSocket Ping & Staleness Trigger:**
   - Transmit `Message::Ping` frames at regular intervals and trigger reconnect if `last_activity_time` exceeds stale timeout threshold.

---

### Sprint 137: Backpressure-Aware Playback Bridge Channel

- **Review Point:** `deepgram_live.rs:241-243` and `gemini_live.rs:881-885` — Playback audio dropped on full bridge channel
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Audible Audio Clipping and Speech Stutter)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Audio Clipping Defect** (Confidence: 100%)
- **What the code actually does:** Uses `playback_tx.try_send(pcm)` with drop-newest policy when the bridge buffer reaches capacity, discarding chunks during audio playback spikes and causing audible distortion.
- **Why it's this way:** Unhandled buffer saturation in WebSocket binary receiver.

#### 2. Recommended Actions for Backend Engineer
1. **Implement Backpressure / Drop-Oldest Policy:**
   - Use async `send()` with backpressure or a circular buffer with drop-oldest semantics to maintain continuous playback stream integrity.


### Sprint 138: Enable Client-Side In-Flight Audio Suppression in Gemini Live

- **Review Point:** `gemini_live.rs:852, 859, 912, 929` — Gemini `interrupt_active` is only ever set `false`, never `true` (dead suppression)
- **Severity:** 🟠 REAL COST AT SCALE (C60 — Client-Side In-Flight Token & Audio Suppression)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/gemini_live.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/gemini_live.rs#L845-L870)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Client-Side Suppression Gap** (Confidence: 85%)
- **What the code actually does:** `interrupt_active` is read as a suppression guard across `modelTurn`, token, and audio processing branches, and reset to `false` on server confirmation, but is never set to `true` on client-side interrupt. As a result, in-flight frames are processed and emitted until the server round-trip completes.
- **Why it's this way:** Missing assignment in the client-side cancel / interrupt handler.

#### 2. Recommended Actions for Backend Engineer
1. **Set `interrupt_active = true` on Barge-In:**
   - In `cancel()` and `ControlEvent::Interrupt`, set `state.lock().interrupt_active = true` to immediately suppress subsequent in-flight chunks before the server responds.

---

### Sprint 139: Realtime Barge-In Server Notification for Gemini Live

- **Review Point:** `gemini_live.rs:190-223` — Gemini realtime (non-PTT) barge-in sends no server message
- **Severity:** 🟠 REAL COST AT SCALE (C60 — Cross-Provider Barge-In Protocol Consistency)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/gemini_live.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/gemini_live.rs#L190-L225)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Inconsistent Barge-In Protocol** (Confidence: 80%)
- **What the code actually does:** In non-PTT mode, `ControlEvent::Interrupt` skips sending any message over the WebSocket, whereas PTT mode sends activity boundary markers and Deepgram sends `{"type":"Clear"}`.
- **Why it's this way:** Assumption that Gemini server-side VAD alone will handle all non-PTT interruptions.

#### 2. Recommended Actions for Backend Engineer
1. **Transmit Server Interruption Frame:**
   - Send explicit cancellation / activity markers across both PTT and non-PTT barge-in events to stop server generation immediately.

---

### Sprint 140: Clear Resampling Configuration & Integration Test Coverage

- **Review Point:** `audio_bridge.rs:40-54`, `playback_bridge.rs:38-57` — Resampling path is unexercised / `requires_*_resampling` flags are misleading
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Latent Resampler Test Coverage)
- **Target File:** [`app/src-tauri/src/services/realtime/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latent Code Coverage Gap** (Confidence: 85%)
- **What the code actually does:** Since default sample rates match provider sample rates (16kHz in, 24kHz out), the `AudioResampler` path is completely bypassed in production, leaving `requires_output_resampling = true` flags ineffective.
- **Why it's this way:** Coincidence of default sample rates matching standard cloud provider rates.

#### 2. Recommended Actions for Backend Engineer
1. **Clarify Resampling Flags & Add Dedicated Unit Tests:**
   - Clean up misleading flag defaults and add unit tests exercising `AudioResampler::process_i16` with non-standard sample rates (e.g., 8kHz, 44.1kHz).

---

### Sprint 141: Async TCP & DNS Health Check Verification

- **Review Point:** `deepgram_live.rs:408-420`, `gemini_live.rs:505-511` — `health_check` uses blocking TCP + synchronous DNS
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Blocking DNS in Health Checks)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Minor Sync I/O Code Smell** (Confidence: 80%)
- **What the code actually does:** Uses `std::net::TcpStream::connect_timeout` and `to_socket_addrs`. While currently wrapped in `spawn_blocking` by IPC callers, direct sync calls could block execution threads.
- **Why it's this way:** Standard library sync networking helpers.

#### 2. Recommended Actions for Backend Engineer
1. **Convert to Tokio Async Network Primitives:**
   - Use `tokio::net::TcpStream::connect` with `tokio::time::timeout` for non-blocking health checks.

---

### Sprint 142: Deepgram PTT Activity Boundary Signalling

- **Review Point:** `deepgram_live.rs:632-639` — Deepgram `activity_start`/`activity_end` are silent no-ops (PTT)
- **Severity:** 🟠 REAL COST AT SCALE (C60 — PTT Activity Marker Parity)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/deepgram_live.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/deepgram_live.rs#L630-L645)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Protocol Documentation / Boundary Alignment** (Confidence: 80%)
- **What the code actually does:** `activity_start` and `activity_end` unconditionally return `Ok(())` for Deepgram without sending boundary tokens.
- **Why it's this way:** Deepgram Agent API operates continuous audio streaming.

#### 2. Recommended Actions for Backend Engineer
1. **Align Boundary Messages or Document Intentional No-Op:**
   - If supported by Deepgram Voice Agent API, send start/stop speech boundary markers; otherwise document clearly that Deepgram relies solely on server-side streaming VAD.


### Sprint 143: Eliminate Redundant JSON Parsing in Gemini Live Receive Loop

- **Review Point:** `gemini_live.rs:793, 268, 289, 403, 424` — Double JSON parse of every Gemini message in the hot path
- **Severity:** ⚡ OPTIMIZATION (C100 — 50% Reduction in WebSocket JSON Deserialization Cycles)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/gemini_live.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/gemini_live.rs#L260-L300)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Double Parse Waste** (Confidence: 100%)
- **What the code actually does:** `handle_gemini_server_message` deserializes the inbound text into `serde_json::Value`, and then the caller immediately re-parses the exact same string to check for `"goAway"` disconnect events.
- **Why it's this way:** Independent helper and receiver loop logic.

#### 2. Recommended Actions for Backend Engineer
1. **Parse Once and Pass `&serde_json::Value`:**
   - Parse inbound WebSocket text frames once into `Value`, check for `"goAway"`, and pass the parsed `&Value` reference directly into `handle_gemini_server_message`.

---

### Sprint 144: Scratch Buffer Reuse Across Audio & Playback Bridges

- **Review Point:** `audio_bridge.rs:90`, `deepgram_live.rs:123`, `gemini_live.rs:143`, `playback_bridge.rs:72` — Per-frame allocations in the audio hot path
- **Severity:** ⚡ OPTIMIZATION (C100 — Allocator Pressure Reduction on Streaming Audio)
- **Target File:** [`app/src-tauri/src/services/realtime/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Allocation Hot-Path Waste** (Confidence: 100%)
- **What the code actually does:** Allocates fresh `Vec<u8>` and `Vec<f32>` buffers for every 16ms/20ms PCM audio frame during both capture-to-WS and WS-to-playback processing.
- **Why it's this way:** Inline vector transformation chains.

#### 2. Recommended Actions for Backend Engineer
1. **Pre-Allocate Reusable Scratch Buffers:**
   - Use reusable thread-local scratch vectors or pre-allocated byte slices to amortize heap allocation overhead.

---

### Sprint 145: Micro-Batch Audio Frames for Gemini Realtime Input

- **Review Point:** `gemini_live.rs:141-164` — One WS text message per audio frame (Gemini)
- **Severity:** ⚡ OPTIMIZATION (C80 — WebSocket Packet Framing & Base64 Overhead Reduction)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/gemini_live.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/gemini_live.rs#L140-L165)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Framing Overhead Reduction** (Confidence: 80%)
- **What the code actually does:** Emits an individual WebSocket JSON text envelope containing a base64 string for every single 16ms PCM chunk, generating 60+ WS frames per second.
- **Why it's this way:** 1:1 chunk forwarding from audio queue.

#### 2. Recommended Actions for Backend Engineer
1. **Batch Frames into 40–80ms Envelopes:**
   - Accumulate 2–4 PCM frames into a single `realtimeInput` base64 payload to significantly lower small-packet network and JSON serialization overhead.

---

### Sprint 146: Pre-Reserve Resampler Capacity

- **Review Point:** `resampler.rs:51-58` — Resampler re-extends `input_buf` each call without reservation
- **Severity:** ⚡ OPTIMIZATION (C60 — Avoid Vector Reallocations in Resampler)
- **Target File:** [`app/src-tauri/src/services/realtime/resampler.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/resampler.rs#L45-L65)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Minor Buffer Reservation Optimization** (Confidence: 100%)
- **What the code actually does:** `input_buf` is dynamically extended on every resampling invocation without initial capacity reservation.
- **Why it's this way:** Simple dynamic vector usage.

#### 2. Recommended Actions for Backend Engineer
1. **Reserve Expected Frame Capacity:**
   - Call `self.input_buf.reserve(nbr_frames_needed)` once during `AudioResampler::new` or initialization.

---

### Sprint 147: Explicit Little-Endian PCM Encoding and Unsafe Pointer Elimination

- **Review Point:** `gemini_live.rs:143-145, 879` — Gemini PCM uses host-endian bytes and unsafe pointer casts
- **Severity:** 🟡 STYLISTIC (C100 — Safety and Big-Endian Architecture Correctness)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/gemini_live.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/gemini_live.rs#L140-L150)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Unsafe & Endian Portability Defect** (Confidence: 100%)
- **What the code actually does:** Uses `unsafe { std::slice::from_raw_parts(...) }` for encoding and `i16::from_ne_bytes` for decoding, relying on host endianness matching little-endian Gemini protocol.
- **Why it's this way:** Unsafe slice casting shortcut.

#### 2. Recommended Actions for Backend Engineer
1. **Use Safe `to_le_bytes` and `from_le_bytes`:**
   - Eliminate `unsafe` slice casts and replace `from_ne_bytes` with `i16::from_le_bytes` to match Deepgram implementation and Gemini spec explicitly.


### Sprint 148: Safe SocketAddr Parsing in Gemini Health Check

- **Review Point:** `gemini_live.rs:508` — `expect()` on a hardcoded constant parse
- **Severity:** 🟡 STYLISTIC (C100 — Panic-Free Health Check Fallback Address)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/gemini_live.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/gemini_live.rs#L505-L512)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Code Style Polish** (Confidence: 100%)
- **What the code actually does:** Calls `.parse().expect(...)` on `GEMINI_HEALTH_CHECK_FALLBACK_IP`. While the IP string is currently a valid constant, an accidental edit to the constant would panic.
- **Why it's this way:** Runtime string parsing of hardcoded IP.

#### 2. Recommended Actions for Backend Engineer
1. **Use Const SocketAddr or Safe Fallback:**
   - Define a static `SocketAddr` literal `SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(142, 250, 190, 42), 443))` to eliminate runtime parsing and potential panics.

---

### Sprint 149: Clarify IEEE 754 Audio Normalization Divisor

- **Review Point:** `mod.rs:23`, `resampler.rs:51`, `playback_bridge.rs:74` — i16→f32 divisor is `32768.0` (asymmetric range mapping)
- **Severity:** 🟡 STYLISTIC (C100 — Audio DSP Standard Convention Clarification)
- **Target File:** [`app/src-tauri/src/services/realtime/mod.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/mod.rs#L20-L25)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Standard DSP Convention** (Confidence: 100%)
- **What the code actually does:** Divides signed 16-bit integer PCM values by `32768.0` to map samples into the range `[-1.0, 0.999969]`.
- **Why it's this way:** Standard convention in CPAL, WebAudio, and libsndfile to avoid clipping on the negative extreme `-32768`.

#### 2. Recommended Actions for Backend Engineer
1. **Document DSP Normalization Standard:**
   - Add inline documentation explaining the `32768.0` divisor convention; retain existing value as mathematically sound.

---

### Sprint 150: Verify Idempotent `start_playback` in Playback Bridge

- **Review Point:** `playback_bridge.rs:78` — `playback_engine.start_playback()` called per chunk
- **Severity:** 🟡 STYLISTIC (C100 — Playback Trigger Idempotency Verification)
- **Target File:** [`app/src-tauri/src/services/realtime/playback_bridge.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/playback_bridge.rs#L75-L82)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Safe Idempotent Invocation** (Confidence: 100%)
- **What the code actually does:** `start_playback` performs an atomic compare-and-swap on `playback_active`, ensuring the CPAL output stream is active whenever chunks arrive.
- **Why it's this way:** Idempotent activation pattern.

#### 2. Recommended Actions for Backend Engineer
1. **Preserve Idempotent Activation:**
   - Retain current call as non-blocking and safe; no changes required.

---

### Sprint 151: Dynamic Resampling Triggering Based on Rate Mismatch

- **Review Point:** `audio_bridge.rs:40-54`, `playback_bridge.rs:38-57` — `requires_*_resampling` flags vs explicit sample rate mismatch
- **Severity:** ❓ UNSURE / QUESTION (Resampler Architecture Simplification)
- **Target File:** [`app/src-tauri/src/services/realtime/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Architectural Simplification** (Confidence: 100%)
- **What the code actually does:** Combines boolean flags with secondary sample rate equality checks, rendering the boolean flags ambiguous.
- **Why it's this way:** Defensive configuration layering.

#### 2. Recommended Actions for Backend Engineer
1. **Trigger Resampling on Rate Inequality:**
   - Replace manual bool flags with automatic resampling activation whenever `source_sample_rate != target_sample_rate`.

---

### Sprint 152: Graceful WebSocket Disconnection & Socket Drop

- **Review Point:** `deepgram_live.rs:622`, `gemini_live.rs:743` — `session.disconnect()` shutdown signal vs explicit WS Close frame
- **Severity:** ❓ UNSURE / QUESTION (WebSocket Teardown Lifecycle Safety)
- **Target File:** [`app/src-tauri/src/services/realtime/providers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/realtime/providers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Safe Teardown** (Confidence: 100%)
- **What the code actually does:** `disconnect()` signals the worker shutdown oneshot, causing the orchestrator to break and drop the underlying `WebSocketStream`, which initiates TCP FIN/TLS teardown.
- **Why it's this way:** Asynchronous clean shutdown via channel signaling.

#### 2. Recommended Actions for Backend Engineer
1. **Retain Channel Shutdown Pattern:**
   - Optionally send `Message::Close(None)` before task abortion if supported by provider endpoints; otherwise drop-on-abort remains safe.


## Module 10: STT & TTS Engines (`sprint-10.md`)

---

### Sprint 153: Prevent Intermediate Frame Loss in Partial STT Coalescing

- **Review Point:** `services/stt/actor.rs:44-72` — `coalesce_partials` silently discards intermediate partial audio
- **Severity:** 🔴 WILL BREAK (C100 on drop, C60 overall — Frame Loss on Delta Audio Inputs)
- **Target File:** [`app/src-tauri/src/services/stt/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/stt/actor.rs#L40-L75)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Audio Frame Loss Risk on Delta Input** (Confidence: 85%)
- **What the code actually does:** `coalesce_partials` drains consecutive `SttCommand::Partial` events from the channel and replaces `utterance` with the newest command's audio payload. If upstream feeds delta PCM frames, all intermediate chunks are permanently discarded.
- **Why it's this way:** Assumes full accumulated buffer is passed in each partial command.

#### 2. Recommended Actions for Backend Engineer
1. **Accumulate Frames or Feed Streaming Online Recognizer:**
   - Either accumulate delta audio frames into an internal worker buffer across turns, or feed audio continuously into `sherpa-onnx` streaming `OnlineStream` so intermediate frames are preserved.

---

### Sprint 154: Carry Streaming Engine State Across STT Chunks

- **Review Point:** `services/stt/providers/embedded.rs` & `nemotron_onnx.rs` — Offline STT engines re-decode from scratch each chunk
- **Severity:** 🔴 WILL BREAK (C100 on mechanism, C80 on latency — O(n²) Re-Decode Compute Blowup)
- **Target File:** [`app/src-tauri/src/services/stt/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/stt/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Compute & Streaming State Mismatch** (Confidence: 100%)
- **What the code actually does:** `transcribe_chunk` creates a fresh offline stream and decodes the entire passed buffer from scratch without carrying state across partial frames, leading to quadratic computational complexity.
- **Why it's this way:** Wrapping offline recognizers with a pseudo-streaming chunk interface.

#### 2. Recommended Actions for Backend Engineer
1. **Use Sherpa-ONNX `OnlineRecognizer`:**
   - Maintain the active `OnlineStream` across audio chunks (as established in the Sherpa-ONNX 1.13.6 transducer migration) so incremental recognition state is carried without full-buffer re-decoding.

---

### Sprint 155: Bounded Request & Connect Timeouts in Remote Chatterbox TTS

- **Review Point:** `services/tts/providers/chatterbox_remote.rs:31-35` — `ChatterboxRemote` has `timeout(None)` on a blocking client
- **Severity:** 🔴 WILL BREAK (C100 — Permanent TTS Actor Thread Stall on Network Hang)
- **Target File:** [`app/src-tauri/src/services/tts/providers/chatterbox_remote.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/providers/chatterbox_remote.rs#L25-L50)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Permanent Hang Vulnerability** (Confidence: 100%)
- **What the code actually does:** `reqwest::blocking::Client` is configured with `timeout(None)`. In `stream_pcm_response`, blocking `response.read()` calls will hang forever if the remote server stalls, wedging the single-threaded TTS worker actor indefinitely.
- **Why it's this way:** Omitted timeout configuration during client initialization.

#### 2. Recommended Actions for Backend Engineer
1. **Configure Bounded HTTP Timeouts:**
   - Set `.timeout(Some(Duration::from_secs(30)))` and `.connect_timeout(Duration::from_secs(5))` on the blocking HTTP client builder.

---

### Sprint 156: Purge Uncalled `SttProvider::transcribe` Trait Method

- **Review Point:** `services/stt/providers/embedded.rs:63-73` & `services/stt/mod.rs:33` — Full `SttProvider::transcribe` path is uncalled
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Dead Trait Interface Purge)
- **Target File:** [`app/src-tauri/src/services/stt/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/stt/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Trait Method** (Confidence: 100%)
- **What the code actually does:** Defines `pub fn transcribe(&mut self, ...)` on `SttProvider` and `SttEngine` traits, but all pipeline execution routes exclusively through `transcribe_chunk`.
- **Why it's this way:** Leftover interface from offline batch transcription.

#### 2. Recommended Actions for Backend Engineer
1. **Remove Unused Trait Method:**
   - Remove `SttProvider::transcribe` and standardize all provider invocations on the streaming `transcribe_chunk` API.

---

### Sprint 157: Purge Dead Audio Buffer in Embedded STT Provider

- **Review Point:** `services/stt/providers/embedded.rs:11, 54, 98, 108` — `stt_audio_buffer` is never written
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Dead Struct Field Elimination)
- **Target File:** [`app/src-tauri/src/services/stt/providers/embedded.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/stt/providers/embedded.rs#L6-L15)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Struct Field** (Confidence: 100%)
- **What the code actually does:** Declares and resets `stt_audio_buffer: Vec<f32>` but never pushes or extends samples into it.
- **Why it's this way:** Leftover buffer field from earlier batching implementation.

#### 2. Recommended Actions for Backend Engineer
1. **Remove Dead Buffer Field:**
   - Delete `stt_audio_buffer` from `EmbeddedSttProviderInner` and remove redundant `.clear()` calls.


### Sprint 158: Propagate Configured Speed to Remote Chatterbox TTS

- **Review Point:** `chatterbox_remote.rs:283-287, 230-234` — `ChatterboxRemote` silently ignores configured TTS speed
- **Severity:** 🟠 REAL COST AT SCALE (C100 — User Configured Speed Has No Effect)
- **Target File:** [`app/src-tauri/src/services/tts/providers/chatterbox_remote.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/providers/chatterbox_remote.rs#L275-L295)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Ignored Parameter Bug** (Confidence: 100%)
- **What the code actually does:** `self.speed` is stored in an atomic variable and updated via `set_speed()`, but `synthesize_chunk` omits `"speed"` from the JSON request payload sent to `/tts/stream-pcm`.
- **Why it's this way:** Incomplete request payload serialization.

#### 2. Recommended Actions for Backend Engineer
1. **Include Speed in Synthesis Payload:**
   - Add `"speed": f32::from_bits(self.speed.load(Ordering::Relaxed))` to the JSON payload, or apply client-side audio speed stretching via `apply_speed_stretch`.

---

### Sprint 159: Reuse Shared Tokio Runtime in EdgeTTS Provider

- **Review Point:** `edge_tts.rs:275-287, 294` — EdgeTTS creates a fresh Tokio runtime + WebSocket per synthesis turn
- **Severity:** 🟠 REAL COST AT SCALE (C100 — 20–100ms Latency Penalty and Thread Pool Churn Per Utterance)
- **Target File:** [`app/src-tauri/src/services/tts/providers/edge_tts.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/providers/edge_tts.rs#L270-L290)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latency & Thread Churn Defect** (Confidence: 100%)
- **What the code actually does:** Calls `tokio::runtime::Runtime::new()` inside `synthesize_chunk` for every single synthesis sentence, creating and destroying a full Tokio runtime and thread pool per turn.
- **Why it's this way:** Standalone runtime instantiation wrapper.

#### 2. Recommended Actions for Backend Engineer
1. **Use App Tokio Runtime Handle:**
   - Pass `tokio::runtime::Handle` into `EdgeTtsProvider` or use a shared static runtime instance to eliminate per-turn reactor creation overhead.

---

### Sprint 160: Incremental PCM Chunking in EdgeTTS Provider

- **Review Point:** `edge_tts.rs:222-253, 315-334` — EdgeTTS buffers the entire MP3 then emits one giant `TtsChunk`
- **Severity:** 🟠 REAL COST AT SCALE (C100 mechanism, C60 barge-in — Monolithic Audio Emission)
- **Target File:** [`app/src-tauri/src/services/tts/providers/edge_tts.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/providers/edge_tts.rs#L315-L345)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Monolithic Emission Defect** (Confidence: 100%)
- **What the code actually does:** Decodes the entire MP3 into a single memory buffer and emits it as one giant `VoxEvent::TtsChunk`, preventing mid-stream cancellation or incremental playback.
- **Why it's this way:** Whole-buffer decoding and emission.

#### 2. Recommended Actions for Backend Engineer
1. **Slice Samples into Standard 2048-Sample Chunks:**
   - Chunk `decoded.samples` into `TTS_CHUNK_SIZE` (2048 samples) and emit incrementally while checking the cancellation flag between chunks.

---

### Sprint 161: EdgeTTS Auth Token Resilience & Health Diagnostics

- **Review Point:** `edge_tts.rs:24-54` — EdgeTTS auth tokens are hardcoded byte arrays
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Upstream Authentication Fragility)
- **Target File:** [`app/src-tauri/src/services/tts/providers/edge_tts.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/providers/edge_tts.rs#L24-L55)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Upstream Token Fragility** (Confidence: 100%)
- **What the code actually does:** Trusted client tokens and GEC version strings are embedded as static constants, which will cause silent synthesis failures if Microsoft rotates the client handshake requirements.
- **Why it's this way:** Protocol reverse-engineering constants.

#### 2. Recommended Actions for Backend Engineer
1. **Add Token Health Probe & Descriptive Errors:**
   - Surface actionable error messages upon EdgeTTS WebSocket handshake failures prompting token updates.

---

### Sprint 162: Atomic Lock-Free Property Updates in Local TTS Providers

- **Review Point:** `chatterbox.rs:131-142`, `supertonic.rs:160-173` — `set_quality_steps`/`set_speed` contention audit
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Concurrency & Contention Audit)
- **Target File:** [`app/src-tauri/src/services/tts/providers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/providers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Safe Lock-Free Updates** (Confidence: 100%)
- **What the code actually does:** `set_quality_steps` and `set_speed` update atomic primitives (`AtomicU32`), ensuring zero lock contention with active audio generation.
- **Why it's this way:** Lock-free design.

#### 2. Recommended Actions for Backend Engineer
1. **Preserve Lock-Free Atomic Pattern:**
   - Retain current atomic update mechanisms; no code changes required.


### Sprint 163: Eliminate O(n²) Re-Decoding of Growing Buffer in Partial STT

- **Review Point:** `qwen_onnx.rs:88-123`, `nemotron_onnx.rs:33-66` — O(n²) re-decode of growing buffer each partial
- **Severity:** ⚡ OPTIMIZATION (C100 — Root STT Streaming Latency Bottleneck Elimination)
- **Target File:** [`app/src-tauri/src/services/stt/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/stt/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Latency Bottleneck (Resolved via Sherpa-ONNX 1.13.6)** (Confidence: 100%)
- **What the code actually does:** Re-running full offline speech recognition over a growing audio window on every partial frame creates quadratic $O(n^2)$ FLOP overhead.
- **Why it's this way:** Pseudo-streaming over offline models.

#### 2. Recommended Actions for Backend Engineer
1. **Leverage Sherpa-ONNX Online Stream State:**
   - Retain the active `OnlineRecognizer` stream state across incremental chunk arrivals, decoding incrementally in constant $O(n)$ time.

---

### Sprint 164: Fuse Filter & Decimation Allocations in Supertonic Resampling

- **Review Point:** `supertonic.rs:62-78` — `resample_44100_to_24000` allocates a full intermediate `Vec<f32>` every callback
- **Severity:** ⚡ OPTIMIZATION (C100 — Allocator Pressure Reduction in Audio Output Path)
- **Target File:** [`app/src-tauri/src/services/tts/providers/supertonic.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/providers/supertonic.rs#L60-L80)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Intermediate Heap Allocation Waste** (Confidence: 100%)
- **What the code actually does:** Materializes a full intermediate `filtered: Vec<f32>` across all 44.1kHz samples before performing decimation down to 24kHz.
- **Why it's this way:** Multi-stage vector transformation.

#### 2. Recommended Actions for Backend Engineer
1. **Fuse Filtering and Decimation:**
   - Process low-pass filtering and interpolation in a single pass without allocating intermediate vectors.

---

### Sprint 165: Eliminate Redundant Vector Allocations in TTS Speed Stretching

- **Review Point:** `chatterbox.rs:107-126`, `chatterbox_remote.rs:112-131` — `apply_speed` allocates new `Vec` per 2048-sample chunk
- **Severity:** ⚡ OPTIMIZATION (C100 — Zero-Copy Pass-Through at Normal Speed)
- **Target File:** [`app/src-tauri/src/services/tts/providers/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/providers/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Per-Chunk Vector Allocation Waste** (Confidence: 100%)
- **What the code actually does:** Calls `samples.to_vec()` even when `speed == 1.0`, allocating a heap vector for every single 2048-sample chunk.
- **Why it's this way:** Owned return type `Vec<f32>` on helper method.

#### 2. Recommended Actions for Backend Engineer
1. **Use `Cow<'_, [f32]>` or Scratch Destination Buffer:**
   - Return `Cow::Borrowed(samples)` when `speed == 1.0` and write into reusable scratch buffers when stretching audio.

---

### Sprint 166: Amortize String Allocations in Speech Recognition Strides

- **Review Point:** `nemotron_onnx.rs:34, 46-48, 60-62` — `transcribe_strides` string allocation overhead
- **Severity:** ⚡ OPTIMIZATION (C100 — String Allocation Amortization)
- **Target File:** [`app/src-tauri/src/services/stt/nemotron_onnx.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/stt/nemotron_onnx.rs#L50-L75)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed String Allocation Overhead** (Confidence: 100%)
- **What the code actually does:** Repeatedly creates intermediate `String` objects per stride.
- **Why it's this way:** Incremental string formatting.

#### 2. Recommended Actions for Backend Engineer
1. **Pre-Allocate Result Capacity:**
   - Use `String::with_capacity` or directly extract token slices from the `OnlineRecognizer` stream.

---

### Sprint 167: Standardize Constants in ONNX Speech Decoders

- **Review Point:** `nemotron_onnx.rs:30-45` & `qwen_onnx.rs` — Extract magic stride and window constants
- **Severity:** 🟡 STYLISTIC (C100 — Config Constant Documentation & Maintainability)
- **Target File:** [`app/src-tauri/src/services/stt/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/stt/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Constant Standardization Polish** (Confidence: 100%)
- **What the code actually does:** Configures Sherpa-ONNX model hyperparameters directly.
- **Why it's this way:** Inline engine initialization.

#### 2. Recommended Actions for Backend Engineer
1. **Declare Named Module Constants:**
   - Maintain named top-level constants (`SAMPLE_RATE`, `NEMOTRON_NUM_THREADS`) with explanatory doc comments.


### Sprint 168: Purge Dead Import in TTS Voice Module

- **Review Point:** `services/tts/voice.rs:8` — Unused import `use crate::symphonia_core::audio::Audio;`
- **Severity:** 🟡 STYLISTIC (C100 — Dead Import Cleanup)
- **Target File:** [`app/src-tauri/src/services/tts/voice.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/tts/voice.rs#L5-L12)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Import** (Confidence: 100%)
- **What the code actually does:** Imports `Audio` which is never referenced in the file.
- **Why it's this way:** Leftover symbol from Symphonia refactoring.

#### 2. Recommended Actions for Backend Engineer
1. **Remove Unused Import:**
   - Delete `use crate::symphonia_core::audio::Audio;`.

---

## Module 11: Monitoring, Setup, Evals & Benchmarks (`sprint-11.md`)

---

### Sprint 169: Wire Pipeline Latency Telemetry Producer

- **Review Point:** `monitoring/aggregator.rs:12, 110-127`, `collector.rs:121-124` — Latency telemetry is dead; `InteractionMetric` is never emitted
- **Severity:** 🔴 WILL BREAK (C100 — Core Sub-200ms KPI Silently Never Collected)
- **Target File:** [`app/src-tauri/src/monitoring/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead KPI Producer** (Confidence: 100%)
- **What the code actually does:** The aggregator handles `TelemetryEvent::InteractionMetric` and collector reads `latest_stt_ms`/`latest_ttft_ms`, but zero pipeline stages ever construct or emit this event. The metrics stay permanently `None`.
- **Why it's this way:** Telemetry producer wiring was deferred during pipeline restructuring.

#### 2. Recommended Actions for Backend Engineer
1. **Emit `InteractionMetric` on Turn Completion:**
   - In `services/pipeline/modular/` and `realtime/`, emit `TelemetryEvent::InteractionMetric` upon STT finalization, first LLM token arrival, and TTS start.

---

### Sprint 170: Non-Blocking Telemetry Dispatch in System Monitor

- **Review Point:** `monitoring/system_monitor.rs:39` — `system_monitor` does a blocking `crossbeam` send inside the async runtime
- **Severity:** 🟠 REAL COST AT SCALE (C90 — Tokio Worker Thread Stall Prevention)
- **Target File:** [`app/src-tauri/src/monitoring/system_monitor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/system_monitor.rs#L35-L45)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Blocking Call in Async Runtime** (Confidence: 90%)
- **What the code actually does:** Calls blocking `crossbeam_channel::Sender::send` inside a Tokio task spawned via `tauri::async_runtime::spawn`.
- **Why it's this way:** Simple synchronous send call.

#### 2. Recommended Actions for Backend Engineer
1. **Use `try_send` with Drop Warning:**
   - Replace `.send(...)` with `.try_send(...)` and log a warning if the telemetry channel is saturated.

---

### Sprint 171: Bounded Timeout and Abortable Model Downloads

- **Review Point:** `setup/model_manager.rs:62, 289, 304` — Model download has no timeout and cannot be truly cancelled
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Unbounded Setup Hang on Stalled Network)
- **Target File:** [`app/src-tauri/src/setup/model_manager.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/setup/model_manager.rs#L60-L70)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Download Hang Risk** (Confidence: 100%)
- **What the code actually does:** `Client::new()` has no timeout, and cancellation only checks a flag between chunk yields without aborting in-flight HTTP requests.
- **Why it's this way:** Default reqwest client instantiation.

#### 2. Recommended Actions for Backend Engineer
1. **Configure Request Timeouts & Task Abortion:**
   - Set `.timeout(Duration::from_secs(300))` and `.connect_timeout(Duration::from_secs(10))` on the client, and abort the download task future on cancellation.

---

### Sprint 172: Safe Fallback for Disk Space Verification

- **Review Point:** `setup/runtime_check.rs:101-116` — `check_disk_space` silently reports "OK" when no matching disk is found
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Silent False-Positive Setup Gate)
- **Target File:** [`app/src-tauri/src/setup/runtime_check.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/setup/runtime_check.rs#L100-L120)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Silent Pass Gap** (Confidence: 80%)
- **What the code actually does:** Returns `(100.0, 100.0, true)` when no matching mount point is found, allowing setup to proceed on unverified storage.
- **Why it's this way:** Overly optimistic fallback return.

#### 2. Recommended Actions for Backend Engineer
1. **Fail-Closed on Unresolved Mounts:**
   - Return `(0.0, 0.0, false)` with an explicit diagnostic log when disk space cannot be verified.

---

### Sprint 173: Verify Model Directory Contents Post-Extraction

- **Review Point:** `setup/runtime_check.rs:147-198`, `update_check.rs:147-159` — Archive integrity is not verified after extraction
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Partial/Corrupt Unpack Detection)
- **Target File:** [`app/src-tauri/src/setup/`](file:///home/addy/projects/apps/vox/app/src-tauri/src/setup/)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Extraction Integrity Gap** (Confidence: 80%)
- **What the code actually does:** Checks only the presence of the `.verified` marker without validating that extracted model directories are populated and uncorrupted.
- **Why it's this way:** Marker-only verification shortcut.

#### 2. Recommended Actions for Backend Engineer
1. **Assert Non-Empty Extracted Tree:**
   - Confirm expected model weight files exist and are non-empty before declaring the model verified.

---

### Sprint 174: Clean Partial Directories on Extraction Failures

- **Review Point:** `setup/model_manager.rs:218-252` — Extraction failure leaves a partially-extracted directory behind
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Model File Corruption on Retry)
- **Target File:** [`app/src-tauri/src/setup/model_manager.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/setup/model_manager.rs#L218-L255)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Stale File Leak** (Confidence: 80%)
- **What the code actually does:** Removes the temporary archive on extraction failure but leaves partially unpacked files in `extract_dest`.
- **Why it's this way:** Missing cleanup error handler.

#### 2. Recommended Actions for Backend Engineer
1. **Purge Incomplete Extraction Directory:**
   - Call `std::fs::remove_dir_all(&extract_dest)` when `do_extract` encounters an error.

---

### Sprint 175: Defer Old Model Cleanup Until Verification Succeeds

- **Review Point:** `setup/model_manager.rs:111, 379-433` — Old model version is deleted before the new one is verified
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Destructive Upgrade on Network Failure)
- **Target File:** [`app/src-tauri/src/setup/model_manager.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/setup/model_manager.rs#L105-L120)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Premature Deletion Risk** (Confidence: 80%)
- **What the code actually does:** Deletes the existing working model before downloading and verifying the new version.
- **Why it's this way:** Pre-clean upgrade sequence.

#### 2. Recommended Actions for Backend Engineer
1. **Perform Cleanup Post-Verification:**
   - Call `cleanup_old_versions` only after the new download and `.verified` marker creation succeed.

---

### Sprint 176: Wire `dropped_events` Counter in VAD Telemetry

- **Review Point:** `monitoring/aggregator.rs:50, 146-152`, `services/vad/actor.rs:160` — `dropped_events` counter is never incremented
- **Severity:** 🟠 REAL COST AT SCALE (C100 — Invisible Telemetry Saturation)
- **Target File:** [`app/src-tauri/src/services/vad/actor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/services/vad/actor.rs#L155-L165)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Dead Metric Counter** (Confidence: 100%)
- **What the code actually does:** The aggregator checks `dropped_events > 0` for warning logs, but the producer never increments the atomic counter on `try_send` drops.
- **Why it's this way:** Incomplete producer counter wiring.

#### 2. Recommended Actions for Backend Engineer
1. **Increment Counter on Send Failure:**
   - Increment `dropped_events.fetch_add(1, Ordering::Relaxed)` when `try_send` fails.

---

### Sprint 177: Poison-Resilient `RwLock` Access in Monitoring Runtime State

- **Review Point:** `monitoring/runtime_state.rs:22-33` — `RwLock` poisoning silently kills monitoring
- **Severity:** 🟠 REAL COST AT SCALE (C80 — Permanent Telemetry Blinding on Panic)
- **Target File:** [`app/src-tauri/src/monitoring/runtime_state.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/runtime_state.rs#L20-L40)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Poison Fragility** (Confidence: 80%)
- **What the code actually does:** Swallows poisoned lock errors via `.ok()`, causing all future telemetry reads to return `None` permanently after any panic.
- **Why it's this way:** Standard `.ok()` lock pattern.

#### 2. Recommended Actions for Backend Engineer
1. **Recover from Poison:**
   - Use `.unwrap_or_else(|e| e.into_inner())` on `history.write()` and `latest.write()`.

---

### Sprint 178: Persist Fetched Manifests to Local Disk Cache

- **Review Point:** `setup/update_check.rs:48-58, 61-73` — `update_check` manifest cache is read but never written
- **Severity:** 🟠 REAL COST AT SCALE (C90 — Dead Offline Cache)
- **Target File:** [`app/src-tauri/src/setup/update_check.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/setup/update_check.rs#L45-L75)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Unwritten Cache** (Confidence: 90%)
- **What the code actually does:** Reads local manifest cache files on startup, but successful network fetches never write to those cache files.
- **Why it's this way:** Missing cache write step post-fetch.

#### 2. Recommended Actions for Backend Engineer
1. **Write Manifest to Disk Cache:**
   - Save fetched manifest JSON to `cache/app_manifest.json` and `cache/models_manifest.json` on successful downloads.

---

### Sprint 179: Continuous Per-Webview RAM Metrics in 10Hz Snapshot Feed

- **Review Point:** `monitoring/collector.rs:181-183`, `profiler.rs:250-251` — Per-webview RAM is always `None` in live snapshot
- **Severity:** ⚡ OPTIMIZATION (C100 — Live Webview Memory Visibility on 8GB Constraint)
- **Target File:** [`app/src-tauri/src/monitoring/collector.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/collector.rs#L175-L190)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Missing Metric in Feed** (Confidence: 100%)
- **What the code actually does:** Unconditionally assigns `None` to `main_webview_ram_mb`, `tray_webview_ram_mb`, and `wizard_webview_ram_mb` in continuous snapshots.
- **Why it's this way:** Profiler computation separation.

#### 2. Recommended Actions for Backend Engineer
1. **Populate Cached Webview RAM:**
   - Store periodic profiler RAM snapshots in atomic/shared state and expose them through `RuntimeSnapshot`.

---

### Sprint 180: Bounded History Query Window in Monitoring State

- **Review Point:** `monitoring/runtime_state.rs:41-46` — `get_history` clones entire 600-snapshot deque on every IPC call
- **Severity:** ⚡ OPTIMIZATION (C100 — Allocation & CPU Reduction on History Polls)
- **Target File:** [`app/src-tauri/src/monitoring/runtime_state.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/runtime_state.rs#L40-L50)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Large Vector Clone** (Confidence: 100%)
- **What the code actually does:** Clones all 600 large `RuntimeSnapshot` structs on every IPC request.
- **Why it's this way:** Full history clone helper.

#### 2. Recommended Actions for Backend Engineer
1. **Support Windowed Queries:**
   - Accept a `limit` parameter or return a bounded slice (e.g. last 120 samples) to prevent unnecessary allocations.

---

### Sprint 181: Reuse System Metrics in System Monitor

- **Review Point:** `monitoring/system_monitor.rs:146-147` — Redundant syscalls in `system_monitor` emit path
- **Severity:** ⚡ OPTIMIZATION (C100 — System Metric Query Deduplication)
- **Target File:** [`app/src-tauri/src/monitoring/system_monitor.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/system_monitor.rs#L140-L155)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Duplicate Query Polish** (Confidence: 100%)
- **What the code actually does:** Re-queries `sys.total_memory()` and CPU count inside `emit_system_stats` after already querying them upstream.
- **Why it's this way:** Independent helper invocation.

#### 2. Recommended Actions for Backend Engineer
1. **Pass Pre-Captured Values:**
   - Pass existing `total_ram_mb` and `cpu_cores` into `emit_system_stats`.

---

### Sprint 182: Explicit Owner Handling in Telemetry Emitter

- **Review Point:** `monitoring/telemetry_emitter.rs:72-79` — `get_target_window` masks unknown owners
- **Severity:** 🟡 STYLISTIC (C80 — Telemetry Window Routing Clarity)
- **Target File:** [`app/src-tauri/src/monitoring/telemetry_emitter.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/telemetry_emitter.rs#L70-L80)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Fallback Routing Masking** (Confidence: 80%)
- **What the code actually does:** Maps all unknown interaction owners to `"tray"`.
- **Why it's this way:** Defensive enum fallback.

#### 2. Recommended Actions for Backend Engineer
1. **Log Unknown Owner Variants:**
   - Explicitly log unknown enum discriminants before applying default window routing.

---

### Sprint 183: Stable Webview Process Ordering

- **Review Point:** `monitoring/profiler.rs:181-217` — Webview role assignment can mislabel at identical start times
- **Severity:** 🟡 STYLISTIC (C60 — Webview Memory Profiling Attribution)
- **Target File:** [`app/src-tauri/src/monitoring/profiler.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/profiler.rs#L180-L220)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Identification Heuristic Polish** (Confidence: 60%)
- **What the code actually does:** Orders webviews by `(start_time, pid)`, which can misattribute roles if webviews start simultaneously.
- **Why it's this way:** Process start time heuristic.

#### 2. Recommended Actions for Backend Engineer
1. **Match on Process Arguments / Window Titles:**
   - Use command line arguments or window handles for deterministic webview mapping.

---

### Sprint 184: Use Standard Temp Directory in Profiler

- **Review Point:** `monitoring/profiler.rs:57-85` — `resolve_temp_dir` creates `./temp` next to executable as fallback
- **Severity:** 🟡 STYLISTIC (C80 — Packaging & Filesystem Hygiene)
- **Target File:** [`app/src-tauri/src/monitoring/profiler.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/profiler.rs#L55-L90)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed CWD Fallback Polish** (Confidence: 80%)
- **What the code actually does:** Creates `./temp` relative to CWD if standard paths are missing.
- **Why it's this way:** Local fallback pathing.

#### 2. Recommended Actions for Backend Engineer
1. **Fallback to `std::env::temp_dir()`:**
   - Standardize fallback temp path resolution on OS temporary directories.

---

### Sprint 185: Fine-Grained Model Update Reporting

- **Review Point:** `setup/update_check.rs:160-163` — `check_model_updates` reports whole groups as outdated
- **Severity:** 🟡 STYLISTIC (C100 — Granular Update Status Reporting)
- **Target File:** [`app/src-tauri/src/setup/update_check.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/setup/update_check.rs#L155-L170)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Reporting Granularity Polish** (Confidence: 100%)
- **What the code actually does:** Flags entire model groups as outdated when any single model inside requires an update.
- **Why it's this way:** Group-level aggregation.

#### 2. Recommended Actions for Backend Engineer
1. **Report Per-Model Updates:**
   - Include individual model identifiers alongside group names in update check results.

---

### Sprint 186: SemVer Version Comparison in Update Checker

- **Review Point:** `setup/update_check.rs:14-45` — `is_newer_version` prerelease handling is heuristic
- **Severity:** 🟡 STYLISTIC (C70 — Robust SemVer Comparison)
- **Target File:** [`app/src-tauri/src/setup/update_check.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/setup/update_check.rs#L15-L45)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed SemVer Heuristic Polish** (Confidence: 70%)
- **What the code actually does:** Uses custom string splitting and hyphen checking for prerelease comparisons.
- **Why it's this way:** Custom version parsing helper.

#### 2. Recommended Actions for Backend Engineer
1. **Use `semver::Version`:**
   - Standardize version comparison on the `semver` crate parser.

---

### Sprint 187: Synchronize Snapshot Documentation with Code

- **Review Point:** `monitoring/snapshot.rs:43` — Doc drift referencing non-existent window owners
- **Severity:** 🟡 STYLISTIC (C100 — Documentation Drift Fix)
- **Target File:** [`app/src-tauri/src/monitoring/snapshot.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/monitoring/snapshot.rs#L40-L50)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Doc Drift** (Confidence: 100%)
- **What the code actually does:** Comment mentions "Tray, MainWindow, Ptt" while `InteractionOwner` only has `Dictation` and `Assistant`.
- **Why it's this way:** Stale comment from earlier UI state design.

#### 2. Recommended Actions for Backend Engineer
1. **Update Doc Comments:**
   - Align comment with current `InteractionOwner` enum variants (`Dictation`, `Assistant`).

---

### Sprint 188: Clarify Manifest Verification Contracts

- **Review Point:** `setup/runtime_check.rs:201` — `models_verified` empty manifest condition
- **Severity:** ❓ UNSURE / QUESTION (Manifest Verification Logic Clarification)
- **Target File:** [`app/src-tauri/src/setup/runtime_check.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/src/setup/runtime_check.rs#L195-L210)

#### 1. Feedback Review & Root Cause
- **Verdict:** ✅ **Confirmed Contract Clarification** (Confidence: 100%)
- **What the code actually does:** Returns `models_verified = false` if `manifest.model_groups` is empty.
- **Why it's this way:** Defensive check requiring at least one model group.

#### 2. Recommended Actions for Backend Engineer
1. **Document Verification Behavior:**
   - Confirm that a valid installation requires non-empty model groups; maintain `!m.model_groups.is_empty()` check.
