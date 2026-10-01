# Dead Code, Redundancy & Tech Debt Audit

**Date:** 2026-10-01
**Scope:** `app/src-tauri/src` (Rust backend, ~49k LOC) and `app/src` (React frontend, ~232 files)
**Method:** Started from `sandbox/scripts/audit_dead_code_v2.py`, then hand-verified every single claim with a whole-word repo-wide grep. ~60% of that script's raw output was false positives (details in §5).

---

## 0. Read this first — how to use this report

Every item below is confirmed. "Hits" means how many times the name appears in the whole repo including tests, docs, and evals. **1 hit = only the definition exists = nobody calls it.**

Order of work is in §6. Sections 1–2 are safe mechanical deletions. Section 3 contains **2 real bugs** you should fix before touching any cleanup. Section 4 is duplication worth merging.

---

## 1. Backend — confirmed dead code (safe to delete)

Nothing here is called from `src/`, `tests/`, `benches/`, or `evals/`. Total ≈ **203 lines**.

### 1a. Whole functions with zero callers

| Location | Name | Lines | Note |
|---|---|---|---|
| `persistence/schema.rs:300` | `rebuild_fixture_db` | ~64 | Biggest single dead item. Fixture-DB builder, superseded. |
| `memory/ingestion/mod.rs:117` | `run_ingestion_cycle_with_embedder` | ~26 | Test-only embedder injection. No test uses it — `memory_ingestion_test.rs` calls `run_ingestion_cycle`. |
| `realtime/audio_bridge.rs:97` | `send_pcm` | ~21 | Sibling `get_sender()` on the same struct is the live path. |
| `persistence/notifications.rs:271` | `dismiss_notification` | ~13 | Singular version. `dismiss_notifications` (plural) is the live one. |
| `harness/chassis.rs:265` | `create_generation_request` | ~12 | |
| `llm/catalog/discovery.rs:30` | `preset_id` | ~8 | |
| `persistence/schema.rs:290` | `recreate_schema` | ~9 | |
| `config/mod.rs:59` | `is_explicitly_classified` | ~4 | Thin wrapper over `classify_known(...).is_some()`. |
| `llm/transport/config.rs:76` | `with_provider_kind` | ~4 | Builder that nobody calls. |
| `utils/paths.rs:318,326,365` | `config_dir`, `diagnostics_dir`, `icon_file` | ~10 | Path accessors. The `VoxPaths` struct fields they read *are* used — just not through these three wrappers. |
| `persistence/notifications.rs:344` | `mark_all_notifications_read` | ~4 | 1-line pass-through to `mark_notifications_read(conn, None)`. Already superseded per `phase11/notification_system_plan.md:165`. |
| `pipeline/atomics.rs:135` | `subscribe_dictation_state` | ~4 | Its backing field `dictation_state_rx` is also never read (§2). Dead API + dead field. |
| `monitoring/snapshots.rs:133` | `get_history` | ~5 | |
| `harness/stages/budget.rs:31` | `set_max_context_tokens` | ~3 | ⚠️ **This one was promised in a spec.** `docs/plans/phase10/wiring_memory_pipeline_refactor_spec.md:134-136` says "Retain & Wire — hook into `ipc/settings/mutation.rs` on `llm.context_window` changes." Never done. |
| `harness/chassis.rs:255` | `commit_turn` | ~3 | |
| `harness/chassis.rs:350` | `set_tool_registry` | ~3 | |
| `harness/chassis.rs:358` | `set_title_set` | ~3 | |
| `harness/chassis.rs:297` | `apply_quiet_session_context` | ~3 | Byte-identical to `apply_quiet_compaction_summary` two lines above it. |
| `vad/providers/mod.rs:82` | `is_onnx` | ~4 | ⚠️ Its logic was deliberately removed — `phase12/recent_work.md:149` records "replaced flawed `vad.is_onnx() (1, 1)` threshold collapse". The method was orphaned, not deleted. |

**What to do:** delete all of the above. Exception: decide on `set_max_context_tokens` first — either wire it or remove it and the spec line, don't leave it half-promised.

### 1b. One Tauri command with no frontend caller

`ipc/pipeline.rs:88` — `restart_engine` is registered in `lib.rs:654` and spec'd as public API in `docs/specs/ipc-spec.md:206-211`, but **no `invoke("restart_engine")` exists anywhere in the frontend.** The real restart path is `restart_engine_inner` via `config/dispatch.rs:78`. So only the IPC wrapper is dead.

**What to do:** delete the command, its registration in `lib.rs`, and the `ipc-spec.md` entry — or keep it and add a frontend caller. Pick one; right now it's neither.

### 1c. Frontend — dead exports and files

`cargo clippy` reports **zero** dead code in Rust (the compiler is clean because everything dead is `pub`). Knip flagged 175 items on the frontend; **81 were false positives**. The real ones:

**Two whole files, zero importers:**
- `app/src/shared/hooks/useConversationList.ts` — 66 lines. A complete conversation-list hook. Home's session list does something else.
- `app/src/shared/components/help/index.ts` — 5-line barrel. Every consumer imports the concrete `HomeHelpContent` etc. directly.

**One whole component, zero render sites:**
- `app/src/shared/ui/Badge.tsx` — 64 lines, 7 variants × 3 sizes + icon support. Exported from `src/shared/ui/index.ts:3`, **never rendered.** Meanwhile 8+ places hand-write the same badge `<span>`. Delete it, or adopt it — right now it's neither.

**Dead functions/consts (each exactly 1 hit):**

| Location | Name | ~Lines |
|---|---|---|
| `shared/lib/diff.ts:25` | `diffWords` + `tokenize` + `DiffToken` | ~85 |
| `shared/components/history/orbitMath.ts:133` | `ellipseAngleFromFraction` | ~25 |
| `services/settingsService.ts:150` | `validateLlmTokenCap` | ~14 |
| `shared/components/history/orbitMath.ts:312` | `formatDuration` | ~10 |
| `shared/components/history/orbitMath.ts:353` | `formatDayShortLabel` | ~7 |
| `data/providersCopy.tsx:130` | `CLOUD_PROVIDER_HOSTS` | ~7 |
| `shared/lib/spatialNavigation.ts:7` | `SPATIAL_CONTAINERS` | ~6 |
| `shared/components/history/orbitMath.ts:39-46` | `ORBIT_Z_BACK_MAX`, `ORBIT_Z_CLOCK`, `ORBIT_Z_FRONT_MIN`, `ORBIT_GUIDE_OPACITY` | 4 consts |
| `services/eventsService.ts:294` | `onPersonalMemoryUpdated` | ~5 |
| `services/setupService.ts:93` | `cancelModelSetup` | ~3 |
| `shared/lib/interactionMode.ts:26` | `normalizeToInteractionModeLower` | ~5 |
| `shared/components/history/orbitMath.ts:280,290,296,301,306` | `daysInMonthKey`, `timeToDialAngle`, `dayToDialAngle`, `dialDegrees`, `dialDotRadius` | ~17 |
| `shared/lib/overlayStack.ts:76` | `getStackIds` | ~3 |
| `services/windowService.ts:3` | `showMainWindow` | ~3 |
| `data/shortcuts.ts:96,102` | `getShortcutsForRoute`, `shortcutSuffix` | ~8 |
| `services/settingsService.ts:95` | `checkSttProviderHealth` | ~5 |
| `shared/hooks/useRuntimeSnapshot.ts:84` | `getLatestSnapshot` | ~3 |
| `shared/components/memory/memoryGraphTypes.ts:85` | `getThemeCollectionColors` | ~3 |
| `data/providersCopy.tsx:68,104` | `OpenAiLogo`, `ElevenLabsLogo` | ~8 |

**Backward-compat aliases that violate the repo's own ZBC rule** (`AGENTS.md §4.1` — "Never introduce compatibility layers, legacy paths"). These are explicitly labelled as such in comments, and all are unused:

- `services/memoryService.ts:209,211,213` — `getActiveFacts`, `getMemorySuggestions`, `resolveMemorySuggestions` (old "facts/suggestions" names aliased to the new "observations/revisions" names)
- `data/settingsCopy.ts:418,642` — `MEMORY_CONFIG_DESK_COPY`, `HISTORY_SETTINGS_COPY`, each commented `// Direct alias for backward-compatibility during component refactor`
- `shared/components/memory/LearnedFactsList.tsx:178` — `ObservationsList = LearnedFactsList`
- `services/pipelineService.ts:252-266` — a whole `// ── Re-exports for Backward Compatibility ──` block re-exporting `getRuntimeSnapshot` and 7 voice functions from their real homes. This one is actively harmful: `ModelsCard.tsx` imports `listVoices` from `voiceService` while `VoiceCarousel.tsx` imports it through `pipelineService`. Same symbol, two import paths.
- `services/settingsService.ts:175` vs `services/setupService.ts:98` — `completeSetupWizard` is declared **twice**, both `invoke("complete_setup_wizard")`. Only one is imported.

**~64 exported-but-file-local-only TypeScript types.** These aren't bugs — knip flags every `export interface` that its own file is the only consumer of. The one worth acting on is the `VoxSettings` family in `src/store/settingsStore.ts` (34 types): they're exported but nothing outside that file imports them, so the store's whole public type surface is invisible to consumers. Either export them from a `types.ts` or stop exporting. Don't churn the other 30.

---

## 2. Backend — state that is written but never read

These are worse than dead functions, because they look alive. Something computes a value, stores it, and nothing ever looks at it.

### 2a. Two filters that silently do nothing — HIGH

**`harness/stages/tools/registry.rs:15` — `ToolFilter.is_first_turn`.**
It is written at both construction sites (`chassis.rs:164` hardcodes `true`; `steps.rs:299` computes it from message count). But `active_definitions()` (`registry.rs:56-84`) only checks `mode`, `title_is_unset`, `memory_retrieval_enabled`, `web_search_enabled`. **`is_first_turn` is never read.** `steps.rs:306-310` even logs it in a diagnostic string, which makes it look alive.

**`harness/steps.rs:98` — `NonTerminalTrigger` enum.**
Both variants (`Compaction`, `NonTerminalTool { tool_name, call_id }`) are constructed at `steps.rs:113` and `:124`. **Zero `match` arms exist anywhere.** `NonTerminalPhase.trigger` is never read either — `enter_non_terminal_phase` only reads `phase.filler_phrase`.

**What this means:** the harness decides "is this the first turn?" and "is this a compaction or a tool turn?" and then throws both answers away. Any tool that should be first-turn-gated or phase-gated currently isn't, and nothing errors. Either implement the gates or delete the fields.

### 2b. Scaffolds with placeholder values

| Location | Field | Current state |
|---|---|---|
| `monitoring/snapshots.rs:83,85,87` | `main_webview_ram_mb`, `tray_webview_ram_mb`, `wizard_webview_ram_mb` | Hardcoded `None` at the only construction site (`:357-359`). `#[serde(skip_serializing_if)]` hides them from the wire. **The doc comment at `:81` claiming "Measured via sysinfo descendant enumeration" is false.** The frontend's working webview RAM comes from the separate `ProfilerSnapshot`, which *is* populated. |
| `harness/mod.rs:177,178` | `ConversationContext.token_count`, `.kv_cache_index` | Both hardcoded `0` at the only construction site (`llm/embedded/mod.rs:104-105`). Never read. The struct promises token accounting that doesn't exist. |
| `harness/stages/compaction.rs:42,43` | `CompactionStage.context_window`, `.is_embedded` | Assigned in `new()`, and their accessors (`:69`, `:73`) have **zero callers**. `is_embedded` is computed at `chassis.rs:62` purely to be discarded. |
| `memory/mod.rs:10,11` | `MemoryAppState.graph_version`, `.user_paused_ingestion` | Constructed at `:24,25`, never read. `user_paused_ingestion` is an `Arc<AtomicBool>` — a user-pause-ingestion feature that was scaffolded and never built. |
| `vad/actor.rs:96-99` | `pcm_scratch`, `partial_recycle_tx`, `partial_recycle_rx`, `realtime_recycle_tx` | A whole PCM buffer-recycling subsystem: channels created at `:164-165`, scratch buffer allocated at `:185`, never read. |
| `vad/actor.rs:74,75` | `VadValidationResult.speech_start_sample`, `.speech_end_sample` | Set at `:308-309`, never read. Both consumers (`pipeline/dictation/ptt.rs:200`) destructure only `(is_speech_detected, audio)`. |
| `harness/stages/tools/executor.rs:19,21,24` | `ToolExecutionOutcome.call_id`, `.tool_flow`, `.duration_ms` | Constructed at all 3 sites. Consumers read only `.result`, `.tool_name`, `.is_error`. |
| `monitoring/resource_scope.rs:62,92,97` | `ScopedProcess.is_runtime`, `LinuxCgroupSnapshot.shmem_thp_bytes`, `.root_pid` | Set, never read. |
| `audio/decode.rs:36` | `DecodedAudio.duration_secs` | Set at `:109`, `:249`, never read. |

**What to do:** these are judgment calls, not blind deletions. `ConversationContext` mirrors a llama.cpp context handle and `webview_ram_mb` has a doc comment describing intended behaviour. Either implement them or delete field *and* comment together — leaving a comment that asserts a measurement that never happens is the actual debt.

---

## 3. Two real bugs found during the audit

### 3a. `Working` state is dropped by the frontend — HIGH

The backend has 9 interaction states including `Working` (`core/state.rs:102`), and `router.rs:94` emits it as the string `"Working"`.

Two copies of the allow-list exist:
- `services/pipelineService.ts:32` — **has `"Working"`** ✅
- `shared/hooks/useSessionEvents.ts:16` — **missing `"Working"`** ❌

`useSessionEvents.ts:45-48` rejects anything not in its list, logs `Invalid state received`, and returns. So **every `Working` transition is discarded before it reaches `sessionStore`.**

`Working` is entered on every non-terminal turn via `harness/steps.rs:151` — that's every turn where a tool runs long enough to need filler audio. Confirmed reachable: `enter_non_terminal_phase` is called from `steps.rs:213` and `:527`.

**What to do:** delete the duplicate. Export one `VALID_STATES` set from a single module and import it in both places. This is the same class of bug that will recur — see §4.

### 3b. 45 `animate-fade-in` class strings ship and do nothing — HIGH

`animate-fade-in` appears **39 times across 17 files**. `animate-in`, `fade-in`, `zoom-in-95`, `slide-in-from-top-2` add 6 more. Together: 45 uses.

None of them are defined. Verified three ways:
- No definition in `src/index.css` or `tailwind.config.js`.
- `tw-animate-css` is in `package.json` dependencies, but `rg -n "tw-animate" src/` → **0 hits.** The stylesheet is never imported. `src/index.css` has exactly 3 directives: a Google Fonts `@import`, `@import "tailwindcss"`, `@config`.
- The production build confirms it: `rg -o "animate-fade-in" dist/assets/index-DXEOAUPM.css` → **0**.

So 45 class strings are in the bundle doing nothing. Every settings-desk fade-in transition is silently absent.

**What to do:** add `@import "tw-animate-css";` to `src/index.css`, or strip the class strings. Decide which — right now the dependency is paid for and unused.

---

## 4. Duplication worth merging

Ordered by size of the win.

### 4a. `ten_onnx.rs` and `silero_onnx.rs` are ~90% the same file — HIGH

`vad/providers/ten_onnx.rs` and `silero_onnx.rs`, 163 lines each. `diff` shows **14 changed lines out of 163.** Identical: `new()` (31 statements), `update_params()` (24 statements), `flush()`, `set_threshold()`, and the whole `VadEngineTrait` impl. They differ only in struct name, config type, `window_size: 256` vs `512`, and 3 log strings.

Both are live (`VadBackend::Silero | VadBackend::Ten`).

**Fix:** one generic `OnnxVadEngine<C: VadModelConfig>` with the config type as a parameter. ~130 lines collapse to ~90.

### 4b. `mark_notifications_read` and `dismiss_notifications` are the same 55 lines twice — HIGH

`persistence/notifications.rs:148` and `:204`. `diff` shows 6 differing lines, all SQL string literals (`'read'`/`'unread'` vs `'dismissed'`).

What's duplicated: the whole 4-branch filter cascade, the empty-`ids` early return, and the SQL-injection-safe quote escaping.

**Fix:** one `update_status(conn, new_status, guard, filter)` helper. The security-sensitive `ids` escaping is currently copy-pasted — that's the risk, not the line count.

### 4c. `ToolFilter` / retryability / timestamp helpers

- **`persistence/mod.rs:185` vs `worker.rs:168`** — two retryability classifiers for one concept. `VoxDb::is_retryable` matches typed variants `Busy | BusySnapshot`; `is_retryable_event_err` re-implements it as a **string match** on `"conflict" | "Busy" | "busy"`. The comment at `worker.rs:167` admits it "Mirrors `VoxDb::is_retryable`". The string version is looser. Note `worker.rs` uses *both* (`:127` string, `:274`/`:309` typed) — so retry behaviour differs by call site today.
- **5 copies of "milliseconds since epoch."** `notifications.rs:391` and `services/notifications/service.rs:194` are byte-identical `current_timestamp_ms() -> i64`. `harness/mod.rs:181` and `core/metrics.rs:37` are byte-identical `now_ms() -> u64`. `notifications/lifecycle.rs:203` is a third variant. Meanwhile **29 inline `SystemTime::now()` calls in `persistence/` alone** bypass all of them.
- **5 identical `set_speed` bodies** in `tts/providers/` (`kokoro.rs:135`, `chatterbox.rs:163`, `chatterbox_remote.rs:235`, `edge_tts.rs:427`, `supertonic.rs:183`, plus `supertonic.rs:183`). Same clamp-then-`store`. A default trait method collapses all of them.
- **4 TTS `health_check` stubs** returning `true` (`kokoro.rs`, `chatterbox.rs`, `supertonic.rs`, `zipvoice.rs`). The TTS `health_check` is called from exactly **one** site (`chatterbox_remote.rs:66`), so even `edge_tts.rs:433`'s real TCP probe never runs.
- **4 duplicate SINC resampler constants.** `realtime/mod.rs:28-32` declares all 5 `SINC_*` constants; `audio/mod.rs:33-37` declares the same 5. Only `SINC_CHUNK_SIZE_INPUT` is imported from the `realtime` copy (at `audio_bridge.rs:12`). The other **4 are dead duplicates** — delete them from `realtime/mod.rs`.
- **Context window stored 3× in one struct.** `chassis.rs:66,71,73` pass `settings.llm.context_window` into `PromptBuilderStage`, `ContextBudgetStage`, and `CompactionStage`. Two of those three also duplicate the same job (context budgeting + compaction-readiness thresholds).
- **Gemini vs Deepgram realtime sessions.** `realtime/providers/{gemini,deepgram}/session.rs`: `send_audio`, `disconnect`, `send_tool_response`, `send_text` are structurally identical, differing only in the provider name inside a `bail!()` and a log tag. `diff` of `send_audio` = 2 changed lines. ~55 duplicated lines.

### 4d. Frontend duplication

- **13 copies of the same numeric-input parse block.** `const clean = e.target.value.replace(/[^0-9]/g, ""); if (isNaN(...)) return;` + range guard, across `LlmSettingsView.tsx:178,215,269`, `WorkingMemoryConfigDesk.tsx:37`, `TtsVoiceManager.tsx:339`, `VadWorkspace.tsx:126,165,204,244`, `AsrWorkspace.tsx:126,202`, `PersonalMemoryConfigDesk.tsx:94,106`. A `PresetInput` already exists in `SettingsTabPane.tsx` — use it.
- **4 copies of the "missing cloud API key" banner logic.** `SettingsCardWrapper.tsx:24-33` and `Settings.tsx:83-90`, **including the same `const isCloudSttMissingKey = false;` suppression and the same TODO comment verbatim.** Both also render the same two footers (`restartingEngine`, `autoSynced`).
- **3 copies of the CPU-profile panel.** `AsrWorkspace.tsx:156`, `LlmSettingsView.tsx:37`, `TtsVoiceManager.tsx:293` each compute `navigator.hardwareConcurrency || 4` then a 4-way profile ternary. The label names have already drifted (`"eco"` vs `"power"` vs `"balanced"`).
- **2 copies of arrow-key tab navigation.** `SegmentedControl.tsx:36-58` and `ViewSelector.tsx:27-42` are the same `querySelectorAll('button…')` + `findIndex(b => b === document.activeElement)` body. `CategorySelector.tsx:31-45` is a third; `ProfilerPanel.tsx:49` and `NotificationPanel.tsx:452` are 4th and 5th copies of the idiom.
- **2 copies of `getErrorMessage(e: unknown)`.** `useHistory.ts:38` and `useConversationList.ts:10`. They aren't even the same — `useHistory` handles the `{message: string}` case, the other doesn't. Plus **11 files** open-code `err instanceof Error ? err.message : String(err)` inline.
- **2 `fuzzyMatch` with different semantics.** `shared/lib/fuzzy.ts:15` returns a scored `FuzzyMatchResult` and is used by the LLM catalog. `HistoryListView.tsx:30` open-codes a bare subsequence scan returning `boolean`. Same search job, two engines, guaranteed to diverge.
- **2 `VoiceBars`.** `shared/ui/VoiceCarousel.tsx:17` (exported, 1 caller) and `realtime/RealtimeVisualElements.tsx:390` (not exported, 1 caller). Same seed-hash → bar heights → `dynamic-eq` keyframe. The second should import the first.
- **2 `YYYY-MM-DD` builders.** `orbitMath.ts:332` (`toDayKey`, 1 caller) and `CalendarPicker.tsx:14` (`toDateKey`, 4 callers). They produce strings that get compared against each other in `HistoryListView.tsx:157`.
- **4 `HH:MM` formatters.** `orbitMath.ts:377` is the exported one; `DetailPanel.tsx:13` and `NotificationPanel.tsx:88` are byte-identical locals; `useHistory.ts:444` is a 4th.
- **2 context menus.** `ui/SessionContextMenu.tsx:59-146` and `ui/ProjectContextMenu.tsx:36-139` — same ~60 lines of outside-click + arrow-key + viewport-flip positioning. Same `anchorRect.right + 6` / `top - 4` math, same two flip/clamps.
- **2 topology tab bars.** `ModelsTopologyMap.tsx:42-51` and `SettingsTopologyMap.tsx:26-37` — same arrow-key wrap-around over an id array, same container class string, same active-button ternary.
- **1 dead component, 8 hand-rolled copies.** `ui/Badge.tsx` is never rendered (§1c), while `LlmConfigDesk.tsx` repeats the identical `px-2 py-0.5 rounded-full bg-[rgb(var(--accent))]/10 …` span 4 times.
- **3 visibility-gated pollers.** `useRuntimeSnapshot.ts:13-53`, `useMonitoringMetrics.ts:21-77`, `useMemoryProfiler.ts:61-171` each implement interval polling with an `inFlightRef` mutex and their own visibility bookkeeping.

### 4e. Dead CSS — verified against the production build

**`src/index.css` defines these classes with zero references anywhere in `src/`** (checked with literal grep, then confirmed each *does* ship in `dist/assets/index-DXEOAUPM.css` — i.e. they're in the bundle doing nothing):

`shimmer-text` · `field-text` · `signal-text` · `ambient-label` · `field-intensity-low/med/high` · `rp-ring-in` · `shimmer-badge` · `custom-color-picker-v2` · `blob-rotate-c` · `animate-wave-flow` · `animate-ribbon-flow` · `animate-particle-drift` · `animate-ring-pulse-slow` · `orbit-card-enter`

Orphan `@keyframes` reachable only from those: `wave-flow`, `ripple-in`, `skeleton-glow`, `ring-pulse-slow`, `shimmer-slide`.

**The entire `theme.extend` block in `tailwind.config.js:11-85` is unreferenced** — ~50 Material-3 color tokens (`bg-surface`, `text-primary`, `bg-on-surface`, `bg-surface-container`, `bg-secondary`, `bg-tertiary`, …), 3 `backgroundImage` gradients (`orb-gradient`, `glass-gradient`, `bg-noise`), 7 animation entries (`animate-float`, `animate-blob`, …), and 5 spacing values (`p-container-padding`, `gap-element-gap`, …). All zero hits. The one live entry is `animate-pulse-slow` — and that's defined **three times**: `tailwind.config.js:88` (4s), `index.css:777` (3s), and `StatusCapsule.tsx:22` hardcodes `animation: "pulse-slow 2.5s …"` inline.

**Two more issues in `index.css`:**
- The global `::-webkit-scrollbar` block is declared **twice** (`:323-343` and `:787-795`). The second wins by cascade order; the first block's `-track`/`-thumb:hover` rules are silently dead. `.custom-scrollbar` at `:344` is a third variant.
- **`tailwind.config.js:9` scans `./src-tray/**/*`, which does not exist.** The tray lives at `src/tray/`. It only works today because those files are also matched by the `./src/**` glob on line 8. Delete the dead path.

---

## 5. What the old script got wrong (don't trust it blind)

`sandbox/scripts/audit_dead_code_v2.py` reported 100 "true dead Rust functions." **89 were `#[cfg(test)]` unit-test functions** that the `cargo test` harness calls — the script's regex counts textual occurrences and cannot see the test runner. The 11 real ones are all in §1a.

Its `webview_created` finding is also wrong: that's a Tauri `Plugin` trait method (`window_customizer.rs:13`), called by the framework, not by us. Clippy correctly reports **0** dead-code warnings for the whole backend — every dead item is `pub`, so the compiler stays silent. Clippy alone is not a sufficient gate here.

Knip flagged 175 frontend items. **81 were false positives** — mostly `export function on` in `eventsService.ts` (234 uses; knip matched the bare word `on`) and the 64 exported-but-file-local types. The script's report also mis-bucketed the types as needing action when they're mostly harmless.

---

## 6. Recommended order of work

**Phase 1 — bugs (do first, they're cheap):**
1. Fix the `Working` state drop: `useSessionEvents.ts:16`. Delete one of the two `VALID_STATES` copies. (§3a)
2. Decide on `animate-fade-in`: import `tw-animate-css` or delete 45 class strings. (§3b)

**Phase 2 — pure deletions (no behaviour change, ~350 lines):**
3. Delete the 22 dead backend functions in §1a. Resolve `set_max_context_tokens` and `is_onnx` first — both have spec/published history.
4. Delete `restart_engine` IPC wrapper + `lib.rs` registration + `ipc-spec.md` entry, or add a caller. (§1b)
5. Delete the 2 dead frontend files + `Badge.tsx`. (§1c)
6. Delete the ~30 dead frontend exports and the 4 backward-compat alias blocks. (§1c) — this is a direct ZBC cleanup.
7. Delete the 14 dead CSS classes, the 5 orphan keyframes, the dead `theme.extend` block, the duplicate scrollbar block, and the `./src-tray/` scan path. (§4e)

**Phase 3 — decide intent (needs a human call, don't blind-delete):**
8. The write-only fields in §2. For each: implement it or delete it *and* its misleading doc comment.
9. `ToolFilter.is_first_turn` and `NonTerminalTrigger` — these two are silently disabling gates you probably think are active. (§2a)

**Phase 4 — merge duplication (mechanical, low risk):**
10. `ten_onnx.rs` / `silero_onnx.rs` → one generic. (~130 lines)
11. `mark_notifications_read` / `dismiss_notifications` → one helper — **do this one for the security-sensitive `ids` escaping, not the line count.** (~55 lines)
12. The 4 `SINC_*` dead duplicates in `realtime/mod.rs`. (4 lines, trivial)
13. The duplicate `is_retryable` — and while you're there, note `worker.rs` currently uses the loose string version at `:127` and the typed version at `:274`/`:309`.
14. The 5 timestamp helpers, the 5 `set_speed` bodies, the 3 `ContextBudgetStage`/`CompactionStage` context-window copies.
15. Frontend: the 4 duplicated cloud-key banners, the 13 numeric-input blocks, the 5 arrow-key tab handlers, the 2 `fuzzyMatch`, the 2 `VoiceBars`, the 2 context menus.

---

## Appendix: things that look like debt but are fine

Recorded so nobody re-investigates them:

- **Zero `TODO`/`FIXME`/`HACK` in Rust** (0 matches across 254 files). The 2 in the frontend are the same deferred-STT-desk note duplicated in two files, and its `:364` anchor has drifted.
- **Zero `as any`, zero `@ts-ignore`/`@ts-expect-error`, 1 `eslint-disable`** in the whole frontend — and `src/test/invariants.test.ts:330` actively enforces the `any` ban.
- **All 64 backend `#[tauri::command]` functions are registered, and all 66 frontend `invoke()` strings map to a real command.** Zero orphan IPC in either direction. The IPC surface is clean except `restart_engine` (§1b).
- **The IPC DTOs match.** 9 of 10 checked Rust↔TS struct pairs are field-identical. The 10th is `RuntimeSnapshot`, differing only by the 3 permanently-`None` fields from §2b.
- `HarnessInit`'s 5 "unused" fields in `realtime/transport.rs:66` are read by destructuring, not dot access.
- `_log_guard` in `core/state.rs:127` is an RAII `tracing` keepalive; the `_` prefix is idiomatic.
- `webview_created` is a Tauri plugin trait method — framework-called, not dead.