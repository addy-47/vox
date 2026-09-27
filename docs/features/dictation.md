---
title: "Vox Dictation Subsystem"
audience: "Internal — backend & frontend contributors"
last_updated: 2026-09-27
owners: "backend-engineer role"
related_docs:
  - "docs/backend.md §3, §8 — Pipeline & events"
  - "docs/features/voice-flow.md §8 — Dictation domain"
  - "docs/specs/integration-test-spec.md — Seam 4 post-refactor dictation truth"
  - "app/src-tauri/src/toast.rs — Cross-platform native desktop notification dispatcher"
  - "docs/specs/notifications-spec.md §8.4 — Native OS Desktop Notification Channel"
  - "docs/specs/ipc-spec.md §3.4 — Decommissioned show_toast IPC event"
---

# 📄 `dictation.md` — Realtime Dictation Subsystem & Output Architecture

---

## 1. Executive Summary & Core Concept

**Realtime Dictation** is a system-level, high-throughput speech-to-text pipeline in Vox inspired by Wispr-flow. It delivers instant, zero-latency transcription directly into any application on the operating system without incurring LLM reasoning or TTS synthesis overhead.

Dictation is **fully decoupled from the desktop Tray HUD and unified**: Passive and PTT share a modular handler package (`pipeline/dictation/{mod,ptt,speech,transcript,error}.rs`) instead of a single monolith. `services/dictation/` holds the reusable primitives (clipboard, input adapters, output_router, hotkey). The central router (`pipeline/router.rs:119-122`) fast-paths `owner==Dictation` events directly to `pipeline/dictation/mod.rs::handle_event` before the assistant handler match:
- **Dictation Core**: The native audio capture, VAD gating, STT acoustic transcription, and Devanagari transliteration engine.
- **Output Mediums**: The transcription capability routes to mutually exclusive output destinations, where the desktop Tray HUD is simply one visual presentation medium among others. Reusable primitives live in `services/dictation/` (`clipboard.rs`, `input.rs`, `output_router.rs`, `hotkey.rs`, `mod.rs`).

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
│                        [Ptt Mode: Alt+V]                                    [Passive Mode]│
└───────────────────┬─────────────────────────────────────────────────────────────┬────┘
                    │                                                     │
                    ▼                                                     ▼
      ┌─────────────────────────── OUTPUT ROUTER ───────────────────────────┐
      │                                                                     │
      │   ┌─────────────────────┬──────────────────────┬────────────────┐   │
      │   ▼                     ▼                      ▼                │   │
      │ [Mode 1: Paste]       [Mode 2: Clipboard]    [Mode 3: Tray] │   │
      │ Simulated Keystroke   OS Clipboard Only      Floating Desktop │   │
      │ (Linux: Ctrl+V        (Silent copy without   Overlay Window   │   │
      │  macOS: Cmd+V          keystroke injection)  (Persistent turns)│   │
      │  Windows: Ctrl+V      350ms restoration)                        │   │
      └─────────────────────────────────────────────────────────────────────┘
```

**Module decomposition**: The unified dictation handler is now `pipeline/dictation/mod.rs::handle_event` which dispatches to `ptt.rs`, `speech.rs`, `transcript.rs`, and `error.rs`. The `services/dictation/` crate holds `clipboard.rs`, `hotkey.rs` (OS integration home), `input.rs`, `output_router.rs`, and `mod.rs`.

---

## 2. Two Independent Decision Axes

Dictation configuration is governed by two independent, orthogonal settings axes:

### Axis 1: Interaction Mode (`dictation.interaction_mode`)
- **`Ptt` (Push-To-Talk, Default)**:
    - Triggered via global system shortcut (default `Alt+V`).
    - **Zero Idle RAM Guarantee**: 0 ONNX models loaded on boot; the unified dictation handler (`pipeline/dictation/mod.rs::handle_event`) lazily initializes audio/STT pipeline on-demand when the hotkey is first pressed.
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

When dictation is active, the pipeline owner is `InteractionOwner::Dictation` (`core/state.rs:57`). The central router dispatches all `VoxEvent`s to `pipeline/dictation/mod.rs::handle_event` (`pipeline/router.rs:119-122`). On `VoxEvent::TranscriptFinal`:

```rust
// Dispatches to OS input router and resets state to Idle
tauri::async_runtime::spawn(async move {
    if let Err(e) =
        crate::services::dictation::output_router::route_transcript(&app_handle, &text_clone)
            .await
    {
        log::warn!("[Dictation] Output routing failed: {}", e);
    }
});

transition(InteractionState::Idle, &ctx, app, state);
```

Dictation has no `start_session`/`end_session` — it rides on the audio engine lifecycle (`lib.rs:360-395` auto-launch for Passive, lazy `ensure_engine_running` for PTT) and the router ownership check. `InteractionState` (`core/state.rs:86-96`) now includes `Sleeping = 7` and `Working = 8` variants; dictation uses `Idle`, `Ready`, `Listening`, `Thinking`, and `Error` states. The unified `transition_dictation()` function (`pipeline/dictation/mod.rs:14`) emits `StateChanged` to `AppWindow::Tray`.

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
6. If injection is UNVERIFIED (Wayland compositor reports success but swallows synthetic
   keystrokes without error — `enigo` returns `Ok` while nothing is delivered):
     Treat exactly as FAILED (no clipboard restore; transcript stays on clipboard).
     Replace the live notification in place with the persistent `📋 Dictation Copied
     (press Ctrl+V)` card. Never log a false "successfully pasted" milestone.
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

## 6. Toast Notification System — Transient Feedback Layer

Dictation's only user-visible confirmation outside `Tray` mode is the **toast overlay** — a dedicated, ephemeral `WebviewWindow` (`label: "toast"`) that surfaces copy/paste success, fallback, and error states without stealing focus. It is **distinct** from the Tray HUD (`Tray` is an output destination that accumulates turns; toast is a transient notification on top of whatever the user is doing).

### 6.1 Architecture & Ownership

All dictation lifecycle cards route strictly through the Notification Service front door (`notifications-spec.md` §11.1 — upstream code never touches the toast dispatcher directly):

```
output_router.rs / ptt.rs / transcript.rs / hotkey.rs
        │  services::notifications::lifecycle::{dictation_listening, dictation_live_update,
        │      dictation_transcribing, dictation_terminal}  (Transient, group "dictation:lifecycle")
        ▼
lifecycle.rs  ──►  resident replace-ID card (Linux --print-id / --replace-id)
        │
        ├──► card exists: update in place (service-owned throttle ~250ms)
        └──► no card / terminal fallback: notify() front door ──► ToastOnly + drawer elevation
                │
                ▼
        toast.rs (dumb dispatcher)  ──►  dispatch_native_notification()
                │
                ├──► Linux:   notify-send -a Vox -i ~/.vox/icons/vox.png -t <duration> <title> <msg>
                ├──► macOS:   osascript -e 'display notification ... with title ... subtitle "Vox"'
                └──► Windows: powershell.exe WinRT [Windows.UI.Notifications.ToastNotificationManager]
```

* **Zero-Webview Architecture:** All ephemeral floating alerts are delivered natively through host operating system notification daemons outside the application webview. This eliminates the 80MB RAM GTK webview overlay window (`ToastApp.tsx`), transparent Cairo shape masks, and Linux compositor black flashes.
* **Non-Stealing / Non-Intrusive:** Native OS notifications do not steal focus from the user's active cursor or text editor and require zero complex positioning coordinates.

### 6.2 Dictation Lifecycle Notifications (Every Logical Boundary)

| Phase / Trigger | Code Path | Title | Message & Markup | Severity | Condition |
|---|---|---|---|---|---|
| **1. Start Listening (persistent)** | `hotkey.rs` | `🎙️ Dictation` | `\n<b>Listening...</b> Speak clearly · a 1.2s pause auto-finishes` | `Info` | Hotkey pressed; turn begins; notification replace-ID retained for in-place updates; 600ms auto-repeat guard armed |
| **1b. Live Partial (throttled update)** | `transcript.rs` via STT partials | `🎙️ Dictation` | `\n<b>Listening...</b>\n"<live partial text>"` | `Info` | Same notification ID replaced in place (~250ms throttle); never a new popup |
| **2. Speech Ended (Transcribing)** | `ptt.rs` | `⏳ Dictation` | `\n<b>Transcribing...</b> Processing speech` | `Info` | Same notification ID replaced; manual tap-stop or 1.2s silence auto-stop |
| **3. Speech Not Detected / Empty** | `ptt.rs` / `transcript.rs` | `⚠️ Dictation: No Speech` | `\n<b>No speech recognized</b>\nSpeak clearly into the microphone` | `Warning` | Audio <100ms or VAD speech not detected or empty STT transcript; replaces live notification in place |
| **4. Paste Succeeded** | `output_router.rs` | `✓ Dictation Pasted` | `\n"{snippet}"` | `Info` | Mode `Paste`: verified keystroke injection delivered into cursor; replaces live notification in place |
| **5. Paste Blocked / Unverified (Clipboard Fallback)** | `output_router.rs` | `📋 Dictation Copied` | `\n<b>Saved to clipboard</b> (press Ctrl+V):\n"{snippet}"` | `Warning` | Mode `Paste`: Wayland/OS blocked or unverified injection; clipboard is NOT restored so the transcript stays pastable; replaces live notification in place with persistent expiry |
| **6. Clipboard Mode** | `output_router.rs` | `📋 Dictation Copied` | `\n<b>Saved to clipboard</b> (press Ctrl+V):\n"{snippet}"` | `Info` | Mode `Clipboard`: written to clipboard |


* **Dimensions:** `360px × 96px` (`TOAST_WIDTH/HEIGHT` `toast.rs:8`), positioned top-center with `24px` top inset (`TOAST_PAD_TOP`). Glassmorphism `glass-card` (`rounded-xl`, `border`, `blur(20px) saturate(180%)` via outer window shape), outer wrapper `w-screen h-screen flex items-start justify-center bg-transparent pointer-events-none p-6`.
* **Stack:** `React` + `framer-motion` `AnimatePresence`; entry `opacity 0→1, y -12→0` `duration 0.36 spring [0.16,1,0.3,1]`, exit `y -12`, inner fade `320ms` before `hide` + `280ms` before `destroy_toast_window_cmd`.
* **Levels (`ToastApp.tsx:8`):** `success → CheckCircle2 @ accent`, `warning → AlertTriangle @ warning`, `error → AlertCircle @ error`, `info → Info @ muted`; each with `bg`/`border` alpha variants and a 1× `bg-[rgba(border,0.06)]` progress track with `scaleX(progress)` fill at `opacity 0.45`.
* **Resilience:** `onShowToast` listen + 700ms `get_last_toast` poll covers cold-start race where the backend's immediate emit lands before `ToastApp` mounts; the backend's 420ms/300ms re-emits cover the inverse.

### 6.4 Relationship to Other Pipelines & Dev Poll

The same toast layer is reused outside dictation for modular pipeline errors (`pipeline/assistant/error.rs`) and realtime failures. A dev-only poll in `lib.rs:350` emits a rotating test toast every 60s (titles `Dictation Copied` / `Dictation Pasted` / `Paste Blocked by OS` / `Voice Error` with `Success/Warning/Critical` levels) to exercise the fallback chain; it does not affect the dictation contract.

---

## 7. Transliteration & Transcript Recovery (FR-08)

### 7.1 Transliteration Invariant
Spoken Hindi/Devanagari text is passed through the ONNX transliteration model (`transliterate_if_hi`) across **all 3 output modes** before reaching the output router, ensuring consistent Romanized/Devanagari transcript output.

### 7.2 Transcript Recovery Engine
Every completed dictation transcript is stored in `AppState.dictation_last_transcript: Mutex<Option<String>>`.

If simulated paste fails or the user accidentally loses their pasted text:
1. **IPC Query**: `get_last_dictation_transcript()` returns the cached string.
2. **IPC Copy**: `copy_last_dictation_transcript()` copies the last transcript back to the OS clipboard and emits `dictation_transcript_copied`.

---

## 8. Zero Swallowed Errors Policy

All dictation errors are strictly typed in `DictationError` and propagated without swallowing. Errors dispatch through `pipeline/dictation/error.rs::on_error` which uses `IpcEvent::ShowToast` with `Severity::Critical` and the `should_show_error_toast` gate. The `voice_error` IPC event was removed in the Phase 11 refactor.

---

## 9. Settings & IPC Interface

### 9.1 Backend Data Structures
```rust
pub struct DictationSettings {
    pub enabled: bool,
    pub interaction_mode: DictationInteractionMode, // Passive | Ptt (snake_case)
    pub hotkey: String,
    pub output_mode: DictationOutputMode, // Paste | Clipboard | Tray
}
```
Note: `DictationInteractionMode` uses `#[serde(rename_all = "snake_case")]`, so the serialized form is `"passive"` or `"ptt"`. The frontend `settingsStore.ts` mirrors this with `interaction_mode: "passive" | "ptt"`. Dictation settings are mutated via `apply_dictation_mutation` in `ipc/settings/mutation.rs`; `enabled` and `interaction_mode` changes trigger `transition_dictation(Ready|Idle)` via `ipc/settings/core.rs`.

### 9.2 Frontend Settings Desk
In `InteractionCard.tsx`, users toggle between **Assistant** and **Dictation**:
- **Voice Typing Switch**: Toggle `dictation.enabled`.
- **Trigger Mode**: Switch between `Push-To-Talk` and `Continuous` (mapped to `DictationInteractionMode::Ptt` / `Passive`).
- **Output Destination**: Segmented control for `Simulated Paste`, `Clipboard Only`, and `Floating Tray`.
- **Activation Hotkey**: Interactive key combination badge with inline edit support.
- **`interaction_mode` normalization**: `shared/lib/interactionMode.ts` exports `InteractionModeUpper = "PASSIVE" | "PTT"` for the assistant domain; dictation uses `DictationInteractionMode` directly.

---

## 10. Platform Compatibility Matrix

This section documents all platform-specific behavior, known limitations, and open verification items.

### 10.1 Paste Output Mode

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

### 10.2 Tray HUD Positioning & Click-Through

| Capability | Linux X11/Wayland | macOS | Windows |
|---|---|---|---|
| Window positioning | GTK virtual layer (fullscreen transparent, Cairo input shape) | `tauri-plugin-positioner` TopRight | `tauri-plugin-positioner` TopRight |
| Click-through | `gtk_window.input_shape_combine_region(cairo::Region)` | `set_ignore_cursor_events(true)` | `set_ignore_cursor_events(true)` |
| Fractional DPI scaling | Handled via `window.scale_factor()` in `setup_linux_virtual_layer` | Handled by OS | Handled by OS |
| HUD dims (logical px) | 380×250, padding 55px right, 15vh top | 380×250 | 380×250 |


### 10.3 Global PTT Hotkey (`Alt+V`)

- **Default Shortcut**: `Alt+V` across all platforms (configurable in Settings → Interaction → Activation Shortcut).
- **macOS**: Requires Accessibility permission in System Settings → Privacy & Security → Accessibility.
- **Windows**: Standard Win32 `RegisterHotKey` — no special permissions needed.
- **Linux X11**: Registered via `tauri-plugin-global-shortcut` (X11 `XGrabKey` on the root window).
- **Linux Wayland (GNOME Compositor)**:
  - *Wayland Security Model & RCA*: Under Wayland, compositors (such as GNOME Mutter) intentionally isolate applications from background keyboard snooping. Legacy X11 `XGrabKey` registrations made through `tauri-plugin-global-shortcut` on Xwayland (`:0`) fail silently whenever a native Wayland client (browser, terminal, editor, or Vox's own WebKitGTK surface) has focus. Furthermore, Ubuntu 24.04 (GNOME 46) does not implement `org.freedesktop.portal.GlobalShortcuts` in `xdg-desktop-portal-gnome`, and `org.gnome.Shell.GrabAccelerator` rejects unprivileged applications with `Access denied`.
  - *Automated Zero-Configuration Solution*: Vox implements a fully automated, native GNOME compositor integration that requires zero sudo or manual user intervention:
    1. **Local Unix Domain Socket Daemon (`~/.vox/vox.sock`)**: On boot, Vox spawns a lightweight background listener on `~/.vox/vox.sock` managed by the Tokio async runtime.
    2. **Self-Provisioning Trigger Script (`~/.vox/bin/vox-trigger`)**: Vox automatically generates an executable launcher script that dispatches `"trigger\n"` to `~/.vox/vox.sock` via `nc -U` (or `python3` fallback). If Vox is not running, the script exits immediately in <1ms.
    3. **Automated GNOME Media-Keys Daemon Registration**: Vox inspects `XDG_CURRENT_DESKTOP`. When running under GNOME, it automatically registers/syncs a custom keybinding via `gsettings` (`org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/vox-dictation/`):
       - `name`: `'Vox Dictation'`
       - `command`: `~/.vox/bin/vox-trigger`
       - `binding`: `<Alt>v` (normalized automatically from Settings format)
    4. **Interactive Operation (Press-to-Talk Toggle + 1.2s Silence Auto-Stop)**:
       - **Activation (Press 1)**: Pressing `Alt+V` globally triggers `HotkeyAction::Toggle`. If Vox dictation is in `Ready` or `Idle`, it boots the audio engine on-demand (preserving zero idle RAM), locks `InteractionOwner::Dictation`, and sends `VoxEvent::PttStart` (transitions to `Listening`).
       - **Finalization (Press 2 or 1.2s Silence)**: Pressing `Alt+V` again sends `VoxEvent::PttStop` immediately (transitions to `Thinking`, validates speech window via VAD, transcribes via STT, and injects/copies via Output Router). Alternatively, the user can simply stop speaking: a silence watchdog in the windowed-validation path auto-dispatches `PttStop` once speech has been detected and no speech frames arrive for **1200ms** (`DICTATION_SILENCE_AUTOSTOP_MS` in `services/dictation/mod.rs`). Pre-speech silence never triggers auto-stop; only post-speech pauses do.
    5. **Dynamic Settings Sync**: When the user updates the hotkey in the Dictation Settings Desk, Vox automatically updates both `tauri-plugin-global-shortcut` and GNOME's GSettings binding in real time. Disabling dictation cleans up the registration.

### 10.4 `enigo` Cargo Feature Configuration

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
>
> **Note on `voice_error`**: The `voice_error` IPC event and `VoiceErrorPayload` were removed in the Phase 11 refactor. Dictation errors now dispatch through `pipeline/dictation/error.rs::on_error` which uses `IpcEvent::ShowToast` with `Severity::Critical` and the `should_show_error_toast` gate.

---

**Last Updated:** 2026-09-10 — module decomposition (`pipeline/dictation/{mod,ptt,speech,transcript,error}.rs`), `DictationState` collapsed into `InteractionState`, `voice_error` removed, `ToastPayload` uses `Severity`, `hotkey.rs` relocated to `services/dictation/`, `transition_dictation` in `pipeline/dictation/mod.rs`.
