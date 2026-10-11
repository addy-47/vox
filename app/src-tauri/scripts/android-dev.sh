#!/usr/bin/env bash
# ─── Vox Android Fast Iteration Helper ─────────────────────────────────────────
#
#   ./scripts/android-dev.sh debug     build + install a DEBUG APK (REQUIRED for HMR)
#   ./scripts/android-dev.sh reverse   setup adb reverse ports (1420, 1421)
#   ./scripts/android-dev.sh restart   restart Vox app on connected device
#   ./scripts/android-dev.sh screen    capture device screenshot to temp/screen.png
#   ./scripts/android-dev.sh logs      stream relevant Rust/Android logs
#   ./scripts/android-dev.sh dev       reverse + Vite dev server (HMR)
#
# WHY `debug` EXISTS — read before expecting hot reload
# ------------------------------------------------------
# A RELEASE APK cannot hot-reload, and no amount of Vite/adb tuning will make it.
# Release builds embed `frontendDist` (the `pnpm build` output) into the APK and
# never contact `devUrl`, so the WebView loads a frozen asset bundle from inside
# the package. The reverse tunnel and the dev server are simply never consulted.
#
# HMR therefore requires the DEBUG variant, which is compiled against
# `build.devUrl` (http://localhost:1420) and also sets
# `manifestPlaceholders["usesCleartextTraffic"] = "true"`, without which Android
# blocks the plain-HTTP dev server outright.
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
    do_reverse
    echo "==> Launching app..."
    adb shell am start -n "$PKG/.MainActivity" || true
    # TAURI_DEV_HOST is REQUIRED here. Without it vite.config.ts points the HMR
    # websocket at 127.0.0.1:1420, which on the phone is the phone's own
    # loopback — the reverse tunnel does carry it, but the address is fragile
    # across reconnects. Setting it makes Vite advertise the LAN address on 1421.
    echo "==> Starting Vite development server (HMR enabled)..."
    cd "$APP" && TAURI_DEV_HOST="${TAURI_DEV_HOST:-$(hostname -I 2>/dev/null | awk '{print $1}')}" \
        pnpm dev --host 0.0.0.0 --port 1420
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
