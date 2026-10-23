# Realtime Architecture Redesign, Voice Cloning & Settings Standardization Plan

## 1. Target End-State

1. **Standardized Card Headers**: All settings cards (`Speech`, `Language`, `Interaction`, `Memory`, `Persona`) use the unified `CardTitleBar` pattern with a corner text breadcrumb (`Dictation >`, `Advanced >`, `Select model >`, `Select voice >`).
2. **Dynamic Persona Card**: The persona card dynamically shows and edits either `modular_prompt` or `realtime_prompt` based on `draftSettings.interaction.pipeline_mode`. The manual mode switcher is removed.
3. **Preserved Interaction Desk**: The bottom desk of `InteractionCard` (`Remote Control`, `Web Search`, `Transliteration`) remains permanently rendered in both modular and real-time modes.
4. **Unified Web Search**: The `interaction.web_search_enabled` toggle controls web search for both modular assistant and Gemini Live Google Search grounding.
5. **Separation of Realtime Models and Voices**:
   - **Language Model Node (04:00)**: Dynamically labeled "Realtime Models" when `pipeline_mode === "realtime"`. Displays `RealtimeModelCard` with authentic provider cards (Gemini Live, Deepgram Agent), live model catalog discovery, API keys, and parameter tuning.
   - **Speech Models Node (08:00)**: Dynamically labeled "Voices" when `pipeline_mode === "realtime"`. Opening the card displays the Voice Catalog view directly.
6. **Live Realtime Voice Fetching & Turso Caching (Zero Hardcoding)**:
   - Deepgram and Gemini voices are fetched dynamically via live provider endpoints (`GET https://api.deepgram.com/v1/models` for Deepgram TTS; Gemini voice catalog) and synced to the local Turso `voices` table via background sync (identical to `sync_edge_tts_voices_to_db`).
7. **Authentic Multi-Color Gemini Logo**:
   - Replace the monochrome/blue gradient in `ProviderLogo.tsx` with the official Google Gemini 4-color spark/gradient (`#4285F4`, `#9B60ED`, `#EA4335`, `#FBBC05`).
8. **In-Place Voice Cloning Card (Drawer Redesign)**:
   - Remove the nested `<Modal>` in `CloneVoiceDrawer.tsx`.
   - Implement an in-place `CloneVoiceCard` in the voices grid using `<AddCard variant="tile" ...>` (mirroring `CustomServerFormCard` in `ProviderGrid`).
   - Appears strictly when `scope.can_clone` is true from the backend `get_voice_scope` contract, with zero ad-hoc frontend string sniffing.
9. **Tooltip System Standardization**:
   - Wrap `Modal.tsx` close and back buttons in `<Tooltip>`, fixing close buttons across `TtsModelsModal`, `ProviderModal`, `ModelModal`, and `VoiceModal`.
   - Add `<Tooltip>` wrappers to icon-only action buttons and remove deprecated native `title` attributes.

---

## 2. Live API Probe & Investigation Findings

### 2.1 Deepgram Agent Think Models (Live Verified)
- **Endpoint**: `GET https://agent.deepgram.com/v1/agent/settings/think/models`
- **Header**: `Authorization: Token {apiKey}`
- **Live Response (33 models verified live)**:
  - **OpenAI**: `gpt-5.2`, `gpt-5.4`, `gpt-4.1`, `gpt-4.1-mini`, `gpt-4.1-nano`, `gpt-4o`, `gpt-4o-mini`, `gpt-5`, `gpt-5-mini`, `gpt-5-nano`, `gpt-5.1` (Thinking), `gpt-5.4-mini`, `gpt-5.4-nano`, `gpt-5.5`, `gpt-5.6-luna`, `gpt-5.6-terra`, `gpt-6-luna`.
  - **Anthropic**: `claude-haiku-4-5`, `claude-haiku-5-5`, `claude-sonnet-4-5`, `claude-sonnet-4-6`, `claude-sonnet-5`, `claude-sonnet-5-5`.
  - **Google**: `gemini-2.5-flash`, `gemini-2.5-flash-lite`, `gemini-3-flash-preview`, `gemini-3.1-flash-lite`, `gemini-3.1-pro-preview`, `gemini-3.5-flash`, `gemini-3.8-flash`.
  - **Groq**: `openai/gpt-oss-20b`.
  - **Nvidia**: `nvidia/nemotron-3.5-lightning-30b-a3b`.

### 2.2 Deepgram Voices (Live Verified)
- **Endpoint**: `GET https://api.deepgram.com/v1/models`
- **Header**: `Authorization: Token {apiKey}`
- **Live Response**: Returns 48+ TTS voices under `tts[]` with:
  - Canonical names: `aura-2-agathe-fr`, `aura-2-asteria-en`, `aura-2-apollo-en`, etc.
  - Metadata: `display_name`, `accent`, `languages`, `tags` (`masculine`, `feminine`, etc.), and `.wav` audio sample URLs (`metadata.sample`).

### 2.3 Gemini Live Models (Live Verified)
- **Endpoint**: `GET https://generativelanguage.googleapis.com/v1beta/models?key={apiKey}`
- **Findings**: The REST `v1beta/models` endpoint returns all Gemini models. `supportedGenerationMethods` does not return `bidiGenerateContent` on REST. The live Flash and real-time models are identified by pattern matching (`gemini-.*flash`, `gemini-.*live`, `gemini-omni-.*`), including:
  - `gemini-2.5-flash`, `gemini-2.5-flash-lite`, `gemini-3-flash-preview`, `gemini-3.1-flash-lite`, `gemini-3.5-flash`, `gemini-3.8-flash`, and `gemini-3.1-flash-live-preview`.

### 2.4 Voice Cloning Contract & Pattern
- **Backend Contract**: `get_voice_scope` returns `VoiceScopeDto { voice_provider, can_clone: caps.clone, cloud_sync }`.
- **Frontend Current State**: `CloneVoiceDrawer.tsx` uses a modal. No button exists in the inline `VoiceCatalogView`.
- **Target Pattern**: Use `<AddCard variant="tile" ...>` in the voices grid that flips in-place to an inline record/upload card when clicked. Zero modal, zero string sniffing.

### 2.5 Tooltip Gap Audit (Subagent Report)
- `Modal.tsx:204` lacks `<Tooltip>` on the close button, affecting every settings modal.
- `CardHeader.tsx` back button lacks `<Tooltip>`.
- Multiple icon buttons (`VoiceCard` preview/favorite, `VoicePreviewBanner` prompt edit, `ProviderGrid` save/cancel key) lack `<Tooltip>`.

---

## 3. Implementation Batches

### Batch 1: Header Standardization & Dynamic Persona
- **Files**:
  - `app/src/shared/components/settings/interaction/InteractionCard.tsx`
  - `app/src/shared/components/settings/memory/MemoryCard.tsx`
  - `app/src/shared/components/settings/persona/PersonaCard.tsx`
- **Actions**:
  - Replace segmented control in `InteractionCard` with `CardTitleBar actionLabel="Dictation"`.
  - Replace segmented control in `MemoryCard` with `CardTitleBar actionLabel="Advanced"`.
  - In `PersonaCard`, remove segmented control; derive active prompt dynamically from `draftSettings.interaction.pipeline_mode`.
- **Verification**: `pnpm lint` and `pnpm build` clean; smooth header transitions.

### Batch 2: Interaction Desk Preservation & Unified Web Search
- **Files**:
  - `app/src/shared/components/settings/interaction/InteractionCard.tsx`
  - `app/src/store/settingsStore.ts`
- **Actions**:
  - Remove conditional swap to `RealtimeConfigDesk`; keep `remote`, `web_search`, and `transliteration` visible across both pipeline modes.
  - Wire `web_search_enabled` to propagate to realtime Gemini Live Google Search grounding.
- **Verification**: Changing pipeline mode leaves the bottom desk intact.

### Batch 3: Radial Hub Node Dynamic Labels & Routing
- **Files**:
  - `app/src/data/settingsCopy.ts`
  - `app/src/shared/components/settings/RadialHub.tsx`
  - `app/src/pages/Settings.tsx`
- **Actions**:
  - Dynamically rename 04:00 to "Realtime Models" and 08:00 to "Voices" when `pipeline_mode === "realtime"`.
  - Route slot 04:00 to `RealtimeModelCard` and slot 08:00 to `VoiceCatalogView`.
- **Verification**: Toggling pipeline mode updates node labels and routed cards dynamically.

### Batch 4: Provider Logos (Authentic Multi-Color Gemini) & Voice Filter Icons
- **Files**:
  - `app/src/shared/components/settings/shared/ProviderLogo.tsx`
  - `app/src/shared/components/settings/speech/voices/VoiceCatalogView.tsx`
  - `app/src/shared/components/settings/speech/voices/VoiceModal.tsx`
- **Actions**:
  - Replace Gemini logo gradient with Google Gemini multi-color palette (`#4285F4`, `#9B60ED`, `#EA4335`, `#FBBC05`).
  - Pass `providerId` and `providerName` to `ProviderLogo` in all realtime provider listings.
  - Pass leading icons (`User`, `CircleDot`, `Globe`, `Languages`) to gender and accent items in `VoiceCatalogView` and `VoiceModal`.
- **Verification**: Gemini logo displays rich multi-color brand colors. Voice filter dropdown shows clean leading icons across all sections.

### Batch 5: Realtime Live Model & Voice Discovery (Turso Sync)
- **Files**:
  - `app/src-tauri/src/services/tts/voice.rs`
  - `app/src-tauri/src/ipc/voices.rs`
  - `app/src/services/catalogService.ts`
  - `app/src/shared/components/settings/realtime/RealtimeModelCard.tsx`
- **Actions**:
  - Backend: Implement `sync_deepgram_voices_to_db` (calling `GET https://api.deepgram.com/v1/models` and parsing `tts[]`) and `sync_gemini_voices_to_db` to cache voices into Turso `voices` table.
  - Frontend: Query `https://agent.deepgram.com/v1/agent/settings/think/models` for Deepgram think models, and pattern-filter Gemini Flash/Live models.
  - Render `RealtimeModelCard` with provider selection, dynamic model picker, and parameter controls.
- **Verification**: Deepgram and Gemini voices populate in Turso DB with preview WAVs, tags, and genders. Live model list displays actual models.

### Batch 6: In-Place Clone Voice Card (Skeleton Pattern)
- **Files**:
  - `app/src/shared/components/settings/speech/voices/CloneVoiceCard.tsx` (new)
  - `app/src/shared/components/settings/speech/voices/VoiceCatalogView.tsx`
  - `app/src/shared/components/settings/speech/voices/VoiceModal.tsx`
  - Remove or deprecate modal from `CloneVoiceDrawer.tsx`.
- **Actions**:
  - Build `CloneVoiceCard` using `<AddCard variant="tile">` that swaps in-place to an inline record/upload card on click.
  - Render it as the first tile in the voices grid only when `scope.can_clone` is true.
  - Zero ad-hoc string checks.
- **Verification**: Chatterbox shows the clone skeleton card; Kokoro/Edge do not. Clicking flips the card in-place with no modal popup.

### Batch 7: Tooltip Consistency Remediation
- **Files**:
  - `app/src/shared/ui/Modal.tsx`
  - `app/src/shared/components/settings/shared/CardHeader.tsx`
  - `app/src/shared/components/settings/speech/voices/VoiceCard.tsx`
  - `app/src/shared/components/settings/speech/voices/VoicePreviewBanner.tsx`
  - `app/src/shared/components/settings/language/ProviderGrid.tsx`
- **Actions**:
  - Wrap `Modal.tsx` back and close buttons in `<Tooltip>`.
  - Add `<Tooltip>` to action buttons identified in the audit.
  - Eliminate native `title` attributes that trigger native OS tooltips.
- **Verification**: Hovering over modal close buttons, back buttons, and favorite hearts shows unified Vox Liquid Space tooltips.
