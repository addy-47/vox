---
title: "Vox Dictation Subsystem"
audience: "Internal — backend & frontend contributors"
last_updated: 2026-09-01
owners: "backend-engineer role"
related_docs:
  - "docs/backend.md §3, §8 — Pipeline & events"
  - "docs/features/voice-flow.md §8 — Dictation domain"
  - "docs/plans/phase10/pipeline_orchestration_spec.md §7.5 — Dictation domain SSOT"
  - "app/src-tauri/src/toast.rs — Toast window lifecycle & emit chain"
  - "app/src-tauri/src/core/events.rs:105 — ToastPayload / IpcEvent::ShowToast"
  - "app/src/toast/ToastApp.tsx — Toast presentation & show_toast handling"
  - "AUDIT_IPC.md §3.D/§8 — Toast IPC surface (86-command audit)"
---

# 📄 `dictation.md` — Realtime Dictation Subsystem & Output Architecture (Phase 10)

---

## 1. Executive Summary & Core Concept

**Realtime Dictation** is a system-level, high-throughput speech-to-text pipeline in Vox inspired by Wispr-flow. It delivers instant, zero-latency transcription directly into any application on the operating system without incurring LLM reasoning or TTS synthesis overhead.

Dictation is **fully decoupled from the desktop Tray HUD and unified**: Passive and PTT share a single `pipeline/dictation.rs` handler (no split files). `services/dictation/` holds the reusable primitives (clipboard, input adapters, output_router, hotkey). The central router (`pipeline/router.rs:12`) fast-paths `owner==Dictation` events directly to `pipeline/dictation.rs::handle_event` before the assistant handler match:
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
  - **Zero Idle RAM Guarantee**: 0 ONNX models loaded on boot; the unified dictation handler (`pipeline/dictation.rs`) lazily initializes audio/STT pipeline on-demand when the hotkey is first pressed.
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

When dictation is active, the pipeline owner is `InteractionOwner::Dictation` (`core/state.rs:10`). The central router dispatches all `VoxEvent`s to `pipeline/dictation.rs::handle_event` (`pipeline/router.rs:10-14`). On `VoxEvent::TranscriptFinal`:

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

Dictation has no `start_session`/`end_session` — it rides on the audio engine lifecycle (`lib.rs:360-395` auto-launch for Passive, lazy `ensure_engine_running` for PTT) and the router ownership check. While `owner==Assistant`, global hotkey `Press` events are received but are a no-op (assistant has exclusive mic priority). `DictationState` (`Recording→Transcribing→Idle`) is emitted to the tray via `emit_dictation_state` (`pipeline/dictation.rs:9`) mapping `Transcribing` to tray string `"Thinking"` for UI reuse.

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

## 6. Toast Notification System — Transient Feedback Layer

Dictation's only user-visible confirmation outside `Tray` mode is the **toast overlay** — a dedicated, ephemeral `WebviewWindow` (`label: "toast"`) that surfaces copy/paste success, fallback, and error states without stealing focus. It is **distinct** from the Tray HUD (`Tray` is an output destination that accumulates turns; toast is a transient notification on top of whatever the user is doing).

### 6.1 Architecture & Ownership

```
output_router.rs (dispatch_to_clipboard / dispatch_to_paste / on_error)
        │  crate::toast::show_toast(app, title, msg, level)
        ▼
toast.rs::show_toast()  ──►  ensure_toast_window("toast")  (lazy, visible:false)
        │                     LAST_TOAST: LazyLock<Mutex<Option<ToastPayload>>>  (toast.rs:12)
        ├──► emit_ipc_to("toast", IpcEvent::ShowToast(payload))   immediate (core/events.rs:238)
        └──► async fallback chain: 420ms emit → 300ms re-emit → 300ms → w.show() if still hidden
                                   + position_toast_window() → setup_linux_toast_layer()
        ▼
Frontend ToastApp.tsx  ──►  onShowToast (eventsService.ts:220)  →  show(payload)
                             ├─ getCurrentWindow().show() (owns first paint, avoids black flash)
                             ├─ fallback invoke("show_toast_window")  (toast.rs:163)
                             ├─ renders 360×96 glass-card, progress bar, auto-dismiss 3400ms
                             └─ on mount also polls invoke("get_last_toast") at 700ms for late joiners
```

* **Window lifecycle:** `ensure_toast_window` (`toast.rs:22`) builds `360×96`, `transparent:true`, `decorations:false`, `always_on_top:true`, `visible:false`, `skip_taskbar:true`. The window **stays hidden** until `ToastApp` has mounted and painted its first frame — this eliminates the WebKitGTK black flash. Backend emits immediately but also retries (420ms + 300ms) and finally falls back to `w.show()` (`toast.rs:254`) if the event was missed. `position_toast_window` (`toast.rs:94`) centers at top with `24px` inset; on Linux it installs a fullscreen transparent GTK virtual layer with a `cairo::Region` input shape so only the 360×96 rect is hit-testable (`setup_linux_toast_layer`, `toast.rs:113`).
* **IPC contract — SSOT `core/events.rs:105`:** `ToastPayload { title: String, message: String, level: ToastLevel, duration_ms?: u64 }`, `ToastLevel { Success, Warning, Error, Info }` (`serde(rename_all="lowercase")`), `IpcEvent::ShowToast(ToastPayload)` → `name="show_toast"` (`events.rs:156`), `emit_ipc_to("toast", …)` targeted to the toast webview (not broadcast). Mirrored in `services/eventsService.ts:93` (`ToastPayload`, `ToastLevel`, `IpcEventMap["show_toast"]`, `onShowToast`). Every backend call goes through `toast::show_toast`; no raw string literals at emit sites.
* **Commands (`lib.rs:511` + `toast.rs:162`):** `show_toast_window` (sync, frontend fallback after `getCurrentWindow().show()`), `hide_toast_window` (`toast.rs:176`), `destroy_toast_window_cmd` (`toast.rs:188`, reclaims RAM after 280ms teardown), `get_last_toast` (`toast.rs:197`, returns `LAST_TOAST` clone for late-joining webviews). Frontend currently calls all 4 directly from `ToastApp.tsx:34,37,55,80` — flagged in `AUDIT_IPC.md:185` as `AGENTS.md:4.1#5` violation; recommended move into `services/windowService.ts` or new `services/toastService.ts`.
* **`should_show_error_toast` (`toast.rs:202`):** Guard that supplements `VoiceError` with a toast only when the main window (`pipeline::WINDOW_MAIN`) is hidden or destroyed — checked in every `on_error` path (`pipeline/dictation.rs:262`, `pipeline/modular/passive.rs:516`, etc.). Prevents duplicate banners when the user is already looking at the main HUD.

### 6.2 Dictation-Specific Toast Triggers

| Trigger | Code Path | Title | Message | `level` | Condition |
|---|---|---|---|---|---|
| Clipboard write succeeded | `output_router.rs:36` `dispatch_to_clipboard` | `Dictation Copied` | `text` (full transcript) | `Success` | `output_mode == Clipboard` |
| Paste injected (`Ctrl+V`/`Cmd+V`) | `output_router.rs:61` `dispatch_to_paste` `Ok(())` | `Dictation Pasted` | `text` | `Success` | `output_mode == Paste`, `with_clipboard_safe` + `simulate_paste` succeeded |
| Paste blocked by OS/compositor | `output_router.rs:76` `dispatch_to_paste` `Err` | `Paste Blocked by OS` | `Transcript saved to clipboard — paste manually with Ctrl+V.` | `Warning` | Wayland security block, macOS Accessibility denied, `enigo` failure — transcript **left** on clipboard |
| Engine/hotkey/STT failure | `pipeline/dictation.rs:263` `on_error` | `Voice Error` | `message` (typed `DictationError`) | `Error` | Also emits `voice_error { source:"Dictation", owner:"Dictation" }` to `WINDOW_TRAY`; toast only if `should_show_error_toast` is true |

Notes:
* `Tray` mode bypasses OS injection entirely (`output_router.rs:20` `DictationOutputMode::Tray => Ok(())`) — **no toast** is emitted; the Tray HUD itself is the presentation surface.
* Duration is caller-controlled via `ToastPayload.duration_ms`; all dictation paths pass `None` → frontend defaults to `DEFAULT_DURATION_MS = 3400ms` (`ToastApp.tsx:15`) with a linear `scaleX` progress bar.
* Clipboard safety and toast are coupled: on `Paste` success the 350ms restore window (`clipboard.rs:with_clipboard_safe`) completes **before** the success toast; on failure the clipboard is **not** restored so the warning toast's "saved to clipboard" claim is accurate.

### 6.3 Frontend Presentation — `ToastApp.tsx`

* **Dimensions:** `360px × 96px` (`TOAST_WIDTH/HEIGHT` `toast.rs:8`), positioned top-center with `24px` top inset (`TOAST_PAD_TOP`). Glassmorphism `glass-card` (`rounded-xl`, `border`, `blur(20px) saturate(180%)` via outer window shape), outer wrapper `w-screen h-screen flex items-start justify-center bg-transparent pointer-events-none p-6`.
* **Stack:** `React` + `framer-motion` `AnimatePresence`; entry `opacity 0→1, y -12→0` `duration 0.36 spring [0.16,1,0.3,1]`, exit `y -12`, inner fade `320ms` before `hide` + `280ms` before `destroy_toast_window_cmd`.
* **Levels (`ToastApp.tsx:8`):** `success → CheckCircle2 @ accent`, `warning → AlertTriangle @ warning`, `error → AlertCircle @ error`, `info → Info @ muted`; each with `bg`/`border` alpha variants and a 1× `bg-[rgba(border,0.06)]` progress track with `scaleX(progress)` fill at `opacity 0.45`.
* **Resilience:** `onShowToast` listen + 700ms `get_last_toast` poll covers cold-start race where the backend's immediate emit lands before `ToastApp` mounts; the backend's 420ms/300ms re-emits cover the inverse.

### 6.4 Relationship to Other Pipelines & Dev Poll

The same toast layer is reused outside dictation for modular pipeline errors (`pipeline/modular/passive.rs:517` `Voice Error`) and realtime failures. A dev-only poll in `lib.rs:350` emits a rotating test toast every 60s (titles `Dictation Copied` / `Dictation Pasted` / `Paste Blocked by OS` / `Voice Error` with `Success/Warning/Error` levels) to exercise the fallback chain; it does not affect the dictation contract.

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

## 9. Settings & IPC Interface

### 9.1 Backend Data Structures
```rust
pub struct DictationSettings {
    pub enabled: bool,                            // Master switch
    pub interaction_mode: DictationInteractionMode, // Passive | Ptt
    pub hotkey: String,                            // "Alt+Space"
    pub output_mode: DictationOutputMode,          // Paste | Clipboard | Tray
}
```

### 9.2 Frontend Settings Desk
In `InteractionCard.tsx`, users toggle between **Assistant** and **Dictation**:
- **Voice Typing Switch**: Toggle `dictation.enabled`.
- **Trigger Mode**: Switch between `Push-To-Talk` and `Continuous`.
- **Output Destination**: Segmented control for `Simulated Paste`, `Clipboard Only`, and `Floating Tray`.
- **Activation Hotkey**: Interactive key combination badge with inline edit support.

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


### 10.3 Global PTT Hotkey (`Alt+Space`)

- **Linux**: Registered via `tauri-plugin-global-shortcut`. Works on X11 and Wayland (portal-permitting).
- **macOS**: Requires Accessibility permission in System Settings → Privacy & Security → Accessibility.
- **Windows**: Standard Win32 `RegisterHotKey` — no special permissions needed.

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

> See [`performance-memory-optimizations.md`](./performance-memory-optimizations.md) for the
> full cross-platform heap trimming strategy (`trim_heap`) applied after model eviction.

---

**Last Updated:** 2026-09-03 — router fast-path uses `pipeline/handlers/` event-driven handlers; `DictationState` tray mapping clarified.
