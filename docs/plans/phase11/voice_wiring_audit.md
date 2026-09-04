# Voice Pipeline Wiring Audit — Frontend Buttons, Transcripts Tray, VAD Dictation Guard, Realtime-Passive Ready

Date: 2026-09-03 | Scope: Home voice controls → `pipelineService` → `ipc/pipeline/assistant.rs` → `pipeline/handlers/session.rs` → VAD/CPAL seam → router/dictation | Mode: audit only, no code changed.

User decisions recorded from Q&A (binding for fixes):
- Transcripts tray: **clear on End** (`dialogueHistory=[]` on disengage/end).
- Dictation `enabled=false`: **full mic/VAD halt** in Idle (zero STT).
- Realtime-passive Engage: **wait for backend Ready** (no optimistic local Ready; add Connecting state).
- Report location: `docs/plans/phase11/` (this file).

## 0. One-line button map — are they correctly mapped?

Yes at the **command-name level** (every Home voice `invoke` resolves to a registered `#[tauri::command]`), **no at the behavior/args level** (optimistic state flips, arg-case mismatches, dead tray clear). One line per button:

- Idle Engage (Power, `Home.tsx:346-347`) → `engage()` (`VoiceSessionContext.tsx:147`) → `invoke("start_session")` (`pipelineService.ts:59`) → `assistant.rs:30 start_session` → CORRECT NAME, WRONG BEHAVIOR (sets local `Ready` on mpsc-send, §4).
- Engaged Disengage/End (X, `Home.tsx:320-321`) → `disengage()` (`:166`) → `invoke("end_session")` or `invoke("test_clip_cancel")` if testing (`:178-182`) → `assistant.rs:63 end_session` / `test.rs:18` → CORRECT BRANCHING, MISSING TRAY CLEAR (§2).
- Passive Pause (Pause icon, `Home.tsx:282`) → `pause()` (`:192`) → `invoke("pause_session")` (`:67`) → `assistant.rs:82 pause_session` → CORRECT NAME, SETS `Paused` BEFORE await (§4).
- Passive Resume/Play (`Home.tsx:283`) → `resume()` (`:204`) → `invoke("resume_session")` (`:71`) → `assistant.rs:101 resume_session` → CORRECT NAME, SETS `Ready` BEFORE await (§4).
- Error Reconnect (`Home.tsx:265-266`, banner `Home.tsx:140-144`) → same `resume()` → `invoke("resume_session")` → CORRECT, SAME OPTIMISTIC FLAW (flips Error→Ready even if backend drops it, §4).
- PTT Mic hold (`Home.tsx:299-302`, Orb hold `Home.tsx:220-222`, Space key `VoiceSessionContext.tsx:283-312`) → `handlePttStart/Stop/Cancel` (`:218-246`) → `invoke("ptt_start"/"ptt_stop"/"ptt_cancel")` (`:75-85`) → `assistant.rs:120/139/158` → CORRECT NAMES.
- Test-clip rows (`TestClipsPopover.tsx:40-47`) → `handleTestClip(clipId)` (`:257`, guarded `if(isEngaged) return`) → `invoke("test_clip",{clipId})` (`:87-89`) → `test.rs:9 test_clip(clip_id)` → NAME MATCHES, **ARG KEY MISMATCH** (`clipId` vs `clip_id`, §5).
- Error-banner Configure/Dismiss (`Home.tsx:149-162`) → `navigate("/settings")` / `setErrorAlert(null)` → no IPC → CORRECT (local only).
- Flask test-mode toggle (`Home.tsx:377-390`) → `setTestMode(!testMode)` local → CORRECT (popover gated `testMode && !isEngaged`, `:396`).
- There is **no dedicated "realtime passive" button**. `pipelineMode` (modular|realtime) is settings-only (`VoiceSessionContext.tsx:44,93,326-328,442-444`); Home always calls `start_session` and the backend branches on `ctx.pipeline_mode` (`session.rs:211-214`). The "instant Ready" you saw is §4, not a missing IPC.

## 1. Transcripts tray not cleared on end session — confirmed bug

Store: `transcript` + `assistantText` (actives) + `dialogueHistory: DialogueTurn[]` (`VoiceSessionContext.tsx:111-117`), rendered as `visibleDialogueTurns=dialogueHistory.slice(-10)` + `ActiveTranscript` (`Home.tsx:110-112,205-210`; `ActiveTranscript.tsx:23-27` nulls when both empty).

What clears today:
- `engage():151-154`, `disengage():169-172`, `handleTestClip():263-264` clear **actives + refs only**, never `dialogueHistory`.
- `onStateChanged Idle:373-376` clears actives; `Ready|Idle:378-379` clears `testingClip`; `Listening:381-382` archives turn. `archiveCurrentTurn():126-145` pushes to `dialogueHistory` (cap 100) then clears actives.
- `clearHistory():274-275` (`setDialogueHistory([])`) has **zero callers** in `app/src` — dead code, no UI button wired to it.
- Boot reloads DB turns (`getTurns(conversation_id)`, `:340-351`), so old-session turns reappear after End + re-Engage.

Fix per your decision (clear on End): add `setDialogueHistory([])` + reset `turnIdCounter.current=0` in `disengage()` (and optionally `engage()` as belt-and-braces), wire an explicit Clear control to `clearHistory()` or delete it. Also clear actives on `Error`→`resume` path or stale partials linger until next `Listening` archive.

## 2. VAD ignores dictation-disabled — confirmed backend bug + race

### 2.1 Entrypoint chain (file:line)

```
CPAL mic callback  services/audio/device.rs:127-170  (push_slice, zero-lock sacred path)
  → SPSC ring 64k f32  core/engine.rs:192-193, constants RING_BUFFER_SIZE
  → VAD actor vox-vad-actor  services/vad/actor.rs:448-533 loop
      :482 process_vad_commands  :486 pop 256  :489 telemetry
      :497 should_suppress_audio (speaker-duck only, :232-246) — NO enabled/owner/Idle check
      :501 match operational_mode → StreamPassthrough :433-445 / ContinuousSegmentation :370-402 / WindowedValidation :405-431
  → STT worker  services/stt/actor.rs:253-321 (checks cancel_flag/tid only, :179-219)
  → router  pipeline/router.rs:10-15  (owner==Dictation → dictation::handle_event + return, NO enabled check)
  → dictation.rs:261-274 → on_speech_start Recording :146-152 / on_speech_end Transcribing :155-161 / on_transcript_final :164-216 (route_transcript + emit, NO enabled/Idle checks)
```

Defaults that arm the trap: `owner=Dictation` at boot (`core/state.rs:353`), `DEFAULT_DICTATION_ENABLED=true` (`core/defaults.rs:8`), frontend filters `owner==="Dictation"` state events (`VoiceSessionContext.tsx:365-367`) so dictation turns are invisible on Home while still pasting via tray/output-router.

### 2.2 Guard-gap matrix

| Stage | `dictation.enabled`? | `owner/state`? | Verdict |
|---|---|---|---|
| CPAL `device.rs:129-170` | No | No | Always pushes while `play()`ing |
| Ring `engine.rs:192` | No | No | No gate by design |
| `should_suppress_audio :232-246` | No | Partial (Speaking+Speaker only) | No Idle/Paused/enabled/owner |
| `ContinuousSegmentation :369-402` | No | No | Segments identically in Idle and Ready |
| `WindowedValidation :405-431` | No | `window_active` bool only | Stuck window buffers in Idle |
| `StreamPassthrough :433-445` | No | `realtime_tx.is_some()` only | Forwards in any state until StopRealtime |
| STT `actor.rs:160-219` | No | cancel_flag/tid only | No owner/enabled/state |
| `RoutingContext mod.rs:27-49` | No | owner snapshot only | `enabled=false + Dictation` still yields dictation mode |
| `router.rs:12-15` | **No** | owner==Dictation → unconditional dictation path | Critical gap |
| `dictation.rs` all handlers | **No** | No Idle/Paused gate | Ghost dictation in Idle |
| Assistant `speech.rs:17-29,76-87`, `transcript.rs:142-156`, `ptt.rs:14-26` | n/a | Yes (drop Idle/Paused, PTT≠Passive) | Safe but relies on owner==Assistant |
| `session.rs on_pause :260-293`, `on_end :429-466` | Yes (read) | Conditional handoff | Pause+disabled stops nothing; End+disabled is the single fragile teardown |

Session teardown gaps: `on_pause` and `on_end`-enabled branch never pause CPAL, never clear ring, never send `StopWindowValidation`; `StopRealtime` only when `ctx.pipeline_mode==Realtime` (`session.rs:249-256,378-385`) so Modular `ContinuousSegmentation` stays hot for dictation handoff by design — with no downstream gate. `mutation.rs:54-64` stops the engine on disable **only if Idle**, else leaves it warm; `stop_audio_engine_sync` from the router thread (`session.rs:463` via `engine.rs:394`) is the only Modular-disabled teardown and its `try_lock` failures only `warn`.

### 2.3 Your race, step by step (cold-engine variant A, most likely)

1. `Idle/Dictation`, engine cold. `engage()` → `start_session` (`assistant.rs:42-44`) awaits `start_audio_engine` (0.5–2 s: TenVAD/STT load, ring split, VAD+STT+router spawn, `AudioStream::play`).
2. ~100 ms later `disengage()` → `end_session` (`:67-75`) reads `event_tx` — still `None` → `Err(Engine(Event router not active))`. Frontend sets `Idle` optimistically anyway (`:183`) or toasts; **no `EndSession` was ever queued**.
3. `start_audio_engine` completes → `start_session :52-56` sends `SessionStart{Assistant}` on the new `event_tx`. Router → `on_session_start` (`session.rs:150-159`): Idle passes, owner Dictation→Assistant, `SetOperationalMode(ContinuousSegmentation)` or `StartRealtime`, `transition Ready :223`.
4. Backend is now `Ready/Assistant` with live mic→ring→VAD; UI shows `Idle`. Speak → VAD segments → STT Final → `speech.rs Listening (next_turn)` → `Thinking` → `transcript.rs spawn_modular_llm_task` → local STT text + LLM/TTS. Perceived as "dictation disabled but transcribes via local".
5. Warm-engine mirror: if `EndSession` races ahead of `SessionStart` in the FIFO, End while Idle is a no-op (`session.rs:366-369`), then delayed Start yields the same ghost Ready.
6. Dictation-handoff variant B (when `enabled=true` at `on_end` instant): `on_end :435-461` stores owner Dictation + `SetOperationalMode(dictation_mode)`, keeps engine warm; a later `enabled=false` toggle races with `mutation.rs:57-64` async stop, and any utterance in the window flows through the ungated dictation path (§2.1) and pastes.

### 2.4 Fix — three layers (your CPAL question: yes, as layer 2)

**Layer 1 (primary, VAD loop entry — before audio reaches segmentation, after pop):** extend `should_suppress_audio` or add `should_gate_audio()` at `actor.rs:497`. Gate condition: `(!dictation_enabled && owner==Dictation) || (state==Idle && owner==Dictation)` plus product call on `Idle/Paused + Assistant` (decide: drop or keep for fast-resume). On gate: `continue` without buffering AND clear `utterance_buffer/window_buffer` on transition into gated so no stale `Final` emits. Wiring: add `dictation_enabled: Arc<AtomicBool>` + `owner: Arc<AtomicU32>` to `VadActorHandles` (`:261-267`), populate in `engine.rs:209-215`, update from single writers (`update_setting enabled` at `mutation.rs:695-703`, `on_session_start/on_pause/on_end owner.store` at `session.rs:159,269,438`, engine stop). Cost: two `Relaxed` loads per 16 ms chunk — negligible, no lock/alloc, preserves AGENTS §4.1.3–4.

**Layer 2 (your CPAL→VAD seam request — second defense, drop before transmit):** yes, feasible and desirable, but **never `settings.read()` in the CPAL callback** (sacred path: zero locks, zero alloc, zero blocking I/O, AGENTS §4.1.3). Implementation: clone the same `Arc<AtomicBool> dictation_enabled` (+ optionally `Arc<AtomicBool> mic_muted = Idle&&!enabled`) into `AudioStream::new(producer, device, gate)` → `build_input_stream(..., gate)` (`device.rs:27,112-118`); at the top of the input closure (`:129`) after resample, `if gate.load(Relaxed) { return; }` — i.e. compute-then-drop, or gate before `push_slice` (`:156-169`). Keep the existing overflow `DROP_COUNT` path; add a throttled gated-drop counter for observability. Writers are the same single points as Layer 1. This kills ring growth while gated (Layer 1 alone would still let the ring fill and overflow-log). Do both: Layer 2 stops transmit, Layer 1 stops segmentation of anything already queued + clears stuck windows.

**Layer 3 (defense in depth, non-hot-path):** `router.rs:12` early-drop when `owner==Dictation && !enabled` (covers in-flight STT Final + hotkey path); `dictation.rs:31,64,146,155,164,261` early-return on `!enabled` + `DictationState==Idle` guard in `on_speech_start/end` (mirror `speech.rs:23`); `session.rs on_pause/on_end` always send `StopWindowValidation`-discard + `StopRealtime` based on `realtime_tx.is_some()` truth (not `ctx.pipeline_mode` snapshot), and make `end+!enabled` teardown retry/log-as-error instead of `warn`.

## 3. Realtime-passive "no IPC, instant Ready" — optimistic-state bug, not missing command

Backend `start_session` (`assistant.rs:46-58`) does `event_tx.send(SessionStart)` on a sync std mpsc (µs) then `Ok(())` — it never awaits engine boot, WS handshake, VAD arm, or `transition(Ready)` (async, `session.rs:211-223`). Frontend `engage()` does `await startSession(); setInteractionState("Ready")` (`VoiceSessionContext.tsx:155-157`), so the await resolves in ms and the UI flips before any `state_changed→Ready` arrives (`:362-385`). Failure asymmetry: backend failure later emits `transition(Error)` (`session.rs:216-220`) — UI flashes Ready→Error; `on_session_start` early-return (not Idle, `:150-157`) emits nothing — UI stays Ready while backend is elsewhere; only backend `InvalidState` (`assistant.rs:34-40`) avoids the flip (catch `:158-160`).

Contributors: `pause()` sets `Paused` before `await pauseSession()` (`:192-196`, flips back to `Ready` on throw with no backend confirm); `resume()` sets `Ready` before `await resumeSession()` (`:204-209`) so Error→Reconnect flips instantly even when backend drops it (`session.rs:301-308` warn+return, no emit); `engage` has no double-click guard (unlike `handleTestClip :257-258`), while `isLaunching` only disables the button after first paint — second click hits backend Idle-guards silently while FE sets Ready per resolve. Fix per your decision: introduce `Connecting` (reuse `isLaunching` as state, not just spinner), set `Ready`/`Error` **only** from `onStateChanged`, keep optimistic `Paused` only if you add rollback on timeout, and debounce `engage`.

## 4. IPC/event contract drift (Phase 10 backend moved, frontend didn't)

- **Arg-case (P0, 11 commands — Tauri matches keys exactly, single-word args safe, every multi-word flat arg mismatched):** `test_clip {clipId→clip_id}` (`pipelineService.ts:88` vs `test.rs:9`); `get_turns {sessionId→session_id}` (`historyService.ts:47` vs `history.rs:68`, comment `:45` even documents `session_id`); `trigger_session_compaction {sessionId→session_id}` (`notificationService.ts:35` vs `notifications.rs:67`); `get_memory_fact_detail {factId→fact_id}` (`memoryService.ts:95` vs `memory.rs:49`); `get_provider_caps {providerId→provider_id}` (`settingsService.ts:56` vs `catalog.rs:128`); `probe_model_capabilities {modelId,targetCap→model_id,target_cap}` (3 sites `:116-147` vs `health.rs:36-38`); `setup_remote_server {connectionString,sshPort,identityKeyPath,remotePath,serverPort→snake}` (`pipelineService.ts:154` vs `health.rs:48-53`); `add_voice_from_file {filePath→file_path}` (`:133` vs `voices.rs:86-89`); `add_voice_from_recording {pcmF32,sampleRate→pcm_f32,sample_rate}` (`:137` vs `voices.rs:161-164`); `retry_failed_queue_items {itemIds→item_ids}` (`memoryService.ts:134,141` vs `memory.rs:320`); `toggle_pipeline_processing {paused→enabled?}` with **inverted semantics** (`memoryService.ts:127` vs `memory.rs:290-292 `Some(e)=>!e`` — caller `MemoryPipelineDrawer.tsx:139` `togglePipelineProcessing(false)` sends unknown key → pure toggle). Correct-by-construction counter-examples: `manage_models {payload:{action,model_id,selected_ids}}` and `manage_memory_fact {payload:{...}}` match because inner payload was written snake_case.
- **Orphans:** backend `resolve_memory_conflict` (`memory.rs:77`, registered `lib.rs:606`) has zero frontend callers (frontend has `getUnresolvedConflicts` but no resolve wrapper); no frontend voice `invoke` lacks a backend (60/61 names match; `setup_remote_server` misplaced in `pipelineService` but registered `lib.rs:567`).
- **Event SSOT violation (AGENTS §4.1.2):** `IpcEventMap` (`eventsService.ts:105-117`, 11 keys) missing all 4 `notification_*` (backend `events.rs:134-172`, 15 events); `notificationService.ts:42,51,60,69` uses raw `listen()`; `LiveTestStep.tsx:40,51,62` + `ModelSetupStep`/`AudioSetupStep` bypass `eventsService` with inline payload types (also §4.1.5 service-boundary breach). `model_progress` dual-case `SetupStep` union (`:67-81`) signals backend drift — verify against `setup/model_manager.rs:14`. `llm_token` handler overwrites (`activeAiTextRef.current=payload.token`, `:423`) — confirm backend sends cumulative string or switch to append.
- **Dead/stale frontend:** `hasCachedSession` hard `useState(false)` (`:94`) — resume badge/aria (`Home.tsx:341-344,353`) unreachable; `clearHistory` dead (see §1); `handleEngage/handleEnd/handlePause/handleResume` aliases (`:500-503`) unused on Home; `isSleeping` duplicates `isPaused` (`:99-100`) and collides with banned `is_sleeping` name — rename (e.g. `isDimmed`); `useHomePage.test.ts:16-21` mocks banned `{is_engaged,is_sleeping}` instead of real `RuntimeSnapshot` (`pipelineService.ts:6-45`); benign fallbacks only (`GOVERNOR_LABELS||governor`, `getCSSColor fallback`, static caps) — not bugs.

## 5. Fix list (ordered)

**P0:** VAD Layer-1 gate (`vad/actor.rs:497`, `VadActorHandles :261`, `engine.rs:209`); CPAL Layer-2 drop-gate (`audio/device.rs:27,112,129,156` + shared `Arc<AtomicBool>` writers in `mutation.rs:695`, `session.rs:159,269,438`); router + `dictation.rs` early-drops; Engage/pause/resume → backend-driven states (+`Connecting`); all 11 arg-case fixes (or `#[serde(rename_all="camelCase")]` — prefer frontend→snake to match backend SSOT); tray clear on End (`disengage()` + `Idle` handler).
**P1:** `on_pause/on_end` always `StopWindowValidation`-discard + truth-based `StopRealtime`; `mutation.rs:57` stop on disable regardless of state (or explicit warm-keep with gate asserted); `VadActorConfig` init from dictation mode when `owner==Dictation` at boot (`engine.rs:177-207`); `resolve_memory_conflict` wrapper or unregister; notifications into `IpcEventMap`; double-engage debounce.
**P2:** `hasCachedSession` real wiring or removal; `isSleeping` rename; test mock refresh; `SetupStep` case unification; `llm_token` append-vs-overwrite contract check; `toggle_pipeline_processing` semantics + key alignment.

## 6. How to confirm live

- Engage click with devtools: `start_session` resolves in single-digit ms with no `state_changed` yet while UI already `Ready`; backend `[Pipeline::Session] Session started … mode: Realtime` lands 10–100s of ms later (post `RealtimeActor::start` + WS handshake).
- Start→End≤200 ms on cold engine: `end_session` → `Engine(Event router not active)`, then delayed `SessionStart{Assistant}` → ghost `Ready/Assistant` with UI `Idle`; speak → local STT + LLM proves variant A. With `enabled=true` at End then disable + speak-while-`Idle/Dictation` proving variant B (paste despite disabled).
- `dialogueHistory` survives End + re-Engage; `get_turns`/`test_clip` throw `missing field` on mismatched keys (often swallowed by `try/catch→null`).

## 7. Files touched by this audit (no code changed)

FE: `pages/Home.tsx`, `shared/hooks/useHomePage.ts`, `shared/context/VoiceSessionContext.tsx`, `services/pipelineService.ts` + `historyService.ts` + `notificationService.ts` + `eventsService.ts`, `data/homeCopy.ts`, `shared/components/home/{TestClipsPopover,ActiveTranscript}.tsx`. BE: `ipc/pipeline/{assistant,test}.rs`, `ipc/{history,settings/mutation}.rs`, `pipeline/{router,mod,dictation}.rs`, `pipeline/handlers/session.rs`, `services/{audio/device,mod,vad/actor}.rs`, `core/{engine,events,state,defaults}.rs`, `lib.rs`.
