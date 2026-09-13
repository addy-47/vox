# Phase 1 — Integration Test Execution Ledger

This ledger records the initial execution results of all translated integration test seams following the `/create-test` and `/test` protocols.

---

## Seam 1: `tests/passive_streaming_test.rs`
- **SUT:** Autonomous VAD segmentation + Nemotron streaming STT (`services/vad/actor.rs` + `services/stt/actor.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test passive_streaming_test --release --nocapture --test-threads=1`
- **Execution Time:** ~8.36s
- **Evidence Observed:**
  - **Subtest 1 (EN `supertonic_01_en_briefing.wav`):** Received `VoxEvent::SpeechStart` and `VoxEvent::SpeechEnd`. Collected 1 `TranscriptFinal` utterance. Met Levenshtein similarity threshold `>= 0.90` against ground truth.
  - **Subtest 2 (HI `supertonic_07_hi_weather.wav`):** Received `VoxEvent::SpeechStart` and `VoxEvent::SpeechEnd`. Collected 1 `TranscriptFinal` utterance. Confirmed Devanagari script presence. Met similarity threshold `>= 0.90`. Confirmed `transliterate_if_hi` yields non-empty ASCII-only Roman script.
  - **Subtest 3 (Silence Only Guard):** Streamed 100 silence frames. Both `vox_event_rx` and `pipeline_event_rx` remained empty after 500ms / 200ms deterministic waits (zero false speech triggers).
  - **Teardown:** Worker threads joined cleanly with zero panics.

---

## Seam 2: `tests/ptt_window_modular_test.rs`
- **SUT:** Push-To-Talk Window Validation (Modular) (`pipeline/assistant/ptt.rs` + `services/vad/actor.rs` + `services/stt/actor.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test ptt_window_modular_test --release --nocapture --test-threads=1`
- **Execution Time:** ~4.88s
- **Defect Resolved:**
  - Initial run caught production panic at `src/pipeline/assistant/ptt.rs:108:34` (`blocking_lock()` on `tokio::sync::Mutex` inside async runtime).
  - Resolved by Backend Engineer applying `try_lock()` discipline at `assistant/ptt.rs:106–121` matching dictation precedent.
- **Evidence Observed:**
  - **Subtest 1 (Speech Validation):** `ptt_start` transitioned to `Listening`. Streamed `supertonic_01_en_briefing.wav`. `ptt_stop` transitioned to `Thinking`. Emitted `TranscriptFinal` meeting threshold `>= 0.90` against ground truth.
  - **Subtest 2 (Ghost Gate):** Streamed 30 silence frames during PTT hold. `ptt_stop` evaluated non-speech and cleanly reverted state to `Ready`. `pipeline_event_rx` remained empty.
  - **Subtest 3 (PTT Cancel):** `ptt_cancel` reverted state to `Ready`, cancelled `turn_token`, and suppressed STT dispatch.
  - **Teardown:** Actors shut down cleanly and joined with zero panics.

---

## Seam 3: `tests/ptt_window_realtime_test.rs`
- **SUT:** Push-To-Talk Window Validation (Realtime / Deepgram Voice Agent) (`pipeline/assistant/ptt.rs` + `services/vad/actor.rs` + `services/realtime/actor.rs` + `services/realtime/providers/deepgram`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test ptt_window_realtime_test --release --nocapture --test-threads=1 -- --ignored`
- **Execution Time:** ~5.18s
- **Defects / Blockers Resolved:**
  1. *Async Runtime Re-entrancy:* Wrapped `actor.start(...)` in `tokio::task::spawn_blocking(...)` to isolate blocking provider handshake from the Tokio runtime thread.
  2. *Rustls CryptoProvider Initialization:* Explicitly installed Ring CryptoProvider (`rustls::crypto::ring::default_provider().install_default()`) for WebSocket TLS.
  3. *Deepgram Model Settings:* Aligned model parameters to `gpt-4o-mini` and `aura-asteria-en` to avoid `INVALID_SETTINGS` rejection from Deepgram API.
- **Evidence Observed:**
  - **Subtest 1 (Speech Validation & Live WebSocket Commit):** `ptt_start` transitioned to `Listening`. Streamed `supertonic_01_en_briefing.wav`. `ptt_stop` transitioned to `Thinking`, converted f32 audio to signed i16 via `(x.clamp(-1.0, 1.0) * 32767.0) as i16`, committed speech turn to live Deepgram Voice Agent session, verified STT channel received zero commands (anti-misrouting), and observed server response event over live WebSocket within 15s deadline.
  - **Subtest 2 (Ghost Gate):** Streamed 30 silence frames during PTT hold. `ptt_stop` evaluated non-speech, cleanly reverted to `Ready`, and verified zero events or STT dispatches.
  - **Subtest 3 (PTT Cancel):** `ptt_cancel` reverted state to `Ready`, cancelled `turn_token`, and verified zero events or STT dispatches.
  - **Teardown:** `RealtimeActor` stopped and VAD actor joined cleanly with zero panics.

---

## Seam 4: `tests/dictation_window_test.rs`
- **SUT:** Dictation PTT + Passive + Ingestion Gate + LLM Zero Invariant (`pipeline/dictation/{mod,ptt,speech,transcript,error}.rs` + `services/vad/actor.rs` + `services/dictation/output_router.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test dictation_window_test --release --nocapture --test-threads=1`
- **Execution Time:** ~8.47s
- **Defects Resolved:**
  1. *Defect 1 (`toast.rs:132` / `toast.rs:77`):* Wrapped `window.gtk_window()` in `with_gtk_window` helper with `catch_unwind` to gracefully handle headless / mock runtimes without panicking.
  2. *Defect 2 (`dictation/error.rs:16-48`):* Fixed auto-recover state logic. Captures `was_idle` prior to `transition_dictation(Error)` so disabled (Idle) dictation restores `Idle` rather than erroneously auto-recovering to `Ready`.
- **Evidence Observed:**
  - **Subtest 1 (PTT Speech Routing & LLM Zero Invariant):** `PttStart` transitioned dictation state to `Listening`. Streamed `supertonic_01_en_briefing.wav`. `PttStop` transitioned to `Thinking` and dispatched audio to Nemotron STT. Emitted `TranscriptFinal` meeting threshold `>= 0.90` (similarity 0.96+). Injected transcript into router: updated `dictation_last_transcript`, returned dictation state to `Ready`, and verified `llm_rx` channel was 100% empty after 500ms deterministic wait.
  - **Subtest 2 (Ghost Gate):** Streamed silence frames during PTT hold. `PttStop` discarded non-speech audio and cleanly reverted state to `Ready`. `pipeline_event_rx` remained empty after 500ms.
  - **Subtest 3 (PttCancel via Router):** `PttCancel` sent via router while `Listening` reverted state to `Ready`. `pipeline_event_rx` remained empty.
  - **Subtest 4 (Passive Speech Routing):** `SpeechStart` transitioned state to `Listening`; `SpeechEnd` transitioned state to `Thinking`. Transcript routing updated `dictation_last_transcript`, returned to `Ready`, and verified `llm_rx` channel was completely empty.
  - **Subtest 5 (Option-C Ingestion Gate Purge):** Closed gate via `Idle` state. Streamed audio while gate was closed. Verified VAD actor purged in-flight buffers. Reopened gate: clean `PttStart`/`PttStop` reverted to `Ready` via ghost gate with zero stale transcript emissions.
  - **Subtest 6 (Disabled / Idle Start):** `PttStart` while in `Idle` invoked `error::on_error` and preserved `Idle` state (preventing rogue activation).
  - **Teardown:** Router, VAD, and STT threads joined cleanly.

---

## Seam 5: `tests/transcript_to_llm_test.rs`
- **SUT:** STT Transcript ──► Harness `prepare_turn` ──► Duplex Dialogue Pipe ──► Real Embedded Qwen LLM (`pipeline/assistant/transcript.rs` + `services/harness/session.rs` + `services/llm/actor.rs` + `services/llm/embedded`)
- **Status:** ✅ **PASS** (Zero-Mock Verified)
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test transcript_to_llm_test --release --nocapture --test-threads=1`
- **Execution Time:** ~5.69s
- **Defects / Blockers Resolved:**
  1. *Self-Deadlock in `QuietCompactionWatcher::on_turn_completed`:* Calling `watcher.on_turn_completed` while holding `harness.lock()` attempted a re-entrant lock acquisition in `watcher.rs:45`. Resolved by evaluating `check_quiet_compaction_eligibility()` on `&self` prior to lock invocation and passing `tracked_turns` directly.
  2. *Async Runtime OS Thread Block:* Teardown `worker_handle.join()` wrapped in `tokio::task::spawn_blocking` with explicit 5-second timeout, adhering to `.agents/rules/testing-style-guide.md §7.1` (eliminating unbounded synchronous blocking on Tokio runtime).
  3. *Context Window & Threshold Calibration:* Sized context window to 4200 (exceeding `EMBEDDED_MODEL_MIN_CONTEXT_WINDOW = 4096`). Seeded 30 turn pairs (~3,300 tokens) to cross the 85% critical threshold (>3,135 tokens) cleanly without multi-minute CPU inference stalls.
  4. *Turn Token Cancellation:* Added explicit `state.pipeline.turn_token().cancel()` upon assertion of interim filler and Working state to immediately halt background compaction tasks.
- **Evidence Observed:**
  - **Subtest 1 (Valid Dispatch & Real Generation):** Valid transcript in `Thinking` entered `on_transcript_final`, dispatched `GenerationRequest` over duplex pipe to real local `EmbeddedProvider` (Qwen GGUF), emitted `VoxEvent::LlmFinished`, and populated `pipeline_accumulator.assistant_response` with non-empty generated tokens.
  - **Subtest 2 (Empty / Whitespace Guard):** Whitespace transcript reverted pipeline state to `Ready`, cleared accumulator, and emitted zero LLM generation requests.
  - **Subtest 3 (Non-Thinking Drop):** Valid transcript received while in `Listening` state was dropped without state changes or pipeline events.
  - **Subtest 4 (Realtime Pipeline Mode):** In `Realtime` mode, preserved `Thinking` state awaiting provider stream, logged transcript in accumulator, and emitted zero modular LLM dispatches.
  - **Subtest 5 (Critical Threshold & Interim Filler):** Pre-seeded buffer (>85% utilization) triggered `TurnPreparation::NeedsInlineCompaction`, dispatched `TtsCommand::Generate` interim filler from `TRANSITION_MESSAGES_EN` with `AudioIntent::InterimFiller`, transitioned state to `Working`, and incremented `pending_synthesis_jobs`.
  - **Teardown:** Clean shutdown via `LlmCommand::Shutdown` and worker join with zero panics.

---

## Seam 6: `tests/llm_to_tts_test.rs`
- **SUT:** Cognitive Stage Orchestration (`HarnessSession::new_modular` ──► `LlmActor` Duplex Dialogue Pipe ──► `StreamRoutingPlugin` Clause Chunking ──► TTS Dispatch ──► History Commit) (`services/harness/session.rs` + `services/harness/plugins/stream.rs` + `services/llm/actor.rs` + `services/llm/embedded.rs` + `pipeline/assistant/llm.rs`)
- **Status:** ✅ **PASS** (Zero-Mock Verified)
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test llm_to_tts_test --release --nocapture --test-threads=1`
- **Execution Time:** ~2.59s
- **Evidence Observed:**
  - **Full Cognitive Stage Flow:** Prepared turn via `harness.prepare_turn(&mut state, &event_tx)`, acquired system prompt and conversation messages, and submitted to duplex dialogue channel `llm_tx.send(LlmCommand::Generate)`.
  - **Real Local Inference:** Real Qwen 3.5 0.8B GGUF model (`EmbeddedProvider`) running on dedicated OS worker thread generated token stream.
  - **Streaming Chunker & Routing:** Handled tokens through `harness.route_stream(&mut state, &event_tx, &handles, chunk)`, chunking streaming tokens into complete sentence clauses dispatched as `TtsCommand::Generate` with `AudioIntent::TurnResponse`.
  - **History Commit & Lifecycle:** Upon `LlmFinished`, routed stream completion through `harness.on_llm_finished(&mut state, &handles)`, verified history commit (`session.history().get_recent_history().len() == 2`), and dispatched turn completion to `QuietCompactionWatcher`.
  - **Pending Accounting Invariant:** Verified `pending_synthesis_jobs` matched the exact count of dispatched clauses ($N = \text{clauses.len()}$).
  - **Teardown:** Clean shutdown via `LlmCommand::Shutdown` and worker join with zero leaks or panics.

---

## Seam 7: `tests/tts_to_playback_test.rs`
- **SUT:** Real Local Supertonic ONNX TTS Synthesis ──► Central Pipeline Router State Transition & Pre-roll Cushion Gates (`services/tts/actor.rs` + `services/tts/providers/supertonic.rs` + `services/audio/playback.rs` + `pipeline/router.rs` + `pipeline/assistant/playback.rs`)
- **Status:** ✅ **PASS** (Zero-Mock Verified)
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test tts_to_playback_test --release --nocapture --test-threads=1`
- **Execution Time:** ~1.32s
- **Defects / Rework Completed:**
  1. *Eliminated Manual `on_playback_started` Invocation:* Subtest 1 now spawns the real central pipeline router (`vox_lib::pipeline::router::spawn_router`), which receives `VoxEvent::PlaybackStarted` over `event_tx` and executes the canonical transition from `InteractionState::Thinking` to `Speaking`.
  2. *Eliminated Bypass of `spawn_tts_worker`:* Subtest 2 now routes generation through `spawn_tts_worker` on a dedicated worker thread, verifying that (1) short chunks (< 12,000 samples) do not arm prematurely inside `ingest_chunk`, (2) `pending_synthesis_jobs` is decremented by the worker loop, and (3) the worker loop automatically triggers `handles.playback.flush_pre_roll()` when remaining jobs $\le 1$, arming playback and emitting `PlaybackStarted`.
  3. *Async Concurrency & Thread Joins:* All test functions wrapped in `tokio::time::timeout`; all thread joins wrapped in `tokio::task::spawn_blocking` bounded by 5-second timeouts (§7.1).
- **Evidence Observed:**
  - **Subtest 1 (Real Synthesis, Pre-roll Cushion & Router State Transition):** Dispatched `TtsCommand::Generate` to real local Supertonic ONNX engine (voice 0, steps 2, speed 1.0, 4 threads). Generated valid non-silent PCM audio (RMS > 0.001, occupied samples > 0). Central router received `VoxEvent::PlaybackStarted` and transitioned pipeline state to `Speaking`. `pending_synthesis_jobs` decremented to 0.
  - **Subtest 2 (Worker Loop Pre-roll Cushion & Automatic Flush):** Synthesized short chunk (2,000 samples @ 24kHz upsampled to 4,000 samples @ 48kHz < 12,000 threshold). Asserted playback was NOT armed during ingestion. Worker loop finished synthesis, decremented `pending_synthesis_jobs` to 0, and automatically called `flush_pre_roll()`, immediately arming `turn_armed` and emitting `VoxEvent::PlaybackStarted`.
  - **Teardown:** Clean shutdown via `TtsCommand::Shutdown` / `VoxEvent::Shutdown` with bounded thread joins and zero panics.

---

## Seam 8: `tests/tts_transition_test.rs`
- **SUT:** Real IPC Voice Hot-Swap, Worker Preservation & Context Compaction Filler Dispatch (`services/tts/actor.rs` + `ipc/settings/core.rs` + `pipeline/assistant/transcript.rs` + `services/harness/session.rs` + `services/harness/plugins/budget.rs`)
- **Status:** ✅ **PASS** (Zero-Mock Verified)
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test tts_transition_test --release --nocapture --test-threads=1`
- **Execution Time:** ~1.47s
- **Defects / Rework Completed:**
  1. *Eliminated Synthetic `SetVoice` Injection:* Path A now enters through real IPC command `ipc::settings::core::update_setting("tts", "voice_index", json!(2))`, verifying that voice updates dynamically propagate to the persistent worker thread without thread restart.
  2. *Verified Active Voice Hot-Swap with Audio Synthesis:* Worker thread verified active voice update on `VoiceTrackingProvider` (0 -> 2) and synthesized subsequent clauses with non-zero RMS (> 0.001) while preserving thread handle identity.
  3. *Eliminated Mock Session Loop in Path B:* Real `on_transcript_final` invokes `spawn_modular_llm_task` with real `HarnessSession::new_modular`. Calibrated context window (4200) and seeded history (>85% utilization threshold) to trigger `TurnPreparation::NeedsInlineCompaction`.
  4. *Verified Real Dispatch & Playback Gating Contracts:* Verified real router dispatch sends `TtsCommand::Generate` with `AudioIntent::InterimFiller` to `tts_rx`, increments `pending_synthesis_jobs`, and transitions state to `InteractionState::Working`. Validated negative playback gating assertions (filler onset/finish keeps state in `Working`, while `TurnResponse` onset transitions to `Speaking`).
- **Evidence Observed:**
  - **Subtest 1 (Real IPC Voice Hot-Swap Without Worker Restart):** Worker thread preserved across voice switch (thread handle ID unchanged). IPC `update_setting` updated active voice index to 2. Subsequent clause synthesis produced valid audio frames with non-zero RMS.
  - **Subtest 2 (Critical Context Compaction Filler Dispatch & Pending Accounting):** Exceeding 85% token capacity triggered immediate transition filler dispatch (`AudioIntent::InterimFiller`) to TTS with atomic increment of `pending_synthesis_jobs` and transition to `Working`. Playback gating contracts verified. Normal turns (<85% capacity) verified to emit zero filler commands.
  - **Teardown:** Clean shutdown via `TtsCommand::Shutdown` with bounded thread joins and zero panics.

---

## Seam 9: `tests/playback_interrupt_test.rs`
- **SUT:** Playback Lifecycle, Real Sink Callback Buffer Drain, Sacred VAD Ducking & 6-Step Barge-in Sequence (`services/audio/playback.rs` + `services/audio/sink.rs` + `pipeline/assistant/playback.rs` + `pipeline/assistant/interrupt.rs` + `pipeline/router.rs` + `services/vad/actor.rs`)
- **Status:** ✅ **PASS** (Zero-Mock Verified)
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test playback_interrupt_test --release --nocapture --test-threads=1`
- **Execution Time:** ~3.34s
- **Defects / Rework Completed:**
  1. *Eliminated Tautological Self-Dispatch:* Subtest 1 now spawns the real central router pump (`spawn_router`) and ingests through `PlaybackEngine`. Buffer drain is processed via real CPAL output sink callback (`PlaybackStreamContext::process_output_buffer`), which autonomously emits `VoxEvent::PlaybackFinished` upon consumer completion with `pending_jobs == 0`, triggering router transition to `Ready`.
  2. *Verified Pre-Roll Cushion & Flush Arming:* Subtest 2 verified that short chunks (< 12,000 samples) strictly hold arming, while explicit `flush_pre_roll` immediately arms playback and emits `PlaybackStarted`.
  3. *Verified Multi-Tier Pending Job Deferral:* Subtest 3 verified that real sink callback records buffer underrun and defers `PlaybackFinished` emission while `pending_jobs > 0`, and the router handler similarly defers state transition until pending synthesis reaches 0.
  4. *Preserved Sacred VAD Ducking Invariants with Bounded Joins:* Subtests 4 and 5 verified real Earshot VAD ONNX speaker ducking suppression during `Speaking` in `Speaker` mode, instant resumption in `Ready`, and transparent bypass in `Headset` mode; thread joins wrapped with 5s bounded timeouts.
  5. *Wired Production Barge-In Seam:* Subtest 6 now triggers barge-in through the real upstream production entry seam (`VoxEvent::PttStart` over router channel) rather than directly invoking `on_interrupt`. Verified all 6 canonical mutations: monotonic turn advancement, old token cancellation, active new token, pending jobs reset to 0, accumulator cleared, playback engine cancellation, and transition to `Listening`.
- **Evidence Observed:**
  - **Subtest 1 (Real Sink Callback Drain & Router Transition):** Pre-roll threshold (12,000 samples) triggered `PlaybackStarted` and transitioned `Thinking` → `Speaking`. Real sink callback drained 12,000 samples and autonomously emitted `PlaybackFinished`, transitioning `Speaking` → `Ready`.
  - **Subtest 2 (Short Utterance Cushion Gate):** Ingesting 2,000 samples did not emit `PlaybackStarted` before flush; `flush_pre_roll` immediately emitted `PlaybackStarted`.
  - **Subtest 3 (Pending Job Deferral):** Sink callback suppressed `PlaybackFinished` and router deferred state transition while `pending_synthesis_jobs == 1`; upon decrement to 0, state transitioned to `Ready`.
  - **Subtest 4 (Sacred VAD Ducking Suppression):** Real speech clip (`supertonic_01_en_briefing.wav`) streaming during `Speaking` under `Speaker` mode produced zero `SpeechStart` events.
  - **Subtest 5 (VAD Ducking Resumption & Headset Invariant):** Returning to `Ready` under `Speaker` mode emitted `SpeechStart`; `Headset` mode in `Speaking` state emitted `SpeechStart` without suppression.
  - **Subtest 6 (Canonical 6-Step Barge-in Sequence):** Upstream `VoxEvent::PttStart` during `Speaking` advanced turn ID, cancelled old token, reset pending jobs to 0, cleared accumulator, cancelled playback engine, and transitioned to `Listening`.
  - **Teardown:** Clean shutdown via `VoxEvent::Shutdown` and bounded thread joins with zero leaks or panics.

---

## Seam 10: `tests/chunking_determinism_test.rs`
- **SUT:** Clause Chunking Determinism & Token Fragmentation Invariance (`services/tts/actor.rs` + `pipeline/assistant/accumulator.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test chunking_determinism_test --release --nocapture --test-threads=1`
- **Execution Time:** ~0.05s
- **Evidence Observed:**
  - **Subtest 1 (Fragmentation Determinism):** Replayed multi-clause complex sentence across fine-grained sub-word tokens (Fragmentation A) vs erratic coarse chunks spanning punctuation boundaries (Fragmentation B). Byte-for-byte reconstructed identical clause sequences in identical order. Verified buffer empty after flush and `acc.clear()` contract verified.
  - **Subtest 2 (Emergency 20-Word Cap):** Fed 30 unpunctuated words across 1-word-per-token vs 3-words-per-token streams. Both fragmentations deterministically emitted exactly 2 chunks: first chunk exactly 20 words from emergency cap, second chunk remaining 10 words from flush.
  - **Subtest 3 (Comma Prosody Gating Stability):** Verified commas preceded by < 5 words do not split before sentence boundary; commas preceded by >= 5 words split immediately. Fragmenting tokens around comma preserved split points deterministically.

---

## Seam 11: `tests/session_lifecycle_test.rs`
- **SUT:** Assistant Session Lifecycle & FSM (`pipeline/assistant/session.rs` + `pipeline/router.rs` + `persistence/worker.rs` + `persistence/sessions.rs` + `services/harness/session.rs` + `services/realtime/session.rs`)
- **Status:** ✅ **PASS** (Zero-Mock Verified)
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test session_lifecycle_test --release --nocapture --test-threads=1`
- **Execution Time:** ~1.07s
- **Defects / Blockers Resolved:**
  1. *Database Path & Pre-seeded Fixture Sync (`tests/common/paths.rs`):* `TempPathsGuard::new()` copied `tests/assets/test_vox.db` to `temp_path.join("test_vox.db")`, whereas `AppState::new` connects to `temp_path.join("vox.db")`. Standardized on `vox_lib::utils::paths::DB_FILENAME` ("vox.db") and made constant public.
  2. *Turso Seed Schema Compliance (`tests/session_lifecycle_test.rs`):* Subtest 2 inserted `NULL` into `project_id` (violating `NOT NULL DEFAULT 'default'`) and targeted non-existent column `summary` instead of v2 schema columns `(trigger_kind, from_turn_id, to_turn_id, compaction_output, status, created_at)` in `session_compactions`. Corrected seed queries to valid v2 schema.
  3. *Realtime Pipeline Mode Purge Assertion (`tests/session_lifecycle_test.rs`):* Subtest 6 configured `new_modular` but asserted `purge_session_cache` on disk, which only runs when `ctx.pipeline_mode == PipelineMode::Realtime`. Configured `settings.interaction.pipeline_mode = PipelineMode::Realtime` and mounted `HarnessSession::new_realtime`.
  4. *Active Context Refresh in `on_session_start` and `on_resume` (`src/pipeline/assistant/session.rs`):* `route_event` captured `ctx` before mutating `state.owner`. Handlers used stale pre-transition context, causing VAD mode to remain in Dictation mode (`WindowedValidation`) rather than Assistant mode (`ContinuousSegmentation`). Re-derived `session_ctx` and `assistant_ctx` from `RoutingContext::from_app_state(state)` immediately after `state.owner` update.
- **Evidence Observed:**
  - **Subtest 1 (`test_session_start_modular_sets_ready_and_identity`):** Dispatched `VoxEvent::SessionStart` from `Idle`. Verified state transitioned to `Ready`, `state.owner` set to `Assistant`, real Turso SQLite row persisted in `sessions` table via `spawn_persistence_worker`, `HarnessSession` initialized with base identity prompt, VAD set to `ContinuousSegmentation`, and subsequent start proved idempotent.
  - **Subtest 2 (`test_session_continuation_seeds_harness`):** Resumed session ID (`987654321`) with pre-seeded turns in Turso DB. Verified continuation history hydrated into `HarnessSession` and turn counter advanced past pre-existing turns.
  - **Subtest 3 (`test_session_pause_resume_transitions`):** Dispatched `PauseSession` -> transitioned `Paused`, cancelled turn token, yielded owner to `Dictation`, set VAD `WindowedValidation`. Dispatched `ResumeSession` -> transitioned `Ready`, restored `Assistant` owner, re-armed turn token, restored VAD `ContinuousSegmentation`.
  - **Subtest 4 (`test_session_resume_from_sleeping_and_error`):** Validated recovery transitions from `Sleeping -> Ready` and `Error -> Ready`; verified resume dropped when `Idle`.
  - **Subtest 5 (`test_session_end_dictation_gate_keeps_engine`):** When Dictation was `Ready`, assistant `EndSession` transitioned assistant to `Idle` while preserving CPAL audio engine (`Some`) and switching VAD to dictation mode. When Dictation was `Idle`, `EndSession` stopped audio engine (`None`).
  - **Subtest 6 (`test_session_end_purges_and_unmounts_harness`):** Dispatched `EndSession` -> unmounted `state.harness` (`None`), purged realtime session cache on disk, cleared accumulator, and cancelled turn token.
- **Teardown:** Router, persistence worker, and VAD threads joined cleanly.

---

## Seam 15: `tests/settings_persistence_test.rs`
- **SUT:** Settings Persistence & Mutation Round-Trip (`core/settings.rs` + `ipc/settings/mutation.rs` + `utils/paths.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test settings_persistence_test --release --nocapture --test-threads=1`
- **Execution Time:** ~0.05s
- **Evidence Observed:**
  - **Subtest 1 (`test_settings_json_roundtrip_persistence`):** Mutated settings across Appearance, Audio, VAD, STT, LLM, TTS, Interaction, Dictation, and Memory using production `apply_setting_mutation`. Persisted to disk via `VoxSettings::save`. Verified physical `settings.json` existence, non-empty size, and inspected raw JSON key/values. Reloaded via `VoxSettings::load` and asserted 100% exact field-wise round-trip preservation.
  - **Subtest 2 (`test_settings_malformed_fallback_to_default`):** Wrote corrupt unclosed JSON into `settings.json`. `VoxSettings::load` cleanly fell back to system defaults without panic, created timestamped backup file `settings.corrupt.<ts>.json` containing the original content, and preserved filesystem integrity.
  - **Subtest 3 (`test_settings_partial_section_recovery`):** Tested schema drift / partial corruption. Loaded JSON with valid Appearance and TTS sections alongside omitted and junk sections. Successfully recovered valid sections while restoring missing domains to system defaults.

---

## Seam 16: `tests/model_eviction_test.rs`
- **SUT:** Model Singleton Eviction & Zero Idle RAM (`services/memory/ml` + `services/tts/actor.rs` + `services/translit.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test model_eviction_test --release --nocapture --test-threads=1`
- **Execution Time:** ~4.93s
- **Evidence Observed:**
  - **Subtest 1 (`test_onnx_model_singleton_lifecycle_eviction`):** Lazily initialized 4 memory ONNX models (MiniLM sentence embedder, DeBERTa v3 NLI, ModernBERT edge classifier, ModernBERT memory scope classifier) and Seq2Seq Hindi transliteration engine. Verified active inference (`generate_embedding` yielded 384-dim vector). Performed partial pipeline eviction (`unload_memory_pipeline_onnx_models`) and verified safe fallback (`generate_embedding` returned `Ok(None)` without crash/SIGSEGV). Performed full eviction (`unload_all_onnx_models`), verified all `is_*_loaded()` returned false, confirmed double-eviction idempotency, and proved clean re-loading into memory.
  - **Subtest 2 (`test_tts_worker_cool_down_clears_handles_and_joins`):** Initialized real Supertonic ONNX TTS worker via `warm_up_tts`. Invoked `cool_down_tts(&mut tts_tx)`: confirmed `tts_tx` was immediately taken (`is_none() == true`), `TtsCommand::Shutdown` was processed, and the dedicated worker OS thread joined cleanly with zero panics.

---

## Seam 17: `tests/model_manager_test.rs`
- **SUT:** Model Manager Lifecycle & Safety (`setup/model_manager.rs` + `setup/manifest.rs` + `setup/manager_ops.rs`)
- **Status:** ✅ **PASS**
- **Command:** `RAYON_NUM_THREADS=$(nproc) OMP_NUM_THREADS=$(nproc) cargo nextest run --test model_manager_test --release --nocapture --test-threads=1`
- **Execution Time:** ~0.046s
- **Evidence Observed:**
  - **Subtest 1 (`test_model_manager_valid_payload_verification`):** Verified synthetic model payload integrity. Proved `.verified` marker file was created on disk with exact matching SHA256, expected file size, model ID, and non-zero timestamp. Subsequent check verified instant cache hit via `.verified` marker.
  - **Subtest 2 (`test_model_manager_corrupted_payload_detection`):** Verified size mismatch (truncated payload) rejects presence and suppresses `.verified` marker creation. Verified tampered marker with mismatched SHA256 falls through to size matching and refreshes the marker with canonical manifest hash.
  - **Subtest 3 (`test_model_manager_zip_slip_and_tar_slip_rejection`):** Synthesized malicious Zip and Tar archives with path traversal entries (`../escaped_file.txt`). Verified `ModelManager::do_extract` detects Zip-Slip and Tar-Slip vulnerabilities, rejects extraction with explicit security error, and leaves destination parent directory untouched. Confirmed legitimate archive unpacks safely.
  - **Subtest 4 (`test_model_manager_removal_cleans_marker_and_dir`):** Verified model deletion via `delete_model_file` removes model binary, purges `.verified` marker file, cleans up empty parent directory structure, and updates presence check to false.

