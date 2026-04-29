# Meeting Reply Mode - Vox Assistant

## Goal

Enable Vox to act as a **silent meeting co-pilot** that allows the user to:
- Listen to and transcribe meetings (Zoom, Google Meet, Microsoft Teams, etc.) in the background
- Maintain live context/summaries of the conversation
- Generate intelligent replies using the LLM
- Speak **in the user's cloned voice** directly into the meeting when the user clicks "Send"
- Let the user participate in meetings **without ever speaking themselves**

This is an optional **advanced feature** (not part of MVP).

## Core Approach (Recommended)

We will use the **Virtual Audio Cable / Loopback** method instead of packet-level or process injection.

### Audio Flow

**Incoming Audio (Capture):**
- User sets meeting app's **Speaker Output** → Virtual Cable (e.g., "VB-Cable" or "Vox Virtual Sink")
- Vox STT continuously listens to this virtual cable
- Transcription → periodic summarization → fed into LLM context

**Outgoing Audio (Reply):**
- Vox generates reply using Gemma 4 E2B (or better model)
- Text → TTS (XTTS-v2 with user's cloned voice preferred)
- Audio played through **Virtual Microphone** (e.g., "VB-Cable Input")
- User selects this virtual mic as their microphone in the meeting app
- When user clicks **"Send to Meeting"** button in Vox, the audio is played into the meeting

## User Experience Principles

- **Minimal clicks** and friction is the highest priority
- One-time setup should be guided and as automatic as possible
- Default behavior should remain simple desktop mode
- Meeting Mode must be explicitly enabled by user
- Clear visual feedback at all times (e.g., "Listening to meeting...", "Generating reply...")

## Setup Flow (Target: ≤ 3 clicks after initial install)

1. **First Time Setup (Guided Wizard)**
   - User enables "Meeting Mode" in Settings
   - App detects OS and guides user to install virtual audio driver (VB-Cable on Windows, PipeWire module on Linux)
   - App provides direct download links + simple instructions
   - User is walked through setting:
     - Meeting Speaker Output → Vox Virtual Sink
     - Meeting Microphone → Vox Virtual Mic (optional)

2. **Daily Usage**
   - User joins meeting normally
   - Opens Vox (or it runs in background)
   - Toggles **"Meeting Mode"** ON with **one click**
   - Vox automatically starts transcribing + maintaining context
   - When user wants to reply → clicks **"Send to Meeting"** (big prominent button)

## Technical Implementation Notes

- **STT**: faster-whisper (tiny/base) listening to virtual audio source
- **LLM**: Gemma 4 E2B (IQ2_M/Q2_K default). Maintain rolling summary of meeting to keep context small
- **TTS**: 
  - Default: Piper (fast)
  - Preferred for meetings: XTTS-v2 using user's cloned voice
- **Context Management**: Periodically summarize last N minutes and refresh LLM context to prevent token bloat
- **Trigger**: User name mention detection (optional) + manual "Send" button (primary)

## Things to Take Care Of (UX Focus)

- **Simplicity**:
  - Never require manual audio routing every time. Save settings and auto-apply when possible.
  - Provide clear status indicators: "Meeting Mode Active", "Listening", "Generating", "Speaking into meeting"
  - Show live transcription preview

- **Reliability**:
  - Graceful fallback if virtual devices are not found
  - Warn user if meeting app audio routing is incorrect

- **Performance**:
  - Keep continuous transcription lightweight (use tiny model by default)
  - Limit LLM context size aggressively using summarization

- **Privacy & Consent**:
  - Clear warning on first use: "You are responsible for informing meeting participants if required by law/company policy"
  - Option to pause recording/transcription easily

- **Cross-Platform**:
  - Different virtual audio solutions needed for Windows vs Linux
  - Abstract audio device handling in code

## Future Enhancements (Phase 2+)

- Automatic name mention detection + suggested replies
- Fully automatic replies (with user-defined confidence threshold)
- Smart mute/unmute of real microphone
- Meeting summary at the end
- Support for more platforms (Discord, Webex, etc.)

## Risks & Limitations

- Requires one-time virtual audio setup (cannot be fully automatic due to OS restrictions)
- Slight latency (1.5–4 seconds) between someone speaking and Vox being ready to reply
- Audio quality of injected voice depends on virtual cable + XTTS performance
- Some meeting apps may have restrictions on virtual microphones

---

**Status**: Planned for v2  
**Priority**: Medium-High  
**Owner**: @adhbhut
