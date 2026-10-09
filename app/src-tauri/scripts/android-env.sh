#!/usr/bin/env bash
#
# Sourceable Android toolchain environment. Source this before ANY Android
# cargo/tauri command:
#
#   source scripts/android-env.sh
#   pnpm tauri android build --debug --apk
#
# It is the single source of truth for the NDK toolchain env. Both
# `verify-targets.sh` and manual `tauri android build` invocations must use it.
#
# WHY EACH VARIABLE EXISTS
# ------------------------
# `ANDROID_NDK`                      llama-cpp-sys build.rs:1889 `.expect()`s it.
# `CARGO_TARGET_..._LINKER`          cargo reads this natively (same precedence as
#                                    config.toml), so no committed NDK path is
#                                    needed. config.toml also sets a BARE linker
#                                    name; exporting this dir on $PATH resolves it.
# `CC_/CXX_/AR_aarch64_linux_android` cc/cmake builds of C++ deps.
# `BINDGEN_EXTRA_CLANG_ARGS_...`     llama-cpp-sys build.rs:1294 passes only
#                                    `--target=<triple>` to bindgen, with NO
#                                    sysroot. Without this it parses HOST
#                                    /usr/include and dies on
#                                    `bits/wordsize.h file not found`.
#                                    The `-D__ANDROID_API__=24` half is REQUIRED:
#                                    the Rust triple carries no API level, so
#                                    bindgen defaults to 21, and NDK r29's libc++
#                                    references `pthread_cond_clockwait` which
#                                    does not exist below API 24.
#
# ⚠️  DO NOT EXPORT `SYSROOT`.
# `rustc` ignores it but `clippy-driver` HONORS it as an authoritative sysroot
# override, so exporting it makes every Android clippy run fail at
# `E0463: can't find crate for std`. It is inlined into the bindgen args below
# and explicitly cleared here so a stray export in the caller's shell cannot
# break the build.
#
# `tauri android build` sets most of these itself from ANDROID_HOME, but it does
# NOT set BINDGEN_EXTRA_CLANG_ARGS — that is the one thing you must supply.
#
# Env overrides: NDK_HOME, ANDROID_API (default 24).

ANDROID_API="${ANDROID_API:-24}"
ANDROID_TARGET="aarch64-linux-android"
ANDROID_ABI="arm64-v8a"

# clippy-driver honors SYSROOT; rustc ignores it. Clear it unconditionally.
unset SYSROOT

if [[ -z "${NDK_HOME:-}" ]]; then
    _ndk_root="${ANDROID_HOME:-$HOME/Android/Sdk}/ndk"
    if [[ -d "$_ndk_root" ]]; then
        NDK_HOME="$(ls -1 "$_ndk_root" | sort -V | tail -1)"
        export NDK_HOME
    fi
fi

if [[ -z "${NDK_HOME:-}" || ! -d "$NDK_HOME" ]]; then
    echo "android-env.sh: NDK not found. Set NDK_HOME to your Android NDK (r29.x)." >&2
    echo "  e.g. export NDK_HOME=\$HOME/Android/Sdk/ndk/29.0.13846066" >&2
    return 1 2>/dev/null || exit 1
fi

_ndk_bin="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
_ndk_sysroot="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot"

if [[ ! -d "$_ndk_sysroot" ]]; then
    echo "android-env.sh: NDK sysroot not found at $_ndk_sysroot" >&2
    return 1 2>/dev/null || exit 1
fi

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export ANDROID_NDK="$NDK_HOME"
export PATH="$_ndk_bin:$PATH"

export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$_ndk_bin/aarch64-linux-android${ANDROID_API}-clang"
export CC_aarch64_linux_android="$_ndk_bin/aarch64-linux-android${ANDROID_API}-clang"
export CXX_aarch64_linux_android="$_ndk_bin/aarch64-linux-android${ANDROID_API}-clang++"
export AR_aarch64_linux_android="$_ndk_bin/llvm-ar"
# sysroot INLINED, never exported as SYSROOT.
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--sysroot=$_ndk_sysroot -D__ANDROID_API__=${ANDROID_API}"

# llama-cpp-sys reads these (build.rs:1885-1901).
export ANDROID_ABI="$ANDROID_ABI"
export ANDROID_PLATFORM="android-${ANDROID_API}"

# ── Persist the same vars into $CARGO_HOME/config.toml ───────────────────────
#
# Exporting above is NOT enough. Cargo resolves `.cargo/config.toml` from the
# CURRENT WORKING DIRECTORY, and build scripts run with cwd = the crate's own
# source dir:
#
#   ~/.cargo/registry/src/.../llama-cpp-sys-4-0.2.61/   (registry dep)
#   <repo>/submodules/chatterbox-rs/                   (path dep)
#
# Neither lives under app/src-tauri/, so a `[env]` block there is never read by
# those crates -- verified: `tauri android build` still failed with
#   /usr/include/features-time64.h:20:10: fatal error: 'bits/wordsize.h' file not found
#
# And nothing exported in the shell can help, because `tauri android build` spawns
# `gradlew` with a hardcoded env allowlist that omits BINDGEN_EXTRA_CLANG_ARGS and
# ANDROID_NDK.
#
# $CARGO_HOME is the one directory every crate's upward config search reaches, so
# writing the `[env]` block there is what actually fixes Gradle-driven builds.
# This block is rewritten in place on every run, so an NDK version bump self-heals.

_cargo_cfg="${CARGO_HOME:-$HOME/.cargo}/config.toml"
_begin="# >>> vox android cross-compile >>>"
_end="# <<< vox android cross-compile <<<"

mkdir -p "$(dirname "$_cargo_cfg")"
# Strip any previous block, then append the current one.
if [[ -f "$_cargo_cfg" ]]; then
    if grep -qF "$_begin" "$_cargo_cfg"; then
        awk -v b="$_begin" -v e="$_end" '
            index($0, b) == 1 { skip = 1; next }
            index($0, e) == 1 { skip = 0; next }
            skip != 1 { print }
        ' "$_cargo_cfg" > "$_cargo_cfg.tmp" && mv "$_cargo_cfg.tmp" "$_cargo_cfg"
    fi
fi

cat >> "$_cargo_cfg" <<EOF

$_begin
# Managed by app/src-tauri/scripts/android-env.sh -- do not edit by hand.
# Required so cargo build scripts running with cwd inside ~/.cargo (registry
# deps) or submodules/ (path deps) can find the Android NDK toolchain.
[env]
BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android = "--sysroot=$_ndk_sysroot -D__ANDROID_API__=${ANDROID_API}"
ANDROID_NDK = "$NDK_HOME"
ANDROID_ABI = "$ANDROID_ABI"
ANDROID_PLATFORM = "android-${ANDROID_API}"
$_end
EOF

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "android-env: NDK=$NDK_HOME ABI=$ANDROID_ABI API=$ANDROID_API"
    echo "android-env: wrote [env] block to $_cargo_cfg (source this, don't run it)"
fi
