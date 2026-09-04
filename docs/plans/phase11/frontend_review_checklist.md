# Frontend Review Sprint Checklist (2026-09-03, expanded pass 2)

Scope: full frontend. 163 TS/TSX files in `app/src`. Voice-path sprints (A/B/C) + direct-read sweep (pass 2) below. Completion = all items checked or explicitly flagged. Re-enumeration confirms: `invoke(` only in `services/`, raw `listen` outside services in exactly 3 wizard steps, 12 + 3 `setInteractionState` sites.

## Sprint A — Voice session state (subagent, verified)
- [x] VoiceSessionContext.tsx (engage/disengage/pause/resume/ptt/testclip + 12 setInteractionState sites + owner filter + throttle timers)
- [x] Home.tsx (all buttons, orb PTT, error banner, tray slice(-10))
- [x] useHomePage.ts (toStatusLabel/isDotActive/toMood)
- [x] TrayApp.tsx (parallel string state + visibility-gated drops)

## Sprint B — IPC/event contracts + service boundary (subagent, verified)
- [x] pipelineService.ts + eventsService.ts + historyService.ts + notificationService.ts
- [x] settingsService.ts + modelService.ts + memoryService.ts + windowService/toastService/memoryProfilerService
- [x] LiveTestStep.tsx + ModelSetupStep.tsx + AudioSetupStep.tsx (direct listen)
- [x] notificationStore.ts + useTelemetry.ts + TitleBar.tsx

## Sprint C — Dead code / fallbacks / redundancy (subagent, verified)
- [x] hasCachedSession, clearHistory collision, aliases, isSleeping, SetupStep union, llm_token, fallbacks, stale test mock, resolve_memory_conflict orphan, TestClipsPopover

## Pass 2 — Direct reads (this thread)
- [x] shell: App.tsx, main.tsx, layout (EdgeNav, ResponsiveLayout, TitleBar), toast/ToastApp.tsx
- [x] stores+contexts: settingsStore, notificationStore, SettingsContext, MemoryProfilerContext
- [x] hooks 13/13: useInteraction, useSettings, useSettingsPage, useOverlay, useVisibility, useStreamingRenderer, useDynamicFPS, useTelemetry, useMonitoringMetrics, useMemoryProfiler, useMemoryTrace, useVoxFootprint (+useHomePage prior)
- [x] services 11/11 + backend contract check (settings.rs structs, memory.rs:290-334, mutation.rs:810-825, state.rs:262)
- [x] pages 5/5: Home (prior), History, Memory, Monitoring, Settings
- [x] settings: ModelsCard (full), InteractionCard, LlmConfigDesk (full), DictationConfigDesk, PipelineModeCard, TriggerModeCard, CategorySelector, ProviderSelectorView (skim), RealtimeConfigDesk, HistoryCard, MemoryCard, ModelStatusOverlay, RestoreDefaultsButton, VadWorkspace, AsrWorkspace (logic), TtsVoiceManager (pt.1), LlmSettingsView (pt.1), AppearanceCard (hook map), RealtimeCard (logic)
- [x] tray 4/4, wizard Root/machine/6 steps, home visuals (AdvancedOrb head+disposal, PipelineField, StatusCapsule, Bell, Popover)
- [x] useHistory (full), MemoryPipelineDrawer (head), overlayStack, SegmentedControl, ErrorBoundary
- [x] cross-cutting greps: whole-store subs (0 found), console.log (2), TODO (0), location.reload (1), direct fetch (0), hook-order sweep (8 violations), test-assert sweep (3 files enshrine keys), query-param sweep (2 dead links)
- [x] presentational remainder pattern-swept (history orbit viz, monitoring chambers, profiler tabs, persona/appearance tails, catalog views, carousel tails, ui primitives, data copy, lib): no additional logic findings; stated as skim, not full-read
