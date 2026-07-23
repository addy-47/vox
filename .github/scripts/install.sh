#!/usr/bin/env bash
# Vox Installer
# Installs vox from the addy-47/vox APT repository.

set -euo pipefail

# Default configuration
REPO_URL="https://addy-47.github.io/vox/"
DEFAULT_TOOLS="vox"
CI_MODE=false
INTERACTIVE=true
REMOVE_MODE=false
PURGE_REPO=false

# Helper functions
log() {
    echo -e "\033[0;34m[install]\033[0m $1"
}

error() {
    echo -e "\033[0;31m[install] ERROR:\033[0m $1" >&2
    exit 1
}

warn() {
    echo -e "\033[1;33m[install] WARNING:\033[0m $1"
}

usage() {
    cat <<EOF
Usage: $0 [options] [package...]

Options:
  --ci                  CI mode: non-interactive, assumes root, installs only vox by default
  --tools=LIST          Comma-separated list of tools to install (default: vox)
  --remove, --uninstall Uninstall specified tools
  --purge-repo          Remove the APT repository and keyring (use with --remove)
  -y, --yes             Non-interactive mode (assume yes)
  -h, --help            Show this help message

Examples:
  $0                    # Install vox
  $0 vox                # Install only vox
  $0 --ci               # CI mode (installs vox)
  $0 --remove vox       # Uninstall vox
EOF
    exit 0
}

# Parse arguments
POSITIONAL_ARGS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        --ci|ci)
            CI_MODE=true
            INTERACTIVE=false
            DEFAULT_TOOLS="vox" # CI defaults to just vox
            shift
            ;;
        --tools=*)
            TOOLS="${1#*=}"
            shift
            ;;
        --remove|--uninstall)
            REMOVE_MODE=true
            shift
            ;;
        --purge-repo)
            PURGE_REPO=true
            shift
            ;;
        -y|--yes)
            INTERACTIVE=false
            shift
            ;;
        -h|--help)
            usage
            ;;
        -*|--*)
            error "Unknown option: $1"
            ;;
        *)
            POSITIONAL_ARGS+=("$1")
            shift
            ;;
    esac
done

# Restore positional args
set -- "${POSITIONAL_ARGS[@]}"

# Determine tools to operate on
if [[ ${#POSITIONAL_ARGS[@]} -gt 0 ]]; then
    # If positional args are provided, use them as the tools list
    TOOLS_TO_INSTALL=("${POSITIONAL_ARGS[@]}")
else
    # Otherwise use the comma-separated list (default or provided via --tools)
    IFS=',' read -ra TOOLS_TO_INSTALL <<< "${TOOLS:-$DEFAULT_TOOLS}"
fi

# Determine SUDO
if [[ "$(id -u)" -eq 0 ]]; then
    SUDO=""
else
    if command -v sudo >/dev/null 2>&1; then
        if [[ "$CI_MODE" == "true" ]]; then
            SUDO="sudo -n"
        else
            SUDO="sudo"
        fi
    else
        error "This script requires root privileges. Please run as root or install sudo."
    fi
fi

# CI Mode overrides
if [[ "$CI_MODE" == "true" ]]; then
    # Verify we can run commands
    if [[ -n "$SUDO" ]]; then
        if ! $SUDO true 2>/dev/null; then
            error "CI mode requires root or passwordless sudo. Sudo requires a password."
        fi
    fi
fi

# --- REMOVAL MODE ---
if [[ "$REMOVE_MODE" == "true" ]]; then
    log "Uninstalling: ${TOOLS_TO_INSTALL[*]}"
    $SUDO apt-get remove -y "${TOOLS_TO_INSTALL[@]}" || true
    $SUDO apt-get autoremove -y || true
    
    if [[ "$PURGE_REPO" == "true" ]]; then
        log "Removing APT repository..."
        $SUDO rm -f /etc/apt/sources.list.d/vox.list
        $SUDO rm -f /usr/share/keyrings/vox.gpg
        log "Repository removed."
    fi
    
    log "Uninstallation complete."
    exit 0
fi

# --- INSTALLATION MODE ---

# 1. Detect OS
log "Detecting OS..."
# We can use the embedded detect-os logic or just simple checks since we target Debian/Ubuntu
if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    if [[ "$ID" != "debian" && "$ID" != "ubuntu" && "$ID_LIKE" != *"debian"* ]]; then
        error "Unsupported OS: $ID. Only Debian/Ubuntu based systems are supported."
    fi
else
    if [[ ! -f /etc/debian_version ]]; then
        error "Unsupported OS. Only Debian/Ubuntu based systems are supported."
    fi
fi

# 1.1 Install Dependencies
log "Installing dependencies..."
# Ensure curl, gpg, and ca-certificates are installed
DEPS_TO_INSTALL=()
for dep in curl gpg ca-certificates; do
    if ! command -v "$dep" >/dev/null 2>&1; then
        DEPS_TO_INSTALL+=("$dep")
    fi
done

if [[ ${#DEPS_TO_INSTALL[@]} -gt 0 ]]; then
    log "Installing missing dependencies: ${DEPS_TO_INSTALL[*]}"
    # We need to update apt cache to install these
    $SUDO apt-get update -qq || true
    $SUDO apt-get install -y -q "${DEPS_TO_INSTALL[@]}"
fi

# 2. Add APT Repository
log "Setting up APT repository..."
# We download the helper script or run logic inline. 
# Since this is the single entrypoint, we should probably rely on the repo being present or 
# download the add-apt-repo.sh script. 
# However, to be self-contained, we can replicate the logic or fetch the script.
# Let's fetch the script to ensure we use the latest logic from the repo.

SCRIPT_BASE_URL="${REPO_URL}"
ADD_REPO_SCRIPT_URL="${SCRIPT_BASE_URL}add-apt-repo.sh"

# Download add-apt-repo.sh to temp
TMP_ADD_REPO="/tmp/add-apt-repo.sh"
if ! curl -fsSL "$ADD_REPO_SCRIPT_URL" -o "$TMP_ADD_REPO"; then
    # Fallback: if we are running from the repo itself (local dev), try to find it
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [[ -f "$SCRIPT_DIR/scripts/add-apt-repo.sh" ]]; then
        cp "$SCRIPT_DIR/scripts/add-apt-repo.sh" "$TMP_ADD_REPO"
    else
        error "Failed to download add-apt-repo.sh and not found locally."
    fi
fi
chmod +x "$TMP_ADD_REPO"

# Run it
$SUDO "$TMP_ADD_REPO"

# 3. Update APT
log "Updating APT cache..."
$SUDO apt-get update

# 4. Install Packages
log "Installing packages: ${TOOLS_TO_INSTALL[*]}"
APT_ARGS=("-y")
if [[ "$INTERACTIVE" == "false" ]]; then
    APT_ARGS+=("-q")
fi

if ! $SUDO apt-get install "${APT_ARGS[@]}" "${TOOLS_TO_INSTALL[@]}"; then
    error "Installation failed. Try running 'apt-get install -f' to fix dependencies."
fi

# 5. Verify Installation
log "Verifying installation..."
VERIFY_SCRIPT_URL="${SCRIPT_BASE_URL}verify-installation.sh"
TMP_VERIFY="/tmp/verify-installation.sh"

if ! curl -fsSL "$VERIFY_SCRIPT_URL" -o "$TMP_VERIFY"; then
     # Fallback local
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [[ -f "$SCRIPT_DIR/scripts/verify-installation.sh" ]]; then
        cp "$SCRIPT_DIR/scripts/verify-installation.sh" "$TMP_VERIFY"
    else
        warn "Failed to download verify-installation.sh. Skipping verification."
        TMP_VERIFY=""
    fi
fi

if [[ -n "$TMP_VERIFY" ]]; then
    chmod +x "$TMP_VERIFY"
    "$TMP_VERIFY" "${TOOLS_TO_INSTALL[@]}"
fi

# Cleanup function
cleanup() {
    rm -f "$TMP_ADD_REPO" "$TMP_VERIFY"
}
trap cleanup EXIT

log "Installation successful!"
exit 0
