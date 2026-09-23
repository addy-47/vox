# Target Event-Domain Architectural Specification (Ground Truth)

---

## 1. Executive Summary & Core Architectural Invariants

This document establishes the **target behavioral, concurrency, and resilience specification** for the `pipeline/` refactor across all 6 interaction domains:
- **Assistant Domains:** **Modular Passive**, **Modular PTT**, **Realtime Passive**, **Realtime PTT** (housed under `pipeline/assistant/`)
- **Dictation Domains:** **Dictation Passive**, **Dictation PTT** (housed under `pipeline/dictation/`)

### Architectural Invariants

1. **Single-Writer Serialization Point:**
   All state-mutating commands (user IPC commands `session_start`, `pause_session`, `resume_session`, `end_session`, `ptt_start`, `ptt_stop`, `ptt_cancel`, and `idle_monitor` timeouts) push strongly-typed events onto the central FIFO `mpsc::Sender<VoxEvent>` queue. The central Router OS thread is the **sole writer / sole serialization point** for `InteractionState` mutations and turn lifecycles, guaranteeing strict FIFO ordering and eliminating multi-threaded transition races.

2. **Decoupled UI Streaming Streams:**
   High-frequency streaming text tokens (`LlmToken`), live interim subtitles (`TranscriptPartial`), and turn milestone telemetry (`TurnMetrics`) carry **zero FSM transition responsibility**. They bypass the central Router queue and are emitted directly to the frontend webview by worker actors (`SttActor`, `LlmActor`, `RealtimeActor`, `TurnMetricsCollector`) via Tauri IPC, keeping the central queue uncongested.

3. **Immediate Speech-End Responsiveness (`SpeechEnd` / `PttStop` $\to$ `Thinking`):**
   When user speech concludes (VAD silence threshold reached in passive mode, or PTT key released in PTT mode), the pipeline transitions `Listening` $\to$ `Thinking` immediately. STT decodes in the background and emits `TranscriptFinal`, which acts as the validation gate: if speech is recognized, LLM generation proceeds; if text is empty, the pipeline recovers `Thinking` $\to$ `Ready` and displays an Info Toast (*"No speech recognized"*).

4. **CPAL Warm Pause vs. Engine Teardown:**
   - **`PauseSession`:** Mutes and drains output audio, suspends microphone passthrough, cancels active tokens, and transitions Assistant to `Paused`. **CPAL hardware audio streams remain RUNNING AND WARM**, guaranteeing sub-10ms resumption latency without audio hardware re-negotiation.
   - **`EndSession`:** Cancels active tokens, clears accumulators, records session metrics, and transitions Assistant to `Idle`. If Dictation track is also `Idle`, completely tears down CPAL audio engine and releases microphone hardware back to the OS.

5. **Unconditional Dictation Owner Handover on Pause/End:**
   When `PauseSession` or `EndSession` executes, `state.owner` is **unconditionally flipped** to `InteractionOwner::Dictation`:
   ```rust
   state.owner.store(InteractionOwner::Dictation as u32, Ordering::Relaxed);
   ```
   There are zero settings reads (`settings.dictation.enabled`) during session transitions. Dictation's own track state (`Ready` vs `Idle`) autonomously governs whether it reacts to hotkeys/audio. When `ResumeSession` or `SessionStart` executes, `state.owner` is restored to `InteractionOwner::Assistant`.

6. **In-Flight Synthesis Guard Disciplinary Distinction:**
   - **Modular Mode:** `pending_synthesis_jobs` is a discrete counter incremented per clause and decremented naturally by `TtsActor`. `LlmFinished` flushes the chunker remainder and does NOT force the counter to 0.
   - **Realtime Mode:** There is no local `TtsActor`. `pending_synthesis_jobs` acts as a streaming guard (held at 1 from turn start). When the server finishes streaming all audio for the turn, `LlmFinished` resets `pending_synthesis_jobs = 0`. The remaining audio in the ring buffer plays out until `consumer.is_empty()`, which then cleanly fires `PlaybackFinished`.

7. **Generic Non-Terminal Phase Contract & `Working` State Gate:**
   When an intermediate, non-terminal operation is initiated by the cognitive layer (such as inline context compaction, episodic memory retrieval, or agentic tool execution), the pipeline transitions `Thinking` $\to$ `Working`. The Harness yields an interim filler phrase (e.g. *"One moment while I organize our conversation..."*), tagged explicitly as `AudioIntent::InterimFiller`, and dispatches it to TTS. The Playback Engine recognises `AudioIntent::InterimFiller` and **does not** transition the state to `Speaking` or `Ready` upon playback completion, keeping the assistant locked in `Working` while the intermediate operation executes. When the finalized conversational turn response begins playback (`AudioIntent::TurnResponse`), it transitions `Working` $\to$ `Speaking`, and finally `Speaking` $\to$ `Ready` on completion. If the non-terminal phase errors, is cancelled, or produces no audio, the pipeline recovers cleanly `Working` $\to$ `Ready`.

8. **Resilient 2D Error Handling & Routing Protocol:**
   `VoxEvent::Error { turn_id, message, source, impact, actionability }` decouples execution from user transparency:
   - **`impact: PipelineImpact::Degraded`**: No state change. Turn completes with degraded fidelity (e.g. identity facts capped to 15%, critical compaction falls back to FIFO, speech synthesis drops while text streams).
   - **`impact: PipelineImpact::TurnAborted`**: The active turn fails cleanly. State transitions **directly to `Ready`** (never locking the assistant in `Error`). Audio playback and turn tokens are cancelled.
   - **`impact: PipelineImpact::SessionHalted`**: The session cannot continue (bad API keys, missing models, audio device unplugged). State transitions to `InteractionState::Error`.
   - **`actionability: Actionability`**: Transient hiccups (`NonActionable`) trigger an ephemeral UI Toast only; persistent or configuration-related issues (`Actionable { category, hint }`) trigger both an ephemeral UI Toast AND a persistent notification in the Notifications drawer (`IpcEvent::NotificationCreated`).

9. **Lock-Free Audio Ingestion Gate Invariant (`ingestion_gate`):**
   Audio ingestion from CPAL into the ring buffer is governed strictly by a single derived `AtomicBool` (`ingestion_gate`) recomputed upon every transition in either track. Zero settings reads on the audio hot path:
   $$\text{Gate Is Open} \iff (\text{assistant\_state} \in \{\text{Ready, Listening, Thinking, Speaking, Working}\}) \lor (\text{dictation\_state} \in \{\text{Ready, Listening, Thinking}\})$$
   - When Gate is CLOSED: CPAL callback drops mono frames immediately before `producer.push_slice()`. The ring buffer stays completely empty, preventing buffer overflows and eliminating VAD CPU consumption.
   - On gate closure: `VadActor` clears internal pre-roll and window buffers, preventing stale audio leak on resume.

10. **Canonical Deep Sleep State (`Sleeping`):**
    `InteractionState::Sleeping` (variant 7) represents cold standby after 5 minutes of sustained `Paused`. The idle monitor offloads local LLM/TTS weights, trims the heap, and transitions `Paused` $\to$ `Sleeping`.
    - `ResumeSession` accepts `Paused | Sleeping | Error` $\to$ `Ready`. Resuming from `Paused` is instantaneous ($<10\text{ms}$); resuming from `Sleeping` triggers background model re-warming ($\approx 1000\text{ms}$).
    - `SpeechStart` and `PttStart` drop audio in `Sleeping` identically to `Idle` and `Paused`.

11. **The LLM Agent Harness Runtime & Duplex Pipe (`docs/specs/harness-spec.md`):**
    The Cognitive Stage is governed by `Harness` (implemented in `services/harness/orchestrator.rs`), mounted 1:1 between `session_start` and `session_end`. Communication between `Harness` and `LlmActor` occurs over a persistent duplex dialogue pipe. The Harness is the single front door for STT transcripts and sole emitter of `VoxEvent::LlmFinished`. Full stage taxonomy (`stages/`), egress streaming (`streaming/`), token budgeting (65% soft / 85% critical), and generic non-terminal phase contracts are defined in the golden **[LLM Agent Harness Specification](file:///home/addy/projects/apps/vox/docs/specs/harness-spec.md)**.

---

## 2. Event Enumerations & Channel Contracts

### Central Router Event Queue (`VoxEvent`)

- **`SessionStart { owner, session_id }`**: Enqueued by IPC `start_session(sessionId)` (Assistant track only). Mounts `Harness` (continuing `session_id` if provided, else fresh), initializes domain stages, establishes duplex pipe to `LlmActor`, restores `owner=Assistant`, and transitions `Idle` $\to$ `Ready`.
- **`PauseSession`**: Enqueued by IPC `pause_session` or `idle_monitor` (Assistant track only). Cancels playback, trips cancellation flags, keeps CPAL warm, unconditionally flips owner to Dictation, and transitions Assistant `Current` $\to$ `Paused`.
- **`ResumeSession`**: Enqueued by IPC `resume_session` (Assistant track only). Restores owner to Assistant, re-arms audio/VAD/network from `Paused`, `Sleeping`, or `Error` $\to$ `Ready`.
- **`EndSession`**: Enqueued by IPC `end_session` (Assistant track only). Unmounts and tears down `Harness`, unconditionally flips owner to Dictation, persists end metrics, transitions Assistant `Current` $\to$ `Idle`. If Dictation is `Idle`, completely tears down CPAL hardware.
- **`PttStart`**: Enqueued by IPC `ptt_start`. Evaluates barge-in vs fresh onset, starts window capture, transitions `Ready`/`Thinking`/`Speaking`/`Working` $\to$ `Listening`.
- **`PttStop`**: Enqueued by IPC `ptt_stop`. Stops window validation. If speech detected $\to$ transitions `Listening` $\to$ `Thinking` and dispatches audio to STT/provider; if silence $\to$ transitions `Listening` $\to$ `Ready`.
- **`PttCancel`**: Enqueued by IPC `ptt_cancel`. Aborts current PTT recording window, transitions `Listening` $\to$ `Ready`.
- **`SpeechStart`**: Emitted by local VAD (modular) or provider actor (realtime). Evaluates barge-in vs onset, allocates monotonic turn ID via `next_turn()`, and transitions `Ready`/`Thinking`/`Speaking`/`Working` $\to$ `Listening`.
- **`SpeechEnd`**: Emitted by local VAD (modular) or provider actor (realtime). Signals speech boundary and transitions `Listening` $\to$ `Thinking`. (In modular, `VadActor` dispatches `SttCommand::Final` directly to STT actor on the dedicated hot-path channel).
- **`TextInput { text }`**: Emitted by IPC `text_input` (`submit_text_input`). Handled by `pipeline::assistant::text::on_text_input`: in Modular mode allocates monotonic `turn_id` via `next_turn()`, advances to `Thinking`, and delegates to `spawn_harness_turn_task` for turn-scoped 7-step Cognitive Loop processing; in Realtime mode forwards query directly to `RealtimeActor::send_text` across the WebSocket connection.
- **`TranscriptFinal { turn_id, text }`**: Emitted by STT worker or provider. Delegated to `Harness::execute_turn`: if a non-terminal phase is initiated (e.g. critical compaction, tool execution), transitions `Thinking` $\to$ `Working`, dispatches `AudioIntent::InterimFiller` to TTS, executes the intermediate operation, and then proceeds with conversational response generation; if nominal $\to$ stays in `Thinking` and generates response; if empty text $\to$ recovers `Thinking` $\to$ `Ready` with Info Toast.
- **`LlmFinished { turn_id }`**: Emitted **exclusively by the Harness** (`StreamRouter`) upon receiving `LlmResponse::Finished` after flushing the clause chunker remainder. Persists turn context to database and flushes audio pre-roll. (Never emitted on stream error, drop, or cancellation).
- **`PlaybackStarted { turn_id }`**: Emitted by PlaybackEngine when ring buffer samples satisfy pre-roll threshold or flush. Transitions `Thinking` $\to$ `Speaking`.
- **`PlaybackFinished { turn_id }`**: Emitted by PlaybackEngine when output ring buffer drains to empty and no synthesis jobs remain. Transitions `Speaking` $\to$ `Ready`.
- **`Error(PipelineError)`**: Emitted on subsystem failures with explicit 2D classification struct `PipelineError { turn_id, message, source, impact, actionability }`. Governs non-fatal recovery (e.g. `Degraded` with no state change, `TurnAborted` resetting to `Ready`, `SessionHalted` transitioning to `Error`).
- **`Cancelled { turn_id }`**: Emitted on turn aborts. Transitions state to `Ready`.
- **`Shutdown`**: Router loop termination sentinel.

### Realtime Provider Internal Stream (`RealtimeProviderEvent`)

- **`AudioChunk(Vec<i16>)`**: Binary PCM audio stream routed directly to playback worker (bypasses Router).
- **`SpeechStart`**: Inbound speech onset detected by server VAD $\to$ translated to `VoxEvent::SpeechStart`.
- **`SpeechEnd`**: Inbound speech completion detected by server VAD $\to$ translated to `VoxEvent::SpeechEnd`.
- **`TranscriptPartial { turn_id, text }`**: Interim user speech transcription $\to$ emitted directly to UI via IPC (bypasses Router).
- **`TranscriptFinal { turn_id, text }`**: Finalized user speech transcription $\to$ translated to `VoxEvent::TranscriptFinal`.
- **`LlmToken { turn_id, token }`**: Streamed assistant text tokens $\to$ emitted directly to UI via IPC (bypasses Router).
- **`LlmFinished { turn_id }`**: Model generation complete marker $\to$ translated to `VoxEvent::LlmFinished`.
- **`ToolCall { id, name, args }`**: Server-initiated function call request parsed from provider wire format (e.g. Gemini `serverContent.toolCall` or Deepgram `FunctionCallRequest`). Dispatched by `RealtimeActor` loop to asynchronous `ToolExecutor`, persisted to `session_tool_calls`, and answered via `OutboundCommand::ToolResponse` / `RealtimeSession::send_tool_response`.
- **`Error { turn_id, message, impact, actionability }`**: Provider network/protocol error $\to$ translated directly to `VoxEvent::Error(PipelineError)`.
- **`SessionResumptionHandle { handle, model }`**: Session cache token $\to$ written to disk non-blocking.

---

## 3. Comprehensive 6-Domain Target Specification Matrix

The system interaction topology is partitioned into 6 distinct, mutually exclusive execution domains:
1. **Modular Passive (Assistant):** Local VAD continuous speech segmentation $\to$ local STT $\to$ local/cloud LLM $\to$ local TTS.
2. **Modular PTT (Assistant):** Assistant Push-To-Talk window validation $\to$ local STT $\to$ local/cloud LLM $\to$ local TTS.
3. **Realtime Passive (Assistant):** Cloud streaming WebSocket bidirectional audio/text.
4. **Realtime PTT (Assistant):** Cloud streaming WebSocket with client-side PTT window gating.
5. **Dictation Passive (Ambient):** Ambient continuous VAD speech segmentation $\to$ local STT $\to$ Devanagari transliteration $\to$ OS output router (zero LLM/TTS).
6. **Dictation PTT (Ambient):** Global OS hotkey (`Alt+Space`) held window validation $\to$ local STT $\to$ Devanagari transliteration $\to$ OS output router (zero LLM/TTS).

| Trigger | Event | Modular Passive | Modular PTT | Realtime Passive | Realtime PTT | Dictation Passive | Dictation PTT |
|---|---|---|---|---|---|---|---|
| **IPC** | **`SessionStart`** | **Pre:** `Idle`<br>**Trans:** `Idle` $\to$ `Ready` (deferred until discovery probe completes; holds `session_starting=true`)<br>**Shared FX:** Mount `Harness(session_id)`, prefetch identity, `cancel_flag=false`, persist `SessionStarted`, spawn `idle_monitor`. **Set `owner=Assistant` only upon probe completion** (preserves dictation hotkey during background probe).<br>**Domain FX:** Start audio engine, warm STT/LLM/TTS workers, VAD set `ContinuousSegmentation` | **Pre:** `Idle`<br>**Trans:** `Idle` $\to$ `Ready`<br>**Shared FX:** Same shared FX<br>**Domain FX:** Start audio engine, lazy worker pool, VAD set `WindowedValidation`, arm hotkey | **Pre:** `Idle`<br>**Trans:** `Idle` $\to$ `Ready`<br>**Shared FX:** Same shared FX<br>**Domain FX:** Start audio engine, `RealtimeActor::start(Passive)` (open WS, check session cache), VAD set `StreamPassthrough` | **Pre:** `Idle`<br>**Trans:** `Idle` $\to$ `Ready`<br>**Shared FX:** Same shared FX<br>**Domain FX:** Start audio engine, `RealtimeActor::start(PTT)` (open WS standby), VAD set `WindowedValidation`, arm hotkey | **N/A** (Dictation is ambient; governed by `settings.dictation.enabled` $\to$ `Ready` / `Idle`) | **N/A** (Dictation is ambient; governed by `settings.dictation.enabled` $\to$ `Ready` / `Idle`) |
| **IPC / IdleMonitor** | **`PauseSession`** | **Pre:** Any active state (if `Idle`/`Paused`/`Sleeping` $\to$ no-op)<br>**Trans:** `Current` $\to$ `Paused` (then `Paused` $\to$ `Sleeping` after 5m continuous inactivity)<br>**Shared FX:** `cancel_flag=true`, `token.cancel()`, `turn_open=false`, `PlaybackEngine::cancel()`, `ACCUMULATOR.clear()`. **CPAL remains warm**. Unconditionally sets `owner=Dictation`. Arms 5m model offload timer<br>**Domain FX:** None (local workers observe token) | Same shared FX.<br>**Domain FX:** None | Same shared FX.<br>**Domain FX:** `VadCommand::StopRealtime` (halt mic passthrough; keep WS alive) | Same shared FX.<br>**Domain FX:** `VadCommand::StopRealtime` | **N/A** (Dictation has no session pause) | **N/A** (Dictation has no session pause) |
| **IPC** | **`ResumeSession`** | **Pre:** `Paused`, `Sleeping`, or `Error`<br>**Trans:** `Paused`/`Sleeping`/`Error` $\to$ `Ready` (on success) \| `Error` $\to$ `Error` (on fail)<br>**Shared FX:** `cancel_flag=false`, renew `token`, restore `owner=Assistant`<br>**Domain FX:** If `Paused`: instant VAD re-arm.<br>If `Sleeping`: re-warm LLM/TTS workers in background ($\approx 1000\text{ms}$). VAD set `ContinuousSegmentation` | Same shared FX.<br>**Domain FX:** If `Sleeping`: re-warm LLM/TTS. VAD set `WindowedValidation`, re-arm hotkey | Same shared FX.<br>**Domain FX:** `VadCommand::StartRealtime`. Attempt WS reconnect if dropped. If fail $\to$ `Error` + Toast: *"Resumption failed. Please restart session."* | Same shared FX.<br>**Domain FX:** `VadCommand::StartRealtime`. Attempt WS reconnect if dropped. If fail $\to$ `Error` + Toast | **N/A** | **N/A** |
| **IPC** | **`EndSession`** | **Pre:** Any state (idempotent)<br>**Trans:** `Current` $\to$ `Idle`<br>**Shared FX:** Unmount `Harness`, `cancel_flag=true`, `token.cancel()`, `turn_open=false`, cancel probe if `session_starting`, `PlaybackEngine::cancel()`, `ACCUMULATOR.clear()`, persist `SessionEnded`, notify memory worker, unconditionally set `owner=Dictation`. Teardown CPAL engine only if `dictation_state == Idle`<br>**Domain FX:** None | Same shared FX.<br>**Domain FX:** Disarm hotkey | Same shared FX.<br>**Domain FX:** `RealtimeActor::stop()`, disconnect WS, purge session cache | Same shared FX.<br>**Domain FX:** `RealtimeActor::stop()`, disconnect WS, purge session cache, disarm hotkey | **N/A** | **N/A** |
| **IPC / Global Hotkey** | **`PttStart`** | **N/A** (Passive rejects PTT) | **Pre:** Any state<br>**Cond:** If `Idle`/`Paused`/`Sleeping`/`Listening` $\to$ drop; if `Thinking`/`Speaking`/`Working` $\to$ delegates completely to `on_interrupt()`; if `Ready` $\to$ direct onset (`next_turn()`, `clear()`, cancel playback, `transition(Listening)`)<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** Starts VAD window validation (`VadCommand::StartWindowValidation`)<br>**Domain FX:** None (local models) | **N/A** | Same pre-state guard, conditions, transitions, and shared FX.<br>**Domain FX:** If barge-in, `on_interrupt()` also dispatches `OutboundCommand::Interrupt` to remote provider | **N/A** (Passive dictation does not use hotkey) | **Pre:** Any state<br>**Cond:** If `Idle` $\to$ emit `VoxError("Dictation is disabled in Settings")` + Toast;<br>If `Ready` $\to$ direct onset (`next_turn()`, `transition_dictation(Listening)`);<br>If `Thinking` $\to$ pipeline overlap: start turn $N+1$ (`next_turn()`) without aborting turn $N$ STT;<br>If `Listening` $\to$ drop duplicate<br>**Trans:** `Ready`/`Thinking` $\to$ `Listening`<br>**Shared FX:** `VadCommand::StartWindowValidation`<br>**Domain FX:** No LLM/TTS allocation |
| **IPC / Global Hotkey** | **`PttStop`** | **N/A** | **Pre:** `Listening` (else drop)<br>**Cond:** Local VAD speech detected and audio non-empty?<br>**Trans:** Speech $\to$ `Thinking` \| Silence $\to$ `Ready`<br>**Shared FX:** Drain VAD window buffer<br>**Domain FX:** If speech $\to$ `SttCommand::Final(turn_id, audio)` to STT worker; if silence $\to$ drop buffer | **N/A** | Same pre-state guard, condition, transition, and shared FX.<br>**Domain FX:** If speech $\to$ convert to PCM i16 and dispatch `RealtimeActor::commit_speech_turn()`; if silence $\to$ drop buffer (0 network frames) | **N/A** | **Pre:** `Listening` (else drop)<br>**Cond:** Local VAD speech detected and audio non-empty?<br>**Trans:** Speech $\to$ `Thinking` \| Silence $\to$ `Ready`<br>**Shared FX:** Drain VAD window buffer via `StopWindowValidation`<br>**Domain FX:** If speech $\to$ `SttCommand::Final(turn_id, audio)` to STT; if silence $\to$ drop buffer |
| **IPC / Global Hotkey** | **`PttCancel`** | **N/A** | **Pre:** `Listening` (else no-op)<br>**Trans:** `Listening` $\to$ `Ready`<br>**Shared FX:** Cancel turn token, `ACCUMULATOR.clear()`, cancel playback, `VadCommand::StopWindowValidation` (discard buffer)<br>**Domain FX:** None | **N/A** | Same pre-state guard, transition, shared FX, and domain FX (audio was never committed to provider) | **N/A** | **Pre:** `Listening` (else no-op)<br>**Trans:** `Listening` $\to$ `Ready`<br>**Shared FX:** Discard window buffer, reset STT stream<br>**Domain FX:** None |
| **VAD / RealtimeActor** | **`SpeechStart`** | **Pre:** Any state<br>**Cond:** If `Idle`/`Paused`/`Sleeping` $\to$ drop; if `Thinking`/`Speaking`/`Working` $\to$ delegates completely to `on_interrupt()`; if `Ready` $\to$ direct onset (`next_turn()`, `clear()`, cancel playback, `transition(Listening)`)<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** Dispatches `SttCommand::ResetStream`<br>**Domain FX:** Sourced from local `VadActor` | **N/A** (`PttStart` owns PTT speech onset) | Same pre-state guard, conditions, transitions, and shared FX.<br>**Domain FX:** Sourced from `RealtimeActor` (server VAD onset / first inbound speech packet); if barge-in, server already detected onset | **N/A** (`PttStart` owns PTT speech onset) | **Pre:** Any state<br>**Cond:** If `Idle` $\to$ drop;<br>If `Ready` $\to$ `next_turn()`, `transition_dictation(Listening)`;<br>If `Thinking` $\to$ pipeline overlap: start turn $N+1$ (`next_turn()`) without aborting turn $N$ STT;<br>If `Listening` $\to$ drop<br>**Trans:** `Ready`/`Thinking` $\to$ `Listening`<br>**Shared FX:** Reset STT stream<br>**Domain FX:** Sourced from local `VadActor` | **N/A** (`PttStart` owns PTT dictation onset) |
| **VAD / RealtimeActor** | **`SpeechEnd`** | **Pre:** `Listening` (else drop)<br>**Trans:** `Listening` $\to$ `Thinking`<br>**Shared FX:** None<br>**Domain FX:** Sourced from local `VadActor` (silence threshold reached); dispatches `SttCommand::Final(turn_id, utterance_buffer)` directly to STT worker | **N/A** (`PttStop` owns PTT speech completion) | **Pre:** `Listening` (else drop)<br>**Trans:** `Listening` $\to$ `Thinking`<br>**Shared FX:** None<br>**Domain FX:** Sourced from `RealtimeActor` (server VAD completion event, e.g. Deepgram `UserStoppedSpeaking`) | **N/A** (`PttStop` owns PTT speech completion) | **Pre:** `Listening` (else drop)<br>**Trans:** `Listening` $\to$ `Thinking`<br>**Shared FX:** None<br>**Domain FX:** Local `VadActor` silence cutoff; dispatches `SttCommand::Final` to STT worker | **N/A** (`PttStop` owns PTT dictation completion) |
| **STT / RealtimeActor** | **`TranscriptFinal`** | **Pre:** `Thinking` (if `Idle`/`Paused`/`Sleeping` $\to$ drop)<br>**Cond:** Text non-empty or empty?<br>**Trans:** Valid text $\to$ Sets `turn_open=true`, remains `Thinking` (or `Working` if non-terminal phase) \| Empty text $\to$ `Thinking` $\to$ `Ready`<br>**Shared FX:** If Valid $\to$ store transcript in accumulator, emit `IpcEvent::TranscriptFinal`. If Empty $\to$ emit `IpcEvent::ShowToast("No speech recognized", Info)`, clear accumulator<br>**Domain FX:** If Valid $\to$ delegate to `Harness::execute_turn`: if non-terminal phase (e.g. compaction, non-terminal tool) triggered $\to$ transition `Thinking` $\to$ `Working`, dispatch `AudioIntent::InterimFiller` to TTS, execute intermediate operation, then generate response via duplex pipe; if nominal $\to$ dispatch generation via duplex pipe | Same guard, condition, transition, shared FX, and domain FX | Same guard, condition, transition, and shared FX.<br>**Domain FX:** If Valid $\to$ assert `pending_synthesis_jobs=1` (server already generating audio in-flight) | Same as Realtime Passive | **Pre:** `Thinking` (else drop)<br>**Cond:** Text non-empty or empty?<br>**Trans:** `Thinking` $\to$ `Ready`<br>**Shared FX:** Cache `last_dictation_transcript`, emit `IpcEvent::TranscriptFinal` to `WINDOW_TRAY`<br>**Domain FX:** If non-empty $\to$ transliterate Devanagari if enabled $\to$ spawn `route_transcript(Paste \| Clipboard \| Tray)` | Same as Dictation Passive |
| **IPC (`submit_text_input`)** | **`TextInput { text }`** | **Pre:** Active session (`Ready`, `Thinking`, `Speaking`, `Working`) or `Paused` (auto-resume first). If `Idle`/`Sleeping` $\to$ drop with error.<br>**Cond:** If `Paused` $\to$ run `ResumeSession` shared FX (`cancel_flag=false`, renew token, `owner=Assistant`, VAD re-arm), then process as `Ready`; if `Thinking`/`Speaking`/`Working` $\to$ delegate to `on_interrupt()`; if `Ready` $\to$ `next_turn()`.<br>**Trans:** `Current` $\to$ `Thinking` (or `Working` if compaction/tool triggered).<br>**Shared FX:** Emit `IpcEvent::TranscriptFinal` to UI; store user query in `ACCUMULATOR`.<br>**Domain FX:** Forward to dedicated `on_text_input` $\to$ `spawn_harness_turn_task` running the 7-step Cognitive Loop. Bypasses CPAL, VAD, and STT; TTS executes normally. | Same pre-state guard, condition, transition, shared FX, and domain FX. | Same pre-state guard, condition, and shared FX.<br>**Domain FX:** Forward to dedicated `on_text_input` $\to$ dispatches text frame directly to provider session via `RealtimeActor::send_text` (`clientContent` for Gemini Live, `ConversationText` for Deepgram Voice Agent). STT bypassed. | Same as Realtime Passive. | **N/A** (Dictation is speech-to-active-window; text input is dropped). | **N/A** (Dictation is speech-to-active-window; text input is dropped). |
| *(Internal Handler)* | **`on_interrupt`** | Invoked strictly inside `SpeechStart` or `PttStart` guards when in `Thinking`, `Speaking`, or `Working`.<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** Immediate audio silence: (1) `PlaybackEngine::cancel()`, (2) Cancel active turn token, (3) Reset `pending_synthesis_jobs=0`, (4) Set `turn_open=false`, `drained_while_open=false`, (5) Optional outbound interrupt signal if Realtime, (6) Clear `ACCUMULATOR`, (7) Advance `next_turn()`, reset `cancel_flag=false`, and execute `transition(Listening)`. **Persistence is exclusively owned by the Harness `Cancelled` branch** (bypasses double-writer race; commits partial text if non-empty, rolls back if empty; discards scratchpad).<br>**Domain FX:** None (local models only) | Invoked strictly inside `PttStart` guard when in `Thinking`, `Speaking`, or `Working`.<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** Same shared FX.<br>**Domain FX:** None (fully local) | Invoked strictly inside `SpeechStart` guard when in `Thinking`, `Speaking`, or `Working`.<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** Same shared FX.<br>**Domain FX:** None (server detected barge-in first) | Invoked strictly inside `PttStart` guard when in `Thinking`, `Speaking`, or `Working`.<br>**Trans:** `Current` $\to$ `Listening`<br>**Shared FX:** Same shared FX.<br>**Domain FX:** Step 5 dispatches outbound `OutboundCommand::Interrupt` to remote provider | **N/A** (Dictation has no TTS/playback interruption; uses pipeline overlap) | **N/A** (Dictation has no TTS/playback interruption; uses pipeline overlap) |
| **LLM / RealtimeActor** | **`LlmFinished`** | **Pre:** `Thinking`, `Speaking`, or `Working`<br>**Trans:** If `drained_while_open == true` and `pending_synthesis_jobs == 0` $\to$ `Current` $\to$ `Ready`; else remain `Speaking`<br>**Shared FX:** Emitted exclusively by `Harness` once after the **terminal** model pass. Clears `turn_open=false`. If assistant text is non-empty, commits to `stages/history.rs` and dispatches `PersistenceEvent::TurnCompleted`. If empty, rolls back user turn from working history and skips `TurnCompleted`.<br>**Domain FX:** Flush `streaming/chunker.rs` remainder to TTS worker; `TtsActor` decrements `pending_synthesis_jobs` on completion; invoke `PlaybackEngine::flush_pre_roll()` | Same shared FX and domain FX | Same shared FX.<br>**Domain FX:** Reset `pending_synthesis_jobs=0`; invoke `PlaybackEngine::flush_pre_roll()` | Same as Realtime Passive | **N/A** (Zero LLM/TTS in dictation) | **N/A** (Zero LLM/TTS in dictation) |
| **Harness (Stage)** | **`NonTerminalPhase`** *(Internal Stage Trigger: Compaction or Non-Terminal Tool)* | **Pre:** `Thinking` or `Speaking`<br>**Trans:** `Current` $\to$ `Working`<br>**Shared FX:** (1) For non-terminal tools, dispatch model-provided `spoken_filler` (or localized fallback if empty/compaction) to TTS as `AudioIntent::InterimFiller`. (2) Lock state in `Working`. (3) Playback of interim filler does not transition out of `Working`.<br>**Domain FX:** Executes cognitive task (compaction or tool invocation) asynchronously off voice hot path. | Same | **Domain FX:** Server-side tool execution via `RealtimeActor` event loop: receives `RealtimeProviderEvent::ToolCall`, executes registered realtime tool via `ToolExecutor::execute_tool(PipelineMode::Realtime, ...)`, persists trace to `session_tool_calls`, and transmits `send_tool_response` over WebSocket. | Same as Realtime Passive | **N/A** | **N/A** |
| **PlaybackEngine** | **`PlaybackStarted`** | **Pre:** `Thinking` or `Working` (if other $\to$ drop)<br>**Trans:** If `AudioIntent::InterimFiller` $\to$ remain `Working`. If `AudioIntent::TurnResponse` $\to$ transition `Thinking`/`Working` $\to$ `Speaking`<br>**Shared FX:** Clears `drained_while_open=false`. Emit `IpcEvent::StateChanged(Speaking)` only for `AudioIntent::TurnResponse`<br>**Domain FX:** Triggered when playback ring buffer reaches modular pre-roll threshold (12,000 samples @ 48kHz = 250ms) or on `flush_pre_roll()` | Same | **Pre:** `Thinking` (if `!=Thinking` $\to$ drop)<br>**Trans:** `Thinking` $\to$ `Speaking`<br>**Shared FX:** Clears `drained_while_open=false`. Emit `IpcEvent::StateChanged(Speaking)`<br>**Domain FX:** Triggered when playback ring buffer reaches realtime pre-roll threshold (3,840 samples @ 48kHz = 80ms) or on `flush_pre_roll()` | Same | **N/A** | **N/A** |
| **PlaybackEngine** | **`PlaybackFinished`** | **Pre:** `Speaking` or `Working` (if other $\to$ drop)<br>**Trans:** If `AudioIntent::InterimFiller` completes $\to$ remain `Working`. If `AudioIntent::TurnResponse` completes:<br>- If `turn_open == true`: **do not transition** to `Ready` or `Working`; set `drained_while_open = true`<br>- If `turn_open == false` and `pending_synthesis_jobs == 0`: transition `Speaking` $\to$ `Ready`<br>**Shared FX:** Emit `IpcEvent::StateChanged(Ready)` only when transitioning to `Ready`<br>**Domain FX:** Fired when output ring buffer drains to empty | Same | Same | Same | **N/A** | **N/A** |
| **Any Actor** | **`Error`** | **Pre:** Any state<br>**Trans:**<br>- If `Degraded` $\to$ No state change<br>- If `TurnAborted` $\to$ `Ready`<br>- If `SessionHalted` $\to$ `Error`<br>**Shared FX:** Attributed logging; if `TurnAborted` or `SessionHalted`, cancel `PlaybackEngine`, cancel turn token, clear `turn_open=false`, `drained_while_open=false`, and reset `pending_synthesis_jobs=0`. If `Actionable`, emit `IpcEvent::NotificationCreated`. If `NonActionable`, emit ephemeral UI toast only. | Same | Same | Same | **Pre:** Any state<br>**Trans:**<br>- If `Degraded` $\to$ No state change<br>- If `TurnAborted` $\to$ auto-recover to `Ready` (if enabled) or `Idle` (if disabled)<br>- If `SessionHalted` $\to$ `Error`<br>**Shared FX:** Attributed logging, ephemeral toast to `WINDOW_TRAY`, notifications only if `Actionable`. | Same as Dictation Passive |
| **Any Actor** | **`Cancelled`** | **Pre:** Any state<br>**Trans:** `Current` $\to$ `Ready`<br>**Shared FX:** Clear `TurnAccumulator`, reset `pending_synthesis_jobs=0`, clear `turn_open=false`, `drained_while_open=false`, cancel audio playback | Same | Same | Same | **Pre:** Any state<br>**Trans:** `Current` $\to$ `Ready`<br>**Shared FX:** Reset STT stream, clear last transcript | Same as Dictation Passive |

---

## 4. Idle Monitor & Model Lifecycle Specification

1. **Idle Inactivity Observer (`spawn_idle_monitor`):**
   - Subscribes to atomic `InteractionState` broadcasts.
   - If state enters `InteractionState::Ready`, arms a continuous 7-minute ($420\text{s}$) timeout timer (`REALTIME_IDLE_TIMEOUT`).
   - If state transitions away from `Ready` (e.g. to `Listening`), the timer resets.
   - If the timer expires while in `Ready`, enqueues `VoxEvent::PauseSession` into `event_tx`.
   - Transitions `Ready` $\to$ `Paused`, disarming mic passthrough while keeping CPAL warm.

2. **Tiered Model Offload Specification (Secondary Paused Offload $\to$ `Sleeping`):**
   - **Trigger:** An asynchronous observer monitors sustained duration in `InteractionState::Paused`.
   - **Threshold:** 5 continuous minutes ($300\text{s}$) in `Paused` state without user resumption.
   - **Offload Action (Reclaiming ~2.0 GB – 2.8 GB RAM):**
     - **Always Offload LLM & TTS:** Unloads local LLM (Qwen) and TTS (Chatterbox/Sherpa) model weights from RAM, releasing ~2.5 GB.
     - **Conditional STT Retention:**
       - If dictation is active (`dictation_state != Idle`): **STT (Nemotron/Whisper) remains resident in RAM.** This guarantees that user hotkey dictation into external applications remains instantaneous (<10ms onset).
       - If dictation is disabled (`dictation_state == Idle`): **STT model is also offloaded**, reclaiming 100% of model memory.
     - **State Transition:** Executes `transition(InteractionState::Sleeping)`. CPAL hardware audio streams remain warm and resident.
   - **Re-warming on Resumption (`ResumeSession`):**
     - Resuming from `Paused`: Instantaneous (<10ms).
     - Resuming from `Sleeping`: Background re-warming of offloaded models (~1000ms) with UI indicator before returning to `Ready`.
   - **Full Engine Teardown:** When application window close is requested (`lib.rs`), if `dictation_state == Idle` and `assistant_state == Idle`, `stop_engine()` is invoked, completely offloading all models and tearing down CPAL audio hardware.

---

## 5. Canonical Internal Handler: Barge-In (`on_interrupt`)

`on_interrupt` is the single, self-contained barge-in handler executed exclusively inside `SpeechStart` and `PttStart` handler guards when user speech or hotkey interaction occurs while the assistant is in `Thinking` or `Speaking`:

1. **Immediate Audio Silence:** Calls `PlaybackEngine::cancel()` to drain and mute the CPAL hardware audio output buffer.
2. **Cancellation & Guard Reset:** Asserts `cancel_flag = true`, cancels the active `CancellationToken` of the current turn, sets `turn_open = false`, `drained_while_open = false`, and resets `pending_synthesis_jobs = 0`.
3. **Outbound Provider Notification:** If in Realtime mode and the interrupt was client-detected (e.g. user pressed PTT key in Realtime PTT), dispatches `OutboundCommand::Interrupt` to the remote WebSocket session via non-blocking channel send.
4. **Single-Writer Barge-In Routing:** `on_interrupt` purposefully does **not** persist turn data or manipulate history. The active `Harness` streaming pass observes cancellation, captures the pass-local generated text, and returns `StreamPassOutcome::Cancelled { partial }`. The `Harness::Cancelled` branch is the **sole persistence writer**:
   - If `partial.trim().is_empty()`: rolls back the user turn from in-memory history and emits no `TurnCompleted` event.
   - If `!partial.trim().is_empty()`: commits `(user_turn, partial)` to in-memory history and dispatches `PersistenceEvent::TurnCompleted`.
   - The turn-local ephemeral tool scratchpad is unconditionally discarded.
5. **Accumulator Reset:** Clears `TurnAccumulator` for the incoming user utterance.
6. **Turn Advance & Token Allocation:** Calls `PipelineAtomics::next_turn()` to atomically allocate the new monotonic `turn_id` and renew the `CancellationToken` for the incoming turn; asserts `cancel_flag = false`.
7. **State Transition:** Executes `transition(InteractionState::Listening)`, broadcasting `IpcEvent::StateChanged(Listening)` carrying the newly allocated `turn_id`.

---

## 6. Target Code Organization for Refactored `pipeline/`

```
app/src-tauri/src/pipeline/
├── mod.rs               # RoutingContext, transition(), target_window(), state query helpers, spawn_idle_monitor
├── router.rs            # spawn_router(): Single-FIFO central event pump loop (dispatches to assistant vs dictation)
│
├── assistant/           # [CANONICAL ASSISTANT EVENT HANDLERS]
│   ├── mod.rs           # Match assistant events & dispatch to handlers
│   ├── session.rs       # on_session_start, on_pause, on_resume, on_end
│   ├── ptt.rs           # on_ptt_start, on_ptt_stop, on_ptt_cancel
│   ├── speech.rs        # on_speech_start, on_speech_end
│   ├── transcript.rs    # on_transcript_final (LLM spawn or empty text recovery, transition filler TTS)
│   ├── llm.rs           # on_llm_finished (DB persistence & pre-roll flush)
│   ├── playback.rs      # on_playback_started, on_playback_finished
│   ├── interrupt.rs     # on_interrupt (shared barge-in handler)
│   └── error.rs         # on_error, on_cancelled, voice_error & toast dispatch
│
└── dictation/           # [CANONICAL DICTATION EVENT HANDLERS]
    ├── mod.rs           # Match dictation events & dispatch to handlers
    ├── ptt.rs           # on_ptt_start, on_ptt_stop, on_ptt_cancel, hotkey listener
    ├── speech.rs        # on_speech_start, on_speech_end (passive continuous segmentation)
    ├── transcript.rs    # on_transcript_final (transliteration & services/dictation/output_router)
    └── error.rs         # on_error, on_cancelled, voice_error & toast dispatch
```

---

## 7. Resilience, Network Failover & Recovery Guarantees

### 1. Network Partition Tolerance & WebSocket Failover
- **Automatic Reconnection:** If the WebSocket drops mid-turn, the transport harness (`services/realtime/transport/connection.rs`) executes exponential backoff reconnect up to `MAX_RECONNECT_ATTEMPTS = 3` (delays: 1s, 2s, 4s $\to$ ~7s total).
- **Session Resumption:** If reconnect succeeds within the 2-hour TTL (`SESSION_CACHE_TTL_MS`), the cached `SessionResumptionHandle` restores conversational state without losing previous turns.
- **Failover to Error State:** If reconnect fails after 3 attempts, `RealtimeActor` emits `VoxEvent::Error { message: "WebSocket connection dropped after 3 retry attempts" }`, transitioning state to `Error`.

### 2. Memory Bounds & Zero Hot-Path Allocation
- **Static Ring Buffers:** Audio playback pre-allocates a fixed $30\text{s}$ @ 48kHz ring buffer ($5.7\text{MB}$). VAD pre-roll is fixed at $300\text{ms}$ ($9.6\text{KB}$).
- **Zero Event Queue Congestion:** With `TranscriptPartial` and `LlmToken` removed from `VoxEvent`, the central channel event rate is $< 5$ events per minute, guaranteeing zero queue backpressure and immunity to OOM under the 8GB RAM budget.

### 3. In-Process Type Safety vs. IPC Serialization
- `VoxEvent` is an in-process, compiled Rust enum passed via stack/pointer over in-memory channels. It does not cross network or serialization boundaries and cannot experience deserialization corruption.
- IPC payloads across the Tauri boundary (`IpcEvent`) are strongly typed and validated by serde JSON serialization.

### 4. Thread Join & Shutdown Discipline
- Worker threads (`VadActor`, `SttActor`, `TtsActor`) observe `engine_shutdown: Arc<AtomicBool>` and `CancellationToken`.
- Upon `VoxEvent::Shutdown`, the main process enforces a bounded $2\text{s}$ timeout on background thread joins before exiting cleanly, preventing dangling OS threads.

### 5. Async Runtime Consolidation
- `on_interrupt` and other event handlers dispatch outbound commands via lock-free MPSC channel `try_send` into the existing Tokio background task. Zero nested Tokio runtimes (`Runtime::new()`) are instantiated.

### 6. Atomic Persistence Rollback
- All SQLite conversational logging is wrapped in SQLite WAL transactions via `persistence/db.rs`. Interrupted turns commit only the yielded text; phantom or unverified turns are never committed to disk.

### 7. Failed Resumption Cleanup Hooks
- If `ResumeSession` fails, cleanup hooks tear down any orphaned socket handles, reset `pending_synthesis_jobs = 0`, and leave `cancel_flag = true` with audio muted.

### 8. Lock-Free Audio Ingestion Gate
- CPAL audio callback reads a single `AtomicBool` (`ingestion_gate.load(Relaxed)`). When closed, incoming frames are dropped immediately before entering the SPSC ring buffer, preventing buffer overruns and zeroing VAD inference overhead when both tracks are inactive.

### 9. Non-Destructive Dictation Overlap
- In Dictation PTT and Passive modes, speech or hotkey triggers during `Thinking` do not abort or drop the prior STT inference task. Turn $N$ completes and routes its transcript to the target destination, while Turn $N+1$ begins recording speech immediately.

---

## 8. Exhaustive Subsystem Error Domain Matrix & Panic Containment

### 1. Orthogonal 2D Classification Schema
Every pipeline error decouples state transition impact from user UI actionability:
- **`PipelineImpact`**:
  - `Degraded`: The pipeline does not halt; the turn completes with degraded fidelity. State transition: **None**.
  - `TurnAborted`: The active turn fails cleanly; state transitions **directly to `Ready`** (never locking into `Error`). Audio playback and turn tokens are cancelled.
  - `SessionHalted`: The session cannot proceed without external intervention. State transitions to **`InteractionState::Error`**.
- **`Actionability`**:
  - `None`: Transient internal hiccups. Emits an ephemeral **UI Toast only**. Never persists to the Notifications drawer.
  - `Actionable { category, hint }`: User configuration or hardware intervention needed. Emits an ephemeral **UI Toast AND a persistent record in the Notifications drawer** (`IpcEvent::NotificationCreated`).

### 2. Comprehensive Subsystem Error Domain Table

| # | Subsystem / Error Trigger | Root Cause & Mechanism | Impact | Actionability | State Target | UI Surface |
|---|---|---|---|---|---|---|
| 1 | **Identity Memory Overflow** | Preloaded active identity facts exceed `DEFAULT_MEMORY_MAX_PERSONAL_SHARE` (15% of `context_window`) | `Degraded` | `Actionable` (if recurring) | *No change* | Ephemeral Toast + Notification ("Memory approaching context limit. Consider increasing context_window.") |
| 2 | **Critical Compaction Failure** | LLM compaction crash or memory DB error during turn prep in `transcript.rs` | `Degraded` | `NonActionable` | *No change* | Logged at WARN; falls back immediately to FIFO sliding window without interrupting user turn. Subtle toast only. |
| 3 | **Dynamic Retrieval Failure** | Semantic memory retrieval query errors or returns empty | `Degraded` | `NonActionable` | *No change* | Turn proceeds without memory enrichment. Debug log / subtle toast; no persistent notification. |
| 4 | **TTS Synthesis / Audio Decode Failure** | Kokoro ONNX or Edge TTS drops clause or fails decoding | `Degraded` | `NonActionable` | *No change* | Turn text preserved on screen; audio playback dropped. Ephemeral Toast ("Voice output issue; text preserved"). |
| 5 | **Remote LLM Rate Limit (429 / 503)** | Upstream cloud provider quota or timeout | `TurnAborted` | `NonActionable` (transient) | `Ready` | Ephemeral Toast ("Provider busy; please speak again"). |
| 6 | **STT Audio Buffer Dropped / VAD Glitch** | Ring buffer overflow or audio anomaly | `TurnAborted` | `NonActionable` | `Ready` | Ephemeral Toast ("Did not catch that; please repeat"). |
| 7 | **System Prompt Exceeds Context Window** | Base system prompt alone exceeds `context_window` | `TurnAborted` | `Actionable` | `Ready` | Ephemeral Toast + Persistent Notification ("System prompt exceeds context window. Increase context_window in Settings.") |
| 8 | **LLM Decode Error (`NoKvCacheSlot`)** | Context preflight guard catch in `generate.rs` | `TurnAborted` | `Actionable` | `Ready` | Ephemeral Toast + Persistent Notification ("Turn exceeded context window.") |
| 9 | **Provider 401 Unauthorized** | Missing or invalid API key | `SessionHalted` | `Actionable` | `Error` | Ephemeral Toast + Persistent Notification ("API Key invalid for provider. Update in Settings.") |
| 10 | **Audio Device Disconnected** | CPAL output device unplugged | `SessionHalted` | `Actionable` | `Error` | Ephemeral Toast + Persistent Notification ("Audio device disconnected. Reconnect device.") |
| 11 | **Model Weights Missing / Corrupted** | Local GGUF or ONNX file missing on disk | `SessionHalted` | `Actionable` | `Error` | Ephemeral Toast + Persistent Notification ("Model files missing. Check Models tab.") |

### 3. Context Budgeting & Personal Share Guard (15%)
- Enforces `DEFAULT_MEMORY_MAX_PERSONAL_SHARE = 0.15` on `ConversationManager::set_identity_facts` and `load_identity_into_system_prompt`:
  $$\text{identity\_budget} = \lfloor \text{context\_window} \times \text{settings.memory.max\_context\_share} \rfloor$$
- Bounded newest-first: Facts are loaded in reverse chronological order until the token estimate reaches `identity_budget`. Excess facts are omitted from the active system prompt, preventing llama.cpp `NoKvCacheSlot` failures before decode begins.

### 4. Panic Containment Architecture
- **Global Panic Hook:** `vox_lib::run()` registers `std::panic::set_hook` to log thread backtraces at `FATAL` and write an emergency crash log to `~/.vox/crash_reports/`.
- **Worker Thread Isolation:** Background actor threads (`LlmWorker`, `VadActor`, `TtsActor`, `AudioEngine`) wrap their turn processing loops in `std::panic::catch_unwind(AssertUnwindSafe(...))`. Unhandled unwinds are intercepted, logged, and converted into `VoxEvent::Error` (`TurnAborted` or `SessionHalted`) rather than killing the process.


