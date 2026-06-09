# Vox — Packaging & Distribution Architecture (Native Stack)

---

## 1. Overview

Vox is distributed as a **self-contained native desktop application**.

The user should:

* download a single installer
* install the app
* run Vox immediately

No manual setup of:

* Python
* Node.js
* system dependencies

---

## 2. Packaging Philosophy

---

### ⚡ Native Binary System

```text
Vox App = UI (WebView) + Rust Runtime + Native Inference Engine
```

* React → bundled into Tauri WebView
* Rust → main runtime + system control
* C++ → inference layer (linked binaries)

---

### ⚡ Zero External Runtime

The system must NOT require:

* Python
* Conda
* CUDA
* external runtimes

Everything is bundled or handled internally.

---

### ⚡ Models are NOT Bundled

* models downloaded on first run
* stored in user directory
* upgradable independently

---

### ⚡ Hardware-Constrained Design

Packaging must respect:

* 8GB RAM systems
* CPU-only execution
* low disk footprint

---

## 3. System Architecture (Packaging View)

---

### Composition

```text
Tauri Application
├── UI (React build)
├── Rust Core
│   ├── Audio (cpal)
│   ├── IPC/Event System
│
├── Native Inference Layer
│   ├── ONNX Runtime (VAD + STT)
│   ├── llama.cpp (LLM)
│   └── TTS Engine
```

---

### Key Change

❌ Old:

```text
Python sidecar process
```

✅ New:

```text
Native libraries + compiled inference
```

---

## 4. Build Pipeline

---

### Step 1 — Build Frontend

```bash
pnpm build
```

Output:

```text
dist/ui/
```

---

### Step 2 — Build Native Components

---

#### Rust (Core)

```bash
cargo build --release
```

---

#### C++ Inference Layer

Options:

* statically linked binaries
  OR
* dynamic libraries bundled with app

Includes:

* ONNX Runtime
* llama.cpp
* TTS engine (Supertonic 3 — sherpa-onnx native, no external phonemizer deps)

> [!IMPORTANT]
> **TTS Architecture**: Supertonic 3 uses sherpa-onnx native C++ with built-in phonemization. No external espeak-ng or language-specific assets need bundling.

> [!IMPORTANT]
> **Linux Build Requirement**: To avoid `std::bad_alloc` crashes in `llama.cpp` caused by `libstdc++` bugs, Linux builds MUST link against LLVM's `libc++`.
>
> Run: `sudo apt install libc++-dev libc++abi-dev`
> Build with: `CXXFLAGS="-stdlib=libc++" LDFLAGS="-lc++"`

---

### Step 3 — Integrate with Tauri

```text
src-tauri/
  ├── target/release/
  ├── native/         # inference libs
  └── tauri.conf.json
```

---

### Step 4 — Bundle Application

```bash
pnpm tauri build
```

---

### Output

* Windows → `.exe`
* Linux → `.AppImage`, `.deb`
* macOS → `.dmg` (future)

---

## 5. Directory Structure

---

```bash
vox/
├── app/
│   ├── ui/
│   ├── src-tauri/
│   │   ├── native/          # ONNX + llama.cpp + TTS libs
│   │   ├── target/
│   │   └── tauri.conf.json
│
├── models/                  # downloaded at runtime
├── dist/
├── scripts/
```

---

## 6. Model Distribution

---

### First Run Flow

```text
App starts
→ checks model directory
→ downloads required models
→ verifies integrity
```

---

### Storage Location

```text
~/.vox/models/
```

---

### Benefits

* smaller installer
* flexible updates
* model switching without reinstall

---

## 7. Auto-Update System

---

### Mechanism

Tauri updater:

```text
App start
→ check release server
→ download update
→ apply patch
```

---

### Constraints

* binary compatibility must be maintained
* inference layer version must match app

---

## 8. Development vs Production

---

### Development Mode

```bash
pnpm tauri dev
```

* Rust runs in debug mode
* native libs loaded dynamically
* hot reload UI

---

### Production Mode

```bash
pnpm tauri build
```

* Rust optimized
* native inference bundled
* minimized binary

---

## 9. CI/CD Pipeline

---

### Steps

1. Install dependencies
2. Build frontend
3. Build Rust core
4. Build native inference layer
5. Bundle Tauri app
6. Upload artifacts

---

### Outputs

* `Vox-Setup.exe`
* `Vox.AppImage`
* `Vox.deb`

---

### Release Strategy

* GitHub Releases
* version tagging
* platform-specific builds

---

## 10. Key Challenges

---

### Native Dependency Management

* ONNX runtime size
* cross-platform compatibility
* library linking

---

### Binary Size

* multiple inference components
* need for optimization

---

### OS Differences

* audio systems differ
* packaging formats differ

---

## 11. Design Constraints

---

### Must Ensure

* single-click install
* fast startup
* minimal memory overhead

---

### Must Avoid

* Python bundling
* redundant libraries
* large default models

---

## 12. Final Principle

> Packaging is the **final integration layer of the system**.

The user should never see:

* inference complexity
* model management
* runtime architecture

Only:

```text
Vox — instant, real-time voice system
```
