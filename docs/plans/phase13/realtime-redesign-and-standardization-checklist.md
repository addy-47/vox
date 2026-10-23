# Realtime Redesign and Standardization Checklist

## Batch 1: Header Standardization & Dynamic Persona
- [ ] `app/src/shared/components/settings/interaction/InteractionCard.tsx`: Replace header segmented control with `CardTitleBar actionLabel="Dictation"`.
- [ ] `app/src/shared/components/settings/memory/MemoryCard.tsx`: Replace header segmented control with `CardTitleBar actionLabel="Advanced"`.
- [ ] `app/src/shared/components/settings/persona/PersonaCard.tsx`: Remove `INSTRUCTION_TABS` segmented control. Derive active prompt dynamically from `draftSettings.interaction.pipeline_mode`. Use standard `CardTitleBar`.

## Batch 2: Interaction Desk Preservation & Search Unification
- [ ] `app/src/shared/components/settings/interaction/InteractionCard.tsx`: Remove conditional swap to `RealtimeConfigDesk`. Keep `INTERACTION_DESK_TABS` permanently visible.
- [ ] `app/src/store/settingsStore.ts`: Wire `web_search_enabled` to propagate to realtime Gemini Live grounding parameters.

## Batch 3: Radial Hub Node Labels & Card Routing
- [ ] `app/src/data/settingsCopy.ts`: Add dynamic labels for realtime mode (`SETTINGS_DOMAINS_REALTIME`).
- [ ] `app/src/shared/components/settings/RadialHub.tsx`: Select node label dynamically based on active `pipeline_mode`.
- [ ] `app/src/pages/Settings.tsx`: Route slot `speech` to `VoiceCatalogView` and slot `language` to `RealtimeModelCard` when `isRealtime` is true.

## Batch 4: Provider Logos & Voice Filter Menu Icons
- [ ] `app/src/shared/components/settings/shared/ProviderLogo.tsx`: Update Gemini logo to official multi-color brand colors.
- [ ] `app/src/shared/components/settings/interaction/RealtimeConfigDesk.tsx`: Pass `providerId` and `providerName` to `ProviderLogo`.
- [ ] `app/src/shared/components/settings/speech/voices/VoiceCatalogView.tsx`: Pass `icon` to gender and accent `CardFilterItem` elements.
- [ ] `app/src/shared/components/settings/speech/voices/VoiceModal.tsx`: Pass `icon` to gender and accent `CardFilterItem` elements.

## Batch 5: Live Realtime Model & Voice Discovery (Turso Sync)
- [ ] `app/src-tauri/src/services/tts/voice.rs`: Implement `sync_deepgram_voices_to_db` and `sync_gemini_voices_to_db` caching to Turso `voices`.
- [ ] `app/src-tauri/src/ipc/voices.rs`: Expose sync commands for realtime voices.
- [ ] `app/src/services/catalogService.ts`: Add live fetchers for Deepgram think models (`/v1/agent/settings/think/models`) and Gemini Flash models.
- [ ] `app/src/shared/components/settings/realtime/RealtimeModelCard.tsx`: Implement provider cards, live model catalog, and parameter tuning.

## Batch 6: In-Place Clone Voice Card
- [ ] `app/src/shared/components/settings/speech/voices/CloneVoiceCard.tsx`: Build in-place skeleton card (`<AddCard variant="tile">`) and inline recording/upload panel.
- [ ] `app/src/shared/components/settings/speech/voices/VoiceCatalogView.tsx`: Render `CloneVoiceCard` when `scope.can_clone` is true.
- [ ] `app/src/shared/components/settings/speech/voices/VoiceModal.tsx`: Render `CloneVoiceCard` when `scope.can_clone` is true; remove `CloneVoiceDrawer` modal.

## Batch 7: Tooltip Consistency Remediation
- [ ] `app/src/shared/ui/Modal.tsx`: Wrap `onBack` and `onClose` buttons in `<Tooltip>`.
- [ ] `app/src/shared/components/settings/shared/CardHeader.tsx`: Wrap `onBack` in `<Tooltip>`.
- [ ] `app/src/shared/components/settings/speech/voices/VoiceCard.tsx`: Wrap preview and favorite buttons in `<Tooltip>`.
- [ ] `app/src/shared/components/settings/speech/voices/VoicePreviewBanner.tsx`: Wrap play, favorite, and edit prompt buttons in `<Tooltip>`.
- [ ] `app/src/shared/components/settings/language/ProviderGrid.tsx`: Wrap save and cancel key buttons in `<Tooltip>`.
