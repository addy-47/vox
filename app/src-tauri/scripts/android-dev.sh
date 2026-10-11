#!/usr/bin/env bash
# ─── Vox Android Fast Iteration Helper ─────────────────────────────────────────
#
#   ./scripts/android-dev.sh debug     build + install a DEBUG APK (REQUIRED for HMR, run once)
#   ./scripts/android-dev.sh dev       reverse ports + start Vite dev server + restart app (HMR)
#   ./scripts/android-dev.sh reverse   setup adb reverse ports (1420, 1421)
#   ./scripts/android-dev.sh restart   restart Vox app on connected device
#   ./scripts/android-dev.sh screen    capture device screenshot to temp/screen.png
#   ./scripts/android-dev.sh logs      stream relevant Rust/Android logs
#
# HOW HMR WORKS (read once, then just use `dev`)
# -----------------------------------------------
# Step 1 — ONCE: install the debug APK:
#     ./scripts/android-dev.sh debug
#   This builds a DEBUG variant (HMR-capable) and installs it.
#   You only redo this when native Rust code changes.
#
# Step 2 — every frontend iteration:
#     ./scripts/android-dev.sh dev
#   This: (a) checks a debug APK is installed, (b) sets up adb reverse, 
#   (c) starts Vite, (d) auto-restarts the app once the server is ready.
#   Vite's HMR websocket then keeps the phone WebView live-updated on every save.
#   You do NOT need to rebuild the APK for frontend changes.
#
# WHY `debug` IS REQUIRED ONCE
# -----------------------------
# A RELEASE APK cannot hot-reload, and no amount of Vite/adb tuning will make it.
# Release builds embed `frontendDist` (the `pnpm build` output) into the APK and
# never contact `devUrl`, so the WebView loads a frozen asset bundle from inside
# the package. The reverse tunnel and the dev server are simply never consulted.
#
# The debug APK is NOT at parity with release — release enables
# `isMinifyEnabled = true`, so R8 JNI renaming (Pitfall 4) and any other
# release-only behaviour will not reproduce here. Always smoke-test release via
# `scripts/android-deploy.sh` before believing a device bug is fixed.
#
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$(cd "$ROOT/.." && pwd)"
PKG="${VOX_PKG:-com.addy.Vox}"
TEMP_DIR="$APP/temp"
DEBUG_APK_DIR="$ROOT/gen/android/app/build/outputs/apk"

mkdir -p "$TEMP_DIR"

require_device() {
    adb devices | grep -qw device || {
        echo "error: No authorized device found. Check 'adb devices'." >&2
        exit 1
    }
}

# Returns 0 if the installed APK has the DEBUGGABLE flag, 1 otherwise or if not installed.
is_debug_apk_installed() {
    adb shell dumpsys package "$PKG" 2>/dev/null \
        | grep -q "DEBUGGABLE"
}

do_reverse() {
    require_device
    echo "==> Setting up ADB port forwarding over USB..."
    # 1420 serves the app; 1421 carries the HMR websocket. Both are required —
    # reversing only 1420 leaves edits invisible until a manual reload.
    adb reverse tcp:1420 tcp:1420
    adb reverse tcp:1421 tcp:1421
    echo "==> Ports 1420 and 1421 reversed successfully."
    echo "==> Active reverse mappings:"
    adb reverse --list || true
}

do_debug() {
    require_device
    do_reverse
    echo "==> Building DEBUG arm64-v8a APK (HMR-capable)..."
    # source android-env.sh for the NDK toolchain; the Tauri CLI does not set
    # BINDGEN_EXTRA_CLANG_ARGS on its own (see verify-targets.sh).
    # shellcheck source=scripts/android-env.sh
    source "$ROOT/scripts/android-env.sh"
    ( cd "$APP" && pnpm tauri android build --debug --apk -t aarch64 ) 2>&1 \
        | tee "$TEMP_DIR/android-debug-build.log"

    local apk
    apk="$(find "$DEBUG_APK_DIR" -type f -name "*debug*.apk" -printf '%T@ %p\n' 2>/dev/null | sort -nr | head -1 | cut -d' ' -f2-)"
    [[ -n "$apk" ]] || {
        echo "error: no debug APK produced; see $TEMP_DIR/android-debug-build.log" >&2
        exit 1
    }
    echo "==> Installing $(basename "$apk")"
    adb install -r -d "$apk" 2>&1 | tail -2
    echo "==> Done. Now run: ./scripts/android-dev.sh dev"
}

do_restart() {
    require_device
    do_reverse
    echo "==> Restarting $PKG..."
    adb shell am force-stop "$PKG"
    sleep 0.5
    adb shell am start -n "$PKG/.MainActivity"
    echo "==> App launched."
}

do_screen() {
    require_device
    local out="${1:-$TEMP_DIR/screen.png}"
    echo "==> Capturing screenshot to $out..."
    adb shell screencap -p /sdcard/screen_tmp.png
    adb pull /sdcard/screen_tmp.png "$out" >/dev/null 2>&1
    adb shell rm /sdcard/screen_tmp.png
    echo "==> Saved: $out"
}

do_logs() {
    require_device
    echo "==> Streaming logs for $PKG (Ctrl-C to stop)..."
    adb logcat -v time | grep --line-buffered -iE "RustStdoutStderr|com.addy.Vox|chromium|Tauri"
}

do_dev() {
    require_device
    do_reverse
    echo "==> Sourcing Android NDK environment..."
    # shellcheck source=scripts/android-env.sh
    source "$ROOT/scripts/android-env.sh"
    echo "==> Starting Tauri Android development environment with live HMR..."
    cd "$APP" && pnpm tauri android dev
}

case "${1:-dev}" in
    debug)   do_debug ;;
    reverse) do_reverse ;;
    restart) do_restart ;;
    screen)  do_screen "${2:-}" ;;
    logs)    do_logs ;;
    dev)     do_dev ;;
    *)
        echo "Usage: $0 {debug|dev|reverse|restart|screen|logs}" >&2
        exit 1
        ;;
esac
