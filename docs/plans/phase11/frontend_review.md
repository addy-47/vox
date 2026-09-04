# Frontend Review — Full Sweep (2026-09-03, expanded)

Reviewing this as **production** scale — long-running Tauri desktop, 8GB RAM CPU-first, sub-200ms voice pipeline. Critique is calibrated accordingly.

Method: `/review` (logic primary: every finding has What / Why at scale / Replacement / severity) + `/create-sprints` (deterministic scope, persistent checklist, 3 subagent sprints for the voice path) **plus a full second pass read directly in this thread** across all domains — shell, stores, contexts, hooks, services, pages, settings workspaces, history/memory/monitoring/profiler, home/tray, wizard, layout, shared UI. No code changed in this pass.

Coverage, this thread (read in full or in logic-relevant part): `App/main`, `layout/*` (3), `store/*` (2), `contexts` (Settings, MemoryProfiler; VoiceSession prior), `hooks` (13/13: useHomePage, useInteraction, useOverlay, useVisibility, useStreamingRenderer, useDynamicFPS, useTelemetry, useMonitoringMetrics, useMemoryProfiler, useMemoryTrace, useVoxFootprint, useSettings, useSettingsPage), `services/*` (11/11), `pages/*` (5/5), settings workspaces (ModelsCard full, InteractionCard, LlmConfigDesk full, DictationConfigDesk, PipelineModeCard, TriggerModeCard, CategorySelector, RealtimeConfigDesk, HistoryCard, VadWorkspace, TtsVoiceManager pt.1, LlmSettingsView pt.1, MemoryCard, ModelStatusOverlay, RestoreDefaultsButton), `wizard/*` (Root, machine, all 6 steps), `tray/*` (4/4), home components (AdvancedOrb head+disposal, PipelineField, StatusCapsule, Bell, Popover, TestClips, ActiveTranscript prior), `useHistory` full, `MemoryPipelineDrawer` head, `overlayStack`, `SegmentedControl`, `ErrorBoundary`, service tests (assert lines), backend `settings.rs`/`memory.rs:290-334`/`mutation.rs:810-825` for contract verification. Pattern-swept (grep, not full-read): remaining presentational components (history orbit viz, monitoring chambers, profiler tabs, persona/appearance/realtime tails, catalog views, carousel tails, ui primitives, data copy, lib). Checklist 26/26 + this sweep.

Part I = prior voice-path findings (kept,聲音). Part II = new full-frontend findings below.

## PART I — Voice path (kept from prior pass, verified again)

🔴 llm_token overwrite (`VoiceSessionContext:423`); test-clip Ready strand (`:257-272`); engage double-tap + mpsc-vs-router optimism (`:147-164`); voice_error never sets Error (`:388-393`); 8 arg-case rejections + toggle semantics (§5 prior); 4 missing notification events + record drift; wizard raw listen; tray hide-wipe + PTT missing await. 🟠 pause optimism, turn_id bleed, Idle discard, snapshot clobber, Space brick, probe/caps masking, compaction category, SetupStep case, stale test mock. Details unchanged — see git history of this file.

## PART II — Full-frontend findings (new)

### 🔴 Will Break

**F1. Conditional-hook violations across 8 settings components.** `InteractionCard:40`, `LlmConfigDesk:36`, `RealtimeConfigDesk:17`, `TtsVoiceManager:81`, `AppearanceCard:48`, `LlmSettingsView:48`, `MemoryCard:17`, `ModelStatusOverlay:75` (and `RealtimeCard:724` inner scope) all `return null` when settings/catalog are null **after** already calling hooks (`useSettingsStore` selectors, `useState`, `useCallback`) and **before** more hooks (`useCallback`/`useEffect`/`useMemo`). Render 1 with null settings calls N hooks; render 2 calls N+M → React "rendered more hooks" crash. Latent only because `SettingsProvider` loads settings at boot and cards lazy-mount later — but any cold path that mounts a card before load (fresh boot straight to Settings, future code nulling settings) crashes inside an ErrorBoundary with no recovery except manual Retry. Safe siblings prove the fix is trivial: `HistoryCard:16`, `VadWorkspace:34`, `AsrWorkspace:37`, `TtsModelWorkspace:35` return before *any* non-selector hook (selectors are still hooks — strictly they share the shape, but with zero hooks after the return the count is stable).
*Replacement:* move the null-guard to the very first line (before all hooks) and pass possibly-undefined values into hooks, or split into outer guard component + inner hooked component:
```tsx
export const MemoryCard = memo(({layoutMode}) => {
  const memory = useSettingsStore((s) => s.draftSettings?.memory);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const handleToggleRetrieval = useCallback(() => {
    if (!memory) return;
    updateDraft("memory", "context_retrieval_enabled", !memory.context_retrieval_enabled);
  }, [memory, updateDraft]);
  if (!memory) return null;
  ...
```

**F2. Memory-ingestion pause diverges: settings toggle doesn't touch runtime.** `MemoryCard` toggle → `updateDraft("memory","pipeline_processing_enabled")` → `update_setting` → `mutation.rs:821-825` persists the bool and stops. Nothing flips the runtime `user_paused_ingestion` atomic — only the `toggle_pipeline_processing` IPC does (`memory.rs:299-309`, which also persists). Atomic boots `false` (`state.rs:262`) and is never synced from the persisted flag. Consequences: pause-via-Settings leaves ingestion running while UI says paused; a persisted pause resumes running after restart. The drawer path (`MemoryPipelineDrawer:135-159`) already compensates by calling IPC *then* `updateDraft`+`commitChanges` — only the card path is incoherent.
*Replacement (pick one):* (a) backend: sync the atomic inside `apply_memory_mutation` post-commit (needs access to AppState — restructure like the dictation side-effect handlers), or (b) frontend: `MemoryCard.handleTogglePipeline` calls `togglePipelineProcessing(next)` IPC first, then drafts the returned state like the drawer does. (b) is the smaller change and matches the existing drawer pattern.

**F3. STT cloud is selectable with no configuration UI.** `InteractionCard:168-175` lets the STT pill switch to `cloud` (`updateDraft("stt","active","cloud")`), but `AsrWorkspace` has zero cloud references (no provider/model/language/key/endpoint inputs — verified by grep) and `LlmConfigDesk`-style remote desks exist only for LLM/TTS. Backend `stt.cloud` fields exist. User can enter an unconfigured cloud-STT state with no UI path to complete it.
*Replacement:* add an STT-cloud desk mirroring the LLM-cloud pattern (provider carousel + key field), or remove `"cloud"` from the STT pill until it exists.

**F4. Realtime OpenAI/ElevenLabs toggles write keys the backend doesn't have.** `RealtimeCard.UnifiedConfig:598-619` renders a toggle bound to `voice_activity_detection` (OpenAI) / `dynamic_vars` (ElevenLabs). Backend `OpenAiRealtimeConfig{api_key,model,voice}` and `ElevenLabsConvaiConfig{api_key,agent_id}` (`settings.rs:828-861`) have neither field; serde drops unknowns on save. Toggle flips in draft, vanishes after commit+reload — UI lies, and the write is indistinguishable from success.
*Replacement:* gate those toggles on fields present in the backend structs (Gemini `enable_web_search`, Deepgram `agent_mode` are real), or extend the backend structs + provider drivers first.

**F5. Tests enshrine the arg-case bugs — fixing keys goes red.** `pipelineService.test:83` asserts `("test_clip", {clipId})`, `historyService.test:66` asserts `("get_turns", {sessionId: 1})`, `settingsService.test:110,119` assert `{modelId, targetCap}`. All three assert the exact mismatched keys the backend rejects/ignores. The fix must update these assertions in the same commit or CI blocks the repair.
*Replacement:* change assertions to `{clip_id}`, `{session_id}`, `{model_id, target_cap}` alongside the service fix.

**F6. Onboarding dead-end on mic-less machines.** `SystemCheckStep:36-40,114-121` gates Continue on `write_access && disk_space_ok && mic_access` with no skip path (`WizardFooter` `showSkip` unset here), and the machine's `FAILURE`/`error` state has no render branch (`WizardRoot:51-95` — `state.matches('error')` falls to `"Unknown State"`). A VM/headless box without a mic can never leave step 2; a report-fetch failure leaves `allOk` falsy with only a generic banner.
*Replacement:* allow skip-with-warning on mic failure (dictation/assistant need mic, but memory/settings don't), and render the machine `error` state explicitly.

**F7. `navigate('/settings?tab=models')` is unread by anyone.** `TitleBar:211` deep-links the Models tab from the update pill; neither `Settings.tsx` nor `useSettingsPage` reads any query param (verified by grep). Click lands on the hub with no card open.
*Replacement:* parse `?tab=` in `useSettingsPage` (or drop the query and open Models by default from this entry).

**F8. Wizard machine progress state is write-only.** Every `model_progress` event sends `PROGRESS` into `setupMachine:87-115`, maintaining `models`/`totalProgress` — zero consumers (verified by grep). Each event re-renders `WizardRoot` for nothing during the heaviest IPC storm of setup.
*Replacement:* delete the `PROGRESS` handler + `models`/`totalProgress` from context (the step keeps its own local progress map), or drive the progress view from it.

### 🟠 Real Cost

**G1. Engine restart per mic click.** `AudioSetupStep:74-84` `stopEngine()` + `launchEngine()` on every device select, no debounce/guard. Rapid clicks stack restarts (model unload/reload) on 8GB CPU-first; a mid-restart Next carries a half-booted engine into LiveTest.
*Replacement:* debounce + `isSwitching` guard, or defer restart to Next (store pending device, apply once).

**G2. ErrorBoundary never resets on navigation.** `ErrorBoundary` has no `key` reset / `componentDidUpdate` path check — a crashed route stays crashed after navigating away and back (same element type, state persists). Only manual Retry/Home recovers.
*Replacement:* `<ErrorBoundary key={location.pathname} name=...>` at route level, or reset on `prevProps.children !== this.props.children`.

**G3. ModelStatusOverlay fail-open + fuzzy match.** `presence[id] ?? true` (`:83-84`) shows healthy until the check completes (inverted for a status indicator); `m.id.includes(ttsKind)` (`:80`) substring-matches group ids; TTS chip has no missing branch at all (`:157-174`, vs LLM/ASR). A deleted TTS group renders healthy.
*Replacement:* default `?? false` with a neutral "checking" skeleton while pending; exact id match with fallback display; add the missing TTS branch.

**G4. Health check fires per keystroke.** `LlmConfigDesk:159-165` deps include the `draftSettings.llm.server` object identity, recreated on every keystroke in URL/key fields; each keystroke re-arms the 500 ms check → "Connection failed"/Offline flicker while typing a valid URL.
*Replacement:* depend on debounced primitives (`server?.base_url`, `server?.api_key`) instead of the object.

**G5. Notification deep-link only works from cold History.** `useHistory:119-129` consumes `?sessionId=` only when `sessions` loads. Clicking a second notification while already on `/history` changes the URL but reselects nothing (`History.tsx` has no location dep).
*Replacement:* add `useLocation().search` dep and select on change.

**G6. Window CustomEvent bridge between cards.** `ModelsCard:206-228` ↔ `InteractionCard:127-156` sync tabs via `sync_pipeline_tab`/`sync_interaction_category` with ping-pong guards. Works, but it's invisible coupling: no types, no registry, DevTools-untraceable, and both sides discard unsaved drill-down state on sync (`discardCategoryChanges`).
*Replacement:* lift `activePipelineTab` into `settingsStore` or a tiny shared hook; keep the events (if at all) as a compat shim.

**G7. Fire-and-forget restores/copies.** `RestoreDefaultsButton:18` doesn't await `restoreDefaults()` (no error feedback on failure); `TitleBar:43-47` clipboard write unawaited/uncaught; `ToastApp:59` stray `console.log`; `useInteraction:30` stray log on every session init.
*Replacement:* await + catch with user-visible error; delete the two logs.

### 🟡 Stylistic / optional

Dead prop threading `edgeTtsError/loadingEdgeVoices/loadEdgeVoices` declared on `TtsVoiceManager:36-38`, never destructured/used (edge load failures silent in the manager); `Footer` re-declares `SystemStats` locally instead of importing the service type; `TranscriptRenderer` ref type is a subset of the real telemetry shape (fine, note only); `VoiceCarousel` side-effect (`handleStopRecording`) inside a `setState` updater (double-invokes in StrictMode dev); `interactionMode={String(...).toUpperCase()}` (`TrayApp:316`) vs `Header` comparing `.toUpperCase()` again (`Header:40`) — double normalization; `useHistory` returns `handleGoToday`/`handleBackToMonth` that `History.tsx` never destructures; `Settings.tsx:144`/`299-312` duplicates the `requiresRestart` + missing-key blocks already in `SettingsCardWrapper:146-174` (drift risk — the store already lists `threads` keys in `updateDraft:491-502`; three sources of restart truth); `SettingsCardSkeleton`/`SettingsTopologyMap` duplication noted but benign; `main.tsx` tray/toast detection triple (`pathname` + `search` + `__TAURI_METADATA__`) is belt-and-braces, keep.

### What's actually fine (verified this pass)

No whole-store subscriptions anywhere (4.1.6 selectors clean); no direct `invoke` outside `services/`; `overlayStack` + `useOverlay` correct (FILO, capture, idempotent install; Monitoring's bespoke outside-click coexists safely with `dismissOnOutside:false`); `SettingsContext` debounce + `isCommitting` guard correct; `useSettingsPage` rAF-batched measurement, changed-flag line calc, discard-on-close all correct; `useHistory` cancellation flags, ref-guarded retry, clamped indices, timer cleanups all correct; `Memory.tsx` version-gated fetch, tooltip toggle-off, mobile select-mode gate all correct; `useMonitoringMetrics`/`useVoxFootprint` in-flight guards + visibility gating correct; `useStreamingRenderer` additive-detection + rAF + hidden-tab fast-forward + unmount cancel correct; `useDynamicFPS` ref-mirror loop + pause teardown + manual start/stop correct; `useMemoryTrace`/`MemoryTracked` + `isProfilerActive` gate correct; `AdvancedOrb` disposes geometries/materials/renderer on unmount; `PipelineField`/`StatusCapsule`/`Header`/`Footer`/`TranscriptRenderer` clean; `WizardRoot` machine guards + `GO_TO` reachability duplicated consistently; `DetailPanel` resets pagination per session; `NotificationBell` StrictMode-safe listener init; `ToastApp` timer/rAF cleanup on unmount correct (except nested destroy timer + log); `SegmentedControl`/`ToggleTile`/`Drawer`/`Tooltip` primitives clean; `FALLBACK_CAPS`/`GOVERNOR_LABELS`/accent fallbacks benign-necessary (kept); id-driven Model Hub (no provider literals in workspaces) is the right call — `TtsModelWorkspace` needing no `kokoro` literal is evidence the design holds.

### Bottom line

Voice-path verdict stands (14 will-break, fix keys/registry/states first). This sweep adds 8 more will-break, all outside the voice path: hook-order violations (latent crash), ingestion-pause divergence (settings lies), STT-cloud dead-end, realtime phantom toggles, tests blocking the fix, onboarding mic gate, dead deep-link, write-only machine state. Fix order overall: F5 with the key renames (unblock), F1 (crash class), F2–F4 (settings truthfulness), F6–F8 (onboarding/wizard), then G1–G7.
