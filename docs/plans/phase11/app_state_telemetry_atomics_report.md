# App State, Telemetry & Pipeline Atomics — Plain-Language Report

Date: 2026-09-04 | Location: `docs/plans/phase11/` | Mode: audit only, no code changed.
Scope: backend `core/state.rs` + `monitoring/*` + `pipeline/*` and frontend `store/*`, `shared/context/VoiceSessionContext.tsx`, `shared/hooks/useTelemetry|useMonitoringMetrics|useInteraction`, `services/eventsService|pipelineService`.

> How to read this: each state gets one plain-English line ("what it means for the user"), then "logic" (who writes it, who reads it, what happens next). File:line pointers are included so you can jump straight to source.

---

## 0. TL;DR — the three buckets

| Bucket | In plain English | Lives in | Drives behavior? |
|---|---|---|---|
| **App / pipeline state** | "What is the app doing right now?" (Idle, Listening, Speaking…). The boss. | `PipelineAtomics` + `AppState` (backend), `VoiceSessionContext.interactionState` (frontend mirror) | **Yes — everything branches on this.** |
| **Pipeline atomics** | The thread-safe control panel: turn counter, cancel button, state broadcast channel. | `core/state.rs:111-245 PipelineAtomics` | **Yes — control plane.** |
| **Telemetry** | "How healthy / loud / fast was that?" Numbers only. Never tells the app what to do. | `TelemetryState` (`state.rs:307-332`) + `monitoring/aggregator.rs` + `monitoring/collector.rs` + frontend `useTelemetry` ref | **No — observation plane only.** |

Golden rule: **state decides, telemetry observes.** If telemetry stops, the voice pipeline still works (you just go blind). If pipeline atomics stop, nothing works.

---

## 1. Every app state, in simple language

### 1.1 `InteractionState` — the main voice state (backend SSOT, frontend mirror)

Defined `core/state.rs:39-47`. Mirrored frontend `services/eventsService.ts:7-14` as `"Idle"|"Ready"|"Listening"|"Thinking"|"Speaking"|"Paused"|"Error"`.

| Value | Simple meaning | Logic (who sets / who reacts) |
|---|---|---|
| `Idle` (0) | "Microphone off, nothing running." Home shows power button. | Boot default (`PipelineAtomics::new`, `state.rs:136`). Set by `end_session` → `transition(Idle)` (`pipeline/assistant/session.rs:468`), engine stop (`core/engine.rs:324`). Router ignores speech/PTT in Idle (`assistant/speech.rs:23`, `assistant/ptt.rs:20`). Idle-monitor thread skips Idle (`pipeline/mod.rs:128-133`). Frontend: `isEngaged = state !== "Idle"` (`VoiceSessionContext.tsx:98`); Idle clears actives (`:373-376`). |
| `Ready` (1) | "Microphone on, waiting for you to speak." | Set by `start_session` (`session.rs:223`), after playback/LLM done (`handlers/playback.rs:61`, `handlers/transcript.rs:169`, `handlers/error.rs:88`). Guards: `facade.rs:283` only allows engage from Ready/Paused; VAD tests seed Ready (`vad/actor.rs:553+`). Frontend: orb idle-pulse, PTT armed. Auto-pause after 7 min Ready (`pipeline/mod.rs:135-152`). |
| `Listening` (2) | "I hear you, keep talking." Recording your voice. | Set on speech-start / PTT-press (`handlers/speech.rs:45`, `handlers/ptt.rs:48`, `handlers/interrupt.rs:82` for barge-in). While Listening, VAD segments audio, STT streams partials. Exit on speech-end → Thinking (`speech.rs:89`) or PTT-release. Frontend `pttStatus="RECORDING"` when PTT+Listening (`VoiceSessionContext.tsx:102-109`); `Listening` archives previous turn (`:381-382`). Collector maps to string (`collector.rs:50`). |
| `Thinking` (3) | "Got it, let me think / ask the AI." | Set on speech-end / PTT-stop (`speech.rs:89`, `ptt.rs:72`). LLM generates; `handlers/llm.rs:74` rejects tokens unless Thinking/Speaking. `playback.rs:15` only starts Speaking from Thinking. Frontend `isThinking=true`, shows spinner/shimmer (`VoiceSessionContext.tsx:101`). |
| `Speaking` (4) | "Playing the answer out loud." | Set by `on_playback_started` (`handlers/playback.rs:24`). While Speaking, VAD speaker-duck suppresses echo (`vad/actor.rs:244`, `audio/sink.rs:67`). `telemetry_emitter.rs:46` samples playback energy only in Speaking. Ends → Ready (`playback.rs:61`). Frontend plays orb wave, `playback_active=true` in snapshot (`collector.rs:119`). |
| `Paused` (5) | "Sleeping to save RAM/battery. Tap to wake." | Set by `pause_session` (`session.rs:295`). Resume → Ready (`:359`). Router treats Paused like Idle for speech (`speech.rs:23`). Realtime connection loop skips sends while Paused (`realtime/transport/connection.rs:224,239`). Telemetry emitter drops audio frames while Paused (`telemetry_emitter.rs:17`). After 5 min Paused, idle-monitor offloads LLM/TTS (`pipeline/mod.rs:154-176`). Frontend `isPaused/isSleeping=true`. |
| `Error` (6) | "Something broke, tap retry." | Set on engine/LLM/STT failure (`session.rs:218,340`, `handlers/error.rs:37`). Frontend shows banner (`VoiceSessionContext.tsx:390-393` via `voice_error`), `pause()` refuses from Error but `resume()` allows Error→Ready (`:196,207`). Resume clears banner (`:210`). |

**Broadcast plumbing (important):** backend never sets a bare variable. `set_state()` writes `current_state_atomic: AtomicU32` **and** `state_tx.send()` on a `tokio::watch` channel (`state.rs:171-180`). Backend workers subscribe (`subscribe_state()`), frontend gets one `state_changed` IPC per transition (`pipeline/mod.rs:61-96`, payload `{owner, state, turn_id}`). Frontend is 100% event-driven — no optimistic flips on engage/disengage/pause/resume (per tray-clear sprint).

### 1.2 `DictationState` — the tray/mini-window state (separate track)

Defined `core/state.rs:71-76`. Values: `Idle (0)` = "tray idle", `Recording (1)` = "tray is recording keys", `Transcribing (2)` = "converting speech to text", `Error (3)` = "dictation failed".

- Why separate? Assistant (main window) and Dictation (tray) can exist side by side. `InteractionOwner` picks which track an event belongs to. Frontend Home **ignores** `owner==="Dictation"` state events (`VoiceSessionContext.tsx:365-367`) — dictation turns never touch the main dialogue tray; they go to paste/clipboard/tray via `dictation/output_router.rs`.
- Logic: `pipeline/dictation.rs:35-257` — speech-start → Recording (`:56-57`), speech-end → Transcribing (`:117-118`), final → Idle (`:202-203`), failure → Error (`:226-227`). Display quirk: Transcribing is shown as "Thinking" in tray UI (`dictation.rs:16`).
- Atomics mirror the assistant track: `dictation_state_atomic: AtomicU32` + `dictation_state_tx/rx: watch<DictationState>` (`state.rs:120-122,187-207`).

### 1.3 `InteractionOwner` — "which window owns this turn?"

`core/state.rs:10-13`: `Dictation (0)` = tray/mini path, `Assistant (1)` = main window path. Stored as `AppState.owner: AtomicU32` (`:270`). Read once per turn into `RoutingContext { pipeline_mode, interaction_mode, owner }` (`pipeline/mod.rs:25-49`) which then picks target window (`target_window()`: Dictation→`WINDOW_TRAY`, Assistant→`WINDOW_MAIN`, `:53-58`) and which settings apply (dictation PTT mode vs assistant mode, `:30-42`). Unknown u32 falls back to Dictation (`state.rs:15-22`) — safe default. Frontend type `InteractionOwner = "Assistant"\|"Dictation"` (`eventsService.ts:17`), carried on `StateChangedPayload`.

### 1.4 `RuntimeStatus` — "did the app itself boot OK?"

`core/state.rs:31-35`: `Initializing` → `Ready` → `Error`. This is **about the process, not the voice turn**. Stored `AppState.runtime_status: AtomicU32` (`:281`), init `Initializing` (`:364`). Frontend boot/wizard gates on it (show window, run system check). Do not confuse with `InteractionState.Ready` (mic ready) — you can have Runtime=Ready but Interaction=Idle (healthy app, mic off).

### 1.5 `MemoryAppState` — "should we remember this?"

`core/state.rs:247-265`: two atomics only.
- `graph_version: AtomicU64` (starts 1) — bumps on every memory write; retrieval caches key off it.
- `user_paused_ingestion: AtomicBool` — user toggle "stop learning for now". Ingestion runner + `MemoryPipelineDrawer` read it; when true, stage pipelines skip commit but retrieval still works.

### 1.6 `AppState` — the big bag holding everything (backend)

`core/state.rs:267-303`. Think of it as rooms in one house:

- **Engines (the workers):** `engine: Mutex<Option<VoxEngine>>` (mic ring + STT/VAD/LLM/TTS thread handles, `:95-109`), `realtime_engine` (Gemini/Deepgram live session), `llm_provider` (current LLM handle), `conversation_manager` (RAM chat history), `pipeline_accumulator` (per-turn transcript+token collector), `event_tx` (router inbox `VoxEvent`).
- **Control (the switches):** `owner`, `pipeline: PipelineAtomics` (§2), `conversation_id: AtomicU64` (DB session id, 0 = tray/no-save), `runtime_status`, `hud_visible: AtomicBool` (tray HUD checkbox), `main_window_destroyed: AtomicBool`, `setup_running: Mutex<bool>` (wizard lock), `save_debounce` (coalesces settings writes).
- **Observation (the meters):** `telemetry: Arc<TelemetryState>` (§3), `monitoring: MonitoringState` (10 Hz snapshot ring, §3.4), `dropped_persistence_events: AtomicU64`, `persist_tx/memory_tx` (DB worker queues; depth read for health).
- **Config/models:** `settings: Arc<RwLock<VoxSettings>>` (§4 — the thing everyone reads), `model_manager` + `manifest` (download state), `cpu_governor: Mutex<String>` + `cpu_governor_optimal: AtomicBool` (Linux powersave warning), `dictation_last_transcript`, `hud_menu_item`, `_log_guard`.

### 1.7 Frontend states (what the user actually sees)

**`VoiceSessionContext` (`shared/context/VoiceSessionContext.tsx`) — the screen's copy of the backend.**
- `interactionState: InteractionState` (`:91`, init `"Idle"`) — **sole truth for Home UI**. Set only from `getRuntimeSnapshot()` boot sync (`:331-335`) and `onStateChanged` (`:363-385`). Never set optimistically.
- Derived (not separate sources — computed with `useMemo`, no extra sync): `isEngaged = state!=="Idle"` (`:98`), `isSleeping/isPaused = state==="Paused"` (`:99-100`), `isThinking = state==="Thinking"` (`:101`), `pttStatus` = IDLE/RECORDING/PROCESSING from mode+state (`:102-109`).
- `interactionMode: "PASSIVE"|"PTT"` (`:92`) + `pipelineMode: "modular"|"realtime"` (`:93`) — hydrated from settings at boot (`:323-328`) and on `settings-updated` (`:434-449`).
- Conversation UI: `transcript` (live user partial), `assistantText` (live AI stream, 30 ms throttled `:399-430`), `dialogueHistory: DialogueTurn[]` (archived turns, cap 100, `:117,126-145`), `turnIdCounter` ref (local display ids — **not** backend `turn_id`).
- Flow flags: `isLaunching` (engage/disengage spinner), `testMode/testingClip` (golden-clip test), `errorAlert` (banner text from `voice_error`), `cpuWarning` (governor banner from snapshot), `hasCachedSession` (currently always false).

**`settingsStore` (`store/settingsStore.ts:390-725`, zustand) — the settings editor.**
`settings` (last-saved from backend) vs `draftSettings` (unsaved edits) — the Save bar compares them. Plus `modelCatalog`, `capabilitiesCache`, `isLoading/hasChanges/restartKeys/isCommitting/autoSavedDomain`. `updateDraft()` auto-saves Hot/WorkerCommand keys after 600 ms, leaves Restart keys dirty (`:454-537`). Appearance applies instantly to `<html>` + localStorage (`:360-385`).

**`notificationStore` (`store/notificationStore.ts`) — the bell.**
`notifications: NotificationRecord[]`, `compactingSessionIds` (spinner per session), `loading`, `isOpen`. Listens `notification_created/updated/dismissed/marked_read` (`:90-160`).

**Wizard `setupMachine` (`wizard/state/setupMachine.ts`, xstate) — first-run installer.**
States `welcome → checking → downloading → audio → testing → completed` (+ `error`), context `{currentStep, models{}, totalProgress, manifestReady, setupComplete, maxReachedIndex}`. Driven by `model_progress` IPC (`ModelProgressPayload`), not by voice state.

**Small hooks:**
- `useTelemetry` (`shared/hooks/useTelemetry.ts`) — subscribes `telemetry` IPC into a **ref** (no re-render; consumed by `requestAnimationFrame` visualizers like orb).
- `useMonitoringMetrics` (`shared/hooks/useMonitoringMetrics.ts`) — polls `get_runtime_snapshot()` 1 Hz while Monitoring visible, keeps last 60 samples for charts.
- `useInteraction` (`shared/hooks/useInteraction.ts`) — legacy local partial/commit buffer (4000-char cap); largely superseded by context `transcript/dialogueHistory`.
- `SettingsContext` (boot loader: `loadSettings+loadModelCatalog`, live `settings-updated` reload), `MemoryProfilerContext`, `useSettingsPage/useSettings` (dirty-check helpers).

**Event + snapshot types (the contracts):**
Internal `VoxEvent` (`core/events.rs:11-45`: SessionStart/Pause/Resume/End, PttStart/Stop/Cancel, SpeechStart/End, TranscriptFinal, LlmFinished, PlaybackStarted/Finished, Cancelled, Error, Shutdown) → router → `transition()` → outward `IpcEvent` (`:133-150`: `state_changed, transcript_partial/final, llm_token, voice_error, model_progress, telemetry, system_stats, settings-updated, toggle_tray, show_toast, notification_*`). Frontend mirror `IpcEventMap` (`eventsService.ts:109-125`). `RuntimeSnapshot` (backend `monitoring/snapshot.rs:6-79` ≈ frontend `pipelineService.ts:6-45`) is the 1 Hz "everything" struct Monitoring UI renders.

---

## 2. Pipeline atomics — the control panel

Source: `core/state.rs:111-245`.

| Field | Plain English | Logic |
|---|---|---|
| `current_state_atomic: AtomicU32` + `state_tx/rx: watch<InteractionState>` | The real `InteractionState` number + a loudspeaker that shouts every change. | `state()` reads atomic (`:166-168`); `set_state()` writes atomic **then** broadcasts (`:171-180`); workers `subscribe_state()` (`:183-185`) e.g. idle-monitor (`pipeline/mod.rs:125`), persistence gate (`persistence/memory_worker.rs:87-97`), realtime pause gate (`realtime/transport/connection.rs:224-239`). |
| `dictation_state_atomic` + `dictation_state_tx/rx` | Same, but for the tray track. | `dictation_state()` / `set_dictation_state()` / `subscribe_dictation_state()` (`:187-207`). |
| `turn_id: AtomicU32` | Ticket number for "this back-and-forth". Starts 0, only goes up. | `next_turn_id()` = fetch_add+1 (`:225-227`); `peek_turn_id()` reads without bumping (`:230-232`). Every `state_changed` payload + transcript/token payload carries it so frontend can match events to turns. Never reset (frontend's local `turnIdCounter` reset on disengage is display-only). |
| `turn_token: Mutex<CancellationToken>` + `turn_epoch: AtomicU64` | The "cancel this answer" button + how many times it was pressed. | `turn_token()` clones current token (`:210-212`); `renew_turn_token()` cancels old, bumps epoch, makes fresh (`:215-222`); `cancel_current_turn()` cancels without new turn (`:242-244`). **Only** `next_turn()` (`:235-239`) may bump id **and** renew token together — invariant §4.1.7. STT/LLM/TTS workers select on this token to abort stale turns (barge-in). |
| `cancel_flag: AtomicBool` | Emergency stop for STT worker loop. | Checked in STT actor hot loop; set on cancel/end. Coarse vs per-turn token (flag = global stop, token = this-turn stop). |
| `transcript_history: Mutex<VecDeque<String>>` | Last N things the user said (RAM only, capped `TRANSCRIPT_HISTORY_LIMIT`). | Used for companion/duplicate suppression; not the DB history. |
| `playback_underruns: AtomicU64` | How many times audio ran dry (glitch counter). | Bumped by playback engine; surfaced in snapshot for health. |
| `pending_synthesis_jobs: AtomicU32` | How many TTS jobs are queued. | Backpressure: skip/interrupt if piling up. |
| `engine_shutdown: AtomicBool` | "App is quitting, everyone stop." | Checked by long loops to exit cleanly. |

---

## 3. Telemetry — the meters

### 3.1 `TelemetryState` — the latest-numbers board (`state.rs:307-332`)

One `Arc<TelemetryState>` shared by all threads. All float metrics stored as `AtomicU32` bits (lock-free), counters as `AtomicU64`, flags as `AtomicBool`:

- Audio (from VAD hot path): `latest_energy, latest_vad_prob, latest_low/mid/high` (frequency-band energies for visualizer).
- Playback: `latest_playback_energy/low/mid/high`.
- System: `latest_sys_cpu, latest_sys_ram, latest_vox_cpu, latest_vox_ram, latest_threads`.
- Latency: `latest_stt_ms, latest_ttft_ms, latest_voice_latency_ms, latest_tts_rtf, latest_playback_start_ms, latest_persistence_rate`.
- Health/flags: `is_db_healthy, is_private_mode` (mirrors `history.private_mode` at boot `:341-343`), `dropped_telemetry_events` (saturation counter), `telemetry_tx` (inbox sender).

### 3.2 `TelemetryEvent` + aggregator — how numbers get on the board

`monitoring/aggregator.rs:10-26`: only two event kinds — `SystemHealth{system_cpu, system_ram_pct, vox_cpu, vox_ram_mb}` and `AudioEnergy{energy, vad_prob, low, mid, high}`. `TelemetryAggregator` runs on its own `vox-telemetry` OS thread with a **bounded** channel (`:80-92`); `handle_event()` just `store(bits, Relaxed)` into the board (`:94-148`). If producers outrun it, events drop and `dropped_events` increments (snapshot surfaces it — lossy by design, latest-wins).

### 3.3 Live IPC: `telemetry` vs `system_stats` vs snapshot

- `telemetry: TelemetryData{energy, vad_prob, low, mid, high}` (`core/events.rs:75-81`) — high-frequency (~VAD frame rate), consumed frontend into a **ref** (`useTelemetry.ts:12-24`, no re-render). Drives orb/waveform only. Paused-gated backend (`telemetry_emitter.rs:17`), Speaking-gated playback fields (`:46`).
- `system_stats: SystemStatsPayload` (`events.rs:64-72`) — CPU/RAM/thread counts for footer badges.
- `RuntimeSnapshot` 1 Hz (`monitoring/snapshot.rs`, `collector.rs:94-224`) — the "everything" struct: pipeline_state string, turn/conversation ids, playback_active, CPU/RAM, VAD energy/prob, STT/TTFT/voice latency, persistence queue depth + dropped, playback buffer + underruns, active_owner, threads, TTS RTF, DB health, per-model `is_*_loaded` flags, cpu_governor, webview RAM, timestamp. Built by `vox-monitor` thread every 100 ms (`collector.rs:10-44`, `COLLECTOR_TICK_INTERVAL`), stored in `MonitoringState` ring (`runtime_state.rs:7-52`, cap `MAX_SNAPSHOT_HISTORY`), served via `get_runtime_snapshot` IPC → `useMonitoringMetrics` charts + `Monitoring.tsx`.

### 3.4 Telemetry vs pipeline atomics — how they differ, where the boundary is

| | Pipeline atomics | Telemetry |
|---|---|---|
| Question answered | "What should we do next?" | "What just happened / how fast?" |
| Examples | state=Speaking, turn_id=42, cancel token | vad_prob=0.93, stt_ms=410, vox_ram=812 MB |
| Write path | `transition()` / `next_turn()` / token renew — exactly-once, ordered, never dropped | fire-and-forget channel send; drops OK, latest-wins |
| Read path | `state()` / `subscribe_state()` / `peek_turn_id()` — every reader sees every transition | `load(Relaxed)` latest value — readers see only newest, history only via snapshot ring |
| Ordering | Strict (Idle→Ready→Listening→Thinking→Speaking→Ready). Skipping breaks logic. | None. Two telemetry events can swap with zero harm. |
| Loss allowed? | **No.** A missed `state_changed` = stuck UI (hence watch channel + IPC retry). | **Yes.** A missed audio-energy frame = one frozen orb frame. |
| Drives UI actions? | Yes (buttons enable/disable, navigation, PTT arming). | No (bars, waves, numbers only). |
| Threading | SeqCst atomics + watch broadcast (control correctness first). | Relaxed atomics + bounded channel (speed first, hot-path safe — no locks, no alloc, invariant §4.1.3). |
| Persists? | turn_id/conversation_id feed DB writes; state itself is RAM-only. | Snapshot history is RAM ring only; DB health counters only. |

**Boundary rules (do not cross):**
1. No pipeline transition may read a telemetry value as its condition (e.g. never `if vad_prob>0.9 → set_state(Listening)` outside VAD actor's own threshold path; and never `if cpu_high → Pause`). Transitions come from `VoxEvent`s + settings + tokens.
2. No telemetry write may change control state (aggregator only `store()`s numbers; it has no handle to `PipelineAtomics::set_state`).
3. The only legal join is the **collector**: it reads `pipeline.current_state_atomic + turn_id + owner` **and** `telemetry.latest_*` to compose a read-only `RuntimeSnapshot`. Snapshot consumers must treat it as stale-on-arrival display data, never feed it back into `transition()`.
4. Frontend mirror: `interactionState` (control, event-driven, re-renders) vs `telemetryRef` (observation, ref-only, never re-renders) vs `monitoring history` (observation, 1 Hz, charts). Do not derive button enablement from telemetry/snapshot.

---

## 4. Settings reads — how many places read settings directly today

> "Directly" = touches the backend `Arc<RwLock<VoxSettings>>` or calls `VoxSettings::load()`, or frontend reads `useSettingsStore(s => s.settings/draftSettings…)` / `getSettings()` outside the store. Centralized helpers (`RoutingContext::from_app_state`, `get_llm_provider_kind`) still count at their call site — listed below.

### 4.1 Backend: 15 `state.settings` touchpoints across 12 files + 2 boot loads

| # | File:line | Read / write | What it reads & why (one line) |
|---|---|---|---|
| 1 | `tray.rs:98` | read | `dictation.enabled` + HUD visibility to build tray menu (checked/enabled flags). |
| 2 | `services/llm/probe.rs:78` | read | Full `VoxSettings` clone to build probe provider config (caps test). |
| 3 | `services/llm/probe.rs:114` | read | Same — second probe path (server vs embedded). |
| 4 | `services/health.rs:36` | read | LLM health check needs `llm.*` (active provider, URL, model). |
| 5 | `services/health.rs:96` | read | STT health check needs `stt.*`. |
| 6 | `services/health.rs:131` | read | TTS health check needs `tts.*`. |
| 7 | `services/harness/facade.rs:326` | read (fallback) | `s.llm.clone()` as fallback when caller passes no explicit LLM settings. |
| 8 | `services/memory/compaction/coordinator.rs:113` | read (clone) | `history.auto_compaction` + `memory.*` guard: only compact when Idle/Paused **and** allowed. |
| 9 | `ipc/tray.rs:13` | read | `setup_completed, dictation.enabled, owner` to decide tray click behavior. |
| 10 | `monitoring/collector.rs:71` | read | `llm.active + server/cloud.provider_name` to label snapshot `llm_provider_kind`. |
| 11 | `ipc/memory.rs:304` | **write** | `settings.write()` persists `pipeline_processing_enabled` toggle (memory pause). Only write outside settings IPC. |
| 12 | `pipeline/mod.rs:28` | read | `RoutingContext::from_app_state`: pipeline_mode + interaction/dictation mode + owner → routes every turn. Hottest read. |
| 13 | `pipeline/assistant/session.rs:199` | read | `start_session` branch: pipeline_mode + interaction mode decide modular vs realtime boot. |
| 14 | `lib.rs:533` | read | Boot/engage gate: `dictation.enabled` when Idle (mic-halt rule). |
| 15 | `pipeline/test.rs:125` | read (test only) | Same routing-context snapshot for pipeline unit tests. |
| — | `core/state.rs:340` | `VoxSettings::load()` | Boot: load settings.json (or defaults) into `AppState.settings`; seeds `is_private_mode`. |
| — | `setup/runtime_check.rs:44` | `VoxSettings::load()` | Pre-flight check reads disk settings without an `AppState` handle. |

**Total backend: 14 production touchpoints + 1 test-only + 2 boot loads = 17 lines that can see settings.** 11 of the 14 production reads are `read()` (shared lock, short scope); 1 is `write()` (`ipc/memory.rs:304`); 2 are full-file loads.

### 4.2 Frontend: 134 `useSettingsStore` lines across 30 files + 5 direct `getSettings()` calls

- `rg "useSettingsStore" app/src` = **134 matching lines** in **30 files** (list): `MemoryPipelineDrawer.tsx`, `SettingsContext.tsx`, `WizardRoot.tsx`, `useSettingsPage.ts`, `settingsStore.ts` (self), `RealtimeConfigDesk.tsx`, `LlmConfigDesk.tsx`, `DictationConfigDesk.tsx`, `MemoryCard.tsx`, `PipelineModeCard.tsx`, `MemoryConfigDesk.tsx`, `InteractionCard.tsx`, `Monitoring.tsx`, `Settings.tsx`, `CategorySelector.tsx`, `TriggerModeCard.tsx`, `HistoryCard.tsx`, `RealtimeCard.tsx`, `PersonaCard.tsx`, `AppearanceCard.tsx`, `LlmCatalogView.tsx`, `ModelsTopologyMap.tsx`, `AsrWorkspace.tsx`, `TtsVoiceManager.tsx`, `TtsModelWorkspace.tsx`, `VadWorkspace.tsx`, `ModelsCard.tsx`, `LlmSettingsView.tsx`, `AuxiliaryWorkspace.tsx`, + `InteractionCard`-internal helpers. Vast majority are `s.draftSettings?.<domain>` (edit form) or `s.settings?.<domain>` (saved badge); ~35 distinct field selectors (e.g. `Monitoring.tsx:52-56` reads `appearance.accent_seed/theme + llm/stt/tts.active` for badges; `Settings.tsx:31,140-158,289-307` dirty/restart-key compare; `VoiceSessionContext` does **not** use the store — it calls the service directly, next bullet).
- Direct `getSettings()` service calls (bypass store): **5 sites** — `VoiceSessionContext.tsx:321` (boot hydrate interaction/pipeline mode), `:437` (`settings-updated` re-hydrate), `settingsStore.ts:402` (`loadSettings`), `:682` (`commitChanges` refresh), `wizard/steps/AudioSetupStep.tsx:33` (mic list boot). Tests excluded (`settingsService.test.ts:34,52`).
- So: **any UI that shows a model name, voice, threshold, prompt, or theme reads settings** — which is correct (settings is the config SSOT), but today there is no selector façade: components reach deep (`s.draftSettings?.llm?.server`, `settings.vad.vad_backend`, …). If a key is renamed, ~30 files feel it. Suggested follow-up (not done here): add typed selectors (`selectLlmActive`, `selectVadBackend`, …) in `settingsStore.ts` and migrate the ~35 field reads; keep backend `RoutingContext` as the single backend reader for hot paths.

### 4.3 Grand total

| Layer | Direct settings touchpoints |
|---|---|
| Backend prod `read/write` | **14** (12 files) |
| Backend test-only | 1 |
| Backend boot `load()` | 2 |
| Frontend store selectors | **134 lines / 30 files** (~35 distinct field paths) |
| Frontend direct `getSettings()` | **5** (3 prod UI + 2 store-internal) |
| **All-in** | **~156 lines** that can see settings today |

---

## 5. Quick lookup — "where does X live?"

- "What turn are we on?" → `PipelineAtomics.turn_id` (backend) → `state_changed.turn_id` → frontend `LocalSnapshot.current_turn_id` / `TranscriptPayload.turn_id`. Frontend `turnIdCounter` is display-only.
- "Can I cancel this answer?" → `PipelineAtomics.turn_token` / `cancel_current_turn()`; frontend just calls `ptt_cancel`/`end_session`, backend cancels token.
- "Is the mic hot?" → `InteractionState` (Listening = hot). Never infer from `vad_prob` or orb height.
- "Why is the orb moving but buttons disabled?" → orb = `telemetry` ref (alive), buttons = `interactionState` (stuck?) — check `state_changed` listener, not telemetry.
- "Which mic/model/voice/prompt is active?" → settings (`RoutingContext` backend, `settingsStore` frontend). State tells you *phase*, settings tell you *configuration*, telemetry tells you *signal*.
- "Is the DB sick?" → `telemetry.is_db_healthy` → snapshot `is_db_healthy` → Monitoring badge. Private mode → `history.private_mode` → `telemetry.is_private_mode` (no transcripts persisted).

---

## 6. Files touched for this report

- Created: `docs/plans/phase11/app_state_telemetry_atomics_report.md` (this file). No code changed.
