# Backend Review — State/Event, Wiring, Turn-ID, Tech Debt & Style

> **Audience:** Internal — backend-engineers, QA, system architect  
> **Date:** 2026-08-30  
> **Scope:** `app/src-tauri/src/**` (143 Rust files, single crate `vox_lib`), validated against `AGENTS.md §2.2`, `core/constants.rs`, `core/events.rs`, `core/state.rs`, `services/pipeline/router.rs`, `.agents/rules/backend-style-guide.md` + `backend-engineer.md`, `docs/plans/phase10/pipeline_orchestration_spec.md`, `memory_formatting_context_assembly_spec.md`  
> **Method:** Read-only source audit (no subagents), `grep -rn` + full file reads, `cargo check --all-targets` and `cargo clippy --all-targets` (0 warnings, 0 errors, `dev` profile 1.50s/12.84s), manual trace of every `VoxEvent` emit site and every `turn_id` allocation site. `clippy` is green, so this report focuses on *semantic* wiring / invariant drift that the compiler does not diagnose (pub functions / dead enum variants / logical seams).  
> **Convention:** Every claim cites `path:line`. No invented code.  
> **Non-goals:** Frontend `app/src/` audit (out of scope), model weight evaluation, perf benchmarking.  
> **SSOT:** `core/state.rs:138` (`PipelineAtomics`), `core/events.rs:1` (`VoxEvent`), `services/pipeline/mod.rs:69` (`transition`), `core/constants.rs:5` (lifecycle events).

---

## 0. Executive Summary

| Bucket | Severity | Count | Headline |
|---|---|---|---|
| **Wiring — PTT buffers unwired (dead `ingest_audio` seam)** | **P0 — functional break** | 3 buffers | Modular PTT, Realtime PTT, Dictation PTT all read `PTT_BUFFER`/`DICTATION_BUFFER` that are never written. `ingest_audio` seams `modular/ptt.rs:24`, `realtime/ptt.rs:21`, `dictation.rs:38` have **zero call sites** (`grep` returns no hits outside their own `pub fn`). VAD `WindowedValidation` `vad/actor.rs:370` never forwards mic audio to those buffers; `audio_sink` `vad/actor.rs:45` is dead (`None` forever, no `SetAudioSink` command since purge `5.13`). Result: `ptt_stop` always sees empty `raw_samples` → transitions `Ready` without STT. Manual PTT is silently broken in all three modes. |
| **Wiring — `audio_sink` / `RealtimeEngine` dead surface** | P1 | 5 symbols | `VadActorState::audio_sink` `vad/actor.rs:45,83,466`, `RealtimeEngine::{activity_start,activity_end,is_connected,last_activity_time}` `realtime/engine.rs:105,115,125,134` and trait defaults `realtime/mod.rs:81,83,85,89` impl'd but never called. `next_turn()` `core/state.rs:242` + `cancel_current_turn()` `core/state.rs:249` — dead since introduced in `5.13` (no callers). |
| **State invariant — `is_*_loaded` flag bag** | P1 — spec violation | 9 atomics + 6 telemetry shadows | `AppState::{is_llm_loaded,is_tts_loaded,is_stt_loaded,is_vad_loaded,is_embedder_loaded,is_query_classifier_loaded,is_intra_edge_classifier_loaded,is_inter_edge_classifier_loaded,is_translit_loaded}` `core/state.rs:289-297` plus `is_dictation_enabled` `core/state.rs:288` and `TelemetryState::is_private_mode/is_db_healthy` `core/state.rs:341-342` duplicate settings that already exist in `VoxSettings` and in `InteractionState`/`DictationState`. Direct violation of `AGENTS.md §2.2`: *"Model readiness must NEVER be modeled as a flat bag of loose `is_<model>_loaded` atomics in `AppState`."* Same bag mirrored into `monitoring/snapshot.rs:53` / `collector.rs:154`. |
| **Event pump — lifecycle events outside allowlist** | P1 | 12 emit sites | §2.2 allowlist is `state_changed` (incl. `dictation_state_changed`) + streaming payloads `transcript_partial/final`, `llm_token`, `pipeline_error`. Actual emits also include `runtime_booting/ready`, `model_loading/ready/failed`, `theme-changed`, `settings-updated`, `cpu_governor_warning`, `model_setup_complete`, `toggle_hud`, `mode_changed`, `optional_model_*`, `dictation_transcript_copied`, etc. (`lib.rs:79,333,350`, `core/engine.rs:77,133,177,261`, `services/llm/actor.rs:32,136`, `services/tts/actor.rs:33`, `ipc/settings/mutation.rs:246,264,290`, `ipc/setup.rs:68,97,378,385`, `monitoring/telemetry_emitter.rs:24`, `services/dictation/output_router.rs:36,64,86`, `services/dictation/hotkey.rs:32,74`, `ipc/settings/health.rs:417-522`). Whether this is drift or intentional needs a spec decision. |
| **Turn-ID fragmentation** | P1 | 4 producers | Canonical SSOT `PipelineAtomics::next_turn_id()` `core/state.rs:232` exists, but `VadActor::handle_speech_start` `vad/actor.rs:239` does its own `fetch_add`, and `realtime/providers/{gemini_live.rs:766,deepgram_live.rs:598}` keep a second monotonic inside `current_server_turn_id` shadowing the global counter. `modular/ptt::ptt_stop` `modular/ptt.rs:225`, `realtime/ptt::ptt_start` `realtime/ptt.rs:191`, `dictation::handle_hotkey_press` `dictation.rs:64` correctly use `next_turn_id/peek_turn_id`, while `modular/passive::on_speech_start` `modular/passive.rs:164` calls `renew_turn_token()` without allocating a new ID (intentional barge-in?) — fragmented semantics. |
| **Blocking on Tokio (`blocking_recv` + `blocking_lock` from `async` command)** | **P0** | 3 sites | `modular/ptt::ptt_stop` `modular/ptt.rs:189` `rx.blocking_recv()` + `realtime/ptt::ptt_stop` `realtime/ptt.rs:218` same, both `pub fn` called from `#[tauri::command] async fn ptt_stop` `ipc/pipeline/assistant.rs:194` — blocks a Tokio worker. `modular/ptt::ptt_stop` `modular/ptt.rs:173` `state.engine.blocking_lock()` also from that async path. Dictation `dictation.rs:91` correctly `await`s `state.engine.lock().await` + `timeout(... rx).await` (clean). |
| **`_` prefix masking / empty handlers** | P1 | 4 | `modular/ptt::on_speech_start/on_speech_end` `modular/ptt.rs:262,265` are `fn(_app,_state)` empty no-ops (silencing unused while staying in the router `match`). `tts/providers/mod.rs:39,42` `fn set_quality_steps(&self, _steps)` etc. are trait defaults, arguably acceptable but technically violate zero-`_` rule. `window_customizer.rs:40` `fn webview_created(&mut self, _webview)` same. |
| **`#[allow]`** | — | 0 found | `grep -rn "#\\[allow"` returns nothing — compliant. |
| **Empty/error swallowing** | — | 0 | No `let _ = tx.send` pattern; all sends log on `Err`. Compliant. |

`cargo clippy --all-targets` is clean (0 warnings) — the items above are semantic / spec drift, not lint violations.

---

## 1. State, Event & Flag-Bag Invariants (`AGENTS.md §2.2`)

### 1.1 InteractionState / DictationState — SSOT respected (no synthetic pipeline booleans)

`PipelineAtomics::state()` `core/state.rs:189` and `dictation_state()` `core/state.rs:203` are the canonical queries. Grep for banned `is_sleeping|is_engaged|is_recording|is_speech_detected|is_earshot|is_idle|is_assistant|is_passive` returns **no pipeline-level synthetic getters** — the purge in `5.12` held. The few hits of `is_speech`/`is_speech_detected` are local `bool` locals (`vad/actor.rs:335`, `realtime/ptt.rs:229`, `modular/ptt.rs:194`), not atomics, which is acceptable. `VadBackend::is_above_noise_gate()` `vad/mod.rs:68` is the intended polymorphic query.

### 1.2 Model-readiness flag bag — **violation**

`AppState` `core/state.rs:288-298`:

```rust
pub is_dictation_enabled: Arc<AtomicBool>,
pub is_llm_loaded: Arc<AtomicBool>,
pub is_tts_loaded: Arc<AtomicBool>,
pub is_stt_loaded: Arc<AtomicBool>,
pub is_vad_loaded: Arc<AtomicBool>,
pub is_embedder_loaded: Arc<AtomicBool>,
pub is_query_classifier_loaded: Arc<AtomicBool>,
pub is_intra_edge_classifier_loaded: Arc<AtomicBool>,
pub is_inter_edge_classifier_loaded: Arc<AtomicBool>,
pub is_translit_loaded: Arc<AtomicBool>,
```

All mirror either `VoxSettings` (`dictation.enabled`, `llm.active`, `tts.active`, `stt.active`, `vad.vad_backend`, `memory.*`) or transient load state that should live inside each service's actor handles / a single typed enum, not as parallel atomics on `AppState`. `monitoring/collector.rs:154` + `snapshot.rs:53` duplicate the same bag into telemetry DTOs. `core/engine.rs:217,254,309-312` + `services/pipeline/modular/mod.rs:34,59` + `services/llm/actor.rs:29` / `tts/actor.rs` / `stt/actor.rs:26` / `vad/actor.rs:215` each take a clone of one of those atomics — spreading the violation. AGENTS §2.2 explicitly bans this pattern.

`TelemetryState::is_private_mode` `core/state.rs:342` duplicates `settings.history.private_mode` (`core/state.rs:355` copies it once at boot via `store`, but thereafter drifts — `ipc/settings/mutation.rs:180` keeps them in sync via a second `store`, which is the flag-bag pattern the spec forbids). `is_db_healthy`/`is_private_mode` are legitimate health/privacy telemetry signals, but the spec names `is_private` as a banned example — needs a spec-level carve-out or rename to `privacy_mode: PrivacyMode` enum.

**Uncertain (flagged):** Whether the architects *intend* to keep `is_private_mode` as a cross-cutting telemetry atom (consumer: `persistence/worker.rs:102` gates private-mode writes). If so, AGENTS §2.2 wording should be amended to whitelist `TelemetryState` health flags. Current spec says zero tolerance, so I flag it as violation.

### 1.3 `transition()` is the sole lifecycle pump — **mostly compliant, with documented exceptions**

`services/pipeline/mod.rs:69` `transition()` `set_state` + `emit_to(target, EVENT_STATE_CHANGED, ...)` `mod.rs:79` is used consistently across all four domain modules. Grep shows **no module manually emits `speech_start/end`, `playback_started/finished`, `session_started/ended`, `ptt_status`** — the purge in `5.12` holds. The only lifecycle-adjacent emits are:

- `transition()` itself (`EVENT_STATE_CHANGED` = `"state_changed"` `pipeline/mod.rs:5`) — canonical.
- `EVENT_DICTATION_STATE_CHANGED` `"dictation_state_changed"` `pipeline/mod.rs:10` emitted exclusively from `pipeline/dictation.rs:24` — this is a second state pump for `DictationState`, analogous to `transition()` but not named `transition`. Spec allows it (`§2.2` lists `DictationState` SSOT), so treated as compliant.
- Everything else is streaming payloads (`transcript_partial/final` `pipeline/mod.rs:6-7`, `llm_token` `mod.rs:8`, `pipeline_error` `mod.rs:9`) — compliant. See §2 below for the non-compliant emits.

---

## 2. Event Map — What Is Actually Emitted

### 2.1 Allowlist per `AGENTS.md §2.2`

> *"The ONLY other IPC events permitted are streaming data payloads: `transcript_partial`, `transcript_final`, `llm_token`, `pipeline_error`."* plus `state_changed` (and by extension `dictation_state_changed`).

`services/pipeline/mod.rs:5-10` canonically defines those 6.

### 2.2 Actual emit inventory (`grep -rn "\.emit"`)

| Event string | Emit sites | Verdict |
|---|---|---|
| `state_changed` | `pipeline/mod.rs:79` (all domains) | ✅ canonical |
| `dictation_state_changed` | `pipeline/dictation.rs:24` | ✅ (Dictation SSOT) — **flagged as implicit exception** (spec lists only `state_changed` by name; `dictation_state_changed` is introduced in code) |
| `transcript_partial` | `pipeline/modular/passive.rs:197`, `ptt.rs:272`, `dictation.rs:170`, `realtime/passive.rs:265` | ✅ |
| `transcript_final` | `modular/passive.rs:220`, `ptt.rs:295`, `dictation.rs:211`, `realtime/passive.rs:289`, `realtime/ptt.rs:310` | ✅ |
| `llm_token` | `modular/passive.rs:320`, `ptt.rs:393`, `realtime/passive.rs:309`, `realtime/ptt.rs:328` | ✅ |
| `pipeline_error` | `modular/passive.rs:423`, `ptt.rs:496`, `dictation.rs:230`, `realtime/passive.rs:365`, `realtime/ptt.rs:366` | ✅ |
| `runtime_booting` / `runtime_ready` | `lib.rs:79`, `lib.rs:350` (`core/constants.rs:20-21`) | ⚠️ **outside allowlist** — boot lifecycle |
| `model_loading` / `model_ready` / `model_failed` | `core/engine.rs:77,86,133,177,261`, `services/llm/actor.rs:32,136,144`, `services/tts/actor.rs:33` (`constants.rs:22-24`) | ⚠️ **outside allowlist** — model lifecycle |
| `cpu_governor_warning` | `lib.rs:333` | ⚠️ |
| `theme-changed` / `settings-updated` | `ipc/settings/mutation.rs:246,264,290`, `ipc/tray.rs:289` | ⚠️ |
| `model_setup_complete` / `optional_model_*` | `ipc/setup.rs:68,97,378,385` | ⚠️ |
| `toggle_hud` / `mode_changed` | `ipc/tray.rs:41,49,283,286` | ⚠️ |
| `dictation_transcript_copied` | `ipc/pipeline/dictation.rs:34` | ⚠️ (but `dictation_state_changed` already exists) |
| `model_loading/ready/failed` (health probes) | `ipc/settings/health.rs:417,432,447,466,486,502,512,522` | ⚠️ |

**Judgement:** The core pipeline (`services/pipeline/**`, `core/engine.rs`, `services/{llm,tts}/actor.rs`) mixing `model_*`/`runtime_*` lifecycle emits with the pipeline event pump is **spec drift**. These are read by the frontend `setup`/`health` UIs, so removing them would break UX. The spec needs an explicit carve-out ("system/model lifecycle events are allowed from `core/engine` + `setup` IPC, not from pipeline domains"). As-written, they are violations.

### 2.3 Frontend alignment (not in scope, but noted)

`pipeline/mod.rs:62` `target_window` routes `Assistant→"main"`, `Dictation→"tray"` — all pipeline emits respect it (`emit_to(WINDOW_MAIN/WINDOW_TRAY)`). No rogue `emit` to wrong window found. Frontend `isEngaged/isSleeping` re-creation would be caught in a TS audit, out of scope here.

---

## 3. Turn-ID — Fragmented Allocation (Spec §2.2 "Centralized Turn Generation")

Canonical SSOT is declared at `core/state.rs:231-250`:

```rust
pub fn next_turn_id(&self) -> u32 { self.turn_id.fetch_add(1, Relaxed) + 1 }   // :232
pub fn peek_turn_id(&self) -> u32 { self.turn_id.load(Relaxed) }               // :237
pub fn next_turn(&self) -> (u32, CancellationToken) { (self.next_turn_id(), self.renew_turn_token()) } // :242
pub fn cancel_current_turn(&self) { self.turn_token.lock().cancel(); }          // :249
pub fn renew_turn_token(&self) -> CancellationToken { self.turn_epoch.fetch_add(1); guard.cancel(); ... } // :222
```

Actual allocation map:

| Call site | Mechanism | Turn-ID source | Correct? |
|---|---|---|---|
| **`VadActor::handle_speech_start`** `vad/actor.rs:239` | `turn_id_atomic.fetch_add(1)+1` directly on the shared `Arc<AtomicU32>` | **bypasses `PipelineAtomics`** | ❌ fragments — this is the *passive autonomous* path (VAD detects onset). Spec says IDs must come from `next_turn_id()`. VAD is an OS thread actor that legitimately needs to allocate, but it should call `state.pipeline.next_turn_id()` (or receive a handle to `PipelineAtomics`). Today it holds a raw `Arc<AtomicU32>` `vad/actor.rs:217` and increments it itself. |
| **`modular/ptt::ptt_start`** `modular/ptt.rs:132` | **no allocation** — `ptt_start` just transitions `Listening`, does not bump `turn_id` | **flagged as uncertain** — `ptt_stop` `modular/ptt.rs:225` does `next_turn_id()` at *stop* time, not at *start*. So the turn's ID is unknown during `Listening`. Frontend cannot correlate `ptt_start` with the eventual `transcript_*` `turn_id`. |
| **`modular/ptt::ptt_stop`** `modular/ptt.rs:225` | `state.pipeline.next_turn_id()` | ✅ correct call, but **timing**: double `transition(Thinking)` (line 162 + line 227), see §5.4 |
| **`realtime/ptt::ptt_start`** `realtime/ptt.rs:191` | `next_turn_id()` at start (correct), but also `renew_turn_token()` `realtime/ptt.rs:180` separately | ⚠️ allocates ID at *start*, unlike modular PTT (inconsistency). Uses **two atomics** (`next_turn_id` + `renew_turn_token`) not the combined `next_turn()`. |
| **`realtime/ptt::ptt_stop`** `realtime/ptt.rs:205` | `peek_turn_id()` (reuses start's ID) | ✅ pairs with `ptt_start` — but modular PTT does not. **Inconsistent contract between modular and realtime PTT.** |
| **`dictation::handle_hotkey_press`** `dictation.rs:64` | `next_turn_id()` at press | ✅ |
| **`dictation::handle_hotkey_release`** `dictation.rs:83` | `peek_turn_id()` | ✅ pairs with press |
| **`modular/passive::on_speech_start`** `modular/passive.rs:158` | **no allocation** — relies on VAD actor's `fetch_add` | ✅ if VAD is the allocator; but then who owns the ID? The router doesn't log it. |
| **`modular/passive::on_transcript_final`** etc. | receives `turn_id` from `VoxEvent` | ✅ event-carried |
| **`realtime/providers/gemini_live::State::current_or_new_turn_id`** `gemini_live.rs:762` | `self.turn_id.fetch_add(1)+1` + `current_server_turn_id` shadow | ❌ **second monotonic** shadowing the global `PipelineAtomics.turn_id`. The provider keeps `current_server_turn_id: Option<u32>` as a per-session cursor and falls back to `turn_id.load()` for errors. Deepgram provider (`deepgram_live.rs:598`) does the same. This means there are **three** turn-ID counters: `PipelineAtomics.turn_id`, `GeminiLive::turn_id` (same Arc but managed separately), `current_server_turn_id`. |
| **`ipc/pipeline/test_clip`** `test_clip.rs:136` | `next_turn_id()` | ✅ |
| **`next_turn()` / `cancel_current_turn()`** `core/state.rs:242,249` | defined | ❌ **dead** — `grep -rn "next_turn(\|cancel_current_turn"` finds zero callers (outside their own definitions). The spec `5.13` claimed migration to `next_turn()`/`cancel_current_turn`, but the codebase never adopted them. |
| **`COMPACTION_SENTINEL_TURN_ID`** `constants.rs:264` = `999_999` | used in `services/memory/compaction/runner.rs` (sentinel for background LLM) | ✅ isolated |

**Summary:** Three allocation strategies coexist (VAD-direct, PTT-stop-time, PTT-start-time + provider-shadow). `next_turn()` is dead. The spec's "single monotonic `next_turn_id()` at turn boundary" needs a clarified definition of *where* the boundary is for PTT (press vs release) and whether VAD is allowed to be an allocator.

**Uncertain (flagged):** The intended PTT contract. `pipeline_orchestration_spec.md §7.2` sketches `ptt_start: Bump turn_id` then `ptt_stop: send buffer to STT(Final, turn_id)`. The code for modular PTT bumps at `ptt_stop`, not `ptt_start` — which is the correct reading? Both realtime PTT and dictation bump at `ptt_start` (and `peek` at stop), so modular PTT is the outlier. This inconsistency is likely a bug, not a design choice, but I cannot be certain without the author's intent.

---

## 4. Wiring, Dead Code & Enum Variants

### 4.1 P0 — PTT ingestion seam is dead (the headline finding)

**Files:**

- `services/pipeline/modular/ptt.rs:15` `static PTT_BUFFER: Mutex<Vec<f32>>`
- `services/pipeline/modular/ptt.rs:24` `pub fn ingest_audio(chunk: &[f32], state: &AppState) { if state==Listening { PTT_BUFFER.lock().extend... } }`
- `services/pipeline/modular/ptt.rs:31` `pub fn get_buffer_len() -> usize`
- `services/pipeline/realtime/ptt.rs:14` `static REALTIME_PTT_BUFFER: Mutex<Vec<i16>>` + `:21` `ingest_audio` + `:32` `get_buffer_len`
- `services/pipeline/dictation.rs:11` `static DICTATION_BUFFER: Mutex<Vec<f32>>` + `:38` `ingest_audio` + `:45` `get_buffer_len`

**Evidence:** `grep -rn "ingest_audio\|get_buffer_len" app/src-tauri/src --include="*.rs" | grep -v "pub fn"` returns **nothing**. No file calls these seams. The spec (`backend-style-guide.md §9.2`) *requires* seams like `ingest_audio(&[f32])` so `VadActor` can feed buffers and tests can observe them — but the VAD actor never does:

- `services/vad/actor.rs:370` `process_windowed_validation` — the `WindowedValidation` path (PTT) does VAD prediction + boundary sampling into `window_*` fields and `pre_roll_buffer`, but **never pushes to any PTT buffer**.
- `services/vad/actor.rs:398` `process_stream_passthrough` — only forwards `f32→i16` to `realtime_tx` (set only for realtime via `VadCommand::StartRealtime` `:161`).
- `services/vad/actor.rs:466` `if let Some(ref sink) = state.audio_sink { sink.try_send(...) }` — but `audio_sink` is `None` forever (default `VadActorState::new` `:83`, never assigned; `VadCommand::SetAudioSink` was purged per `AGENTS.md 5.13` and no replacement `audio_sink: Option<Sender>` setter exists).

**Consequence:** `modular/ptt::ptt_stop` `modular/ptt.rs:164` `let raw_samples = PTT_BUFFER.lock().clone()` is always empty → early return `Ready` (`:167-171`). Same for `realtime/ptt::ptt_stop` `:206` and `dictation::handle_hotkey_release_with_sender` `:82`. **PTT capture produces no audio, hence no `SttCommand::Final`, no transcription — the feature is silently broken.** Realtime passive and VAD-driven passive still work (they stream continuously), so this regression is easy to miss in passive-only testing.

**Fix shape:** Wire `VadActor` → `ingest_audio` for the `WindowedValidation` path, or replace the global `PTT_BUFFER` static with a `VadCommand::StartWindowValidation` that returns the trimmed buffer directly (which is already computed via `window_*` boundaries — the validation result already contains `speech_start_sample`/`speech_end_sample`). The current `ptt_stop` validation does trim `raw_samples[start..end]` `:208-215`, so re-wiring is minimal. The transient `PTT_BUFFER` static itself violates `backend-style-guide.md §9.2` isolation anti-pattern and should be owned by `AppState` or passed as a handle.

**Confidence:** High. Zero call sites is definitive.

### 4.2 Dead fields / commands

| Symbol | Location | Evidence |
|---|---|---|
| `VadActorState::audio_sink` | `vad/actor.rs:45` field, `84` init `None`, `466` send | Written nowhere. Dead. Purge or re-introduce `SetAudioSink`. |
| `VadActorState::realtime_tx` | `vad/actor.rs:44` | Alive (set via `VadCommand::StartRealtime`). Keep. |
| `VadActorState::pcm_scratch` | `vad/actor.rs:46` | Alive (passthrough path). Keep. |
| `PipelineAtomics::next_turn()` | `core/state.rs:242` | Dead (0 callers). |
| `PipelineAtomics::cancel_current_turn()` | `core/state.rs:249` | Dead (0 callers). |
| `RealtimeEngine::{activity_start,activity_end,is_connected,last_activity_time}` | `realtime/engine.rs:105,115,125,134` | Dead (0 callers). Providers impl `activity_*` etc. but engine facade is never used. |
| `RealtimeSession::{is_connected,last_activity_time}` defaults | `realtime/mod.rs:85,89` | Dead as facade; alive inside providers (Gemini/Deepgram keep `last_activity_time` atomics themselves). |
| `modular/ptt::on_speech_start/on_speech_end` | `modular/ptt.rs:262,265` | Empty no-ops with `_app,_state` to silence unused. Router still dispatches `SpeechStart/SpeechEnd` into them (`ptt.rs:523`). These VoxEvents are *spurious* in PTT mode (VAD is in `WindowedValidation`, not `ContinuousSegmentation`, so `SpeechStart/End` should never fire — but they currently do nothing and mask the fact that the router dispatches them). Either remove the dispatch arms or make them log. |
| `realtime/ptt::on_speech_start/on_speech_end` | `realtime/ptt.rs:295,304` | Same pattern — `on_speech_start` calls `realtime_barge_in`, `on_speech_end` is `drop(audio)` no-op. Keep or drop explicitly. |
| `pipeline/modular/ptt::CHUNKER`, `CURRENT_ASSISTANT_RESPONSE`, `CURRENT_USER_TRANSCRIPT` (LazyLock statics) | `modular/ptt.rs:16,18,20` | Alive (TTS tokeniser). But duplicated per domain (passive, PTT, dictation each have own `CHUNKER` static — §4.3 notes this duplicates `TtsClauseChunker` logic). |

### 4.3 VoxEvent enum coverage

`core/events.rs:2` `VoxEvent` has 13 variants. Router `services/pipeline/router.rs:15` dispatches on `RoutingContext` splits:

- `modular/passive.rs:448` handles 12 variants (`SpeechStart`..`Error` + `_` catch-all) — covers `Shutdown` via top-level `router.rs:45` break.
- `modular/ptt.rs:516` handles 8 + `_`; ignores `Tts*`, `Playback*`? Actually handles `TtsChunk`, `TtsFinished`, `PlaybackStarted/Finished`, `SpeechStart/End` (as no-ops). So somewhat symmetrical.
- `realtime/passive.rs:385` handles 8 + `_` (missing `TtsChunk/TtsFinished` — correctly, realtime providers synthesize server audio, not TTS).
- `realtime/ptt.rs:386` handles 7 + `_` (same rationale).
- `dictation.rs:251` handles 6 + `_` (only `Speech*`, `Transcript*`, `Cancelled`, `Error`).

All have `_ => {}` catch-alls, so **compilation won't warn on unhandled variants**. Recommend exhaustive `match` per domain (or a central `ensure_handle_event_covers(VoxEvent)` test). `Shutdown` is handled at `router.rs:45` before `route_event`, so domain `_` there is fine.

`VadCommand` `core/state.rs:104` has 8 variants (`UpdateThreshold/NoiseGate/Mode/AudioMode`, `SetOperationalMode`, `StartWindowValidation`, `StopWindowValidation`, `Shutdown`, `StartRealtime/StopRealtime`). All are matched in `vad/actor.rs:101`. No dead variants.

### 4.4 `is_connected` naming vs AGENTS ban

`realtime/mod.rs:85` `fn is_connected(&self) -> bool` and `realtime/engine.rs:125` `pub fn is_connected(&self) -> bool` are literally the banned name `is_connected` in AGENTS §2.2. However, the ban lists synthetic pipeline booleans ("is_connected, is_idle, is_engaged…"). A WebSocket's `is_connected` is a transport-level query, not a pipeline state alias. **Recommendation:** keep the name but add a comment that it is transport health, or rename to `session_connected()` to avoid lint false positives. Not a violation in spirit, but a literal one.

---

## 5. Blocking, Ownership & Thread Boundaries

### 5.1 The real blocking defect — `blocking_recv` inside Tauri commands

```rust
// modular/ptt.rs:156 pub fn ptt_stop (sync) called from:
#[tauri::command] pub async fn ptt_stop(app, state) -> Result { ... modular::ptt::ptt_stop(&app,&state) } // ipc/pipeline/assistant.rs:194
// inside:
let (tx, rx) = oneshot::channel();
engine.vad_tx.send(StopWindowValidation { response_tx: tx }).is_ok()
rx.blocking_recv().ok() // modular/ptt.rs:189  — BLOCKS Tokio worker
```

Same in `realtime/ptt.rs:218`. The sync `ptt_stop` holds no runtime claim, but it is **invoked inside an `async` Tauri command** (which runs on a Tokio worker thread per `tauri::async_runtime::spawn`). `blocking_recv()` parks that worker. `dictation.rs:95` correctly does `tokio::time::timeout(100ms, rx).await` — the fix is to make `ptt_stop` `async` and `.await` the oneshot, or to `spawn_blocking` the validation wait, or to pre-await outside. Realtime dictation already shows the right pattern.

`state.engine.blocking_lock()` inside those same `ptt_stop` paths (`modular/ptt.rs:173`, `realtime/ptt.rs:214`) has the same shape: a sync `parking_lot::Mutex::blocking_lock()` (actually `AppState::engine` is `tokio::sync::Mutex<Option<VoxEngine>>`; its `blocking_lock()` parks the Tokio worker). From `modular/passive::on_transcript_final` `:245` or `on_llm_token` `:336` this is fine — those run on the **router OS thread** (`services/pipeline/router.rs:38` `std::thread::Builder::new().spawn`), not on Tokio. From `ptt_stop` (inside a Tauri command) it is not.

**Fix:** promote `ptt_stop` (both modular + realtime) to `async`, change `blocking_recv` → `rx.await` with timeout, `blocking_lock` → `lock().await` (or `try_lock` + fallback). `dictation.rs:91` is the reference impl.

### 5.2 Other `blocking_lock` sites — sound

- `modular/passive.rs:245,336,355` `blocking_lock()` — called from `on_transcript_final`/`on_llm_token`/`on_llm_finished`, all invoked via `router.rs:49` `route_event` on the **router dedicated thread** (`vox-router`). Sound: blocking a router thread is fine, it is not a Tokio worker.
- `core/engine.rs:170,301` `lock().await` — canonical `await` (spawned from async `start_audio_engine`/`stop_audio_engine`). Sound.
- `lib.rs:557` `blocking_lock()` — inside `RunEvent::Exit` (shutdown path on Tauri's main thread). Sound.

### 5.3 Lock ordering (`backend-style-guide.md §6` canonical order)

> *Canonical Mutex Lock Order: Strictly acquire `state.engine` before `state.realtime_engine`. Never reversed.*

Audit:

- `realtime/passive::start_session` `realtime/passive.rs:23` `engine.lock().await` (clone), then `:37` `realtime_engine.lock().await` → **engine → realtime_engine** ✅
- `realtime/passive::resume_session` `:138` `engine.lock().await` then `:143` `realtime_engine.lock().await` → ✅
- `realtime/ptt::start_session` `realtime/ptt.rs:40` `engine.lock().await` then `:54` `realtime_engine.lock().await` → ✅
- `realtime/ptt::ptt_start` `realtime/ptt.rs:182` only `engine.try_lock()` then no realtime lock → okay (no inversion opportunity).
- `realtime/session::realtime_barge_in` `realtime/session.rs:39` only `realtime_engine.try_lock()` → invoked from `passive::on_speech_start` etc., which do not already hold `engine` at call time → no inversion.
- `core/engine::stop_audio_engine` `core/engine.rs:301` only `engine.lock().await` → trivial.
- No path does `realtime_engine` then `engine`. **No ordering violation found.**

### 5.4 Actor-Engine separation

`vad/actor.rs` holds `VadBackend` (engine) inside the actor thread but via the split `VadActor{Config,Handles,Channels}` + `VadActorState` model — separation is light but respects the rule (actor owns thread + state machine, engine is `VadBackend` trait). `stt/actor.rs:291` spawns `vox-stt-worker` thread for provider inference, correctly elevates priority — sound. `tts/actor.rs:181` `vox-tts-persistent` and `llm/actor.rs:164` `vox-llm-persistent` likewise dedicated threads (LLM thread creates its own `tokio::runtime::Builder::new_current_thread().enable_all().build()` `:38` and `block_on(provider.generate(...))` `:51` — this is `RemoteTransport`'s tokio work happening on a dedicated thread, not on the App's worker pool, so it satisfies `backend-style-guide.md §6` "Inference on dedicated OS threads"). `llm/actor.rs:30` `is_local` gating for `is_llm_loaded` is part of the flag bag already noted.

---

## 6. Tech Debt Catalogue

| Debt | Location | Why it matters | Confidence |
|---|---|---|---|
| **Global PTT/Dictation buffers as `static Mutex<Vec<…>>`** | `modular/ptt.rs:15`, `realtime/ptt.rs:14`, `dictation.rs:11` | Isolated statics that tests cannot inject/observe, violate `backend-style-guide.md §9.2` ("Module-level statics must never form isolated black boxes"). Also the ingest seam is dead. | High |
| **Per-domain `CHUNKER`/`CURRENT_*` duplication** | `modular/passive.rs:15-20`, `modular/ptt.rs:16-20`, `realtime/passive.rs:14-17`, `realtime/ptt.rs:15-17` | Four copies of `LazyLock<Mutex<TtsClauseChunker>>` + pair of `LazyLock<Mutex<String>>` statics. Should be a single `ConversationChunkState` owned by `AppState`/`ConversationManager`, not cross-domain statics. Causes cross-turn bleed risk if `clear()` races (`modular/ptt::ptt_start` `:146` clears CHUNKER but `passive` may still hold tokens). | High |
| **Dual cancel signals** | `core/state.rs:139` `cancel_flag: Arc<AtomicBool>` vs `:149` `turn_token: Mutex<CancellationToken>` | Both are manipulated on the same transitions (`modular/passive::on_speech_start` `:165` `renew_turn_token()` but not `cancel_flag.store(true)`; `modular/passive::start_session` `:30` only clears `cancel_flag`). The harness `facade.rs` passes `CancellationToken` to `prepare_turn_context` while `PlaybackEngine::ingest_chunk` `:103` checks `cancel_flag`. Two truth sources for one concept. | Medium — the split may be intentional (immediate audio drain vs cooperative LLM cancel), but no comment explains when each is authoritative. |
| **`PlaybackEngine` shares `cancel_flag` with pipeline** | `core/engine.rs:155` `cancel_flag: Arc::clone(&state.pipeline.cancel_flag)` | `PlaybackEngine::cancel()` `playback.rs:121` sets `cancel_flag=true`, which also cancels STT/LLM via the same flag. Coupling playback cancellation to STT/LLM cancellation via one bool may be intentional (barge-in), but means any `playback.cancel()` also aborts generation even when only draining. Flagged as **uncertain design**. | Medium |
| **Hard-coded `SERVICE` strings / sleep delays** | `core/engine.rs:593` `std::thread::sleep(Duration::from_millis(150))` in `RunEvent::Exit` | Arbitrary 150ms join window for worker threads; may truncate persistence flush under load. `monitoring/collector` `system_monitor` 5s intervals are fine. | Low |
| **`is_loaded` propagation redundancy** | `core/state.rs:309-312` `stop_audio_engine` resets 4 of the 9 flags but leaves 5 ONNX flags + `is_dictation_enabled` | Inconsistent teardown (ONNX flags cleared via `unload_all_onnx_models()` `:367` which also touches the atomics via `services/memory/ml/*::is_*_loaded` statics). Two sources of truth for model readiness. | Medium |
| **`ConversationManager::on_speech_start()` called twice in passive** | `modular/passive.rs:174` then `175` `pop_last_user_turn()` | `on_speech_start` followed immediately by `pop_last_user_turn` looks like a workaround for a barge-in race (new turn bumps speculative user turn). No comment. Uncertain whether the pop is meant to undo an optimistic `push_user_turn` that happened elsewhere. | **Flagged uncertain** |
| **`modular/ptt::ptt_stop` double `transition(Thinking)`** | `modular/ptt.rs:162` then `227` | Two `Thinking` transitions in one function (once unconditionally, once before STT dispatch). Second is redundant; may cause duplicate UI state broadcasts. Same in `realtime/ptt::ptt_stop` `realtime/ptt.rs:264` after `cancel_flag` store (actually Thinking again). | Medium |
| **`llm/actor.rs:38-51` runtime `expect`** | `:40` `.expect("Failed to build LLM worker runtime")` | Style bans `unwrap`; `expect` in `src/` is the same class (panics). Should be `?` + error log. | Low |

---

## 7. Style-Guide Compliance (per `backend-style-guide.md`)

| Rule | Result |
|---|---|
| **Constant hierarchy** (`core/constants.rs` → `core/defaults.rs` → domain `mod.rs` → file-top) | ✅ `core/constants.rs:5` ring/telemetry/prompt, `core/defaults.rs` provider/model defaults, `services/vad/mod.rs`, `services/llm/mod.rs`, `services/tts/mod.rs` domain constants. No buried magics found. |
| **`mod.rs` is declarations only** | ✅ `services/pipeline/mod.rs:14-17`, `services/audio/mod.rs`, `services/memory/mod.rs`, `core/mod.rs` — only `pub mod` + constants. `lib.rs:1` is module declarations + Tauri assembly only. |
| **Visibility `pub(crate)` vs `pub`** | ⚠️ Most `pub fn` in `services/pipeline/modular/*`, `realtime/*`, `dictation` are `pub` but consumed only via `router`/`assistant` IPC. Could be `pub(crate)` per guide §2 to limit crate boundary. Minor. |
| **Function 50-line cap** | ⚠️ `modular/passive::on_transcript_final` `modular/passive.rs:210` ~106 lines, `modular/ptt::ptt_stop` ~84 lines, `realtime/passive::start_session` ~94 lines — over soft cap without justification comment. `backend-style-guide §4` expects helper extraction. |
| **Docstrings: one `///` per function, zero inline step comments** | ⚠️ Mixed — many functions have number-step `// Step 1:` inline comments (e.g. `modular/passive.rs:22-70` documented but bodies still carry `// Step N`). Style bans numbered steps unless each is a helper. Minor but systematic. |
| **No toggle functions, no `is_*` flag bags** | ❌ Flag bag (see §1.2). No toggle-function violations found. |
| **Zero `#[allow]`** | ✅ 0. |
| **Zero `_` masking** | ⚠️ 4 (see §4.2). |
| **`?` + `.context` over `let _ =`, no fallback chains** | ✅ No silent fallback chains observed; errors either `?` or `log::warn` on `Err`. |
| **`AppHandle<R: Runtime>`** | ✅ All actors, workers, domain handlers, IPC use `<R: tauri::Runtime + 'static>` or `R: Runtime`. No concrete `AppHandle<Wry>` bindings. |
| **Channels over `Arc<Mutex>`** | ⚠️ PTT digitisation: statics are `parking_lot::Mutex<Vec<f32>>`, not channels. Preferred fix is channel → actor or `AppState`-owned buffer. |

---

## 8. Uncertain Logic — Flagged for Decision (per your ask)

> **Items in this section are not asserted as bugs. I am not confident what the intended behavior is. Each states the observation, the hypotheses, and what decision is needed.**

### U1 — PTT `turn_id` boundary: press vs release

- **Obs:** `realtime/ptt::ptt_start` `:191` bumps `next_turn_id()` at press; `modular/ptt::ptt_stop` `:225` bumps at release; `dictation::handle_hotkey_press` `:64` bumps at press (then `peek` at release `:83`). Three conventions coexist.
- **Hyp A:** Press-time is correct (spec `§7.2` says "`ptt_start` • Bump `turn_id`"). Modular PTT is buggy.
- **Hyp B:** Release-time is correct (avoid allocating on accidental taps/cancelled holds that never produce `Final`). Then realtime PTT + dictation are wasteful.
- **Decision:** Clarify spec §7.2 for each domain and unify. I lean **press-time** (UX: frontend can bind UI to the final `turn_id` at press, ghost-cancel still drains correctly with `Cancelled { turn_id: tid }`): make modular PTT match realtime PTT.

### U2 — PTT audio ingestion path: VAD → buffer vs direct mic → buffer

- **Obs:** `ingest_audio` seams exist but are unwired. VAD's `WindowedValidation` already computes `speech_start_sample/end_sample` boundaries, but the raw audio is never routed to the buffers that `ptt_stop` trims.
- **Hyp A (intended):** Mic → ringbuf → VAD actor (consumes chunk) → `ingest_audio(buffer[chunk])` should be called inside `process_windowed_validation`’s `window_active` branch.
- **Hyp B (intended refactor):** Delete the static buffers entirely; have `StopWindowValidation` return `Vec<f32>` directly from VAD's `pre_roll_buffer + window` buffers, avoiding globals. Current implementation half-did this (VAD returns `VadValidationResult` with boundaries, then `ptt_stop` trims a *separate copy* of the buffer).
- **Decision:** Wire (A) quickly if short-term, but (B) is architecturally cleaner per `§9.2` and eliminates the statics.

### U3 — `audio_sink: Option<Sender<Vec<f32>>>` in `VadActorState`

- **Obs:** Dead field, no setter. Was `VadCommand::SetAudioSink` deleted in `5.13` but the field kept.
- **Hyp A:** It was the previous PTT ingestion sink; the refactor replaced it with the static-buffer seam but forgot to delete the field.
- **Hyp B:** It is reserved for a future "audio sink" feature (e.g. test-clip injection).
- **Decision:** Delete it (purge) and its `if let Some(ref sink)` branch `:466` if (A); or reintroduce a setter if (B). Keeping dead state is not free (confusing).

### U4 — Dual cancel signals (`cancel_flag` vs `turn_token`)

- **Obs:** `cancel_flag` (`AtomicBool`) gates hot-path (`PlaybackEngine::ingest_chunk` `:103`, `STT actor` `:119`) and is set by `pause_session`/`end_session`/`ptt_cancel`. `turn_token` (`CancellationToken`) is passed to `prepare_turn_context` and LLM generation (`LlmCommand::Generate` `llm/actor.rs:13`). `renew_turn_token()` `:222` increments `turn_epoch` and cancels the old token, but *does not* touch `cancel_flag` (some call sites set `cancel_flag=false` manually, e.g. `realtime/ptt::ptt_stop` `:263`).
- **Hyp A:** The two signals are meant to be distinct — `cancel_flag` = "flush buffers now", `turn_token` = "abort this turn's async LLM/memory work".
- **Hyp B:** They are vestigial duplication from the `5.13` migration to `CancellationToken` and should be unified (`CancellationToken::cancel()` drives everything, `is_cancelled()` replaces `cancel_flag.load()`).
- **Decision:** Document the intended split. If duplication, remove `cancel_flag` from any call site that already has a `CancellationToken` (e.g. LLM dispatch).

### U5 — `modular/passive::on_speech_start` double conversation call

- **Obs:** `modular/passive.rs:174` `conversation_manager.lock().on_speech_start()` then `:175` `pop_last_user_turn()`. `ConversationManager::on_speech_start` is not trivial (it advances epoch/slots). Immediately popping suggests the new-turn speculative `user_text` was pre-pushed optimistically.
- **Hyp A:** This is barge-in bookkeeping — when `Thinking/Speaking → Listening` the next `TranscriptFinal` would otherwise duplicate the previous user turn; popping corrects it.
- **Hyp B:** It papers over a race where `prepare_turn_context` already pushed a speculative user turn; popping just unwinds it.
- **Decision:** Add a comment documenting the invariant; consider coalescing into `conversation_manager.restart_turn()` instead of two-call sequence. Low risk.

### U6 — Realtime providers keep their *own* turn counter

- **Obs:** Gemini/Deepgram each clone `turn_id: Arc<AtomicU32>` from `PipelineAtomics` but manage `current_server_turn_id: Option<u32>` `:758` and methods `current_or_new_turn_id`/`peek_or_current_turn_id` `:762`. They also call `fetch_add` themselves rather than through `PipelineAtomics::next_turn_id`.
- **Hyp A:** Providers need server-side turn affinity (one `turn_id` survives reconnects via `sessionResumption`). So they keep a logical "server turn" distinct from the global pipeline turn.
- **Hyp B:** It is leftover from the pre-SSOT design before `PipelineAtomics` existed; providers should instead be *consumers* of `PipelineAtomics.turn_id` without their own cursor.
- **Decision:** If (A), rename `current_server_turn_id` → `server_turn_cursor` with a comment that it is not the pipeline's `turn_id` and can diverge. If (B), delete the shadow and route through `PipelineAtomics::next_turn_id`. I lean (A) is plausible — Gemini's `server_turn_complete` and reconnection `sessionResumptionUpdate` do suggest server affinity — but the naming collision is still a hazard; rename regardless.

### U7 — `model_*` / `runtime_*` lifecycle events: spec or drift?

- **Obs:** §2.2 says only `state_changed` + streaming payloads are permitted; the code emits `model_loading/ready/failed`, `runtime_booting/ready`. Frontend `Health` + `Setup` panes subscribe to them.
- **Hyp A:** They are legitimate system events and §2.2 is too narrow — the allowlist should be expanded to `system lifecycle` events from `core/engine` + `setup`, not from pipeline domains.
- **Hyp B:** They should be folded into `state_changed` payloads (e.g. `InteractionState::Error { reason: ModelFailed }`) or a `system_status` event.
- **Decision:** Choose one — do not leave code and spec contradictory. I recommend (A) with a scoped carve-out annotating which modules may emit lifecycle events.

---

## 9. Recommended Sprint Backlog (buildable in `build` mode)

Each sprint is a *single rule × N files* per your `create-sprints` usage. Ordered by blast radius (wire → blocking → flag-bag → cleanup).

| Sprint | Title | Files (primary) | Acceptance |
|---|---|---|---|
| **S01** | Wire PTT `ingest_audio` seam (P0) | `vad/actor.rs:370`, `pipeline/modular/ptt.rs:24`, `pipeline/realtime/ptt.rs:21`, `pipeline/dictation.rs:38` | `ptt_stop` receives non-empty `raw_buffer` under manual PTT hold; add `test_ingest_audio_seam_integration_test` driving `ingest_audio` without hardware. Decide U2 (channel vs buffer) before coding. |
| **S02** | Delete dead `audio_sink` | `vad/actor.rs:45,83,466` | Clippy clean, no `_sink` references. |
| **S03** | Promote `ptt_stop` → `async`, remove `blocking_recv`/`blocking_lock` | `pipeline/modular/ptt.rs:173,189`, `pipeline/realtime/ptt.rs:214,218` | No `blocking_recv`/`blocking_lock` from `async fn` after `grep`; `cargo check` passes; `ptt_stop` aligns with `dictation.rs:91` pattern (timeout 100ms). |
| **S04** | Unify turn-ID allocation via `next_turn_id/peek_turn_id` | `vad/actor.rs:239`, `realtime/providers/gemini_live.rs:762`, `deepgram_live.rs:598`, `pipeline/modular/ptt.rs:156` | No raw `fetch_add` on `turn_id_atomic` outside `core/state.rs:232`; providers renamed `server_turn_cursor`; U1 decision applied consistently. |
| **S05** | Re-hydrate `next_turn()`/`cancel_current_turn()` or delete | `core/state.rs:242,249` | Either callers adopted (`realtime/ptt::ptt_start` uses `next_turn()`) or both fns removed and spec `5.13` amended. |
| **S06** | Collapse `is_*_loaded` flag bag → typed readiness | `core/state.rs:288-298`, `monitoring/collector.rs:154`, `snapshot.rs:53`, `core/engine.rs:217,254,309` | Single `ModelReadiness { stt,vad,llm,tts, onnx: OnnxReadiness }` enum/struct queried by state, not parallel atomics. `AGENTS.md §2.2` re-checked. |
| **S07** | Remove/rate-limit `_` masks, empty PTT handlers | `pipeline/modular/ptt.rs:262,265`, `tts/providers/mod.rs:39,42`, `window_customizer.rs:40` | Zero `fn _param` outside RAII guards; empty handlers either log or removed + router `match` pruned. |
| **S08** | Clarify event allowlist (U7) | `core/constants.rs:20-24`, `services/pipeline/mod.rs:5-10`, `AGENTS.md:68-77` | Spec updated to list system/model lifecycle carve-out; or lifecycle emits moved onto `state_changed`/`pipeline_error`. Code/spec no longer contradict. |
| **S09** | Deduplicate `CHUNKER`/`CURRENT_*` statics | `pipeline/modular/{passive,ptt}.rs:15`, `realtime/{passive,ptt}.rs:14`, `core/state.rs` (new) | Single `AppState::chunker` or `ConversationManager` owner, behind `RwLock`; global `LazyLock<Mutex<TtsClauseChunker>>` removed. |
| **S10** | Normalize `cancel_flag` vs `CancellationToken` split (U4) | `core/state.rs:139,149`, `services/pipeline/**/*.rs`, `services/llm/actor.rs:13`, `audio/playback.rs:103` | Documented contract in `core/state.rs:216` docstring; redundant stores pruned. |
| **S11** | Fix double `transition(Thinking)` / parallel `conversation_manager` double-call | `pipeline/modular/ptt.rs:162,227`, `pipeline/realtime/ptt.rs:264`, `pipeline/modular/passive.rs:174-175` | One `Thinking` per turn; `on_speech_start` docs the pop or coalesces. |
| **S12** | Promote `RealtimeEngine::{activity_*}` dead surface or delete | `services/realtime/engine.rs:105-140`, `services/realtime/mod.rs:81-89` | Called paths (`realtime/ptt` `activity_start/end` on PTT hold) or removed; grep shows no dead `pub fn`. |
| **S13** | Style sweeps (50-line caps, helper extraction, `pub→pub(crate)`) | `pipeline/modular/passive.rs:210` `on_transcript_final`, `realtime/passive::start_session`, `core/settings.rs:926` | Each function ≤50 lines or `/// justify`; `pub` tightened where not on crate boundary. |

---

## 10. Immediate Action (blocking before feature work)

**Fix S01 + S03 first.** S01 is a silent functional break of PTT (easy to miss because passive mode keeps passing). S03 parks Tokio workers. Both are safe, isolated, testable without touching the spec. Propose:

1. Decide **U1** (press-time vs release-time) and **U2** (buffer vs channel) synchronously (or default to press-time + wire `ingest_audio` inside `process_windowed_validation`).
2. Execute **S01 → S03 → S04** in one stack (they touch the same turn-ID/buffer seam).
3. Then **S06** (flag bag) before new models are added, otherwise new flags accumulate.

---

## 11. Appendix — Grep receipts

- `grep -rn "\.emit(" app/src-tauri/src --include="*.rs"` → 44 hits above (full list in §2.2).
- `grep -rn "is_state_changed\|is_llm_loaded\|is_tts_loaded\|is_stt_loaded\|is_vad_loaded"` → hits listed in §1.2.
- `grep -rn "#\\[allow"` → 0.
- `grep -rn "let _ =.*send\|let _ ="` → 0 relevant (dead swallows purged).
- `grep -rn "ingest_audio\|get_buffer_len" | grep -v "pub fn"` → 0 (seam unwired).
- `grep -rn "next_turn(\|cancel_current_turn\|activity_start\|activity_end\|is_connected\|last_activity_time" | grep "pub fn"` → dead list §4.1.
- `cargo check --all-targets` — `Finished` 1.50s, 0 errors.
- `cargo clippy --all-targets` — `Finished` 12.84s, 0 warnings.

---

## 12. Files that MUST NOT be edited (per `AGENTS.md §2`)

`submodules/{chatterbox-rs,query-sieve-rs,distilbert-query-classifier,vox-models}` — not audited and not to be edited.

---

*Report written without subagents per your request, all uncertain logic explicitly flagged `U1..U7` above. Switch back to `plan` for sign-off, or approve the sprint order above to proceed in `build` mode.*
