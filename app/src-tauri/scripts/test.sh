#!/usr/bin/env bash
# Local test entry point. Vox does not run tests in CI by design.
# CI is compile-only (see .agents/rules/testing-style-guide.md §7.1).
#
# Usage:
#   ./scripts/test.sh default          # unit + structural seams, no models, no network
#   ./scripts/test.sh models           # seams that load ONNX/GGUF weights
#   ./scripts/test.sh all              # every non-remote binary, sequential, non-ignored
#   ./scripts/test.sh remote           # the gated remote control binaries
#   ./scripts/test.sh seam <binary>    # one seam binary, e.g. seam vad_segmentation_test
#
# Environment:
#   VOX_MODELS_DIR   model root (default: $HOME/.vox/models)

set -euo pipefail

cd "$(dirname "$0")/.."

TIER="${1:-default}"
SEAM="${2:-}"

export VOX_MODELS_DIR="${VOX_MODELS_DIR:-$HOME/.vox/models}"
export RAYON_NUM_THREADS="${RAYON_NUM_THREADS:-$(nproc)}"
export OMP_NUM_THREADS="${OMP_NUM_THREADS:-$(nproc)}"

if [ -z "${VOX_HOME:-}" ]; then
  TEST_SANDBOX_DIR=$(mktemp -d -t vox-test-sandbox-XXXXXX)
  export VOX_HOME="$TEST_SANDBOX_DIR"
  cleanup_test_sandbox() {
    rm -rf "$TEST_SANDBOX_DIR"
  }
  trap cleanup_test_sandbox EXIT
fi

run() {
  cargo nextest run "$@" --release --test-threads=1 --no-fail-fast
}

binaries() {
  find tests -maxdepth 1 -name '*_test.rs' -printf '%f\n' | sed 's/\.rs$//' | sort
}

is_remote() {
  case "$1" in
    remote_*) return 0 ;;
    *) return 1 ;;
  esac
}

in_default_tier() {
  grep -qxF "$1" tests/tier-default.txt 2>/dev/null
}

echo "VOX_MODELS_DIR=$VOX_MODELS_DIR"
echo "tier=$TIER"

case "$TIER" in
  seam)
    if [ -z "$SEAM" ]; then
      echo "usage: $0 seam <binary>" >&2
      exit 2
    fi
    run --test "$SEAM"
    ;;
  default)
    run --lib
    while IFS= read -r t; do
      [ -z "$t" ] && continue
      case "$t" in \#*) continue ;; esac
      run --test "$t"
    done < tests/tier-default.txt
    ;;
  models)
    # Every non-remote binary, with model weights reachable. Default-tier binaries are
    # included on purpose: some of them have a model-dependent branch (for example the
    # transliteration fallback) that only runs when the weights are present.
    run --lib
    while IFS= read -r t; do
      is_remote "$t" && continue
      run --test "$t"
    done < <(binaries)
    ;;
  all)
    # Every non-remote binary. The remote control feature is work in progress, so its
    # binaries are gated behind the `remote` tier.
    run --lib
    while IFS= read -r t; do
      is_remote "$t" && continue
      run --test "$t"
    done < <(binaries)
    ;;
  remote)
    # The remote control binaries, gated because the feature is work in progress.
    while IFS= read -r t; do
      is_remote "$t" || continue
      run --test "$t"
    done < <(binaries)
    ;;
  *)
    echo "unknown tier: $TIER (expected default | models | all | remote | seam)" >&2
    exit 2
    ;;
esac
