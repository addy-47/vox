# Contributing to Vox

Thank you for your interest in contributing to Vox! This document outlines guidelines, repository structure, development workflows, and coding conventions to help you get started.

---

## 🛠️ Getting Started

### 1. Prerequisites
Before setting up the project, make sure you have the following installed:
- **Rust Toolchain**: Install via [rustup](https://rustup.rs/) (stable channel).
- **Node.js**: Version 20 or 24.
- **PNPM**: Fast, disk space efficient package manager (`npm install -g pnpm`).
- **Platform Development SDKs**:
  - **Linux**: GTK+ 3, WebKit2GTK, and audio development packages:
    ```bash
    sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev patchelf libasound2-dev
    ```
  - **macOS**: Xcode Command Line Tools.
  - **Windows**: Visual Studio 2022 with the "Desktop development with C++" workload.

### 2. Setting Up the Workspace
1. Clone the repository:
   ```bash
   git clone https://github.com/addy-47/vox.git
   cd vox
   ```
2. Install frontend dependencies:
   ```bash
   cd app
   pnpm install
   ```
3. Run the application in development mode:
   ```bash
   pnpm tauri dev
   ```

---

## 📁 Project Structure

The project is structured as a Tauri monorepo:
*   `app/` - Frontend workspace (Vite, React, TypeScript, TailwindCSS).
*   `app/src-tauri/` - Rust application backend (Tauri handlers, core audio loop, model loaders).
*   `app/src-tauri/src/services/` - Subsystems for Audio, ASR, LLM reasoning, and TTS engines.
*   `app/src-tauri/tests/` - Integration and unit tests.
*   `manifests/` - Contains system and model validation manifests defining required file hashes.
*   `scripts/` - Installation and automated deployment scripts.

---

## 💻 Coding Guidelines

To keep the codebase reliable, cross-platform, and high-performance:

### 1. Platform Agnosticism
*   Avoid raw absolute system paths. Always use relative paths or helper libraries like `dirs` to construct platform-specific paths.
*   Platform-specific features (e.g., Wayland transparent window mechanics on Linux) must be isolated behind target gates (`#[cfg(target_os = "linux")]`).
*   HUD window adjustments (like click-through / cursor ignore) must support native fallbacks on macOS and Windows using Tauri's `window.set_ignore_cursor_events()`.

### 2. Rust Conventions
*   Run `cargo fmt` to format your changes.
*   Verify your changes with `cargo clippy` and fix any warnings.
*   Keep dependencies minimal. Target-gate heavy dependencies (like `gtk`) if they are only needed on specific operating systems.

### 3. Frontend Conventions
*   Keep components reusable and isolated.
*   Manage complex UI state transitions (like setup and active speech status) using state machines or custom hooks.

---

## 🧪 Testing

We require all automated tests to pass before merging any features:

1.  **Backend Rust Tests**:
    Run all tests:
    ```bash
    cd app/src-tauri
    cargo test
    ```
2.  **Specific Integration Tests**:
    Run model-manager and LLM family tests:
    ```bash
    cargo test --test llm_family_test
    ```

---

## 🚀 Release and CI/CD Workflow

Vox uses GitHub Actions to build binaries automatically on tagged releases.

### 1. Tag Conventions
*   **Test Releases**: Use `v0.x.x-test[n]` (e.g. `v0.8.0-test8`). These build test packages without overwriting production drafts.
*   **Production Releases**: Use `v0.x.x` (e.g. `v0.8.0`).

### 2. Creating a Release
To publish your changes:
1. Squash-merge your feature branch into `master`.
2. Apply the production tag:
   ```bash
   git tag v0.8.0
   git push origin v0.8.0
   ```
This triggers three parallel pipelines (`release-linux`, `release-macos`, `release-windows`) that compile the binaries and publish them automatically to the GitHub release page.
