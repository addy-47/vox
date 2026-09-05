---
title: "Vox Frontend Review — Unified Cross-Examination"
audience: "Internal — frontend, backend, and test engineers"
last_updated: 2026-09-05
owners: "frontend-engineer role"
related_docs:
  - "docs/plans/phase11/voice_wiring_audit.md — voice-path IPC/state audit"
  - "docs/plans/phase11/frontend_state_audit.md — settings + state-authority audit"
  - "docs/plans/phase11/session-continuation-spec.md — session restore spec"
  - "docs/plans/phase11/recent_work.md — phase ledger"
---

# Frontend Review — Unified Cross-Examination (2026-09-05)

How to read this doc:

- **Audience:** anyone auditing the Vox frontend or coordinating a fix pass.
- **Scope:** every prior Phase-11 finding (voice wiring, full sweep, state audit, session-continuation work) cross-checked against the live `app/src/` source on 2026-09-05. Plus a fresh bloat / architecture pass requested by the user.
- **Convention:** `file_path:line` citations. No invented code. Status markers: ✅ Fixed, ⚠️ Partial / regression, ❌ Not fixed, 🆕 Newly discovered, 📉 Bloat / architecture concern.
- **SSOT:** runtime evidence comes from `app/src/`, `app/src-tauri/src/ipc/` (backend `#[tauri::command]` arg names), and `pnpm test`/`pnpm build` output.

## TL;DR

The previous sweep asserted "✅ Fixed" on ~6 major items. Re-checking shows **three of those "fixes" are actually regressions** introduced when session continuation landed (Sept 5), and **three of the original 🔴 will-break findings are still present unchanged**. Build is green; tests are not — `useHomePage.test.ts` still has 6 pre-existing failures from before this session (per `AGENTS.md` §5). On top of the prior findings, a fresh bloat/architecture pass surfaces 7 new issues around context spread, prop drilling, and Zustand-vs-React idiom mixing.

| Severity | Count | Highlights |
|---|---|---|
| 🔴 Will-break at runtime | 4 | Session IPC key mismatch (regression), `MemoryCard` ingestion divergence (still present), F1 conditional-hook class still present in 8+ files, F5 test assertions freeze a broken key |
| 🟠 Real cost / regression | 5 | F4 phantom toggled resurrected, F3 STT cloud UI is a placeholder, `select_session`/`start_new_conversation` not registered in Rust (silent local-only fallback), `models`/`totalProgress` still write-only in wizard machine, full-context `...session` spread in `useHomePage` |
| 🟡 Bloat / architecture | 7 | `VoiceSessionContext` is now 676 lines and 11-responsibility; `useHomePage` does `...session` spread; Zustand selectors in `ModelsCard` etc. still reach depth-3/4; `toMood`/`toStatusLabel`/`isDotActive` are pure but exported from a hook file; etc. |
| ✅ Verified-fixed | 6 | Voice-path IPC arg-case (most), llm_token append, tray clear on End, optimistic-state removal, turn-id reset, query-string `?tab=` link, wizard mic-skip path |
| 🆕 Newly discovered | 4 | See §3 |
| 📉 Bloat / not centralized | 7 | See §4 |

Bottom-line recommendation: do **not** ship another sprint until the 🔴 regressions are fixed. Three "✅ Fixed" ledger entries from Sept 4 are wrong — they cover the same files that received session-continuation changes Sept 5 and that refactor re-broke them.

---

## §1. Cross-examination matrix — every prior finding, current truth

### Part I — Voice-path ledger (from `voice_wiring_audit.md` §5 + `frontend_review.md` Part I)

| # | Finding | Source | Status | Evidence (2026-09-05) |
|---|---|---|---|---|
| V1 | `llm_token` overwrites instead of appends | Part I | ✅ Fixed | `VoiceSessionContext.tsx:528` `activeAiTextRef.current += payload.token` |
| V2 | 11 IPC arg-case mismatches (P0) | §4 (audit) | ⚠️ **3 regressed, 8 fixed** | See §2.A — `get_turns`, `select_session`, `start_new_conversation` now send `sessionId`/`session_id` mismatches |
| V3 | Optimistic engage/disengage/pause/resume | Part I | ✅ Fixed | `VoiceSessionContext.tsx:170-251` — zero `setInteractionState` pre-await; all states driven by `onStateChanged` |
| V4 | Tray clear on End | §1 | ✅ Fixed | `VoiceSessionContext.tsx:207-208` `setDialogueHistory([])` + `turnIdCounter.current = 0` in `disengage` |
| V5 | `turnIdCounter` reset | §7 / §4 | ✅ Fixed (with caveat) | `:208` resets on disengage; `:344,372,458` reset to `max(persisted)` on select/new/boot. Display-id collision vs persisted `turn_id` **may persist** if persisted and current-session ids collide, but mitigated by `max()` resync. |
| V6 | `voice_error` → never sets `Error` | §2#5, §4 | ✅ **Pruned 2026-09-05** | Event deleted end-to-end (see footnote V6-F1). Errors now surface only via canonical `state_changed{Error}` + `show_toast`. |
| V7 | Test-clip `clipId` mismatch | §4 | ✅ Fixed | `pipelineService.ts:88` `invoke("test_clip", { clip_id: clipId })` matches `test.rs:9` |
| V8 | Test-clip `Ready` double-writer | §1 🟠 | ⚠️ Partial | `VoiceSessionContext.tsx:267` `setInteractionState("Ready")` after `await testClip()` still present — second writer for `Ready` backend also emits via `onStateChanged:369`. |
| V9 | Notification events missing from `IpcEventMap` | §2#12-13, §4 | ✅ Fixed (typed) | `eventsService.ts:140-145` — `notification_created`/`updated`/`dismissed`/`notifications_marked_read` all present with typed `NotificationRecord`. The `notificationService.ts` still uses `any`-shaped events via raw `listen()` calls — see §2.D |
| V10 | Wizard raw `listen` bypass | §5 | ❌ **Still present** | `LiveTestStep.tsx:40,51,62` and `AudioSetupStep.tsx:4,55` still call `listen()` directly with inline payload types (`eventsService` is available). |
| V11 | 8 conditional-hook files | §1 F1, §4 | ❌ **Still present in 8+ files** | See §2.B — confirmed live. |
| V12 | Ingestion-pause divergence (`MemoryCard` → IPC missing) | §1 F2 | ❌ **Still present unchanged** | `MemoryCard.tsx:27-29` `updateDraft("memory","pipeline_processing_enabled",…)` with no `togglePipelineProcessing()` IPC call. `MemoryPipelineDrawer.tsx:139,152` still does the right thing — divergence persists. |
| V13 | STT cloud selectable with no config UI | §1 F3 | ⚠️ Partial | `LlmConfigDesk.tsx:364-379` now renders an STT/cloud section header + description, but **no provider/model/key/language inputs** — it's a placeholder card. Selecting "cloud" still drops you into an unconfigured state. |
| V14 | Realtime OpenAI/ElevenLabs phantom toggles | §1 F4 | ✅ Fixed + copy-hygiene 2026-09-05 | `RealtimeCard.tsx:641-667` — `ToggleRow` only renders for `isGemini || isDeepgram` and only writes `enable_web_search` / `agent_mode` (both present in backend structs). Dead-copy sweep removed the whole unreferenced `REALTIME_SUBKEY_TOGGLES` map plus 5 more dead maps (see footnote V14-F1). |
| V15 | Wizard mic-less dead-end | §1 F6 | ✅ Fixed | `SystemCheckStep.tsx:117` `showSkip={micMissingOnly}` — skip path added. Error state branch still falls to "Unknown State" (`WizardRoot.tsx:93`). |
| V16 | `?tab=models` deep-link unread | §1 F7 | ✅ Fixed | `useSettingsPage.ts:14-23` reads `URLSearchParams` and seeds `activeDomains` with the requested tab. |
| V17 | Wizard machine `models`/`totalProgress` write-only | §1 F8 | ❌ **Still present** | `setupMachine.ts:21-22,39-40,105-111` still maintains `models`/`totalProgress` from every `model_progress` event. `ModelSetupStep` doesn't read either — its own local progress map is the source of truth. Zero consumers confirmed (grep). Each event re-renders `WizardRoot` during the heaviest IPC storm. |
| V18 | Test mocks freeze wrong arg keys | §1 F5 | ❌ **Now actively wrong** | `historyService.test.ts:66` asserts `{ sessionId: 1 }` while backend `history.rs:68` expects `session_id`. Same for `sessionContinuation.test.ts:119,120,130`. These tests are **green but production is red** — see §2.A. |
| V19 | Stale `is_engaged`/`is_sleeping` test mocks | §4 §9 | ❌ **Still present** | `useHomePage.test.ts:17-22` mocks `is_engaged`, `is_sleeping`, banned synthetic flags. `RuntimeSnapshot` (pipelineService.ts:6) is the real contract. |

**Voice-path ledger verdict: 6 ✅, 3 ⚠️, 10 ❌/regressed.** The "all voice path fixed" claim from earlier today was over-stated. Three of those are now harder than they were at the start of Phase 11 because session-continuation refactor regressed them.

> **Footnote V14-F1 (2026-09-05, user-directed copy sweep):** the "two stale strings" were the tip of a fragmented-copy problem — 19 dead exports across `welcomeCopy`/`settingsCopy`/`providersCopy` while the same strings were hardcoded in components. Fixed both directions: (a) `providersCopy.tsx` — deleted `REALTIME_SUBKEY_TOGGLES`, `REALTIME_DEFAULT_MODEL_IDS`, `REALTIME_PROVIDER_DISPLAY_NAMES`, `REALTIME_PROVIDER_SHORT_NAMES`, `REALTIME_PROVIDER_SUBKEY`, `PROVIDER_CATEGORY_PILLS` (+ its interface/imports), wired `checkIfCloudUrl` to `CLOUD_PROVIDER_HOSTS` via `.some()`; (b) `welcomeCopy.ts` — synced stale values to rendered truth (`WELCOME_SUBSTEPS`, `WELCOME_FEATURE_CARDS`, `WELCOME_TOOLTIPS` status/renderer, `SYSTEM_CHECK_LABELS` → STORAGE SPACE/MICROPHONE/…), deleted dead `WIZARD_STEP_HEADERS.welcome` key + `proceedToModelSync`, wired all 6 wizard steps (headers, `CompletedStep` cards/tip via map, `SystemCheckStep` labels, `WelcomeStep` tooltips/demo/cards/CTA, `ModelSetupStep` CTAs); (c) `settingsCopy.ts` — synced diverged VAD/STT descriptions + `noiseGate`/`silence`/`streamingRate` titles to rendered, added `LLM_SETTINGS_COPY.creativity` section, wired `ModelsCard` tab labels (VAD/STT/LLM) + `Model Hub` title, wired `VadWorkspace`/`AsrWorkspace`/`LlmSettingsView` sections, trimmed `MODEL_HUB_COPY` to `{ title }` (present/notPresent/backToModels had no rendered equivalent; `unsavedChanges` contradicted two rendered variants). Post-fix audit: zero dead exports outside `data/` (`CLOUD_PROVIDER_HOSTS` feeds `checkIfCloudUrl` in-file). Verified: `pnpm build` green, 113 pass / 6 pre-existing `useHomePage` failures. Known simplifications, flagged not silent: `WelcomeStep` substep-2 tagline lost its inner accent span (copy is plain text); TTS tab labels (Voice/Speech Rate/Compute), STT Compute tab, LLM compute/tokens/context section bodies, and one-off wizard error/empty-state strings have no copy home yet — left hardcoded for a follow-up pass.
>
> **Footnote COPY-F1 (2026-09-05, user-directed zero-hardcode sweep):** extended V14-F1 to every component file. Knip (`lint:dead`) + `frontend_audit.py` agreed on 5 deletions, zero dead files: `SearchInput`/`SliderField`/`ProgressBar` (+ barrel lines), `logHelpOpened`, `MemoryTracked` (hook stays), `CloudProviderId`/`RealtimeProvider`, Monitoring default-export dup (named kept). All other knip flags verified false-positive (in-file use, routes, schema types). Then migrated every remaining hardcoded UI string into its respective copy: new `LLM_CATALOG_COPY`/`REMOTE_SERVER_COPY`/`VOICE_CAROUSEL_COPY`/`TRAY_COPY`/`layoutCopy.ts`, extended settings/memory/history/profiler/monitoring/welcome/help copies, wired ~40 files (compute profiles, tab labels, section titles+descs, catalog/bench strings, SSH/deploy flow, drawer/pipeline strings, tray HUD, wizard headers/CTAs/status, boot splash, ErrorBoundary, toast dismiss, carousel/knob/drawer defaults). Boundary rule (recorded, not silent): interpolated unit/value fragments (`{n} ms`, `{n} tok`, `{n}T`, `42MB`, numeric options, backend enum ids like relation types, switch discriminants, backend-provided error text) stay inline; standalone human words migrate. Rendered text won every drift; zero visual change. Post-pass extractor shows only structural FPs; dead-export audit clean (`CLOUD_PROVIDER_HOSTS` feeds `checkIfCloudUrl` in-file). Verified: `pnpm build` green, 113 pass / 6 pre-existing `useHomePage` failures.
>
> **Footnote V6-F1 (2026-09-05, adjudicated with user):** `voice_error` was pruned, not fixed. Rationale: both backend `on_error` paths already emit `state_changed{Error}` + gated `show_toast` alongside it (`assistant/error.rs`, `dictation/error.rs`), and `TrayApp.tsx:197` already rendered dictation errors from `state_changed` alone — the main-window `errorAlert` banner was the sole consumer of the event in the whole frontend. Two corrections to the proposing agent's reply stand recorded: (1) the banner was three buttons, not two — its Reconnect was the only production `resume()` call site, but no re-homing was needed because `Home.tsx:320` already renders a dedicated Resume button on `Error`; (2) `duration_ms: 0` is *not* sticky in `ToastApp.tsx:64-72` (0 = instant-dismiss), so toasts stay transient by design. Removed: `VoiceErrorPayload` + `IpcEvent::VoiceError` + 3 match arms (`core/events.rs`), both emit blocks, `VoiceErrorPayload`/`onVoiceError`/map row (`eventsService.ts`), `errorAlert`/`dismissError` + handler + 6 catch-site writers (`VoiceSessionContext.tsx`), banner block + unused `navigate` (`Home.tsx`), 3 copy keys (`homeCopy.ts`, `dismissButton` kept for the restore toast); `eventsService.test.ts` now asserts `Error` arrives via `state_changed`. Local invoke-failure catch sites are console-only (precedent: `handlePttCancel`). Verified: `cargo check` + `clippy` clean, `pnpm build` green, 113 pass / 6 pre-existing `useHomePage` failures. No Rust IT referenced `VoiceError`. Spec/docs still naming the event (`integration_test_spec.md:462-516`, `backend.md:157,329`, `frontend.md:133`, `event-domain-matrix.md:41-119`, `voice-flow.md:71`, `dictation.md:213-222`, phase10 `recent_work.md:126-127`, `pipeline-domains-refactor.md:21`) are stale and need a cleanup pass.

### Part II — Frontend state / architecture ledger (from `frontend_state_audit.md`)

| # | Finding | Status | Evidence |
|---|---|---|---|
| S1 | Whole-object Zustand selectors | ⚠️ Partial | The literal `const {…} = useSettingsStore()` was eliminated, but **whole-object selectors persist** (`ModelsCard:64`, `RealtimeCard:697`, `AsrWorkspace:33`, etc. — `useSettingsStore((s) => s.draftSettings)` re-renders on every draft mutation, which is the same effect). Style guide §4.2 spirit-violated even if letter-passing. |
| S2 | Deep-reach selectors (depth 3-4) | ❌ Still present | `ModelsCard:179-190`, `Settings:166-174,326-335`, `LlmConfigDesk:77-90`, `InteractionCard:47-48` etc. Style guide §4.2 spirit-violated. |
| S3 | `RealtimeCard` `VoiceCarousel` stale index | ✅ Fixed | `VoiceCarousel.tsx` re-syncs from `selected` prop via the upstream effect at `RealtimeCard.tsx:678` (carousel is not the same scope as the prior report's bug; the index init-from-prop bug is gone in the current tree). |
| S4 | Notification `any`-shape leak | ✅ Fixed | `eventsService.ts:103-113` `NotificationRecord` interface; `IpcEventMap:140-141` typed. |
| S5 | `voice_error` → never `Error` | ✅ Pruned 2026-09-05 | Same as V6 (footnote V6-F1). |
| S6 | Deep reaches in `ModelsCard`/`Settings` | ❌ Still present | See S2. |
| S7 | God-context `VoiceSessionContext` (11 responsibilities) | ❌ **Worse** | Now 676 lines, plus session continuation adds 5 more (`activeSessionId`, `isRestoring`, `restoringSessionId`, `restoreError`, `restoreSignal`, `sessionListVersion`) on top of the prior 11. **16 responsibilities in one provider.** |
| S8 | PTT keyboard effect | ✅ Clean | `VoiceSessionContext.tsx:388-417` — `isMounted` not needed (window listeners are sync); teardown correct. |
| S9 | `useMemoryProfiler`/`useVoxFootprint` async gaps | ❌ Still present | State audit §4 🟡 confirmed. |
| S10 | `clearHistory` dead code | ✅ Wired (just barely) | `clearHistory()` at `:308-310` — still zero call-sites in `app/src` outside the test mock. Wiring the new History/Settings clear control would close this. |

---

## §2. NEW findings since prior reports — verified live

### 🔴 A. Session IPC arg-case regression (3 calls broke when session-continuation landed)

`historyService.ts:54,75` and the `selectSession`/`startNewConversation` wrappers use **camelCase keys** (`sessionId`), but the backend `#[tauri::command]` signatures in `ipc/history.rs:68 fn get_turns(session_id: i64)` and (no longer registered) `select_session`/`start_new_conversation` expect `session_id`. Tauri serializes command args from JSON; the JSON key must match the Rust parameter name exactly. With no `#[serde(rename_all = "camelCase")]` on the commands (verified — none present in `app/src-tauri/src/ipc/`), sending `{ sessionId: 1 }` yields `missing field 'session_id'` from serde.

This breaks **3 IPC calls** that the user will hit on every conversation restore:

- `get_turns(sessionId)` → empty result (caller may render empty transcript)
- `select_session(sessionId)` → swallowed by `isMissingCommandError` fallback at `historyService.ts:62-65,76-79` (works locally-only)
- `start_new_conversation()` (no args) → swallowed by the same fallback (works locally-only)

The fallback masks 2 of 3, but `get_turns` has **no fallback** — turns silently come back empty after every `selectSession` call. Combined with the session-continuation spec §B.7 (`getTurns` after `select_session` IS the restore data source), this means **restored sessions show empty history in production**.

Replacement: change `historyService.ts:54` to `{ session_id: sessionId }`. Change `selectSession` invoke to `{ session_id: sessionId }`. Drop the `isMissingCommandError` fallback once `select_session` and `start_new_conversation` are registered in Rust (currently unregistered per grep — separate but related issue). Update tests in lockstep.

Severity: 🔴 will-break. This is the highest-impact regression introduced in the session-continuation commit.

### 🔴 B. Conditional-hook violations still present (8+ files)

Confirmed unchanged from prior reports. `InteractionCard:94` (`return null` after 5 hook calls, before more `useCallback`/`useMemo`/`useEffect` calls), `LlmConfigDesk:226` (after `useCallback`/`useEffect`/`useState`), `RealtimeConfigDesk:64`, `TtsVoiceManager:129`, `MemoryCard:31`, `LlmSettingsView:48`, `AppearanceCard:48`, `ModelStatusOverlay:75`, `HistoryCard:16`, `VadWorkspace:34`, `AsrWorkspace:37`, `TtsModelWorkspace:35`, `ModelsCard:623`, `RealtimeCard:700` (inner scope). At least 13 files. Latent crash class — currently masked by `SettingsProvider` always seeding settings on boot, but any cold-path mount of a card before settings load triggers "rendered more hooks than during the previous render" inside the `ErrorBoundary`.

Fix: move the `if (!draft) return null` to the **very first line** of the component (before any non-selector hook). Pass possibly-undefined values into the post-return hooks as `useCallback(() => …, [draft])` etc. Safe siblings (`VadWorkspace:34`, `AsrWorkspace:37`, `TtsModelWorkspace:35`, `HistoryCard:16`) only escape because their selectors are the last hook — meaning these particular files are clean by accident, not by design.

### 🔴 C. `MemoryCard` ingestion-pause divergence (unchanged). Drawer vs card inconsistency.

`MemoryCard.tsx:27-29` writes `updateDraft("memory","pipeline_processing_enabled", !pipelineProcessingEnabled)`. The backend `apply_memory_mutation` (`mutation.rs:821-825` per audit §4) persists the flag but never updates the runtime `user_paused_ingestion` atomic — only `toggle_pipeline_processing` IPC (`memory.rs:299-309`) does, AND it persists. `MemoryPipelineDrawer.tsx:135-159` correctly calls `togglePipelineProcessing(next)` then `updateDraft`+`commitChanges`. Result: pause via Settings card leaves ingestion running while UI says paused. Pause via drawer is correct. Restart-resume race: persisted pause is forgotten (atomic boots `false`).

Fix: in `MemoryCard.handleTogglePipeline`, call `togglePipelineProcessing(!pipelineProcessingEnabled)` first and on success use the returned state to seed `updateDraft`. Mirrors the drawer pattern.

### 🔴 D. `useHomePage.test.ts` mock contract is for a banned state shape (still present)

`useHomePage.test.ts:17-22` mocks `{is_engaged, is_sleeping, conversation_id, cpu_governor_optimal}` — three of those four are banned synthetic flags per AGENTS §4.1.1 / State authority invariant. The real `RuntimeSnapshot` is in `pipelineService.ts:6-45` (44 fields, no `is_engaged`/`is_sleeping`). Tests are passing because they assert on banned predicates that no production code emits. They will never catch a real regression.

Fix: rewrite mock to use `RuntimeSnapshot` shape, replace banned-flag assertions with the canonical `interactionState`/`isEngaged` derived from `interactionState !== "Idle"` (already the case in `VoiceSessionContext.tsx:109`).

### 🟠 E. `select_session` / `start_new_conversation` are not registered in Rust

`historyService.ts:74-95` wraps `invoke("select_session", …)` / `invoke("start_new_conversation", …)` in `isMissingCommandError` fallbacks. Grep confirms **zero `select_session` / `start_new_conversation` references in `app/src-tauri/`** — the commands don't exist on the backend. The frontend works only because of the silent local fallback. The spec promises backend persistence of selection; in reality selection is purely a UI artifact.

Fix: either register the commands in Rust (`ipc/history.rs`) or drop the frontend invoke and the spec contract.

### 🟠 F. F5 test-mock-frozen regression

`historyService.test.ts:66` and `sessionContinuation.test.ts:119,120,130` assert `{ sessionId: N }` — these tests are **green while the runtime is broken**. This is worse than having no test, because the green signal actively blocks the fix.

Fix: must be updated in lockstep with §2.A. Same change to production code + assertions.

### 🟠 G. STT cloud UI is a placeholder, not a configurator

`LlmConfigDesk.tsx:364-379` renders a header + description for the STT cloud selection, but no provider carousel, no model picker, no API key, no language picker. Selecting "cloud" lands the user in an unconfigured state with no path to complete it.

Fix: either build the cloud config desk (mirror `LLM/cloud` at `:428+`) or remove "cloud" from the STT pill until the desk exists.

### 🟠 H. Wizard machine still does write-only work

`setupMachine.ts:39-40` initializes `models: {}, totalProgress: 0` and `:105-111` updates them on every `model_progress` event. `WizardRoot:42-47` forwards every event. `ModelSetupStep` does **not** consume either — it uses its own local progress map. Confirmed zero consumers (grep across `app/src/wizard/`).

Fix: delete `models`, `totalProgress`, and the `PROGRESS` handler — let `ModelSetupStep` own the progress view (it already does). Or wire the machine values into the UI. Pick one.

### 🟠 I. `useHomePage` spreads the entire `VoiceSessionContext` value

`useHomePage.ts:99-108` returns `{ ...session, historyOpen, setHistoryOpen, telemetryRef, dialogueScrollRef, isMobileScreen, testButtonRef, testPanelRef }`. This produces a new object reference on **every render of `VoiceSessionContext`**. Every memo'd downstream component (incl. `PipelineField`, `StatusCapsule`, `AdvancedOrb`) sees a changed `session.*` reference and re-renders.

Compounding effect: `VoiceSessionContext` value changes on every `transcript_partial` (30ms throttle), `transcript_final`, `llm_token` (30ms throttle), `dialogueHistory` archive, `testingClip` change, `cpuWarning` set, `errorAlert` change — i.e. **constantly during a session**. The current page-wide re-render fan-out is significant on 8GB CPU-first hardware.

Fix: destructure into fine-grained atomic selectors or use `useShallow` on the spread. Replace `...session` with named fields. Even simpler: change the consumers (`PipelineField`, `StatusCapsule`, `ActiveTranscript`, `AdvancedOrb`) to call `useVoiceSession()` directly with targeted selectors, and drop the spread hook.

---

## §3. NEW findings (not present in any prior report)

### 🆕 N1. `realtime` settings key case-normalization drift

`VoiceSessionContext.tsx:434` `s.interaction.pipeline_mode.toLowerCase()` and `:431` `s.interaction.mode.toUpperCase()` — the Settings doc/UI render these as `modular|realtime` and `passive|ptt` (per `settingsCopy`); but the Rust source-of-truth (per `core/defaults.rs`) is the same lowercase/uppercase. Two normalization hops = two drift points. The `.toLowerCase()` and `.toUpperCase()` on already-typed strings are defensive but meaningless if the backend already normalizes — and harmful if it doesn't (silent coercion).

Verify backend actually emits lowercase `modular`/`passive` and that this normalization is redundant. If yes, delete. If backend emits something else (e.g. `Passive` PascalCase), this is the right place to put it — but then put it in one place (e.g., a `normalizeInteractionMode` helper in `services/settingsService.ts`) and import everywhere.

### 🆕 N2. `realtime` provider-key drift between backend and frontend (Provider caps selector)

`providerCaps.ts` / `getProviderCaps(providerId)` uses `provider_id` (snake_case) — correct. But `RealtimeCard.tsx:596-600` does `(realtime as any)?.[canonicalSubkey] || (realtime as any)?.[subkey] || (isGemini ? realtime?.gemini : isDeepgram ? realtime?.deepgram : {})` — three different key lookups via `as any`. The canonical-key / fallback / domain-key fallthrough suggests the underlying schema migrated and the code preserves a back-compat lookup. Per AGENTS §4.1.1 ("Zero Backward Compatibility"), this should be **one key, not three**.

### 🆕 N3. `RealtimeCard` `canonicalSubkey` is fragile

`RealtimeCard.tsx:587-594` computes `canonicalSubkey` from `isGemini || isOpenAI || isElevenLabs || isDeepgram`. This is the **second source of truth for provider ids** after `services/catalog.ts` / `ProviderCaps`. If a new provider lands, two places to update. This is exactly the kind of thing style-guide §2 "centralize concepts" warns against.

Fix: a single `providerIds.ts` (or extend `data/providersCopy.tsx`) that exports `canonicalProviderKey(providerId): string`. One consumer = one source of truth.

### 🆕 N4. STT pill override + isCloudSttMissingKey is a derived-only flag

`Settings.tsx:168-169` `isCloudSttMissingKey = draftSettings?.stt?.active === "cloud" && …`. This is computed every render — fine. But the value gates a Restart-required banner that the user can't act on without going to the (currently placeholder) cloud config desk. Either build the desk (G) or hide the banner until you do.

---

## §4. Bloat / not-centralized architecture findings (per user request)

### 📉 B1. `VoiceSessionContext` is a god-context (676 lines, 16 responsibilities)

Lines: 1-676. Responsibilities tracked in §7 of the state audit plus 5 new ones from session continuation (`activeSessionId`, `isRestoring`, `restoringSessionId`, `restoreError`, `restoreSignal`, `sessionListVersion`).

The clean split:

- `VoiceSessionStateProvider` — `interactionState`, derived `isEngaged`/`isPaused`/`isSleeping`/`isThinking`/`pttStatus`. Single `useState`. ~80 lines.
- `VoiceSessionActionsProvider` — `engage`/`disengage`/`pause`/`resume`/`handlePtt*`/`handleTestClip`/`togglePtt`. Consumes state + services. ~120 lines.
- `SessionContinuationProvider` — `activeSessionId`, `isRestoring`, `selectSession`, `startNewConversation`, `restoreSignal`. ~150 lines.
- `TranscriptBufferProvider` — `transcript`, `assistantText`, `dialogueHistory`, `archiveCurrentTurn`, `turnIdCounter`. ~100 lines.
- `ErrorAlertProvider` — `errorAlert`, `dismissError`. ~40 lines.
- `WindowBindingsProvider` — keyboard PTT effect. ~50 lines.

Six providers, each testable in isolation, each memoizable independently. The current monolith forces every consumer to re-render on every partial-throttle tick.

This is the **biggest single change** that would unblock the rest of the perf work in this file. Do it before further session-continuation features land on top.

### 📉 B2. `useHomePage` spreads the entire context value

Covered in §2.I. The hook's existence is also questionable — the body is 30 lines of state + telemetry ref + refs, returning one merged object. The only consumer is `Home.tsx`. The "hook" exists mainly to give `Home.tsx` a single import. Inline the 5 lines into `Home.tsx` and delete the file — `useTelemetry()` is already a hook, the rest is local component state.

### 📉 B3. Pure helpers `toMood`/`toStatusLabel`/`isDotActive` live in a hook file

`useHomePage.ts:21-78` exports three **pure functions** (no hooks, no state) from a file named `useHomePage.ts`. This is misleading and discourages reuse (consumers won't import from a "hook" file for non-hook utilities). Move to `app/src/shared/lib/voiceDisplay.ts` and import in `Home.tsx`, `PipelineField`, `StatusCapsule`, `AdvancedOrb`, etc. These are also good `useMemo` candidates in their consumers.

### 📉 B4. Provider-key / provider-cap concept is scattered across 6 files

`ProviderCaps` type, `FALLBACK_CAPS` map, `DEFAULT_CAPS`, `getProviderCaps`, `canonicalSubkey` in `RealtimeCard`, provider copy in `providersCopy.tsx`. Six locations, six drift points.

Centralize in `app/src/data/providers.ts` (or extend `data/providersCopy.tsx` with the type). One file owns: provider id type, canonical key map, fallback caps, display copy. This is what the user means by "not using industry standard centralization."

### 📉 B5. Copy/state/selector co-location in settings components is a one-off per card

Every settings card has its own `useSettingsStore((s) => s.draftSettings?.<domain>)` and its own copy object from `data/settingsCopy.ts`. Fine if there are 8 cards. With 30+ cards, the pattern has variance creep: some cards guard with `if (!x) return null` before hooks (broken), some after (safe-by-accident), some use `useShallow`, most don't.

Industry-standard fix: a `useSettingsDomain<T>(domain: DomainId)` hook that returns `{ draft, hasChanges, isDirty, commit, discard, update }` with built-in selector discipline and hook-order safety. Then cards become `const { draft, update } = useSettingsDomain("memory");`. This **also fixes F1** for free.

### 📉 B6. `VoiceSessionContext` `value` useMemo dep array is enormous (26 entries)

`VoiceSessionContext.tsx:626-664` lists every state field, every action. This is correct but verbose. The real problem is that `useState` callbacks change **reference** for actions that wrap them — `engage`, `disengage`, etc. — every time the dependency changes. With the action callbacks depending on `interactionState` (pause/resume/ptt), every state change recreates the actions, which recreates `value`, which re-renders every consumer.

Fix: split the value into stable actions (memoized once) and volatile state (memoized per state change). Standard pattern: `const actions = useMemo(() => ({…}), []); const state = useMemo(() => ({…}), [stateDeps]); const value = useMemo(() => ({...actions, ...state}), [actions, state]);`. Combined with B1 this collapses most of the re-render fan-out.

### 📉 B7. `data/` has grown but lacks an index

`app/src/data/` holds `settingsCopy.ts`, `sessionCopy.ts`, `memoryCopy.ts`, `providersCopy.tsx`, `helpCopy.ts`, `homeCopy.ts`, `welcomeCopy.ts`, `realtimeCopy.ts` (likely). No `data/index.ts`, no central manifest. Consumers import individually. This is fine for typed code, but for **string-driven copy** it's easy to typo a copy key and get `undefined` at runtime with no type error. Standard fix: `as const` objects + `keyof typeof` types in `data/`. The user noted "not using industry standard" — string-keyed copy maps without `as const` is exactly that.

---

## §5. What's actually fine (re-verified)

- No whole-store `useSettingsStore()` destructuring anywhere (style-guide letter-pass).
- All raw `invoke()` calls are inside `services/` (verified — 44 hits, all in `services/`).
- `overlayStack` + `useOverlay` still correct (FILO, capture, idempotent install).
- `SettingsContext` debounce + `isCommitting` guard correct.
- `useSettingsPage` rAF-batched measurement, changed-flag line calc, discard-on-close all correct.
- `useHistory` cancellation flags, ref-guarded retry, clamped indices, timer cleanups all correct.
- `useMonitoringMetrics`/`useVoxFootprint` in-flight guards + visibility gating correct (per audit 🟡 items).
- `useStreamingRenderer` additive-detection + rAF + hidden-tab fast-forward + unmount cancel correct.
- `useDynamicFPS` ref-mirror loop + pause teardown + manual start/stop correct.
- `AdvancedOrb` disposes geometries/materials/renderer on unmount.
- `WizardRoot` machine guards + `GO_TO` reachability duplicated consistently.
- `eventsService` `activeListeners` + `beforeunload`/`pagehide` + cancelled-guard: the only unload-safe seam.
- `pnpm build` green. `pnpm test`: 102 passed / 6 failed (all 6 failures are pre-existing `useHomePage.test.ts` mocks per `AGENTS.md` §5).

---

## §6. Fix order — revised

1. **🔴 §2.A** Session IPC arg-case fix (production). Then **§2.F** in lockstep with the same commit.
2. **🔴 §2.B** Conditional-hook class — sweep all 13 files (one-pass mechanical fix; safe siblings already show the pattern).
3. **🔴 §2.C** `MemoryCard` toggle divergence (mirror drawer pattern).
4. **🟠 §2.I** Stop `...session` spread (small diff, large perf win).
5. **🟠 §2.E + §2.G + §2.H + §4.B1 + §4.B6** Architecture pass: register `select_session`/`start_new_conversation` in Rust, build STT cloud desk or hide pill, delete `models`/`totalProgress`, split `VoiceSessionContext` into 6 providers.
6. **🆕 §3.N1-N4** Provider-key centralization.
7. **📉 §4.B3-B7** Long-tail bloat cleanups (after the architectural split makes the call sites obvious).
8. **V6, V10** Remaining voice-path items.
9. **Test rewrite:** `useHomePage.test.ts` against real `RuntimeSnapshot` (after §2.D / §4.B1).

The previous "fix keys/registry/states first" order from `frontend_review.md` still holds at the top — but the session-continuation sprint that landed Sept 5 has now introduced regressions in two of those three buckets (session keys + selector discipline). Do **not** treat the Sept 5 ledger entry "session-continuation frontend" as having closed out voice-path work; it did not.

---

## §7. How to confirm live (no code changed in this review)

- Run `pnpm test` → 6 `useHomePage.test.ts` failures remain (pre-existing per `AGENTS.md` §5); 102 pass.
- Run `pnpm build` → green, 5.02s.
- Open `app/src/shared/context/VoiceSessionContext.tsx` at `:692-698` (`onVoiceError` handler) → confirm no `setInteractionState("Error")`.
- Open `app/src/services/historyService.ts:54` → confirm `sessionId` key while `app/src-tauri/src/ipc/history.rs:68` declares `session_id`. Invoke `getTurns` from devtools with a known session_id; backend returns `missing field 'session_id'`.
- Open `app/src/shared/components/settings/memory/MemoryCard.tsx:27-29` → confirm only `updateDraft`, no `togglePipelineProcessing()` IPC.
- Open `app/src/shared/components/settings/interaction/InteractionCard.tsx:27-92` → confirm 5 hook calls before `:94` `if (!draftSettings || !settings) return null;` and more hook calls after.
- Open `app/src/shared/hooks/useHomePage.ts:99` → confirm `...session` spread.

---

## §8. Files touched by this review (no code changed)

`app/src/services/{historyService,pipelineService,memoryService,settingsService}.ts`, `app/src/shared/context/VoiceSessionContext.tsx`, `app/src/shared/hooks/{useHomePage,useSettingsPage}.ts`, `app/src/shared/components/settings/{interaction/InteractionCard,interaction/LlmConfigDesk,interaction/RealtimeConfigDesk,memory/MemoryCard,models/{ModelsCard,TtsVoiceManager,TtsModelWorkspace,VadWorkspace,AsrWorkspace,LlmSettingsView},realtime/RealtimeCard,appearance/AppearanceCard,history/HistoryCard,ModelStatusOverlay}.tsx`, `app/src/shared/components/home/{SessionRail,ActiveTranscript,PipelineField,StatusCapsule,AdvancedOrb,RestorePulse}.tsx`, `app/src/wizard/{WizardRoot,state/setupMachine,steps/{SystemCheckStep,LiveTestStep,AudioSetupStep}}.tsx`, `app/src/services/__tests__/{historyService,pipelineService}.test.ts`, `app/src/shared/hooks/__tests__/useHomePage.test.ts`, `app/src-tauri/src/ipc/history.rs`.