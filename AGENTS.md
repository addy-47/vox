# AGENTS.md — Vox Workspace Rules

---

## 1.. MANDATORY RULE: AGENTS.md Sync Hook

> 🛑 **MANDATORY POST-TASK HOOK (NON-NEGOTIABLE) — TWO STEPS, IN ORDER:**
>
> **Step 1 — Always: Append to `AGENTS.md` Section 5 only.**
> After every completed task, add a concise bullet to Section 5 describing what changed. Do NOT simultaneously write to `docs/`, `recent_work.md`, or any other file — `AGENTS.md` is the only target. [Note: SKip this for tasks that lack accountability or can impact a future agent's work]
>
> **Step 2 — Only when approaching 175 lines: Migrate Section 5.**
> After appending, check `AGENTS.md` total line count. If it is at or above **110 lines** (the warning threshold before the 175-line ceiling)
>
> 1. Migrate **only the delta**: append to `docs/plans/<current_phase>/recent_work.md` under a `## Past Work (YYYY-MM-DD)` heading just the Section 5 entries added since the last migration. Dedupe against the file first — never re-archive entries already present there (snapshotting the whole Section 5 duplicates history).
> 2. Replace Section 5 in `AGENTS.md` with a compact 3–5 bullet summary of only the highest-level milestones.
> 3. Keep the deep link at the top of Section 5: `📖 Full History: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/<current_phase>/recent_work.md)`.
>
> **This is the complete hook. Nothing else is mandatory on every task.**

---

## 2. Workspace Directory Map

| Path                      | Purpose                                             | Rules                                                                                                 |
| ------------------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `app/src-tauri/src/`      | Purpose Rust source                                 | No test logic. No benchmarks.                                                                         |
| `app/src-tauri/tests/`    | Integration tests                                   | Named `<feature>_test.rs`. Tests public API only.                                                     |
| `app/src-tauri/benches/`  | Performance benchmarks                              | Named `<feature>_bench.rs`. `harness = false` + custom `fn main()`.                                   |
| `app/src-tauri/scripts/`  | Build & verification scripts                        | `verify-targets.sh` is the compile gate (see §3.0). Env SSOT for the NDK toolchain.                 |
| `.agents/rules/`          | Role-specific agent instruction files               | Read relevant file before acting in that role. `android-style-guide.md` is mandatory for Android-touching work (see §4.2). |
| `docs/plans/`             | Architecture specs and phase plans                  | Source of truth for specs. Do not contradict. `phase13/` is active.                                  |
| `docs/features/`          | Implemented feature ledgers                         | Update after completing features.                                                                     |
| `sandbox/`                | Scratch space for experiments, evaluations, scripts | Non-production code. Results in `sandbox/results/`. Datasets in `sandbox/datasets/`.                  |
| `temp/`                   | Ephemeral runtime files: logs, raw LLM outputs      | `temp/.env` (API keys). `temp/server.txt` (remote GPU server creds). Not versioned.                   |
| `submodules/`             | Git submodules                                      | `chatterbox-rs`, `query-sieve-rs`, `distilbert-query-classifier`, `vox-models`, `nexus-rs`.       |
| `~/.vox/models/`          | Local model weights                                 | Canonical manifest: `~/.vox/models/models_manifest.json`.                                             |
**Remote GPU server:** `root@[IP_ADDRESS]` (creds in `temp/server.txt`). Ollama . **Never kill running server processes.**

---

## 3 Execution & Testing Invariants (All Agents)

0. **Compile Gate — always use the script, never a bare `cargo check` on a cross target:**
   ```bash
   cd app/src-tauri && ./scripts/verify-targets.sh
   ```
1. **Sequential Execution with Release flag:** Run benchmarks, evals ,test suites or any cargo command strictly one at a time to prevent CPU, memory, and I/O contention and always use the `--release` flag.
2. **Isolated Test Runner (`cargo-nextest`):** Use the cmd listed below and its variations . 
   ```bash
   RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --release --test-threads=1 --no-fail-fast
   ```
   > ⏱️ **Full Suite Baseline Run Time:** ~45.5s test execution without cold compilation , always use a timeout or cron to monitor test runs and avoid hangs.
4. **Test with External Dependencies(`#[ignore]`):** Cloud or remote server tests must be run manually only with explicit user approval: `cargo nextest -- --ignored`.
---

## 4. Invariants ,Rules and Specs [MUST FOLLOW]

### 4.1 Critical Architectural & Logical Invariants (Non-Negotiable Concepts)
**Zero Backward Compatibility (ZBC):** Backward compatibility is not a requirement unless explicitly stated. Break, replace, or redesign existing interfaces when that produces the better architecture. Never introduce compatibility layers, legacy paths, 
> 🛑 **MANDATORY PRE-EXPLORATION HOOK:**
> - Before any codebase exploration, architecture lookup, graph query, or broad search, read `.agents/rules/codebase-memory-mcp.md` and follow its graph-first workflow.
>
**Local Turso Database CLI (`tursodb`):** For inspecting or querying the local Turso SQLite database (`~/.vox/vox.db`), use the native `tursodb` CLI tool:
   ```bash
   tursodb ~/.vox/vox.db "SELECT name FROM sqlite_master WHERE type='table';"
   ```

### 4.2. HARD GATE: Code Modification Gate

> 🛑 **MANDATORY PRE-MODIFICATION HOOK:**
> - You MUST read the corresponding style guide and engineer rule file for the specific area you are working on located in .agents/rules/.
> - **If the change touches anything that must compile on Android** — `cpal`, tray/menu, dictation, `utils/paths.rs`, model loading, Cargo.toml target tables, or any `cfg(` — you MUST also read [`.agents/rules/android-style-guide.md`](file:///home/addy/projects/apps/vox/.agents/rules/android-style-guide.md). Android support is **incomplete**; several confirmed traps there have already caused false "done" reports.

### 4.3 Specifications, Behavioral Contracts & Non-Drift Hook [MANDATORY]

> 🛑 **MANDATORY SPEC ALIGNMENT HOOK:**
> Every agent working on Vox must adhere strictly to the approved specifications.
> 1. **Specification Divergence / Additions**: If an agent needs to implement, modify, or add behavior, commands, or schemas that diverge from or are not defined in the relevant spec, it MUST STOP and ask the user for approval. If approved, the agent MUST update the spec artifact FIRST before authoring or modifying code. Specifications must NEVER quietly drift from code.
> 2. **Code Divergence / Legacy Code**: If existing code implements nuances or legacy behaviors not defined in the spec, the agent MUST confirm with the user first before either pruning the code or updating the spec to capture the behavior.

### 4.4 Active Specifications Ledger
Authoritative system specifications reside in [`docs/specs/`](file:///home/addy/projects/apps/vox/docs/specs/). Key target specs include **[storage-spec.md](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md)** (SSOT filesystem, 3-way config decomposition, and runtime paths), **[events-spec.md](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md)** (SSOT pipeline & 6-domain contracts), **[dictation-spec.md](file:///home/addy/projects/apps/vox/docs/specs/dictation-spec.md)** (dictation & OS output), **[harness-spec.md](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)** (LLM runtime), **[db-spec.md](file:///home/addy/projects/apps/vox/docs/specs/db-spec.md)**, **[memory-spec.md](file:///home/addy/projects/apps/vox/docs/specs/memory-spec.md)**, **[ownership-spec.md](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md)**, etc.

---

## 5. Current Phase Summary

> 📖 **Phase 13 (active):** [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase13/recent_work.md)
> Phase 12 Archive: [recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md

- **Phase 13 scope:** Android platform preparation (re-homed from Phase 12 on 2026-10-09) plus the critical on-device feature set — remote control, disfluency ML model, speaker lock. Feature designs stay in `docs/plans/wip/`; only the Android plan was promoted into `phase13/`.
- **Android Batch 4 complete and honestly verified:** Desktop-only crates (`enigo`, `arboard`, `tauri-plugin-positioner`, tray) sit behind platform facades; `router.rs` carries zero inline `#[cfg]`. `setup/remote_server.rs` is gated behind `#[cfg(desktop)]` with a typed mobile stub, while remote *runtime* stays available on mobile over plain HTTP. **Both targets compile at 0 warnings / 0 errors.**
- **Dual-target gate is now automated:** `app/src-tauri/scripts/verify-targets.sh` is the sole compile gate (both targets, sequential, `--release`, `-D warnings`, defensive `unset SYSROOT`). Verified by mutation in both directions. `.agents/rules/android-style-guide.md` documents the confirmed traps — notably that `target_os = "linux"` is **false** on Android (61 dead blocks across 14 files) while `cfg(unix)` is true.
- **Android APK now builds (`arm64-v8a`):** Phase 2 produced a signed, installable 207.6 MB APK — only `lib/arm64-v8a/`, `minSdk 24`. This required six native-toolchain fixes that `cargo check` structurally could not see (it compiles but never links): `clippy-driver` honoring `SYSROOT`, **duplicate `ggml_*` symbols** (llama.cpp is vendored twice), `crate-type` emitting no `.so`, bindgen having no sysroot inside Gradle, chatterbox-rs missing an Android toolchain file, and Gradle building all 4 ABIs. `turso`/`libsql` and `llama.cpp` are now **VERIFIED** for arm64. Three non-obvious rules: a config-file rustflag is silently overridden by Tauri (use `build.rs`); cargo finds config from **CWD**, not `--manifest-path`, so `[env]` must live in `$CARGO_HOME/config.toml`; Gradle's env is a hardcoded allowlist. Always build with `-t aarch64`.
- **Phase 3 done — `ort` NNAPI enabled, and reproducibility closed:** `nnapi` had to be added as a **target-scoped** dependency, not globally — `ort-sys` demands an exact feature-set match against `dist.tsv`, so a global `nnapi` resolves Android correctly but breaks the desktop build. The packaged `libonnxruntime.so` is byte-identical either way (the resolver already picked the NNAPI row by fallthrough); what was actually missing was the Rust-side `ort::ep::NNAPI`, gated behind the feature. **Verify by feature graph, never by binary diff** — `cargo tree -e features -i ort-sys` gives 3 `nnapi` refs on `--target aarch64-linux-android` and 0 on desktop. Separately, the chatterbox-rs Android toolchain fix is **upstream** as `c9d17bd`; the local `[patch]` and its submodule dependency are gone, so a fresh clone builds the APK unaided.
- **First real device run (Phase 4, partial — 2026-10-10):** installs on a POCO redwoodin / Android 14 and reaches `[BOOTSTRAP] Runtime Ready`. Three launch crashes fixed in sequence: (1) a thread spawned above `paths::init()`, whose panic hook then called `paths::get()` and destroyed its own message; (2) R8 minification JNI renaming fixed by `gen/android/app/jni-keep.pro`; (3) `$HOME` missing fixed with `paths::init_from_app_data_dir`. Traps 18-21.
- **Android Fast Iteration, Adaptive Icons & Mobile Shell (2026-10-10):** single-window routing deadlock solved (mobile mounts `WizardRoot` directly, dev skip bypass); adaptive icons across 5 densities with `#090D16` background; desktop chrome unmounted on mobile with `EdgeNav` as a native bottom bar; USB reverse HMR via `scripts/android-dev.sh`. Both targets clean.
- **Doc debt — open:** `.agents/rules/android-style-guide.md` stops at **Pitfall 7**. Phases 2–4 produced at least 14 more ($CARGO_HOME `[env]` injection, the Gradle env allowlist, the double link, `stderr` as the only live channel on release builds, `home_dir()` returning `/`, R8 keep rules), and they exist only in the plan. AGENTS.md §4.2 makes that guide mandatory for Android-touching work, so the migration is not optional cleanup.
- **Android plan repaired; backend batches 2 & 8 planned (2026-10-10):** the plan's own sequencing table was stale (claimed Batch 2 next, 3/7/8 not started — all contradicted by its Phase 2–4 results below). Batch 3 (CI APK) is **dropped by user decision**; `android-deploy.sh` fills that role locally at lower latency. Three backend defects found by direct read, none of which any compile gate can catch: (1) **`gen/android/.../AndroidManifest.xml` declares only `INTERNET` — no `RECORD_AUDIO`**, so no input stream can open on Android; (2) **capture is hardcoded to `f32`** (`services/audio/device.rs:140`, also `tts/voice.rs:319`) with no format negotiation; (3) **`MODEL_DOWNLOAD_TIMEOUT_SECS = 300` is a reqwest *total* deadline**, so any download slower than ~30 MB/s fails outright — a desktop bug too. Batch 8 is bigger than "a `Range` header": **SHA-256 cannot resume over an appended file** without first hashing the prefix, and a server ignoring `Range` returns `200`, which corrupts the file silently if appended to. `tauri.conf.json` has **no `bundle.android` block** — the single-ABI build came from `-t aarch64` on the CLI, so the restriction is not persisted. `download_and_hash` had **zero test coverage**; tests were written before the implementation, not after.
- **Batch 8 shipped — resumable model downloads (2026-10-10):** `download_and_hash` now resumes. Five things had to be right together, and the two non-obvious ones are the reason a naive `Range` header is not enough. **SHA-256 is not resumable over an appended file** — the hash covers the whole byte stream, so the existing prefix must be streamed through the hasher before new bytes are folded in, or a resumed download hashes only its tail and fails verification forever. And **a server that ignores `Range` replies `200` with the whole body**, so appending would duplicate every byte on disk and corrupt the file with no error surfaced; `resume_offset_for` branches on `PARTIAL_CONTENT` and truncates for everything else. `416` means the partial is stale or corrupt, so it is discarded and retried once from zero. The partial `.tmp` is now **retained on every failure** — deleting it was what made every interrupted download restart from byte zero. Progress is absolute, including the pre-transfer emit (a resume at 90% used to snap the UI to 0%). New `ModelManager::with_emitter` is the egress seam that made progress observable headlessly.
- **Download timeout corrected (2026-10-10):** `MODEL_DOWNLOAD_TIMEOUT_SECS = 300` was a reqwest **total** request deadline, which caps a multi-GB transfer outright rather than detecting a stall. Replaced with `MODEL_DOWNLOAD_IDLE_TIMEOUT_SECS = 60` via `ClientBuilder::read_timeout` — a per-read idle deadline, so a stalled transfer fails and a slow one survives. **This was a desktop bug too.**
- **Storage preflight added (2026-10-10):** `utils/disk.rs::available_space_at` picks the longest mount-point prefix (so a models dir on a mounted volume resolves to that volume, not `/`) and `setup_model` fails with an actionable message — "Not enough storage to download X. Needs N more, but only M is free." An *unknown* free-space reading never blocks a download, so platforms that do not report it are unaffected.
- **`RECORD_AUDIO` declared (2026-10-10):** added to `AndroidManifest.xml` alongside `MODIFY_AUDIO_SETTINGS`. Verified against wry 0.55's `RustWebChromeClient.onPermissionRequest` (`src/android/kotlin/RustWebChromeClient.kt:94-118`), which requests exactly those two for `AUDIO_CAPTURE` and grants them app-wide via `permissionLauncher` — so one frontend `getUserMedia` grant also unblocks the native Oboe path. Previously no input stream could open on Android at all; it failed at stream open rather than at launch, so no compile gate could catch it.
- **Test harness for HTTP (2026-10-10):** `tests/common/http_server.rs` is a hand-rolled `tokio::net::TcpListener` origin with deliberately-misbehaving modes (honours `Range`, ignores it, `416`s), because the interesting cases are *wrong* servers, which mocking libraries make awkward. No new dependency. It caught a real spec bug during authoring: a `206` must advertise the **range** length in `Content-Length`, not the whole entity (RFC 9110 §15.3.7). **14 model-manager tests, all 8 resume guards mutation-verified.**
- **Android UI, Onboarding Redesign & Placeholder Parity (2026-10-10, WIP):**
  - **Memory & History Empty State Parity (Verified):** Fixed major WebGL collision bug where `MemoryGraph` pink core sphere rendered over `EmptyState`. `MemoryGraph` and `GraphControlDock` now mount strictly when `facts.length > 0`. Memory and History empty states unified using shared `EmptyState` (`icon={Ghost}`, `max-w-sm border-0 bg-transparent`). Verified via CDP inspection on live device: both cards render at identical geometry (`top: 333.36px`, `height: 200px`, `icon top: 390.95px`, `parent top: 35px`, `parent height: 796.73px`).
  - **SSOT Onboarding Configuration:** Removed routing bypass overrides in `App.tsx`; `system.setup_completed` in `settings.jsonc` (`/data/user/0/com.addy.Vox/files/config/settings.jsonc`) is the sole SSOT. Desktop `vox.db` (6.0 MB) pushed to `/data/user/0/com.addy.Vox/files/data/db/vox.db`.
  - **Mobile Wizard Overhaul (In Progress):** Vertically centered orb in stage on `MobileHeroStep` & `MobileIntelligenceStep`. Vertically stacked footer buttons (`Continue` primary `h-14` on top, `Back` secondary `h-12` on bottom). Structured 5 linear wizard steps (`MobileHeroStep`, `MobileIntelligenceStep`, `MobilePermissionsStep`, `MobileModelsStep`, `MobileReadyStep`).
  - **Mobile Shell Polish:** Eliminated Settings card corner snapping and height reflow flicker during auto-save in `SettingsCardWrapper.tsx`. Implemented horizontal swipe navigation between primary tabs and safe bottom inset padding `pb-[calc(60px+env(safe-area-inset-bottom))]` in `ResponsiveLayout.tsx`. TypeScript compiles clean (`pnpm tsc --noEmit` exit code 0).