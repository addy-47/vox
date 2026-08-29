# Function Inventory Checklist (Total: 572 functions across 94 files)

## `app/src-tauri/src/ipc/audio.rs` (78 lines, 2 functions)

- [x] `L  12`: `list_input_devices` [Production]
- [x] `L  46`: `list_output_devices` [Production]

## `app/src-tauri/src/ipc/dictation.rs` (45 lines, 3 functions)

- [x] `L  12`: `get_dictation_settings` [Production]
- [x] `L  21`: `get_last_dictation_transcript` [Production]
- [x] `L  30`: `copy_last_dictation_transcript` [Production]

## `app/src-tauri/src/ipc/history.rs` (154 lines, 5 functions)

- [x] `L   8`: `get_transcript_history` [Production]
- [x] `L  17`: `commit_session_to_history` [Production]
- [x] `L  64`: `get_sessions` [Production]
- [x] `L  99`: `get_turns` [Production]
- [x] `L 138`: `delete_session` [Production]

## `app/src-tauri/src/ipc/memory/conflicts.rs` (124 lines, 2 functions)

- [x] `L  16`: `get_unresolved_conflicts` [Production]
- [x] `L  76`: `resolve_memory_conflict` [Production]

## `app/src-tauri/src/ipc/memory/graph.rs` (283 lines, 4 functions)

- [x] `L  67`: `get_graph_version` [Production]
- [x] `L  76`: `get_memory_graph_topology` [Production] (Updated with parameterized collection filtering)
- [x] `L 186`: `get_memory_fact_detail` [Production]
- [x] `L 278`: `get_memory_stats` [Production]

## `app/src-tauri/src/ipc/memory/ingestion.rs` (215 lines, 5 functions)

- [x] `L  45`: `get_memory_relations` [Production]
- [x] `L  78`: `get_memory_queue_status` [Production]
- [x] `L 150`: `toggle_pipeline_processing` [Production] (Unified enable/pause with settings sync)
- [x] `L 177`: `retry_failed_queue` [Production] (Guarded against paused pipeline)
- [x] `L 204`: `retry_failed_queue_items` [Production] (Guarded against paused pipeline)

## `app/src-tauri/src/ipc/memory/mutations.rs` (229 lines, 5 functions)

- [x] `L  10`: `edit_fact_content` [Production]
- [x] `L  89`: `reassign_fact_collection` [Production]
- [x] `L 136`: `soft_delete_fact` [Production]
- [x] `L 197`: `user_edit_memory` [Production]
- [x] `L 223`: `user_delete_memory` [Production]

## `app/src-tauri/src/ipc/memory_profiler.rs` (356 lines, 6 functions)

- [x] `L   6`: `resolve_temp_dir` [Production]
- [x] `L  34`: `sanitize_page_name` [Production]
- [x] `L  84`: `extract_descendant_processes` [Production] (Extracted helper ≤50 lines)
- [x] `L 155`: `assign_webview_roles` [Production] (Extracted helper ≤50 lines)
- [x] `L 197`: `collect_profiler_snapshot_internal` [Production] (Decomposed coordinator ≤50 lines)
- [x] `L 267`: `collect_profiler_snapshot` [Production]
- [x] `L 277`: `get_profiler_snapshot` [Production]
- [x] `L 313`: `record_memory_profile_event` [Production]

## `app/src-tauri/src/ipc/monitoring.rs` (22 lines, 3 functions)

- [x] `L   8`: `get_runtime_snapshot` [Production]
- [x] `L  14`: `get_runtime_history` [Production]
- [x] `L  20`: `clear_runtime_history` [Production]

## `app/src-tauri/src/ipc/pipeline/engine_launch.rs` (394 lines, 1 functions)

- [x] `L  22`: `launch_engine` [Disaster] (394-line monolithic launcher orchestrating 5+ workers in IPC layer; queued for Layer 2 rewrite)

## `app/src-tauri/src/ipc/pipeline/lifecycle.rs` (416 lines, 5 functions)

- [x] `L  14`: `check_engine_status` [Production]
- [x] `L  22`: `stop_engine` [Bad Code] (58 lines orchestrating model eviction, thread joins, persistence shutdown; queued for Layer 2)
- [x] `L  82`: `engage` [Disaster] (202 lines, toggle function violation, direct persistence channel sends, ONNX lazy loads; queued for Layer 2 rewrite)
- [x] `L 287`: `pause_pipeline` [Bad Code] (52 lines, handles modular vs realtime routing inline; queued for Layer 2)
- [x] `L 342`: `resume_pipeline` [Bad Code] (74 lines, lazy reconnect & inline settings/state resolution; queued for Layer 2)

## `app/src-tauri/src/ipc/pipeline/realtime.rs` (408 lines, 4 functions)

- [x] `L  17`: `start_realtime_session_internal` [Disaster] (296 lines, complex session initialization, resumption token parsing, idle timeout loop; queued for Layer 2)
- [x] `L 317`: `start_realtime_session` [Production]
- [x] `L 329`: `get_realtime_session_cache` [Production]
- [x] `L 360`: `stop_realtime_session` [Production]

## `app/src-tauri/src/ipc/pipeline/test_clip.rs` (203 lines, 3 functions)

- [x] `L  13`: `decode_wav_to_mono_f32` [Production]
- [x] `L  58`: `test_clip` [Bad Code] (109 lines, handles engine orchestration directly in IPC; queued for Layer 2)
- [x] `L 171`: `test_clip_cancel` [Production]

## `app/src-tauri/src/ipc/settings/catalog.rs` (135 lines, 3 functions)

- [x] `L  39`: `request_boot_state` [Production]
- [x] `L  59`: `request_model_catalog` [Production]
- [x] `L 127`: `get_settings` [Production]

## `app/src-tauri/src/ipc/settings/health.rs` (515 lines, 8 functions)

- [x] `L   9`: `check_llm_provider_health` [Production]
- [x] `L  77`: `check_stt_provider_health` [Production]
- [x] `L 120`: `check_tts_provider_health` [Production]
- [x] `L 170`: `list_llm_models` [Production]
- [x] `L 218`: `get_cached_capabilities` [Production]
- [x] `L 232`: `probe_model_capabilities` [Production]
- [x] `L 286`: `validate_llm_token_cap` [Production]
- [x] `L 309`: `resolve_setup_script` [Production] (Extracted helper ≤50 lines)
- [x] `L 332`: `parse_setup_progress` [Production] (Extracted helper ≤50 lines)
- [x] `L 353`: `run_remote_ssh_task` [Production] (Decomposed SSH streaming task)
- [x] `L 488`: `setup_remote_server` [Production] (Thin IPC dispatcher ≤50 lines)

## `app/src-tauri/src/ipc/settings/mutation.rs` (819 lines, 9 functions)

- [x] `L  30`: `handle_dictation_side_effects` [Production] (Extracted helper ≤50 lines)
- [x] `L  93`: `handle_interaction_side_effects` [Production] (Extracted helper ≤50 lines)
- [x] `L 135`: `handle_setting_side_effects` [Production] (Extracted dispatcher ≤50 lines)
- [x] `L 165`: `update_setting` [Production] (Decomposed coordinator ≤50 lines)
- [x] `L 225`: `update_theme` [Production]
- [x] `L 241`: `reset_settings` [Production]
- [x] `L 261`: `apply_setting_mutation` [Production] (Type-safe domain/key settings parser with unit tests)
- [x] `L 752`: `dispatch_worker_command` [Production]
- [x] `L 796`: `schedule_debounced_save` [Production]

## `app/src-tauri/src/ipc/settings/tests.rs` (106 lines, 2 functions)

- [x] `L  12`: `test_apply_setting_mutation_type_safety` [Production]
- [x] `L  50`: `test_setting_numeric_bounds` [Production]

## `app/src-tauri/src/ipc/setup.rs` (423 lines, 15 functions)

- [x] `L  11`: `check_for_updates` [Production]
- [x] `L  19`: `check_for_model_updates` [Production]
- [x] `L  27`: `fetch_manifest` [Production]
- [x] `L  47`: `get_runtime_report` [Production]
- [x] `L  56`: `execute_model_setup_task` [Production] (Extracted background download loop ≤50 lines)
- [x] `L  88`: `start_model_setup` [Production] (Thin IPC coordinator ≤50 lines)
- [x] `L 128`: `cancel_model_setup` [Production]
- [x] `L 134`: `get_onboarding_status` [Production]
- [x] `L 141`: `complete_setup_wizard` [Production]
- [x] `L 182`: `ensure_manifest_loaded` [Production]
- [x] `L 209`: `resolve_model_dest_path` [Production] (Extracted helper ≤50 lines)
- [x] `L 225`: `is_model_file_present` [Production] (Extracted verification helper ≤50 lines)
- [x] `L 259`: `check_model_exists` [Production] (Thin query coordinator ≤50 lines)
- [x] `L 279`: `download_optional_model` [Production] (Dedicated background downloader ≤50 lines)
- [x] `L 311`: `reveal_wizard` [Production]
- [x] `L 321`: `delete_model_file` [Production] (Extracted file & folder cleanup helper ≤50 lines)
- [x] `L 348`: `delete_model` [Production] (Thin deletion coordinator ≤50 lines)

## `app/src-tauri/src/ipc/tray.rs` (273 lines, 8 functions)

- [x] `L   9`: `toggle_hud_visibility` [Production] (Valid UI toggle for HUD overlay checkmark)
- [x] `L  53`: `cancel_active_dictation_turn` [Production] (Extracted helper ≤50 lines)
- [x] `L  87`: `hide_tray_window` [Production]
- [x] `L 104`: `sync_hud_visibility` [Production] (Decomposed synchronization coordinator ≤50 lines)
- [x] `L 160`: `set_hud_ignore_cursor` [Production]
- [x] `L 177`: `evaluate_main_mode_engine_lifecycle` [Production] (Extracted helper ≤50 lines)
- [x] `L 204`: `update_interaction_mode` [Production] (Decomposed coordinator ≤50 lines)
- [x] `L 268`: `show_main_window` [Production]

## `app/src-tauri/src/ipc/voices.rs` (215 lines, 13 functions)

- [x] `L  29`: `from` [Production]
- [x] `L  42`: `open_db` [Production]
- [x] `L  49`: `now_epoch` [Production]
- [x] `L  57`: `validate_wav` [Production] (Thin IPC dispatcher ≤50 lines)
- [x] `L  65`: `list_voices` [Production]
- [x] `L  74`: `add_voice_from_file` [Production] (Thin IPC coordinator ≤50 lines)
- [x] `L 118`: `add_voice_from_recording` [Production] (Thin IPC coordinator ≤50 lines)
- [x] `L 165`: `delete_voice` [Production]
- [x] `L 186`: `rename_voice` [Production]
- [x] `L 196`: `preview_voice` [Production] (Thin IPC coordinator ≤50 lines)
- [x] `L 229`: `start_backend_recording` [Production]
- [x] `L 235`: `stop_backend_recording` [Production]
- [x] `L 241`: `fetch_edge_tts_voices` [Production]

## `app/src-tauri/src/services/tts/voice.rs` (360 lines, 10 functions)

- [x] `L  24`: `validate_wav_file` [Production]
- [x] `L  42`: `extract_mono_f32_samples` [Production]
- [x] `L  85`: `decode_audio_stream` [Production]
- [x] `L 138`: `resample_linear` [Production]
- [x] `L 158`: `pad_or_truncate_audio` [Production]
- [x] `L 170`: `write_f32_wav` [Production]
- [x] `L 190`: `convert_and_validate_audio` [Production]
- [x] `L 207`: `write_pcm_to_wav` [Production]
- [x] `L 217`: `pre_bake_speaker_tensors` [Production]
- [x] `L 251`: `synthesize_preview_clip` [Production]
- [x] `L 293`: `start_recording` [Production]
- [x] `L 339`: `stop_recording` [Production]
- [x] `L 357`: `fetch_remote_edge_voices` [Production]

## `app/src-tauri/src/services/audio/decode.rs` (371 lines, 13 functions)

- [x] `L  41`: `decode_bytes_to_24khz_mono` [Production]
- [x] `L  52`: `decode_to_24khz_mono` [Production]
- [x] `L  67`: `decode_mss` [Production]
- [x] `L 115`: `decode_packets` [Production] (Extracted helper)
- [x] `L 151`: `append_samples_as_f32_mono` [Production] (Extracted helper)
- [x] `L 218`: `resample_linear` [Production]
- [x] `L 236`: `truncate_to` [Production]
- [x] `L 250`: `write_wav_f32` [Production]
- [x] `L 282`: `write_wav_f32_raw` [Production]
- [x] `L 316`: `create_test_wav` [Production]
- [x] `L 336`: `test_decode_wav_to_24khz` [Production]
- [x] `L 347`: `test_truncate_to` [Production]
- [x] `L 357`: `test_write_and_read_back` [Production]

## `app/src-tauri/src/services/audio/device.rs` (176 lines, 6 functions)

- [x] `L  19`: `new` [Production]
- [x] `L  37`: `start` [Production]
- [x] `L  45`: `resolve_input_device` [Production] (Extracted helper)
- [x] `L  71`: `resolve_audio_host` [Production] (Extracted helper)
- [x] `L  96`: `build_input_stream` [Production] (Extracted helper)
- [x] `L 154`: `drop` [Production]

## `app/src-tauri/src/services/audio/playback.rs` (497 lines, 19 functions)

- [x] `L  16`: `upsample_2x` [Production]
- [x] `L  60`: `new` [Production]
- [x] `L 108`: `ingest_chunk` [Production]
- [x] `L 128`: `start_playback` [Production]
- [x] `L 145`: `cancel` [Production]
- [x] `L 154`: `is_idle` [Production]
- [x] `L 159`: `buffer_len` [Production]
- [x] `L 164`: `total_samples_ingested` [Production]
- [x] `L 169`: `reset_samples_ingested` [Production]
- [x] `L 175`: `build_cpal_stream` [Production]
- [x] `L 223`: `resolve_output_device_and_config` [Production] (Extracted helper)
- [x] `L 267`: `process_output_buffer` [Production] (Extracted helper)
- [x] `L 286`: `reset_telemetry_state` [Production] (Extracted helper)
- [x] `L 293`: `drain_and_telemetry` [Production] (Extracted helper)
- [x] `L 351`: `update_energy_metrics` [Production] (Extracted helper)
- [x] `L 384`: `drop` [Production]
- [x] `L 396`: `test_upsample_2x_boundaries` [Production]
- [x] `L 434`: `test_upsample_2x_sine_wave_fidelity` [Production]
- [x] `L 478`: `test_playback_barge_in_discard` [Production]

## `app/src-tauri/src/services/audio/router.rs` (152 lines, 6 functions)

- [x] `L  26`: `spawn` [Production]
- [x] `L  79`: `set_mode` [Production]
- [x] `L  86`: `start_realtime` [Production]
- [x] `L  93`: `stop_realtime` [Production]
- [x] `L 101`: `handle_router_commands` [Production] (Extracted helper)
- [x] `L 124`: `route_audio_chunk` [Production] (Extracted helper)

## `app/src-tauri/src/services/dictation/clipboard.rs` (73 lines, 3 functions)

- [x] `L   5`: `get_text` [Production]
- [x] `L  27`: `set_text` [Production]
- [x] `L  48`: `with_clipboard_safe` [Production]

## `app/src-tauri/src/services/dictation/controller.rs` (215 lines, 6 functions)

- [x] `L  14`: `handle_press` [Production]
- [x] `L  47`: `handle_release` [Production]
- [x] `L  78`: `handle_cancel` [Production]
- [x] `L 106`: `ensure_engine_running` [Production] (Extracted helper)
- [x] `L 128`: `begin_dictation_turn` [Production] (Extracted helper)
- [x] `L 165`: `handle_silent_dictation_release` [Production] (Extracted helper)
- [x] `L 186`: `finalize_dictation_audio` [Production] (Extracted helper)

## `app/src-tauri/src/services/dictation/hotkey.rs` (76 lines, 1 functions)

- [x] `L  11`: `register_global_hotkey` [Production]

## `app/src-tauri/src/services/dictation/input.rs` (210 lines, 6 functions)

- [x] `L  10`: `simulate_paste` [Production]
- [x] `L  17`: `simulate_paste` (X11) [Production]
- [x] `L  59`: `simulate_paste` (Wayland) [Production]
- [x] `L  96`: `simulate_paste` (macOS) [Production]
- [x] `L 138`: `simulate_paste` (Windows) [Production]
- [x] `L 169`: `create_input_adapter` [Production]

## `app/src-tauri/src/services/dictation/output_router.rs` (105 lines, 4 functions)

- [x] `L  13`: `route_transcript` [Production]
- [x] `L  38`: `dispatch_to_tray` [Production] (Extracted helper)
- [x] `L  60`: `dispatch_to_clipboard` [Production] (Extracted helper)
- [x] `L  75`: `dispatch_to_paste` [Production] (Extracted helper)

## `app/src-tauri/src/services/llm/actor.rs` (63 lines, 1 functions)

- [x] `L  16`: `spawn_llm_worker` [Production]

## `app/src-tauri/src/services/llm/capabilities.rs` (141 lines, 7 functions)

- [x] `L  30`: `static_supported` [Production]
- [x] `L  40`: `static_unsupported` [Production]
- [x] `L  49`: `unknown` [Production]
- [x] `L  71`: `default_for_kind` [Production]
- [x] `L 106`: `new` [Production]
- [x] `L 113`: `get_or_insert_default` [Production]
- [x] `L 129`: `update_observation` [Production]

## `app/src-tauri/src/services/llm/capability_probe.rs` (970 lines, 19 functions)

- [x] `L  53`: `probe_capabilities` [Production]
- [x] `L  88`: `probe_local_embedded` [Production] (Extracted helper)
- [x] `L 124`: `probe_openai_compat_endpoint` [Production] (Extracted helper)
- [x] `L 194`: `probe_ollama_metadata` [Production] (Extracted helper)
- [x] `L 266`: `resolve_gpu_status` [Production] (Extracted helper)
- [x] `L 338`: `resolve_chat_url` [Production]
- [x] `L 353`: `validate_token_cap` [Production]
- [x] `L 412`: `is_cloud_provider` [Production]
- [x] `L 436`: `title_case` [Production]
- [x] `L 448`: `heuristic_embedded_caps` [Production]
- [x] `L 460`: `parse_token_ceiling_from_error` [Production]
- [x] `L 495`: `streaming_inference_probe` [Production]
- [x] `L 659`: `structured_tool_probe` [Production]
- [x] `L 730`: `test_heuristic_embedded_caps_known_families` [Production]
- [x] `L 772`: `test_heuristic_embedded_caps_unknown_models` [Production]
- [x] `L 792`: `test_is_cloud_provider_by_provider_name` [Production]
- [x] `L 822`: `test_is_cloud_provider_by_base_url` [Production]
- [x] `L 846`: `test_is_cloud_provider_local_and_custom` [Production]
- [x] `L 867`: `test_parse_token_ceiling_from_error_valid_patterns` [Production]
- [x] `L 926`: `test_parse_token_ceiling_bounds_and_edge_cases` [Production]

## `app/src-tauri/src/services/llm/llama_cpp.rs` (721 lines, 16 functions)

- [x] `L  28`: `detect` [Production]
- [x] `L  48`: `format_system_prompt` [Production]
- [x] `L  71`: `format_user_prompt` [Production]
- [x] `L  97`: `format_prompt` [Production]
- [x] `L 105`: `format_conversation` [Production]
- [x] `L 127`: `stop_sequences` [Production]
- [x] `L 143`: `tags_to_strip` [Production]
- [x] `L 186`: `strip_tags_raw` [Production]
- [x] `L 260`: `strip_tags` [Production]
- [x] `L 284`: `partial_tag_len` [Production]
- [x] `L 297`: `new` [Production]
- [x] `L 338`: `token_to_bytes` [Production]
- [x] `L 344`: `ctx_size` [Production]
- [x] `L 348`: `run_loop` [Production]
- [x] `L 384`: `init_context` [Production] (Extracted helper)
- [x] `L 410`: `generate` [Production]

## `app/src-tauri/src/services/llm/mod.rs` (59 lines, 2 functions)

- [x] `L  36`: `generate` [Production]
- [x] `L  52`: `global_llama_backend` [Production]

## `app/src-tauri/src/services/llm/policy.rs` (113 lines, 5 functions)

- [x] `L  10`: `calculate_compaction_max_tokens` [Production]
- [x] `L  41`: `from_settings` [Production]
- [x] `L  60`: `build_request` [Production]
- [x] `L  90`: `test_dynamic_compaction_scaling` [Production]
- [x] `L  97`: `test_policy_compaction_budget_clamped_for_cloud` [Production]

## `app/src-tauri/src/services/llm/probe.rs` (103 lines, 3 functions)

- [x] `L  12`: `new` [Production]
- [x] `L  22`: `registry` [Production]
- [x] `L  27`: `probe_top_k` [Production]

## `app/src-tauri/src/services/llm/providers/embedded.rs` (133 lines, 8 functions)

- [x] `L  21`: `new` [Production]
- [x] `L  32`: `generate` [Production]
- [x] `L  51`: `capabilities` [Production]
- [x] `L  55`: `health_check` [Production]
- [x] `L  59`: `list_models` [Production]
- [x] `L  67`: `kind` [Production]
- [x] `L  71`: `max_context_tokens` [Production]
- [x] `L  77`: `list_models_in_dir` [Production]

## `app/src-tauri/src/services/llm/providers/lm_studio.rs` (218 lines, 5 functions)

- [x] `L  21`: `new` [Production]
- [x] `L  39`: `capabilities` [Production]
- [x] `L  43`: `generate` [Production]
- [x] `L  81`: `build_request_body` [Production] (Extracted helper)
- [x] `L 142`: `stream_response` [Production] (Extracted helper)

## `app/src-tauri/src/services/llm/providers/mod.rs` (56 lines, 6 functions)

- [x] `L  25`: `generate` [Production]
- [x] `L  34`: `capabilities` [Production]
- [x] `L  37`: `health_check` [Production]
- [x] `L  40`: `list_models` [Production]
- [x] `L  43`: `kind` [Production]
- [x] `L  46`: `max_context_tokens` [Production]

## `app/src-tauri/src/services/llm/providers/ollama.rs` (194 lines, 5 functions)

- [x] `L  21`: `new` [Production]
- [x] `L  39`: `capabilities` [Production]
- [x] `L  43`: `generate` [Production]
- [x] `L  81`: `build_request_body` [Production] (Extracted helper)
- [x] `L 125`: `stream_response` [Production] (Extracted helper)

## `app/src-tauri/src/services/llm/providers/openai/chat_completions.rs` (246 lines, 6 functions)

- [x] `L  23`: `new` [Production]
- [x] `L  49`: `capabilities` [Production]
- [x] `L  53`: `inject_headers` [Production]
- [x] `L  69`: `generate` [Production]
- [x] `L 109`: `build_request_body` [Production] (Extracted helper)
- [x] `L 165`: `stream_response` [Production] (Extracted helper)

## `app/src-tauri/src/services/llm/providers/openai/responses.rs` (221 lines, 6 functions)

- [x] `L  22`: `new` [Production]
- [x] `L  46`: `capabilities` [Production]
- [x] `L  50`: `inject_headers` [Production]
- [x] `L  57`: `generate` [Production]
- [x] `L  94`: `build_request_body` [Production] (Extracted helper)
- [x] `L 142`: `stream_response` [Production] (Extracted helper)

## `app/src-tauri/src/services/llm/providers/openai_compat.rs` (713 lines, 12 functions)

- [x] `L  33`: `new` [Production]
- [x] `L  89`: `inject_headers` [Production]
- [x] `L 106`: `detect_backend_kind` [Production]
- [x] `L 216`: `generate` [Production]
- [x] `L 465`: `capabilities` [Production]
- [x] `L 469`: `health_check` [Production]
- [x] `L 492`: `list_models` [Production]
- [x] `L 582`: `kind` [Production]
- [x] `L 587`: `block_on` [Production]
- [x] `L 617`: `user_text_is_warmup` [Production]
- [x] `L 621`: `process_line` [Production]
- [x] `L 687`: `parse_heuristic_metadata` [Production]

## `app/src-tauri/src/services/llm/types.rs` (126 lines, 1 functions)

- [x] `L  74`: `default` [Production]

## `app/src-tauri/src/services/memory/classifiers/inter_edge_classifier.rs` (269 lines, 10 functions)

- [x] `L  22`: `init_edge_classifier` [Production]
- [x] `L  86`: `unload_edge_classifier` [Production]
- [x] `L  94`: `is_edge_classifier_loaded` [Production]
- [x] `L  98`: `ensure_edge_classifier_loaded` [Production]
- [x] `L 116`: `tokenize_input` [Production] (Extracted helper)
- [x] `L 142`: `run_inference` [Production] (Extracted helper)
- [x] `L 174`: `compute_softmax` [Production] (Extracted helper)
- [x] `L 204`: `classify_edge` [Production]
- [x] `L 237`: `test_special_state_collections_reject_inter_collection_edges` [Production]
- [x] `L 249`: `test_class_c_taxonomy_connection_matrix_compliance` [Production]

## `app/src-tauri/src/services/memory/classifiers/intra_edge_classifier.rs` (415 lines, 14 functions)

- [x] `L  19`: `as_str` [Production]
- [x] `L  56`: `init_nli_engine` [Production]
- [x] `L 127`: `unload_nli_engine` [Production]
- [x] `L 136`: `calibrate` [Production]
- [x] `L 186`: `raw_predict` [Production]
- [x] `L 194`: `encode_batch_pairs` [Production] (Extracted helper)
- [x] `L 254`: `raw_predict_batch` [Production]
- [x] `L 300`: `is_nli_loaded` [Production]
- [x] `L 306`: `ensure_nli_loaded` [Production]
- [x] `L 335`: `classify_pair` [Production]
- [x] `L 344`: `classify_batch` [Production]
- [x] `L 389`: `relation_from_result` [Production]
- [x] `L 400`: `get_calibrated_class_mapping` [Production]
- [x] `L 406`: `get_calibrated_class_mapping_strings` [Production]

## `app/src-tauri/src/services/memory/classifiers/query_classifier.rs` (140 lines, 9 functions)

- [x] `L  16`: `load` [Production]
- [x] `L  30`: `classify` [Production]
- [x] `L  42`: `init_scope_classifier` [Production]
- [x] `L  70`: `unload_scope_classifier` [Production]
- [x] `L  79`: `ensure_scope_classifier_loaded` [Production]
- [x] `L 100`: `classify_scope` [Production]
- [x] `L 116`: `is_scope_classifier_loaded` [Production]
- [x] `L 125`: `test_uninitialized_classifier_fallback` [Production]
- [x] `L 135`: `test_classifier_path_constants` [Production]

## `app/src-tauri/src/services/memory/deduplication.rs` (68 lines, 5 functions)

- [x] `L  10`: `jaccard_similarity` [Production]
- [x] `L  36`: `is_exact_duplicate` [Production]
- [x] `L  45`: `test_jaccard_similarity` [Production]
- [x] `L  55`: `test_jaccard_similarity_devanagari_matras` [Production]
- [x] `L  63`: `test_exact_duplicate_checks` [Production]

## `app/src-tauri/src/services/memory/embedder.rs` (370 lines, 13 functions)

- [x] `L  27`: `init_embedder` [Production]
- [x] `L  91`: `unload_embedder` [Production]
- [x] `L 102`: `ensure_embedder_loaded` [Production]
- [x] `L 124`: `encode_text` [Production] (Extracted helper)
- [x] `L 155`: `mean_pool_and_normalize` [Production] (Extracted helper)
- [x] `L 188`: `generate_embedding` [Production]
- [x] `L 226`: `is_embedder_loaded` [Production]
- [x] `L 232`: `l2_normalize_in_place` [Production]
- [x] `L 243`: `l2_normalize` [Production]
- [x] `L 250`: `cosine_similarity` [Production]
- [x] `L 269`: `test_l2_normalization_zero_vector` [Production]
- [x] `L 290`: `test_l2_normalization_standard` [Production]
- [x] `L 319`: `test_cosine_similarity_edge_cases` [Production]

## `app/src-tauri/src/services/memory/formatter.rs` (88 lines, 2 functions)

- [x] `L   5`: `format_relative_timestamp` [Production]
- [x] `L  44`: `format_user_profile_context` [Production]

## `app/src-tauri/src/services/memory/ingestion.rs` (227 lines, 9 functions)

- [x] `L  19`: `build_compaction_request` [Production] (Extracted helper)
- [x] `L  62`: `execute_compaction_attempt` [Production] (Extracted helper)
- [x] `L 104`: `run_compaction` [Production]
- [x] `L 177`: `test_compaction_empty_history` [Production]
- [x] `L 180`: `kind` [Production]
- [x] `L 183`: `health_check` [Production]
- [x] `L 186`: `capabilities` [Production]
- [x] `L 192`: `list_models` [Production]
- [x] `L 200`: `generate` [Production]

## `app/src-tauri/src/services/memory/mod.rs` (102 lines, 5 functions)

- [x] `L  36`: `unload_memory_pipeline_onnx_models` [Production]
- [x] `L  45`: `unload_all_onnx_models` [Production]
- [x] `L  62`: `trim_heap` [Production]
- [x] `L  76`: `GetCurrentProcess` [Production]
- [x] `L  77`: `EmptyWorkingSet` [Production]

## `app/src-tauri/src/services/memory/pipeline/runner.rs` (130 lines, 4 functions)

- [x] `L  12`: `run_pipeline_cycle` [Production]
- [x] `L  16`: `run_pipeline_cycle_with_id_seq` [Production]
- [x] `L 103`: `drain_pipeline_queue` [Production]
- [x] `L 110`: `drain_pipeline_queue_with_run_id` [Production]

## `app/src-tauri/src/services/memory/pipeline/stage1_dedup.rs` (329 lines, 6 functions)

- [x] `L  24`: `claim_staged_items` [Production] (Extracted helper)
- [x] `L  58`: `load_active_and_queue_facts` [Production] (Extracted helper)
- [x] `L  92`: `dedup_item_against_active` [Production] (Extracted helper)
- [x] `L 186`: `commit_dedup_statuses` [Production] (Extracted helper)
- [x] `L 212`: `run_stage1_dedup` [Production]
- [x] `L 216`: `run_stage1_dedup_with_metrics` [Production]

## `app/src-tauri/src/services/memory/pipeline/stage2_embed.rs` (236 lines, 4 functions)

- [x] `L  25`: `claim_deduped_items` [Production] (Extracted helper)
- [x] `L  56`: `process_stage2_item` [Production] (Extracted helper)
- [x] `L 162`: `run_stage2_embed` [Production]
- [x] `L 166`: `run_stage2_embed_with_metrics` [Production]

## `app/src-tauri/src/services/memory/pipeline/stage3_eval.rs` (483 lines, 8 functions)

- [x] `L  34`: `eval_subbranch_a_nli_sync` [Production]
- [x] `L 157`: `eval_subbranch_b_edges_sync` [Production]
- [x] `L 268`: `claim_embedded_items` [Production] (Extracted helper)
- [x] `L 310`: `evaluate_stage3_item` [Production] (Extracted helper)
- [x] `L 384`: `run_stage3_eval` [Production]
- [x] `L 388`: `run_stage3_eval_with_metrics` [Production]
- [x] `L 392`: `run_stage3_eval_with_metrics_seq` [Production]
- [x] `L 468`: `test_bidirectional_trigger_policy` [Production]

## `app/src-tauri/src/services/memory/pipeline/stage4_commit.rs` (213 lines, 4 functions)

- [x] `L  27`: `claim_commit_candidates` [Production] (Extracted helper)
- [x] `L  62`: `commit_item_to_storage` [Production] (Extracted helper)
- [x] `L 134`: `run_stage4_commit` [Production]
- [x] `L 138`: `run_stage4_commit_with_metrics` [Production]

## `app/src-tauri/src/services/memory/retrieval.rs` (224 lines, 4 functions)

- [x] `L  22`: `collect_sql_sections` [Production] (Extracted helper)
- [x] `L  68`: `collect_vector_graph_sections` [Production] (Extracted helper)
- [x] `L 156`: `retrieve_personal_context` [Production]
- [x] `L 218`: `test_vector_distance_ranking_order` [Production]

## `app/src-tauri/src/services/memory/scope_router.rs` (83 lines, 5 functions)

- [x] `L  11`: `route_scope` [Production]
- [x] `L  41`: `test_chitchat_scope_prunes_all_collections` [Production]
- [x] `L  48`: `test_user_scope_routing` [Production]
- [x] `L  58`: `test_domain_scope_routing` [Production]
- [x] `L  72`: `test_temporal_scope_routing` [Production]

## `app/src-tauri/src/services/memory/tokenizer.rs` (58 lines, 3 functions)

- [x] `L   8`: `get_bpe` [Production]
- [x] `L  17`: `estimate_tokens` [Production]
- [x] `L  44`: `test_bpe_english_and_devanagari_token_counts` [Production]

## `app/src-tauri/src/services/memory/working_memory.rs` (824 lines, 38 functions)

- [x] `L  23`: `fmt` [Production]
- [x] `L  47`: `current_timestamp_ms` [Production]
- [x] `L  74`: `new` [Production]
- [x] `L  99`: `total_token_count` [Production]
- [x] `L 103`: `set_max_context_tokens` [Production]
- [x] `L 113`: `load_identity_into_system_prompt` [Production]
- [x] `L 150`: `new_session` [Production]
- [x] `L 170`: `build_narrative_context_chain` [Production]
- [x] `L 192`: `update_system_prompt` [Production]
- [x] `L 206`: `push_user_turn` [Production]
- [x] `L 222`: `push_assistant_turn` [Production]
- [x] `L 243`: `pop_last_user_turn` [Production]
- [x] `L 256`: `context_utilization` [Production]
- [x] `L 264`: `get_messages` [Production]
- [x] `L 268`: `needs_threshold_maintenance` [Production]
- [x] `L 272`: `build_session_history_xml` [Production] (Extracted helper)
- [x] `L 302`: `consolidate_system_message` [Production] (Extracted helper)
- [x] `L 322`: `build_context` [Production]
- [x] `L 393`: `perform_fifo_maintenance` [Production]
- [x] `L 420`: `apply_compaction_result` [Production] (Extracted helper)
- [x] `L 448`: `perform_compaction_maintenance` [Production]
- [x] `L 493`: `try_trigger_opportunistic` [Production]
- [x] `L 517`: `commit_opportunistic` [Production]
- [x] `L 562`: `on_pipeline_idle` [Production]
- [x] `L 566`: `on_speech_start` [Production]
- [x] `L 570`: `cancel_opportunistic` [Production]
- [x] `L 578`: `latest_summary` [Production]
- [x] `L 598`: `test_conversation_manager_fifo` [Production]
- [x] `L 632`: `generate` [Production]
- [x] `L 650`: `capabilities` [Production]
- [x] `L 656`: `health_check` [Production]
- [x] `L 659`: `list_models` [Production]
- [x] `L 662`: `kind` [Production]
- [x] `L 668`: `test_perform_compaction_maintenance_json` [Production]
- [x] `L 694`: `test_perform_compaction_maintenance_markdown_fences` [Production]
- [x] `L 719`: `test_perform_compaction_maintenance_fallback_prose` [Production]
- [x] `L 745`: `test_fix_missing_commas` [Production]
- [x] `L 763`: `test_resilient_deserialization_of_compaction_response` [Production]
- [x] `L 784`: `test_single_system_message_and_9_collection_session_history` [Production]

## `app/src-tauri/src/services/pipeline/event_loop.rs` (1161 lines, 1 functions)

- [x] `L  17`: `run_event_loop` [Disaster] (Queued untouched for Layer 2 rewrite)

## `app/src-tauri/src/services/pipeline/handlers.rs` (316 lines, 4 functions)

- [x] `L  15`: `update_interaction_state` [Bad Code] (Queued untouched for Layer 2 rewrite)
- [x] `L  54`: `get_idle_state` [Bad Code] (Queued untouched for Layer 2 rewrite)
- [x] `L  62`: `get_current_owner` [Bad Code] (Queued untouched for Layer 2 rewrite)
- [x] `L  71`: `on_transcript_final` [Bad Code] (Queued untouched for Layer 2 rewrite)

## `app/src-tauri/src/services/pipeline/llm_lifecycle.rs` (100 lines, 2 functions)

- [x] `L  11`: `warm_up_llm` [Bad Code] (Queued untouched for Layer 2 rewrite)
- [x] `L  93`: `cool_down_llm` [Bad Code] (Queued untouched for Layer 2 rewrite)

## `app/src-tauri/src/services/pipeline/mod.rs` (119 lines, 1 functions)

- [x] `L  66`: `new` [Bad Code] (Queued untouched for Layer 2 rewrite)

## `app/src-tauri/src/services/pipeline/tests.rs` (157 lines, 5 functions)

- [x] `L  14`: `test_pipeline_state_variants_and_default` [Bad Code] (Queued untouched for Layer 2 rewrite)
- [x] `L  32`: `test_interaction_owner_conversions` [Bad Code] (Queued untouched for Layer 2 rewrite)
- [x] `L  69`: `test_cancellation_flag_and_atomic_turn_bumping` [Bad Code] (Queued untouched for Layer 2 rewrite)
- [x] `L 106`: `test_turn_id_filtering_for_stale_tasks` [Bad Code] (Queued untouched for Layer 2 rewrite)
- [x] `L 136`: `test_barge_in_cancellation_flow` [Bad Code] (Queued untouched for Layer 2 rewrite)

## `app/src-tauri/src/services/pipeline/tts_lifecycle.rs` (124 lines, 2 functions)

- [x] `L  15`: `warm_up_tts` [Bad Code] (Queued untouched for Layer 2 rewrite)
- [x] `L 119`: `cool_down_tts` [Bad Code] (Queued untouched for Layer 2 rewrite)

## `app/src-tauri/src/services/pipeline/types.rs` (83 lines, 1 functions)

- [x] `L  42`: `resolve_reference_audio` [Bad Code] (Queued untouched for Layer 2 rewrite)

## `app/src-tauri/src/services/ptt.rs` (521 lines, 7 functions)

- [x] `L  11`: `ptt_start` [Disaster] (Queued untouched for Layer 2 domain module decomposition)
- [x] `L 134`: `ptt_stop` [Disaster] (Queued untouched for Layer 2 domain module decomposition)
- [x] `L 244`: `ptt_cancel` [Disaster] (Queued untouched for Layer 2 domain module decomposition)
- [x] `L 315`: `handle_ptt_audio_sync` [Disaster] (Queued untouched for Layer 2 domain module decomposition)
- [x] `L 470`: `reset_ptt_state_inner` [Disaster] (Queued untouched for Layer 2 domain module decomposition)
- [x] `L 482`: `discard_ptt_hold_inner` [Disaster] (Queued untouched for Layer 2 domain module decomposition)
- [x] `L 495`: `test_ptt_state_reset_and_discard` [Disaster] (Queued untouched for Layer 2 domain module decomposition)

## `app/src-tauri/src/services/realtime/audio_bridge.rs` (144 lines, 8 functions)

- [x] `L  13`: `default` [Production]
- [x] `L  19`: `new` [Production]
- [x] `L  23`: `start` [Production]
- [x] `L  66`: `stop` [Production]
- [x] `L  70`: `get_sender` [Production]
- [x] `L  74`: `send_pcm` [Production]
- [x] `L 101`: `test_audio_bridge_non_blocking_drop` [Production]
- [x] `L 132`: `test_audio_bridge_closed_channel_safety` [Production]

## `app/src-tauri/src/services/realtime/engine.rs` (126 lines, 10 functions)

- [x] `L  20`: `new` [Production]
- [x] `L  33`: `start` [Production]
- [x] `L  63`: `stop` [Production]
- [x] `L  76`: `push_audio` [Production]
- [x] `L  80`: `get_audio_sender` [Production]
- [x] `L  84`: `barge_in` [Production]
- [x] `L  93`: `activity_start` [Production]
- [x] `L 102`: `activity_end` [Production]
- [x] `L 111`: `is_connected` [Production]
- [x] `L 119`: `last_activity_time` [Production]

## `app/src-tauri/src/services/realtime/mod.rs` (44 lines, 11 functions)

- [x] `L  21`: `kind` [Production]
- [x] `L  22`: `audio_config` [Production]
- [x] `L  23`: `connect` [Production]
- [x] `L  29`: `health_check` [Production]
- [x] `L  33`: `send_audio` [Production]
- [x] `L  34`: `cancel` [Production]
- [x] `L  35`: `disconnect` [Production]
- [x] `L  36`: `activity_start` [Production]
- [x] `L  37`: `activity_end` [Production]
- [x] `L  38`: `is_connected` [Production]
- [x] `L  41`: `last_activity_time` [Production]

## `app/src-tauri/src/services/realtime/playback_bridge.rs` (75 lines, 5 functions)

- [x] `L  11`: `default` [Production]
- [x] `L  17`: `new` [Production]
- [x] `L  21`: `start` [Production]
- [x] `L  68`: `stop` [Production]
- [x] `L  72`: `get_sender` [Production]

## `app/src-tauri/src/services/realtime/providers/deepgram_live.rs` (698 lines, 14 functions)

- [x] `L  32`: `new` [Production]
- [x] `L  46`: `kind` [Production]
- [x] `L  50`: `audio_config` [Production]
- [x] `L  59`: `connect` [Production]
- [x] `L 389`: `health_check` [Production]
- [x] `L 404`: `perform_handshake` [Production]
- [x] `L 578`: `send_audio` [Production]
- [x] `L 588`: `cancel` [Production]
- [x] `L 598`: `disconnect` [Production]
- [x] `L 605`: `activity_start` [Production]
- [x] `L 609`: `activity_end` [Production]
- [x] `L 613`: `is_connected` [Production]
- [x] `L 617`: `last_activity_time` [Production]
- [x] `L 622`: `handle_deepgram_server_message` [Production]

## `app/src-tauri/src/services/realtime/providers/gemini_live.rs` (945 lines, 14 functions)

- [x] `L  32`: `new` [Production]
- [x] `L  46`: `kind` [Production]
- [x] `L  50`: `audio_config` [Production]
- [x] `L  59`: `connect` [Production]
- [x] `L 497`: `health_check` [Production]
- [x] `L 509`: `perform_handshake` [Production]
- [x] `L 715`: `send_audio` [Production]
- [x] `L 725`: `cancel` [Production]
- [x] `L 735`: `disconnect` [Production]
- [x] `L 742`: `activity_start` [Production]
- [x] `L 752`: `activity_end` [Production]
- [x] `L 762`: `is_connected` [Production]
- [x] `L 766`: `last_activity_time` [Production]
- [x] `L 772`: `handle_gemini_server_message` [Production]

## `app/src-tauri/src/services/realtime/resampler.rs` (154 lines, 6 functions)

- [x] `L  17`: `new` [Production]
- [x] `L  51`: `process_i16` [Production]
- [x] `L 105`: `test_resampler_process_exact_sample_count` [Production]
- [x] `L 117`: `test_resampler_empty_input` [Production]
- [x] `L 124`: `test_resampler_boundary_and_chunk_buffering` [Production]
- [x] `L 146`: `test_resampler_downsampling` [Production]

## `app/src-tauri/src/services/stt/actor.rs` (233 lines, 5 functions)

- [x] `L  30`: `coalesce_partials` [Production] (Extracted helper)
- [x] `L  60`: `handle_partial_command` [Production] (Extracted helper)
- [x] `L 125`: `handle_final_command` [Production] (Extracted helper)
- [x] `L 185`: `drain_reset_stream` [Production] (Extracted helper)
- [x] `L 205`: `spawn_stt_worker` [Production]

## `app/src-tauri/src/services/stt/mod.rs` (29 lines, 3 functions)

- [x] `L  24`: `transcribe` [Production]
- [x] `L  26`: `transcribe_chunk` [Production]
- [x] `L  28`: `reset_state` [Production]

## `app/src-tauri/src/services/stt/nemotron_onnx.rs` (113 lines, 5 functions)

- [x] `L  12`: `new` [Production]
- [x] `L  32`: `transcribe_strides` [Production] (Extracted helper)
- [x] `L  65`: `transcribe` [Production]
- [x] `L  95`: `transcribe_chunk` [Production]
- [x] `L 108`: `reset_state` [Production]

## `app/src-tauri/src/services/stt/providers/embedded.rs` (141 lines, 7 functions)

- [x] `L  32`: `ensure_loaded` [Production]
- [x] `L  64`: `new` [Production]
- [x] `L  80`: `transcribe` [Production]
- [x] `L  92`: `transcribe_chunk` [Production]
- [x] `L 126`: `reset_state` [Production]
- [x] `L 133`: `health_check` [Production]
- [x] `L 138`: `kind` [Production]

## `app/src-tauri/src/services/stt/providers/mod.rs` (68 lines, 6 functions)

- [x] `L  34`: `transcribe` [Production]
- [x] `L  41`: `transcribe_chunk` [Production]
- [x] `L  44`: `reset_state` [Production]
- [x] `L  47`: `health_check` [Production]
- [x] `L  50`: `kind` [Production]
- [x] `L  56`: `create_stt_provider` [Production]

## `app/src-tauri/src/services/stt/qwen_onnx.rs` (130 lines, 5 functions)

- [x] `L  17`: `new` [Production]
- [x] `L  71`: `strip_cjk` [Production]
- [x] `L  86`: `transcribe` [Production]
- [x] `L 123`: `transcribe_chunk` [Production]
- [x] `L 127`: `reset_state` [Production]

## `app/src-tauri/src/services/translit.rs` (329 lines, 10 functions)

- [x] `L  16`: `new` [Production]
- [x] `L  74`: `encode_source_ids` [Production] (Extracted helper)
- [x] `L  92`: `decode_autoregressive` [Production] (Extracted helper)
- [x] `L 182`: `transliterate_word` [Production]
- [x] `L 225`: `init_transliteration_engine` [Production]
- [x] `L 247`: `unload_transliteration_engine` [Production]
- [x] `L 255`: `is_transliteration_engine_loaded` [Production]
- [x] `L 259`: `transliterate` [Production]
- [x] `L 294`: `test_transliteration_engine_uninitialized_fallback` [Production]
- [x] `L 300`: `test_transliteration_engine_local_models` [Production]

## `app/src-tauri/src/services/tts/actor.rs` (315 lines, 13 functions)

- [x] `L  31`: `spawn_tts_worker` [Production]
- [x] `L  90`: `new` [Production]
- [x] `L  97`: `push_str` [Production]
- [x] `L 103`: `flush` [Production]
- [x] `L 114`: `clear` [Production]
- [x] `L 119`: `buffer` [Production]
- [x] `L 124`: `is_empty` [Production]
- [x] `L 129`: `find_split_point` [Production] (Extracted helper)
- [x] `L 165`: `extract_chunks` [Production]
- [x] `L 211`: `is_abbreviation` [Production]
- [x] `L 246`: `test_tts_clause_chunker_abbreviations` [Production]
- [x] `L 269`: `test_tts_clause_chunker_punctuation` [Production]
- [x] `L 287`: `test_tts_turn_cancel_clears_buffer` [Production]

## `app/src-tauri/src/services/tts/providers/chatterbox.rs` (269 lines, 7 functions)

- [x] `L  54`: `new` [Production]
- [x] `L 139`: `apply_speed` [Production]
- [x] `L 162`: `set_quality_steps` [Production]
- [x] `L 168`: `set_speed` [Production]
- [x] `L 174`: `kind` [Production]
- [x] `L 178`: `health_check` [Production]
- [x] `L 183`: `synthesize_chunk` [Production]

## `app/src-tauri/src/services/tts/providers/chatterbox_remote.rs` (330 lines, 8 functions)

- [x] `L  32`: `new` [Production]
- [x] `L 118`: `apply_speed_stretch` [Production]
- [x] `L 145`: `stream_pcm_response` [Production] (Extracted helper)
- [x] `L 190`: `set_quality_steps` [Production]
- [x] `L 196`: `set_speed` [Production]
- [x] `L 202`: `kind` [Production]
- [x] `L 206`: `health_check` [Production]
- [x] `L 226`: `synthesize_chunk` [Production]

## `app/src-tauri/src/services/tts/providers/edge_tts.rs` (310 lines, 12 functions)

- [x] `L  25`: `get_trusted_client_token` [Production]
- [x] `L  39`: `generate_sec_ms_gec` [Production]
- [x] `L  59`: `resolve_full_voice_name` [Production]
- [x] `L  82`: `new` [Production]
- [x] `L  93`: `connect_edge_websocket` [Production] (Extracted helper)
- [x] `L 155`: `send_ssml_request` [Production] (Extracted helper)
- [x] `L 205`: `collect_mp3_payload` [Production] (Extracted helper)
- [x] `L 245`: `synthesize_chunk` [Production]
- [x] `L 299`: `set_speed` [Production]
- [x] `L 305`: `kind` [Production]
- [x] `L 310`: `health_check` [Production]
- [x] `L 325`: `test_edge_tts_synthesis` [Production]

## `app/src-tauri/src/services/tts/providers/mod.rs` (76 lines, 5 functions)

- [x] `L  54`: `synthesize_chunk` [Production]
- [x] `L  63`: `set_quality_steps` [Production]
- [x] `L  66`: `set_speed` [Production]
- [x] `L  69`: `kind` [Production]
- [x] `L  75`: `health_check` [Production]

## `app/src-tauri/src/services/tts/providers/supertonic.rs` (290 lines, 12 functions)

- [x] `L  39`: `new_lpf_11k` [Production]
- [x] `L  55`: `process` [Production]
- [x] `L  67`: `resample_44100_to_24000` [Production]
- [x] `L 104`: `new (AtomicF32)` [Production]
- [x] `L 110`: `load` [Production]
- [x] `L 114`: `store` [Production]
- [x] `L 126`: `new (TtsEngine)` [Production]
- [x] `L 172`: `set_quality_steps` [Production]
- [x] `L 179`: `set_speed` [Production]
- [x] `L 184`: `kind` [Production]
- [x] `L 188`: `health_check` [Production]
- [x] `L 193`: `synthesize_chunk` [Production]

## `app/src-tauri/src/services/utils.rs` (678 lines, 19 functions)

- [x] `L   4`: `ends_at_word_boundary` [Production]
- [x] `L  18`: `lerp` [Production]
- [x] `L  24`: `should_flush` [Production]
- [x] `L  64`: `count_words` [Production]
- [x] `L  69`: `is_devanagari` [Production]
- [x] `L  78`: `tokenize_devanagari_slices` [Production] (Extracted helper)
- [x] `L 115`: `transliterate_if_hi` [Production]
- [x] `L 155`: `to_friendly_hinglish` [Production]
- [x] `L 160`: `edit_distance` [Production]
- [x] `L 185`: `words_soft_match` [Production]
- [x] `L 205`: `is_soft_subslice` [Production]
- [x] `L 225`: `find_alignment_match` [Production] (Extracted helper)
- [x] `L 255`: `find_sequential_overlap` [Production] (Extracted helper)
- [x] `L 280`: `stitch_transcripts` [Production]
- [x] `L 325`: `init_paths_for_testing` [Production]
- [x] `L 330`: `test_hinglish_normalization` [Production]
- [x] `L 337`: `test_stitch_transcripts_overlap` [Production]
- [x] `L 519`: `test_translit_mixed_script_preservation` [Production]
- [x] `L 595`: `test_devanagari_matra_normalization` [Production]

## `app/src-tauri/src/services/vad/actor.rs` (662 lines, 9 functions)

- [x] `L  13`: `spawn_vad_actor` [Disaster] (Queued for Layer 2 rewrite per spec)
- [x] `L 534`: `new` [Disaster] (Queued for Layer 2 rewrite per spec)
- [x] `L 542`: `push` [Disaster] (Queued for Layer 2 rewrite per spec)
- [x] `L 550`: `clear` [Disaster] (Queued for Layer 2 rewrite per spec)
- [x] `L 554`: `as_slice` [Disaster] (Queued for Layer 2 rewrite per spec)
- [x] `L 560`: `calculate_rms` [Disaster] (Queued for Layer 2 rewrite per spec)
- [x] `L 568`: `is_above_noise_gate` [Disaster] (Queued for Layer 2 rewrite per spec)
- [x] `L 582`: `test_pre_roll_circular_buffer_cap` [Disaster] (Queued for Layer 2 rewrite per spec)
- [x] `L 618`: `test_noise_gate_rms_threshold` [Disaster] (Queued for Layer 2 rewrite per spec)

## `app/src-tauri/src/services/vad/earshot_vad.rs` (134 lines, 4 functions)

- [x] `L  36`: `new` [Production]
- [x] `L  57`: `update_threshold` [Production]
- [x] `L  73`: `flush` [Production]
- [x] `L  87`: `predict` [Production]

## `app/src-tauri/src/services/vad/mod.rs` (56 lines, 4 functions)

- [x] `L  13`: `predict (VadEngine)` [Production]
- [x] `L  26`: `predict (VadBackend)` [Production]
- [x] `L  39`: `update_threshold` [Production]
- [x] `L  50`: `flush` [Production]

## `app/src-tauri/src/services/vad/ten_onnx.rs` (70 lines, 5 functions)

- [x] `L  12`: `new` [Production]
- [x] `L  23`: `create_detector` [Production]
- [x] `L  54`: `update_detector` [Production]
- [x] `L  60`: `flush` [Production]
- [x] `L  66`: `predict` [Production]
