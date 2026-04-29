# Vox — Packaging & Distribution Architecture

---

## 1. Overview

Vox is distributed as a **self-contained desktop application**.

The user should:

* download a single installer
* install the app
* run Vox immediately

No manual setup of:

* Python
* dependencies
* models (handled separately)

---

## 2. Packaging Philosophy

---

### ⚡ Single Binary Experience

The entire system is delivered as:

```text
Vox App = UI + Runtime + Backend
```

* Frontend (React) → bundled into Tauri
* Backend (Python) → compiled into binary
* Delivered as one installable application

---

### ⚡ Zero Dependency Requirement

End users must NOT:

* install Python
* install Node.js
* configure environment variables

All dependencies are bundled.

---

### ⚡ Models are NOT Bundled

* Models are downloaded on first run
* Keeps installer lightweight
* Allows dynamic upgrades

---

### ⚡ Cross-Platform First

Supported targets:

* Windows → `.exe` (primary)
* Linux → `.AppImage`, `.deb`
* macOS → `.dmg` (future)

---

## 3. System Architecture

---

### Packaging Composition

```text
Tauri App (container)
├── UI (React build)
├── Backend (Python binary — sidecar)
└── Native OS bindings
```

---

### Sidecar Model

The backend runs as a **sidecar process**:

* compiled using PyInstaller
* spawned by Tauri at runtime
* communicates via IPC / HTTP

This pattern allows bundling external runtimes into the app without requiring user setup ([GitHub][1])

---

## 4. Build Pipeline

---

### Step 1 — Build Backend

Compile Python into binary:

```bash
pyinstaller main.py --onefile --name vox_backend
```

Output:

```
dist/backend/vox_backend
```

---

### Step 2 — Attach Backend to App

Move binary to Tauri:

```bash
app/src-tauri/binaries/vox_backend-<target>
```

---

### Step 3 — Configure Sidecar

In `tauri.conf.json`:

```json
{
  "bundle": {
    "externalBin": ["binaries/vox_backend"]
  }
}
```

Tauri automatically includes platform-specific binaries during build ([Tauri][2])

---

### Step 4 — Build Application

```bash
pnpm tauri build
```

Output:

* Windows → `.exe` installer
* Linux → `.AppImage`, `.deb`
* macOS → `.dmg`

---

## 5. Directory Structure (Packaging-Relevant)

---

```bash
vox/
├── app/
│   ├── ui/
│   ├── src-tauri/
│   │   ├── binaries/          # sidecar binaries
│   │   └── tauri.conf.json
│
├── backend/
│   └── src/
│
├── dist/                     # build outputs (ignored in git)
│   ├── backend/
│   └── app/
│
├── scripts/
│   ├── build-backend.sh
│   ├── build-app.sh
│   └── release.sh
```

---

## 6. CI/CD Pipeline

---

### Trigger

* On push to `main`
* On tagged release

---

### Pipeline Steps

1. Install dependencies
2. Build frontend (Vite)
3. Compile backend (PyInstaller)
4. Copy backend binary to Tauri
5. Build Tauri app
6. Upload artifacts

---

### Outputs

Artifacts per OS:

* `Vox-Setup.exe`
* `Vox.AppImage`
* `Vox.deb`

---

### Release System

* Use GitHub Releases
* Attach build artifacts
* Version tied to Git tags

---

## 7. Auto-Update System

---

### Mechanism

Vox uses Tauri’s built-in updater.

Flow:

```text
App launches
→ checks update server
→ new version available
→ downloads update
→ installs silently / prompts user
```

---

### Update Source

* GitHub Releases (default)
* Custom update server (optional)

---

### Requirements

* versioned builds
* signed artifacts (recommended)
* consistent release naming

---

### Important Constraints

* updater must not break sidecar compatibility
* backend + frontend must be version-aligned

---

## 8. Model Distribution

---

### First Run Behavior

```text
App starts
→ checks model directory
→ downloads default models
```

---

### Storage

* stored in user directory
* not inside app bundle

---

### Benefits

* smaller installer
* flexible upgrades
* no reinstall needed for model changes

---

## 9. Development vs Production

---

### Development Mode

```bash
pnpm dev
pnpm tauri dev
```

* backend runs uncompiled
* hot reload enabled

---

### Production Mode

```bash
pnpm tauri build
```

* backend compiled
* optimized bundle

---

## 10. Known Challenges

---

### PyInstaller Limitations

* slow startup for large binaries
* process management complexity ([GitHub][3])

---

### Sidecar Lifecycle

* must handle:

  * startup
  * crash recovery
  * graceful shutdown

---

### OS Differences

* different binaries per platform
* different installer formats

---

## 11. Design Constraints

---

### Must Ensure

* single-click installation
* minimal system overhead
* reliable startup

---

### Must Avoid

* requiring external runtimes
* bundling large models
* manual setup steps

---

## 12. Final Principle

> Packaging is not distribution logic.

It is the **final form of the system**.

The user should never see:

* the backend
* the models
* the complexity

Only:

> Vox — ready to use.

---

[1]: https://github.com/dieharders/example-tauri-v2-python-server-sidecar?utm_source=chatgpt.com "dieharders/example-tauri-v2-python-server-sidecar"
[2]: https://v2.tauri.app/develop/sidecar/?utm_source=chatgpt.com "Embedding External Binaries"
[3]: https://github.com/orgs/tauri-apps/discussions/1645?utm_source=chatgpt.com "Executing python scripts using Tauri #1645"
