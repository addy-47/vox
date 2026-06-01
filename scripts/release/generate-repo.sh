#!/usr/bin/env bash
# Script to generate APT repository metadata (Packages, Release)

set -euo pipefail

# Configuration
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APT_DIR="$REPO_ROOT/apt"
DIST_CODENAME="stable"
COMPONENT="main"
ARCH="amd64"

# Check dependencies
if ! command -v dpkg-scanpackages >/dev/null 2>&1; then
    echo "Error: dpkg-scanpackages not found. Install 'dpkg-dev'." >&2
    exit 1
fi
if ! command -v apt-ftparchive >/dev/null 2>&1; then
    echo "Error: apt-ftparchive not found. Install 'apt-utils'." >&2
    exit 1
fi

echo "Generating APT metadata in $APT_DIR..."

# 1. Generate Packages and Packages.gz
cd "$APT_DIR"

# Ensure dist directories exist
mkdir -p "dists/$DIST_CODENAME/$COMPONENT/binary-$ARCH"

echo "Scanning packages in pool/$COMPONENT..."
# Scan pool/main and output to dists/.../Packages
# We use . as the prefix so the filenames in Packages are relative to apt root (e.g. pool/main/...)
dpkg-scanpackages --multiversion pool/$COMPONENT /dev/null > "dists/$DIST_CODENAME/$COMPONENT/binary-$ARCH/Packages"
gzip -9c "dists/$DIST_CODENAME/$COMPONENT/binary-$ARCH/Packages" > "dists/$DIST_CODENAME/$COMPONENT/binary-$ARCH/Packages.gz"

echo "Generated Packages and Packages.gz"

# 2. Generate Release file
echo "Generating Release file..."
cd "dists/$DIST_CODENAME"
apt-ftparchive \
    -o APT::FTPArchive::Release::Origin="Vox" \
    -o APT::FTPArchive::Release::Label="Vox" \
    -o APT::FTPArchive::Release::Suite="$DIST_CODENAME" \
    -o APT::FTPArchive::Release::Codename="$DIST_CODENAME" \
    -o APT::FTPArchive::Release::Architectures="$ARCH" \
    -o APT::FTPArchive::Release::Components="$COMPONENT" \
    release . > Release

echo "Generated Release file at dists/$DIST_CODENAME/Release"

# 3. GPG Signing
echo "Signing APT repository..."
if [[ -n "${APT_GPG_PASSPHRASE:-}" ]]; then
    # Write passphrase to temporary file safely to avoid exposing it in process tree
    PASSPHRASE_FILE=$(mktemp)
    echo "$APT_GPG_PASSPHRASE" > "$PASSPHRASE_FILE"
    
    gpg --batch --yes --pinentry-mode loopback --passphrase-file "$PASSPHRASE_FILE" --clearsign -o InRelease Release
    gpg --batch --yes --pinentry-mode loopback --passphrase-file "$PASSPHRASE_FILE" -abs -o Release.gpg Release
    
    rm -f "$PASSPHRASE_FILE"
else
    # Fallback if passphrase is not set (e.g. key doesn't have one)
    gpg --batch --yes --clearsign -o InRelease Release
    gpg --batch --yes -abs -o Release.gpg Release
fi

echo "Done."
