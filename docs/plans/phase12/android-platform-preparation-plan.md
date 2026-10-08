# Phase 12.2 — Android Platform Preparation Plan

> **Goal:** A single push to `master` builds an installable APK. The app runs on Android end-to-end: first-run wizard completes, models download, mic capture and playback work, and inference runs locally on-device.
>
> **Scope note — read this first.** This plan contains **no remote-execution work.** It is platform *preparation* plus local Android enablement. Every batch is classified as either:
>
> - 🟢 **Desktop-neutral** — removes platform coupling, adds no Android logic, desktop behavior byte-identical.
> - 🟡 **Android-additive** — adds Android-only branches behind a flag, desktop path provably untouched.
> - ⛔ **Android-necessary** — cannot be avoided to make Android work; desktop must be guarded, not altered.
>
> **Companion document:** [`wip/remote-control-android.md`](file:///home/addy/projects/apps/vox/docs/plans/wip/remote-control-android.md). This plan unblocks its execution-environment question; it does not implement it.

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

---

## Sequencing at a Glance

| Batch | Scope | Class | Effort | Gate |
|---|---|---|---|---|
| 1 | Dependency hygiene | 🟢 Desktop-neutral | S | `cargo check` green on desktop; `openssl-sys` off the Android target graph (full Android `cargo check` is gated on Batch 4 — see §1.5) |
| 2 | Audio backend seam | 🟢 Desktop-neutral | M | Desktop audio regression tests pass |
| 3 | CI APK pipeline | 🟢 Desktop-neutral | M | **APK installs on a physical device** |
| 4 | Desktop-only code exclusion | 🟡 Android-additive | M | **Android `cargo check` green**; desktop bundles unchanged |
| 5 | Frontend capability layer | 🟡 Android-additive | M | Desktop visual regression unchanged |
| 6 | Wizard responsiveness | 🟡 Android-additive | M | First-run completes on a phone |
| 7 | Native library packaging | ⛔ Android-necessary | M-L | `llama.cpp` + `turso` link for `arm64-v8a` |
| 8 | Model delivery on mobile | ⛔ Android-necessary | L | Models download and load on-device |
| 9 | WebGL thermal tuning | 🟡 Android-additive | M | Sustained fps on a mid-range device |

**Critical path:** Batch 3 is the highest-value gate. It converts every subsequent batch from guesswork into a build log. Do not defer it.

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

**⚠️ The original instruction here was wrong and has been removed.** It said to commit a `[target.aarch64-linux-android]` block to `app/src-tauri/.cargo/config.toml`, and asserted such a block "already received during the spike." Neither was true — the file contains desktop targets only. More importantly, **committing an absolute NDK path is wrong**: it hardcodes `/home/addy/...`, which cannot resolve on a CI runner or any other machine.

**The linker is supplied by the build driver, not by cargo config.** The Tauri CLI (v2.11.0) resolves the NDK itself and injects the target-specific environment variables:

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

## Batch 2 — Audio Backend Seam 🟢

**Class:** Desktop-neutral. Introduces a trait; the cpal implementation is unchanged.
**Effort:** M
**Blast radius:** `services/audio/`, `services/tts/voice.rs`, `ipc/audio.rs`

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

## Batch 4 — Desktop-Only Code Exclusion 🟡

**Class:** Android-additive. Desktop behavior unchanged; Android omits these paths.
**Effort:** M
**Blast radius:** broad — **one file per commit**

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

### 4.2 Guard `AppState.hud_menu_item`

`core/state.rs:134` declares `hud_menu_item: ParkingMutex<Option<CheckMenuItem<Wry>>>`. `tray.rs:113` is the only **writer**. Readers: `ipc/tray.rs:64`, `ipc/tray.rs:109`, `config/dispatch.rs:141`, `config/dispatch.rs:192`.

> **⚠️ Correction.** The original text said "only tray code reads it," and that `config/dispatch.rs` is the module that must be ported. The second half is right and the first half is wrong: `config/dispatch.rs` is the settings-mutation dispatcher, not tray code. It is the one reader that is *not* in the tray exclusion set, so it is the actual reason this field needs a desktop guard. The access-site set is exactly those five lines plus the declaration at `core/state.rs:134` and initializer at `core/state.rs:188`; no readers were missed.

**Change:** move it behind a desktop-only side-struct, or box it behind a trait object.

**Desktop safety:** unchanged on desktop. On Android the tray code is already excluded, so the field is never populated.

### 4.3 `sherpa-onnx` target arms

`Cargo.toml:116,119` splits only on `windows` / `not(windows)`. Android currently falls into the `not(windows)` arm. **However, the two arms are byte-identical** — both are `sherpa-onnx = { version = "1.13.8", default-features = false, features = ["shared"] }`. The split is a pure no-op carrying no information.

Separately, `sherpa-onnx-sys-1.13.8/build.rs` **already handles Android**: line 72 copies the built archive into the Tauri `jniLibs` directory so Gradle bundles it into the APK (see also the ABI-aware lookup at `:171-174`). So the "shared library packaging" path is already implemented upstream. What remains is verifying the `not(windows)` arm actually resolves and links for `arm64-v8a` once Batch 7's blockers clear.

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

> **Two corrections.** (1) Command count: the backend registers **69** `#[tauri::command]` attributes across 12 files in `src-tauri/src/ipc/`, and `generate_handler![]` at `lib.rs:669-745` lists all 69. The original "67" is the count of *frontend-invoked* names; the two with no frontend caller are `get_observations` and `show_main_window`. The "14 events" figure is correct (`IpcEvent` at `core/events.rs:213-227`, `name()` at `:250-267`) — 67/14/11 was otherwise right. (2) The invariant's actual coverage is narrower than its title: despite being labelled "invoke/listen" there is **no `listen(` regex**, only the `@tauri-apps/api/event` import check. A `listen()` call outside `src/services/` that avoids a direct import would slip through. Line 92 also exempts any line containing the substring `services` or `pipelineService`, which is a loose escape hatch. Worth tightening separately; out of scope here.

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

**Action:** enable the `nnapi` feature on `ort` for the Android target, then verify VAD (`silero_onnx.rs`, `ten_onnx.rs`) and the memory embedder (`services/memory/ml/embedder.rs`) run under NNAPI. NNAPI is GPU/driver-accelerated and hardware-varied, so this needs a real device — an emulator will not validate it.

### 7.4 ABI and packaging

Confirm `jniLibs` population for `sherpa-onnx` (`build.rs:64-72` already handles this) and set `abiFilters` to `arm64-v8a` to avoid shipping unused 32-bit slices.

---

## Batch 8 — Model Delivery on Mobile ⛔

**Class:** Android-necessary.
**Effort:** L — **this is the true cost of "full local functionality," not any code problem.**

### 8.1 What already transfers

Models are not in the APK and the wizard performs first-install downloads. That architecture is correct and ports directly.

### 8.2 Storage path

`utils/paths.rs` is the single filesystem SSOT, which is exactly right. On Android `dirs::data_local_dir()` resolves to app-private storage, so `~/.vox` does not apply. Two adjustments:

- `migrate_legacy_layout` (`paths.rs:154-258`) is desktop-migration-only — no-op on a fresh device, and should be skipped rather than run
- `paths.rs:276-281` does `#[cfg(unix)]` chmod 0700, and `paths.rs:283-289` does a `#[cfg(target_os="linux")] include_bytes!` of a desktop icon — both need an Android arm (⚠️ the original cited the combined range as `276-288`; the two blocks are `276-281` and `283-289`)

### 8.3 Flat model layout is the real blocker

Models are assumed as plain files at `models/<dir>/<file>` — see `services/stt/mod.rs:23-28`, `services/llm/mod.rs:33,35`, `services/tts/mod.rs:45-46`, `services/tts/providers/kokoro.rs:32-36`. There is no asset-pack abstraction.

On mobile this must become: storage accounting (does the user have room?), resumable transfers, and progress that survives process death.

### 8.4 `ModelManager` assumes a desktop session

`setup/model_manager.rs` is built for long-lived desktop sessions and is **not resumable**. Android will kill the app mid-download.

> **⚠️ Correction to the original framing.** It does not retain an `AppHandle`: `setup/model_manager.rs:56` takes `app: Option<AppHandle<R>>` and immediately converts it into a boxed `ModelStatusEmitter` closure (57-63) held via `Arc`; `struct ModelManager` (49-53) stores only `app_emitter`, `client`, `cancel_flag`. The real defect is sharper and worse than stated — it is **actively anti-resumable**:
>
> - `download_and_hash` (284-332) issues a bare `self.client.get(url).send()` with **no `Range` header**.
> - It writes to a `.tmp` sibling (135), then `File::create(dest)` (299, truncating).
> - **On failure or cancel it deletes the temp file** (155, 189, 235, 250, 256) instead of preserving it.
> - Only `rename(temp, dest)` (260) on success.
>
> So progress is not merely lost — the partial bytes are actively destroyed on every interruption. Preserving the `.tmp` and issuing a `Range` request is the minimum viable fix; that is still Batch 8 scope, not a Batch 1 add.

**Approval gate:** this batch changes download UX and storage semantics. Per AGENTS.md §4.3, confirm scope before implementation.

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

| Batch | Check |
|---|---|
| 1 | `cargo check` green for `x86_64-unknown-linux-gnu`; `cargo clippy --all-targets` clean; `cargo tree -e normal -i native-tls` empty (no OpenSSL on the target graph); `cargo check --target aarch64-linux-android` reaches `enigo` and no further |
| 2 | `cargo nextest run --release --test-threads=1 --no-fail-fast` — full suite, no audio regression |
| 3 | APK installs and launches on a physical device |
| 4 | Desktop bundles build unchanged; `cargo nextest` green |
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
6. **Update AGENTS.md §5 and `docs/plans/phase12/recent_work.md`** per the mandatory sync hook.

---

## Open Decisions for the User

These change scope materially and need an answer before the relevant batch starts.

1. ~~**llama.cpp on Android (§7.1).**~~ **RESOLVED — no decision needed.** `llama-cpp-sys` auto-disables OpenMP on aarch64-Android (`build.rs:1899`) and cross-compiles cleanly. The "fall back to a remote LLM provider" option is off the table.
2. **Model download UX (§8).** Wi-Fi-only downloads by default? Explicit per-model size confirmation? Resume-after-process-death is mandatory either way — and is strictly more work than the original text implied, because `setup/model_manager.rs` currently *deletes* partial downloads on interrupt (§8.4).
3. ⛔ **Dictation on Android — now blocking, not hypothetical.** This was listed as a scope preference; §1.5 shows it now gates the Android build entirely. `enigo` has no Android backend, so `services/dictation/**` **must** be excluded to compile. That answers the question by necessity rather than choice: unless dictation is reimplemented against Android's `InputManager`, Android is assistant-only.
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
- [`implementation_plan.md`](file:///home/addy/projects/apps/vox/docs/plans/phase12/implementation_plan.md) — Phase 12.1 agentic pipeline plan
- [`../specs/ipc-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md) — IPC contract
- [`../specs/storage-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/storage-spec.md) — filesystem SSOT