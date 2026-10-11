---
trigger: manual
description: Authoritative guide and invariants for Android platform engineering on Vox. Read before touching any code that compiles on Android, platform cfgs, audio backends, UI safe-areas, or build pipelines.
---

# Android Engineering Style Guide & Invariants

This guide defines the architectural invariants, platform capability contracts, fast iteration workflows, and verified technical traps for Android on Vox.

---

## 1. Architectural & Execution Invariants

### 1.1 Dual-Target Compile Gate
Never commit changes without running the automated gate. Never use bare `cargo check` for cross targets.
```bash
cd app/src-tauri && ./scripts/verify-targets.sh          # Desktop + Android
cd app/src-tauri && ./scripts/verify-targets.sh desktop  # Desktop only (when NDK not present)
```
- The gate compiles both targets sequentially in `--release` mode with `-D warnings`.
- Android builds compile against `aarch64-linux-android` with API 24 sysroot.
- A task is unverified until this gate exits with code 0.

### 1.2 Single-Window Architecture vs Multi-Window
- **Android has ONE window**: Vox runs in a single Android `Activity` (`MainActivity`) with a single WebView instance.
- **Desktop has multiple windows**: Desktop manages `main`, `wizard`, and tray HUD windows.
- **Rule**: Never route Android through desktop window proxies or stand-ins (e.g., `<WizardStandIn />`). The mobile webview must render the active flow directly. Never attempt to construct secondary OS windows on mobile.

### 1.3 Zero Backward Compatibility (ZBC)
Break, replace, or redesign interfaces when required. Never introduce legacy bridge layers or deprecated compatibility paths.

---

## 2. Platform Matrix: Desktop vs Android

| Feature / System | Desktop (Linux / macOS / Windows) | Android (`aarch64-linux-android`) |
| :--- | :--- | :--- |
| **Input Simulation** | `enigo`, `arboard`, X11 / Wayland / Win32 / Cocoa | **None**. Desktop input backends do not exist on Android. Dictation injection is excluded on mobile. |
| **System Tray & HUD** | Supported via OS system tray and lazy window construction | **None**. Tray HUD is disabled on mobile (`#[cfg(desktop)]`). |
| **Filesystem Root** | Standard `$HOME` directory (`~/.vox`) | **No `$HOME`**. Paths resolve to app-private storage (`/data/user/0/<pkg>/files`) via `app.path().app_data_dir()`. |
| **Remote Provisioning** | SSH-based script execution (`setup/remote_server.rs`) | **Disabled**. Provisioning is desktop-only. Remote *runtime* inference works over plain HTTP on mobile. |
| **Window Frame & Chrome** | Custom desktop `TitleBar` with minimize/maximize/close | **Edge-to-Edge System Bars**. Desktop titlebar is unmounted; UI respects status bar and gesture navigation insets. |
| **Navigation Model** | Floating glass dock (`EdgeNav`) with shortcut keys | Full-width edge-to-edge bottom navigation bar with 48dp+ touch targets and gesture bar padding. |
| **Hardware EP** | CUDA, DirectML, Metal, CPU | **NNAPI** (Neural Networks API) via target-scoped `ort` dependency. |
| **Audio Backend** | ALSA, PulseAudio, CoreAudio, WASAPI | **Oboe** via `cpal` high-performance audio streams. |

---

## 3. Fast Iteration & Development Workflow

Do not rebuild the full APK for every frontend change. Use the USB reverse HMR loop for instant iteration.

### 3.1 The USB Reverse HMR Loop (Sub-Second UI Updates)
1. **Connect device via USB** and verify connectivity:
   ```bash
   adb devices
   ```
2. **Setup reverse port forwarding**:
   ```bash
   adb reverse tcp:1420 tcp:1420
   adb reverse tcp:1421 tcp:1421
   ```
   *Why*: Reverse forwarding routes requests to `127.0.0.1:1420` from the phone across the USB cable to your laptop. It does not depend on WiFi, router client isolation, or changing LAN IP addresses.
3. **Start local Vite development server**:
   ```bash
   cd app && pnpm dev --host 0.0.0.0 --port 1420
   ```
4. **Iterate via HMR**:
   Edit React components, layout styles, and CSS. Changes hot-reload on the device in ~200ms without touching Cargo or Gradle.
5. **Use Helper Script**:
   ```bash
   ./app/src-tauri/scripts/android-dev.sh dev      # Sets reverse, starts Vite, launches app
   ./app/src-tauri/scripts/android-dev.sh restart  # Quick restart on device
   ./app/src-tauri/scripts/android-dev.sh screen   # Capture screenshot to temp/screen.png
   ./app/src-tauri/scripts/android-dev.sh logs     # Stream filtered runtime logs
   ```

### 3.2 Live DOM & Console Inspection
Inspect the running mobile WebView using Chrome DevTools:
1. Open Chrome on your laptop to `chrome://inspect/#devices`.
2. Locate `com.addy.Vox` under your connected target.
3. Click **Inspect** to access DOM elements, styles, network requests, and live console logs.

### 3.3 Onboarding Bypass during UI Development
To inspect the main application shell without downloading multi-gigabyte models:
- In development mode, click **Skip Setup** in the mobile onboarding flow.
- This sets `vox_skip_wizard_dev = "true"` in `localStorage` and routes immediately to the primary navigation views (`/`, `/history`, `/memory`, `/settings`, `/monitoring`).

---

## 4. Confirmed Android Technical Pitfalls & Traps

### Pitfall 1: `target_os = "linux"` is FALSE on Android
- **Trap**: Android reports `target_os = "android"`. Any block gated with `#[cfg(target_os = "linux")]` is silently dead on Android with zero compiler warnings.
- **Rule**:
  - Use `#[cfg(desktop)]` / `#[cfg(not(desktop))]` for desktop vs mobile branching.
  - Use `#[cfg(unix)]` only when intentionally including Android.
  - Never use `#[cfg(target_os = "linux")]` unless referring specifically to desktop Linux.

### Pitfall 2: `clippy-driver` Honors `SYSROOT`; `rustc` Does Not
- **Trap**: Exporting `SYSROOT=<NDK>/.../sysroot` causes `clippy-driver` to hunt for host `x86_64` std inside the Android sysroot, breaking host build scripts with `E0463`.
- **Rule**: Never export `SYSROOT` into the environment. Pass sysroot directly inside `BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android`. `verify-targets.sh` enforces this defensively.

### Pitfall 3: `cargo check` Compiles but Never Links
- **Trap**: `cargo check --target aarch64-linux-android` does not run the linker. It cannot detect missing sysroots, duplicate vendored symbols (e.g. duplicate `ggml_*`), or missing dynamic library packaging (`.so`).
- **Rule**: Test actual packaging with `pnpm tauri android build --apk -t aarch64` before declaring native toolchain changes complete.

### Pitfall 4: R8 Minification Renames JNI Entry Points in Release
- **Trap**: Android release builds enable `isMinifyEnabled = true`. R8 renames Java methods resolved by name over JNI (e.g. `WryActivity.getId()`), crashing with uncatchable `NoSuchMethodError` aborts.
- **Rule**: Maintain explicit keep rules in `gen/android/app/jni-keep.pro` (or auto-globbed `.pro` files). Always test release APK builds on real hardware, as debug builds disable minification.

### Pitfall 5: Target-Scoped Dependencies for Features
- **Trap**: Some crates (such as `ort-sys`) enforce exact feature matches against platform prebuilts. Enabling features globally (like `nnapi`) will succeed for Android but break desktop builds.
- **Rule**: Place platform-specific features in target tables:
  ```toml
  [target.'cfg(target_os = "android")'.dependencies]
  ort = { version = "...", features = ["nnapi"] }
  ```

### Pitfall 6: Panic Logging Before Tracing Initialization
- **Trap**: If code panics before `logging::init()` completes, tracing log sinks are dead. If panic hooks call functions that access uninitialized state (like `paths::get()`), double-panic aborts occur with no diagnostics.
- **Rule**: In early bootstrap, route panics to stderr (`RustStdoutStderr` logcat tag). Never call state getters inside panic hooks without fallback protection.

### Pitfall 7: Android Adaptive Icons Require Safe-Zone Padding
- **Trap**: Android 8.0+ adaptive icons place foreground assets on a 108dp canvas with a 66dp–72dp safe zone. Full-bleed foreground images get clipped by launcher masks, and default white background values (`#fff`) clash with dark UI themes.
- **Rule**: Foreground glyphs must have ~20% safe-zone margin on all sides with transparent backgrounds. Set `ic_launcher_background.xml` to brand dark slate (`#090D16`).

---

## 4A. Build & Toolchain Pitfalls (Phases 2–4)

### Pitfall 8: `SYSROOT` Must Never Be Exported
- **Trap**: `rustc` ignores the `SYSROOT` env var; `clippy-driver` honours it. Exporting `SYSROOT=<NDK>/…/sysroot` makes clippy hunt for host `x86_64` std inside the Android sysroot and fail at `E0463: can't find crate for std`. Build scripts compile for the *host* with `clippy-driver`, so this breaks host crates.
- **Rule**: Pass the sysroot inline via `BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--sysroot=…"`. `verify-targets.sh` defensively `unset SYSROOT`.

### Pitfall 9: Config-File rustflags Are Silently Overridden
- **Trap**: A rustflag in `.cargo/config.toml` is silently replaced by the `CARGO_TARGET_<TRIPLE>_RUSTFLAGS` that the Tauri CLI exports, with no warning.
- **Rule**: Inject required flags from `build.rs`, which the CLI cannot override.

### Pitfall 10: Cargo Resolves Config From CWD, Not `--manifest-path`
- **Trap**: Build scripts run with cwd inside `~/.cargo/registry/`, so a project-relative `.cargo/config.toml` is invisible to them.
- **Rule**: Put `[env]` in `$CARGO_HOME/config.toml`, not the project config.

### Pitfall 11: Gradle Cannot Be Driven Standalone
- **Trap**: Gradle's task dials back to the Tauri CLI over a WebSocket. Invoking Gradle directly hangs.
- **Rule**: Build via `pnpm tauri android build --apk -t aarch64`. Always pass `-t aarch64`; the single-ABI build is not persisted in `tauri.conf.json` (there is no `bundle.android` block).

### Pitfall 12: Gradle's Environment Is a Hardcoded Allowlist
- **Trap**: Gradle does not inherit the full shell environment. Toolchain vars set ad-hoc are missing in the Gradle pass even though they work for the CLI pass.
- **Rule**: Source `scripts/android-env.sh` before building. This is also why Vox is **compiled and linked twice per APK** (CLI pass + Gradle `rustBuild`) — roughly half the cost of a Rust-only change, and still unfixed.

### Pitfall 13: `llama.cpp` Is Vendored Twice → Duplicate `ggml_*` Symbols
- **Trap**: `llama-cpp-sys-4` and `chatterbox-rs` each vendor llama.cpp, producing duplicate `ggml_*` symbols at link time. `cargo check` cannot see this because it never links.
- **Rule**: Reach an actual **link** before declaring native work done. `verify-targets.sh link` builds the bin target, which is what proves it.

### Pitfall 14: CMake Needs an Explicit Android Toolchain File
- **Trap**: A vendored C++ crate that does not receive `CMAKE_TOOLCHAIN_FILE` lets CMake shadow `CMAKE_SYSTEM_PROCESSOR` with the host value. ggml then selects its x86 backend and fails on `-march=native`.
- **Rule**: Every vendored C++ crate must receive the Android toolchain file. Fixed upstream in chatterbox-rs `c9d17bd`.

### Pitfall 15: `ort-sys` Requires an EXACT Feature-Set Match
- **Trap**: `ort-sys/build/download/resolve.rs:126` matches the enabled cargo features against `build/download/dist.tsv` exactly. The only `aarch64-linux-android` row is `nnapi`; every desktop row is `none`/`webgpu`/`directml`/`cuda13`. Enabling `nnapi` globally resolves Android correctly and **breaks the desktop build**.
- **Rule**: Target-scoped dependencies only (see Pitfall 5).
- **Trap within the trap**: with `nnapi` off, `resolve.rs` computes an empty feature set, every Android candidate intersects at 0, `0 == 0` passes, and it falls through to `candidates.first()` — the NNAPI artifact anyway. The packaged `.so` is byte-identical either way. **Verify by feature graph, never by binary diff**: `cargo tree -e features -i ort-sys` gives 3 `nnapi` refs on `--target aarch64-linux-android` and 0 on desktop.

### Pitfall 16: `stderr` Is the Only Always-Live Log Channel on Release
- **Trap**: A release APK is not debuggable, so `adb pull` of its private `crashes/` directory is impossible.
- **Rule**: Route early-boot panics to **stderr**, which Android surfaces under the `RustStdoutStderr` logcat tag. `tracing` is not initialised during bootstrap.

### Pitfall 17: `dirs::home_dir()` Returns `/` on Android
- **Trap**: Android has no `$HOME`. `dirs::home_dir()` returns `/`, which is **not** `None`, so a `?:` fallback selects the read-only root and paths resolve to `/.vox`. `create_dir_all` then panics.
- **Rule**: On mobile use `paths::init_from_app_data_dir(app.path().app_data_dir())`, never `paths::init()`. Related: `paths::init()`'s own comment says "must be first" — spawning a thread above it is a race.

### Pitfall 18: R8 Renames JNI Entry Points (release only)
- **Trap**: Release sets `isMinifyEnabled = true`. Rust resolves `WryActivity.getId()` **by name over JNI**, so R8 renames it and produces `NoSuchMethodError` inside a JNI callback, where unwinding is forbidden — an abort with no way to catch it.
- **Rule**: Keep explicit rules in `gen/android/app/jni-keep.pro`. `build.gradle.kts` auto-globs `**/*.pro`, so a separate file survives `tauri android init`.
- **Corollary**: **debug and release are not at parity. Always smoke-test release.**

### Pitfall 19: A Backend Panic Becomes a Silent Frontend Hang
- **Trap**: Tauri spawns async commands and returns via `return_result`. A panic unwinds past it, dropping the `OwnedInvokeResponder` **without replying**. The JS `invoke()` promise then **never settles** — no rejection, no error, no timeout. Release uses `panic = "unwind"`, so the process survives and the UI spins forever.
- **Rule**: The frontend must bound its own IPC calls (timeout + cancel affordance). Never assume the backend will fail politely. See Phase 13.2 D1.3.

### Pitfall 20: `tauri android build` (Even `--debug`) NEVER Hot-Reloads — ONLY `tauri android dev` Connects to `devUrl`
- **Trap**: **Running `tauri android build --debug --apk` builds a frozen APK with `#[cfg(not(dev))]` and packages `frontendDist`. It NEVER contacts `devUrl` and NEVER hot-reloads.**
- **Rule**: **You MUST run `pnpm tauri android dev` (or `./scripts/android-dev.sh dev`). This is the ONLY command that compiles with `--cfg dev`, points the Android WebView to `devUrl`, and connects to Vite HMR.**
- **Host & Port Rule**: **`vite.config.ts` must use `host: "0.0.0.0"` on port `1420`. Do NOT configure dual ports (1421), LAN IP lookups, or `TAURI_DEV_HOST` branching. With `adb reverse tcp:1420 tcp:1420`, port 1420 handles both HTTP and WebSocket HMR simultaneously over USB.**

### Pitfall 21: DEBUG and RELEASE Root at Different Paths
- **Trap**: DEBUG roots at `/data/user/0/<pkg>`; RELEASE roots at `/data/user/0/<pkg>/files`. A relative `adb shell run-as <pkg> ls models` inspects a different tree than expected and produces plausible, wrong output.
- **Rule**: Always `cd` to an absolute path and print `pwd` before concluding anything about on-device state.

### Pitfall 22: `ndk_context::android_context()` Panics Unless Explicitly Initialized
- **Trap**: it is `unsafe { ANDROID_CONTEXT.expect("android context was not initialized") }`. Nothing initializes it unless **`ndk-glue` is in the dependency graph** or `initialize_android_context` is called explicitly. Tauri does **not** do this for you — `run_on_android_context` attaches JNI and passes the activity, but never populates that static.
- **Consequence**: cpal's Oboe backend calls it during `AudioStream::new`, so **every** audio start panics on Android.
- **Rule**: Initialize it once from `app.run_on_android_context` (preferred — no new native linkage), guard against double-init (`initialize_android_context` asserts it is called once), and make every JNI helper **fail soft** rather than panic. Never write a `SAFETY` comment asserting an initializer exists without verifying it in `Cargo.lock`.

---

## 5. UI & Touch Standards on Mobile

1. **System Bar Insets**: Every mobile root container must account for Android 14+ edge-to-edge system bars:
   ```css
   padding-top: env(safe-area-inset-top);
   padding-bottom: env(safe-area-inset-bottom);
   ```
2. **Touch Target Dimensions**: Interactive elements (buttons, nav items, drawer toggles) must have a minimum touch target size of 48px × 48px.
3. **No Desktop Hover Reliance**: Never rely on hover states (`hover:`) for primary information or functionality on mobile; use tap-to-toggle or persistent indicators.
4. **No Artificial Device Chrome**: Never render device frames, fake punch holes, or mock gesture bars on real hardware.
