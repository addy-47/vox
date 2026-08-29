# Backend Review Sprints Checklist (1 Review Point = 1 Sprint)

> **Master Checklist:** 1:1 direct mapping from all findings and UNSURE questions in `docs/backend-review/sprint-01.md` through `sprint-11.md`. Total Sprints: **188**.


## Sprint 01 — Core + App Shell (`sprint-01.md`)

- [x] **Sprint 001** [🔴 WILL BREAK] — 1. Any unparseable enum value in `settings.json` wipes the *entire* settings file
- [x] **Sprint 002** [🟠 REAL COST AT SCALE] — 2. `VadBackendOption` default contradicts its own doc — Earshot is never the default
- [x] **Sprint 003** [🟠 REAL COST AT SCALE] — 3. `block_on` a tokio async `Mutex` from synchronous shell contexts
- [x] **Sprint 004** [🟠 REAL COST AT SCALE] — 4. `InteractionState` has three redundant representations that can diverge
- [x] **Sprint 005** [🟠 REAL COST AT SCALE] — 5. Bootstrap logs emitted before the logger is initialized are silently dropped
- [x] **Sprint 006** [🟠 REAL COST AT SCALE] — 6. Corrupt-settings backup is lost on a second corruption
- [x] **Sprint 007** [🟠 REAL COST AT SCALE] — 7. Dead constants + a third source of truth for the collection→class taxonomy
- [x] **Sprint 008** [🟠 REAL COST AT SCALE] — 8. `save()` is not safe against concurrent callers sharing one tmp path
- [x] **Sprint 009** [⚡ OPTIMIZATION] — 9. Shell/bootstrap modules are not hot paths — no meaningful latency/alloc issues found
- [x] **Sprint 010** [🟡 STYLISTIC] — **`core/constants.rs:160`** — `use serde::{Deserialize, Serialize};` appears
- [x] **Sprint 011** [🟡 STYLISTIC] — **`core/constants.rs:68`** — The section banner
- [x] **Sprint 012** [🟡 STYLISTIC] — **`core/constants.rs:280`** — `inverse_edge_for_relation` matches
- [x] **Sprint 013** [🟡 STYLISTIC] — **`core/error.rs:29` + `:110`** — `VoxError::Io(#[from] std::io::Error)` and
- [x] **Sprint 014** [🟡 STYLISTIC] — **`lib.rs:549`** — `.expect("error while building tauri application")`. This is a
- [x] **Sprint 015** [🟡 STYLISTIC] — **`window_customizer.rs:6-10`** — Trivial `Default` impl; could derive
- [x] **Sprint 016** [❓ UNSURE / QUESTION] — **`window_customizer.rs:17-63` — correctness/safety of the unsafe GTK gesture
- [x] **Sprint 017** [❓ UNSURE / QUESTION] — **`core/constants.rs:276-285` — is `inverse_edge_for_relation` a true two-way inverse?**
- [x] **Sprint 018** [❓ UNSURE / QUESTION] — **`core/state.rs:144-228` vs `:231-256` — intentional duplication of telemetry atomics?**

## Sprint 02 — IPC Layer (`sprint-02.md`)

- [x] **Sprint 019** [🔴 WILL BREAK] — 1. `voices.rs:123` — Unbounded `pcm_f32: Vec<f32>` from untrusted IPC → OOM
- [x] **Sprint 020** [🔴 WILL BREAK] — 2. `pipeline/test_clip.rs:59-83` — Path traversal in `resolve_clip_path` (arbitrary file read)
- [x] **Sprint 021** [🔴 WILL BREAK] — 3. `pipeline/test_clip.rs:13` — `source_rate == 0` → `inf` → `usize::MAX` → panic/OOM
- [x] **Sprint 022** [🟠 REAL COST AT SCALE] — 4. `audio.rs:13,42` — Blocking cpal enumeration on the async command executor
- [x] **Sprint 023** [🟠 REAL COST AT SCALE] — 5. `voices.rs:71` — Arbitrary `file_path` from IPC read via `add_voice_from_file`
- [x] **Sprint 024** [🟠 REAL COST AT SCALE] — 6. `settings/mutation.rs:456,459,480,546,552,…` — Silent integer truncation on untrusted numeric settings
- [x] **Sprint 025** [🟠 REAL COST AT SCALE] — 7. `settings/health.rs:509-537` — `setup_remote_server` has no mutual-exclusion guard
- [x] **Sprint 026** [🟠 REAL COST AT SCALE] — 8. `memory/mutations.rs:25-30` — `edit_fact_content` loads the full embedder model per edit
- [x] **Sprint 027** [🟠 REAL COST AT SCALE] — 9. `memory/ingestion.rs:201-233` — Unbounded `item_ids: Vec<i64>` from frontend
- [x] **Sprint 028** [🟠 REAL COST AT SCALE] — 10. `settings/health.rs:166` — `check_tts_provider_health` EdgeTTS arm always returns `Ok(true)`
- [x] **Sprint 029** [🟠 REAL COST AT SCALE] — 11. `setup.rs:336` — `download_optional_model` has no `setup_running` guard
- [x] **Sprint 030** [🟠 REAL COST AT SCALE] — 12. `history.rs:17-36` — Unbounded `text` length into `transcript_history`
- [x] **Sprint 031** [⚡ OPTIMIZATION] — 13. `memory/graph.rs:174-179` — Full edge set fetched every topology request
- [x] **Sprint 032** [⚡ OPTIMIZATION] — 14. `history.rs:12,34` — Full `Vec` clone on every history read/commit
- [x] **Sprint 033** [⚡ OPTIMIZATION] — 15. `audio.rs:18-37,47-66` — Re-enumerate all device configs per call, no cache
- [x] **Sprint 034** [⚡ OPTIMIZATION] — 16. `pipeline/test_clip.rs:11` — Needless `to_vec` on already-16k audio
- [x] **Sprint 035** [🟡 STYLISTIC] — 17. `settings/mutation.rs:325-771` — Giant flat `match` in `apply_setting_mutation`
- [x] **Sprint 036** [🟡 STYLISTIC] — 18. `memory/mutations.rs & conflicts.rs` — Manual `BEGIN/COMMIT/ROLLBACK` string statements
- [x] **Sprint 037** [❓ UNSURE / QUESTION] — **`settings/health.rs:361-505` (`run_remote_ssh_task`)** — Intended trust boundary? The frontend fully supplies `connection_string`, `ssh_port`, `identity_key_path`, `remote_path`, and `server_port`; these become `ssh` args to a trusted local `setup_server.sh` (piped via stdin, with `remote_path`/`server_port` as `$1`/`$2`). Args are passed (not shell-eval'd), so no classic injection, but the frontend can direct SSH at *any* host with *any* key path. Is the frontend trusted to choose the remote target, or should `connection_string`/`identity_key_path` be constrained to admin-configured values? **UNSURE — flag for product/security owner.** (No code change proposed until intent is confirmed.)
- [x] **Sprint 038** [❓ UNSURE / QUESTION] — **`pipeline/test_clip.rs:96-99`** — `test_clip` sets `state.owner = Assistant` and `state.pipeline.is_engaged = true` and starts the engine, but `test_clip_cancel` only clears `is_engaged` and `cancel_flag` (not `owner`), and a successful injection never resets state. Is leaving the engine "engaged"/owner=Assistant after a test clip intended, or should `test_clip` restore prior owner/engaged on completion? **UNSURE — flag for pipeline owner.**
- [x] **Sprint 039** [❓ UNSURE / QUESTION] — **`settings/health.rs:166` (`EdgeTts` health)** — Is returning `Ok(true)` for EdgeTTS a deliberate "assumed available" shortcut, or a bug? Treated as 🟠 #10 but intent ambiguous. **UNSURE.**

## Sprint 03 — Persistence (`sprint-03.md`)

- [x] **Sprint 040** [🔴 WILL BREAK] — F1 — `TurnCompleted` is never emitted, so turns are never persisted
- [x] **Sprint 041** [🔴 WILL BREAK] — F2 — Memory worker event producers are missing; `SessionEnd`/`PersonalFactsReady`/`PipelineIdle`/`PipelineActive`/`ActiveSessionChanged` are never sent
- [x] **Sprint 042** [🔴 WILL BREAK] — F3 — `pending` and `staged` personal-memory-queue statuses are orphaned → silent memory loss
- [x] **Sprint 043** [🟠 REAL COST AT SCALE] — F4 — `supersede_user_fact` is not atomic (partial-commit leaves dangling fact/vector/relation)
- [x] **Sprint 044** [🟠 REAL COST AT SCALE] — F5 — `VoxDb::open_readonly` opens a fully *writable* connection
- [x] **Sprint 045** [🟠 REAL COST AT SCALE] — F6 — Bounded producer channels can block the realtime pipeline (latency violation)
- [x] **Sprint 046** [🟠 REAL COST AT SCALE] — F7 — `update_preview_wav` is dead code
- [x] **Sprint 047** [🟠 REAL COST AT SCALE] — F8 — `decode_f32_blob` silently truncates mis-sized blobs
- [x] **Sprint 048** [⚡ OPTIMIZATION] — O1 — Two writer connections contend on one WAL file under concurrent load
- [x] **Sprint 049** [⚡ OPTIMIZATION] — O2 — `process_idle_queue` opens a full `COUNT(*)` scan every poll tick
- [x] **Sprint 050** [⚡ OPTIMIZATION] — O3 — `fetch_*_candidates` build the SQL string every call
- [x] **Sprint 051** [🟡 STYLISTIC] — S1 — Shutdown via string-matched error (`worker.rs:108,248`)
- [x] **Sprint 052** [🟡 STYLISTIC] — S2 — `get_tokio_handle` fallback leaks a runtime (`db.rs:13-19`)
- [x] **Sprint 053** [🟡 STYLISTIC] — S3 — `SessionStarted`/`TurnCompleted` double-insert sessions with inconsistent `started_at`

## Sprint 04 — Audio + Translit + Utils (`sprint-04.md`)

- [x] **Sprint 054** [🔴 WILL BREAK] — F-01 — Translit ONNX outputs indexed by hardcoded tensor names (panic on model mismatch)
- [x] **Sprint 055** [🔴 WILL BREAK] — F-02 — `state.settings.read().unwrap()` poisons → engine-thread panic
- [x] **Sprint 056** [🟠 REAL COST AT SCALE] — F-03 — `PlaybackEngine::is_idle()` is dead code
- [x] **Sprint 057** [🟠 REAL COST AT SCALE] — F-04 — `AudioError` re-export is unused
- [x] **Sprint 058** [🟠 REAL COST AT SCALE] — F-05 — Detached `spawn_event_forwarder` task (JoinHandle dropped)
- [x] **Sprint 059** [🟠 REAL COST AT SCALE] — F-06 — Playback `buffer_samples` atomic is double-counted / racy vs. actual ring occupancy
- [x] **Sprint 060** [🟠 REAL COST AT SCALE] — F-07 — `transliterate()` lazy-init re-acquires the global `RwLock` write lock on the hot path
- [x] **Sprint 061** [🟠 REAL COST AT SCALE] — F-08 — `decode_bytes_to_24khz_mono` needlessly clones the entire input
- [x] **Sprint 062** [⚡ OPTIMIZATION] — F-09 — Per-chunk allocation in `upsample_2x` on the playback ingest path
- [x] **Sprint 063** [⚡ OPTIMIZATION] — F-10 — `append_samples_as_f32_mono` grows `raw_samples` unreserved + nested index per sample
- [x] **Sprint 064** [⚡ OPTIMIZATION] — F-11 — `fix_missing_commas_in_json` is O(n²) and mangles substring keys
- [x] **Sprint 065** [⚡ OPTIMIZATION] — F-12 — `resample_linear` (decode) recomputes `input.len() - 1` and floors each sample
- [x] **Sprint 066** [🟡 STYLISTIC] — F-13 — Possibly-unused `use symphonia_core::audio::Audio;` import
- [x] **Sprint 067** [🟡 STYLISTIC] — F-14 — `paths::get()` documents a panic
- [x] **Sprint 068** [🟡 STYLISTIC] — F-15 — `clean_json_content` trailing-fence handling is fragile

## Sprint 05 — Dictation + VAD (`sprint-05.md`)

- [x] **Sprint 069** [🔴 WILL BREAK] — 1. Duplicate audio frames sent to the realtime server in passive/PTT-realtime modes
- [x] **Sprint 070** [🔴 WILL BREAK] — 2. VAD actor thread is unsupervised — a panic permanently disables VAD with `is_loaded` left `true`
- [x] **Sprint 071** [🟠 REAL COST AT SCALE] — 3. `accumulate_speech_frames` forwards to `realtime_tx` without the `is_ptt` guard that `stream_passive_realtime` has
- [x] **Sprint 072** [🟠 REAL COST AT SCALE] — 4. PTT + realtime path appears to double-handle audio (VAD actor AND `ingest_audio`)
- [x] **Sprint 073** [🟠 REAL COST AT SCALE] — 5. `EarshotVadEngine` internal debounce state is dead — `predict()` returns the raw per-frame decision
- [x] **Sprint 074** [🟠 REAL COST AT SCALE] — 6. `Ten` backend reloads the ONNX model from disk on every threshold update
- [x] **Sprint 075** [🟠 REAL COST AT SCALE] — 7. `PreRollBuffer::push` does an O(n) `drain(0..excess)` on every chunk once full
- [x] **Sprint 076** [🟠 REAL COST AT SCALE] — 8. Large clones per partial: `VAD_MAX_PARTIAL_WINDOW_SAMPLES = 240000` copied every ~0.8 s
- [x] **Sprint 077** [🟠 REAL COST AT SCALE] — 9. Hotkey press/release spawn independent async tasks racing on shared `AppState`
- [x] **Sprint 078** [🟠 REAL COST AT SCALE] — 10. Hotkey re-registration has no visible unregister path → possible stacked handlers
- [x] **Sprint 079** [⚡ OPTIMIZATION] — 11. Per-chunk `Vec<i16>` allocation in the audio hot path
- [x] **Sprint 080** [⚡ OPTIMIZATION] — 12. `emit_audio_telemetry` computes filter-bank + RMS on every chunk even when frames are suppressed
- [x] **Sprint 081** [🟡 STYLISTIC] — 13. Redundant second `try_recv()` in `process_vad_commands` can silently drop a command
- [x] **Sprint 082** [🟡 STYLISTIC] — 14. `with_clipboard_safe` 350 ms blocking-ish sleep on the success path
- [x] **Sprint 083** [❓ UNSURE / QUESTION] — **`is_above_noise_gate` Earshot ×1.5 multiplier.** `actor.rs:614-620` raises the noise gate by 50% for Earshot. Combined with the +0.15 threshold offset, Earshot and Ten have substantially divergent gate/threshold math. Intended calibration or drift?
- [x] **Sprint 084** [❓ UNSURE / QUESTION] — **`UpdateMode`/`UpdateAudioMode` commands.** `process_vad_commands` updates `state.mode`/`state.audio_mode` but `effective_mode` (actor.rs:491) only overrides to `Passive` when a realtime session is active; otherwise PTT vs Passive is taken from `state.mode`. Confirm the owner/engine that sends these commands keeps `state.mode` coherent with `owner_atomic` (loaded each frame at line 477) so PTT/Passive selection is deterministic.

## Sprint 06 — LLM + Providers (`sprint-06.md`)

- [x] **Sprint 085** [🔴 WILL BREAK] — F-01 — `OpenAiCompatProvider` misclassifies authenticated OpenAI as LM Studio → wrong chat URL → 404
- [x] **Sprint 086** [🔴 WILL BREAK] — F-02 — LM Studio backend is targeted with a wrong URL through the active provider
- [x] **Sprint 087** [🔴 WILL BREAK] — F-03 — Embedded KV-cache prefix reuse is permanently disengaged (`kv_cache_index` hardwired to 0)
- [x] **Sprint 088** [🟠 REAL COST AT SCALE] — F-04 — Four dedicated provider adapters are entirely dead code
- [x] **Sprint 089** [🟠 REAL COST AT SCALE] — F-05 — Embedded prefill phase cannot be cancelled
- [x] **Sprint 090** [🟠 REAL COST AT SCALE] — F-06 — `capability_probe` returns a hardcoded/wrong context window
- [x] **Sprint 091** [🟠 REAL COST AT SCALE] — F-07 — Unsound `LlamaContext<'static>` lifetime transmute
- [x] **Sprint 092** [🟠 REAL COST AT SCALE] — F-08 — Reqwest client 180 s *total* timeout can abort long remote generations
- [x] **Sprint 093** [🟠 REAL COST AT SCALE] — F-09 — `with_use_mlock(true)` can fail model load on constrained RAM and is all-or-nothing
- [x] **Sprint 094** [🟠 REAL COST AT SCALE] — F-10 — `NonZeroU32::new(self.ctx_size).unwrap()` panics if context window is configured as 0
- [x] **Sprint 095** [⚡ OPTIMIZATION] — F-11 — Regex recompiled on every `parse_token_ceiling_from_error` call
- [x] **Sprint 096** [⚡ OPTIMIZATION] — F-12 — Capability probe buffers the full streaming reply in memory before measuring
- [x] **Sprint 097** [⚡ OPTIMIZATION] — F-13 — `with_n_batch(self.ctx_size)` / `with_n_ubatch(self.ctx_size)` oversize the decode batch
- [x] **Sprint 098** [⚡ OPTIMIZATION] — F-14 — `actor.rs` emits `EVENT_MODEL_READY` for remote providers before any model is loaded/verified

## Sprint 07 — Memory (`sprint-07.md`)

- [x] **Sprint 099** [🔴 WILL BREAK] — F1 — System prompt duplicates `<session_history>` on every `build_context()` call (unbounded growth → context‑window overflow)
- [x] **Sprint 100** [🔴 WILL BREAK] — F2 — Items claimed into transient `processing_*` status are orphaned on restart → facts silently lost
- [x] **Sprint 101** [🟠 REAL COST AT SCALE] — F3 — Empty facts are committed as `superseded` memory_facts instead of dropped
- [x] **Sprint 102** [🟠 REAL COST AT SCALE] — F4 — Compaction generation requested with `max_tokens = 999_999`
- [x] **Sprint 103** [🟠 REAL COST AT SCALE] — F5 — Edge / NLI classifiers reuse `EMBEDDING_TOKENIZER_FILENAME` ("tokenizer.json"); silent no‑op if the classifier model dir lacks that file
- [x] **Sprint 104** [🟠 REAL COST AT SCALE] — F6 — Pipeline `error_count` is hard‑coded to 0 in every stage's metrics
- [x] **Sprint 105** [🟠 REAL COST AT SCALE] — F7 — Embedding dimension mismatch risk with the BGE‑M3 fallback
- [x] **Sprint 106** [🟠 REAL COST AT SCALE] — F8 — Stage 2 embeds items one‑by‑one (no batching) under the 8 GB / sub‑200 ms budget
- [x] **Sprint 107** [⚡ OPTIMIZATION] — F9 — `spawn_blocking` runs two independent CPU sub‑branches but each item is still serial across the batch
- [x] **Sprint 108** [⚡ OPTIMIZATION] — F10 — Narrative facts are embedded then thrown away
- [x] **Sprint 109** [🟡 STYLISTIC] — F11 — Dead code: `format_user_profile_context`, `format_relative_timestamp`, `cosine_similarity`
- [x] **Sprint 110** [🟡 STYLISTIC] — F12 — Two divergent `<user_profile>` assemblers can drift
- [x] **Sprint 111** [🟡 STYLISTIC] — F13 — `is_exact_duplicate` called with a hard‑coded `0.0` cosine in Stage 1
- [x] **Sprint 112** [🟡 STYLISTIC] — F14 — `expect()` panics on tokio runtime construction in the compaction path

## Sprint 08 — Pipeline Orchestration (`sprint-08.md`)

- [x] **Sprint 113** [🔴 WILL BREAK] — F1 — Realtime PTT double-delivers (and triple-counts) the utterance to the cloud provider
- [x] **Sprint 114** [🔴 WILL BREAK] — F2 — Modular passive barge-in does not actually stop playback
- [x] **Sprint 115** [🔴 WILL BREAK] — F3 — Realtime PTT turn-id incoherence (three/four different ids per single interaction)
- [x] **Sprint 116** [🟠 REAL COST AT SCALE] — F4 — `VoxEvent::Cancelled` is ignored by every domain handler; pipeline state can hang
- [x] **Sprint 117** [🟠 REAL COST AT SCALE] — F5 — Six dead public accessors (`is_recording` / `get_buffer_len`)
- [x] **Sprint 118** [🟠 REAL COST AT SCALE] — F6 — `SPEECH_DETECTED` in `modular/ptt.rs` is write-only (dead logic)
- [x] **Sprint 119** [🟠 REAL COST AT SCALE] — F7 — `try_lock` failures silently drop the user's turn / response
- [x] **Sprint 120** [🟠 REAL COST AT SCALE] — F8 — `RwLock::read().unwrap()` / `lock().unwrap()` can panic on poison and are used per-event
- [x] **Sprint 121** [🟠 REAL COST AT SCALE] — F9 — Realtime providers hardcode `turn_id: 0` for all response events
- [x] **Sprint 122** [⚡ OPTIMIZATION] — O1 — Full `VoxSettings` clone on *every* routed event
- [x] **Sprint 123** [⚡ OPTIMIZATION] — O2 — Two DB opens per modular turn
- [x] **Sprint 124** [⚡ OPTIMIZATION] — O3 — `try_lock` per TTS chunk in token/finished handlers
- [x] **Sprint 125** [⚡ OPTIMIZATION] — O4 — Per-event `serde_json::json!({...})` allocations
- [x] **Sprint 126** [🟡 STYLISTIC] — **`modular/context.rs:67` `build_generation_request(..., _turn_id: u32, ...)`** — the `_turn_id`
- [x] **Sprint 127** [🟡 STYLISTIC] — **`realtime/passive.rs` / `realtime/ptt.rs` `on_transcript_final` push user turns to
- [x] **Sprint 128** [🟡 STYLISTIC] — **`mod.rs:34-37` re-imports** (`use crate::core::settings::...`, `use crate::core::state::...`,
- [x] **Sprint 129** [🟡 STYLISTIC] — **`dictation.rs:185` spawns an async task** for `output_router::route_transcript` while the
- [x] **Sprint 130** [❓ UNSURE / QUESTION] — **`pop_last_user_turn` safety on first-turn barge-in (modular/passive.rs:166).** If the user
- [x] **Sprint 131** [❓ UNSURE / QUESTION] — **`is_recording()` / `get_buffer_len()` (F5) — dead or reserved API?** Were these intended as a
- [x] **Sprint 132** [❓ UNSURE / QUESTION] — **`SPEECH_DETECTED` in modular/ptt (F6):** is the absence of a ghost-audio guard in modular PTT

## Sprint 09 — Realtime (`sprint-09.md`)

- [x] **Sprint 133** [🔴 WILL BREAK] — F1 — `Handle::current()` + `block_in_place` panic risk if ever called off-runtime
- [x] **Sprint 134** [🟠 REAL COST AT SCALE] — F2 — Unbounded `audio_tx`/`control_tx` growth after permanent disconnect (8GB leak)
- [x] **Sprint 135** [🟠 REAL COST AT SCALE] — F3 — Handshake blocks a tokio worker thread for up to 5s (violates <200ms)
- [x] **Sprint 136** [🟠 REAL COST AT SCALE] — F4 — No real silent-disconnect detection (no WS ping/pong; Gemini has no keepalive)
- [x] **Sprint 137** [🟠 REAL COST AT SCALE] — F5 — Playback audio dropped on full bridge channel (audible TTS gaps/clipping)
- [x] **Sprint 138** [🟠 REAL COST AT SCALE] — F6 — Gemini `interrupt_active` is only ever set `false`, never `true` (dead suppression)
- [x] **Sprint 139** [🟠 REAL COST AT SCALE] — F7 — Gemini realtime (non-PTT) barge-in sends no server message
- [x] **Sprint 140** [🟠 REAL COST AT SCALE] — F8 — Resampling path is unexercised / `requires_*_resampling` flags are misleading
- [x] **Sprint 141** [🟠 REAL COST AT SCALE] — F9 — `health_check` uses blocking TCP + synchronous DNS (mitigated by caller)
- [x] **Sprint 142** [🟠 REAL COST AT SCALE] — F10 — Deepgram `activity_start`/`activity_end` are silent no-ops (PTT)
- [x] **Sprint 143** [⚡ OPTIMIZATION] — O1 — Double JSON parse of every Gemini message in the hot path
- [x] **Sprint 144** [⚡ OPTIMIZATION] — O2 — Per-frame allocations in the audio hot path
- [x] **Sprint 145** [⚡ OPTIMIZATION] — O3 — One WS text message per audio frame (Gemini)
- [x] **Sprint 146** [⚡ OPTIMIZATION] — O4 — Resampler re-extends `input_buf` each call without reservation
- [x] **Sprint 147** [🟡 STYLISTIC] — S1 — Gemini PCM uses host-endian bytes, not the spec's little-endian
- [x] **Sprint 148** [🟡 STYLISTIC] — S2 — `expect()` on a hardcoded constant parse
- [x] **Sprint 149** [🟡 STYLISTIC] — S3 — i16→f32 divisor is `32768.0` (slightly asymmetric)
- [x] **Sprint 150** [🟡 STYLISTIC] — S4 — `playback_engine.start_playback()` called per chunk
- [x] **Sprint 151** [❓ UNSURE / QUESTION] — **`requires_*_resampling` flags (F8).** Are input/output sample rates ever expected to differ
- [x] **Sprint 152** [❓ UNSURE / QUESTION] — **`session.disconnect()` does not explicitly close the TCP socket** — it signals the orchestrator

## Sprint 10 — STT + TTS (`sprint-10.md`)

- [x] **Sprint 153** [🔴 WILL BREAK] — 🔴-1 — `coalesce_partials` silently discards intermediate partial audio
- [x] **Sprint 154** [🔴 WILL BREAK] — 🔴-2 — Offline STT engines re-decode from scratch each chunk; no streaming state is ever carried
- [x] **Sprint 155** [🔴 WILL BREAK] — 🔴-3 — `ChatterboxRemote` has `timeout(None)` on a blocking client with no overall request timeout → worker thread can wedge forever
- [x] **Sprint 156** [🟠 REAL COST AT SCALE] — 🟠-1 — Dead code: full `SttProvider::transcribe` path is uncalled
- [x] **Sprint 157** [🟠 REAL COST AT SCALE] — 🟠-2 — Dead field: `EmbeddedSttProviderInner::stt_audio_buffer` is never written
- [x] **Sprint 158** [🟠 REAL COST AT SCALE] — 🟠-3 — `ChatterboxRemote` silently ignores configured TTS speed
- [x] **Sprint 159** [🟠 REAL COST AT SCALE] — 🟠-4 — EdgeTTS creates a fresh Tokio runtime + WebSocket per synthesis turn
- [x] **Sprint 160** [🟠 REAL COST AT SCALE] — 🟠-5 — EdgeTTS buffers the entire MP3 then emits one giant `TtsChunk` → no real streaming, barge-in cannot stop emitted audio
- [x] **Sprint 161** [🟠 REAL COST AT SCALE] — 🟠-6 — EdgeTTS auth tokens are hardcoded byte arrays (fragility, not a crash)
- [x] **Sprint 162** [🟠 REAL COST AT SCALE] — 🟠-7 — `set_quality_steps`/`set_speed` do not contend (no defect — logged for transparency)
- [x] **Sprint 163** [⚡ OPTIMIZATION] — OPT-1 — O(n²) re-decode of growing buffer each partial (root latency cause)
- [x] **Sprint 164** [⚡ OPTIMIZATION] — OPT-2 — `resample_44100_to_24000` allocates a full intermediate `Vec<f32>` every callback
- [x] **Sprint 165** [⚡ OPTIMIZATION] — OPT-3 — `apply_speed_stretch`/`apply_speed` allocate a new `Vec` per 2048-sample chunk
- [x] **Sprint 166** [⚡ OPTIMIZATION] — OPT-4 — `transcribe_strides` builds a `String` and `push_str` per stride (repeated alloc)
- [x] **Sprint 167** [⚡ OPTIMIZATION] — OPT-5 — `stitch_transcripts` allocates several `Vec<&str>` + result `Vec` per call
- [x] **Sprint 168** [❓ UNSURE / QUESTION] — **`NEMOTRON_STRIDE_SAMPLES = 8960`** (`stt/mod.rs:19`) — this is a hop of 280 ms at 32 kHz. Is the audio routed to Nemotron actually 32 kHz? `SAMPLE_RATE` is 16000 (used by Qwen). If the audio layer does not upsample to 32 kHz before `Partial` for Nemotron, `transcribe_strides` only ever hits the pad branch (`:52-63`) and decodes padded garbage. **Please confirm the per-provider sample rate in the audio router** (outside this sprint). C60.
- [x] **Sprint 169** [❓ UNSURE / QUESTION] — **`stitch_transcripts` merge rules are heuristic** (`stitcher.rs:132-163`). `is_soft_subslice` returns the *prefix* when the suffix is contained in it; `find_alignment_match` only searches `max_j = 8` suffix words. For highly overlapping partials from Qwen (see 🔴-2) the merge may drop unique tail words or duplicate. The unit tests pass for the chosen examples, but real partials (which overlap heavily) are not covered. **Recommend fuzz/property tests with real overlapping Qwen partials before relying on it.**

## Sprint 11 — Monitoring + Setup (`sprint-11.md`)

- [x] **Sprint 170** [🔴 WILL BREAK] — F1 — Latency telemetry is dead: `InteractionMetric` is never emitted, so STT/TTFT/voice-latency are always `None`
- [x] **Sprint 171** [🟠 REAL COST AT SCALE] — F2 — `system_monitor` does a blocking `crossbeam` send inside the async runtime
- [x] **Sprint 172** [🟠 REAL COST AT SCALE] — F3 — Model download has no timeout and cannot be truly cancelled → setup can hang forever
- [x] **Sprint 173** [🟠 REAL COST AT SCALE] — F4 — `check_disk_space` silently reports "OK" when no matching disk is found
- [x] **Sprint 174** [🟠 REAL COST AT SCALE] — F5 — Archive integrity is not verified after extraction
- [x] **Sprint 175** [🟠 REAL COST AT SCALE] — F6 — Extraction failure leaves a partially-extracted directory behind
- [x] **Sprint 176** [🟠 REAL COST AT SCALE] — F7 — Old model version is deleted before the new one is verified
- [x] **Sprint 177** [🟠 REAL COST AT SCALE] — F8 — `dropped_events` counter is never incremented (dead/misleading metric)
- [x] **Sprint 178** [🟠 REAL COST AT SCALE] — F9 — `RwLock` poisoning silently kills monitoring
- [x] **Sprint 179** [🟠 REAL COST AT SCALE] — F10 — `update_check` manifest cache is read but never written (dead cache, always hits network)
- [x] **Sprint 180** [⚡ OPTIMIZATION] — O1 — Per-webview RAM is always `None` in the live 10Hz snapshot
- [x] **Sprint 181** [⚡ OPTIMIZATION] — O2 — `get_history` clones the entire 600-snapshot deque on every IPC call
- [x] **Sprint 182** [⚡ OPTIMIZATION] — O3 — Redundant syscalls in `system_monitor` emit path
- [x] **Sprint 183** [🟡 STYLISTIC] — S1 — `get_target_window` masks unknown owners
- [x] **Sprint 184** [🟡 STYLISTIC] — S2 — WebView role assignment can mislabel at identical start times
- [x] **Sprint 185** [🟡 STYLISTIC] — S3 — `resolve_temp_dir` creates `./temp` next to the executable as fallback
- [x] **Sprint 186** [🟡 STYLISTIC] — S4 — `check_model_updates` reports whole groups as outdated
- [x] **Sprint 187** [🟡 STYLISTIC] — S5 — `is_newer_version` prerelease handling is heuristic
- [x] **Sprint 188** [🟡 STYLISTIC] — S6 — Doc drift: `snapshot.rs` references owners "Tray, MainWindow, Ptt" that don't exist
