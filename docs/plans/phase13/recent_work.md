# Phase 13 — Recent Work Ledger (Android & Critical Features)

Phase 13 takes over **Android platform work** (moved out of Phase 12) and adds the
**critical Vox feature set** that must work on-device.

> 📖 **Phase 12 Archive:** [../phase12/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase12/recent_work.md)
> 📖 **Phase 11 Archive:** [../phase11/recent_work.md](file:///home/addy/projects/apps/vox/docs/plans/phase11/recent_work.md)

---

## Phase 13 Scope

| Area | Source of truth | Status |
|---|---|---|
| **Android platform preparation** | [`android-platform-preparation-plan.md`](file:///home/addy/projects/apps/vox/docs/plans/phase13/android-platform-preparation-plan.md) | 🟡 Phases 0–1 done; Phase 2 (first `arm64-v8a` link) next |
| **Remote control** | [`../wip/remote-control-android.md`](file:///home/addy/projects/apps/vox/docs/plans/wip/remote-control-android.md) | ⬜ Not started (stays in `wip/`) |
| **Disfluency ML model** | [`../wip/dictation-cleanup-engine.md`](file:///home/addy/projects/apps/vox/docs/plans/wip/dictation-cleanup-engine.md) | ⬜ Not started (stays in `wip/`) |
| **Speaker lock** | [`../wip/speaker-lock-architecture.md`](file:///home/addy/projects/apps/vox/docs/plans/wip/speaker-lock-architecture.md) | ⬜ Not started (stays in `wip/`) |
| **Denoise integration** | [`../wip/denoise-integration.md`](file:///home/addy/projects/apps/vox/docs/plans/wip/denoise-integration.md) | ⬜ Not started (stays in `wip/`) |

The feature WIP docs deliberately remain in [`../wip/`](../wip/) — they are design
sketches, not plans. Only the Android platform plan was promoted into `phase13/`.

---

## Android Work Carried Over From Phase 12

These entries were originally logged in Phase 12 and moved here on 2026-10-09 when
Android work was re-homed to Phase 13.

- **Android Platform Preparation (Batch 1 done):** `reqwest` moved to `default-features = false` with `charset`/`http2`/`system-proxy` re-listed explicitly — removes `openssl-sys`/`native-tls` from the target graph (the `ort` → `ureq` path is a host-side build-dep, not an Android blocker). Unused `zbus` deleted. Verified: desktop `cargo check` + `clippy --all-targets` clean; Android cross-compile reaches 102 crates.
- **Android Toolchain Contract (no committed config):** The Tauri CLI injects `CARGO_TARGET_<TRIPLE>_LINKER`/`RUSTFLAGS` from `ANDROID_HOME`/`NDK_HOME`, so **no** absolute NDK path belongs in `.cargo/config.toml`. Bare `cargo` builds need `ANDROID_NDK`, `CC_/CXX_/AR_aarch64_linux_android`, and `BINDGEN_EXTRA_CLANG_ARGS_aarch64-linux-android="--sysroot=$SYSROOT -D__ANDROID_API__=24"` (the last one is the only item the CLI does not set).
- **⛔ `enigo` Blocks the Android Build:** Vox keys Linux deps on `cfg(target_os = "linux")`, which excludes Android, so `enigo`'s `x11rb` feature drops out and it hits a hard `compile_error!`. No Android input backend exists (X11/Wayland/XDO/libei are all desktop). Requires Batch 4's dictation exclusion (~14 files). Android `cargo check` gate therefore moved from Batch 1 to Batch 4.
- **Android Plan Corrections:** Audit of the Phase 12.2 plan refuted or amended 20+ claims — NDK is r29 not 27.2, no linker block was ever committed, `nexuss` needed repinning to **0.1.2** (not 0.1.1, which predates the char-boundary fix), `llama-cpp-sys` auto-disables OpenMP on Android so §7.1 needs no decision, `ort` **does** ship an `aarch64-linux-android` NNAPI artifact (the `nnapi` feature just isn't enabled), the Batch 4 LOC column was shifted by one row, 35 shortcuts not 51, and Batch 6's `WizardFooter`/`prefers-reduced-motion` items were already done.

---

## Past Work (2026-10-09)

- **Android Batch 4 Re-verification:** Audited a "Batch 4 complete" claim and found it overstated — the compile gate held, but **6 of 10** planned exclusions in plan §4.1 had never been made. Confirmed the `enigo`/`arboard` blocker was genuinely resolved (`router.rs` has zero inline `#[cfg]`; dictation + tray correctly excluded). Cleared **14 Android-only warnings** by `cfg`-gating desktop-only imports and dead constants across `state.rs`, `resource_scope.rs`, `crash.rs`, `hardware.rs`, `toast.rs`, `utils/mod.rs`, `lib.rs`, and `services/memory/ml/mod.rs`, with desktop behavior byte-identical.
- **⛔ Toolchain Trap — `SYSROOT` Breaks Android Clippy:** `rustc` **ignores** the `SYSROOT` env var; `clippy-driver` **honors** it. Because `cargo clippy` compiles this crate's build script for the *host* with `clippy-driver`, exporting `SYSROOT=<NDK>/.../sysroot` makes clippy hunt for host `x86_64` std inside the Android sysroot and fail at `E0463: can't find crate for std`. `cargo check --target` was unaffected, which is why the defect hid behind a green build. Verified by isolated reproduction.
- **Phase 0 — Verification Automation:** Added `app/src-tauri/scripts/verify-targets.sh` as the sole compile gate (both targets, sequential, `--release`, `-D warnings`, sysroot inlined, defensive `unset SYSROOT`). Replaced AGENTS.md §3's broken one-line gate. Added `.agents/rules/android-pitfalls.md` (10 confirmed traps, chiefly that `target_os = "linux"` is **false** on Android — 61 dead blocks across 14 files, while `cfg(unix)` is true). Gate was mutation-proven in both directions: survives a hostile `SYSROOT` export, and fails on a single injected unused import.
- **Phase 0 — IPC Registration Invariant:** New `ipc_command_registration_test.rs` catches unregistered IPC commands **and** deleted `#[cfg(not(desktop))]` stub arms. Its first version was a **false green** (passed while a mobile stub arm was deleted, because the desktop arm still supplied the name). Rewritten with brace-depth `cfg` scope tracking after finding that `#[cfg(desktop)]` sits on the *module* and an intervening `#[cfg(target_os="linux")]` `use` line was shadowing the guard. Mutation-verified in both directions.
- **Phase 1 — Batch 4 Closed:** `setup/remote_server.rs` gated behind `#[cfg(desktop)]` with a typed inline mobile stub, keeping `ipc/catalog.rs` free of platform conditionals. `<RemoteServerSetup>` panel hidden behind `isDesktop()` in `ModelsCard.tsx`. Remote *runtime* (plain HTTP via `services/llm/transport`) deliberately still works on mobile — only *provisioning* needs `ssh`. **Batch 4 is now genuinely complete.** `tsc --noEmit`, `pnpm build`, and both clippy targets exit 0.
- **Plan Corrections:** `remote_server.rs` re-characterized — it is remote *provisioning* (spawns `ssh`, streams `setup_server.sh` phases), **not** the inference path. **Batch 8 re-scoped L → M:** the "flat model layout is the real blocker" claim was backwards — `paths.rs:347-349` (`model_dir()`) makes loading fully portable today, with zero `home_dir`/absolute-path assumptions under `services/{stt,llm,tts}/`. The real defect is that `setup/model_manager.rs` is not resumable: no `Range` header (`:291`) and `.tmp` is **deleted** on all 5 failure paths (`:155,189,235,250,256`), so every interrupted download restarts from zero. That is an S fix (resume) plus S (storage preflight), and roughly 70% of it also improves desktop. Also corrected: the `.cargo/config.toml` `[target.aarch64-linux-android]` block now exists (bare linker name, correctly portable), and the registered IPC command count is **70** (72 attributes, since `ipc/tray.rs` defines two commands under dual `#[cfg]` arms), not 69.
- **Execution Order Established:** Phases 0→6 recorded. Batch 3 (APK on device) precedes Batch 2 (audio seam) so the seam is driven by observation rather than assumption. **Phase 2 — first `arm64-v8a` *release link*** is the highest-information action remaining: `cargo check` passes but nothing has been proven to *link*, and it settles the two claims still marked UNVERIFIED (`turso`/`libsql` §7.2, `ort` NNAPI §7.3).
