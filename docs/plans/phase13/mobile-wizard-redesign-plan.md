# Mobile Wizard UI/UX Redesign Implementation Plan

## 1. Target End-State

A cohesive, centered, mobile-first onboarding experience for Vox on Android and compact viewports.
- **Header & Navigation:** Clean separation between step progress and the Skip action. Clicking Skip opens a non-destructive confirmation modal explaining what models are deferred and how to configure them later in Settings.
- **Model Downloads Step:** Clear purpose-driven presentation. Users select from explicit stack presets (**Minimal Voice Core** at ~834 MB, **Full Offline Assistant** at ~1.51 GB, or **Custom**) instead of navigating an unguided file tree. Clear labels distinguish local offline vs. free cloud vs. remote options.
- **Final Step:** Centered layout matching earlier wizard steps. Ambient glowing VoxOrb, celebratory typography, and centered status badges.

---

## 2. UI/UX Lead Recommendations & Concept Architecture

### 2.1 Problem Analysis
1. **Header Clutter & Destructive Skip:**
   - The current header packs `SKIP · 1/6` into a single button.
   - Touching this button triggers immediate setup completion without warning.
   - Users land in an unconfigured app without models or speech recognition.
2. **Model Download Confusion:**
   - The current step presents six raw model categories without clear purpose.
   - It does not explain coupling: STT requires VAD and Transliteration for accurate capture; Memory Embedding indexes speech transcripts.
   - It does not distinguish free cloud choices (e.g., Edge TTS: 0 MB download, 0 MB RAM) from heavy local models (e.g., Qwen 0.8B: 532 MB).
   - Layout is left-aligned and text-heavy, breaking the centered visual language of Steps 1–4.
3. **Final Ready Step Inconsistency:**
   - Step 6 switches to left-aligned cards and loses the VoxOrb visual anchor.
   - It lacks celebratory polish.

---

### 2.2 Model Architecture & Coupling Matrix

| Component | Manifest ID | Size | Purpose | Tier / Coupling | Cloud / Local |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Voice Activity Detection** | `silero_vad` (`earshot` built-in) | 644 KB | Detects speech start and end | **Core Stack (Required)**. Coupled with STT. | Local (ONNX) |
| **Speech-to-Text** | `nvidia_nemotron` | ~682 MB | Converts voice to text | **Core Stack (Required)**. Essential for dictation. | Local (ONNX) |
| **Phonetic Transliteration** | `vox_translit_rnn` | ~16 MB | Normalizes spoken Hindi/English | **Core Stack (Required)**. Cleans transcript phonetics. | Local (ONNX) |
| **Semantic Memory** | `minilm_l12` | ~135 MB | Generates memory vector embeddings | **Core Stack (Required)**. Couples conversation to memory. | Local (ONNX) |
| **Reasoning Engine (LLM)** | `qwen_3_5_0_8b` | ~532 MB | Generates conversational replies | **Optional**. Not needed if using Cloud or Remote server. | Local (GGUF) |
| **Speech Synthesis (TTS)** | `edge_tts` | 0 B | Reads responses aloud | **Optional**. Free neural cloud voices, 0 MB storage. | Cloud (Free) |
| **Speech Synthesis (TTS)** | `supertonic` / `kokoro` | ~144 MB – 365 MB | Offline speech synthesis | **Optional**. Full air-gapped voice replies. | Local (ONNX) |

---

### 2.3 Stack Presets for Mobile Users

To eliminate choice fatigue, the redesigned Step 5 presents 3 clear presets:
1. **Minimal Voice Core (~834 MB) [Recommended Default for Low Storage]:**
   - Includes: VAD + Nemotron STT + Translit + MiniLM Memory.
   - Purpose: Enables complete offline voice dictation, transcription, and memory recall. Conversational reasoning uses free cloud or remote server.
2. **Full Offline Assistant (~1.51 GB) [Recommended for Airplane / Air-Gapped Mode]:**
   - Includes: Minimal Voice Core + Qwen 0.8B Local LLM + Supertonic TTS.
   - Purpose: 100% private and air-gapped speech-in, speech-out with zero cloud dependencies.
3. **Custom Stack:**
   - Expandable inspector for power users to toggle individual models and see exact RAM and storage costs.

---

### 2.4 Centered Layout & Visual Hierarchy

- **Header:**
  - Left: Vox logo + wordmark.
  - Center/Right: Clean `Step X of 6` indicator.
  - Far Right: Ghost `Skip` button with touch padding (min 48px).
- **Skip Modal:**
  - Glassmorphism alert dialog with centered copy.
  - Informs the user that models can be installed anytime via **Settings → Models & Audio**.
  - Buttons: "Continue Setup" (primary accent) vs "Skip to Home" (ghost).
- **Model Step:**
  - Centered title, description, and Wi-Fi badge.
  - Centered Stack Preset toggle pills (**Minimal Core** vs **Full Offline** vs **Custom**).
  - Centered stack summary card with storage total, RAM footprint, and purpose description.
  - Expandable custom inspector with clean category badges (`Core`, `Free Cloud`, `Offline AI`).
- **Ready Step:**
  - Centered glowing VoxOrb in "Listening" or "Speaking" state with ambient glow.
  - Centered headline: "You're All Set".
  - Centered status capsule: `Offline Engine Active · Private & Encrypted`.
  - Centered prompt: "Tap the center orb anytime to start speaking."

---

## 3. Atomic Batches & Blast Radius

### Batch 1: Copy & Type Definitions (`app/src/data/welcomeCopy.ts`)
- **Changes:**
  - Add `MOBILE_ONBOARDING_COPY.skipModal` with title, description, settings hint, cancel, and confirm actions.
  - Add `MOBILE_ONBOARDING_COPY.modelPresets` detailing Minimal Core, Full Offline, and Custom presets.
  - Update `MOBILE_ONBOARDING_COPY.ready` with centered status chips and copy.
- **Blast Radius:** `welcomeCopy.ts` only.
- **Build state:** Green throughout.

### Batch 2: Skip Confirmation Modal & Header Refactor (`MobileOnboardingFlow.tsx`, `MobileSkipConfirmModal.tsx`)
- **Changes:**
  - Create `app/src/wizard/mobile/MobileSkipConfirmModal.tsx`.
  - Update `MobileOnboardingFlow.tsx` header: decouple step counter from Skip button.
  - Wire Skip button to open `MobileSkipConfirmModal`.
- **Blast Radius:** Mobile wizard root.
- **Build state:** Green throughout.

### Batch 3: Centered Model Download Step (`MobileModelStep.tsx`)
- **Changes:**
  - Add preset state (`'minimal' | 'full' | 'custom'`).
  - Preselect model IDs based on active preset.
  - Provide centered preset selector cards explaining coupling, purpose, and cloud/local status.
  - Keep truthful download progress bar and error retry mechanism from defect fixes.
  - Ensure all container copy and cards are center-aligned.
- **Blast Radius:** `MobileModelStep.tsx`.
- **Build state:** Green throughout.

### Batch 4: Centered Ready Step (`MobileReadyStep.tsx`)
- **Changes:**
  - Embed centered `VoxOrb` with ambient radial glow.
  - Align all copy, status chips, and instructions to the center.
  - Redesign capability cards into a clean, centered horizontal badge group.
- **Blast Radius:** `MobileReadyStep.tsx`.
- **Build state:** Green throughout.

### Batch 5: Verification & Compile Gate
- **Execution:**
  - Run `pnpm tsc --noEmit` and `pnpm test`.
  - Run `./scripts/verify-targets.sh` (desktop + android cross-compile).
  - Test responsive layout and touch interactions.
