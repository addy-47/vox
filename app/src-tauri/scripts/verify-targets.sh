#!/usr/bin/env bash
#
# Dual-target compile gate for Vox.
#
# A change is NOT done until BOTH of these are 0 warnings / 0 errors:
#   1. cargo clippy --all-targets --release                            (desktop)
#   2. cargo clippy --target aarch64-linux-android --lib --release     (Android)
#
# Usage:
#   scripts/verify-targets.sh                 # desktop + Android clippy
#   scripts/verify-targets.sh desktop         # desktop only (no NDK needed)
#   scripts/verify-targets.sh link            # + Android RELEASE LINK (slow, ~20min cold)
#
# The toolchain env lives in `scripts/android-env.sh` — this script sources it.
# Source that file directly before running `pnpm tauri android build`, which does
# NOT set BINDGEN_EXTRA_CLANG_ARGS on its own.
#
# See .agents/rules/android-pitfalls.md.

set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${1:-both}"

# `-D warnings` is essential: plain `cargo clippy` exits 0 even when it prints
# warnings, so without it the success message could lie. It applies to workspace
# members only, not third-party dependencies.
echo "==> desktop: cargo clippy --all-targets --release -D warnings"
cargo clippy --all-targets --release -- -D warnings

if [[ "$MODE" == "desktop" ]]; then
    echo
    echo "OK: desktop compiles with 0 warnings / 0 errors."
    exit 0
fi

# shellcheck source=scripts/android-env.sh
source ./scripts/android-env.sh

echo "==> android: cargo clippy --target aarch64-linux-android --lib --release -D warnings"
cargo clippy --target "$ANDROID_TARGET" --lib --release -- -D warnings

echo
echo "OK: both targets compile with 0 warnings / 0 errors."

if [[ "$MODE" == "link" ]]; then
    # `crate-type = ["lib"]` yields an rlib, so `cargo build --lib` never links.
    # The bin target is what proves the native stack actually resolves symbols.
    # This is the gate that caught duplicate ggml symbols between llama-cpp-sys-4
    # and chatterbox-rs, which `cargo check` structurally cannot see.
    echo
    echo "==> android release LINK: cargo build --target aarch64-linux-android --release --bin Vox"
    cargo build --target "$ANDROID_TARGET" --release --bin Vox
    echo
    echo "OK: arm64-v8a binary linked."
    file "target/$ANDROID_TARGET/release/Vox"
fi
