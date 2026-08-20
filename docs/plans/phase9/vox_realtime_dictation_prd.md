# Vox — Realtime Dictation PRD

**Status:** Proposed  
**Scope:** Desktop realtime dictation, decoupled from Tray UI  
**Primary goal:** Press a global hotkey, speak, and have the resulting text inserted into the currently focused application; fall back to clipboard when insertion is unavailable.

# **1\. Product Definition**

Realtime Dictation becomes a first-class Vox capability. The existing Tray is no longer the owner of dictation; it is one optional presentation mode for a shared dictation session.

Core model: Global Hotkey → Dictation Session → Vox STT → Transcript → Output Adapter

# **2\. User Experience**

* Vox runs in the background.  
* User places the cursor in any text field.  
* User presses and holds the configured global dictation hotkey.  
* Vox starts the same backend PTT path already used by the Tray.  
* User speaks naturally.  
* User releases the hotkey.  
* Vox finalizes transcription.  
* Vox attempts to paste the transcript into the active application.  
* If paste/insertion cannot be completed, Vox leaves the transcript on the clipboard and exposes a recovery path.

Target experience: app-agnostic dictation similar in interaction model to Wispr Flow. Flow currently uses background push-to-talk, then pastes the finished transcription into the active app; its documentation also confirms clipboard-based recovery when paste fails. citeturn0search2turn0search3

# **3\. Modes**

| Mode | Behavior | MVP |
| :---- | :---- | :---- |
| System Input | Insert transcript into the currently focused application. | Yes |
| Clipboard | Place transcript on clipboard without attempting insertion. | Yes |
| Tray | Render the active dictation session in the existing Tray HUD. | Yes |

Important: Tray is a presentation/output mode, not the dictation engine.

# **4\. Functional Requirements**

| ID | Requirement | Definition |
| :---- | :---- | :---- |
| FR-01 | Global hotkey | Register a configurable system-wide shortcut. Tauri's global-shortcut plugin supports Pressed/Released events on Windows, Linux and macOS. citeturn0search0turn0search8 |
| FR-02 | PTT lifecycle | Pressed starts dictation; Released ends dictation and requests final transcription. |
| FR-03 | Existing audio path | Reuse the existing Vox PTT/backend audio \+ STT lifecycle rather than creating a second transcription pipeline. |
| FR-04 | Target preservation | The intended destination is the application/text field that was active when dictation began. |
| FR-05 | System insertion | Attempt to deliver the final transcript to the target through the platform's supported paste/input mechanism. |
| FR-06 | Clipboard fallback | If there is no usable target or insertion fails, keep the final transcript on the clipboard. |
| FR-07 | Clipboard safety | For clipboard-based paste, preserve the user's prior clipboard contents and restore them after successful insertion where the platform permits. This mirrors Flow's documented behavior. citeturn0search3 |
| FR-08 | Recovery | Provide a way to paste/copy the last transcript after failed insertion. |
| FR-09 | Tray compatibility | If Tray mode is selected, the same dictation session may be visualized by Tray without changing backend behavior. |
| FR-10 | Cancellation | Canceling a session must not inject partial/unfinished text. |
| FR-11 | Hotkey conflict | If the configured global shortcut is unavailable, show a clear configuration error; Tauri does not trigger the handler when another application owns the shortcut. citeturn0search0 |

# **5\. Architecture**

The architectural separation should be explicit:

Global Shortcut → Dictation Controller → Audio/VAD/STT → Transcript → Output Router → System Input / Clipboard / Tray

The Dictation Controller owns session state. Output adapters own destination-specific behavior. No output adapter should own STT, VAD, or PTT.

# **6\. System Input Strategy**

MVP should use clipboard \+ simulated paste rather than attempting arbitrary character-by-character keyboard injection. The active application receives the normal paste action at its focused text target. This is also the mechanism documented by Wispr Flow for desktop dictation.

System Input → save clipboard → write transcript to clipboard → trigger native paste shortcut → verify/timeout → restore clipboard on success OR retain transcript on failure. Flow documents this same clipboard/paste/recovery model. citeturn0search3turn0search5

Do not promise universal insertion on every Linux environment in v1. Linux X11 and Wayland have different input-security models. Wayland exposes a virtual-keyboard protocol for emulating keyboard behavior, but compositor support and deployment conditions must be validated per environment. citeturn0search10

# **7\. Platform Requirements**

| Platform | MVP approach | Risk |
| :---- | :---- | :---- |
| Windows | Clipboard \+ Ctrl+V / platform-native paste path | Low–medium; elevated/admin apps can create input-boundary issues. |
| macOS | Clipboard \+ Cmd+V; Accessibility permission may be required for automation. | Medium; user permission required. |
| Linux X11 | Clipboard \+ Ctrl+V using X11-compatible input automation. | Medium; validate desktop environments. |
| Linux Wayland | Clipboard \+ compositor-supported input mechanism; treat as compatibility track until tested. | High; compositor/security differences. |

# **8\. Target Semantics**

The product should not attempt to locate a cursor visually. It relies on the OS notion of the currently focused application/input target. The user should place the cursor before pressing the hotkey.

If no text target is available, the session is still considered successful if transcription completes and the transcript is retained on the clipboard.

# **9\. Settings**

* Dictation enabled  
* Global hotkey  
* Activation mode: Hold-to-talk / future Hands-free  
* Output mode: System Input / Clipboard / Tray  
* Fallback: Clipboard (default and recommended)  
* Show Tray while dictating: On/Off  
* Last-transcript recovery shortcut

# **10\. Non-Goals for MVP**

* AI rewriting or polishing of dictated text.  
* Automatic punctuation/rewriting beyond what the current STT path already provides.  
* Application-specific integrations.  
* Semantic understanding of the target application.  
* Universal Wayland injection guarantees.  
* Building a new Tray UI.

# **11\. Failure & Recovery**

| Condition | Behavior | User outcome |
| :---- | :---- | :---- |
| No focused text field | Do not attempt blind insertion; keep transcript on clipboard. | User can paste manually. |
| Paste automation fails | Keep transcript on clipboard and surface recovery. | No data loss. |
| Target app blocks paste | Treat as insertion failure; retain clipboard transcript. | Manual paste available. |
| Hotkey conflict | Do not start dictation from that shortcut. | User selects another shortcut. |
| STT failure | Do not inject partial text. | Show failure and keep no misleading output. |
| User cancels | Discard unfinished transcript unless already finalized. | No accidental insertion. |

# **12\. Performance Requirements**

* Hotkey-to-recording start should reuse the existing PTT path with no new model initialization.  
* No Tray window should be created or shown when Tray output is disabled.  
* Dictation must not keep a second STT model resident.  
* Output routing must add negligible latency relative to STT finalization.  
* The feature must preserve Vox's 8 GB baseline and realtime-runtime constraints.

# **13\. Acceptance Criteria**

* With Vox in the background, the configured hotkey starts PTT without focusing Vox.  
* Speech is transcribed using the existing Vox STT backend.  
* With a focused text field in a supported application, the final transcript appears at the active insertion point.  
* With no usable text target, the transcript is available on the clipboard.  
* When paste fails, the original clipboard is not silently destroyed where restoration is supported.  
* Tray mode can display the same session without owning its lifecycle.  
* The same dictation engine works independently of Tray UI.  
* The feature behaves predictably across supported Windows, macOS, and tested Linux environments.

# **14\. Research Basis**

Wispr Flow's current desktop documentation confirms the core interaction: background app → hold hotkey → dictate → release → paste into active app. It also documents clipboard-based recovery and platform-specific paste limitations. citeturn0search2turn0search3turn0search4

Tauri provides the global shortcut primitive needed for the system-wide trigger. citeturn0search0turn0search8

Wayland's virtual keyboard protocol exists for emulated keyboard input, but Vox should treat Linux Wayland insertion as a compatibility problem to validate rather than assume universal support. citeturn0search10