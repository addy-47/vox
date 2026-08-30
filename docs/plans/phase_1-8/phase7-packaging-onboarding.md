# Phase 7 — Packaging, Runtime Hardening & First-Run Wizard

## Goal

Convert Vox from a dev-only project into a fully installable desktop application.

Success criteria:

```text
download installer
→ install Vox
→ launch app
→ onboarding wizard appears
→ models download/setup
→ realtime tray works
→ STT/LLM/TTS function correctly
→ persistence survives reboot
```

---

# Core Principles

* startup must feel instant
* tray must appear before heavy model loading
* no Python/runtime dependencies
* models are NOT bundled in installer
* all runtime state centralized in `.vox`
* wizard is part of app, NOT separate executable

---

# Required Runtime Structure

```text

Required Runtime Structure
OS-Specific Local Data (via `dirs::data_local_dir().join("vox")`)
(e.g., ~/.local/share/vox, ~/Library/Application Support/vox, %LocalAppData%\vox)
├── models/
~/.vox/
├── models/
├── logs/
├── cache/
├── sessions/
├── settings.json
├── settings.json.bak
└── vox.db
```

---

# Required Frontend Libraries

Install:

```bash
pnpm add xstate
pnpm add react-router-dom
pnpm add @tauri-apps/plugin-dialog
pnpm add @tauri-apps/plugin-updater
pnpm add @tauri-apps/plugin-process
pnpm add @tauri-apps/plugin-fs
```

Optional:

```bash
pnpm add react-dropzone
pnpm add sonner
```

---

# Required Rust Crates

Add:

```toml
tauri-plugin-dialog = "2"
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
tauri-plugin-fs = "2"

sysinfo = "0.30"
dirs = "6"
walkdir = "2"
sha2 = "0.10"
reqwest = { version = "0.12", features = ["stream", "rustls-tls"] }
tokio-util = "0.7"
futures-util = "0.3"
zip = "2.0"          # Required for extracting Windows/Mac model archives
tar = "0.4"          # Required for Linux model archives
flate2 = "1.0"       # Required for gzip extraction
```

---

# Phase Breakdown

---

# Phase 7.0 — Runtime Hardening

## Goal

Remove all dev-mode assumptions.

---

## Tasks

### Centralize Runtime Paths

Create:

```text
src/utils/paths.rs
```

Must own:

* models dir
* logs dir
* db path
* cache path
* temp path

NO direct:

```rust
./models
./assets
./bin
```

allowed anymore.

---

## Required API

```rust
pub fn vox_dir() -> PathBuf
pub fn models_dir() -> PathBuf
pub fn logs_dir() -> PathBuf
pub fn db_path() -> PathBuf
pub fn settings_path() -> PathBuf
```

---

## Validation

* packaged app works outside repo
* app survives arbitrary install location
* no relative path assumptions remain

---

# Phase 7.1 — Production Boot Lifecycle

## Goal

Instant startup even with cold models.

---

## Rules

DO NOT:

```text
boot app
→ load all models
→ show tray
```

Correct flow:

```text
boot app
→ create tray instantly
→ load lightweight services
→ lazy load inference workers
→ emit readiness events
```

---

## Required Events

```rust
RuntimeBooting
RuntimeReady
ModelLoading
ModelReady
ModelFailed
```

Frontend reacts ONLY to events.

---

## Validation

* tray visible <1s
* UI responsive during model load
* app never freezes during startup

---

# Phase 7.2 — Packaging System

## Goal

Generate production installers.

---

## Required Output

### Windows

* NSIS installer (`.exe`)

### Linux

* `.AppImage`
* `.deb`

macOS deferred.

---

## Tauri Config

Configure:

```json
{
  "bundle": {
    "active": true,
    "targets": ["nsis", "appimage", "deb"]
  }
}
```

Use NSIS over MSI for updater compatibility. ([Tauri][1])

---

## Required Build Commands

```bash
pnpm build
pnpm tauri build
```

---

## Validation

* installer installs correctly
* tray launches after install
* packaged app works outside repo
* no missing runtime libs

---

# Phase 7.3 — Runtime Dependency Verification

## Goal

Detect missing/corrupted runtime state.

---

## Create

```text
src/setup/runtime_check.rs
```

---

## Required Checks

### System

* write permissions
* disk space
* microphone access

### Runtime

* settings.json exists
* model directory exists
* required models exist
* model file sizes match expected
* .verified flag exists (DO NOT hash models on boot; hash only once during install)

### Engine

* STT load test
* TTS load test
* llama.cpp boot test

---

## Validation

* corrupted state detected cleanly
* missing models detected cleanly
* app never crashes from invalid setup

---

# Phase 7.4 — First Run Wizard

## Goal

Guide user through initial setup.

---

# Critical Rule

Wizard is INSIDE app.

DO NOT create:

```text
wizard.exe
```

---

# Frontend Architecture

Create:

```text
frontend/src/wizard/
```

Structure:

```text
wizard/
├── WizardRoot.tsx
├── state/
├── steps/
├── hooks/
└── components/
```

---

## Use XState

Wizard flow must use state machine.

CRITICAL RULE: Rust backend owns the download state (`Arc<Mutex<DownloadState>>`). 

The XState machine purely reflects the backend state via IPC events and sends start/cancel commands. 

DO NOT use:

nested React state chaos
scattered booleans
giant useEffect chains
frontend-owned download tracking (if UI closes, download must safely continue/pause in Rust)

---

# Required Wizard States

```text
welcome
→ system_check
→ model_setup
→ audio_setup
→ live_test
→ completed
```

---

# Required Wizard Steps

---

## Step 1 — Welcome

Explain Vox briefly:

```text
local-first realtime voice system
```

Avoid:

* AI buzzwords
* model terminology

---

## Step 2 — System Check

Show:

* microphone status
* storage available
* CPU thread count
* runtime folder access

---

## Step 3 — Model Setup

Required:

* model download progress
* disk usage display
* hash verification
* retry handling

---

## Model Download Rules

Models download into:

```text
~/.vox/models/
```

Installer NEVER bundles models.

---

## Backend Download Manager

Create:

```text
src/setup/model_manager.rs
```

Responsibilities:

* download
* checksum verification
* extraction
* progress reporting
* cancellation

---

## Required Events

```rust
DownloadStarted
DownloadProgress
DownloadFinished
DownloadFailed
```

---

## Step 4 — Audio Setup

Required:

* microphone selector
* live input meter
* test recording

User MUST verify mic works.

---

## Step 5 — Live Voice Test

MOST IMPORTANT STEP.

Flow:

```text
press button
→ speak
→ transcript appears
→ optional assistant reply
```

This validates:

* audio
* VAD
* STT
* overlay
* IPC
* rendering

---

## Step 6 — Completion

Save:

```json
{
  "setup_completed": true
}
```

Then:

```text
wizard closes
→ normal Vox runtime begins
```

---

# Phase 7.5 — Auto Update Foundation

## Goal

Prepare update infrastructure.

NOT full release infra yet.

---

## Use

```toml
tauri-plugin-updater = "2"
```

Docs:

* [Tauri Updater Docs](https://v2.tauri.app/plugin/updater/?utm_source=chatgpt.com)
* [Windows Installer Docs](https://v2.tauri.app/distribute/windows-installer/?utm_source=chatgpt.com)

---

## Requirements

* updater config
* signed builds
* version checks
* restart flow

Deferred:

* CI automation
* release server
* staged rollout

---

# Phase 7.6 — Installed Runtime Validation

## Goal

Full real-world validation.

---

## Required Tests

### Install Tests

* clean install
* reinstall
* uninstall
* upgrade

### Runtime Tests

* tray boot
* STT works
* TTS works
* persistence survives reboot
* settings persist
* logs persist

### Failure Tests

* corrupted settings
* missing model
* interrupted download
* disk full

---

# Required Frontend Changes

## Add Wizard Route

```text
/wizard
```

---

## App Boot Logic

```text
app start
→ check setup_completed
→ if false:
      open wizard
  else:
      normal runtime
```

---

## Required UX Rules

* no traditional form wizard feel
* minimal UI
* realtime feel
* smooth transitions
* no giant settings screens

---

# Required Backend Modules

Create:

```text
src/setup/
├── mod.rs
├── runtime_check.rs
├── model_manager.rs
├── wizard_state.rs
└── installer.rs
```

---

# Explicitly Out Of Scope

DO NOT build:

* cloud sync
* account system
* telemetry backend
* analytics
* hotword engine
* mobile builds
* multi-user profiles
* marketplace/plugins

---

# Final Validation Checklist

Phase complete only if:

* packaged app installs successfully
* app works outside repo
* no relative path assumptions remain
* wizard completes successfully
* models download correctly
* tray works in installed app
* STT works in installed app
* TTS works in installed app
* persistence survives reboot
* startup remains responsive
* no blocking model initialization
* updater foundation functional

---

# Critical Failure Conditions

Phase is FAILED if:

* startup blocks on model loading
* installer bundles models
* app depends on repo structure
* tray fails outside dev mode
* runtime crashes on missing models
* frontend owns setup state
* wizard becomes separate executable
* packaged runtime differs from dev runtime

---

# Final Architectural Principles

Vox is NOT:

* a chatbot installer
* a dashboard app
* a traditional setup utility

Vox IS:

```text
a realtime local-first voice runtime
with an integrated onboarding layer
```