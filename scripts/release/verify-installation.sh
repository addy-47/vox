#!/usr/bin/env bash
# Script to verify DevOps Toolkit installation

set -euo pipefail

# Source common functions if available
# SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# if [[ -f "$SCRIPT_DIR/common.sh" ]]; then
#     source "$SCRIPT_DIR/common.sh"
# fi

# Helper for logging
log() {
    echo -e "\033[0;34m[verify]\033[0m $1"
}

error() {
    echo -e "\033[0;31m[verify] ERROR:\033[0m $1" >&2
    exit 1
}

# Verify a single package
verify_package() {
    local pkg=$1
    log "Verifying package: $pkg"

    # Check if installed via dpkg
    if ! dpkg -s "$pkg" >/dev/null 2>&1; then
        error "Package '$pkg' is not installed."
    fi

    # Get version
    local version
    version=$(dpkg-query -W -f='${Version}' "$pkg")
    log "Package '$pkg' is installed (version: $version)"

    # Check binary if applicable (simple mapping)
    local binary="$pkg"
    # Handle special cases if package name != binary name
    case "$pkg" in
        u-cli)
            binary="u"
            ;;
    esac
    
    if command -v "$binary" >/dev/null 2>&1; then
        log "Binary '$binary' found at $(command -v "$binary")"
        # Try running version command if possible
        if "$binary" --version >/dev/null 2>&1; then
            log "Binary version: $("$binary" --version | head -n 1)"
        fi
    else
        log "WARNING: Binary '$binary' not found in PATH."
    fi
}

# Main
main() {
    if [[ $# -eq 0 ]]; then
        error "Usage: $0 <package1> [package2] ..."
    fi

    for pkg in "$@"; do
        verify_package "$pkg"
    done

    log "All requested packages verified successfully."
}

main "$@"