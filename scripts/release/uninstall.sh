#!/usr/bin/env bash
# Uninstall script for DevOps Toolkit

set -euo pipefail

# Just forward to install.sh with --remove
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_SCRIPT="$SCRIPT_DIR/install.sh"

if [[ ! -f "$INSTALL_SCRIPT" ]]; then
    echo "Error: install.sh not found in $SCRIPT_DIR" >&2
    exit 1
fi

"$INSTALL_SCRIPT" --remove "$@"