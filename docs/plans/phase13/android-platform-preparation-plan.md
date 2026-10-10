# Phase 13.1 — Android Platform Preparation Plan

> **Re-homed from Phase 12 on 2026-10-09.** This plan began as "Phase 12.2" but
> Android platform work now lives in Phase 13, alongside the critical feature set
> (remote control, disfluency ML, speaker lock) that must run on-device. The file
> moved with `git mv` — history is preserved. Cross-references to
> `phase12/recent_work.md` were repointed to `phase13/recent_work.md`. The feature
> WIP documents deliberately remain in `docs/plans/wip/`.

> **Goal:** A single push to `master` builds an installable APK. The app runs on Android end-to-end: first-run wizard completes, models download, mic capture and playback work, and inference runs locally on-device.
>
> **Scope note — read this first.** This plan contains **no remote-execution work.** It is platform *preparation* plus local Android enablement. Every batch is classified as either:
>
> - 🟢 **Desktop-neutral** — removes platform coupling, adds no Android logic, desktop behavior byte-identical.
> - 🟡 **Android-additive** — adds Android-only branches behind a flag, desktop path provably untouched.
> - ⛔ **Android-necessary** — cannot be avoided to make Android work; desktop must be guarded, not altered.
>
> **Companion document:** [`../wip/remote-control-android.md`](file:///home/addy/projects/apps/vox/docs/plans/wip/remote-control-android.md). This plan unblocks its execution-environment question; it does not implement it.

---

## Explicitly Out of Scope

| Item | Reason |
|---|---|
| Remote execution / audio streaming | Separate feature. See `wip/remote-control-android.md`. |
| Harness Tauri-decoupling (`TurnExecutionRequest.app: AppHandle`) | Only needed for a non-Tauri execution host. Tauri runs fine on Android via `wry`'s Android backend. Deferred. |
| Frontend transport abstraction behind `src/services/` | Same reason. `invoke()` over Tauri's Android bridge is sufficient. Deferred. |
| Volume-key dictation hotkey | Requires an Android `Service` + `MediaSession`/accessibility layer. Out of scope for "app runs end-to-end." Tracked in §9. |
| Local *vs* remote mode toggle | Not needed until remote exists. |

---

## Product Decisions (2026-10-09, user-confirmed)

These supersede earlier assumptions in this document.

| Decision | Answer | Consequence |
|---|---|---|
| **On-device capability scope** | **Fully local: STT + TTS + VAD on device** | Batch 7 (native packaging) and Batch 8 (model delivery) are both **required**. |
| **`llama.cpp` on the Android target graph** | **Keep it; cross-compile it now** | The M-L Batch 7 item stands. Do not drop it for a remote-LLM-only Android build. |
| **Batch ordering** | **Batch 3 before Batch 2** | Do not do the audio seam until a real APK has run on a device. |
| **CI enforcement** | **Script + rules file only, no CI yet** | Verification is script-driven until Batch 3 lands. |
| **Remote inference** | Not the Android story | Remote LLM already works over plain HTTP (`services/llm/transport/`). It stays available but is not the mobile path. |

### What `setup/remote_server.rs` actually is — corrected

An earlier revision of this plan described it only as "spawns `ssh`" and implied it was part of the remote-execution story. **That is wrong.**

`setup/remote_server.rs` is a **one-time remote-server provisioning helper**, not the inference path:

1. Takes `connection_string` (`root@IP`), optional port and identity key
2. Spawns `ssh … bash -s -- <remote_path> <server_port>` (`:88`)
3. Pipes `setup_server.sh` (5.6K, seven phases: download → extract → verify → smoke test) over stdin
4. Parses stdout `Phase N` / `Smoke test passed` into `ModelProgress` events (`:55-75`)

**Runtime inference against that box needs no `ssh` at all.** `services/llm/transport/config.rs:8-13` exposes `TransportType::{ChatCompletions, OllamaNative, Responses}` over plain HTTP. An Android build can already talk to a remote box with zero Rust changes.

**Consequence:** provisioning a GPU box is a desktop-admin task — you provision from a laptop, the phone then connects over HTTP. So `remote_server.rs` is correctly gated off mobile (§4.1a). If in-app provisioning is ever wanted, it needs a pure-Rust SSH client (`russh`) replacing the `ssh` subprocess; that is a feature, not a portability fix, and belongs in `wip/remote-control-android.md`.

---

## Verified Baseline

Established by direct inspection and by an actual cross-compile attempt. Not assumed.

| Claim | Evidence |
|---|---|
| Cargo resolves the full tree for Android | ⚠️ **Was mis-stated.** `cargo metadata` has no `--target` flag in cargo 1.94; the equivalent is `--filter-platform`. The plan's cited command never ran. Resolution does work — `cargo check --target aarch64-linux-android` proceeds past dependency resolution. |
| NDK toolchain wiring works | ⚠️ **Was mis-stated.** NDK is **r29** (`29.0.13846066`), not 27.2. No `[target.aarch64-linux-android]` block was ever added to `.cargo/config.toml` — the file contains desktop targets only. See §1.3 for why no block should be committed. |
| `cpal` has an Android backend | `cpal-0.15.3/Cargo.toml:106-120` → `jni`, `ndk`, `oboe`, `ndk-context` |
| `sherpa-onnx` ships an Android path | `sherpa-onnx-sys-1.13.8/build.rs:64-72` → copies `.so` to Tauri `jniLibs` for Gradle |
| `ort` has Android API-level features | `ort-sys-2.0.0-rc.13/Cargo.toml` → `api-17` … `api-28` |
| `wry` has a full Android backend | `wry-0.55.0/src/android/`, deps at `Cargo.toml:242-264` |
| Mobile entry point already present | `app/src-tauri/src/lib.rs:113` → `#[cfg_attr(mobile, tauri::mobile_entry_point)]` |
| `zbus` is declared but unused | Zero usages across `app/src-tauri/src/` |
| Only 14 of 205 Rust files carry `target_os` cfgs | `#[cfg(target_os)]` census |

**Current compile blocker:** `zbus` (unused dependency) and `reqwest`'s default `default-tls` feature, which pulls `native-tls` → `openssl-sys`. Both resolved in Batch 1.

**Second blocker, discovered during Batch 1 implementation — `enigo` cannot compile for Android.** This is not a Batch 1 item and it invalidates Batch 1's stated gate. See §1.5.

**Both blockers are now resolved.** Commit `6310dacf` re-keyed the `enigo`/`arboard` dependency block on `cfg(not(any(target_os = "android", target_os = "ios")))` and excluded the dictation modules behind `cfg(desktop)`, so `enigo` is off the Android target graph entirely. Verified: `cargo clippy --target aarch64-linux-android --lib --release` exits 0. The open question of *whether dictation should exist on Android at all* (Open Decision 3) is now answered by necessity — it does not, and Android is assistant-only.

---

## Sequencing at a Glance

| Batch | Scope | Class | Effort | Gate | Status |
|---|---|---|---|---|---|
| 1 | Dependency hygiene | 🟢 Desktop-neutral | S | `cargo check` green on desktop; `openssl-sys` off the Android target graph | ✅ Done |
| 4 | Desktop-only code exclusion | 🟡 Android-additive | M | **Android `cargo check` green**; desktop bundles unchanged | ✅ **Done** (compile gate in `6310dacf`; `remote_server` closed in Phase 1) |
| 2 | Audio backend seam | 🟢 Desktop-neutral | M | Desktop audio regression tests pass | ⬜ Next |
| 3 | CI APK pipeline | 🟢 Desktop-neutral | M | **APK installs on a physical device** | ⬜ Not started |
| 5 | Frontend capability layer | 🟡 Android-additive | M | Desktop visual regression unchanged | ⬜ Not started |
| 6 | Wizard responsiveness | 🟡 Android-additive | M | First-run completes on a phone | 🟡 Partially done (mobile wizard flow shipped in `6310dacf`) |
| 7 | Native library packaging | ⛔ Android-necessary | M-L | `llama.cpp` + `turso` link for `arm64-v8a` | ⬜ Not started |
| 8 | Model delivery on mobile | ⛔ Android-necessary | L | Models download and load on-device | ⬜ Not started |
| 9 | WebGL thermal tuning | 🟡 Android-additive | M | Sustained fps on a mid-range device | ⬜ Not started |

**Critical path:** Batch 3 is the highest-value gate. It converts every subsequent batch from guesswork into a build log. Do not defer it.

**Status legend:** ✅ gate met · 🟡 partial · ⬜ not started. Statuses are set against a re-verification run of both targets (see §Verification).

---

## Batch 1 — Dependency Hygiene 🟢

**Class:** Desktop-neutral. Removes dead weight and an unused TLS backend. No Android logic.
**Effort:** S
**Blast radius:** `app/src-tauri/Cargo.toml`, `app/src-tauri/Cargo.lock`

### 1.1 Disable `reqwest` default TLS

`app/src-tauri/Cargo.toml:38`

```toml
# Current — pulls default-tls → native-tls → openssl-sys
reqwest = { version = "0.12", features = ["stream", "rustls-tls", "json", "blocking"] }

# Target (as implemented)
reqwest = { version = "0.12", default-features = false, features = ["stream", "rustls-tls", "charset", "http2", "system-proxy", "json", "blocking"] }
```

**Rationale:** Rust enables default features unless told otherwise. reqwest's `default` set is `[default-tls, charset, http2, system-proxy]` — **four** features, not one. `default-tls` is OpenSSL, which does not exist on Android (BoringSSL is API-incompatible).

> **⚠️ Correction to the original plan.** The first draft of this section proposed `default-features = false` with only the pre-existing feature list. That silently drops `charset`, `http2`, and `system-proxy` as well, which is a real desktop regression: it would drop HTTP/2 from every LLM/provider call and disable corporate-proxy support. It would not have failed the build — `.text()` still compiles without `charset` via a lossy-UTF-8 fallback (`reqwest-0.12.28/src/async_impl/response.rs:164-173`) — so this would have shipped silently. `charset`, `http2`, and `system-proxy` must be re-listed explicitly.

**Desktop safety:** `rustls-tls` was already explicitly enabled, and the three re-listed features restore the previous behavior exactly. The only delta is that OpenSSL is gone, which also drops the OpenSSL native dependency from desktop bundles.

**Note on the second `openssl-sys` root.** `openssl-sys` is *also* reachable via `ort` → `ort-sys` → `ureq` → `native-tls`. That path is a **build-dependency** (`ort-sys-2.0.0-rc.13/Cargo.toml:144` is `[build-dependencies.ureq]`), so it compiles for the host, never for the Android target, and is not a blocker. Verified with `cargo tree -e normal -i native-tls`: after this change, `native-tls` is absent from the target graph entirely.

**Regression check:** all `services/llm/transport/*` provider tests — `ollama.rs`, `chat_completions.rs`, `responses.rs`, `sse.rs` — plus `services/realtime/`.

### 1.2 Delete `zbus`

**Rationale:** Declared at `Cargo.toml:70`, zero usages in `src/`. Dead dependency.

**Desktop safety:** none — nothing references it.

### 1.3 Android toolchain wiring — no committed config change

**⚠️ The original instruction here was wrong and has been removed.** It said to commit a `[target.aarch64-linux-android]` block to `app/src-tauri/.cargo/config.toml`, and asserted such a block "already received during the spike." Neither was true at the time — the file contained desktop targets only. More importantly, **committing an absolute NDK path is wrong**: it hardcodes `/home/addy/...`, which cannot resolve on a CI runner or any other machine.

**Update (commit `6310dacf`): a block now exists, and it is correct.** `app/src-tauri/.cargo/config.toml:28-29` contains:

```toml
[target.aarch64-linux-android]
linker = "aarch64-linux-android24-clang"
```

The linker is a **bare name**, not an absolute path. Cargo resolves a bare linker through `$PATH`, so exporting the NDK's `prebuilt/<host>/bin` directory is the entire setup and the file stays portable across machines and CI runners. This is the right form of the original instruction and it is now in place.

**The rest of the toolchain is supplied by the build driver, not by cargo config.** The Tauri CLI (v2.11.0) resolves the NDK itself and injects the target-specific environment variables:

- It reads `ANDROID_HOME` / `ANDROID_SDK_ROOT`, and finds the NDK revision from `source.properties` (`Pkg.Revision`) — see `cargo_mobile2::android::{env, ndk, source_props}`.
- It emits `CARGO_TARGET_<TRIPLE>_LINKER` and `CARGO_TARGET_<TRIPLE>_RUSTFLAGS` (template constant `CARGO_TARGET__LINKER_RUSTFLAGS`).
- It maps `aarch64-linux-android` → ABI `arm64-v8a`, adds `-landroid -lOpenSLES`, and requires `libc++_shared.so`.

Cargo natively reads `CARGO_TARGET_<TRIPLE>_*` env vars with the same precedence as `config.toml`, so the env route works for a bare `cargo` invocation too, and `env`-based config is what makes CI portable.

**Required environment (verified working — `llama-cpp-sys-4` compiles for `aarch64-linux-android` with exactly this set):**

```bash
NDK=~/Android/Sdk/ndk/<version>          # r29.x
NDKB="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"
SYSROOT="$NDK/toolchains/llvm/prebuilt/linux-x86_64/sysroot"

export ANDROID_NDK="$NDK"                # llama-cpp-sys build.rs hard-requires this
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDKB/aarch64-linux-android24-clang"
export CC_aarch64_linux_android="$NDKB/aarch64-linux-android24-clang"
export CXX_aarch64_linux_android="$NDKB/aarch64-linux-android24-clang++"
export AR_aarch64_linux_android="$NDKB/llvm-ar"
export BINDGEN_EXTRA_CLANG_ARGS_aarch64-linux-android="--sysroot=$SYSROOT -D__ANDROID_API__=24"
```

Three of these are non-obvious and each one is a hard failure if missing:

1. `ANDROID_NDK` — `llama-cpp-sys-4-0.2.61/build.rs:1889` calls `.expect()` on it.
2. `BINDGEN_EXTRA_CLANG_ARGS_aarch64-linux-android` with `--sysroot` — `build.rs:1294` passes only `--target=<triple>` to bindgen with no sysroot, so without this it parses **host** `/usr/include` and dies on `bits/wordsize.h not found`.
3. `-D__ANDROID_API__=24` — NDK r29's libc++ references `pthread_cond_clockwait`, which does not exist below API 24. The Rust triple `aarch64-linux-android` carries no API level, so bindgen defaults to 21 and fails on `use of undeclared identifier 'pthread_cond_clockwait'`. `llama-cpp-sys` does not expose this itself; it can only be injected here.

In CI, `tauri android build` sets the first six automatically from `ANDROID_HOME`. **The `-D__ANDROID_API__` bindgen argument is the one thing the CLI does not set**, so a CI job doing a bare `cargo build --target aarch64-linux-android` must export it (or use NDK ≤ 27, whose libc++ predates this symbol).

**Desktop safety:** nothing committed, nothing to regress.

#### ⛔ Do NOT export `SYSROOT` — it breaks every `cargo clippy` invocation

This is a **confirmed** defect in the recipe above, found while re-verifying Batch 4. `SYSROOT` must be inlined into `BINDGEN_EXTRA_CLANG_ARGS` and never exported on its own.

| | Honors the `SYSROOT` env var? |
|---|---|
| `rustc` | **No** — ignores it, always uses the host/toolchain sysroot |
| `clippy-driver` | **Yes** — treats it as an authoritative sysroot override |

Because `cargo clippy` compiles the Vox **build script for the host** with `clippy-driver`, exporting `SYSROOT=<NDK>/.../sysroot` makes clippy look for the *host* `x86_64-unknown-linux-gnu` std inside the *Android* sysroot, and every Android clippy run dies at:

```
error[E0463]: can't find crate for `std`
  = note: the `x86_64-unknown-linux-gnu` target may not be installed
error: could not compile `Vox` (build script) due to 1 previous error
```

`cargo check --target aarch64-linux-android` is unaffected (it uses `rustc`), which is why the failure went unseen while earlier batches were verified with `cargo check` only. **Verified working env for both cargo and clippy:**

```bash
NDK=~/Android/Sdk/ndk/29.0.13846066
NDKB="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"
export ANDROID_NDK="$NDK"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDKB/aarch64-linux-android24-clang"
export CC_aarch64_linux_android="$NDKB/aarch64-linux-android24-clang"
export CXX_aarch64_linux_android="$NDKB/aarch64-linux-android24-clang++"
export AR_aarch64_linux_android="$NDKB/llvm-ar"
# NOTE: sysroot inlined, NOT exported
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--sysroot=$NDK/toolchains/llvm/prebuilt/linux-x86_64/sysroot -D__ANDROID_API__=24"
```

### 1.4 `nexuss` — repinned to 0.1.2 ✅ (was mis-stated as "already applied" at 0.1.1)

**⚠️ The original text claimed this was "Already applied" at `version = "0.1.1"`.** That was wrong twice over: it had not been applied, and **0.1.1 would have been the wrong version** — it predates the `N0/P1` char-boundary fix and would have dropped it.

**Now actually done, correctly.** `app/src-tauri/Cargo.toml:50` is `nexus = { package = "nexuss", version = "0.1.2" }`, resolved from crates.io (`Cargo.lock:4366-4369`, checksum `63b5813...`). 0.1.2 is the release published *from* this fork, so it carries the fix. The fix is confirmed present in the in-tree fork at `submodules/nexus-rs/src/extraction/cleaner.rs:33` (`while idx > 0 && !html.is_char_boundary(idx)`).

No Android-specific risk: a registry dep cross-compiles identically to the path dep it replaced. Verified — `cargo clippy --all-targets` clean on the repinned tree.

### 1.5 ⛔ Blocker: `enigo` cannot compile for Android — Batch 1's gate is unreachable

**This is the significant finding from implementing Batch 1, and it contradicts the plan's sequencing.**

`Cargo.toml:104` keys the Linux dependency block on `cfg(target_os = "linux")`. On Android, `target_os` is `"android"`, so **that block does not apply** — and the `enigo` entry inside it (which enables the `x11rb` feature) is silently dropped. Meanwhile `enigo-0.2.1` compiles its Linux backend on Android (`enigo/src/lib.rs:107` gates on `all(unix, not(target_os = "macos"))`), and then hits a hard stop:

```
error: either feature `wayland`, `x11rb`, `xdo` or `libei` must be enabled
       for this crate when using linux
  --> enigo-0.2.1/src/linux/mod.rs:15
```

There is no Android workaround. X11, Wayland, XDO, and libei are all desktop Linux input stacks; none exist on Android. Enabling one of them anyway would make the build go green while producing a runtime failure — a fake green, and explicitly not an acceptable way through this gate.

**Consequence for sequencing.** Batch 1's stated gate is "`cargo check` green on desktop **and** Android". That gate cannot be met by Batch 1. Excluding `enigo` requires gating `services/dictation/input.rs`, which cascades through at minimum:

`services/dictation/mod.rs` → `pipeline/dictation/**` → `pipeline/{router, atomics, mod}.rs` → `pipeline/assistant/session.rs` → `config/{settings, mutation, dispatch, persistence, files, mod}.rs` → `ipc/{tray, persistence}.rs` → `core/state.rs` → `utils/paths.rs` → `lib.rs`

That is Batch 4. It is also entangled with **Open Decision 3** (whether dictation exists on Android at all), which is unanswered. Batch 1 is therefore correctly scoped as *desktop-only* plus "removes the OpenSSL blocker"; the Android `cargo check` gate must move to the end of Batch 4.

**Verified progress toward that gate:** with §1.1, §1.2, §1.3 and §1.5's env set, `cargo check --target aarch64-linux-android` compiles **102 crates** and reaches `enigo`. `llama-cpp-sys-4` — the plan's biggest unknown — cross-compiles cleanly.

---

## Batch 2 — Audio Backend Seam 🟢 — **NEXT**

**Class:** Desktop-neutral. Introduces a trait; the cpal implementation is unchanged.
**Effort:** M
**Blast radius:** `services/audio/`, `services/tts/voice.rs`, `ipc/audio.rs`
**Prerequisite:** Batch 4's remaining `remote_server` item (§4.1a) is independent and can proceed in parallel.

### 2.1 Introduce `AudioCapture` / `AudioSink`

**Problem:** Vox has traits for STT, TTS, VAD, and LLM. It has **none** for audio. `cpal` is called directly, and worse, the concrete type is baked into struct layouts:

- `PlaybackEngine` stores `cpal::Stream` inline — `services/audio/playback.rs:36`
- `AudioStream` stores `Option<cpal::Stream>` — `services/audio/device.rs:20`
- Both carry `unsafe impl Send/Sync` to cross thread boundaries — `playback.rs:39-40`, `device.rs:23-24`

**Change:** Define the two traits, box the streams behind them. `cpal` becomes an implementation detail of the desktop adapter.

**Desktop safety:** `services/audio/sink.rs` (the realtime callback) already has **zero** cpal references and is pure DSP — it does not move. Capture and playback keep their existing logic verbatim behind the trait.

**Why now:** this is the last structural blocker to swapping audio backends. Doing it before any Android audio work means Android audio is a new file rather than a refactor of working code.

### 2.2 Collapse duplicate host resolution

**Problem:** `cpal::default_host()` is called independently at three sites, bypassing the existing `resolve_audio_host()`:
- `services/tts/voice.rs:301` (voice-cloning recorder)
- `ipc/audio.rs:44` (device enumeration)
- `setup/runtime_check.rs:166` (runtime report)

**Change:** route all three through `resolve_audio_host()` (`services/audio/device.rs:119`).

> **⚠️ Correction.** The original text listed **four** bypass sites and cited `device.rs:120` as one of them. That line is the *body* of `resolve_audio_host()` itself, not a bypass of it. There are three genuine bypasses.

**Desktop safety:** pure deduplication. No behavior change.

---

## Batch 3 — CI APK Pipeline 🟢

**Class:** Desktop-neutral. Build plumbing only.
**Effort:** M
**Blast radius:** `.github/workflows/`

### 3.1 Establish the goal

One push to `master` produces **both** desktop bundles and an installable APK. The four existing workflows (`release-linux.yml`, `release-macos.yml`, `release-windows.yml`) currently trigger on tags only; the Android job must be reachable from a plain master push.

### 3.2 Add `build-android.yml`

Required steps, modeled on the existing release workflows:

1. Checkout with `fetch-depth: 0`
2. Node 22 + pnpm 9 (match existing workflows)
3. `pnpm install` in `app/`
4. Rust toolchain + `rustup target add aarch64-linux-android`
5. Android SDK + NDK install, with `ANDROID_HOME` exported
6. `pnpm tauri android init` (once; afterwards the generated `gen/android` is committed)
7. `pnpm tauri android build --apk`

**Desktop safety:** new file. Existing workflows untouched.

### 3.3 Known obstacles to resolve during this batch

- `tauri android init` requires a `tauri.conf.json` `bundle.android` block and a `gen/android` Gradle project that does not yet exist in this repo.
- Icon and splash assets must be Android-dimensioned; current icons are desktop-only.
- `frontendDist` (`../dist`) must be produced before the Gradle assemble step.
- Signing config: start with a debug keystore so the pipeline is green, then add release signing.

**Gate:** an APK installs on a physical device and launches. Even a blank screen is success at this stage.

---

## Batch 4 — Desktop-Only Code Exclusion 🟡 — **PARTIALLY COMPLETE**

**Class:** Android-additive. Desktop behavior unchanged; Android omits these paths.
**Effort:** M
**Blast radius:** broad — **one file per commit**

### 4.0 Actual state as of `6310dacf` — the "Batch 4 complete" claim was wrong

A previous agent reported this batch complete. Re-verification shows the **compile gate is met** but **6 of the 10 paths in §4.1 were never excluded**. Both targets now build clean (0 warnings, 0 errors), so the residue is behavioral, not a build failure.

| §4.1 path | Intended | Actual | Effect on Android |
|---|---|---|---|
| `services/dictation/**` | exclude | ✅ `#[cfg(desktop)]` on all 5 submodules + a `#[cfg(not(desktop))]` stub | Correctly absent |
| `pipeline/dictation/**` | exclude | ✅ `#[cfg(desktop)]` + `#[cfg(not(desktop))]` stub | Correctly absent |
| `tray.rs` | exclude | ✅ `#[cfg(desktop)]` / `#[cfg(not(desktop))]` | Correctly absent |
| `ipc/tray.rs` | exclude | ✅ same | Correctly absent |
| `window_customizer.rs` | exclude | ⚠️ **not excluded** — but self-guards via `#[cfg(target_os = "linux")]` at `:14` | **Harmless.** Compiles, no-op. Exclusion was over-broad. |
| `toast.rs` | exclude | ⚠️ **not excluded** — falls to the `#[cfg(not(any(linux, macos, windows)))]` no-op branch | **Acceptable** — logs instead of shelling out. `notify-send`/`osascript`/`powershell` are never spawned. |
| `monitoring/resource_scope.rs` | exclude | ⚠️ **not excluded** — `/proc` + cgroup reads are `#[cfg(target_os = "linux")]`-gated | **Needs review** — see below. |
| `setup/remote_server.rs` | exclude | ⚠️ **not excluded** — **zero `cfg` tags in the file** | **Live defect.** Spawns `ssh`, which does not exist on Android. Compiles, fails at runtime. |
| `utils/crash.rs` | exclude | ⚠️ **not excluded** — signal handler is `#[cfg(target_os = "linux")]` | **Acceptable** — no-op on Android; panic hook in `lib.rs` still covers Rust panics. |
| `utils/hardware.rs` | exclude | ⚠️ **not excluded** — `/dev/nvidia0` probe is `#[cfg(target_os = "linux")]` | **Acceptable** — reports no local GPU. |

Also done: `AppState.hud_menu_item` is guarded (`core/state.rs:13,136`), `router.rs` has **zero** inline `#[cfg]` tags and dispatches straight to `super::dictation::handle_event` / `super::assistant::handle_event`, and all **69** IPC command handlers stay registered on every target. The `Cargo.toml` block at `:103` re-keys `enigo`/`arboard`/`tauri-plugin-positioner` off Android.

**Status: ✅ CLOSED.** Only `setup/remote_server.rs` was a genuine defect, and it is gated as of Phase 1 (§4.1a). The other five unexcluded paths (`toast.rs`, `window_customizer.rs`, `utils/crash.rs`, `utils/hardware.rs`, `monitoring/resource_scope.rs`) compile correctly on Android via existing `#[cfg]` fallback branches — **they should stay, not be excluded.** This batch is done.

### 4.1 What to exclude

| Path | Reason | LOC |
|---|---|---|
| `services/dictation/**` | enigo keystroke injection, arboard clipboard, gsettings global hotkeys, Unix-socket triggers. None exist on Android. | 1,133 |
| `pipeline/dictation/**` | The dictation track's output half pastes into other apps. Meaningless on a phone. | 741 |
| `tray.rs` | System tray; `gtk::prelude`, `cairo::Region` | 234 |
| `ipc/tray.rs` | System tray IPC | 155 |
| `window_customizer.rs` | GTK/WebKit2GTK pinch-zoom plugin | 64 |
| `toast.rs` | Shells out to `notify-send` / `osascript` / `powershell.exe` | 330 |
| `monitoring/resource_scope.rs` | Parses `/proc/{pid}/smaps_rollup`, `/sys/fs/cgroup` — Linux only | 414 |
| `setup/remote_server.rs` | Spawns `ssh` | 350 |
| `utils/crash.rs` | `libc::signal` handlers, no non-Linux arm | 107 |
| `utils/hardware.rs` | `/dev/nvidia0`, `wmic` GPU tiers | 130 |

> **⚠️ Correction.** The original LOC column was shifted by one row from `tray.rs` onward: it reported `389 / 64 / 330 / 414 / 350 / 107 / 130` against paths whose real sizes are `234 / 155 / 64 / 330 / 414 / 350 / 107`. `window_customizer.rs` and `monitoring/resource_scope.rs` matched only by coincidence after the shift. All paths exist; no path was missing.

> **⚠️ Second correction (Batch 4 re-verification).** Re-measured on `6310dacf`, the first two rows changed: `services/dictation/**` is **1,146** LOC across 6 files (not 1,133) and `pipeline/dictation/**` is **749** across 6 files (not 741), because the facade work added a non-desktop stub to each `mod.rs`. The remaining figures still hold. The practical correction is not the LOC — it is that **this table should no longer be read as a work list.** Per §4.0, only `setup/remote_server.rs` still needs excluding. The other five unexcluded paths compile correctly on Android via existing `#[cfg]` fallbacks.

### 4.1a Remaining work: `setup/remote_server.rs` — ✅ CLOSED (Phase 1)

`setup/remote_server.rs` (350 LOC) contained **no `cfg` tags at all** and spawned `ssh` to reach remote hosts. On Android, `ssh` does not exist, so any call failed at runtime with an opaque spawn error. It was reachable from the frontend through the `setup_remote_server` IPC command (`lib.rs:46`).

**What shipped:**

1. `setup/mod.rs` — `pub mod remote_server;` is now `#[cfg(desktop)]`, with a `#[cfg(not(desktop))]` inline stub module exposing the identical `start_remote_setup` signature and returning a typed error. Declared **inline** rather than behind a call-site `cfg` so `ipc/catalog.rs` stays free of platform conditionals — the same dual-arm shape as `ipc/tray.rs`.
2. `ModelsCard.tsx` — the `<RemoteServerSetup>` panel is gated behind `isDesktop()`. Computed as a `showRemoteServerSetup` flag *before* the JSX rather than as a nested ternary, keeping the render structure and diff minimal.

**What deliberately still works on mobile:** pointing TTS at an already-provisioned remote server. That path is plain HTTP via `services::llm::transport` and is unaffected. Only *provisioning* is desktop-only.

**Desktop safety:** the desktop arm is the original module, unmodified. `tsc --noEmit` and `pnpm build` both exit 0.

> **Caveat on `isDesktop()`.** `app/src/lib/capabilities.ts:32-34` defines it as `!isCoarsePointer() && !isTouch()` — a *pointer* heuristic, not a platform check. A touchscreen laptop therefore hides the panel even though it has `ssh`. This is a Batch 5 concern, not a Batch 4 one, but it is a real (minor) false negative. Worth replacing with a platform-derived signal when the IPC boundary gets a real platform channel.

### 4.1b Superseded — the original exclusion list

Options previously offered: (1) gate + typed stub + hide UI, (2) gate + stub only, (3) hide UI only. **Option 1 shipped.** The rationale for "provisioning is a desktop-admin task" is recorded under §Product Decisions.

### 4.2 Guard `AppState.hud_menu_item`

`core/state.rs:134` declares `hud_menu_item: ParkingMutex<Option<CheckMenuItem<Wry>>>`. `tray.rs:113` is the only **writer**. Readers: `ipc/tray.rs:64`, `ipc/tray.rs:109`, `config/dispatch.rs:141`, `config/dispatch.rs:192`.

> **⚠️ Correction.** The original text said "only tray code reads it," and that `config/dispatch.rs` is the module that must be ported. The second half is right and the first half is wrong: `config/dispatch.rs` is the settings-mutation dispatcher, not tray code. It is the one reader that is *not* in the tray exclusion set, so it is the actual reason this field needs a desktop guard. The access-site set is exactly those five lines plus the declaration at `core/state.rs:134` and initializer at `core/state.rs:188`; no readers were missed.

**Change:** move it behind a desktop-only side-struct, or box it behind a trait object.

**Desktop safety:** unchanged on desktop. On Android the tray code is already excluded, so the field is never populated.

### 4.3 `sherpa-onnx` target arms

`Cargo.toml:116,119` splits only on `windows` / `not(windows)`. Android currently falls into the `not(windows)` arm. **However, the two arms are byte-identical** — both are `sherpa-onnx = { version = "1.13.8", default-features = false, features = ["shared"] }`. The split is a pure no-op carrying no information.

Separately, `sherpa-onnx-sys-1.13.8/build.rs` **already handles Android**: line 72 copies the built archive into the Tauri `jniLibs` directory so Gradle bundles it into the APK (see also the ABI-aware lookup at `:171-174`). So the "shared library packaging" path is already implemented upstream. What remains is verifying the `not(windows)` arm actually resolves and links for `arm64-v8a` once Batch 7's blockers clear.

> **Status: still open.** The no-op split is unchanged on `6310dacf` — `Cargo.toml:120-124` still carries two byte-identical arms. This is cosmetic noise rather than a build risk, and it is the natural place to fold in `cfg(not(any(android, ios)))` if the arms ever need to diverge for `arm64-v8a` packaging. Low priority.

---

## Batch 5 — Frontend Capability Layer 🟡

**Class:** Android-additive. All additions are inert on desktop.
**Effort:** M
**Blast radius:** `app/src/`, `app/index.html`

### 5.0 Important framing — responsiveness is already done

The existing responsive work is **real and should not be redone**: `src/layout/breakpoints.ts:12` is the SSOT (`BREAKPOINT_COMPACT_MAX = 1024`), `tailwind.config.js:14` pins `screens.lg` to `1024px`, `src/test/invariants.test.ts:913-927` (Invariant 26) *fails* if those two drift, and `src/layout/viewportResize.ts` coalesces resize with a perf gate. `useHomePage.ts:29,37` is the only consumer of `isMobileWidth()`.

> **⚠️ One overstatement to correct.** Invariant 26 fails a `vitest run` / `pnpm test` invocation — it does **not** fail the release pipeline. No CI workflow currently runs `pnpm test`; `release-macos.yml:44` and `release-windows.yml:42` run `pnpm build` only (`tsc && vite build`). So a breakpoint drift fails nothing in CI today. Batch 3 should add `pnpm test` to the new workflow, or this invariant is decorative.

What exists is **"small desktop window"** responsiveness, validated at 1024px. What is missing is **touch-native behavior** — a genuinely separate concern from responsive layout.

### 5.1 Single capability module

Create `src/lib/capabilities.ts` exporting `isDesktop()`, `isCoarsePointer()`, `isTouch()`. Derived from `matchMedia('(pointer: coarse)')` and `navigator.maxTouchPoints`.

**Rationale:** currently capability checks are scattered and ad-hoc (`shared/hooks/useHomePage.ts:29,37` is the only `isMobileWidth()` consumer). One module prevents the scattered-conditional rot that the existing `invariants.test.ts` rules were written to prevent.

### 5.2 Un-block touch gestures

`app/index.html:149-181` unconditionally suppresses pinch-zoom, multi-touch `touchstart`/`touchmove`, and WebKit `gesturestart`/`gesturechange`/`gestureend`. `index.html:72` sets `touch-action: none` on `html, body, #root` (rule starts at `:66`), preventing scroll entirely. There is no `matchMedia`, `pointer`, or platform gate anywhere in the file.

**Change:** gate the gesture-suppression script and `touch-action` behind a desktop pointer check.

**Desktop safety:** the suppression is inert on desktop once gated by `pointer: fine`. This is a no-op for desktop users.

### 5.3 Hover behind a media query

637 `hover:` occurrences across 106 files (raw substring count, so it includes 71 `group-hover:` variants; 566 are non-group). On touch, the first tap fires hover and the second fires click, so hover-revealed menus and tooltips break.

**Change:** wrap hover-dependent affordances in `@media (hover: hover)` so they apply only where hovering is possible.

**Desktop safety:** `hover: hover` matches all desktop input devices. Visually identical.

> **Note:** the pattern is not unwritten — there is one existing precedent using Tailwind arbitrary variants at `shared/components/common/NotificationPanel.tsx:269` (`[@media(hover:hover)]:opacity-0`). Follow it for consistency. No `any-hover` or `pointer: fine` query exists anywhere yet.

### 5.4 Gate keyboard and mouse-only interaction

- `src/layout/ResponsiveLayout.tsx:245-358` — global keymap; `shared/lib/spatialNavigation.ts` imports `@noriginmedia/norigin-spatial-navigation`
- `src/data/shortcuts.ts` — **35** registered shortcuts (⚠️ the original text said 51; counted `{ id: "..." }` entries in the `SHORTCUTS` array at `shortcuts.ts:22-75`)
- `src/shared/ui/SessionContextMenu.tsx` (310 LOC), `ProjectContextMenu.tsx` (232 LOC) — right-click menus
- `src/shared/ui/Drawer.tsx:257` (`cursor-row-resize`), `RotaryKnob.tsx:132` (`cursor-grab`) — ⚠️ both live under `shared/ui/`, not `layout/` as the original text stated
- `src/layout/TitleBar.tsx:113,122,132` — `minimize` / `toggleMaximize` / `close`. Note the whole bar is already Tauri-gated (`TitleBar.tsx:137`)
- `src/layout/ResponsiveLayout.tsx:272-286` — Ctrl+W, Ctrl+Q

**Change:** guard behind `isCoarsePointer()` / `isDesktop()`.

### 5.5 Safe-area insets

No `env(safe-area-inset-*)` usage anywhere. Notches and Android gesture bars will overlap edge-anchored UI.

> **⚠️ Correction to the premise.** The original text cited `ResponsiveLayout.tsx:512,576,587` as `fixed bottom-4 left-4` surfaces. Only **one** such element exists, at `:500`; the other two are `fixed bottom-4 right-4` at `:555` and `:565`. All three are `hidden lg:flex`, so below the 1024px boundary **none of them render at all**. The inset work is still needed, but it is not about these three elements — the surfaces that actually overlap on a phone are the wizard footer (§6), the edge nav, and the top bar.

**Change:** pad the root shell by the inset values. Zero on desktop.

### 5.6 Note on the existing IPC boundary — do not touch

`src/test/invariants.test.ts:65-104` already fails the build if `invoke`/`listen` appear outside `src/services/`. All commands and events funnel through 11 files in `src/services/` (1,775 LOC). This boundary is correct and **requires no change**. Do not introduce a transport abstraction for its own sake; that belongs to the remote feature.

> **Two corrections.** (1) Command count: the backend registers **70** `#[tauri::command]` **names** across 12 files in `src-tauri/src/ipc/`, and `generate_handler![]` at `lib.rs:697-776` lists all 70. A raw attribute count gives 72, because `ipc/tray.rs` defines `hide_tray_window` and `set_window_click_through` twice under `#[cfg(desktop)]` / `#[cfg(not(desktop))]` (`:97,167` and `:122,170`) — the dual-arm stub pattern, which is what keeps them registered on Android. This supersedes the "69" figure recorded before `6310dacf` added the mobile onboarding commands. The original "67" was the count of *frontend-invoked* names. The "14 events" figure is correct (`IpcEvent` at `core/events.rs:213-227`, `name()` at `:250-267`). (2) The invariant's actual coverage is narrower than its title: despite being labelled "invoke/listen" there is **no `listen(` regex**, only the `@tauri-apps/api/event` import check. A `listen()` call outside `src/services/` that avoids a direct import would slip through. Line 92 also exempts any line containing the substring `services` or `pipelineService`, which is a loose escape hatch. Worth tightening separately; out of scope here.

---

## Batch 6 — Wizard Responsiveness 🟡

**Class:** Android-additive.
**Effort:** M
**Blast radius:** `app/src/wizard/` (2,253 LOC, 13 files)

### 6.1 Why this blocks everything

The user has already identified this. If the wizard does not render on a phone, first-run cannot complete, and no downstream capability matters.

### 6.2 Current state

Steps use fixed viewport-anchored shells — `LiveTestStep.tsx:114` and `CompletedStep.tsx:44` both use `h-full max-h-[100vh] overflow-hidden`. `AudioSetupStep.tsx:35` hardcodes `max-w-[280px]` (same literal also at `components/ModelCategory.tsx:191`). `WelcomeStep.tsx` is 420 lines with hover-revealed explanatory copy at lines 37-105.

**Blast radius:** `app/src/wizard/` is **2,253 LOC across 13 files** (⚠️ the original said 2,124).

### 6.3 Work

- Replace `max-h-[100vh] overflow-hidden` shells with scrollable layouts below `BREAKPOINT_COMPACT_MAX`
- Add a mobile variant for the step footer. ⚠️ `WizardFooter.tsx` **already exists** (81 LOC) and is already imported by all five steps that need it (`CompletedStep.tsx:7`, `ModelSetupStep.tsx:17`, `AudioSetupStep.tsx:35`, `SystemCheckStep.tsx:7`, `LiveTestStep.tsx:9`). This is a variant of a shipped component, not a new one.
- Convert hover-revealed copy in `WelcomeStep.tsx` to always-visible or tap-toggled
- Respect safe-area insets on the footer

> **Two corrections to the original §6.3.**
> 1. *"Convert hover-revealed copy ... touch users can never trigger"* — inaccurate. All five hover regions in `WelcomeStep.tsx` already have `onClick` toggles (lines 41, 60, 72, 86, 108) plus `tabIndex={0}`. Touch users are not locked out. The actual defect is narrower: the callout uses pointer-following geometry (`hoveredElement`, rendered at 140-181) with no touch equivalent, so the tooltip appears but does not track.
> 2. *"Apply `prefers-reduced-motion`"* — **already satisfied.** `index.css:333` is a universal `*, *::before, *::after` rule zeroing animation and transition duration, which wizard markup inherits. The residual gap is `framer-motion` springs, which `WelcomeStep.tsx` uses throughout via `motion.div`/`motion.span`; those are JS/WAAPI-driven and CSS `animation-duration` does not reach them. If reduced-motion on the wizard is in scope, name framer-motion explicitly.

**Desktop safety:** all changes sit below the existing 1024px boundary, so desktop rendering is untouched.

---

## Batch 7 — Native Library Packaging ⛔

**Class:** Android-necessary. Desktop must be guarded, not altered.
**Effort:** M-L
**Gate:** app builds and links for `arm64-v8a`

### 7.1 `llama.cpp`

`Cargo.toml:40` enables the `openmp` feature, so this section's premise needs revising.

**✅ RESOLVED — no action needed.** `llama-cpp-sys-4-0.2.61/build.rs:1885-1901` already has an explicit aarch64-Android branch that requires `ANDROID_NDK` and sets `GGML_OPENMP=OFF` (line 1899), along with `ANDROID_ABI=arm64-v8a`, `ANDROID_PLATFORM=android-28`, and `CMAKE_SYSTEM_PROCESSOR=arm64`. **OpenMP is disabled automatically on this target.** Verified: `llama-cpp-sys-4` cross-compiles for `aarch64-linux-android` (102 crates compiled before the next blocker).

The remaining constraint is the `ANDROID_NDK` requirement, which is a plain environment variable — see §1.3. Option 3 (fall back to a remote LLM provider) is **not** needed and should not be proposed.

### 7.2 `turso` / `libsql`

`turso_core` has `cfg(target_os = "android")` branches, so it is Android-aware, but `libsql` is built from source and has no verified Android cross-compile path.

> **⚠️ Line-number correction.** The original text cited `Cargo.toml:482,493` for the `aegis` Android branches. Those line numbers belong to `turso_core 0.8.0-pre.11`, which is present in the registry cache but **not** in `Cargo.lock`. The locked version is `turso_core 0.8.2`, where the branches are at `Cargo.toml:495` (`cfg(any(target_os = "android", target_os = "macos"))` → `aegis` with `pure-rust`) and `:506` (the negation).

**Note:** `turso_core-0.8.2/build.rs` contains no `android` string at all — it only defines a `host_shared_wal` cfg alias. There is no build-script Android handling to verify, so cross-compilability rests on the dependency cfgs alone. **Unverified.**

**Action:** verify the build. If it fails, evaluate `sqlx` or a bundled-SQLite backend behind the existing persistence module boundary.

### 7.3 `ort`

`ort` uses `download-binaries`. **⚠️ Correction to the original text** ("there is no Android ONNX Runtime artifact wired" — this is not quite right).

A prebuilt `aarch64-linux-android` artifact **does** exist: `ort-sys-2.0.0-rc.13/build/download/dist.tsv:6` lists `aarch64-linux-android` with feature set `nnapi` and a live URL. `ort-sys/build/download/resolve.tsv` resolution matches on the exact target triple, so an Android build will select it. The static-link path also covers Android (`build/static_link/mod.rs:27-28` selects `c++_shared`; `:108-110` maps the three Android triples).

**The real gap is the feature, not the artifact.** Vox's `ort` dependency (`Cargo.toml:42`) enables only `download-binaries` and `ndarray`; it does **not** enable `nnapi`. Under `resolve.tsv:120-130`, a feature-set miss falls back to `candidates.first()`, so the build would link the NNAPI artifact and only fail later, at runtime feature validation, if the app needs an execution provider other than NNAPI.

**✅ RESOLVED (Phase 3).** `nnapi` enabled via a `[target.'cfg(target_os = "android")'.dependencies]` entry — a global feature would satisfy Android but break the desktop exact-match check (trap 16). **Still requires a device** to confirm VAD (`silero_onnx.rs`, `ten_onnx.rs`) and the memory embedder (`services/memory/ml/embedder.rs`) run under NNAPI. NNAPI is GPU/driver-accelerated and hardware-varied, so this needs a real device — an emulator will not validate it.

### 7.4 ABI and packaging

Confirm `jniLibs` population for `sherpa-onnx` (`build.rs:64-72` already handles this) and set `abiFilters` to `arm64-v8a` to avoid shipping unused 32-bit slices.

---

## Batch 8 — Model Delivery on Mobile ⛔ — **RE-SCOPED: L → M**

**Class:** Android-necessary.
**Effort:** **M, not L.** See the corrections below — the original "L — this is the true cost of full local functionality" estimate did not survive verification.

### 8.0 Three corrections — the original framing was wrong

**1. "Flat model layout is the real blocker" is backwards.** It is the reason the code is *already* portable. `paths.rs:347-349`:

```rust
pub fn model_dir(name: &str) -> PathBuf {
    get().models.join(name)
}
```

Every provider builds from this one function — `chatterbox_path` (`tts/factory.rs:92`), `kokoro_path` (`:81`), qwen encoder/decoder/joiner (`stt/providers/qwen.rs:32-67`). A grep of `services/{stt,llm,tts}/` for `home_dir`, absolute paths, and `CARGO_MANIFEST_DIR` returns **zero hits**. `dirs::data_local_dir()` resolves to app-private storage on Android and the relative layout is identical.

**Download once, then every model loads exactly as it does on desktop.** No asset-pack abstraction, no per-platform path mapping, no loading-side work at all.

**2. The actual defect is narrow and already located.** `setup/model_manager.rs` is not resumable, and is *actively anti-resumable*:

- `download_and_hash:291` issues a bare `self.client.get(url)` — **no `Range` header**
- `:135` writes to a `.tmp` sibling; the write truncates
- `:155,189,235,250,256` **delete** the `.tmp` on every failure, cancel, and verification path
- `:260` `rename(temp, dest)` on success only

So progress is not merely lost — partial bytes are destroyed on every interruption. **Every interrupted download restarts from zero.** On desktop that is an annoyance. On Android, where the OS kills backgrounded processes aggressively, a multi-GB `chatterbox` download is near-guaranteed to be interrupted.

**3. Most of this is a robustness fix, not an Android port.** A download that dies on desktop also restarts from zero today. Android does not introduce the bug; it removes the user's ability to avoid it.

### 8.1 Re-scoped work

| Item | Effort | Notes |
|---|---|---|
| Skip `migrate_legacy_layout` (`paths.rs:154`) on fresh devices | XS | Desktop-migration-only; no-op on a clean install |
| Android arm for the two cfg blocks at `paths.rs:276-281,283-289` | XS | ⚠️ **Already effectively fine** — `#[cfg(unix)]` is *true* on Android and the chmod errors are discarded with `let _ =`; the `#[cfg(target_os="linux")]` icon `include_bytes!` is already dead there |
| **Resumable transfer** — `Range: bytes=N-`, append instead of truncate, stop deleting `.tmp` | **S** | ~30 lines, all inside `download_and_hash` |
| **Resume on launch** — scan `models/` for stale `.tmp`, offer resume | **S** | Converts "restart at 0" into "continue where it stopped" |
| **Storage preflight** — check free headroom before starting | **S** | Android has far less headroom than desktop |
| **Total** | **M** | ~70% of it also improves desktop |

> **Note:** the original plan also proposed "Wi-Fi-only downloads by default" and "explicit per-model size confirmation." Those remain **open product questions**, not engineering work — see §Open Decisions.

### 8.2 The one thing that would legitimately re-inflate this

Using Android's system `DownloadManager` (native resume across reboot, system notification, no app-process dependency) is a real alternative, but it is a Tauri plugin with more integration work than `Range` + `.tmp`. Recommendation: `Range` + `.tmp` — roughly 95% of the value at S cost — unless system-managed downloads are specifically wanted.

### 8.3 What already transfers — the loading side needs **zero** work

Models are not in the APK and the wizard performs first-install downloads. That architecture is correct and ports directly.

Beyond the path analysis in §8.0(1), the loading side requires **no Android work whatsoever**: no asset-pack abstraction, no platform path mapping, no format changes. Once bytes land in `models/<dir>/<file>`, `stt/`, `llm/`, and `tts/` read them identically on every platform.

### 8.4 Storage path — effectively already correct

`utils/paths.rs` is the single filesystem SSOT, which is exactly right. On Android `dirs::data_local_dir()` resolves to app-private storage, so `~/.vox` does not apply.

Both originally-listed adjustments turned out to be non-issues on inspection:

- `migrate_legacy_layout` (`paths.rs:154-258`) is already a no-op on a fresh device, since there is no legacy layout to migrate. It needs no Android arm — it is simply never triggered.
- `paths.rs:276-281` `#[cfg(unix)]` chmod 0700 — **`cfg(unix)` is *true* on Android**, and the result is already discarded with `let _ =`. It runs and harmlessly does nothing.
- `paths.rs:283-289` `#[cfg(target_os="linux")]` icon `include_bytes!` — already **dead** on Android (`target_os` is `"android"`, not `"linux"`). No arm needed; the icon simply is not written.

> ⚠️ The original plan listed all three as "need an Android arm." None do. This is the `target_os="linux"` pitfall documented in `.agents/rules/android-pitfalls.md` showing up in our own plan.

**Approval gate:** the resumable-download change alters download UX and storage semantics. Per AGENTS.md §4.3, confirm scope before implementation.

---

## Batch 9 — WebGL Thermal Tuning 🟡

**Class:** Android-additive.
**Effort:** M
**Gate:** sustained frame rate on a mid-range device

### 9.1 Affected surfaces

| Surface | LOC |
|---|---|
| `shared/hooks/useMemoryGraphScene.ts` | 1,330 |
| `shared/components/home/AdvancedOrb.tsx` | 910 |
| `shared/components/memory/PixelSynthesisCanvas.tsx` | 281 |
| `shared/components/monitoring/LiquidChamber.tsx` | 441 |

> **⚠️ Correction.** The original table left the last two surfaces as "—". They resolve, but note they are **2D canvas** (`getContext("2d")` at `PixelSynthesisCanvas.tsx:26` and `LiquidChamber.tsx:87`), not WebGL, so Batch 9's framing does not fit them. Only `AdvancedOrb.tsx` is real WebGL (`import * as THREE from 'three'` at line 2) — and it already has adaptive logic (`fpsActive: 60` / `fpsIdle: 12` and a software-raster downgrade at 692-716), so the "add a frame-rate budget" item is partly pre-built there.

### 9.2 Problem

These are tuned for **sustained desktop 60fps**. Phone GPUs thermal-throttle within seconds. Expect correct-then-degrading behavior rather than outright failure.

`src/shared/hooks/useTelemetry.ts:19` already gates on `window.__TAURI_INTERNALS__`, so the metrics plumbing has a precedent for platform gating.

### 9.3 Work

- Add a coarse-pointer frame-rate budget and degrade scene complexity (particle counts, shadow maps, post-processing) rather than dropping frames
- Honor `prefers-reduced-motion` (already supported in `index.css:279,333,818,871,958`)
- Reduce `backdrop-filter` usage on mobile — 45 occurrences in `src/`, but **17 of those are inside `src/test/invariants.test.ts`** (assertion fixtures, not runtime cost). Runtime-relevant count is **28**. If the intent was Tailwind `backdrop-blur-*` utilities instead, that count is 60 across 37 files.

---

## Verification Per Batch

Per AGENTS.md §3, cargo commands run sequentially with `--release`. Tests require explicit approval.

### The canonical dual-target check

Run these **two** commands, not one. Both must be 0 warnings / 0 errors. Use the env recipe from §1.3 — **without** the `SYSROOT` export.

```bash
cargo clippy --all-targets --release
cargo clippy --target aarch64-linux-android --lib --release
```

> **Why clippy and not `cargo check`.** `cargo check --target aarch64-linux-android` was used through Batch 4 and reported success while **14 Android-only warnings** sat in the tree, and while the `SYSROOT` bug (§1.3) went unnoticed because `rustc` ignores that variable. Clippy on the Android target is the only one of the two that (a) surfaces target-specific dead code and (b) exercises `clippy-driver`. Treat **desktop and Android as one gate** — a change is not done until both are clean.

### Per-batch checks

| Batch | Check |
|---|---|
| 1 | `cargo check` green for `x86_64-unknown-linux-gnu`; `cargo clippy --all-targets` clean; `cargo tree -e normal -i native-tls` empty (no OpenSSL on the target graph); `cargo check --target aarch64-linux-android` reaches `enigo` and no further |
| 2 | `cargo nextest run --release --test-threads=1 --no-fail-fast` — full suite, no audio regression; **plus both clippy commands above** |
| 3 | APK installs and launches on a physical device |
| 4 | Both clippy commands above clean; desktop bundles build unchanged; `cargo nextest` green |
| 5 | `pnpm test` green (invariant tests); desktop visual check |
| 6 | Wizard completes on a physical device, portrait and landscape |
| 7 | Release build links for `arm64-v8a` |
| 8 | Full model download → load → inference on device |
| 9 | 5-minute soak on a mid-range device |

---

## Work Per Commit — Desktop Neutrality Rules

These rules exist so no batch silently regresses desktop.

1. **One file per commit** for Batch 4. Broad refactors hide regressions.
2. **No unconditional behavior change.** Every Android branch sits behind a `cfg` or a capability check whose desktop branch is the existing code.
3. **`#[cfg(desktop)]` over `#[cfg(target_os = "...")]`** for new guards — Tauri normalizes this and it stays correct for iOS if that ever matters.
4. **Run the frontend invariant suite after any `breakpoints.ts` or `tailwind.config.js` touch.** `invariants.test.ts:921-926` enforces they stay pinned.
5. **Never widen the 637 hover utilities by hand.** Batch 5 is media-query wrapping, not per-component rewrites.
6. **Update AGENTS.md §5 and `docs/plans/phase13/recent_work.md`** per the mandatory sync hook.

---

## Execution Order — Phase 0 → 6

**This supersedes the batch numbering for scheduling.** Batches are still identified by number; the phase list is the order they execute in.

| Phase | Work | Effort | Why here |
|---|---|---|---|
| **0** | Verification automation (§Verification script, `AGENTS.md` §3 fix, `.agents/rules/android-pitfalls.md`, IPC-registration invariant test) | S | Every later phase is worthless if regressions go unseen. Also retires the two traps that made Batch 4 look complete when it was not. |
| **1** | Close Batch 4 — `setup/remote_server.rs` only | S | ✅ **Done (Phase 1)**. Last known defect; closed by gating the module behind `#[cfg(desktop)]` with a typed mobile stub, plus an `isDesktop()` gate on the `<RemoteServerSetup>` panel. Desktop arm unmodified. |
| **2** | **First `arm64-v8a` release LINK** — `tauri android init`, `bundle.android` config, then *link* not *check* | M | ✅ **Done.** Produced a signed, installable APK. Settled both UNVERIFIED claims (`turso` §7.2, `llama.cpp` §7.1) and found six native-toolchain blockers `cargo check` could never see — see §Phase2-Results. |
| **3** | Batch 7 — fix whatever Phase 2's link exposes | M | ✅ **Done.** Phase 2 *was* the diagnostic; §7.1/§7.2/§7.4 all resolved. §7.3 needed one real fix: `ort`'s `nnapi` feature, and it had to be **target-scoped** — see §Phase3-Results. |
| **4** | APK installs and launches on a physical device | M | First real validation. Confirms `oboe` audio and NNAPI on real hardware — an emulator validates neither. |
| **5** | Batch 8 — resumable downloads | M | Re-scoped from L. Robustness fix; mostly benefits desktop too. |
| **6** | Batch 2 — audio backend seam | M | Deliberately last. Its purpose is to make a future Android audio backend "a new file rather than a refactor." Doing it before Phase 4 means refactoring against assumptions instead of observations. |

**Two ordering rules worth keeping:**

1. **Never `cargo check` a cross target as a gate.** `cargo check --target aarch64-linux-android` passes while Android-only dead code sits in the tree, and it structurally cannot catch it. Use `scripts/verify-targets.sh`.
2. **Reach a link before investing in anything downstream of it.** Phase 2 exists to convert the plan's remaining guesses into build-log fact before Phase 5 spends M on model delivery.

---

## Phase 2 Results — first `arm64-v8a` link ✅

**Outcome:** a signed, installable `arm64-v8a` APK now builds end to end.
`app-universal-release-unsigned.apk` (207.6 MB), containing only `lib/arm64-v8a/`
with `libvox_lib.so`, `libc++_shared.so`, `libonnxruntime.so` and the three
`libsherpa-onnx-*.so`. `minSdk 24`, `targetSdk 36`.

### The core lesson

`cargo check --target aarch64-linux-android` had been green for the whole project.
It **compiles but never links**, so it structurally cannot see native-toolchain
failures. Every blocker below was found only by actually producing a binary.

All six are **native-stack** problems, not "Android is hard". A Rust-only Tauri app
would hit zero of them; a desktop build solved all of them years ago.

| # | Symptom | Root cause | Fix | Location |
|---|---|---|---|---|
| 1 | `E0463: can't find crate for std` | `clippy-driver` honors `SYSROOT`; `rustc` ignores it | `unset SYSROOT`, inline sysroot instead | `scripts/android-env.sh` |
| 2 | `duplicate symbol: ggml_*` | llama.cpp bundled **twice** (llama-cpp-sys + chatterbox), each vendoring its own | `--allow-multiple-definition` | `build.rs` (not config.toml — see below) |
| 3 | `Library artifact not found: libvox_lib.so` | `crate-type = ["lib"]` emits an rlib only | `["staticlib","cdylib","rlib"]` | `Cargo.toml` |
| 4 | `bits/wordsize.h file not found` | bindgen had no sysroot inside Gradle | `[env]` written to `$CARGO_HOME/config.toml` | `scripts/android-env.sh` |
| 5 | `unsupported argument 'native' to '-march='` | chatterbox got no toolchain file; CMake shadowed `CMAKE_SYSTEM_PROCESSOR` and ggml picked x86 | Android toolchain branch | `submodules/chatterbox-rs/build.rs` |
| 6 | `no prebuilt binaries for armv7-linux-androideabi` | Gradle builds all 4 ABIs by default | `-t aarch64` | build command |

### Three findings worth internalising

**A config-file rustflag is not enough for Android.** Tauri exports
`CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS`, and per cargo precedence an env
var beats `config.toml`. A `--allow-multiple-definition` entry in
`.cargo/config.toml` works for a bare `cargo build` and is **silently ignored**
under `tauri android build`. It has to be injected from `build.rs`.

**Cargo finds config from CWD, not from `--manifest-path`.** Build scripts run with
cwd = the crate's own source dir (`~/.cargo/registry/src/...`,
`submodules/chatterbox-rs`). Neither is under `app/src-tauri/`, so a project-level
`[env]` block is invisible to them. `$CARGO_HOME` is the only shared ancestor.

**The Gradle env is a hardcoded allowlist.** Tauri spawns `gradlew` with a fixed
variable set, omitting `BINDGEN_EXTRA_CLANG_ARGS*`, `CC_`/`CXX_`/`AR_*` and
`ANDROID_NDK`. Gradle also cannot be driven standalone — its task connects back
to the parent CLI over a WebSocket. There is no `--ci` flag. So the `$CARGO_HOME`
write is the only injection point that reaches the Gradle cargo pass.

Full detail: `.agents/rules/android-pitfalls.md` traps 11-15.

### Now verified (previously UNVERIFIED)

- ✅ **`turso` / `libsql` (§7.2)** — links for `arm64-v8a`. The plan's caution was warranted but unfounded.
- ✅ **`llama.cpp` (§7.1)** — links.
- ✅ **`chatterbox-rs`** — links and builds with correct ARM arch selection.
- ✅ **`sherpa-onnx`** — links; `jniLibs/arm64-v8a/` populated by its build script.

### Still unverified

- ⛔ **`ort` NNAPI (§7.3)** — `libonnxruntime.so` is packaged, but the `nnapi` feature is still not enabled, so the resolved artifact is not confirmed to be the NNAPI build. Requires real hardware; an emulator validates nothing.
- ⛔ **Nothing has run on a device.** No APK has ever been installed. Audio (`cpal`/oboe), NNAPI, and thermal behaviour are all unproven.

### Build cost

A cold Android build is ~20-25 min, dominated by the C++ stack. Two things cut it:
`-t aarch64` (4x less work — Gradle otherwise builds every ABI) and keeping
`target/aarch64-linux-android/llama-cmake-cache` (3.6 GB, **do not delete**).

`profile.release` has `strip = false`, and `libvox_lib.so` carries ~39 MB of debug
sections in a 170 MB binary. `strip = "symbols"` would cut roughly 25% off every
artifact and shrink the APK — not applied, because it costs a full rebuild and
degrades crash-reporting backtraces.

---

## Phase 3 Results — `ort` NNAPI ✅

**Outcome:** the APK ships an ONNX Runtime exposing the NNAPI execution provider,
and the `ort::ep::NNAPI` Rust API is available. Phase 3 was one real item (§7.3);
§7.1, §7.2 and §7.4 were already settled by Phase 2's link.

### The fix

`ort` had `["download-binaries", "ndarray"]` but **not** `nnapi`. §7.3's
correction stands: *the real gap is the feature, not the artifact.*

It could not simply be added to `[dependencies]`. `ort-sys` requires an **exact**
feature-set match on the downloaded artifact (`build/download/resolve.rs:126`),
`dist.tsv` has exactly one `aarch64-linux-android` row and it is `nnapi`, and
every desktop row is `none` / `webgpu` / `directml` / `cuda13`. Enabling it
globally resolves Android correctly and **breaks the desktop build**. It is
declared as a `[target.'cfg(target_os = "android")'.dependencies]` entry instead,
so Cargo unifies the feature only into the Android graph.

### Why the binary did not change

The rebuilt `libonnxruntime.so` is **byte-identical** (sha256 `33847ad43bffe204`,
22,249,560 B) to the pre-Phase-3 APK. The packaging was already correct — and
almost accidentally so.

With `nnapi` off, `resolve.rs` builds an empty `feature_set`, every
`aarch64-linux-android` candidate intersects it at 0, so `0 == 0` passes the
exact-match check and it falls through to `candidates.first()` — the NNAPI
artifact. One added row, or a changed sort, would have silently broken it.

The genuine defect was the Rust side: `ort::ep::NNAPI` is gated behind
`#[cfg(feature = "nnapi")]`, so the provider existed in the binary but was
unselectable in code. **This change is verified by the feature graph, not by a
binary diff** — `cargo tree -e features -i ort-sys` returns 3 `nnapi` refs for
`--target aarch64-linux-android` and **0** for desktop.

Verified in the built APK: `ARM aarch64`, `NnapiExecutionProvider` present, no
`GLIBC` undefined symbols, `liblog`/`libEGL` linkage. Full signed APK rebuilt at
207.7 MB; both targets clippy clean.

Traps 16-17 in `.agents/rules/android-pitfalls.md`.

### Also closed: the chatterbox `[patch]`

`chatterbox-rs`'s Android toolchain branch (trap 14) is **upstream** as `c9d17bd`
(`4d84eb3..c9d17bd` on `origin/main`). The local `[patch]` redirecting to
`submodules/chatterbox-rs` is **removed**; `Cargo.toml` pins `rev = "c9d17bd"` and
`Cargo.lock` resolves to `git+...?rev=c9d17bd`. `cargo metadata --locked` passes.
**A fresh clone can now build the APK without any local submodule state** — that
was the last reproducibility hole.

---

## Open Decisions for the User

These change scope materially and need an answer before the relevant batch starts.

1. ~~**llama.cpp on Android (§7.1).**~~ **RESOLVED — no decision needed.** `llama-cpp-sys` auto-disables OpenMP on aarch64-Android (`build.rs:1899`) and cross-compiles cleanly. The "fall back to a remote LLM provider" option is off the table.
2. **Model download UX (§8.1).** Wi-Fi-only downloads by default? Explicit per-model size confirmation? Both remain open and are **product** calls, not engineering blockers. Resume-after-process-death is now an S item (a `Range` header plus stopping the five `remove_file` calls at `model_manager.rs:155,189,235,250,256`) and is mandatory either way — the original plan's framing of it as a large, architecture-shaped problem did not survive verification (§8.0).
3. ✅ **Dictation on Android — RESOLVED by necessity.** This was listed as a scope preference; §1.5 shows it now gates the Android build entirely. `enigo` has no Android backend, so `services/dictation/**` **must** be excluded to compile. That answers the question by necessity rather than choice: unless dictation is reimplemented against Android's `InputManager`, Android is assistant-only. **Shipped on `6310dacf`.**
4. **ORT execution provider on Android (§7.3).** The only prebuilt Android artifact is NNAPI. Enable the `nnapi` feature and validate on real hardware, or supply an XNNPACK/full-EP artifact?

---

## Deferred — Preparation for Remote Execution

Recorded here so the work is not lost. **None of it is required for local Android.** All of it becomes necessary only when `wip/remote-control-android.md` is implemented.

| Item | Location | Why remote needs it |
|---|---|---|
| Harness host decoupling | `services/harness/mod.rs:81` — `TurnExecutionRequest.app: AppHandle<R>` | A remote host runs the pipeline without a Tauri window |
| `AppState` neutralization | `core/state.rs:134` `CheckMenuItem<Wry>`; `:136` tauri `JoinHandle` | Same |
| Frontend transport abstraction | `app/src/services/` (11 files) | IPC carrier must become swappable |
| `AudioBackend` trait | introduced in **Batch 2** | Remote streams audio instead of opening a local device |

**Note:** Batch 2's `AudioCapture`/`AudioSink` traits are the one item that serves both this plan and remote execution. That is the primary justification for doing Batch 2 now rather than deferring it.

---

## Related Documents

- [`wip/remote-control-android.md`](file:///home/addy/projects/apps/vox/docs/plans/wip/remote-control-android.md) — the feature this plan prepares for
- [`../phase12/implementation_plan.md`](file:///home/addy/projects/apps/vox/docs/plans/phase12/implementation_plan.md) — Phase 12.1 agentic pipeline plan (stays in Phase 12)
- [`../specs/ipc-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md) — IPC contract
- [`../specs/storage-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md) — filesystem SSOT