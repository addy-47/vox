#!/usr/bin/env bash
# OS and architecture detection script

set -euo pipefail

# Source common functions if available
# SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# if [[ -f "$SCRIPT_DIR/common.sh" ]]; then
#     source "$SCRIPT_DIR/common.sh"
# fi

# Detect OS
detect_os() {
    local os
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        os="linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        os="darwin"
    elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
        os="windows"
    else
        echo "Unsupported OS: $OSTYPE" >&2
        exit 1
    fi
    echo "$os"
}

# Detect architecture
detect_arch() {
    local arch=$(uname -m)
    case $arch in
        x86_64)
            arch="amd64"
            ;;
        aarch64|arm64)
            arch="arm64"
            ;;
        i386|i686)
            arch="386"
            ;;
        *)
            echo "Unknown architecture: $arch" >&2
            exit 1
            ;;
    esac
    echo "$arch"
}

# Detect distribution (for Linux)
detect_distro() {
    if [[ -f /etc/os-release ]]; then
        # shellcheck source=/dev/null
        source /etc/os-release
        echo "$ID"
    elif [[ -f /etc/debian_version ]]; then
        echo "debian"
    elif [[ -f /etc/redhat-release ]]; then
        echo "rhel"
    else
        echo "unknown"
    fi
}

# Check if running on Debian/Ubuntu
is_debian_based() {
    local distro=$(detect_distro)
    [[ "$distro" == "debian" ]] || [[ "$distro" == "ubuntu" ]] || [[ -f /etc/debian_version ]]
}

# Main function
main() {
    local cmd="${1:-info}"
    
    case "$cmd" in
        os)
            detect_os
            ;;
        arch)
            detect_arch
            ;;
        distro)
            detect_distro
            ;;
        debian)
            is_debian_based && echo "true" || echo "false"
            ;;
        info)
            echo "OS: $(detect_os)"
            echo "Architecture: $(detect_arch)"
            echo "Distribution: $(detect_distro)"
            echo "Debian-based: $(is_debian_based && echo "yes" || echo "no")"
            ;;
        *)
            # If sourced, don't exit
            if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
                echo "Usage: $0 {os|arch|distro|debian|info}"
                exit 1
            fi
            ;;
    esac
}

# Run main function if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi