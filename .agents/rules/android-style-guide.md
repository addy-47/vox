---
trigger: manual
description: Android platform engineering invariants, cross-compilation traps, USB HMR workflows, and native pitfalls for Vox (`aarch64-linux-android`).
---

# Android Engineering Style Guide & Invariants

Durable architectural invariants, hardware platform contracts, and confirmed technical pitfalls for Vox on Android (`aarch64-linux-android`). Read before editing audio backends, platform `cfg` gates, path resolution, or build configurations.

---

## 1. Architectural Invariants

### 1.1 Dual-Target Compile Gate
Every code change touching cross-platform paths must pass the compile gate:
```bash
cd app/src-tauri && ./scripts/verify-targets.sh          # Desktop + Android
cd app/src-tauri && ./scripts/verify-targets.sh desktop  # Desktop only (when NDK absent)
```
- Compiles desktop and `aarch64-linux-android` sequentially in `--release` mode with `-D warnings`.
- A task is unverified until this gate exits with code 0.

### 1.2 Single-Window Architecture
- Android runs in a single `Activity` (`MainActivity`) hosting one WebView.
- Never construct secondary OS windows or desktop window proxies (`<WizardStandIn />`) on Android. The mobile WebView renders active flows directly.

### 1.3 Zero Backward Compatibility (ZBC)
Redesign interfaces cleanly. Never add deprecated compatibility paths or legacy bridges.

---

## 2. Platform Matrix: Desktop vs Android

| Feature / Subsystem | Desktop (Linux / macOS / Windows) | Android (`aarch64-linux-android`) |
| :--- | :--- | :--- |
| **Input Simulation** | `enigo`, `arboard`, X11 / Wayland / Win32 | **None.** Desktop input backends do not exist on Android; dictation injection is excluded. |
| **System Tray & HUD** | Supported via OS tray and secondary windows | **None.** Tray HUD is disabled on mobile (`#[cfg(desktop)]`). |
| **Filesystem Root** | `$HOME` directory (`~/.vox`) | **App-private sandbox:** `app.path().app_data_dir()` (`/data/user/0/<pkg>/files`). No `$HOME`. |
| **Remote Provisioning**| SSH-based script execution (`setup/remote_server.rs`) | **Disabled.** Provisioning is desktop-only. Remote inference operates over HTTP. |
| **Window Frame** | Custom desktop `TitleBar` with window controls | **Edge-to-Edge System Bars.** TitleBar unmounted; UI handles system insets. |
| **Navigation** | Floating dock (`EdgeNav`) with keyboard shortcuts | Edge-to-edge bottom navigation bar with 48px+ touch targets. |
| **Execution Provider**| CUDA, DirectML, Metal, CPU | **NNAPI** via target-scoped `ort` dependency. |
| **Audio Backend** | ALSA, PulseAudio, CoreAudio, WASAPI | **Oboe** via `cpal` streams. |

---

## 3. Fast Iteration & Development Workflow

Do not rebuild the full APK for frontend changes. Use the USB reverse HMR loop:

1. **Verify device connection:**
   ```bash
   adb devices
   ```
2. **Reverse port forwarding:**
   ```bash
   adb reverse tcp:1420 tcp:1420
   ```
3. **Start local Vite server:**
   ```bash
   cd app && pnpm dev --host 0.0.0.0 --port 1420
   ```
4. **Helper script commands:**
   ```bash
   ./app/src-tauri/scripts/android-dev.sh dev      # Sets reverse, starts Vite, launches app
   ./app/src-tauri/scripts/android-dev.sh restart  # Quick restart on device
   ./app/src-tauri/scripts/android-dev.sh screen   # Capture screenshot to temp/screen.png
   ./app/src-tauri/scripts/android-dev.sh logs     # Stream filtered runtime logs
   ```

---

## 4. Confirmed Android Technical Pitfalls

### Pitfall 1: `target_os = "linux"` is FALSE on Android
Android reports `target_os = "android"`. Code guarded with `#[cfg(target_os = "linux")]` is silently excluded on Android without warnings. Use `#[cfg(desktop)]` / `#[cfg(not(desktop))]`, or `#[cfg(unix)]` when intentionally including Android.

### Pitfall 2: Never Export `SYSROOT` into Environment
Exporting `SYSROOT=<NDK>/...` causes `clippy-driver` to hunt for host `x86_64` std inside the Android sysroot, breaking host build scripts with `E0463`. Pass the sysroot inline via `BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--sysroot=..."`.

### Pitfall 3: `cargo check` Never Runs the Linker
`cargo check --target aarch64-linux-android` does not run the linker. It cannot detect missing dynamic libraries (`.so`) or duplicate vendored C++ symbols. Verify compilation by building the binary target (`verify-targets.sh link`).

### Pitfall 4: R8 Minification Renames JNI Entry Points in Release
Release builds enable `isMinifyEnabled = true`. R8 renames Java methods resolved by name over JNI (e.g. `WryActivity.getId()`), causing fatal `NoSuchMethodError` aborts. Maintain explicit keep rules in `gen/android/app/jni-keep.pro`. Always test release APKs.

### Pitfall 5: Target-Scoped Dependencies for Platform Features
Enabling platform-specific crate features globally (such as `ort/nnapi`) breaks desktop compilation. Scope platform features strictly in `Cargo.toml`:
```toml
[target.'cfg(target_os = "android")'.dependencies]
ort = { version = "...", features = ["nnapi"] }
```

### Pitfall 6: Route Bootstrap Panics to `stderr`
If code panics before `logging::init()` completes, tracing log sinks are dead. Route early boot panics to `stderr`, which Android captures under the `RustStdoutStderr` logcat tag.

### Pitfall 7: Android Adaptive Icons Require Safe-Zone Padding
Adaptive icons require foreground assets on a 108dp canvas with a 66dp–72dp safe zone. Full-bleed assets get masked by launcher shapes. Keep 20% transparent margins around glyphs; set `ic_launcher_background.xml` to `#090D16`.

### Pitfall 8: Inject Rustflags via `build.rs`
Flags in `.cargo/config.toml` are silently overridden by `CARGO_TARGET_<TRIPLE>_RUSTFLAGS` exported by the Tauri CLI. Inject mandatory flags from `build.rs`.

### Pitfall 9: Gradle Cannot Be Invoked Standalone
Gradle dials back to the Tauri CLI over a WebSocket; direct Gradle calls hang. Build APKs via `pnpm tauri android build --apk -t aarch64`. Source `scripts/android-env.sh` to ensure toolchain environment variables are inherited.

### Pitfall 10: `llama.cpp` Duplicate `ggml_*` Symbols
Vendoring multiple crates containing llama.cpp produces duplicate symbols at link time. Ensure all C++ dependencies share a single toolchain definition via `CMAKE_TOOLCHAIN_FILE`.

### Pitfall 11: `dirs::home_dir()` Returns `/` on Android
Android has no `$HOME`. `dirs::home_dir()` returns `/` instead of `None`, causing paths to resolve to read-only root `/.vox`. On mobile, initialize storage via `paths::init_from_app_data_dir(app.path().app_data_dir())`.

### Pitfall 12: Backend Panics Cause Silent Frontend Hangs
When an async Tauri command panics, the unwinding runtime drops the responder without replying. The frontend `invoke()` promise never settles. Frontends must enforce client-side timeouts on critical IPC calls.

### Pitfall 13: `tauri android dev` is Required for HMR
`tauri android build --debug` packages frozen assets and never connects to `devUrl`. Only `pnpm tauri android dev` (or `android-dev.sh dev`) compiles with `--cfg dev` and connects to the Vite HMR server. Use `host: "0.0.0.0"` and port `1420`.

### Pitfall 14: DEBUG and RELEASE Root at Different Paths
Debug APKs root at `/data/user/0/<pkg>`, while release APKs root at `/data/user/0/<pkg>/files`. Always use absolute paths when verifying on-device file locations with `adb`.

### Pitfall 15: `ndk_context::android_context()` Requires Explicit Initialization
CPAL's Oboe backend calls `ndk_context::android_context()` during audio stream initialization. This panics unless initialized. Call `initialize_android_context` once from `app.run_on_android_context` during application bootstrap.

---

## 5. Mobile UI & Touch Standards

- **Safe-area insets:** Mobile root containers must apply safe-area padding:
  ```css
  padding-top: env(safe-area-inset-top);
  padding-bottom: env(safe-area-inset-bottom);
  ```
- **Touch target minimum:** Interactive controls must provide a minimum touch target of 48px × 48px.
- **No hover dependencies:** Never hide essential indicators or controls behind CSS `:hover` states on mobile.
