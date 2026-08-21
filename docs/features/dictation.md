# 📄 `dictation.md` — Realtime Dictation Subsystem & Output Architecture (Phase 9)

---

## 1. Executive Summary & Core Concept

**Realtime Dictation** is a system-level, high-throughput speech-to-text pipeline in Vox inspired by Wispr-flow. It delivers instant, zero-latency transcription directly into any application on the operating system without incurring LLM reasoning or TTS synthesis overhead.

In Phase 9, Dictation is **fully decoupled from the desktop Tray HUD**:
- **Dictation Core**: The native audio capture, VAD gating, STT acoustic transcription, and Devanagari transliteration engine.
- **Output Mediums**: The transcription capability routes to mutually exclusive output destinations, where the desktop Tray HUD is simply one visual presentation medium among others.

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                                 DICTATION BACKEND PIPELINE                               │
│                                                                                          │
│  Mic Audio (16kHz) ──► VAD Engine (Earshot/TenVAD) ──► STT Engine (Nemotron/Qwen3-ASR)   │
│                                                                   │                      │
│                                                                   ▼                      │
│                                                     Transliteration Engine (Hindi/Dev)   │
│                                                                   │                      │
│                                                                   ▼                      │
│                                                       Fast-Path Pipeline Intercept       │
│                                                      (InteractionOwner::Dictation = 0)   │
│                                                                   │                      │
│                               ┌───────────────────────────────────┴─────────────────┐    │
│                               ▼                                                     ▼    │
│                        [Ptt Mode: Alt+Space]                                [Passive Mode]│
└───────────────────────────────┬─────────────────────────────────────────────────────┬────┘
                                │                                                     │
                                ▼                                                     ▼
                  ┌─────────────────────────── OUTPUT ROUTER ───────────────────────────┐
                  │                                                                     │
                  │   ┌─────────────────────┬──────────────────────┬────────────────┐   │
                  │   ▼                     ▼                      ▼                │   │
                  │ [Mode 1: Paste]       [Mode 2: Clipboard]    [Mode 3: Tray HUD] │   │
                  │ Simulated Keystroke   OS Clipboard Only      Floating Desktop   │   │
                  │ (Linux: Ctrl+V        (Silent copy without   Overlay Window     │   │
                  │  macOS: Cmd+V          keystroke injection)  (Persistent turns) │   │
                  │  Windows: Ctrl+V      350ms restoration)                        │   │
                  └─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Two Independent Decision Axes

Dictation configuration is governed by two independent, orthogonal settings axes:

### Axis 1: Interaction Mode (`interaction_mode`)
- **`Ptt` (Push-To-Talk, Default)**:
  - Triggered via global system shortcut (default `Alt+Space`).
  - **Zero Idle RAM Guarantee**: 0 ONNX models loaded on boot; `DictationController` lazily initializes audio/STT pipeline on-demand when the hotkey is first pressed.
  - Recording captures speech while held/toggled, and finishes on release.
- **`Passive` (Continuous Sense)**:
  - Audio engine is pre-warmed on application boot.
  - Continuously monitors microphone energy via 300ms VAD gate and dispatches transcripts automatically when speech boundaries conclude.

### Axis 2: Output Destination (`output_mode`)
Every output mode is **mutually exclusive** — at any given moment, transcription output routes to exactly one destination:
1. **`Paste` (Simulated Keystroke Injection)**:
   - Injects the transcribed text directly into the user's active cursor position in any app (browser, code editor, chat, terminal).
   - Executes via a platform-specific input adapter selected at compile time (see §9 Platform Compatibility):
     - **Linux X11**: `X11InputAdapter` → `enigo` + `x11rb` → `Ctrl+V`
     - **Linux Wayland**: `WaylandInputAdapter` → `enigo` (compositor-permitting) → `Ctrl+V`; graceful fallback to clipboard on compositor block
     - **macOS**: `MacOsInputAdapter` → `enigo` → **`Cmd+V`** (Meta key — not Ctrl+V)
     - **Windows**: `WindowsInputAdapter` → `enigo` Win32 SendInput → `Ctrl+V`
   - Uses safe clipboard backup and restore (`with_clipboard_safe`).
2. **`Clipboard` (Clipboard Only)**:
   - Silently writes the transcribed text to the system clipboard (`arboard`) without simulating key events.
   - Ideal for manual pasting workflows.
3. **`Tray` (Floating Desktop HUD Window)**:
   - Renders live streaming transcription inside the desktop floating overlay window (`TrayApp.tsx`).
   - Accumulates multi-turn transcripts separated by newlines (`\n`) for long-form dictation.

---

## 3. Fast-Path Pipeline Interception

When dictation is active, the pipeline owner is set to `InteractionOwner::Dictation = 0`.

In `services/pipeline/event_loop.rs`, upon receiving `VoxEvent::TranscriptFinal`:
```rust
if owner == InteractionOwner::Dictation {
    let transliterated = crate::services::utils::transliterate_if_hi(
        &text,
        true,
        transliterate_enabled,
    );
    
    // Route to selected output destination
    services::dictation::output_router::route_transcript(
        app_handle,
        &transliterated,
        output_mode,
        &dictation_last_transcript,
    ).await;

    // Reset interaction state to Idle and SKIP LLM + TTS completely
    state.pipeline.update_interaction_state(InteractionState::Idle, owner, app_handle);
    continue;
}
```

**Benefits**:
- **0ms LLM Overhead**: No prompt templating, context construction, token generation, or quantization lag.
- **0ms TTS Overhead**: No clause chunking, neural voice synthesis, or audio playback allocation.
- **Sub-150ms Perceived Latency**: Time from speech completion to pasted text is bounded only by STT inference time + 350ms clipboard restore window.

---

## 4. Clipboard Safety & Keystroke Injection Engine

### 4.1 Safety Contract (`with_clipboard_safe`)
Simulated paste requires writing text to the system clipboard and dispatching `Ctrl+V`. To prevent destroying previous user clipboard contents:

```
1. Capture current OS clipboard text (if present) -> Option<String>
2. Write final transcribed dictation text to OS clipboard
3. Dispatch simulated Ctrl+V keystroke via platform input adapter
4. If injection SUCCEEDED:
     Wait 350ms (allows target application to consume paste event)
     Restore original clipboard text captured in step 1
5. If injection FAILED:
     DO NOT restore clipboard (leaves dictation text intact so user can manual paste)
     Log error with full context and emit `dictation_error` event
```

### 4.2 Platform Adapters

| Platform | Adapter Struct | Backend | Paste Key | Notes |
|---|---|---|---|---|
| Linux X11 | `X11InputAdapter` | `enigo` + `x11rb` feature | `Ctrl+V` | Default on X11 session |
| Linux Wayland | `WaylandInputAdapter` | `enigo` (best-effort) | `Ctrl+V` | Falls back to clipboard-only on security block |
| macOS | `MacOsInputAdapter` | `enigo` (CGEvent) | **`Cmd+V`** | Correct macOS paste shortcut |
| Windows | `WindowsInputAdapter` | `enigo` Win32 SendInput | `Ctrl+V` | Standard Windows paste |

**Factory**: `create_input_adapter()` in `services/dictation/input.rs` selects the correct adapter at compile time via `#[cfg(target_os)]` branches. The old `#[cfg(not(target_os = "linux"))]` fallthrough to `X11InputAdapter` was a functional bug on macOS — fixed in the cross-platform pass.

**Clipboard layer**: `arboard` with `wayland-data-control` feature handles OS clipboard read/write across all platforms. The `wayland-data-control` feature is gracefully ignored on non-Wayland targets.

---

## 5. Floating Tray HUD Presentation System (Output Mode: `Tray`)

When `output_mode == DictationOutputMode::Tray`, the desktop floating overlay window is engaged.

### 5.1 UX & Visual Specifications
- **Dimensions**: Fixed `380px × 250px` floating glassmorphism card (`rounded-2xl`).
- **Positioning**: Right edge of display, vertically centered with configurable screen padding. Platform-specific:
  - **Linux X11/Wayland**: Fullscreen transparent GTK virtual layer with `cairo::Region` input shape (click-through) via `setup_linux_virtual_layer`. Handles fractional scaling.
  - **macOS / Windows**: `tauri-plugin-positioner` `Position::TopRight` — no virtual layer needed; `set_ignore_cursor_events(true)` handles click-through.
- **Styling**: `backdrop-filter: blur(20px) saturate(180%)`, background `rgba(var(--card), 0.88)`.
- **Transitions**: Smooth slide & opacity transitions (**150ms entry / 500ms exit**).

### 5.2 Visibility State Machine
```typescript
enum VisibilityState {
  HIDDEN = 'HIDDEN',      // Tray window hidden from desktop (unmapped)
  APPEARING = 'APPEARING', // 150ms slide-in & zoom reveal
  ACTIVE = 'ACTIVE',      // Fully visible, persistent, and interactive
  FADING = 'FADING'       // 500ms fade-out transition on sleep or manual dismiss
}
```

### 5.3 Turn Accumulation & Zero-Flicker Threshold
- **VAD 300ms Gate**: Mic noise, breathing, or accidental clicks are suppressed before STT processing.
- **Streaming Partial Updates**: The tray HUD appears on the first non-empty transcribed character.
- **Multi-Turn Continuity**: Each speech segment appends to the active text canvas with a newline (`\n`), enabling continuous dictation without clearing context between sentences.
- **Auto-Sleep**: After 3 minutes of inactivity, the current text session is auto-committed to ephemeral history and the HUD smoothly fades out.

### 5.4 Ephemeral Session History
- **In-Memory Ring Buffer**: Stores up to 15 recent dictation sessions in RAM.
- **Privacy Guarantee**: Never written to disk; wiped completely on runtime shutdown.
- **Navigation**: Footer `<` and `>` controls allow cycling through recent transcripts with one-click copy.

---

## 6. Transliteration & Transcript Recovery (FR-08)

### 6.1 Transliteration Invariant
Spoken Hindi/Devanagari text is passed through the ONNX transliteration model (`transliterate_if_hi`) across **all 3 output modes** before reaching the output router, ensuring consistent Romanized/Devanagari transcript output.

### 6.2 Transcript Recovery Engine
Every completed dictation transcript is stored in `AppState.dictation_last_transcript: Mutex<Option<String>>`.

If simulated paste fails or the user accidentally loses their pasted text:
1. **IPC Query**: `get_last_dictation_transcript()` returns the cached string.
2. **IPC Copy**: `copy_last_dictation_transcript()` copies the last transcript back to the OS clipboard and emits `dictation_transcript_copied`.

---

## 7. Zero Swallowed Errors Policy

All dictation errors are strictly typed in `DictationError` and propagated without swallowing:

```rust
pub enum DictationError {
    ClipboardFailed { message: String },
    InputSimulationFailed { message: String },
    HotkeyRegistrationFailed { message: String },
    EngineNotReady { message: String },
}
```

Every failure logs detailed diagnostic context and surfaces an error event to the frontend (`dictation_error`).

---

## 8. Settings & IPC Interface

### 8.1 Backend Data Structures
```rust
pub struct DictationSettings {
    pub enabled: bool,                            // Master switch
    pub interaction_mode: DictationInteractionMode, // Passive | Ptt
    pub hotkey: String,                            // "Alt+Space"
    pub output_mode: DictationOutputMode,          // Paste | Clipboard | Tray
}
```

### 8.2 Frontend Settings Desk
In `InteractionCard.tsx`, users toggle between **Assistant** and **Dictation**:
- **Voice Typing Switch**: Toggle `dictation.enabled`.
- **Trigger Mode**: Switch between `Push-To-Talk` and `Continuous`.
- **Output Destination**: Segmented control for `Simulated Paste`, `Clipboard Only`, and `Floating Tray`.
- **Activation Hotkey**: Interactive key combination badge with inline edit support.

---

## 9. Platform Compatibility Matrix

This section documents all platform-specific behavior, known limitations, and open verification items.

### 9.1 Paste Output Mode

| Capability | Linux X11 | Linux Wayland | macOS | Windows |
|---|---|---|---|---|
| Simulated paste shortcut | `Ctrl+V` | `Ctrl+V` (best-effort) | `Cmd+V` ✅ | `Ctrl+V` |
| Input simulation backend | `enigo` + x11rb | `enigo` (CGEvent fallback → clipboard) | `enigo` (CGEvent) | `enigo` Win32 SendInput |
| Clipboard backup/restore | ✅ `arboard` | ✅ `arboard` + wayland-data-control | ✅ `arboard` | ✅ `arboard` |
| Fallback on failure | Clipboard mode | Clipboard mode (security block) | Clipboard mode | Clipboard mode |

> **Known Limitation — macOS `enigo` CI verification**: The `enigo 0.2` macOS CGEvent path
> compiles cleanly in `cargo check` but has not been verified on a live macOS build in CI.
> The Cargo feature configuration (`default-features = false`, no extra features needed on macOS)
> is correct per `enigo 0.2` documentation. A macOS smoke test of dictation paste mode is
> required before the DMG target ships. Track as: `TODO(cross-platform): enigo macOS paste live test`.

### 9.2 Tray HUD Positioning & Click-Through

| Capability | Linux X11/Wayland | macOS | Windows |
|---|---|---|---|
| Window positioning | GTK virtual layer (fullscreen transparent, Cairo input shape) | `tauri-plugin-positioner` TopRight | `tauri-plugin-positioner` TopRight |
| Click-through | `gtk_window.input_shape_combine_region(cairo::Region)` | `set_ignore_cursor_events(true)` | `set_ignore_cursor_events(true)` |
| Fractional DPI scaling | Handled via `window.scale_factor()` in `setup_linux_virtual_layer` | Handled by OS | Handled by OS |
| HUD dims (logical px) | 380×250, padding 55px right, 15vh top | 380×250 | 380×250 |


### 9.3 Global PTT Hotkey (`Alt+Space`)

- **Linux**: Registered via `tauri-plugin-global-shortcut`. Works on X11 and Wayland (portal-permitting).
- **macOS**: Requires Accessibility permission in System Settings → Privacy & Security → Accessibility.
- **Windows**: Standard Win32 `RegisterHotKey` — no special permissions needed.

### 9.4 `enigo` Cargo Feature Configuration

The `x11rb` feature is Linux-only. macOS and Windows compile `enigo` with `default-features = false` and no additional feature flags (both platforms are supported by enigo's default build):

```toml
# Global — no platform features
enigo = { version = "0.2", default-features = false }

# Linux-only: enable x11rb backend
[target.'cfg(target_os = "linux")'.dependencies]
enigo = { version = "0.2", default-features = false, features = ["x11rb"] }
```

> **⚠️ CI Gap**: The macOS and Windows `enigo` builds have not been verified in CI.
> Cross-compile checks (`cargo check --target x86_64-apple-darwin` and
> `cargo check --target x86_64-pc-windows-msvc`) should be added to the pipeline.

> See [`performance-memory-optimizations.md`](./performance-memory-optimizations.md) for the
> full cross-platform heap trimming strategy (`trim_heap`) applied after model eviction.
