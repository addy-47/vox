# Unified Model Hub Checklist

## Batch 1: Speech Modals, Flow Banners & VAD Inline Refactor (COMPLETED)
- [x] `app/src/data/settingsCopy.ts`:
  - `SPEECH_CARD_COPY`: Added `sttModelsTitle`, `sttModelsSubtitle`, `sttModelsAria`, `expandStt`, `vadModelsTitle`, `vadModelsSubtitle`, `vadModelsAria`, `expandVad`, `vadDividerLabel`, `sttFlow`, `vadFlow`.
- [x] `app/src/shared/components/settings/speech/SpeechFlowBanner.tsx`:
  - Created multi-mode flow banner generalizing `EngineFlowBanner` with `"tts" | "stt" | "vad"` modes and domain-specific vector paths and animations.
- [x] `app/src/shared/components/settings/speech/EngineFlowBanner.tsx`:
  - Exported `SpeechFlowBanner` and backward-compatible `EngineFlowBanner` wrapper.
- [x] `app/src/shared/components/settings/speech/SttModelsModal.tsx`:
  - Created centered expanded modal with `CatalogModalShell`, `CatalogFacetRail` (sources: All, On-device, Cloud), STT model grid, and `SpeechFlowBanner(kind="stt")`.
- [x] `app/src/shared/components/settings/speech/VadModelsModal.tsx`:
  - Created centered expanded modal with `CatalogModalShell`, `CatalogFacetRail` (tiers: All, Built-in, ONNX), VAD model grid, inline divider, `VadSettingsPanel`, and `SpeechFlowBanner(kind="vad")`.
- [x] `app/src/shared/components/settings/speech/SttModelList.tsx`:
  - Wrapped inline grid in `ExpandableList` triggering `SttModelsModal`.
- [x] `app/src/shared/components/settings/speech/VadModelList.tsx`:
  - Wrapped inline grid in `ExpandableList` triggering `VadModelsModal`, with integrated detection knobs below divider.
- [x] `app/src/shared/components/settings/speech/SpeechCard.tsx`:
  - Removed corner button `vadSettingsOpen`.
  - Rendered VAD catalog view inline: `VadModelList` followed by divider and `VadSettingsPanel`.
  - Mounted expand buttons for STT and VAD.

---

## Batch 2: Homepage Quick Popover & Symmetrical Dock Trigger
- [ ] `app/src/layout/ResponsiveLayout.tsx`:
  - Add 44×44 round button with Gear icon at bottom-right (`monitorBtnRef` symmetrical counterpart).
  - Make `ModelStatusOverlay` interactive on hover/click.
  - Render `ModelQuickPopover` conditionally on trigger.
- [ ] `app/src/shared/components/settings/ModelQuickPopover.tsx`:
  - Create 386×458px Tier 1 floating popover anchored above bottom-right dock.
  - Implement LLM quick section (provider badge, model switcher, temperature slider).
  - Implement Voice quick section (engine badge, voice switcher, audio preview button, speech rate slider).
  - Implement STT/VAD status row.
  - Add Maximize button and "Manage All Models" footer button.

---

## Batch 3: Unified Model Hub Shell (`UnifiedModelModal`) & Overview Dashboard
- [ ] `app/src/shared/components/settings/UnifiedModelModal.tsx`:
  - Create centered modal dialog (`Modal.tsx`) with left facet rail.
  - Implement rail items: Overview, divider, STT, TTS, Voices, divider, Providers, LLM, Tuning.
  - Implement in-place `< Back to Overview` breadcrumb when navigated from Overview.
- [ ] `app/src/shared/components/settings/PipelineOverviewPanel.tsx`:
  - Create 3-column dashboard for Listening, Reasoning, and Speaking with quick selectors, sliders, and "Configure" buttons.
- [ ] Connect rail tabs to `SttModelList`, `TtsEngineGrid`, `VoiceCatalogView`, `ProviderGrid`, `RemoteCatalogSection`/`LocalModelsView`, `VadSettingsPanel`/`LlmTuningControls`.

---

## Batch 4: Settings Consolidation & Legacy Pruning
- [ ] `app/src/shared/components/settings/language/LanguageCard.tsx`:
  - Connect expand triggers to `UnifiedModelModal` with initial tab `"llm"` or `"providers"`.
- [ ] `app/src/shared/components/settings/speech/SpeechCard.tsx`:
  - Connect expand triggers to `UnifiedModelModal` with initial tab `"stt"`, `"tts"`, or `"voice"`.
- [ ] Prune legacy standalone modal wrappers if superseded.
- [ ] Run `pnpm lint:arch`, `pnpm test`, and `pnpm build`.
