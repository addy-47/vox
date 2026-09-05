---
title: "Pipeline Prompt Budget Guard — Robust LLM Prefill Under Tight Context Windows"
audience: "Internal — backend (Rust) contributors, ML/research, system architects, agents"
last_updated: 2026-09-05
owners: "backend-engineer role (production), ml-research-engineer (model sizing), test-engineer (bench coverage)"
related_docs:
  - "docs/plans/phase11/integration_test_spec.md — Seams 1–11, 15–17 (LLM/turn-context stage coverage)"
  - "docs/plans/phase11/recent_work.md — Phase 11 work ledger"
  - "docs/features/voice-flow.md — End-to-end voice pipeline reference"
  - "app/src-tauri/src/services/llm/embedded/generate.rs — Chunked prefill + KV cache reuse"
  - "app/src-tauri/src/services/llm/embedded/worker.rs — LlamaContext n_ctx allocation"
  - "app/src-tauri/src/services/harness/accountant.rs — Token budgeting & FIFO shifts"
  - "app/src-tauri/src/services/harness/manager.rs — Identity-fact preload"
  - "app/src-tauri/benches/pipeline_bench.rs — E2E bench (default 2048 ctx → NoKvCacheSlot)"
---

# Pipeline Prompt Budget Guard — Robust LLM Prefill Under Tight Context Windows

> **Type:** Production behavior spec (single concern: prevent pipeline failure when the assembled conversation prompt exceeds the active LLM `context_window`).
> **Status:** Draft — awaiting approval. **Raised by** `pipeline_bench` failure surfaced on 2026-09-05.
> **Source of truth for the bug:** `app/src-tauri/benches/pipeline_bench.rs` real log (Nemotron + Qwen 0.8B Q4 + Kokoro, default settings) — see §6.
> **Out of scope:** model context-window defaults (separate ticket), identity-fact overload policy at ingest (separate ticket), KV-cache eviction policies (separate ticket).

---

## 1. How to read this doc

- **Audience:** backend engineer (implements), ML research (signs off on truncation strategy), test engineer (adds Seam 18 coverage), system architect (signs off on defaults).
- **Scope:** The boundary between the **conversation manager's** assembled prompt (system + identity facts + prior turns + current user turn) and the **embedded LLM actor's** prefill. When the assembled prompt exceeds `settings.llm.context_window`, the pipeline must not crash. It must **truncate predictably, warn the user, and continue**.
- **Convention:** claims cite `path/file.rs:line`; no invented code blocks. The minimal fix is named explicitly per §5.
- **Non-goals:** changing `context_window` defaults, increasing `n_ctx` allocation, swapping to a larger model, dropping identity facts at ingest, async background compaction. Those are separate.
- **SSOT for the model-facing knobs:** `core/settings.rs` `LlmSettings::context_window`, `core/defaults.rs` `DEFAULT_LLM_CONTEXT_WINDOW` (currently `2048`).
- **SSOT for the LLM worker prefill path:** `services/llm/embedded/generate.rs:183` `prefill_or_reuse_kv_cache`, `services/llm/embedded/worker.rs:100` `init_context` (`with_n_ctx(effective_ctx)`).

---

## 2. The bug, in one sentence

When the assembled prompt (system prompt + preloaded identity facts + prior turns + current user turn) exceeds `settings.llm.context_window`, the **embedded LLM actor's chunked prefill returns `Decode Error 1: NoKvCacheSlot` from llama.cpp**, the LLM worker emits `VoxEvent::Error` instead of streaming tokens, the TTS pipeline receives zero clauses, and the user sees the pipeline stop responding mid-turn — with no warning that the failure was caused by prompt size, and no graceful recovery.

---

## 3. How the failure surfaces today (the chain of evidence)

Captured live on 2026-09-05 from `cargo bench --bench pipeline_bench -- --mode modular_passive --stt nemotron --llm qwen --tts kokoro` with `RUST_LOG=debug` and `RUST_BACKTRACE=full`. Full log in `temp/pipeline_bench_20260905_162703.log` (created by this work). The relevant chronological slice:

```
1.  [Harness] Identity preload completes
    [ConversationManager] Successfully preloaded 207 Identity facts into System Prompt.

2.  [Harness] Token budget overflow
    [Harness] Critical threshold reached (195.6% utilization). Performing Maintenance...
    [TokenAccountant] FIFO shift complete. Retained 2 messages (3004 tokens, utilization 195.6%).

3.  [LLM] LLM context is allocated
    [LLM] Model loaded. family=Qwen ctx_size=2048 n_threads=4
    ... (later)
    [LLM] Lazy initializing LlamaContext on stable execution address...

4.  [LLM] Prefill begins
    [LLM] KV cache miss (turn: 1). Prefilling full conversation context...

5.  [LLM] Prefill fails (the bug)
    [LLM Worker] Generation error (turn 1):
      Engine error: [LLM] Full prompt decode failed: Decode Error 1: NoKvCacheSlot

6.  [Pipeline] Pipeline halts
    [LLM Actor] Sent VoxEvent::Error(LlmActor) → router → [Pipeline::Error] Error on turn 1
    [Audio::Playback] Cancelled — buffer signal sent
    [VAD Actor] Ingestion gate closed — purged in-flight audio buffers
```

Stage-by-stage scoreboard for the same run:

| Stage                                   | Outcome | Evidence                                                                                                                               |
| --------------------------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Audio capture (cpal / bench ringbuffer) | OK      | 7.83 s clip streamed in 16 ms chunks                                                                                                   |
| VAD                                     | OK      | `SpeechStart` + `SpeechEnd` emitted, state `Listening → Ready`                                                                         |
| STT (Nemotron)                          | OK      | Final transcript "You check my calendar and give me a quick briefing on today's scheduled meetings" emitted                            |
| Conversation manager                    | **Bad** | 207 identity facts preloaded → 3004 tokens against `usable_budget = 2048 − 512 = 1536` → 195.6% utilization → FIFO shift cannot reduce |
| LLM prefill (Qwen 0.8B Q4)              | **Bug** | `n_ctx=2048` < `prompt_tokens=3004` → `Decode Error 1: NoKvCacheSlot`                                                                  |
| TTS (Kokoro)                            | Starved | 0 clauses dispatched → 0.00 s synthesized audio                                                                                        |
| Playback                                | Idle    | No `PlaybackStarted` ever fired                                                                                                        |

Net user experience: voice input is acknowledged, transcript is shown, then the assistant goes silent for the remainder of the 60 s bench window. No warning. No recovery. The `VoxEvent::Error` is logged at ERROR level and the toast gate (`should_show_error_toast`) decides whether the user sees anything — and even when they do, the message is the raw "Engine error: [LLM] Full prompt decode failed: Decode Error 1: NoKvCacheSlot", which is not actionable.i

---

## 4. Why this is a production bug, not a bench-only artifact

The bench was not configured to fail deliberately. The settings used (`VoxSettings::default()`) match the persisted `~/.vox/settings.json` (`context_window: 2048`). The 207 identity facts were preloaded from the **production** `vox.db` (the user has been running Vox with substantial memory; the assistant behavior and DB state are real). The transcript that triggered the failed prefill is a normal "briefing" prompt, not a stress test. Any user who has accumulated ≥ ~50 identity facts and asks Vox a normal question on a 2048-context-window LLM will hit this exact failure mode. The bench simply reproduces what the user would experience.

---

## 5. Must Be True

### A. Pipeline never breaks on prompt overflow

1. The LLM prefill path must detect, at the start of `prefill_or_reuse_kv_cache` (`services/llm/embedded/generate.rs:183`), when the assembled prompt token count exceeds the available KV-cache slots (i.e. exceeds `effective_ctx` minus the reserved output budget).
2. On detection, the prefill path must **truncate the conversation to fit** (drop oldest non-system messages first, keep the system prompt + the current user turn intact) rather than calling `ctx.decode` with a prompt that will fail.
3. The truncation must be **logged at WARN level with a single, clear, actionable message** naming the dropped tokens, the source (which message IDs were dropped), and the new token count — not a raw llama.cpp decode error.
4. The truncation must be **emitted to the frontend as a `toast` and as a `notifications` record** with a stable category (`prompt_truncated`) and message ("Trimmed N oldest messages to fit the LLM context window."). The user must see this once per turn where truncation occurred; the bench's `should_show_error_toast` gate must default to `true` for this category.
5. If, after truncation, the prompt still does not fit (e.g. system prompt alone > `context_window`), the LLM actor must **emit a single `VoxEvent::Error`** with `source = "LlmActor"`, `message = "System prompt exceeds LLM context window. Reduce identity facts or increase context_window in Settings."` and the pipeline must transition to `Ready` (not to `Error`) so the user can retry after fixing the configuration.

### B. Pipeline bench is not a memory retrieval workload

6. The pipeline bench must **disable memory context retrieval** (`settings.memory.context_retrieval_enabled = false`) by default. The bench measures audio-in → audio-out pipeline latency and throughput; memory retrieval is a separate workload with its own benchmarks and is not part of the live turn path being measured.
7. The bench must **clear the conversation manager's preloaded identity facts** before each run, so two consecutive runs of the bench do not see the first run's accumulated identity facts polluting the second. The clean mechanism is a fresh ephemeral `vox.db` fixture per bench invocation (parallel to the existing `tests/common/paths.rs` `TempPathsGuard` pattern).
8. The bench must **record the prefill result (success vs. truncated vs. error) as a first-class field in `report.json` and `latest.json`** so any regression in this behavior is caught by inspecting the artifact, not by reading stdout.

### C. Observable surface

9. The LLM worker must emit a new structured event `VoxEvent::PromptTruncated { turn_id, dropped_messages, original_tokens, truncated_tokens }` to the central event router when truncation occurs. The router must route this to both the conversation manager (to update the persisted transcript view) and to the frontend (as `IpcEvent::Notification`). This is the production seam for the user-visible warning.
10. The LLM worker must continue to emit `VoxEvent::LlmFinished` (not `VoxEvent::Error`) on successful post-truncation generation, so the pipeline's success-path telemetry is not polluted by recoverable truncation.

### D. Backwards compatibility / migration

11. Existing `VoxSettings` JSON files (with `context_window: 2048`) must continue to deserialize without modification. No migration is required.
12. The `report.json` schema gains a single new field (`prefill: { outcome, original_tokens, truncated_tokens, dropped_messages }`). Existing readers that ignore unknown fields continue to work. The schema bump is documented in `docs/tests/pipeline_benchmark_report.md`.

---

## 6. Minimal fix (named explicitly, per the user's instruction)

The minimal production fix is **bounded truncation at the LLM worker boundary, plus an event-surfaced warning, plus bench isolation from memory state**. Specifically:

1. In `app/src-tauri/src/services/llm/embedded/generate.rs` `prefill_or_reuse_kv_cache`, before the chunked decode loop, compute `available = effective_ctx - reserved_output_tokens - safety_margin`. If `prompt_tokens.len() > available`, partition the message list into `[system_prompt] + [keep_tail]`, drop oldest messages from `keep_tail` until the recomputed token count fits, and proceed with the truncated prompt. Emit `WARN` + `VoxEvent::PromptTruncated`.
2. In `app/src-tauri/src/core/events.rs`, add the `VoxEvent::PromptTruncated { turn_id, dropped_messages, original_tokens, truncated_tokens }` variant. In `app/src-tauri/src/pipeline/router.rs` `route_event`, route it to the conversation manager (drop the dropped messages from the in-memory transcript) and to IPC as a new `IpcEvent::Notification` with `category = "prompt_truncated"`. In `app/src-tauri/src/toast.rs`, ensure `should_show_error_toast` defaults to `true` for this category.
3. In `app/src-tauri/benches/pipeline_bench.rs` and `app/src-tauri/benches/common/pipeline_harness.rs`, set `settings.memory.context_retrieval_enabled = false` before `setup_e2e_pipeline`; in the bench, after `setup_e2e_pipeline`, call `state.conversation_manager.lock().clear()` to drop the preloaded identity facts; record `prefill` outcome into the benchmark report.
4. In `app/src-tauri/src/services/llm/embedded/generate.rs` `generate`, when the system prompt alone does not fit in `effective_ctx`, return a typed error (not a raw llama.cpp decode error) and have the LLM actor emit `VoxEvent::Error` with the user-actionable message from §5.1.5, then transition the pipeline to `Ready`.

No production code is rewritten. The fix is strictly additive (one new event variant, one new router arm, one new IPC event, one truncation guard at the prefill boundary, one bench config knob).

---

## 7. False-green audit (per `/create-test` discipline)

For each "Must Be True" item, the table below names what would falsely pass a naive test.

| #   | Must Be True                                       | What a false-green test would look like                                         | What a real test must do                                                                                                                                                                           |
| --- | -------------------------------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 5.1 | Truncate-and-continue on overflow                  | Mock the LLM to return tokens regardless of prompt size. Pass.                  | Use the real `EmbeddedProvider` with the real Qwen 0.8B GGUF; assemble a prompt of `n_ctx + 500` tokens; assert that `LlmFinished` is emitted and the truncated `assistant_response` is non-empty. |
| 5.2 | Bench disabled retrieval                           | Set the flag but never assert it was honored by the production code path. Pass. | After `setup_e2e_pipeline`, assert `state.conversation_manager.lock().message_count() == 0` and assert `state.settings.read().memory.context_retrieval_enabled == false`.                          |
| 5.3 | Bench reports prefill outcome                      | Hardcode `prefill.outcome = "success"` in the report. Pass.                     | Read the actual `VoxEvent` log from the run and populate the field from the event stream.                                                                                                          |
| 5.4 | User-visible warning                               | Send the toast in a test mock and assert the mock received it. Pass.            | Capture the real `IpcEvent::Notification` from the production router and assert the category is `prompt_truncated` and the message matches the spec.                                               |
| 5.5 | `Ready` (not `Error`) after unrecoverable overflow | Assert `state.pipeline.state() == Error` to make the test pass. Pass.           | Assert the state is `Ready`; assert the `Error` event is emitted with the exact actionable message from §5.1.5.                                                                                    |

---

## 8. Open questions (must be answered before implementation)

1. **What is the right reserved-output budget for the embedded LLM?** Current `RESERVED_GENERATION_TOKENS = 512` (`services/memory/mod.rs:29`). With `max_output_tokens = 300` default, 512 reserves more than 300 generates. Should this be `max(max_output_tokens, RESERVED_GENERATION_TOKENS)` to avoid over-reserving?
2. **What is the right safety margin?** llama.cpp's KV cache bookkeeping is exact in tokens; an explicit 8-token safety margin was assumed in some prior benches. Confirm or remove.
3. **Should truncation be FIFO of whole messages, or token-level (keep last N tokens of the oldest kept message)?** Whole-message is simpler and matches the existing FIFO shift in `TokenAccountant`; token-level is gentler. Recommend whole-message.
4. **Should the `prompt_truncated` notification auto-dismiss after a few seconds, or persist until the user dismisses?** Persist (matches the existing `Notifications` drawer pattern).
5. **Should the bench's `clear()` of the conversation manager also reset the persona / system prompt, or only the identity facts?** Only identity facts — the persona prompt is part of `settings` and must remain.

---

## 9. Out of scope (separate tickets)

- **Ticket A (open):** Raise `DEFAULT_LLM_CONTEXT_WINDOW` from 2048 to 4096 (or make the default model-dependent). The current default forces every user to truncate their identity facts after ~50 entries.
- **Ticket B (open):** Smarter persona / system-prompt compression (extract summary, drop low-priority identity facts at ingest when the budget is tight).
- **Ticket C (open):** Re-introduce the user-facing setting `compact_before_generate` (was removed in a prior phase) so the user can opt into pre-turn compaction when utilization is high.
- **Ticket D (open):** Background async memory ingestion gate (already designed in §3 of `integration_test_spec.md` but not yet implemented for the bench) so the bench is not affected by background compaction running during a measurement.

---

## 10. Acceptance criteria

- All items in §5 have passing production-seam tests (Seam 18 in `integration_test_spec.md`).
- `cargo bench --bench pipeline_bench -- --mode modular_passive --stt nemotron --llm qwen --tts kokoro` runs to completion in under 30 s and reports `prefill.outcome = "truncated"` with non-empty `assistant_response` and non-empty synthesized audio.
- `cargo nextest run --release --test-threads=1` is fully green (no regressions in Seams 1–11, 15–17).
- `cargo clippy --all-targets -- -D warnings` is clean.
- The user-facing `prompt_truncated` notification appears in the `Notifications` drawer with the exact message from §5.1.4.

---

## 11. Glossary

- **KV cache slot** — a single `(token, position)` entry in llama.cpp's pre-allocated key/value cache. Slots are reserved at `n_ctx` allocation time; decoding more than `n_ctx` tokens fails with `NoKvCacheSlot`.
- **Reserved output budget** — the number of KV-cache slots held back to allow the LLM to generate the next `max_output_tokens` without evicting the prompt.
- **Usable budget** — `context_window − reserved_output_tokens`; the maximum number of prompt tokens the LLM can accept.
- **Chunked prefill** — the loop in `prefill_or_reuse_kv_cache` that decodes the prompt in `n_batch`-sized chunks. Chunking does **not** reduce the number of KV-cache slots used; it only amortizes the per-decode overhead.
- **Identity fact** — a structured memory fact auto-extracted from prior turns; preloaded into the system prompt by the persona manager. The bench inherits whatever the production DB has, currently 207.
