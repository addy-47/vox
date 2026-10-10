#!/usr/bin/env bash
# ─── Vox Android Fast Iteration Helper ─────────────────────────────────────────
#
#   ./scripts/android-dev.sh reverse   setup adb reverse ports (1420, 1421)
#   ./scripts/android-dev.sh restart   restart Vox app on connected device
#   ./scripts/android-dev.sh screen    capture device screenshot to temp/screen.png
#   ./scripts/android-dev.sh logs      stream relevant Rust/Android logs
#   ./scripts/android-dev.sh dev       run reverse + Vite dev server + launch app
#
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$(cd "$ROOT/.." && pwd)"
PKG="${VOX_PKG:-com.addy.Vox}"
TEMP_DIR="$APP/temp"

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
    adb reverse tcp:1420 tcp:1420
    adb reverse tcp:1421 tcp:1421
    echo "==> Ports 1420 and 1421 reversed successfully."
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
    adb pull /sdcard/screen_tmp.png "$out" >/dev/null
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
    echo "==> Starting Vite development server (HMR enabled)..."
    cd "$APP" && pnpm dev --host 0.0.0.0 --port 1420
}

case "${1:-dev}" in
    reverse) do_reverse ;;
    restart) do_restart ;;
    screen)  do_screen "${2:-}" ;;
    logs)    do_logs ;;
    dev)     do_dev ;;
    *)
        echo "Usage: $0 {dev|reverse|restart|screen|logs}" >&2
        exit 1
        ;;
esac
