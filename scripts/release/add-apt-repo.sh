#!/usr/bin/env bash
# Script to add the DevOps Toolkit APT repository

set -euo pipefail

# Default values
REPO_URL="https://addy-47.github.io/vox/"
REPO_NAME="vox"
REPO_LIST_FILE="/etc/apt/sources.list.d/${REPO_NAME}.list"
PUBLIC_KEY_URL="${REPO_URL}public.gpg"
KEYRING_PATH="/usr/share/keyrings/${REPO_NAME}.gpg"

# Source common functions if available
# SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# if [[ -f "$SCRIPT_DIR/common.sh" ]]; then
#     source "$SCRIPT_DIR/common.sh"
# fi

# Helper for logging
log() {
    echo -e "\033[0;34m[add-apt-repo]\033[0m $1"
}

error() {
    echo -e "\033[0;31m[add-apt-repo] ERROR:\033[0m $1" >&2
    exit 1
}

# Check for root
if [[ "$(id -u)" -ne 0 ]]; then
    error "This script must be run as root"
fi

# Check dependencies
for cmd in curl gpg apt-get; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        error "Command '$cmd' not found. Please install it first."
    fi
done

# Main logic
main() {
    log "Adding APT repository..."

    # 1. Download and install public key (even if using trusted=yes, it's good practice to have it, 
    #    and we might switch to signed-by later)
    log "Downloading public key from $PUBLIC_KEY_URL..."
    if ! curl -fsSL "$PUBLIC_KEY_URL" -o /tmp/public.gpg; then
        error "Failed to download public key"
    fi

    # Create keyrings directory if it doesn't exist
    mkdir -p /usr/share/keyrings

    # Import key
    log "Importing public key to $KEYRING_PATH..."
    rm -f "$KEYRING_PATH"
    if ! gpg --dearmor -o "$KEYRING_PATH" /tmp/public.gpg; then
        error "Failed to dearmor public key"
    fi
    rm -f /tmp/public.gpg

    # 2. Add sources list
    log "Creating source list at $REPO_LIST_FILE..."
    
    # Check architecture
    ARCH="amd64" # Default
    if command -v dpkg >/dev/null 2>&1; then
        ARCH=$(dpkg --print-architecture)
    fi
    
    # Construct the sources line
    # Using signed-by to enforce cryptographic signature verification using the imported public keyring
    SOURCES_LINE="deb [arch=$ARCH signed-by=$KEYRING_PATH] ${REPO_URL}apt/ stable main"
    
    echo "$SOURCES_LINE" > "$REPO_LIST_FILE"
    
    log "Repository added successfully."
    log "Run 'apt-get update' to refresh package lists."
}

main "$@"