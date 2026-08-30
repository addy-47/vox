# Vox Linux CI/CD + APT Distribution Setup

## Goal

Build a production Linux release pipeline that:

1. Builds Vox `.deb`
2. Uploads release artifacts to GitHub Releases
3. Publishes a real APT repository
4. Supports:

   * `sudo apt install vox`
   * `sudo apt upgrade vox`
5. Exposes app + model manifests for in-app update notifications
6. Supports install script:

   ```bash
   curl -fsSL https://vox.ai/install.sh | bash
   ```

---

# Repository Structure

```text
.github/
└── workflows/
    └── release-linux.yml

scripts/
├── build_linux.sh
├── publish_apt.sh
├── generate_manifests.py
└── install.sh

apt/
├── dists/
├── pool/
└── metadata files

manifests/
├── app_manifest.json
└── model_manifest.json
```

---

# CI/CD Flow

On git tag:

```bash
git tag v0.8.0
git push origin v0.8.0
```

GitHub Actions must:

1. Build production `.deb`
2. Upload `.deb` to GitHub Releases
3. Copy `.deb` into `apt/pool/`
4. Generate/update apt metadata:

   * Packages
   * Packages.gz
   * Release
   * InRelease
5. Sign repo metadata using GPG
6. Publish `apt/` + `manifests/` to `gh-pages`
7. Update app manifest version/release notes

---

# Required GitHub Secrets

```text
APT_GPG_PRIVATE_KEY
APT_GPG_PASSPHRASE
```

Used for signing apt repository metadata.

---

# GitHub Pages

Enable GitHub Pages.

Deploy branch:

```text
gh-pages
```

Hosted content includes:

```text
/apt
/manifests
/install.sh
```

---

# Install Script

User installs Vox via:

```bash
curl -fsSL https://vox.ai/install.sh | bash
```

Script responsibilities:

1. Detect Debian/Ubuntu
2. Install Vox GPG key
3. Add Vox apt source
4. Run apt update
5. Install Vox package

After installation:

```bash
sudo apt update
sudo apt upgrade vox
```

must work normally.

---

# App Manifest

Create:

```text
manifests/app_manifest.json
```

Structure:

```json
{
  "latest_version": "0.8.1",
  "release_notes": [
    "Lower RAM usage",
    "Improved tray startup"
  ],
  "linux": {
    "package": "vox",
    "update_command": "sudo apt update && sudo apt upgrade vox"
  }
}
```

Purpose:

* Vox checks this on startup
* Shows update pill in UI
* Shows release summary + update command

---

# Model Manifest

Keep existing HuggingFace manifest structure.

Rename:

```json
"version"
```

to:

```json
"models_version"
```

Add:

```json
"release_notes"
```

Purpose:

* Vox compares local vs remote model version
* Shows model update notifications in UI

---

# Runtime Update Behavior

Vox NEVER auto-installs updates.

Behavior:

```text
boot
→ fetch app manifest
→ fetch model manifest
→ compare versions
→ show update pills in header
→ user manually runs update command
```

No silent OTA.
No automatic binary replacement.

---

# Required Outputs

GitHub Releases:

* `.deb`
* release notes

GitHub Pages:

* apt repository
* manifests
* install.sh

APT install:

```bash
sudo apt install vox
```

APT upgrade:

```bash
sudo apt upgrade vox
```

