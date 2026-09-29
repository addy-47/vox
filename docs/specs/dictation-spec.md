# Dictation Subsystem & Output Architecture Specification (v1)

---

- **Status:** Approved Target Spec (SSOT for Dictation Subsystem & OS Output Routing)
- **Policy:** Zero Backward Compatibility (ZBC) — Clean architectural boundaries; no legacy wrappers.
- **Scope:** Defines audio capture, fast-path pipeline interception, platform input simulation, output routing, and session recovery contracts.
- **Related Specs:**
  - [`docs/specs/events-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/events-spec.md) — Pipeline event routing and interaction state transitions.
  - [`docs/specs/ownership-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/ownership-spec.md) — Dual-track ownership model and hotkey preemption gating.
  - [`docs/specs/notifications-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/notifications-spec.md) — SSOT for dictation lifecycle cards and OS toast dispatch.
  - [`docs/specs/ipc-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md) — Dictation settings mutations and transcript recovery IPC commands.

---

## 1. Executive Summary & Core Architectural Invariants

The **Dictation Subsystem** is Vox's high-throughput, low-latency speech-to-text pipeline. It transcribes user speech and delivers it directly into any active operating system application or overlay without incurring LLM reasoning loops or TTS audio synthesis overhead.

### Architectural Invariants (Non-Negotiable)

1. **Fast-Path Pipeline Bypass:**
   When dictation is active (`owner == InteractionOwner::Dictation`), the central event router (`pipeline/router.rs`) fast-paths all pipeline events directly to `pipeline/dictation/mod.rs::handle_event` before any assistant handler match. Dictation executes with:
   - **0ms LLM Overhead:** No prompt templating, context construction, token generation, or quantization lag.
   - **0ms TTS Overhead:** No clause chunking, neural voice synthesis, or audio playback allocation.
2. **State Check Over Settings Check:**
   Runtime routing code never branches on `settings.dictation.enabled`. State is driven strictly by `dictation_state` (`InteractionState::Idle` $\iff$ disabled, `InteractionState::Ready` $\iff$ enabled and standing by).
3. **Decoupled Output Routing:**
   Transcription delivery routes to strictly mutually exclusive output destinations managed by `services/dictation/output_router.rs`. The desktop Tray HUD is merely one visual presentation medium among others.
4. **Notification Layer Delegation:**
   Dictation emits typed lifecycle events strictly through `services::notifications::lifecycle::dictation_*`. All desktop notification formatting, replace-IDs, and platform dispatch contracts are governed authoritatively by [`docs/specs/notifications-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/notifications-spec.md).

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                               DICTATION BACKEND PIPELINE                               │
│                                                                                        │
│  Mic Audio (16kHz) ──► VAD Engine (Earshot/TenVAD) ──► STT Engine (Nemotron/Qwen3)     │
│                                                                   │                    │
│                                                                   ▼                    │
│                                                     Transliteration Engine (Hindi/Dev) │
│                                                                   │                    │
│                                                                   ▼                    │
│                                                       Fast-Path Pipeline Intercept     │
│                                                      (InteractionOwner::Dictation = 0) │
│                                                                   │                    │
│                               ┌───────────────────────────────────┴────────────────┐   │
│                               ▼                                                    ▼   │
│                      [Ptt Mode: Ctrl+Alt+V]                                  [Passive] │
└───────────────────────────────┬────────────────────────────────────────────────────┬───┘
                                │                                                    │
                                ▼                                                    ▼
                  ┌─────────────────────────── OUTPUT ROUTER ───────────────────────────┐
                  │                                                                     │
                  │   ┌─────────────────────┬──────────────────────┬────────────────┐   │
                  │   ▼                     ▼                      ▼                │   │
                  │ [Mode 1: Paste]       [Mode 2: Clipboard]    [Mode 3: Tray]     │   │
                  │ Simulated Keystroke   OS Clipboard Only      Floating Desktop   │   │
                  │ (AT-SPI / Enigo)      (Silent copy without   Overlay Window     │   │
                  │                       keystroke injection)   (Persistent turns) │   │
                  └─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Interaction Modes & Execution Lifecycles

Dictation operates under two mutually exclusive interaction modes (`dictation.interaction_mode`):

### 2.1 Push-To-Talk Mode (`DictationInteractionMode::Ptt`, Default)
- **Activation**: Triggered via global system hotkey (default `Ctrl+Alt+V`).
- **Zero Idle RAM Guarantee**: 0 ONNX models loaded on system boot. The unified dictation handler lazily initializes the audio engine and STT pipeline on-demand when the hotkey is first pressed (`ensure_engine_running`).
- **Press-to-Talk Toggle & Silence Auto-Stop**:
  - **Press 1 (Activation)**: Globally triggers `HotkeyAction::Toggle`. Transitions dictation state to `Listening`, locks dictation ownership, and begins recording speech frames.
  - **Press 2 or 1.2s Silence (Finalization)**: Pressing the hotkey again sends `VoxEvent::PttStop` immediately (transitions to `Thinking`). Alternatively, if the user stops speaking, a silence watchdog in `services/dictation/mod.rs` auto-dispatches `PttStop` once speech has concluded and no speech frames arrive for **1200ms** (`DICTATION_SILENCE_AUTOSTOP_MS`).
- **Auto-Eviction**: After 5 minutes of inactivity, idle audio and STT resources are eligible for automatic eviction.

### 2.2 Passive Continuous Mode (`DictationInteractionMode::Passive`)
- **Activation**: Audio engine is pre-warmed on application boot.
- **Continuous Monitoring**: Microphone energy is continuously monitored via a 300ms VAD gate. When speech boundary concludes, transcripts are automatically packaged into `VoxEvent::TranscriptFinal` and routed to the active output destination.

---

## 3. Output Destinations & Platform Input Simulation

The output medium is governed by `dictation.output_mode`, with exactly one destination active at any time:

### 3.1 Mode 1: Simulated Keystroke Injection (`DictationOutputMode::Paste`)
Transcribed text is injected directly into the user's active cursor position in any desktop application via platform-specific input adapters instantiated by `create_input_adapter()` in `services/dictation/input.rs`.

#### Platform Input Adapters Matrix
| Platform | Adapter Struct | Primary Backend | Paste Keystroke | Fallback Backend |
| :--- | :--- | :--- | :--- | :--- |
| **Linux Wayland** | `WaylandInputAdapter` | **Dynamic AT-SPI loader** (`libatspi.so.0` / `libatspi.so`) | `Ctrl+V` | `enigo` (best-effort) $\to$ clipboard preservation |
| **Linux X11** | `X11InputAdapter` | `enigo` + `x11rb` feature | `Ctrl+V` | Clipboard preservation |
| **macOS** | `MacOsInputAdapter` | `enigo` (CGEvent) | **`Cmd+V`** (Meta+V) | Clipboard preservation |
| **Windows** | `WindowsInputAdapter`| `enigo` (Win32 `SendInput`) | `Ctrl+V` | Clipboard preservation |

#### Linux Wayland Dynamic AT-SPI Loader Contract
Under GNOME Wayland (Mutter), synthetic keystrokes dispatched through standard X11/XTest or Enigo are silently swallowed by compositor security policies when native Wayland clients have focus. Vox solves this via dynamic runtime loading:
1. `AtspiLibrary::load()` dynamically resolves `atspi_init` and `atspi_generate_keyboard_event` via `libc::dlopen` from candidate sonames (`libatspi.so.0`, `libatspi.so`).
2. Synthesizes hardware-level keyboard events directly through the Linux accessibility bus (`org.a11y.atspi.Registry` on `/run/user/<uid>/at-spi/bus`):
   - Key press: `KEYSYM_CONTROL_L` (`65507`, `ATSPI_KEY_PRESS = 0`).
   - Key click: `KEYSYM_V_LOWER` (`118`, `ATSPI_KEY_PRESSRELEASE = 2`).
   - Key release: `KEYSYM_CONTROL_L` (`65507`, `ATSPI_KEY_RELEASE = 1`).
3. If AT-SPI initialization fails or is absent, the adapter attempts `enigo` simulation. If the compositor is known to block synthetic input (`is_blocking_compositor()`), injection is treated as unverified.

#### Clipboard Safety Contract (`with_clipboard_safe`)
Simulated paste temporarily writes transcript text to the system clipboard (`arboard`) before dispatching the paste key combination. To prevent destroying existing user clipboard contents:
1. Capture existing OS clipboard text (if present) $\to$ `Option<String>`.
2. Write transcribed text to OS clipboard.
3. Dispatch simulated paste keystroke via platform input adapter.
4. **On Injection Success**:
   - Wait **350ms** (`CLIPBOARD_RESTORE_DELAY_MS`) allowing the target application to consume the paste event.
   - Restore original clipboard text captured in step 1.
5. **On Injection Failure or Unverified Wayland Session**:
   - **Do NOT restore clipboard**. Preserves the dictation transcript on the clipboard so the user can paste manually (`Ctrl+V`).
   - Spawns a background keeper thread holding clipboard ownership alive for **30,000ms** (`CLIPBOARD_KEEPER_ALIVE_MS`).
   - Dispatches terminal notification card: *"📋 Dictation Copied (press Ctrl+V)"*.

### 3.2 Mode 2: System Clipboard Only (`DictationOutputMode::Clipboard`)
Silently writes the transcribed text to the system clipboard via `arboard` without simulating key events. Leaves previous clipboard contents overwritten by user intent and holds text alive for 30s.

### 3.3 Mode 3: Floating Tray HUD (`DictationOutputMode::Tray`)
Renders transcription inside the dedicated desktop floating overlay window (`AppWindow::Tray` / `TrayApp.tsx`):
- **Linux X11/Wayland Virtual Layer**: Renders through a fullscreen transparent GTK virtual layer with Cairo input shape mask (`input_shape_combine_region`) positioning the 380px × 250px glassmorphic card with click-through across the rest of the display.
- **macOS / Windows**: Positioned top-right with `tauri-plugin-positioner` and `set_ignore_cursor_events(true)`.
- **Multi-Turn Continuity**: Speech turns append sequentially separated by newlines (`\n`).
- **RAM-Only Ephemeral Ring Buffer**: Stores up to 15 recent turns in memory; wiped completely on application exit.

---

## 4. Global Hotkey & OS Compositor Integration

- **Default Keybinding:** `Ctrl+Alt+V` across all platforms (configurable via Settings $\to$ Dictation $\to$ Shortcut).
- **Linux GNOME Wayland Integration:**
  1. On boot, Vox spawns a local UNIX domain socket listener at `~/.vox/vox.sock`.
  2. Generates an executable trigger script at `~/.vox/bin/vox-trigger` that writes to the socket.
  3. Automatically registers a GNOME media-keys custom keybinding via GSettings (`org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/.../vox-dictation/`).
  4. Changes in Dictation Settings automatically sync GSettings and `tauri-plugin-global-shortcut` in real time.
- **macOS:** Requires macOS Accessibility permissions (`System Settings -> Privacy & Security -> Accessibility`).
- **Windows:** Registered via standard Win32 `RegisterHotKey`.

---

## 5. Persistence, State & Transcript Recovery

- **Zero LLM Database Storage:** Dictation utterances do not create session rows or message records in `sessions` or `messages` tables.
- **Transcript Recovery Cache:** Every completed transcript is cached in `AppState.dictation_last_transcript: Mutex<Option<String>>`.
  - `get_last_dictation_transcript()` IPC command returns the cached string.
  - `copy_last_dictation_transcript()` IPC command copies the last transcript back to the clipboard.
- **Transliteration Contract:** Spoken Hindi/Devanagari text passes through `transliterate_if_hi` before reaching the output router, preserving consistent Devanagari/Romanized output across all modes.

---

## 6. Deferred Future Scope (Post-v1 Architecture)

### 6.1 Neural Dictation Speech Refinement Engine (v2)
- **Scope**: A compact, edge-hosted Small Language Model (SLM, e.g. `LiquidAI/LFM2.5-230M` or `Qwen3.5-0.8B`) slotted between acoustic STT and the Output Router.
- **Capabilities**:
  - Speech repairs & backtracking (*"Let's meet Friday wait no Saturday"* $\to$ *"Let's meet Saturday"*).
  - Inverse Text Normalization (spoken numbers, dates, currency to digits, e.g. *"twenty five dollars"* $\to$ *"$25"*).
  - Punctuation, truecasing, quotes (*"quote ... unquote"* $\to$ *'"..."'*), and filler removal (*"um"*, *"uh"*).
  - Strictly English-only for initial rollout; Devanagari bypasses the refiner.
- **Safety Guards**: Deterministic Rust runtime validation (number conservation, length-ratio clamp) with zero-cost regex fallback.

### 6.2 Floating Caret Overlay Pill UI (v2)
- **Scope**: Ephemeral, cursor-anchored glassmorphic pill rendering staged, live, uncommitted transcription directly at the active text caret.
- **Execution**: Subject to platform accessibility caret coordinate discovery (`AXUIElement` on macOS, UI Automation on Windows, AT-SPI on Linux). Finalized text snaps into the target app via simulated paste upon hotkey release.
