# Trace: LLM Tokens → Harness → TTS → Playback (Modular TurnResponse Seam)

Date: 2026-09-18 | Scope: one assistant turn from `LlmCommand::Generate` to `PlaybackFinished → Ready`, Modular pipeline, `TurnResponse` intent only. Realtime/interim-filler paths named where they branch, not expanded. Mode: trace only, no code changed.

Entry point: `LlmCommand::Generate` posted to the LLM worker inbox (`services/harness/orchestrator.rs:560-567`).
Exit point(s): `PlaybackFinished → Ready` (success); side exits — barge-in → new turn in `Listening`, cancel → `Ready`, stream/turn error → `Ready`/`Error`, worker offload → `Sleeping`.
Depth: every call on the TurnResponse path expanded; outside systems (Ollama/cloud API, sherpa-onnx/ONNX Runtime internals, CPAL/OS audio) marked External.

## Phase 1 — Turn dispatch (how the Generate order is produced)

1. **What:** A finished user text is stored in the shared turn notebook (accumulator), mirrored to the frontend, and a background reply job is started.
   **Who:** Transcript handler (`pipeline/assistant/transcript.rs`) starts it; the router calls it after minting a turn id.
   **Trigger:** Router processed `TextInput` in `Ready` (fresh `next_turn()` + notebook clear), or barge-in minted the turn from `Thinking`/`Speaking`/`Working`.
   **Grounding:** Traced — `transcript.rs:161-204`, `router.rs:161-170`.
   **Owner:** `pipeline/assistant`.

2. **What:** The reply job opens a dedicated two-way message tube (duplex pipe) plus a cancel-flag bridge task, then posts the Generate work order (full request text, turn id, cancel token, reply tube) to the LLM worker's inbox.
   **Who:** Harness turn runner (`services/harness/orchestrator.rs`) initiates; LLM worker (`services/llm/actor.rs`) receives.
   **Trigger:** Step 1 started the reply job.
   **Grounding:** Traced — `orchestrator.rs:550-567`.
   **Owner:** `services/harness`.

3. **What:** The LLM worker files the order: it logs turn, purpose, message count, input chars, temperature, top-p, top-k, max-tokens and seed, opens an inner event tube to the model driver, and runs the driver as a background job.
   **Who:** LLM worker thread initiates; model driver (transport) receives.
   **Trigger:** Step 2 posted `LlmCommand::Generate`.
   **Grounding:** Traced — `services/llm/actor.rs:192-220`.
   **Owner:** `services/llm`.

## Phase 2 — Token streaming (model → harness)

4. **What:** The Ollama driver reads the server's reply stream piece by piece, pulls the text out of each server message, and forwards every non-empty piece as a Token event down the tube; on the server's done-marker it sends Finished and stops.
   **Who:** Transport task initiates each send; LLM worker (step 3's relay loop) receives. The server side itself is outside this codebase.
   **Trigger:** Step 3 started the driver; then each network chunk arriving.
   **Grounding:** Traced — `services/llm/transport/ollama.rs:168-190`; server behavior External — Ollama/cloud HTTP SSE API.
   **Owner:** `services/llm/transport`.

5. **What:** If cancellation is raised mid-stream, the driver sends Finished instead of more tokens and stops (it does not send a Cancelled marker itself).
   **Who:** Transport task.
   **Trigger:** Cancel token firing, checked before every network read.
   **Grounding:** Traced — `transport/ollama.rs:144-166`.
   **Owner:** `services/llm/transport`.

6. **What:** The worker relays each Token event down the reply tube to the harness, skipping sends once cancelled; when the driver ends it forwards Finished (or Cancelled if the turn was cancelled).
   **Who:** LLM worker initiates; harness stream router receives.
   **Trigger:** Each event arriving on the inner tube from step 4/5.
   **Grounding:** Traced — `services/llm/actor.rs:222-250`.
   **Owner:** `services/llm`.

## Phase 3 — Stream routing (harness egress)

7. **What:** The stream router reads reply-tube messages one at a time on a blocking worker thread; each loop first checks the shared cancel flag — on cancel it announces a Cancelled pipeline event and returns whatever reply text was collected so far.
   **Who:** Stream router (`services/harness/stages/streaming/router.rs`).
   **Trigger:** Each message arriving on the reply tube from step 6.
   **Grounding:** Traced — `streaming/router.rs:52-67`.
   **Owner:** `services/harness`.

8. **What:** For every Token piece, in this order: (a) the raw piece is forwarded to the frontend subtitle feed, (b) the piece is appended to the full-reply notebook and fed to the clause splitter, (c) any finished clauses the splitter returns are posted to TTS, (d) no filtering of any kind is applied — the file header states tag filtering is deferred and text flows straight through to chunker, subtitles and history.
   **Who:** Stream router initiates all four; frontend IPC, turn notebook (accumulator), and TTS inbox receive.
   **Trigger:** A `Token` message in the loop of step 7.
   **Grounding:** Traced — `streaming/router.rs:114-131`, `pipeline/assistant/accumulator.rs:40-43`; (d) Traced — `streaming/router.rs:1-2`.
   **Owner:** `services/harness` (+ `pipeline/assistant` for the notebook).

9. **What:** For every clause bound for speech: a numbering stamp is claimed, the shared pending-jobs counter is raised by one, and a Generate work order (turn id, clause text, TurnResponse tag) is posted to the TTS inbox; each posting is logged with turn, clause number, char/word counts and pending total.
   **Who:** Stream router initiates; TTS worker receives.
   **Trigger:** Step 8(c) receiving one or more finished clauses.
   **Grounding:** Traced — `streaming/router.rs:133-176`.
   **Owner:** `services/harness`.

10. **What:** On Finished: any leftover unpunctuated text is flushed as one final clause (same counting/posting/logging), a "stream finished" line is logged with total reply chars/words, and an `LlmFinished` pipeline event is posted.
    **Who:** Stream router initiates; TTS inbox and central event pump receive.
    **Trigger:** `Finished` message in the loop of step 7.
    **Grounding:** Traced — `streaming/router.rs:107-111`, `:178-221`, `:233-259`.
    **Owner:** `services/harness`.

11. **What:** On stream error an Error pipeline event is posted and the turn fails; if the tube closes with no Finished and no cancel, an error is returned instead.
    **Who:** Stream router initiates; central event pump receives.
    **Trigger:** `Error` message, or tube disconnect.
    **Grounding:** Traced — `streaming/router.rs:77-105`.
    **Owner:** `services/harness`.

12. **What:** Back in the reply job: on a clean finish with no cancel, the full reply text is written into conversation memory (history) and the turn reports Completed; on cancel it reports Cancelled and rolls the user turn back.
    **Who:** Harness turn runner; history store receives.
    **Trigger:** Step 10's return value (reply text or error) arriving back in the reply job.
    **Grounding:** Traced — `services/harness/orchestrator.rs:610-638`.
    **Owner:** `services/harness`.

## Phase 4 — Clause splitting rules

13. **What:** The splitter keeps an internal text buffer; every push appends the new piece and carves off each finished clause it can find, in order.
    **Who:** Clause splitter (`services/harness/stages/streaming/chunker.rs`), driven by step 8(b).
    **Trigger:** Each `push_str` call with the newest token text.
    **Grounding:** Traced — `chunker.rs:17-21`, `:140-159`.
    **Owner:** `services/harness`.

14. **What:** Newline, question mark and exclamation mark always split immediately, with no minimum-length check.
    **Who:** Clause splitter.
    **Trigger:** One of those three characters appearing anywhere in the buffer.
    **Grounding:** Traced — `chunker.rs:68-71`.
    **Owner:** `services/harness`.

15. **What:** Commas, semicolons, colons and dashes split only once the words before them reach the stage minimum; periods split likewise unless the period sits inside digits (3.14) or after a known abbreviation, and a too-early period is rewritten into a comma so the voice keeps a rising pitch. Word minimums grow per clause: (5,8,12), then (10,15,20), then (16,24,32) words.
    **Who:** Clause splitter.
    **Trigger:** One of those characters appearing in the buffer during a scan.
    **Grounding:** Traced — `chunker.rs:51-58`, `:73-119`.
    **Owner:** `services/harness`.

16. **What:** With no punctuation at all and the buffer past the stage maximum word count, the splitter cuts at a whitespace near the stage target count instead of waiting forever.
    **Who:** Clause splitter.
    **Trigger:** Buffer word count reaching the stage maximum.
    **Grounding:** Traced — `chunker.rs:122-134`.
    **Owner:** `services/harness`.

17. **What:** Nothing in the splitting rules reads arrival speed: thresholds depend only on clause count and characters seen. There is no token-rate (TPS) input anywhere in the splitter.
    **Who:** Clause splitter (statement about the file's contents).
    **Trigger:** Not applicable — describes what the code does not consult.
    **Grounding:** Traced — `chunker.rs:51-58` and `:61-137` contain no rate or timing references.
    **Owner:** `services/harness`.

18. **What:** Leftover text with no closing punctuation only leaves the splitter at stream end, through the flush call in step 10.
    **Who:** Clause splitter, called by the stream router.
    **Trigger:** `flush()` after the Finished message.
    **Grounding:** Traced — `chunker.rs:23-33`.
    **Owner:** `services/harness`.

## Phase 5 — TTS synthesis

19. **What:** One dedicated worker thread pulls TTS work orders strictly one at a time; each Generate logs job number, turn, intent, char/word counts and pending total before synthesizing.
    **Who:** TTS worker (`services/tts/actor.rs`).
    **Trigger:** Each `Generate` order posted in step 9/10.
    **Grounding:** Traced — `services/tts/actor.rs:53-82`.
    **Owner:** `services/tts`.

20. **What:** Voice, speed and quality-step change orders take effect on the live provider between synthesis jobs, never mid-job.
    **Who:** TTS worker applies; provider stores.
    **Trigger:** `SetVoice`/`SetSpeed`/`SetQualitySteps` orders arriving on the worker inbox.
    **Grounding:** Traced — `services/tts/actor.rs:142-153`.
    **Owner:** `services/tts`.

21. **What:** Per clause the Kokoro provider first bails out quietly if cancelled, and refuses Devanagari text with an error event (the model cannot speak it).
    **Who:** Kokoro provider (`services/tts/providers/kokoro.rs`).
    **Trigger:** Start of `synthesize_chunk` for the clause.
    **Grounding:** Traced — `kokoro.rs:136-155`.
    **Owner:** `services/tts`.

22. **What:** The provider reads the live voice id and speed, builds generation settings (voice, speed, silence scale), and runs the neural-net inference while holding the provider lock.
    **Who:** Kokoro provider initiates; the model runtime computes (External — sherpa-onnx/ONNX Runtime internals).
    **Trigger:** Step 21 passing the guards.
    **Grounding:** Traced — `kokoro.rs:157-181`.
    **Owner:** `services/tts`.

23. **What:** After inference, cancelled turns drop their audio silently; a failed inference with no cancel reports an error instead.
    **Who:** Kokoro provider.
    **Trigger:** Inference returning (or failing).
    **Grounding:** Traced — `kokoro.rs:183-195`.
    **Owner:** `services/tts`.

24. **What:** Edge quiet below −45dBFS is trimmed (keeping 15ms attack / 40ms release cushions), a 10ms smooth fade is applied at both ends of the piece, and the piece is handed to the playback feeder with its intent tag.
    **Who:** Kokoro provider initiates; playback engine (`services/audio/playback.rs`) receives.
    **Trigger:** Successful inference with non-empty trimmed audio and no cancel.
    **Grounding:** Traced — `kokoro.rs:197-200`, `:230-271`.
    **Owner:** `services/tts` (+ `services/audio` for the feeder).

25. **What:** The job is timed wall-clock; seconds-of-audio and RTF (elapsed ÷ audio) are logged, and the RTF value is stored in the shared telemetry slot.
    **Who:** Kokoro provider writes; monitoring snapshot reads the slot later.
    **Trigger:** Step 24 completing ingest.
    **Grounding:** Traced — `kokoro.rs:202-225`.
    **Owner:** `services/tts`.

26. **What:** Job completion ticks the shared pending counter down by one; when it was the last outstanding job, the playback pre-roll gate is released early.
    **Who:** TTS worker initiates; playback engine receives the flush.
    **Trigger:** Each job finishing in step 19's loop.
    **Grounding:** Traced — `services/tts/actor.rs:98-115`.
    **Owner:** `services/tts`.

## Phase 6 — Playback gating and drain

27. **What:** The feeder stretches each 24kHz piece to 48kHz, appends it to the shared sound queue (ring buffer), and the first time the queue holds at least the modular pre-roll threshold (12,000 samples) it announces PlaybackStarted; overflow beyond queue capacity is dropped with a warning.
    **Who:** Playback engine (`services/audio/playback.rs`).
    **Trigger:** Each piece ingested in step 24.
    **Grounding:** Traced — `playback.rs:104-155`; threshold value `services/audio/mod.rs:20`.
    **Owner:** `services/audio`.

28. **What:** When all synthesis jobs are done, a flush call announces PlaybackStarted immediately if sound is still queued but the gate never fired.
    **Who:** Playback engine, called by the TTS worker (step 26).
    **Trigger:** Pending counter reaching zero with an un-armed turn and non-empty queue.
    **Grounding:** Traced — `playback.rs:197-227`.
    **Owner:** `services/audio`.

29. **What:** The sound-card callback (which asks for samples; the OS side initiates) outputs quiet and clears the queue when the turn is neither armed nor Speaking, or when cancelled.
    **Who:** Output callback (`services/audio/sink.rs`); sound card driver (External — CPAL/OS) drives the timing.
    **Trigger:** Each hardware callback tick.
    **Grounding:** Traced — `sink.rs:64-95`.
    **Owner:** `services/audio`.

30. **What:** Otherwise the callback pops queued sound frame by frame into stereo, eases volume toward full in small ramp steps, and measures loudness plus three frequency bands into shared telemetry slots.
    **Who:** Output callback.
    **Trigger:** Hardware callback tick with an armed or Speaking turn.
    **Grounding:** Traced — `sink.rs:104-145`, `:186-220`.
    **Owner:** `services/audio`.

31. **What:** An empty queue with synthesis still pending counts one underrun (gap counter) and holds the last sound value while easing volume toward zero.
    **Who:** Output callback; gap counter slot receives.
    **Trigger:** Queue draining faster than step 24 refills it.
    **Grounding:** Traced — `sink.rs:147-153` with hold/ramp at `:113-141`.
    **Owner:** `services/audio`.

32. **What:** An empty queue with nothing pending and an armed turn announces PlaybackFinished carrying total played samples and seconds, then resets its counters.
    **Who:** Output callback initiates; central event pump receives.
    **Trigger:** Queue empty + pending counter at zero + turn armed.
    **Grounding:** Traced — `sink.rs:154-183`.
    **Owner:** `services/audio`.

33. **What:** Cancel wipes the queue, disarms the turn and resets counters; a separate discard signal does the same on demand.
    **Who:** Playback engine (`cancel()`) and output callback act together.
    **Trigger:** `cancel_flag` raised (pause/interrupt) or an explicit discard request.
    **Grounding:** Traced — `playback.rs:270-278`, `sink.rs:64-72`, `:80-89`.
    **Owner:** `services/audio`.

## Phase 7 — Turn finalization

34. **What:** PlaybackStarted for a reply while Thinking or Working moves the pipeline to Speaking (and mirrors it to the frontend); in any other state it is dropped.
    **Who:** Playback handler (`pipeline/assistant/playback.rs`), called by the central pump.
    **Trigger:** Step 27/28's `PlaybackStarted` event.
    **Grounding:** Traced — `assistant/playback.rs:13-44`, `pipeline/router.rs:187-189`.
    **Owner:** `pipeline`.

35. **What:** PlaybackFinished while Speaking with zero pending jobs moves the pipeline to Ready; with jobs still pending it is deferred and the turn stays Speaking.
    **Who:** Playback handler, called by the central pump.
    **Trigger:** Step 32's `PlaybackFinished` event.
    **Grounding:** Traced — `assistant/playback.rs:46-90`, `pipeline/router.rs:190-192`.
    **Owner:** `pipeline`.

36. **What:** `LlmFinished` while Thinking/Speaking/Working releases the pre-roll gate early (modular path), takes the full reply plus user text out of the notebook, logs the complete text with char/word counts, and files the turn in the save queue for the database worker.
    **Who:** LLM-finalize handler (`pipeline/assistant/llm.rs`), called by the central pump.
    **Trigger:** Step 10's `LlmFinished` event.
    **Grounding:** Traced — `assistant/llm.rs:33-84`, `pipeline/router.rs:184-186`.
    **Owner:** `pipeline`.

37. **What:** The central pump is a single max-priority OS thread doing a blocking read of the event tube and routing one event at a time; a Shutdown event stops it.
    **Who:** Router pump (`pipeline/router.rs`).
    **Trigger:** Each event posted to the tube; thread started once at engine boot.
    **Grounding:** Traced — `pipeline/router.rs:206-234`.
    **Owner:** `pipeline`.

38. **What:** Every state change is logged with before/after states, owner, both modes and turn id, and mirrored to the frontend as a state event.
    **Who:** Transition helper (`pipeline/router.rs`), called by every handler above.
    **Trigger:** Any handler deciding a state move.
    **Grounding:** Traced — `pipeline/router.rs:57-105`.
    **Owner:** `pipeline`.

## Phase 8 — Side exits (interrupt, pause, offload, error)

39. **What:** Barge-in runs a fixed sequence: stop sound output, raise cancel, cancel the turn token, zero the pending counter, file the partial turn in the save queue, clear the notebook, mint a fresh turn id, lower cancel, move to Listening.
    **Who:** Interrupt handler (`pipeline/assistant/interrupt.rs`), called by the router.
    **Trigger:** Speech, PTT action, or text input arriving while Thinking/Speaking/Working.
    **Grounding:** Traced — `interrupt.rs:14-86`, `pipeline/router.rs:162-166`.
    **Owner:** `pipeline`.

40. **What:** Pause raises cancel, cancels the turn token, clears the notebook, stops sound output, and moves to Paused; pause orders in Idle, Paused or Sleeping are dropped without effect.
    **Who:** Session handler (`pipeline/assistant/session.rs`), called by the central pump.
    **Trigger:** `PauseSession` event (user button, auto-pause timer, or lifecycle monitor).
    **Grounding:** Traced — `assistant/session.rs:302-319` (guard), `:316-329` (cancel/clear/stop).
    **Owner:** `pipeline`.
    **Note:** the Sleeping arm of this guard was added 2026-09-18; the spec table (`docs/specs/events-spec.md:111`) already required it.

41. **What:** Five minutes of continuous Paused sends shutdown orders to the LLM and TTS workers (their channel senders are taken, so later sends fail closed), trims heap, and moves to Sleeping.
    **Who:** Idle lifecycle monitor (`pipeline/lifecycle.rs`).
    **Trigger:** 5-minute timer firing while state is still Paused.
    **Grounding:** Traced — `pipeline/lifecycle.rs:47-75`.
    **Owner:** `pipeline`.

42. **What:** Worker shutdown orders end the LLM and TTS loops and drop their model instances; a later resume must re-warm them before any turn can run (a turn attempted with no LLM channel fails with "No LLM channel available" and rolls back).
    **Who:** Worker loops act on the order; harness reports the missing channel.
    **Trigger:** Step 41's shutdown orders.
    **Grounding:** Traced — `services/llm/actor.rs:142-149`, `services/tts/actor.rs:154-161`, `services/harness/orchestrator.rs:537-548`.
    **Owner:** `services/llm`, `services/tts`, `services/harness`.

43. **What:** Resume from Paused, Sleeping or Error clears cancel, renews the turn token, hands ownership back to the assistant, re-arms voice detection, and moves to Ready (failures move to Error with a notification).
    **Who:** Session handler, called by the central pump.
    **Trigger:** `ResumeSession` event.
    **Grounding:** Traced — `assistant/session.rs:369-451` (entry guards and Ready/Error landings; VAD re-arm in between).
    **Owner:** `pipeline`.

## Appendix A — Metrics inventory (what is measured and logged today)

Per-turn log lines (all `log::info!` unless noted):

| What | Where | Values carried |
|---|---|---|
| LLM request filed | `services/llm/actor.rs:201-212` | turn, purpose, message count, input chars, temperature, top-p, top-k, max-tokens, seed |
| Clause posted to TTS | `services/harness/stages/streaming/router.rs:165-173` | turn, clause number, chars, words, intent, pending-jobs total |
| Remainder flushed | `streaming/router.rs:211-219` | turn, clause number, chars, words, intent, pending total |
| Stream finished | `streaming/router.rs:246-251` | turn, reply chars, reply words |
| Turn context load | `services/harness/orchestrator.rs:382-387` | turn, context-window utilization %, status word |
| Turn committed | `services/harness/orchestrator.rs:624-628` | turn, reply chars |
| Clause synthesis start | `services/tts/providers/kokoro.rs:159-164` | turn, clause text, voice id |
| Clause synthesis done | `kokoro.rs:216-221` | turn, seconds of audio, RTF (elapsed ÷ audio) |
| TTS job start/finish | `services/tts/actor.rs:67-75`, `:112-115` | job number, turn, intent, chars, words, pending/remaining totals, pre-roll-flush flag |
| Pre-roll gate fired | `services/audio/playback.rs:147-152` | queued samples, turn, intent |
| Pre-roll flushed early | `playback.rs:218-224` | queued samples, turn, intent |
| Playback done | `services/audio/sink.rs:166-173` | turn, intent, played samples, seconds played |
| State move | `pipeline/router.rs:70-78` | before/after states, owner, both modes, turn id |
| User text captured | `pipeline/assistant/transcript.rs:167-171` | turn, query text |
| Turn filed to DB queue | `pipeline/assistant/llm.rs:70-77` | turn, reply chars/words, full text |
| Voice/speed hot-swap | `services/tts/actor.rs:143-149` | new voice index / speed |
| Interrupt handled | `pipeline/assistant/interrupt.rs:79-83` | interrupted + new turn ids |
| Pause/resume/offload | `assistant/session.rs:366`, `:450`, `pipeline/lifecycle.rs:51` | state names only |
| Buffer overflow drops | `services/audio/playback.rs:127-132` | dropped sample count (`warn!`) |
| Dead-channel turn failure | `services/harness/orchestrator.rs:539`, `:569` | turn id (`error!`) |

Shared counters (atomics; read by the monitoring snapshot at `monitoring/snapshots.rs:297-303`, served via runtime-snapshot IPC — not logged per turn):

| Counter | Writer |
|---|---|
| `playback_underruns` (gap count) | `services/audio/sink.rs:153` |
| `latest_tts_rtf` (last clause RTF) | `services/tts/providers/kokoro.rs:223-225` |
| `played_samples` (per-turn drain total) | `services/audio/sink.rs:145` |
| Loudness + 3 frequency bands | `services/audio/sink.rs:186-220` |

Not measured anywhere on the active remote-LLM path: per-turn time-to-first-token, tokens-per-second, per-stage latency split (LLM vs chunk-wait vs synthesis vs queued-behind-prior-clause), chunk queue wait times, time-to-first-audio. Time-to-first-token and tokens-per-second logging exists only in the embedded local-LLM path (`services/llm/embedded/generate.rs:497-505`) and in model-capability probing (`services/llm/catalog/probe.rs:495-517`), neither of which runs on this path.

## Appendix B — Chunking algorithm in full (`ClauseChunker`)

Source: `services/harness/stages/streaming/chunker.rs:1-160` (unit tests `:190-320`). The instance is a field of the shared turn notebook (`pipeline/assistant/accumulator.rs:8`): the stream router feeds it through `push_token` on every token (`accumulator.rs:40-43`, driven from `streaming/router.rs:114-131`), drains it through `flush_chunker` at stream end (`accumulator.rs:46-48`, called at `streaming/router.rs:180`), and it is wiped whenever the notebook is cleared — interrupt (`pipeline/assistant/interrupt.rs:73`), pause (`pipeline/assistant/session.rs:318`), fresh turn (`pipeline/router.rs:169`).

**B1. State.** Two fields only: `buffer: String` (unconsumed text) and `chunk_index: usize` (clauses emitted so far this turn, starts 0, reset by `clear`). No timers, no rate/throughput input, no token counts — the algorithm sees characters and words only.

**B2. Threshold ladder.** Each emitted clause advances `chunk_index`, which selects the (minimum, target, maximum) word counts (`:51-58`):

| chunk_index | w_min | w_target | w_max |
|---|---|---|---|
| 0 (first clause) | 5 | 8 | 12 |
| 1 (second clause) | 10 | 15 | 20 |
| 2+ (steady state) | 16 | 24 | 32 |

**B3. Single left-to-right scan.** `find_split_point` (`:61-137`) collects the buffer's char indices once, then walks them in order and returns the first split it accepts, as a (byte position, delimiter byte length) pair. Rules in scan order:

1. **Strong terminators split unconditionally** — newline, `?`, `!`. No word-count check (`:68-71`).
2. **Weak terminators split only past the minimum** — `,`, `;`, `:`, em-dash `—`, en-dash `–`. The words before the mark must reach w_min (`:73-81`).
3. **Periods are guarded three ways** (`:83-119`):
   - digit on both sides (`3.14`) → skipped;
   - the word before the period is a known abbreviation → skipped (list in B5);
   - words before the period reach w_min → split. If not, and the next character is whitespace, the period is **rewritten to a comma in place** (`:114-118`) so the clause keeps flowing instead of ending short — e.g. `"Hai Addy. I'm Vox."` becomes `"Hai Addy, I'm Vox, ..."` (test `:233-238`). The in-code comment attributes this to a previous TTS engine (StyleTTS2).
4. **Emergency fallback** (`:122-134`): only if the whole scan found no split point AND the buffer holds at least w_max words. Then it cuts at the whitespace at or after w_target words — e.g. 30 unpunctuated words at index 0 emit an 8-word first chunk (test `:263-273`).

**B4. Extraction loop.** `extract_chunks` (`:140-159`) repeats: find split → carve buffer at `pos + len` → trim → emit (bumping `chunk_index`) — until no split is found or the buffer is empty. So one `push_str` can emit several clauses, and every clause after the first is judged against the next rung of the ladder. Emission order is buffer order; nothing is reordered.

**B5. Abbreviation list** (`is_abbreviation`, `:162-188`): dr, mr, mrs, ms, prof, sr, jr, st, vs, e.g, i.e, etc, approx, dept, fig, ver, vol, inc, ltd, co, no, p, pg, pp (case-insensitive), plus `v` + digits (`v2`, `v10`), plus any single uppercase letter (`J`). Matching strips non-alphanumeric edges off the preceding word first (`:101-105`), so `"Smith,"` still matches.

**B6. Lifecycle edges.** `flush` (`:24-33`) returns the trimmed remainder as one final clause (bumping the index) or `None` on empty; `clear` (`:36-39`) empties the buffer and resets the index to 0 — reached via the notebook's `clear` (`accumulator.rs:32-37`) on interrupt, pause and fresh-turn paths — so the next turn restarts the ladder from the (5,8,12) rung; `is_empty` (`:47-49`) treats whitespace-only as empty.

**B7. Worked shape.** For a typical reply, clause 1 leaves after 5+ words at weak punctuation (or instantly at `?`/`!`/newline), clause 2 needs 10+, and every clause from the third on needs 16+ words before weak punctuation may split it — strong terminators and the 32-word emergency cap excepted throughout.
