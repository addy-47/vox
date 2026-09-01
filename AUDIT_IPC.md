# Vox IPC Full Audit — Frontend ↔ Backend

> **Scope:** Every cross-boundary contract between Rust backend (`app/src-tauri/src`) and TypeScript frontend (`app/src`). Covers `IpcEvent` (backend→frontend), `VoxEvent` (internal), and `invoke` commands (frontend→backend).  
> **Date:** 2026-09-01 — **Auditor:** opencode (read-only sweep, 3 parallel explorers)  
> **Baseline:** `lib.rs:494` `generate_handler![86]` + `events.rs:128` `IpcEvent` 11 variants  
> **Verdict:** Backend→frontend **audited** (11/11 mirrored). Frontend→backend **missed** — 86 registered vs 75 consumed, 12 orphans/dead, 2 service-boundary violations, 6 merge clusters. Recommended target **48–52** handlers (−38%).

---

## 1. Executive Summary

| Direction | Registry | Consumed | Dead / Orphan | Mirrored | Status |
|---|---:|---:|---:|---:|---|
| Backend→Frontend `IpcEvent` `core/events.rs:128` | 11 | 11 | 0 | 11/11 `services/eventsService.ts:105` | ✅ Audited `recent_work.md:125` §22 |
| Internal `VoxEvent` `core/events.rs:11` | 12 | 12 (router) | 0 | — | ✅ Audited |
| Frontend→Backend `invoke` `lib.rs:494` | 86 (+1 unregistered) | 75 distinct / 76 sites | 12 | — | ❌ Missed — this report |

**Net recommendation:** Prune 11 strict-dead, merge 6 clusters (−14 handlers), fix 2 violations. Conservative **59**, aggressive **48–52**. No runtime hot-path change; all cuts are cold config or superset folds.

---

## 2. Methodology

1. Grep `#[tauri::command]` across `app/src-tauri/src` → 87 definitions.
2. Grep `tauri::generate_handler!` in `lib.rs:494–587` → 86 registered (delta = `rename_voice` `ipc/voices.rs:235` defined, never registered).
3. Grep `invoke\(["']` across `app/src` → 76 sites, 75 distinct strings ( `complete_setup_wizard` duplicated in 2 service files). Classified by `services/*.ts` vs stray.
4. For each distinct invoke string: counted production callers outside `services/__tests__` / `__tests__` mocks; flagged `REACHABLE` (≥1 component/hook), `DEAD` (0 outside tests), `STRAY` (direct `invoke` outside `services/` violating `AGENTS.md:4.1#5`).
5. For each orphan command: read backend impl (`ipc/*.rs`) to confirm guard semantics, payload, and superset relationship.
6. No code was changed during this audit.

---

## 3. Backend Registry — 86 `invoke` Handlers

### 3.1 `lib.rs:494` handler map

```
Pipeline/Engine 10 | Tray/Window 6 | Toast 4 | Settings 13 | PTT 3
History 5 | Dictation 3 | Voices 7 | Monitoring 5 | Setup 12 | Audio 2 | Memory 16
= 86
```

### 3.2 Full table (87 definitions, 86 registered)

#### A. Pipeline / Engine — `ipc/pipeline/assistant.rs` + `ipc/pipeline/mod.rs:18` + `ipc/pipeline/test_clip.rs`

| # | String | File:Line | Signature | Async | Purpose |
|---|---|---|---|---|---|
| 1 | `check_engine_status` | `ipc/pipeline/assistant.rs:11` | `state: State<Arc<AppState>> -> bool` | async | Is audio engine active |
| 2 | `launch_engine` | `ipc/pipeline/assistant.rs:18` | `app: AppHandle` | async | Launch 3-tier engine |
| 3 | `stop_engine` | `ipc/pipeline/assistant.rs:25` | `app: AppHandle` | async | Shutdown + unload |
| 4 | `start_session` | `ipc/pipeline/assistant.rs:32` | `app, state` | async | Idle→Ready, create conv, route to domain |
| 5 | `end_session` | `ipc/pipeline/assistant.rs:120` | `app, state` | async | Domain `end_session`, persist `SessionEnded`, →Idle |
| 6 | `pause_session` | `ipc/pipeline/assistant.rs:226` | `app, state` | async | Pause (reject Idle) |
| 7 | `resume_session` | `ipc/pipeline/assistant.rs:248` | `app, state` | async | Resume from Paused |
| 8 | `test_clip` | `ipc/pipeline/test_clip.rs:105` | `app, state, clip_id: String` | async | Decode WAV→16k mono, inject `SttCommand::Final` |
| 9 | `test_clip_cancel` | `ipc/pipeline/test_clip.rs:153` | `state` | async | Cancel clip, renew turn token |
| 10 | `get_realtime_session_cache` | `ipc/pipeline/mod.rs:18` | `() -> RealtimeSessionCache` | async | Read `~/.cache/realtime_session.json` TTL |

#### B. PTT — `ipc/pipeline/assistant.rs:270`

| 34 | `ptt_start` | `assistant.rs:270` | `app, state` | async | Guard not Idle/Paused → domain `ptt_start` |
| 35 | `ptt_stop` | `assistant.rs:287` | `app, state` | async | Require Listening → finalize |
| 36 | `ptt_cancel` | `assistant.rs:305` | `app, state` | async | Cancel if Listening |

#### C. Tray / Window — `ipc/tray.rs`

| 11 | `hide_tray_window` | `tray.rs:102` | `app` | async | Hide tray + cancel dictation turn |
| 12 | `toggle_tray_visibility` | `tray.rs:11` | `app` | async | Toggle + menu checkmark guard |
| 13 | `sync_hud_visibility` | `tray.rs:122` | `app, visible: bool` | async | Sync HUD with owner/VAD |
| 14 | `set_hud_ignore_cursor` | `tray.rs:184` | `WebviewWindow, ignore: bool` | sync | Click-through (Linux GTK) |
| 15 | `update_interaction_mode` | `tray.rs:232` | `app, target, mode` | async | Mutate `interaction.mode`, persist, `VadCommand::UpdateMode` |
| 16 | `show_main_window` | `tray.rs:300` | `app` | async | Rebuild/focus main |

#### D. Toast — `toast.rs:162` (all `sync fn`)

| 17 | `show_toast_window` | `toast.rs:162` | `AppHandle<R>` | sync | Show after frontend mount |
| 18 | `hide_toast_window` | `toast.rs:176` | `AppHandle<R>` | sync | Hide without destroy |
| 19 | `destroy_toast_window_cmd` | `toast.rs:187` | `AppHandle<R>` | sync | Destroy webview to reclaim RAM |
| 20 | `get_last_toast` | `toast.rs:196` | `() -> Option<ToastPayload>` | sync | Poll for late-joining webviews |

#### E. Settings — `ipc/settings/catalog.rs:32` + `ipc/settings/health.rs:6` + `ipc/settings/mutation.rs:218`

| 21 | `request_boot_state` | `settings/catalog.rs:32` | `app -> BootState{settings, models_dir_exists, settings_path}` | async | Boot snapshot |
| 22 | `request_model_catalog` | `settings/catalog.rs:53` | `app -> ModelCatalog` | async | Manifest-derived catalog + voices/colors |
| 31 | `get_settings` | `settings/catalog.rs:121` | `state -> VoxSettings` | async | Settings clone |
| 23 | `check_llm_provider_health` | `settings/health.rs:6` | `state, provider?: LlmProviderConfig -> bool` | async | Embedded file or `/health` |
| 24 | `check_stt_provider_health` | `settings/health.rs:73` | `state, provider? -> bool` | async | Cloud/local check |
| 25 | `check_tts_provider_health` | `settings/health.rs:117` | `state, provider? -> bool` | async | Supertonic/Chatterbox/Edge check |
| 26 | `list_llm_models` | `settings/health.rs:181` | `state, provider? -> Vec<LlmModelInfo>` | async | Dir scan or remote list |
| 27 | `get_cached_capabilities` | `settings/health.rs:227` | `() -> HashMap<String,ModelCapabilities>` | async | Read disk cache |
| 28 | `probe_model_capabilities` | `settings/health.rs:242` | `state, provider?, model_id? -> ModelCapabilities` | async | Live probe + cache write |
| 29 | `validate_llm_token_cap` | `settings/health.rs:305` | `state, provider?, model_id?, target_cap: u32 -> Option<u32>` | async | Ceiling check |
| 30 | `setup_remote_server` | `settings/health.rs:580` | `app, connection_string, ssh_port?, key?, remote_path, server_port` | async | SSH-stream `setup_server.sh` |
| 32 | `update_setting` | `settings/mutation.rs:218` | `domain, key, value: Value, app -> SettingUpdateResult` | async | Generic hot-apply + side-effects |
| 33 | `reset_settings` | `settings/mutation.rs:273` | `app -> VoxSettings` | async | Reset + emit `SettingsUpdated` |

#### F. History — `ipc/history.rs:7`

| 37 | `get_transcript_history` | `history.rs:7` | `state -> Vec<String>` | async | Ephemeral tray buffer |
| 38 | `commit_session_to_history` | `history.rs:18` | `text, state` | async | Bounded 10k dedup push |
| 39 | `get_sessions` | `history.rs:70` | `() -> Vec<SessionRow>` | async | SQLite last 500 |
| 40 | `get_turns` | `history.rs:103` | `session_id: i64 -> Vec<TurnRow>` | async | Turns for session |
| 41 | `delete_session` | `history.rs:138` | `id: i64` | async | CASCADE delete |

#### G. Dictation — `ipc/pipeline/dictation.rs:8`

| 42 | `get_dictation_settings` | `dictation.rs:8` | `state -> DictationSettings` | async | Clone settings.dictation |
| 43 | `get_last_dictation_transcript` | `dictation.rs:17` | `state -> Option<String>` | async | Last transcript (recovery) |
| 44 | `copy_last_dictation_transcript` | `dictation.rs:26` | `state` | async | Clipboard copy |

#### H. Voices — `ipc/voices.rs:59`

| 45 | `list_voices` | `voices.rs:59` | `() -> Vec<VoiceEntryDto>` | async | DB newest-first |
| 46 | `fetch_edge_tts_voices` | `voices.rs:258` | `() -> Vec<EdgeTtsVoiceDto>` | async | Online Edge list |
| 47 | `add_voice_from_file` | `voices.rs:71` | `name, file_path -> VoiceEntryDto` | async | Validate+convert+prebake |
| 48 | `add_voice_from_recording` | `voices.rs:137` | `name, pcm_f32, sample_rate -> VoiceEntryDto` | async | PCM→WAV+bake |
| 49 | `start_backend_recording` | `voices.rs:246` | `()` | async | Backend mic start |
| 50 | `stop_backend_recording` | `voices.rs:252` | `() -> (Vec<f32>, u32)` | async | Stop → PCM+rate |
| 51 | `delete_voice` | `voices.rs:207` | `id` | async | DB+disk delete |
| — | `rename_voice` **UNREGISTERED** | `voices.rs:235` | `id, name` | async | Dead def, not in handler |

#### I. Monitoring — `ipc/monitoring.rs:8`

| 52 | `get_runtime_snapshot` | `monitoring.rs:8` | `state -> Option<RuntimeSnapshot>` | sync | Latest throttled snapshot |
| 53 | `get_runtime_history` | `monitoring.rs:14` | `state -> Vec<RuntimeSnapshot>` | sync | ~60s history |
| 54 | `clear_runtime_history` | `monitoring.rs:20` | `state` | sync | Clear buffer |
| 55 | `get_profiler_snapshot` | `monitoring.rs:26` | `app -> ProfilerSnapshot` | async | `spawn_blocking(collect)` |
| 56 | `record_memory_profile_event` | `monitoring.rs:41` | `event: MemoryProfileLogEvent` | async | Append JSONL |

#### J. Setup — `ipc/setup.rs:11`

| 57 | `fetch_manifest` | `setup.rs:29` | `state -> VoxManifest` | async | Remote or cached |
| 58 | `check_for_updates` | `setup.rs:11` | `() -> UpdateReport` | async | AppManifest diff |
| 59 | `check_for_model_updates` | `setup.rs:20` | `() -> ModelUpdateReport` | async | VoxManifest diff |
| 60 | `get_onboarding_status` | `setup.rs:154` | `state -> bool` | async | `setup_completed` |
| 61 | `get_runtime_report` | `setup.rs:47` | `state -> RuntimeReport` | async | Hardware+model verification |
| 62 | `start_model_setup` | `setup.rs:94` | `app, state, selected_ids?` | async | Download required/selected |
| 63 | `cancel_model_setup` | `setup.rs:147` | `state` | async | Cancel ongoing |
| 64 | `complete_setup_wizard` | `setup.rs:161` | `app, state` | async | Mark complete, close wizard |
| 65 | `reveal_wizard` | `setup.rs:380` | `app` | async | Focus wizard |
| 66 | `check_model_exists` | `setup.rs:300` | `model_id, state -> bool` | async | Presence + SHA marker |
| 67 | `download_optional_model` | `setup.rs:326` | `model_id, app, state` | async | Single group background |
| 68 | `delete_model` | `setup.rs:439` | `model_id, state` | async | Delete + verified markers |

#### K. Audio — `ipc/audio.rs:21`

| 69 | `list_input_devices` | `audio.rs:21` | `() -> Vec<AudioDevice>` | async | Filter virtual/monitor, 5s cache |
| 70 | `list_output_devices` | `audio.rs:66` | `() -> Vec<AudioDevice>` | async | Same, output |

#### L. Memory — `ipc/memory/graph.rs:66` + `mutations.rs:9` + `conflicts.rs:17` + `ingestion.rs:44`

| 71 | `get_graph_version` | `memory/graph.rs:66` | `state -> u64` | async | Monotonic version |
| 72 | `get_memory_graph_topology` | `memory/graph.rs:145` | `state, filter? -> MemoryGraphPayload` | async | Nodes+edges+version |
| 73 | `get_memory_fact_detail` | `memory/graph.rs:205` | `fact_id -> MemoryFactDetail` | async | Single fact + relations |
| 74 | `get_memory_stats` | `memory/graph.rs:272` | `() -> MemoryStats` | async | Counts (sessions/facts/queue) |
| 75 | `edit_fact_content` | `memory/mutations.rs:9` | `state, fact_id, new_content` | async | In-place UPDATE + re-embed |
| 76 | `reassign_fact_collection` | `memory/mutations.rs:87` | `state, fact_id, new_collection` | async | Stage reassigned |
| 77 | `soft_delete_fact` | `memory/mutations.rs:134` | `state, fact_id` | async | Tombstone + SUPERSEDES |
| 78 | `user_edit_memory` | `memory/mutations.rs:181` | `state, old_fact_id, new_fact, collection -> String` | async | Supersede via new fact |
| 79 | `user_delete_memory` | `memory/mutations.rs:221` | `state, fact_id` | async | Wrapper over soft_delete |
| 80 | `get_unresolved_conflicts` | `memory/conflicts.rs:17` | `() -> Vec<MemoryConflict>` | async | CONFLICTS not superseded |
| 81 | `resolve_memory_conflict` | `memory/conflicts.rs:75` | `state, winner_id, loser_id` | async | Supersede loser |
| 82 | `get_memory_relations` | `memory/ingestion.rs:44` | `() -> Vec<MemoryRelationEntry>` | async | All edges |
| 83 | `get_memory_queue_status` | `memory/ingestion.rs:75` | `() -> MemoryQueueSummary` | async | Counts + 50 items |
| 84 | `toggle_pipeline_processing` | `memory/ingestion.rs:144` | `state, paused? -> bool` | async | Pause/resume ingestion |
| 85 | `retry_failed_queue` | `memory/ingestion.rs:174` | `state -> u32` | async | Reset all failed |
| 86 | `retry_failed_queue_items` | `memory/ingestion.rs:200` | `state, item_ids: Vec<i64> -> u32` | async | Reset selected (max 1000) |

---

## 4. Frontend Consumption — 76 Sites, 75 Strings

**Import discipline:** `from "@tauri-apps/api/core"` appears in 9 files: 7 `services/*.ts` (compliant) + 2 violations.

| Service | File | Sites | Strings |
|---|---|---|---|
| pipeline | `services/pipelineService.ts:61` | 23 | `stop_engine`…`setup_remote_server` |
| settings | `services/settingsService.ts:33` | 14 | `request_boot_state`…`complete_setup_wizard` |
| model | `services/modelService.ts:69` | 12 | `fetch_manifest`…`get_onboarding_status` |
| history | `services/historyService.ts:32` | 5 | `get_transcript_history`…`delete_session` |
| window | `services/windowService.ts:4` | 4 | `show_main_window`…`set_hud_ignore_cursor` |
| memory | `services/memoryService.ts:69` | 11 | `get_graph_version`…`get_memory_queue_status` |
| profiler | `services/memoryProfilerService.ts:77` | 2 | `get_profiler_snapshot`, `record_memory_profile_event` |
| **stray** toast | `toast/ToastApp.tsx:34` | 4 | `hide_toast_window`, `destroy_toast_window_cmd`, `show_toast_window`, `get_last_toast` |
| **stray** LLM cap | `shared/components/settings/models/LlmSettingsView.tsx:99` | 1 | `validate_llm_token_cap` |

No frontend `emit` — `grep emit\(` across `app/src` = 0 functional hits.

### Reachability (outside tests)

6 wrappers are dead (`DEAD` = 0 production importers, only `services/__tests__`):

| Wrapper | File:Line | String | Evidence |
|---|---|---|---|
| `getRuntimeHistory` | `pipelineService.ts:108` | `get_runtime_history` | only `__tests__/pipelineService.test.ts:104` |
| `clearRuntimeHistory` | `pipelineService.ts:112` | `clear_runtime_history` | only `__tests__/pipelineService.test.ts:109` |
| `getRealtimeSessionCache` | `pipelineService.ts:116` | `get_realtime_session_cache` | only test mocks `__tests__/useHomePage.test.ts:18` |
| `checkSttProviderHealth` | `settingsService.ts:71` | `check_stt_provider_health` | only `__tests__/settingsService.test.ts:95` |
| `cancelModelSetup` | `modelService.ts:93` | `cancel_model_setup` | only `__tests__/modelService.test.ts:61` |
| `completeSetupWizard` (model copy) | `modelService.ts:98` | `complete_setup_wizard` | prod uses `settingsService.ts:105` (`CompletedStep.tsx:2` imports from there); this copy is orphaned |

---

## 5. Gap Analysis

* **Backend→frontend audited:** `recent_work.md:125` §22 documents `IpcEvent` registry ownership (`core/events.rs:128` enum, `eventsService.ts:105` `IpcEventMap`, `cargo clippy` + `pnpm build` gate). Every `app.emit(app.emit_to)` now goes through `emit_ipc` helpers — zero raw strings at emit sites.
* **Frontend→backend missed:** no `CommandMap`, no `InvokeRegistry`, no handler→wrapper trace, no orphan/dead pass, no `AGENTS.md:4.1#5` service-boundary check. Style guide `backend-style-guide.md:137` notes `Commands are not events and must remain in their owning actor/service command enum` — never executed as task.

Result: audit that claimed "`IPC events`" covered only one direction (emit). The 86-command `generate_handler!` surface—auth, mutation vs query, hot vs cold, guard coverage—was unexamined.

---

## 6. Orphans & Dead Code — 12 Handlers, 0 Frontend Consumers

| # | Command | File | Why dead | Verdict |
|---|---|---|---|---|
| 1 | `get_dictation_settings` | `ipc/pipeline/dictation.rs:8` | `settings.dictation.clone()` — payload already in `request_boot_state.settings.dictation` | **Prune**, fold into boot |
| 2 | `get_last_dictation_transcript` | `dictation.rs:17` | `Mutex<Option<String>>` read-only here, never consumed | **Prune** (re-add as `dictation:{action:"get"}` if clipboard returns) |
| 3 | `copy_last_dictation_transcript` | `dictation.rs:26` | Same lock → `clipboard::set_text`, unreachable | **Prune** |
| 4 | `list_output_devices` | `ipc/audio.rs:66` | No output picker UI; mirror of input filter+cache | **Prune** or merge into `list_devices{kind}` |
| 5 | `check_engine_status` | `ipc/pipeline/assistant.rs:11` | Superset available in `get_runtime_snapshot.is_*_loaded` | **Prune** |
| 6 | `toggle_tray_visibility` | `ipc/tray.rs:11` | Internal tray menu already calls helper directly `lib.rs:294`; frontend uses `sync_hud_visibility` | **Demote** to internal helper, remove from handler |
| 7 | `get_memory_stats` | `memory/graph.rs:272` | Overlaps `get_memory_queue_status`+`get_memory_graph_topology` counts | **Prune** |
| 8 | `get_memory_relations` | `memory/ingestion.rs:44` | `get_memory_graph_topology.edges` already filtered | **Prune** |
| 9 | `get_runtime_history` | `ipc/monitoring.rs:14` | Client builds history from polling `get_runtime_snapshot` (`useMonitoringMetrics.ts:33`) | **Prune** |
| 10 | `clear_runtime_history` | `ipc/monitoring.rs:20` | Same — fire-and-forget, never pulled | **Prune** |
| 11 | `get_realtime_session_cache` | `ipc/pipeline/mod.rs:18` | Service exists but 0 component callers outside test mocks | **Prune** or fold into `request_boot_state` resumption field |
| 12 | `get_settings` | `settings/catalog.rs:121` | Strict subset of `request_boot_state` (`VoxSettings` alone) | **Prune** |

Plus **unregistered dead def** `rename_voice` `voices.rs:235` — delete or register+wire.

---

## 7. Duplicate / Merge Clusters — 6 Clusters, ~14 Handlers Save

| # | Cluster | Files | Current | Proposed | Save |
|---|---|---|---|---|---|
| C1 | **Health** `check_llm|stt|tts_provider_health` | `settings/health.rs:6,73,117` | 3 | `check_provider_health{kind:"llm"|"stt"|"tts", provider?}` | −2 |
| C2 | **Updates** `check_for_updates`+`check_for_model_updates` | `setup.rs:11,20` | 2 | `check_updates{scope:"app"|"models"|"all"}` | −1 |
| C3 | **Models** `start_model_setup`+`download_optional_model`(+`cancel` variant) | `setup.rs:94,147,326` | 3 | `manage_models{action:"download"|"cancel", ids?:string[]}` (+ `delete`/`exists` consolidated via C6) | −2 |
| C4 | **Toast** `show|hide|destroy` | `toast.rs:162` | 3 (+ `get_last_toast` kept) | `manage_toast_window{action:"show"|"hide"|"destroy"}` keep `get_last_toast` | −2 |
| C5 | **Audio** `list_input|output_devices` | `audio.rs:21,66` | 2 | `list_devices{kind:"input"|"output"}` shared `DEVICE_CACHE_TTL=5s` | −1 |
| C6 | **Memory mutations** `edit_fact_content`+`user_edit_memory`+`soft_delete_fact`+`user_delete_memory`+`reassign_fact_collection` | `memory/mutations.rs:9` | 5 | `manage_fact{action:"edit"|"reassign"|"delete", mode?:"in_place"|"supersede"}` | −2 |
| — | **Tray** `toggle_tray_visibility`+`sync_hud_visibility`+`hide_tray_window`+`show_main_window` | `ipc/tray.rs:11` | 4 (+ `set_hud_ignore_cursor` kept) | `set_tray_visibility{mode:"show"|"hide"|"toggle", target?}` | −2 (partial) |
| — | **Catalog** `fetch_manifest` vs `request_model_catalog` | `setup.rs:29` + `settings/catalog.rs:53` | 2 | `request_model_catalog` as SSOT; make wizard call it (keep `fetch_manifest` only as internal fallback) | −1 |
| — | **Capabilities** `validate_llm_token_cap` vs `probe_model_capabilities` | `settings/health.rs:242,305` | 2 | `probe_capabilities{provider?, modelId?, validateCap?}` | −1 |
| — | **Model status** `check_model_exists` vs `get_runtime_report` | `setup.rs:300,47` | 2 | `get_model_status{id?}` superset shares `verify_runtime(manifest)` | −1 |

**Immediate 16-handler saving** without touching hot session/PTT/pipeline paths.

### Other overlaps noted (lower priority)

* `get_graph_version` `memory/graph.rs:66` vs `get_memory_graph_topology` `graph.rs:145` — version already returned as `MemoryGraphPayload.version`; separate call saves one round-trip on poll. Keep if 10s poll cost matters, else prune.
* `get_transcript_history`+`commit_session_to_history` `history.rs:7` — ephemeral tray buffer intentionally separate from SQLite `get_turns`. Keep.
* Frontend wrapper dupe `completeSetupWizard` in `modelService.ts:98` + `settingsService.ts:104` → consolidate to `modelService` single export.

---

## 8. Service-Boundary Violations — `AGENTS.md:4.1#5`

| # | File | Lines | Violation | Fix |
|---|---|---|---|---|
| V1 | `app/src/toast/ToastApp.tsx:5,34,37,55,80` | 4 invokes | `import { invoke } from "@tauri-apps/api/core"` outside `services/` | Move 4 into `services/windowService.ts` or new `services/toastService.ts`: `showToastWindow`/`hideToastWindow`/`destroyToastWindow`/`getLastToast` |
| V2 | `app/src/shared/components/settings/models/LlmSettingsView.tsx:7,99` | 1 invoke | Direct `invoke("validate_llm_token_cap")` in component | Add `validateLlmTokenCap` to `services/settingsService.ts` or `services/modelService.ts`, call via store |

All 7 `services/*.ts` imports are compliant; 0 other `invoke` leaks outside those 9 files. No `emit` from frontend (0 hits) — events remain backend→frontend only.

---

## 9. Read vs Write & Hot vs Cold

**Classification (86):** 38 write/mutation (44%) — many with guards in `assistant.rs:32` (`start` rejects not-Idle, `pause` rejects Idle, `ptt_stop` requires Listening), but dictation trio and `clear_runtime_history` have none. 48 read/query.

**Profile:** ~18 hot (session: `start|pause|resume|end`, `ptt_*`, polling `get_runtime_snapshot` @1–2s `useMonitoringMetrics.ts:31`, `get_profiler_snapshot` while drawer open; memory graph `get_graph_version`+`get_memory_graph_topology` @10s `Memory.tsx:90`); ~30 cold config (12 setup + 8 health/catalog + 5 voices + 5 history). Cold config is where 42% shrink lives with zero runtime risk.

---

## 10. Recommendations

### 10.1 Prune Now — 12 handlers (no production callers, no guard risk)

| Prune | File | Replacement |
|---|---|---|
| `get_dictation_settings`, `get_last_dictation_transcript`, `copy_last_dictation_transcript` | `ipc/pipeline/dictation.rs:8` | Already in `request_boot_state`; clipboard path re-add as `dictation_clipboard{action}` if needed |
| `list_output_devices` | `ipc/audio.rs:66` | Merge into `list_devices{kind}` or prune until picker exists |
| `check_engine_status` | `ipc/pipeline/assistant.rs:11` | Derive from `get_runtime_snapshot` |
| `toggle_tray_visibility` IPC entry | `ipc/tray.rs:11` | Keep internal helper `toggle_tray_visibility(handle)` for tray menu; remove from `generate_handler!` |
| `get_memory_stats` | `memory/graph.rs:272` | `get_memory_queue_status` + topology counts |
| `get_memory_relations` | `memory/ingestion.rs:44` | `get_memory_graph_topology.edges` |
| `get_runtime_history`, `clear_runtime_history` | `ipc/monitoring.rs:14,20` | Client ring buffer (`useMonitoringMetrics`) |
| `get_realtime_session_cache` | `ipc/pipeline/mod.rs:18` | Fold `expires_in_seconds` into `BootState` |
| `get_settings` | `settings/catalog.rs:121` | `request_boot_state` |
| `rename_voice` dead def | `voices.rs:235` | Delete or register+wire UI |

**Delta:** 86→74.

### 10.2 Merge Clusters — 6 groups (saves ~14 handlers, no behavior loss)

Apply these signatures (keep return types, preserve guards):

```ts
// Health — settings/health.rs:6
check_provider_health(kind: "llm"|"stt"|"tts", provider?: ProviderConfig): Promise<boolean>

// Updates — setup.rs:11
check_updates(scope: "app"|"models"|"all"): Promise<{ app?: UpdateReport, models?: ModelUpdateReport }>

// Models — setup.rs:94  (action-folded)
manage_models(action: "download"|"cancel"|"delete"|"exists", id?: string, ids?: string[]): Promise<void|boolean>
// download replaces start_model_setup + download_optional_model; cancel folded; exists replaces check_model_exists

// Toast — toast.rs:162
manage_toast_window(action: "show"|"hide"|"destroy"): Promise<void>
// keep: get_last_toast(): Promise<ToastPayload|null>

// Audio — audio.rs:21
list_devices(kind: "input"|"output"): Promise<AudioDevice[]>

// Memory — memory/mutations.rs:9
manage_fact(action: "edit"|"reassign"|"delete", fact_id: string, content?: string, collection?: string, mode?: "in_place"|"supersede"): Promise<void|string>
// edit w/ mode covers edit_fact_content vs user_edit_memory; delete covers soft vs user; reassign kept
// retry pair -> retry_queue(scope:"all"|"selected", ids?:number[])

 // Tray — tray.rs:11  (optional aggressive)
set_tray_visibility(mode: "show"|"hide"|"toggle", target?: "tray"|"main"): Promise<void>
// replaces toggle + sync + hide/show; keep set_hud_ignore_cursor separate

 // Capabilities — settings/health.rs:242
probe_capabilities(provider?: LlmProviderConfig, modelId?: string, validateCap?: number): Promise<ModelCapabilities>
// fold validate_llm_token_cap flag

 // Catalog — settings/catalog.rs:53
request_model_catalog(): ModelCatalog // SSOT; make wizard call it; demote fetch_manifest to internal fallback
```

**Phased delta:**

| Phase | Op | Δ | Total |
|---|---|---:|---|
| Start | 86 registered | — | 86 |
| A Prune strict dead (§10.1) | −12 | 74 |
| B Dedup `user_edit|user_delete` + wrapper | −2 | 72 |
| C Health 3→1 | −2 | 70 |
| D `check_updates` 2→1 | −1 | 69 |
| E Models 3→1 (+ `check_model_exists`→`manage_models exists`) | −3 | 66 |
| F Toast 4→2 | −2 | 64 |
| G Audio 2→1 | −1 | 63 |
| H Memory mutations 5→3 + `resolve_memory_conflict` wire/prune | −1 | 62 |
| I Capabilities 2→1 | −1 | 61 |
| J Tray 4→2 | −2 | 59 |
| Aggressive (fold `get_graph_version` into topology, `fetch_manifest` into catalog, `get_realtime_session_cache` into boot) | −3 | **56** |

**Conservative target: 59. Aggressive target: 48–52** (if all of §7's “−1” notes taken). Recommendation: ship **A–G now (→63, 27% shrink)**, schedule **H–J + aggressive** behind `settingsStore` + `VocabManager` tests.

### 10.3 Fix Violations (before next clippy gate)

* Create `services/toastService.ts` or extend `windowService.ts:1` with `showToastWindow`/`hideToastWindow`/`destroyToastWindow`/`getLastToast`; update `toast/ToastApp.tsx:34` to drop `invoke` import.
* Add `validateLlmTokenCap` to `services/settingsService.ts:87` (next to `probe_model_capabilities`); update `LlmSettingsView.tsx:99`.

### 10.4 Add Missing `InvokeRegistry` (prevent drift)

Mirror `core/events.rs:128` for commands — e.g. `core/commands.rs` enum or `ipc/mod.rs` re-export table with `name()` + typed payload guards. Frontend mirrors via `CommandMap` in `services/commandMap.ts` and generic `invokeCommand<K>(k, payload)`. Each new `#[tauri::command]` must amend the registry or `cargo check` fails (like `IpcEvent` today).

### 10.5 Guard & Safety Notes

* Preserve `assistant.rs:32` session guards; dictation prune removes unguarded trio.
* Monitoring folds keep `get_runtime_snapshot` throttling — do not re-introduce `get_runtime_history` bypass.
* Memory `edit_fact_content` guard `pipeline_processing_enabled||retrieval_enabled` should be shared across `manage_fact` delete/reassign (currently soft_delete has none).
* Thread invariants unchanged: `spawn_blocking` for audio enumeration, `tokio` for session/monitoring remain.

---

## 11. Appendix — Frontend→Backend Mapping (for trace)

Full 75-string map `app/src/services/*.ts` → `lib.rs:494` is 1:1 spelling exact (0 typos). Remaining 11 orphan strings have zero grep hits in `app/src` by design (above table). 6 `DEAD` wrappers have production hits only inside `services/__tests__` mocks.

---

*Evidence files: `lib.rs:494`, `core/events.rs:11,128`, `services/eventsService.ts:105`, `services/pipelineService.ts:61`, `services/settingsService.ts:33`, `services/modelService.ts:69`, `services/memoryService.ts:69`, `services/windowService.ts:4`, `services/historyService.ts:32`, `services/memoryProfilerService.ts:77`, `toast/ToastApp.tsx:34`, `shared/components/settings/models/LlmSettingsView.tsx:99`, `ipc/pipeline/*.rs`, `ipc/tray.rs:11`, `ipc/audio.rs:21`, `ipc/monitoring.rs:8`, `ipc/setup.rs:11`, `ipc/memory/*.rs`, `ipc/voices.rs:235`.*
