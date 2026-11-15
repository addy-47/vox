#!/usr/bin/env bash
# ==============================================================================
# remote-exec.sh — Fast rsync + remote command execution on the 32-core server
# ==============================================================================
# Usage:
#   ./scripts/remote-exec.sh cargo test --test harness_loop_test --release
#   ./scripts/remote-exec.sh --sync-only
#   ./scripts/remote-exec.sh --no-sync cargo test --test audio_capture_aec_test --release
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_SRC_TAURI="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${APP_SRC_TAURI}/../.." && pwd)"

SERVER_FILE="${PROJECT_ROOT}/temp/server.txt"

if [[ ! -f "${SERVER_FILE}" ]]; then
    echo "ERROR: Server credentials file not found at ${SERVER_FILE}" >&2
    exit 1
fi

# Parse SSH target from server.txt (first line typically has: ssh -v user@host)
SSH_LINE=$(head -n 1 "${SERVER_FILE}")
REMOTE_TARGET=$(echo "${SSH_LINE}" | grep -oE '[a-zA-Z0-9_-]+@[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+' || true)

if [[ -z "${REMOTE_TARGET}" ]]; then
    echo "ERROR: Could not parse remote user@host from ${SERVER_FILE}" >&2
    echo "Line was: ${SSH_LINE}" >&2
    exit 1
fi

REMOTE_WORKSPACE="~/.vox-remote-workspace/app/src-tauri"

SYNC_ENABLED=true
if [[ "${1:-}" == "--no-sync" ]]; then
    SYNC_ENABLED=false
    shift
fi

if [[ "${1:-}" == "--sync-only" ]]; then
    SYNC_ENABLED=true
    CMD=""
else
    CMD="$*"
fi

# 1. Sync local codebase to remote workspace
if [[ "${SYNC_ENABLED}" == true ]]; then
    echo "[remote-exec] Syncing ${APP_SRC_TAURI}/ to ${REMOTE_TARGET}:${REMOTE_WORKSPACE}/..."
    ssh -o BatchMode=yes -o ConnectTimeout=5 "${REMOTE_TARGET}" "mkdir -p ${REMOTE_WORKSPACE}"
    
    rsync -az --delete \
        --exclude="target/" \
        --exclude=".git/" \
        --exclude="node_modules/" \
        --exclude="temp/" \
        --exclude="*.log" \
        --exclude="*.tmp" \
        "${APP_SRC_TAURI}/" "${REMOTE_TARGET}:${REMOTE_WORKSPACE}/"
    echo "[remote-exec] Sync completed in <2s."
fi

if [[ "${1:-}" == "--sync-only" ]]; then
    exit 0
fi

if [[ -z "${CMD}" ]]; then
    echo "Usage: $0 [--no-sync|--sync-only] <command...>" >&2
    exit 1
fi

# 2. Execute remote command in the remote workspace
echo "[remote-exec] Executing remotely on 32-core server:"
echo "              ${CMD}"
echo "------------------------------------------------------------------------"

REMOTE_WRAPPER="
source ~/.cargo/env 2>/dev/null || export PATH=\"\$HOME/.cargo/bin:\$PATH\"
cd ${REMOTE_WORKSPACE}
export RAYON_NUM_THREADS=\$(nproc)
export OMP_NUM_THREADS=\$(nproc)
export VOX_MODELS_DIR=\"\${VOX_MODELS_DIR:-\$HOME/.vox/models}\"
${CMD}
"

ssh -o BatchMode=yes -o ConnectTimeout=10 -t "${REMOTE_TARGET}" "bash -lc '${REMOTE_WRAPPER}'"
