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

## 5. UI & Touch Standards on Mobile

1. **System Bar Insets**: Every mobile root container must account for Android 14+ edge-to-edge system bars:
   ```css
   padding-top: env(safe-area-inset-top);
   padding-bottom: env(safe-area-inset-bottom);
   ```
2. **Touch Target Dimensions**: Interactive elements (buttons, nav items, drawer toggles) must have a minimum touch target size of 48px × 48px.
3. **No Desktop Hover Reliance**: Never rely on hover states (`hover:`) for primary information or functionality on mobile; use tap-to-toggle or persistent indicators.
4. **No Artificial Device Chrome**: Never render device frames, fake punch holes, or mock gesture bars on real hardware.
