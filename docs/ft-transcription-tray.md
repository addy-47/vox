📄 Transcription Tray — Product Feature Definition
1. Problem

Modern workflows involve working across multiple applications simultaneously:

IDEs (VSCode, terminals, CLI tools like Gemini CLI)
Browsers
AI tools with inconsistent input methods

Only a few applications provide native voice input (STT). Most require:

Speak → open another app → transcribe → copy → paste

This creates:

friction
context switching
interruption in flow
loss of speed advantage of voice input
2. Core Insight

Voice input should be system-level, not app-level.

The user should be able to:

speak anywhere
get text instantly
use it in any application

Without:

switching context
opening another tool
breaking flow
3. Solution
Vox Transcription Tray

A system-level, real-time transcription overlay that works across all applications.

4. How It Works (User Perspective)
Background Behavior
Vox runs a lightweight always-on VAD + STT service
It passively listens for speech
No manual trigger required (unless disabled)
On Speech Detection
User speaks
→ speech detected
→ small overlay appears from right edge
During Speech
Real-time transcription streams into the tray
Text updates continuously (no waiting)
UI remains minimal and non-intrusive
On Silence (End of Turn)
Silence detected
→ transcription finalizes
→ tray fades out and disappears
Next Interaction
New speech → new tray instance
No history carried in tray
5. UX Principles
⚡ Ephemeral by Design
Tray is temporary
Exists only during active speech
Never persists or accumulates history

This aligns with Vox’s core interaction model:

⚡ Zero Friction

User should not:

click anything
switch apps
trigger manually (default mode)
⚡ Non-Intrusive
Does not steal focus
Does not block interaction
Appears softly, disappears cleanly
⚡ Instant Feedback
Partial transcription must appear immediately
No waiting for full sentence completion
6. UI Behavior Requirements
Visibility
Appears on speech_start
Updates during speech
Fades on speech_end
Fully hides after timeout
Readability
High contrast text
Clean typography
No visual clutter

Design must follow system style:

Copy Interaction
One-click copy to clipboard
No text selection required
Copy must be instant
Long Transcriptions
Internal scrolling enabled
Tray size remains fixed
Manual Controls

Minimal controls only:

copy button
close button (optional)
drag/move (optional, later)
7. Lifecycle (System-Level)
speech_start
    → show tray
    → stream transcript

speech_active
    → update text

speech_end
    → finalize text
    → fade UI
    → destroy / hide
8. Key Constraints
Must NOT
persist transcript in tray
block user interaction
require manual activation
behave like a sidebar or panel
Must Ensure
<200ms visible feedback
smooth animation
minimal CPU usage
9. Relationship to Core System

This feature is a direct surface of the backend pipeline:

audio → VAD → STT → transcript events → tray UI

It relies on the real-time architecture defined in:

10. Final Definition

The Transcription Tray is a system-level, ephemeral voice input layer
that allows users to speak anywhere and instantly obtain usable text — without breaking workflow.