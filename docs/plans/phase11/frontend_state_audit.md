# Frontend State / Settings / Owner Audit — Findings Only, No Fixes

Date: 2026-09-04 | Scope: `app/src/shared/context/` (3), `app/src/store/` (2), `app/src/services/` (11), `app/src/shared/hooks/` (14), settings components (31) + `pages/Settings.tsx` | Mode: audit only, no code changed.
Ground truth: `app/src-tauri/src/core/events.rs` (15 `IpcEvent` variants), `docs/backend.md` §8, `docs/features/voice-flow.md`.
Prior reports: `frontend_review.md`, `voice_wiring_audit.md` (no `feedback_review_report.md` file exists — feedback items live inside `frontend_review.md`).

> How to read: one axis at a time (§1–§10 = rubric axes). Each finding: severity + `file:line` + one-line evidence. No proposed fixes in this pass — fix pass follows triage. ✅ = verified pass, counts as a finding of "no bug" so silent areas stay visible.

---

## §0. Coverage

- Sprint 0: `events.rs`, `backend.md`, `voice-flow.md`, `frontend_review.md`, `voice_wiring_audit.md` — full reads.
- Sprint 1: `VoiceSessionContext.tsx`, `SettingsContext.tsx`, `MemoryProfilerContext.tsx`, `settingsStore.ts`, `notificationStore.ts`, `useInteraction.ts` — full reads.
- Sprint 2: all 12 service modules + `events.rs` re-read; `invoke`/`listen` grep across all of `app/src`.
- Sprint 3: all 31 settings components + `pages/Settings.tsx` — full reads.
- Sprint 4: 13 hooks + both contexts (hook/closure lens) — full reads.
- Sprint 5 (this report): dead-code caller grep + closure table.

---

## §1. Single-writer violations (settings / backend mirrors)

**`interactionState` — one declaration, three internal writers, one escape hatch:**

- ✅ `VoiceSessionContext.tsx:91` sole `useState("Idle")`; ✅ no `setInteractionState` in `SettingsContext`, `MemoryProfilerContext`, either store, `useInteraction`.
- ✅ `engage:147-164`, `disengage:166-193`, `pause:195-204`, `resume:206-216`, `handlePttStart/Stop/Cancel:218-246` — zero `setInteractionState`, comments `:157,186,199,211` hold. Prior optimism bugs are **fixed**.
- 🟠 `VoiceSessionContext.tsx:267` — `handleTestClip` writes `setInteractionState("Ready")` after `await testClip()`: post-confirm, but a **second writer** for the same `Ready` the backend also emits via `onStateChanged:369`. Double-source for one transition.
- 🟠 `VoiceSessionContext.tsx:378-380` — `onStateChanged` `Ready|Idle` clears `testingClip`, racing `:261` set + `:267` write (last-write-wins same tick).
- 🔴 `VoiceSessionContext.tsx:41,471` — `setInteractionState` exposed in context interface + forwarded in `useMemo` value: any consumer can bypass the backend event ordering entirely.
- 🟡 `VoiceSessionContext.tsx:334` boot `getRuntimeSnapshot()` sync + `:369` `onStateChanged` (Dictation-filtered `:365-367`) — the two legitimate writers.

**Other mirrors:**

- 🔴 `VoiceSessionContext.tsx:473,475,489,491` — `setInteractionMode`, `setPipelineMode`, `setDialogueHistory` (bypasses 100-cap `:138`), `setErrorAlert` all exposed; mode setters give every consumer a third writer beside boot hydration (`:324,327`) and `onSettingsUpdated` re-hydration (`:440,443`).
- 🟠 `VoiceSessionContext.tsx:210` — `resume` clears `setErrorAlert(null)` after `await resumeSession()` but before any `onStateChanged` confirm: optimistic clear (backend may warn+return with no emit, `session.rs:301-308`).
- 🟠 `VoiceSessionContext.tsx:391` — `onVoiceError` writes `setErrorAlert(msg)` last-write-wins against six catch-site writers (`:160,189,202,214,225,235`), no dedup; `handlePttCancel:239-246` writes nothing (asymmetric).
- 🟠 `VoiceSessionContext.tsx:177,193` — `wasTesting` captured before clear with `testingClip` dep: stale if clip changes mid-flight.
- 🟠 `VoiceSessionContext.tsx:337` — `setCpuWarning` set from boot snapshot when `!optimal`, never cleared to `null` afterwards (stale-once-set).
- 🟠 `VoiceSessionContext.tsx:130,220,259,382` — four archive triggers (`engage`, `pttStart`, `testClip`, `onStateChanged Listening`): double-archive overlap between PTT press and the `Listening` event it causes.
- 🟡 `VoiceSessionContext.tsx:348,350` — boot `getTurns().slice(-100)` restore consistent with 100-cap, but `turnIdCounter = max(id)` has no empty-array fallback beyond the `:349` guard.

## §2. Event contract fidelity (15/15 rows)

Backend `events.rs:134-150`, frontend `eventsService.ts:109-124`. Field names match on all 15 (no case bugs — the `clipId` class is **fixed**, all service wrappers send `snake_case`).

| # | Event | Payload | Semantics / consumer | Sev |
|---|---|---|---|---|
| 1 | `state_changed` | ✅ exact (`owner,state,turn_id`) | `VoiceSessionContext:363-369` + `TrayApp:201-222`, Dictation-filtered | ✅ |
| 2 | `transcript_partial` | ✅ exact | buffered + 30ms throttle-flush `:396-406`; **ignores `owner`** (Dictation partials can feed Assistant UI) | 🟡 |
| 3 | `transcript_final` | ✅ exact | clears pending partial timer first `:411-414`, correct order; same `owner` caveat; `TrayApp:238-246` commits directly | 🟡 |
| 4 | `llm_token` | ✅ exact | `activeAiTextRef.current += payload.token` `:423` — **appends; prior overwrite report is fixed** | ✅ |
| 5 | `voice_error` | ✅ exact | sole consumer only `setErrorAlert` `:388-392`; **nothing ever sets `Error` state** (grep: no `setInteractionState("Error")` anywhere) — backend `Error` unreachable from this event | 🟠 |
| 6 | `model_progress` | ✅ names; frontend `step` union is tolerant superset (snake+Pascal) | `ModelSetupStep.tsx:84` checks `p.step === 'Complete'` — backend never emits it (only `completed`) → dead half-condition | 🟡 |
| 7 | `telemetry` | ✅ exact (5 fields) | `useTelemetry:20-24` ref-only, correct; wizard `typeof === 'number'` branches (`LiveTestStep:63`, `AudioSetupStep:56`) dead — backend always sends object | 🟡 |
| 8 | `system_stats` | ✅ exact (7 fields) | sole consumer `TrayApp:250-254`; main window never subscribes | 🟡 |
| 9 | `settings-updated` | ✅ unit↔void | two consumers, both guarded (`VoiceSession:434-448`, `SettingsContext:36-43` 80ms debounce + `isCommitting`) | ✅ |
| 10 | `toggle_tray` | ✅ unit↔void | sole consumer `TrayApp:258-262` | ✅ |
| 11 | `show_toast` | ✅ exact incl. `duration_ms?`, lowercase levels | sole consumer `ToastApp:77`, all 4 levels covered | ✅ |
| 12 | `notification_created` | 🔴 shape divergence (below) + `any`-erased (`eventsService:121`) | backend `{id,category,title,message,status,session_id?,metadata,is_read,created_at}` vs `notificationService:4-16` `{…,turn_id,action_payload,*_ms}` — live backend-shaped events flow into frontend-shaped store (`notificationStore:94-106`) | 🔴 |
| 13 | `notification_updated` | 🔴 same as #12 (`eventsService:122`, `notificationStore:110-132` reads agree only by luck) | 🔴 | 🔴 |
| 14 | `notification_dismissed` | ✅ exact (`{id}`) | passthrough + filter by `id` | ✅ |
| 15 | `notifications_marked_read` | ✅ unit↔void | marks all read | ✅ |

## §3. Derived vs. stored

- ✅ Pure derivations, no bug: `isEngaged:98`, `isThinking:101`, `pttStatus:102-109` (in memo deps), `isDomainDirty/isCategoryDirty` (computed-on-call).
- 🟡 `VoiceSessionContext.tsx:99-100` — `isSleeping` and `isPaused` are the identical predicate (`=== "Paused"`); two names, one bit (both live-consumed: `Home:63-64,104,108,249`, `AdvancedOrb:587`).
- 🟡 `VoiceSessionContext.tsx:94` — `hasCachedSession` stored-immutable `false` (no setter); should be derived or deleted.
- 🟠 `settingsStore.ts:396,484-488` — `hasChanges` stored + hand-synced on every `updateDraft/commit/discard`; one missed `set` desyncs the save dot.
- 🟠 `useInteraction.ts:15,20,26-27,56-57` — `interactionId` + shadow `currentIdRef` hand-synced pair, bounds disagree with context (4000-char cap `:48` vs 100-turn cap).
- 🟡 Intentional drag/edit buffers with sync gaps: `AppearanceCard:20-26` (local color + direct DOM write, not reverted on discard), `DictationConfigDesk:30-37` (temp hotkey overwritten on mid-edit draft change).
- 🟠 `InteractionCard.tsx:35-36,87-95,169-187` — `sttPillOverride/ttsPillOverride` preview state dual-written with `updateDraft`, diverges from draft by design.
- 🔴 `RealtimeCard.tsx:470-472` (inner `VoiceCarousel`) — `index` init'd from `selected` prop once, never re-synced: stale index → wrong `currentVoice`.
- 🟡 Unmemoized per-render recomputes (could-be-`useMemo`, no bug): `ModelStatusOverlay:35-50`, `ModelsCard:141-202,627-652`, `TtsVoiceManager:149-156`, `TtsModelWorkspace:38-52`, `PersonaCard:252`, `LlmSettingsView:28-46`, `DictationConfigDesk:91-128`, `RealtimeConfigDesk:24-39` (`LlmCatalogView:81-151` already ✅ `useMemo`).

## §4. Hook correctness

- 🔴 Real conditional-hook violations — 5 files, 16 hook-sites after early `return null` (rendered-more-hooks crash class): `MemoryCard:17→25,29`; `TtsVoiceManager:81→114`; `RealtimeConfigDesk:17→41,52`; `LlmConfigDesk:36→46,92,102,116,173,190,207`; `InteractionCard:40→105,113,127,137`.
- 🟡 Prior report over-counted: `ModelStatusOverlay`, `AppearanceCard` (+ `HistoryCard`, `AsrWorkspace`, `TtsModelWorkspace`, `ModelsCard`, `LlmSettingsView`, `VadWorkspace`, `RealtimeCard`, `Settings.tsx`) verified clean — all hooks above their returns. 3 of the 8 named files were false positives.
- ✅ No hooks in loops/branches anywhere in scope; zero conditional hooks in all 15 hook/context files (Sprint 4).
- 🟠 `useSettingsPage.ts:132-138` — `document.getElementById` + layout reads **inside** `setLines(prev => …)` updater (impure updater; StrictMode double-invoke).
- ✅ Dep arrays touching `interactionState`/`settings`/IPC listeners: `VoiceSessionContext` master effect `[archiveCurrentTurn]` (stable, single subscription, no churn), keyboard effect complete, `value` memo complete; `SettingsContext`, `useMonitoringMetrics`, `useVisibility`, `useDynamicFPS`, `useStreamingRenderer` (exemplary rAF-from-ref) all pass.
- 🟡 Stale/unguarded async edges: `useVoxFootprint:34` no mounted guard + `:46` no in-flight guard (sibling `useMonitoringMetrics` has both); `useMemoryProfiler:38-40` eager `querySelectorAll("*")` per render, `:194` double-capture on mount, `:66` 6+ unguarded writes after `await`, `:141` untracked 350ms timer; `useMonitoringMetrics:12` dead `latestRef`; `useHomePage:100` spreads whole session value (per-token re-renders); `useOverlay:39` ref-mirror staleness tradeoff; `useMemoryTrace:18` re-registers if provider fns unmemoized.

## §5. IPC boundary discipline

- ✅ Raw `invoke(`: 44 hits, **all inside `services/`** — zero outside. Boundary holds.
- 🟠 Raw `listen(` bypassing `eventsService.on` (untyped, untracked): `LiveTestStep.tsx:3,40,51,62` (`transcript_partial/final`, `telemetry`), `AudioSetupStep.tsx:4,55` (`telemetry`).
- 🟡 `TitleBar:63-108` (`tauri://focus/blur` window-manager events, not `IpcEvent`) + `ToastApp:6` (`getCurrentWindow`) — window-chrome direct imports, not contract bypasses. Note only.
- ✅ No `invoke`/`listen` in any hook/context file; test `vi.mock` lines are expected.

## §6. Settings access pattern

- No selector layer (except 2-file `useSettings.ts` facade): **68 subscribed selectors + 12 imperative `getState/setState` = 80 touches** across 31 components + `Settings.tsx` (down from the 134-line `rg` count, which included multi-line selectors and store-internal lines — 80 is the audited call-site count).
- 🔴 Whole-object subscriptions (re-render on any settings change): `RealtimeCard:690-691`, `HistoryCard:13`, `AsrWorkspace:33`, `TtsModelWorkspace:31`, `ModelsCard:64`, `TtsVoiceManager:59`, `VadWorkspace:30`, `DictationConfigDesk:20`, `RealtimeConfigDesk:14`, `LlmConfigDesk:28-29`, `InteractionCard:27-28`, `Settings:141-142,301-302`. (Zero banned `const {…} = useSettingsStore()` destructuring — ✅ on the letter of §4.2, but whole-object selectors have the same effect.)
- 🔴 Deep reaches (selection, worst first): `ModelsCard:179-190,818-823` (depth 4, `llm.server/cloud.base_url/model/api_key/provider_name`); `Settings:166-174,326-335` (depth 4, incl. `(as any).gemini` `:174,335`); `AsrWorkspace:39,54,213` (depth 3); `LlmConfigDesk:77-90` (depth 3, incl. `tts.chatterbox_remote`); `TtsVoiceManager:153` (depth 3); `InteractionCard:47-48,75-76` (depth 3). Depth-2 reaches pervasive across `VadWorkspace`, `TriggerModeCard`, `PipelineModeCard`, `PersonaCard`, `AppearanceCard`, `DictationConfigDesk`, `LlmSettingsView`, `TtsModelWorkspace`, `HistoryCard` (full per-file paths in Sprint 3 output).
- 🟠 `LlmCatalogView:126,368,381,602`, `ModelsCard:486,507,538` — imperative `getState().draftSettings?…` reads inside click/keydown handlers (bypass subscription, can read mid-commit).
- 🟠 `Settings.tsx:145` — dynamic-closure selector `(s:any) => s.isDomainDirty(domain.id)` (non-atomic subscription) + `any`-casts `:145,174,335`; `RealtimeConfigDesk:34-36` `(realtime as any)` ×3; `RealtimeCard:571-572` `any`-typed props; `PersonaCard:170-177,192` `props:any` ×9; `TtsVoiceManager:231-233` `as any` ×3; `RemoteServerSetup:14` `setupStatus:any`.
- ✅ Direct `getSettings()` bypass count unchanged at **2** (`VoiceSessionContext:321,437`); store-internal `:402,682` are canonical.

## §7. God-context / responsibility bleed (`VoiceSessionContext`)

11 responsibilities: state authority (§1), lifecycle wrappers (thin ✅), PTT + Space keybinding (self-contained ✅), clip testing (duplicates `Ready` path 🟠), transcript buffering (shadows backend accumulator; `+=` + throttled paint 🔴-adjacent — currently correct, single event consumer), dialogue archive (**shadows backend `turn_id`: reset-to-0 `:175` collides with persisted `t.turn_id` after boot `max()` resync `:350` → duplicate display ids 🔴**), settings hydration (duplicates store + `SettingsContext` listener with divergent case normalization 🟠), boot runtime sync (no single boot orchestrator 🟠), error surfacing (parallel to `notificationStore`, no shared contract 🟡), launch gating (`isLaunching` engage/disengage only, not derived 🟡), escape-hatch setters (§1 🔴).

## §8. Cleanup / leak discipline

- ✅ `eventsService:130,144-145,152-184` — `activeListeners` + `beforeunload`/`pagehide` + cancelled-guard: the only unload-safe seam; all 6 context subscriptions, `SettingsContext`, `ToastApp`, `TrayApp`, `WizardRoot`, `ModelSetupStep`, `ModelsCard`, `useTelemetry` pair correctly.
- 🟡 `VoiceSessionContext:359-360` throttle timers never cleared on unmount (guarded, ≤30ms overrun, no `setState` — timer leak only).
- 🟡 `notificationService:46-74` Promise-wraps sync unlistens (one-microtask deferred teardown; uncalled return = leak, consumer-dependent).
- 🟡 `LiveTestStep:76-78` / `AudioSetupStep:69-71` `unlisten.then(u=>u())` with no cancelled-guard (unmount-before-resolve leaks); `AudioSetupStep` never `stopEngine`s on unmount (contrast `LiveTestStep:80`).
- ✅ `useMonitoringMetrics`, `useVisibility` (both timers via `clearTimers`), `useStreamingRenderer`, `useDynamicFPS`, `useSettingsPage` (nested rAF+timeout both cleared) all clean.

## §9. Dead code (diffed vs prior audit)

- ✅ Still dead: `clearHistory` (`VoiceSessionContext:274-275`) — zero callers (the `useMemoryProfiler:250` namesake is a different local function).
- ✅ Still orphan: `resolve_memory_conflict` (backend `memory.rs:78`, `lib.rs:606`) — zero FE callers.
- ✅ Still test-only: `handleEngage/handleEnd/handlePause/handleResume` aliases (`:500-503`) — prod-unused.
- 🟡 Changed since prior audit: `useInteraction` (marked "superseded") now has a live consumer — `TrayApp.tsx:7,41`. Either tray was always its home or something reintroduced it; needs an ownership decision (tray hook vs deleted duplicate).
- 🟡 Dead-flag/live-consumer: `hasCachedSession` still always `false` (`:94`), but `Home.tsx:343,355` renders resume-badge/aria off it (unreachable branch, not dead code).
- 🟡 Live duplicate, not dead: `isSleeping` consumed by `Home` + `AdvancedOrb` — keep-or-merge decision with `isPaused`.

## §10. Known-bug closure (prior `frontend_review.md` + `voice_wiring_audit.md`)

Re-verified this pass (evidence above); "ledger" = `AGENTS.md` §5 claim, not re-read here:

| # | Bug | Status |
|---|---|---|
| 1 | `llm_token` overwrite | ✅ Fixed (`+=` `:423`) |
| 2 | 11 arg-case mismatches | ✅ Fixed (all wrappers `snake_case`) |
| 3 | Optimistic engage/disengage/pause/resume | ✅ Fixed (zero pre-confirm writes) |
| 4 | Tray clear on End | ✅ Fixed (`:174-175`) |
| 5 | `turnIdCounter` reset | ✅ Fixed (`:175`) — but display-id collision vs persisted ids remains (§7 🔴) |
| 6 | `voice_error` → Error | Still present (§2#5) |
| 7 | Notification events missing | ⚠️ Partial (listeners exist; `any` + shape divergence new §2#12-13 🔴) |
| 8 | Wizard raw `listen` | Still present (`LiveTestStep`, `AudioSetupStep`) |
| 9 | 8 conditional-hook files | ⚠️ Partial — 5 files/16 sites real (§4); 3 named files were false positives |
| 10 | Ingestion-pause divergence (`MemoryCard` vs atomic) | Ledger-fixed, not re-read this pass |
| 11 | STT-cloud guard, phantom toggles, `?tab=` link, mic-less onboarding, setup-machine consumers | Ledger-fixed, not re-read this pass |
| 12 | VAD `dictation.enabled` gap, cold-engine race, `StopRealtime`/`StopWindowValidation` gaps | Backend scope — not re-verified (unchanged on frontend side: `mutation.rs:57` gate, dictation filter `:365-367` intact) |
| 13 | `backend.md` §8 lists 10 events, source has 15; `VoxEvent` count 14 vs 16 actual; `handlers/` vs `assistant/` rename drift | 🟡 Doc drift (new — Sprint 0): `events.rs` authoritative per invariant §4.1.2 |

## §11. Count sheet (for the fix pass)

- 🔴 9 (notification shape ×2, escape-hatch setters, display-id collision, 5 hook-crash files counted as one class with 16 sites, deep-reach worst sites, `RealtimeCard` stale index, `PersonaCard`/`RealtimeCard`/`TtsVoiceManager` `any`s, whole-object subscriptions class)
- 🟠 12 (test-clip second writer, testingClip race, resume error-clear, error-dedup, 4 archive triggers, cpuWarning stale, `hasChanges` hand-sync, `useInteraction` shadow pair, pill overrides, imperative `getState` in handlers, dynamic selector, updater-impurity)
- 🟡 30+ (owner-ignored partials, dead literals/branches, timer overruns, unguarded polls, eager sampling, hardcoded strings per §4d inventory, doc drift, dead-flag consumers)
- ✅ Holds: `invoke` boundary zero, `getSettings` bypass count 2 (no growth), dep arrays clean, no hooks-in-loops, `llm_token` append, arg-case, optimism removal.
