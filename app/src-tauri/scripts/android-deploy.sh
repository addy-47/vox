#!/usr/bin/env bash
# ─── Android deploy + diagnose loop ───────────────────────────────────────────
#
#   ./scripts/android-deploy.sh build     build, sign, install, launch
#   ./scripts/android-deploy.sh logs      stream logcat (Ctrl-C to stop)
#   ./scripts/android-deploy.sh crash     launch, capture, print the crash reason
#   ./scripts/android-deploy.sh pull      copy logs/ + crashes/ off the device
#   ./scripts/android-deploy.sh cycle     build -> crash  (the inner loop)
#
# Why this exists: a release APK writes crash reports into APP-PRIVATE storage,
# which `adb pull` cannot read. The only reason the launch crash was diagnosable
# was the stderr sink under Android's `RustStdoutStderr` logcat tag. So `crash`
# is the primary mode, not `pull`.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$(cd "$ROOT/.." && pwd)"
PKG="${VOX_PKG:-com.addy.Vox}"
APK_DIR="$ROOT/gen/android/app/build/outputs/apk/universal/release"
UNSIGNED="$APK_DIR/app-universal-release-unsigned.apk"
SIGNED="$APK_DIR/app-arm64-v8a-debug-signed.apk"
LOG_DIR="${VOX_LOG_DIR:-/tmp/opencode/vox-android}"
BUILD_LOG="$LOG_DIR/build-last.log"

BT="$(ls -d "$HOME"/Android/Sdk/build-tools/*/ 2>/dev/null | sort -V | tail -1)"
BT="${BT%/}"

mkdir -p "$LOG_DIR"

die() { echo "error: $*" >&2; exit 1; }

require_device() {
    adb devices | grep -qw device || {
        echo "No authorized device. Check 'adb devices'." >&2
        exit 1
    }
}

# ── build ─────────────────────────────────────────────────────────────────────
do_build() {
    # shellcheck source=/dev/null
    source "$ROOT/scripts/android-env.sh"

    # ── Incremental skip ──────────────────────────────────────────────────────
    # The Rust+C++ rebuild is the 8-20 minute cost of this loop, and it is NOT
    # invalidated by frontend-only changes or by an unchanged crate. Gradle and
    # cargo both no-op on their own, but the wrapper still pays the ~1 min of
    # Vite + config + packaging startup. Skip the whole thing when no Rust or
    # config file is newer than the APK that is already on disk.
    local newest_apk="$UNSIGNED"
    [[ -f "$SIGNED" && "$SIGNED" -nt "$UNSIGNED" ]] && newest_apk="$SIGNED"
    if [[ -f "$newest_apk" ]]; then
        local newest_src
        newest_src="$(find "$ROOT/src" "$ROOT/Cargo.toml" "$ROOT/build.rs" \
            "$ROOT/gen/android/app/build.gradle.kts" \
            -newer "$newest_apk" -print -quit 2>/dev/null || true)"
        if [[ -z "$newest_src" ]]; then
            echo "==> Nothing newer than $newest_apk — skipping build (~${VOX_SKIP_MIN:-1} min saved)."
            SKIP_BUILD=1
        else
            echo "==> Changed: $newest_src"
        fi
    fi
    if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
        echo "==> Building arm64-v8a APK (8-20 min cold; ~1-3 min incremental)"
        ( cd "$APP" && pnpm tauri android build --apk -t aarch64 ) 2>&1 | tee "$BUILD_LOG"
        [[ -f "$UNSIGNED" ]] || die "no APK produced; see $BUILD_LOG"
    fi

    do_sign
}

do_sign() {
    [[ -f "$UNSIGNED" ]] || die "no APK to sign; run '$0 build' first"

    # A debug keystore signature keeps the APK installable without shipping a
    # release key. zipalign must precede apksigner or the signature is rejected.
    cp "$UNSIGNED" "$LOG_DIR/vox-unsigned.apk"
    "$BT/zipalign" -f -p 4 "$LOG_DIR/vox-unsigned.apk" "$LOG_DIR/vox-aligned.apk" \
        || die "zipalign failed"
    "$BT/apksigner" sign \
        --ks "$HOME/.android/debug.keystore" --ks-pass pass:android \
        --key-pass pass:android --ks-key-alias androiddebugkey \
        --out "$SIGNED" "$LOG_DIR/vox-aligned.apk" \
        || die "apksigner failed"
    "$BT/apksigner" verify "$SIGNED" || die "signature verification failed"
    echo "==> Signed: $SIGNED"
}

do_install() {
    require_device
    echo "==> Installing"
    adb install -r -d "$SIGNED" 2>&1 | tail -3
}

do_launch() {
    require_device
    adb shell am force-stop "$PKG"
    sleep 1
    adb logcat -c 2>/dev/null
    adb shell monkey -p "$PKG" -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1
    sleep "${VOX_WAIT:-8}"
    if adb shell pidof "$PKG" >/dev/null 2>&1; then
        echo "==> RUNNING (pid $(adb shell pidof "$PKG"))"
    else
        echo "==> NOT RUNNING — crashed"
    fi
}

# ── crash ─────────────────────────────────────────────────────────────────────
do_crash() {
    do_launch
    local log="$LOG_DIR/crash-last.log"
    adb logcat -d -v time > "$log" 2>/dev/null

    echo
    echo "═══ Rust panic / abort ═══"
    grep -E "RustStdoutStderr|Fatal signal|beginning of crash|Abort message" "$log" \
        | head -25

    echo
    echo "═══ Rust frames ═══"
    grep -E "libvox_lib\.so|libc\.so.*Fatal|signal 6|signal 11" "$log" | head -15

    echo
    echo "═══ App-scoped errors ═══"
    grep -iE "$PKG|Vox.*(error|fail)|Native crash|tombstone" "$log" \
        | grep -viE "ActivityTaskManager|ActivityManager|BatteryStats|whetstone|MiuiNetworkPolicy|ArtChoreographer" \
        | head -20
    echo
    echo "(full log: $log)"
}

case "${1:-cycle}" in
    build)  do_build; do_install; do_launch ;;
    sign)   do_sign; do_install; do_launch ;;
    rebuild) SKIP_BUILD=0 do_build; do_install; do_launch ;;
    logs)   require_device; exec adb logcat -v time ;;
    crash)  do_crash ;;
    relaunch) require_device; do_crash ;;
    pull)   require_device
            OUT="$LOG_DIR/pulled-$(date +%H%M%S)"
            mkdir -p "$OUT"
            # Only works for debuggable builds; release keeps data/ unreadable.
            adb shell "run-as $PKG ls logs crashes 2>/dev/null" > "$OUT/index.txt" 2>&1
            adb shell "run-as $PKG cat logs/*.log" > "$OUT/vox.log" 2>&1
            adb shell "run-as $PKG cat crashes/*" > "$OUT/crashes.txt" 2>&1
            echo "==> Pulled to $OUT"; head -30 "$OUT/crashes.txt" ;;
    cycle)  do_build; do_install; do_crash ;;
    *)      cat >&2 <<EOF
usage: $0 <command>

  cycle     build (if stale) -> sign -> install -> launch -> print crash reason
  build     same as cycle
  rebuild   force a full rebuild even if no source file changed
  sign      sign the EXISTING APK, install, launch      (seconds)
  crash     launch, capture and print the crash reason  (seconds)
  logs      stream logcat
  pull      copy logs/ + crashes/ off the device (debuggable builds only)
EOF
        exit 2 ;;
esac
