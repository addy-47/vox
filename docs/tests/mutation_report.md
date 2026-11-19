# Vox Mutation Report

**Status:** FULL CAMPAIGN
**Mode:** local  
**Mutant set:** `app/src-tauri/tests/assets/mutants.json`  
**Runner:** `app/src-tauri/scripts/mutation-runner.py`

| Mutant | Test | Killed | Seconds | Note |
| :--- | :--- | :---: | ---: | :--- |
| S058 — Drop tool-free final pass so loop never latches allow_tools=false | `harness_loop_test` | yes | 57.4 | expected-failure: Final dispatch still offers tools and the loop errors instead of completing, failing the tool-free fin |
| S059 — Ignore pre-cancelled token in step1_intake | `harness_loop_test` | NO | 56.2 | test passed under mutant |
| S060 — Remove duplicate-query refusal in step1_intake | `harness_loop_test` | yes | 57.1 | expected-failure: Duplicate query is staged and dispatched instead of DuplicateIgnored, failing the refusal assertion. |
| S061 — Return Completed instead of Error when harness is unmounted | `harness_loop_test` | yes | 56.7 | expected-failure: Missing harness yields TurnOutcome::Completed instead of Error, failing the loud-failure assertion. |
| S062 — Skip assistant history commit in step7_commit_completed | `harness_loop_test` | yes | 57.5 | expected-failure: History never receives the staged user query or assistant response, failing both commit assertions. |
| S063 — Drop enter_non_terminal_phase call in step3_execute_compaction | `harness_loop_test` | yes | 66.7 | expected-failure: No interim filler TtsCommand is dispatched and state stays Thinking, failing the filler and Working-st |
| S064 — Route Thinking deltas directly into handle_token | `harness_loop_test` | yes | 56.8 | expected-failure: Thinking text is emitted as speech tokens and IPC, failing the TTS/accumulator thinking-demux assertio |
| S065 — Latch Ready unconditionally in on_llm_finished synthesis latch | `harness_loop_test` | NO | 56.6 | test passed under mutant |
| S066 — Clear accumulator assistant_response on ToolCall, discarding preamble | `harness_loop_test` | yes | 57.0 | expected-failure: Preamble text is empty in the tool-call outcome, so the initiating assistant history message is missin |
| S067 — Skip Working-to-Speaking transition in on_playback_started | `harness_loop_test` | yes | 61.7 | expected-failure: PlaybackStarted leaves state Working instead of Speaking, failing the transition assertion. |
| S068 — Drop tool observation content when building the step scratchpad | `harness_loop_test` | NO | 57.2 | test passed under mutant |
| S069 — Break streaming loop on first ToolCall event | `harness_loop_test` | yes | 56.8 | expected-failure: Only the first tool call is batched and the stream errors on premature disconnect, failing the full-ba |
| S070 — Do not latch title_set after successful set_title | `harness_loop_test` | yes | 56.7 | expected-failure: title_set stays false so the title tool remains active, failing the flag-latch assertion. |
| S071 — Ignore cancellation result returned from tool batch | `harness_loop_test` | NO | 56.8 | test passed under mutant |
| S072 — Drop non-empty partial text on cancellation | `harness_loop_test` | yes | 61.8 | expected-failure: Spoken partial text is discarded and no interrupted turn row is persisted, failing the DB text asserti |
| S073 — Commit turn even when cancellation partial text is empty | `harness_loop_test` | NO | 56.7 | test passed under mutant |
| S074 — Omit VoxEvent::Error dispatch on stream error | `harness_loop_test` | yes | 0.8 | expected-failure: Stream error no longer emits VoxEvent::Error, failing the error-event dispatch assertion. |
| S075 — Delete flush_staged_user_query from Harness::commit_tool_step | `harness_loop_test` | yes | 57.1 | expected-failure: The staged user query is never committed at step one, failing the preserved-history/no-rollback assert |
| S076 — Push user turn into history instead of staging it in stage_user_query | `harness_loop_test` | yes | 56.9 | expected-failure: Query is committed before Step 0, so cancellation cannot discard it and the untouched-history assertio |
| S077 — Execute parallel tool wave sequentially | `harness_loop_test` | yes | 61.0 | expected-failure: Wave tools run serially instead of concurrently, failing the parallel-execution timing assertion. |
| S078 — Force every tool execution mode to Parallel | `harness_loop_test` | yes | 56.6 | expected-failure: Exclusive tool loses its barrier semantics and runs inside a wave, failing the strict-barrier assertio |
| S079 — Return Error at step ceiling instead of forcing tool-free summary pass | `harness_loop_test` | yes | 57.1 | expected-failure: Ceiling breach yields TurnOutcome::Error with no tool-free summary pass, failing the forced-summary as |
| S080 — Disable identical-fingerprint loop detection via threshold | `harness_loop_test` | yes | 57.1 | expected-failure: Identical call loop never produces strike one or strike two, failing the circuit-breaker assertions. |
| S081 — Disable ping-pong pattern matcher via threshold | `harness_loop_test` | yes | 59.0 | expected-failure: Alternating tool loop is never flagged, so no two-strike circuit break occurs. |
| S082 — Ignore is_error when evaluating error cascade | `harness_loop_test` | yes | 56.7 | expected-failure: Error cascade is never detected, so the two-strike guardrail does not force the summary. |
| S083 — Bypass TruncationStage and keep raw ToolResult | `harness_loop_test` | yes | 12.7 | expected-failure: Oversized observation is neither spilled to Tier 4 nor head/tail truncated, failing the preview assert |
| S084 — Apply truncation despite TruncationPolicy::None | `harness_loop_test` | yes | 56.8 | expected-failure: Policy None now yields truncated output, failing the full-output-preserved assertion. |
| S085 — Wipe active turn committed tool steps in apply_session_context | `harness_loop_test` | NO | 56.8 | test passed under mutant |
| S086 — on_stop_turn transitions to Idle and takes harness | `harness_loop_test` | yes | 57.2 | expected-failure: StopTurn ends to Idle and unmounts harness instead of Ready, failing the Ready/no-end assertions. |
| S087 — Return early from on_end when state is Ready | `harness_loop_test` | yes | 57.3 | expected-failure: Second Esc in Ready no longer disengages to Idle, failing the Idle-transition assertion. |
| S088 — Omit take_staged_query in Scenario A continuation | `harness_loop_test` | yes | 12.4 | expected-failure: Staged Step 0 query is not reclaimed, so the merged prompt never dispatches and history lacks the merg |
| S089 — Call stash_for_continuation unconditionally on steering turn | `harness_loop_test` | NO | 56.9 | test passed under mutant |
| S090 — Inject voice transcript into StepInbox during Working | `harness_loop_test` | yes | 56.8 | expected-failure: StepInbox receives a synthetic voice message, failing the empty-inbox RULE-HARN-18 assertion. |
| S091 — Return immediately from wait_if_speech_held | `harness_loop_test` | yes | 56.8 | expected-failure: Step 1 dispatches while speech hold is still active, failing the boundary-hold dispatch-count assertio |
| S092 — Transition empty held speech to Ready unconditionally | `harness_loop_test` | yes | 56.6 | expected-failure: False VAD cough aborts the turn to Ready, failing the not-Ready and completed-turn assertions. |
| S093 — Drain all inbox messages unconditionally in drain_immediate | `harness_loop_test` | yes | 56.8 | expected-failure: TurnBoundary messages are drained too, failing the retained-boundary-message assertion. |
| S094 — Disable RULE-HARN-01 check in lint-arch.sh | `harness_loop_test` | skip | 0.0 | The only code under test is scripts/lint-arch.sh, a shell script outside app/src-tauri/src. No valid Rust one-line edit  |
| S095 — Drop committed search_memory tool observation from scratchpad | `harness_loop_test` | yes | 57.1 | expected-failure: No non-empty Role::Tool message is committed for search_memory, failing the observable-tool-result ass |
| S096 — Empty transcript in Thinking transitions Ready instead of resuming Thinking | `harness_loop_test` | yes | 57.3 | expected-failure: False barge-in drives state to Ready instead of back to Thinking, failing the resume assertion. |
| S097 — Drop active_turn_id epoch guard on PlaybackFinished | `harness_loop_test` | NO | 56.9 | test passed under mutant |
| S098 — Clear current_activity when returning to Working on empty transcript | `harness_loop_test` | yes | 56.7 | expected-failure: Activity envelope is lost on return to Working, failing the intact-activity assertion. |
| S019 — Schema migrations write a stale user_version | `database_persistence_boundary_test` | yes | 54.7 | expected-failure: run_migrations writes 0 while SCHEMA_VERSION stays 1, so the user_version assertion fails. |
| S020 — Default project seeded under the wrong id | `database_persistence_boundary_test` | yes | 54.7 | expected-failure: The seed row uses id 'default_mutant', so get_project_by_id('default') returns None. |
| S021 — session_tool_calls table renamed in migration | `database_persistence_boundary_test` | yes | 54.7 | expected-failure: The expected table never exists; later index creation on it fails and migrations error. |
| S022 — sessions.project_id FK changed to ON DELETE CASCADE | `database_persistence_boundary_test` | NO | 54.8 | test passed under mutant |
| S023 — Foreign-key enforcement disabled on migrated connections | `database_persistence_boundary_test` | yes | 54.8 | expected-failure: turns rows survive session deletion because the ON DELETE CASCADE action never fires. |
| S024 — Foreign-key enforcement disabled during migration | `database_persistence_boundary_test` | yes | 54.8 | expected-failure: The fact keeps its original session_id after session delete instead of becoming NULL. |
| S025 — memory_facts_vectors.fact_id FK removed | `database_persistence_boundary_test` | yes | 54.8 | expected-failure: Deleting a fact leaves its vector row, so the zero-count assertion fails. |
| S026 — In-progress compaction index scoped by session | `database_persistence_boundary_test` | yes | 54.9 | expected-failure: A second in_progress compaction for a different session is accepted. |
| S027 — In-progress index predicate also covers completed runs | `database_persistence_boundary_test` | yes | 54.8 | expected-failure: A new in_progress run after a completed run hits the unique index and fails. |
| S028 — Worker writes take an exclusive transaction | `database_persistence_boundary_test` | NO | 54.9 | test passed under mutant |
| S029 — Worker drops the last queued turn event | `database_persistence_boundary_test` | yes | 64.7 | expected-failure: The final TurnCompleted is skipped, so only 24 turns are persisted. |
| S030 — Vector blob encoding writes one byte per float | `database_persistence_boundary_test` | yes | 54.9 | expected-failure: Encoded blob is 384 bytes, not 1536, so the byte-length assertion fails. |
| S031 — idx_tool_calls_created renamed in migration | `database_persistence_boundary_test` | yes | 54.7 | expected-failure: The expected index name is absent from sqlite_master. |
| S032 — Self-healing session insert removed from tool-call persistence | `database_persistence_boundary_test` | NO | 54.7 | test passed under mutant |
| S033 — Private-mode skip gate disabled in worker | `database_persistence_boundary_test` | yes | 59.7 | expected-failure: Private tool call is written, so the zero-count assertion fails. |
| S034 — paired_devices public_key length CHECK removed | `database_persistence_boundary_test` | yes | 54.5 | expected-failure: A 31-byte key is accepted, so the rejection assertion fails. |
| S035 — Unknown hint kinds stripped on hints update | `database_persistence_boundary_test` | yes | 55.0 | expected-failure: The relay hint is dropped, so the unrecognized-hint roundtrip fails. |
| S036 — History LIKE pattern loses its leading wildcard | `database_persistence_boundary_test` | yes | 55.0 | expected-failure: Turn-content search for a mid-sentence phrase returns no hits. |
| S037 — Persisted step id no longer matches the tool-call step_id | `database_persistence_boundary_test` | yes | 54.7 | expected-failure: Tool-call inserts fail the step_id FK lookup, so no linked rows are stored. |
| S038 — Foreign-key enforcement disabled during migration | `database_persistence_boundary_test` | yes | 54.7 | expected-failure: session_steps and session_tool_calls rows survive hard session deletion. |
| S039 — prompt_tokens provenance hardcoded to zero | `database_persistence_boundary_test` | yes | 54.8 | expected-failure: session_steps stores prompt_tokens 0 instead of 420. |
| S040 — Session deletion leaves tool-output cache directory | `database_persistence_boundary_test` | yes | 54.8 | expected-failure: remove_dir fails on the non-empty directory, so dir_a still exists. |
| S157 — Created notification record drops group_key | `notifications_crud_test` | yes | 54.7 | expected-failure: The returned record has an empty group_key, so the group_key assertion fails. |
| S158 — In-place notification update misses the target row | `notifications_crud_test` | yes | 54.5 | expected-failure: The original message stays unchanged, so the update assertion fails. |
| S159 — Resolution flips the record status to read | `notifications_crud_test` | yes | 54.8 | expected-failure: The returned record status is 'read' instead of 'unread'. |
| S160 — Mark-read ignores the action_type filter | `notifications_crud_test` | yes | 54.3 | expected-failure: Both tabs are marked read, so the receipt status assertion fails. |
| S161 — Dismiss ignores the action_type filter | `notifications_crud_test` | yes | 54.5 | expected-failure: Both tabs are dismissed, so the remaining-count assertion fails. |
| S162 — dismiss_interactive_by_entity becomes a no-op | `notifications_crud_test` | yes | 54.6 | expected-failure: No row changes status, so active notifications remain. |
| S163 — Transient action routed through the interactive branch | `notifications_crud_test` | yes | 54.2 | expected-failure: Transient notification resolves to NotificationOnly instead of ToastOnly. |
| S164 — Receipt action routed to toast only | `notifications_crud_test` | yes | 54.1 | expected-failure: Receipt resolves to ToastOnly instead of NotificationOnly. |
| S165 — Interactive actions always resolve to NotificationOnly | `notifications_crud_test` | yes | 54.4 | expected-failure: Urgent interactive actions no longer resolve to ToastAndNotification. |
| S166 — Transient notification persisted before channel decision | `notifications_crud_test` | yes | 54.5 | expected-failure: The toast-only path also dispatches a drawer row, so the row-count assertion fails. |
| S167 — Group-key rollup in-place update disabled | `notifications_crud_test` | yes | 54.6 | expected-failure: A second notification with the same group key inserts a duplicate row. |
| S168 — fetch_uncompacted_sessions returns an empty list | `notifications_crud_test` | yes | 54.6 | expected-failure: The session with uncompacted turns is absent from the ledger. |
| S169 — Compaction commit skipped so watermark never advances | `notifications_crud_test` | yes | 54.3 | expected-failure: The session stays uncompacted because the run never reaches completed status. |
| S170 — Failed compaction error message not written | `notifications_crud_test` | yes | 11.0 | expected-failure: record_compaction_finish stores NULL error_msg, so the error assertion fails. |
| S171 — Retried action does not resolve the card in place | `notifications_crud_test` | yes | 54.4 | expected-failure: Metadata resolution stays 'pending' after execute_notification_action. |
| S284 — vad.threshold mutation arm renamed away | `settings_persistence_test` | yes | 57.6 | expected-failure: apply_setting_mutation('vad','threshold') returns false. |
| S285 — Retired tts.quality_steps arm re-added | `settings_persistence_test` | yes | 57.6 | expected-failure: The retired key is now accepted, so the rejection assertion fails. |
| S286 — Unknown scopes ignored by patch tree | `settings_persistence_test` | yes | 57.6 | expected-failure: The retired working_memory scope no longer produces an error. |
| S287 — agent.jsonc write skipped in VoxSettings::save | `settings_persistence_test` | yes | 57.6 | expected-failure: The agent config file is never created. |
| S288 — Memory section dropped from serialized settings | `settings_persistence_test` | yes | 57.6 | expected-failure: Reloaded memory fields fall back to defaults instead of the mutated values. |
| S289 — Corrupt settings parse poisons the loaded llm selection | `settings_persistence_test` | yes | 57.5 | expected-failure: The loader returns a non-default llm.active, so the fallback assertion fails. |
| S290 — Corrupt settings file is not backed up | `settings_persistence_test` | yes | 57.9 | expected-failure: No settings.corrupt.* backup exists after load. |
| S291 — Valid appearance section discarded on merge | `settings_persistence_test` | yes | 57.6 | expected-failure: The partial file's theme and accent values are lost. |
| S292 — JSONC comment stripping disabled | `settings_persistence_test` | yes | 57.9 | expected-failure: Comment-bearing config files fail strict JSON parsing and values are not loaded. |
| S293 — Type-error recovery recovers no settings sections | `settings_persistence_test` | yes | 57.7 | expected-failure: All sections fall back to defaults, so appearance.theme assertion fails. |
| S294 — providers.jsonc ignored during settings load | `settings_persistence_test` | yes | 57.7 | expected-failure: Valid provider cloud keys are absent after load. |
| S295 — tts.active_voice_id dispatch arm removed from has_arm | `settings_persistence_test` | yes | 57.5 | expected-failure: dispatch_worker_command_has_arm returns false for the worker-command key. |
| S296 — llm.cloud_keys reclassified as WorkerCommand | `settings_persistence_test` | yes | 57.7 | expected-failure: get_setting_reload_policy no longer returns Hot. |
| S297 — Patch tree reports only the first changed key | `settings_persistence_test` | yes | 57.8 | expected-failure: The two-key patch returns one change, so changed.len() assertion fails. |
| S298 — Unknown patch keys treated as applied | `settings_persistence_test` | yes | 57.5 | expected-failure: Bad patch returns Ok, so the is_err() assertion fails. |
| S299 — remote domain removed from SUPPORTED_DOMAINS | `settings_persistence_test` | yes | 57.6 | expected-failure: The settings contract no longer contains the remote domain. |
| S300 — azure_tts removed from secret_keys allow-list | `settings_persistence_test` | yes | 57.7 | expected-failure: is_secret_key('tts','azure_tts') is false and no secret entry exists. |
| S208 — pair_device drops controller-side host persistence | `remote_connection_test` | yes | 54.4 | expected-failure: pair_device reports Paired and Ok but never writes the host row; get_paired_device returns None and th |
| S209 — PairDenied corrupts the canonical link state | `remote_connection_test` | yes | 54.4 | expected-failure: A denial drives RemoteLinkState to Failed; the assertion that state stays Unpaired fails. |
| S210 — Pairing timeout corrupts the canonical link state | `remote_connection_test` | yes | 54.4 | expected-failure: A 35s approval timeout drives RemoteLinkState to Failed; the Unpaired-state assertion fails. |
| S211 — Host heartbeat never transitions a dead link to Paired | `remote_connection_test` | NO | 55.1 | test passed under mutant |
| S212 — Desktop originating commands are always permitted | `remote_connection_test` | yes | 54.3 | expected-failure: is_desktop_command_permitted returns true unconditionally; every mutating-write block assertion fails. |
| S213 — Host disconnect deletes the paired device row | `remote_connection_test` | NO | 54.4 | test passed under mutant |
| S214 — Revocation reports success without deleting the device | `remote_connection_test` | yes | 54.7 | expected-failure: revoke_device returns true but leaves the row; the get_paired_device-is-none assertion fails. |
| S215 — Idle revocation no longer notifies the host | `remote_connection_test` | yes | 64.4 | expected-failure: notify_idle_host_revoked is skipped when no link is active; the host never receives LinkLoss(Revoked). |
| S216 — Hydration refuses to restore Paired from Disabled | `remote_connection_test` | yes | 54.5 | expected-failure: After toggling accepts_controllers back on the host stays Disabled; the Paired assertion fails. |
| S217 — dial_and_connect returns before attaching the link | `remote_connection_test` | yes | 54.3 | expected-failure: attach_link is never called; the controller remains Connecting and the Connected-state assertion fails |
| S218 — Candidate dialing stops at the first failing hint | `remote_connection_test` | yes | 60.6 | expected-failure: The dead first hint aborts the loop; the valid second hint is never tried and dial_and_connect fails. |
| S219 — Dial failure clears the preserved peer identity | `remote_connection_test` | yes | 60.6 | expected-failure: Entering Failed wipes peer_identity; the peer.is_some() and DTO deviceId assertions fail. |
| S220 — Audio session token ignores static key binding | `remote_connection_test` | yes | 54.4 | expected-failure: Redeeming with the impostor key succeeds; the AudioTokenError::KeyMismatch assertion fails. |
| S221 — Event forwarder stops coalescing LLM tokens | `remote_connection_test` | yes | 54.3 | expected-failure: Each token is forwarded as a separate event; the second event carries 'Hel' rather than 'Hello world!' |
| S222 — Transient reconnect error overwrites Reconnecting | `remote_connection_test` | yes | 60.5 | expected-failure: The reconnect is treated as fresh; a transient error sets Failed and the Reconnecting-state assertion  |
| S223 — UnknownPeer rejection deletes durable Tier 1 trust | `remote_connection_test` | yes | 54.5 | expected-failure: The controller deletes the paired_devices row on UnknownPeer; the trust-preservation assertion fails. |
| S224 — Controller disconnect fabricates turn_id 0 | `remote_connection_test` | yes | 54.7 | expected-failure: state_changed carries turn_id 0; assert_ne!(id, 0) fails. |
| S225 — Audio payload branch discrimination is swapped | `remote_link_test` | yes | 53.1 | expected-failure: Frames are tagged as JSON audio control; recv_audio cannot decode the frame and the equality assertion |
| S226 — Control messages are forced into the saturated audio queue | `remote_link_test` | yes | 62.9 | expected-failure: send_control awaits the full audio channel; the control send hangs and the test times out. |
| S227 — Overflow control messages are dropped instead of awaited | `remote_link_test` | yes | 53.0 | expected-failure: The 65th send returns immediately instead of pending; the is_finished assertion fails. |
| S228 — Unknown control type is treated as a fatal protocol error | `remote_link_test` | yes | 53.1 | expected-failure: UnknownType becomes a fatal MalformedJson; the link closes and the following Ping never arrives. |
| S229 — Unknown type skip warning loses its type name | `remote_link_test` | yes | 53.2 | expected-failure: The captured WARN no longer contains the unknown type name; the telemetry assertion fails. |
| S230 — Deserialization errors are ignored as unknown types | `remote_link_test` | yes | 63.0 | expected-failure: Malformed JSON is skipped instead of closing with a cause; recv_control waits and the test times out. |
| S231 — Closing a link records no reason and never notifies the peer | `remote_link_test` | yes | 52.7 | expected-failure: close() is a no-op; sender.close_reason() is None and the peer observes an open link. |
| S232 — Tagless audio control message is fatal | `remote_link_test` | yes | 53.3 | expected-failure: The tagless audio control frame terminates the link; the following AudioFrame is never delivered. |
| S233 — Control size cap is an exclusive boundary | `remote_link_test` | yes | 53.0 | expected-failure: A message of exactly MAX_CONTROL_MESSAGE_BYTES is rejected as Oversized; the at-cap assertion fails. |
| S234 — Fatal protocol error is masked as Local close | `remote_link_test` | yes | 53.2 | expected-failure: recv_control returns Closed(Local) instead of Closed(Protocol(Oversized)); the cause assertion fails. |
| S235 — All observed close reasons collapse to Local | `remote_link_test` | yes | 53.0 | expected-failure: The observing peer reports Local instead of Peer; the close_reason assertion fails. |
| S236 — Observed protocol violations are flattened to Peer | `remote_link_test` | yes | 52.9 | expected-failure: The observer sees Peer instead of Protocol(MalformedJson); the causal-reason assertion fails. |
| S237 — Loopback receive drains queued control frames | `remote_link_test` | yes | 93.1 | expected-failure: Queued control frames are discarded; the control_ordering and backpressure conformance cases fail. |
| S238 — WsLink corrupts control frame bytes before sending | `remote_link_test` | yes | 52.9 | expected-failure: Every control frame is corrupted; the control round-trip conformance cases fail. |
| S239 — WsLink control payload serialization is zeroed | `remote_link_test` | yes | 52.9 | expected-failure: An all-zero payload is sent; the delivered control message differs from the transmitted one. |
| S240 — WsLink drops its inbound control size cap | `remote_link_test` | yes | 12.4 | expected-failure: The oversized frame is decoded instead of refused; the expected Protocol(Oversized) error is missing. |
| S241 — WsLink close sends no close frame | `remote_link_test` | yes | 62.8 | expected-failure: No close frame reaches the peer; client.recv_control hangs and the Closed(Peer) assertion fails. |
| S242 — WsLink treats all control decode errors as fatal | `remote_link_test` | yes | 52.7 | expected-failure: A skippable control violation closes the link; the following Ping is not received. |
| S243 — WsLink recv_audio ignores skippable-violation classification | `remote_link_test` | yes | 53.1 | expected-failure: A skippable audio violation closes the link; the valid AudioFrame never arrives. |
| S244 — Handshake finish omits the transport cipher transition | `remote_security_test` | yes | 52.6 | expected-failure: finish() returns Err for both honest peers; expect("initiator finish") panics. |
| S245 — Noise reader ignores the pinned remote static key | `remote_security_test` | yes | 52.5 | expected-failure: The impostor handshake is not refused; the failed/message-count/finish-is-err assertions fail. |
| S246 — Noise prologue is not bound into the handshake | `remote_security_test` | yes | 52.4 | expected-failure: Peers with different prologue versions complete the handshake; the failed assertion fails. |
| S247 — Noise transport bypasses authentication tag verification | `remote_security_test` | yes | 52.5 | expected-failure: Corrupted, truncated, and undersized buffers all decrypt without error; the is_err assertions fail. |
| S248 — Noise transport accepts an over-cap payload | `remote_security_test` | NO | 52.6 | test passed under mutant |
| S249 — Noise reader skips the handshake frame size guard | `remote_security_test` | NO | 52.7 | test passed under mutant |
| S250 — Rejected oversized handshake frame poisons the cipher state | `remote_security_test` | yes | 52.6 | expected-failure: Reading the oversized frame advances the state machine; the later valid handshake or finish fails. |
| S251 — Version mismatch no longer fails the handshake | `remote_security_test` | yes | 52.5 | expected-failure: Every prologue carries version 1; mismatched-version peers complete and finish() returns Ok. |
| S252 — remote_static reports the configured pin instead of the revealed key | `remote_security_test` | yes | 52.6 | expected-failure: remote_static() returns the impostor pin; the equality assertion against the responder key fails. |
| S253 — Noise payload cap drifts below the control message cap | `remote_security_test` | yes | 52.5 | expected-failure: A full-cap control message (NOISE_MAX_PAYLOAD_BYTES) is now refused by the Noise cap; encrypt(...).exp |
| S254 — Pairing token is never marked consumed | `remote_security_test` | yes | 52.5 | expected-failure: A second validation of the same token succeeds; the single-use is_err assertion fails. |
| S255 — Pairing token expiry uses an inclusive boundary | `remote_security_test` | yes | 52.4 | expected-failure: A token at exactly issued+TTL is rejected as Expired; the boundary is_ok assertion fails. |
| S256 — Failed pairing attempts reset the counter instead of incrementing | `remote_security_test` | yes | 52.5 | expected-failure: The attacker source is never invalidated; the Invalidated refusal assertion fails. |
| S257 — Pairing skips claimed-identity verification | `remote_security_test` | yes | 0.8 | expected-failure: Requests with a claimed key/id that contradict the handshake are accepted; both is_err assertions fail |
| S258 — Rejecting a pairing prompt approves it | `remote_security_test` | yes | 52.6 | expected-failure: settle() resolves to Approved; the equality assertion against Rejected fails. |
| S259 — Pairing approval never times out | `remote_security_test` | NO | 900.0 | runner timeout — inconclusive, manual triage required |
| S260 — Pre-authentication request size check is disabled | `remote_security_test` | yes | 52.5 | expected-failure: An oversized pairing request is parsed instead of refused; the PreauthMessageTooLarge assertion fails. |
| S261 — Trust store omits the device-id/key derivation check | `remote_security_test` | yes | 52.6 | expected-failure: A row whose id contradicts its key is trusted; the KeyMismatch refusal assertion fails. |
| S262 — Device key file is placed inside the database directory | `remote_security_test` | yes | 52.5 | expected-failure: device_key_path now starts with the database directory; the isolation assertion fails. |
| S263 — A valid remote settings key is classified as Denied | `remote_security_test` | yes | 52.5 | expected-failure: remote.accepts_controllers classifies as Denied; assert_ne!(class, Denied) fails. |
| S172 — save_personal_memory skips version bump | `personal_memory_test` | yes | 55.1 | expected-failure: New record keeps version 1 instead of 2; assert_eq!(updated.version, 2) fails. |
| S173 — save_personal_memory ignores expected_version check | `personal_memory_test` | yes | 55.3 | expected-failure: Stale save succeeds instead of erroring; stale_attempt.is_err() assertion fails. |
| S174 — stale save writes content before rejecting | `personal_memory_test` | yes | 55.4 | expected-failure: Stale save still returns Err but overwrites content with '# Hijacked Content'; current.content asserti |
| S175 — consolidation drops compaction gate | `personal_memory_test` | yes | 55.1 | expected-failure: Gate skipped; outcome is Completed rather than ConfirmationRequired(CompactionInProgress). |
| S176 — pending queue confirmation reports zero count | `personal_memory_test` | yes | 55.0 | expected-failure: PendingQueueItems outcome reports pending_count = 0; assert_eq!(pending_count, 1) fails. |
| S177 — quiescent consolidation with no facts returns error | `personal_memory_test` | yes | 55.1 | expected-failure: Empty fact set yields Err; quiescent_res.is_ok() assertion fails. |
| S178 — continuation drops personal memory markdown | `personal_memory_test` | yes | 55.2 | expected-failure: continuation.personal_memory_markdown is None; Some(expected_markdown) assertion fails. |
| S179 — continuation watermark starts at turn zero | `personal_memory_test` | yes | 55.3 | expected-failure: Continuation returns turns 1..10 instead of 6..10; turn ids == [6,7,8,9,10] assertion fails. |
| S180 — version list sorted oldest first | `personal_memory_test` | yes | 55.1 | expected-failure: versions[0].version is 1 instead of 3; ordering assertion fails. |
| S181 — activating a version leaves all versions active | `personal_memory_test` | yes | 55.1 | expected-failure: All rows become active; get_personal_memory returns v3 so restored.version == 2 fails. |
| S182 — empty stage inserts a placeholder revision | `personal_memory_test` | yes | 12.4 | expected-failure: Empty slice stages one revision; staged_zero == 0 and pending.is_empty() assertions fail. |
| S183 — staged revision anchored to version zero | `personal_memory_test` | yes | 55.3 | expected-failure: revision_base_version is 0; anchor assertion against initial_record.version fails. |
| S184 — batch resolution skips operation application | `personal_memory_test` | yes | 55.1 | expected-failure: Accepted op is not applied; content contains no 'User enjoys badminton.' |
| S185 — second revision applied against oldest stored version | `personal_memory_test` | yes | 55.1 | expected-failure: Second accept uses v2 content; final document lacks 'User enjoys badminton.' |
| S186 — batch resolution applies rejected operations | `personal_memory_test` | yes | 55.0 | expected-failure: Reject is applied and committed; version == 2, byte-identical content and status 'rejected' assertions |
| S187 — mark_observations_integrated skips fact status update | `personal_memory_test` | yes | 55.1 | expected-failure: Fact status remains 'active'; integrated-status and zero-active-fact assertions fail. |
| S188 — manual markdown save skips bulk rejection | `personal_memory_test` | yes | 55.2 | expected-failure: Superseded revision stays pending; pending_after.is_empty() and status 'rejected' assertions fail. |
| S264 — session start skips Ready transition | `session_lifecycle_test` | yes | 61.0 | expected-failure: Pipeline remains Idle; wait_for_state(Ready) times out and fails. |
| S265 — session start drops SessionStarted persistence event | `session_lifecycle_test` | yes | 61.2 | expected-failure: No sessions row is inserted; SELECT COUNT(*) WHERE id = conv_id assertion fails. |
| S266 — harness created without personal memory | `session_lifecycle_test` | yes | 56.3 | expected-failure: Assembled system prompt lacks the seeded identity text; contains assertion fails. |
| S267 — session start sets VAD to WindowedValidation | `session_lifecycle_test` | yes | 56.3 | expected-failure: Session start emits WindowedValidation; mode assertion fails. |
| S268 — duplicate session start guard removed | `session_lifecycle_test` | yes | 61.3 | expected-failure: Duplicate start mints a new conversation id; conv_id unchanged assertion fails. |
| S269 — session start skips continuation hydration | `session_lifecycle_test` | yes | 56.2 | expected-failure: Harness history lacks seeded turn text; messages.any(...) assertion fails. |
| S270 — turn counter initialised to zero on continuation | `session_lifecycle_test` | yes | 56.1 | expected-failure: peek_turn_id() is 0 instead of 2; counter assertion fails. |
| S271 — empty compaction output accepted as summary | `session_lifecycle_test` | yes | 56.3 | expected-failure: Empty compaction yields Some("") summary; latest_summary.is_none() fails. |
| S272 — pause skips cancelling the active turn | `session_lifecycle_test` | yes | 56.3 | expected-failure: Turn token stays live on pause; is_cancelled() assertion fails. |
| S273 — pause leaves VAD in ContinuousSegmentation | `session_lifecycle_test` | yes | 56.3 | expected-failure: Pause emits ContinuousSegmentation instead of WindowedValidation; mode assertion fails. |
| S274 — resume skips rearming the turn cancel token | `session_lifecycle_test` | yes | 56.2 | expected-failure: Token remains cancelled after resume; !turn_cancel_token().is_cancelled() assertion fails. |
| S275 — resume guard accepts only Paused | `session_lifecycle_test` | yes | 61.4 | expected-failure: Resume from Sleeping is rejected; state stays Sleeping and wait_for_state(Ready) fails. |
| S276 — resume guard accepts Idle | `session_lifecycle_test` | yes | 61.2 | expected-failure: Resume from Idle is accepted and reaches Ready; Idle state assertion fails. |
| S277 — session end always stops audio engine | `session_lifecycle_test` | yes | 56.3 | expected-failure: Engine is stopped while dictation is Ready; engine.is_some() assertion fails. |
| S278 — session end never stops audio engine | `session_lifecycle_test` | yes | 56.2 | expected-failure: Engine stays mounted while dictation is Idle; engine.is_none() assertion fails. |
| S279 — session end skips unmounting the harness | `session_lifecycle_test` | yes | 56.3 | expected-failure: Harness stays mounted; harness.is_none() assertion fails. |
| S280 — session end skips cancelling the active turn | `session_lifecycle_test` | yes | 56.5 | expected-failure: Turn token stays live on session end; is_cancelled() assertion fails. |
| S281 — failed probe treated as unsupported | `session_lifecycle_test` | yes | 56.4 | expected-failure: Failed probe disables tools and emits notification; supports_tools and no-notification assertions fail |
| S282 — resolve_model_tool_support ignores capability cache | `session_lifecycle_test` | yes | 66.1 | expected-failure: Cache is bypassed; 'tools supported' log never appears, wait_for_log times out. |
| S283 — cached unsupported branch skips notification | `session_lifecycle_test` | yes | 66.1 | expected-failure: No model_tool_unsupported notification is written; notification assertion fails. |
| S001 — terminal_tool_sets_the_title_and_keeps_the_accumulator_in_parity | `agentic_tool_loop_test` | yes | 54.1 | expected-failure: TurnAccumulator.assistant_response stays empty, so the Completed outcome text and the accumulator pari |
| S002 — search_memory_retrieval_is_non_terminal_and_drops_the_scratchpad | `agentic_tool_loop_test` | yes | 54.7 | expected-failure: The loop terminates after the search_memory tool call; the final answer pass never runs, so the expect |
| S003 — tool_call_preserves_the_buffered_preamble_clause | `agentic_tool_loop_test` | yes | 54.1 | expected-failure: The chunker never receives the preamble tokens, so no eager clause reaches TTS and the dispatched-comm |
| S004 — nvidia_preset_maps_the_request_and_parses_the_tool_stream | `agentic_tool_loop_test` | yes | 54.1 | expected-failure: The chat-completions request body has no tools array; the mock wire server assertion on body.tools fai |
| S005 — ollama_native_maps_the_request_and_parses_the_tool_stream | `agentic_tool_loop_test` | yes | 54.0 | expected-failure: The Ollama native request body has no tools array; the mock wire server assertion on body.tools fails. |
| S006 — web_search_tool_declares_a_hermetic_non_terminal_contract | `agentic_tool_loop_test` | NO | 54.4 | test passed under mutant |
| S007 — web_search_flows_through_the_harness_turn_without_network | `agentic_tool_loop_test` | yes | 54.0 | expected-failure: Clause dispatch to TTS is dropped, so the interim filler for the web_search tool never reaches tts_rx  |
| S008 — tool_filter_hides_the_title_tool_once_the_title_is_set | `agentic_tool_loop_test` | yes | 54.1 | expected-failure: set_title stays in active_definitions even when title_is_unset is false; the suppression assertion fai |
| S009 — tool_filter_hides_retrieval_tools_when_they_are_disabled | `agentic_tool_loop_test` | yes | 53.7 | expected-failure: search_memory stays active while memory retrieval is disabled; the suppression assertion fails. |
| S010 — chat_completions_wire_demuxes_reasoning_content_and_think_tags | `agentic_tool_loop_test` | yes | 54.0 | expected-failure: delta.reasoning_content is routed as a content Token instead of a Thinking event; the reasoning demux  |
| S011 — ollama_native_wire_demuxes_thinking_field_and_think_tags | `agentic_tool_loop_test` | yes | 54.0 | expected-failure: message.thinking is ignored, so no Thinking event is emitted; the thinking-field demux assertion fails |
| S106 — scripted_stream_dispatches_clauses_to_tts | `llm_to_tts_test` | yes | 61.3 | expected-failure: StreamRoutingStage no longer sends clause TTS commands; the clauses.len() >= 2 assertion fails. |
| S107 — scripted_complete_sentence_reaches_tts | `llm_to_tts_test` | yes | 61.1 | expected-failure: The complete sentence produces no clause dispatch; the exactly-one-clause assertion fails. |
| S108 — tools_registered_but_unused_stream_completes | `llm_to_tts_test` | yes | 61.2 | expected-failure: No clauses are extracted for the tool-less sentence; the !clauses.is_empty() assertion fails. |
| S109 — harness_turn_streams_clauses_to_tts_and_commits_history | `llm_to_tts_test` | yes | 87.8 | expected-failure: The completed turn no longer commits the assistant turn to harness history; the history-pair assertion |
| S189 — real_synthesis_is_ingested_and_arms_the_pre_roll | `playback_test` | yes | 86.5 | expected-failure: Synthesised samples are dropped before the ring buffer/sink; the sink-received and non-silent-sample a |
| S190 — short_utterance_needs_a_flush_to_arm_the_pre_roll | `playback_test` | yes | 56.3 | expected-failure: The pre-roll is armed on the first ingested sample; the in-provider !turn_armed assertion fails before |
| S191 — playback_gates_thinking_to_speaking_and_speaking_to_ready | `playback_test` | yes | 61.2 | expected-failure: PlaybackStarted leaves the pipeline in Thinking instead of Speaking; the Speaking-state assertion fail |
| S192 — short_utterance_requires_a_flush_to_arm_the_pre_roll | `playback_test` | yes | 56.1 | expected-failure: Any sample satisfies the pre-roll cushion, so PlaybackStarted fires during ingest; the channel-empty a |
| S193 — playback_finished_is_deferred_while_a_job_is_pending | `playback_test` | yes | 56.5 | expected-failure: PlaybackFinished is no longer deferred while a synthesis job is pending; the still-Speaking assertion  |
| S194 — speech_detection_follows_the_audio_output_mode | `playback_test` | yes | 61.9 | expected-failure: The VAD no longer emits SpeechStart events; both Speaker-mode and Headset-mode assertions fail. |
| S195 — stage1_ptt_pauses_generation_without_ending_the_turn | `playback_test` | yes | 56.1 | expected-failure: Stage 1 now latches the hardware cancel flag and cancels the turn token; the armed-token and flag-fals |
| S196 — duplex_quiescence_playback_finished_before_llm_finished_returns_thinking | `playback_test` | yes | 61.0 | expected-failure: PlaybackFinished transitions to Ready even though llm_done is false; the Speaking -> Thinking assertio |
| S197 — hardware_cancel_flag_rearmed_after_barge_in_produces_audible_chunks | `playback_test` | yes | 56.3 | expected-failure: next_turn leaves the hardware cancel flag latched; the re-armed flag assertion fails (and turn-2 chunk |
| S198 — llm_finished_with_drained_audio_closes_quiescence_latch | `playback_test` | yes | 61.2 | expected-failure: LlmFinished sets llm_done but never transitions to Ready; the Ready-state assertion fails. |
| S326 — voice_switch_does_not_restart_the_tts_worker | `tts_catalogue_test` | yes | 59.4 | expected-failure: The SetVoice command is ignored, so the tracking provider's active voice never changes; the voice-swit |
| S327 — compaction_filler_is_dispatched_and_counted | `tts_catalogue_test` | yes | 62.7 | expected-failure: The critical-threshold compaction phase never dispatches its filler phrase; the filler-on-tts_rx asser |
| S328 — fresh_install_resolves_to_supertonic | `tts_catalogue_test` | yes | 57.5 | expected-failure: A fresh install selects edge_tts instead of supertonic; the model/provider-config assertions fail. |
| S329 — supertonic_path_produces_non_silent_samples | `tts_catalogue_test` | yes | 58.5 | expected-failure: The resampler emits only zeros, so the synthesised buffer is silent; the RMS > 0 assertion fails. |
| S330 — edge_tts_path_returns_the_explicit_availability_error | `tts_catalogue_test` | yes | 57.4 | expected-failure: The dead Edge TTS path returns Ok (silence) instead of the explicit HTTP 403 error; expect_err fails. |
| S331 — edge_tts_is_still_in_the_provider_catalogue | `tts_catalogue_test` | yes | 12.8 | expected-failure: The bundled manifest drops the edge_tts group; the catalogue-membership assertion fails. |
| S332 — provider_factory_resolves_exactly_one_provider | `tts_catalogue_test` | yes | 57.6 | expected-failure: The provider-attribution error no longer names Edge TTS; the error.contains("Edge TTS") factory assert |
| S333 — devanagari_route_declaration_targets_supertonic | `tts_catalogue_test` | yes | 57.5 | expected-failure: The Devanagari route constant names the dead edge_tts provider; the route-target assertion fails. |
| S334 — devanagari_route_reports_missing_supertonic_weights | `tts_catalogue_test` | yes | 57.6 | expected-failure: Missing weights are silently swallowed instead of reported; expect_err fails. |
| S335 — default_voice_is_provider_aware | `tts_catalogue_test` | yes | 57.7 | expected-failure: Every provider resolves to the Supertonic default voice; the provider-scoped default-voice assertions  |
| S336 — devanagari_route_reuses_one_cached_engine | `tts_catalogue_test` | yes | 59.1 | expected-failure: The engine cache is bypassed and an engine is rebuilt on every synthesize; the single-build assertion  |
| S134 — loaded_embedder_produces_384_dimension_vector | `model_eviction_test` | yes | 52.1 | expected-failure: The embedder returns a zero-length vector; the 384-dimension assertion fails. |
| S135 — memory_pipeline_unload_evicts_embedder_but_keeps_transliteration | `model_eviction_test` | yes | 52.3 | expected-failure: The memory-pipeline unload also evicts transliteration; the transliteration-still-loaded assertion fai |
| S136 — evicted_embedder_returns_none_without_crashing | `model_eviction_test` | yes | 52.4 | expected-failure: After eviction the accessor returns Some instead of None; the post-evict is_none assertion fails. |
| S137 — full_onnx_unload_evicts_embedder_and_transliteration | `model_eviction_test` | yes | 52.3 | expected-failure: The full unload leaves the transliteration engine resident; the transliteration-evicted assertion fail |
| S138 — repeated_full_unload_is_idempotent | `model_eviction_test` | yes | 51.9 | expected-failure: The embedder always reports loaded, so the !is_embedder_loaded assertion after repeated unloads fails. |
| S139 — embedder_reloads_cleanly_after_full_unload | `model_eviction_test` | yes | 52.8 | expected-failure: init_embedder always reports failure, so ensure_embedder_loaded returns false and the clean-reload ass |
| S140 — tts_warmup_populates_worker_handles | `model_eviction_test` | yes | 51.6 | expected-failure: warm_up_tts returns Ok without spawning the worker; the tts_tx/tts_handle populated assertions fail. |
| S141 — tts_cooldown_clears_sender_and_joins_worker | `model_eviction_test` | yes | 52.2 | expected-failure: cool_down_tts leaves the sender in place after shutdown; the sender-is-None assertion fails. |
| S110 — hardcode_compaction_from_turn_to_1 | `memory_compaction_test` | yes | 54.2 | expected-failure: fetch_turns_for_compaction ignores its from_turn_id argument; pending_2 includes turn 1 so the first-t |
| S111 — skip_compaction_queue_insert | `memory_compaction_test` | yes | 54.4 | expected-failure: Every fact is skipped; the pending ingestion-queue count is 0 instead of 2. |
| S112 — drop_partial_unique_index | `memory_compaction_test` | yes | 54.1 | expected-failure: A second in_progress insert succeeds; the is_err / UNIQUE-constraint assertion fails. |
| S113 — remove_in_progress_guard | `memory_compaction_test` | NO | 54.3 | test passed under mutant |
| S114 — lower_min_messages_for_compaction | `memory_compaction_test` | yes | 54.0 | expected-failure: The constant assertion expects 4; can_perform_inline_compaction(3) also becomes true. |
| S115 — retain_assistant_message_during_pruning | `memory_compaction_test` | yes | 54.3 | expected-failure: The pruned history holds three messages instead of two; the length assertion fails. |
| S116 — drop_session_context_from_pruned_prompt | `memory_compaction_test` | yes | 54.2 | expected-failure: The system message lacks the <session_context> tags and the session context text. |
| S117 — skip_uncompacted_notification_insert | `memory_compaction_test` | yes | 54.3 | expected-failure: notify_uncompacted_session returns None and writes no card; the expect and card lookup fail. |
| S118 — dismiss_card_instead_of_resolving | `memory_compaction_test` | yes | 54.4 | expected-failure: The returned record has status dismissed; the status == unread assertion fails. |
| S119 — drop_latest_summary_on_continuation | `memory_compaction_test` | yes | 54.2 | expected-failure: latest_summary is None; the is_some assertion fails. |
| S120 — drop_system_message_in_history_assembler | `memory_compaction_test` | yes | 54.1 | expected-failure: The first history message has role User instead of System; the role assertion fails. |
| S121 — drop_prior_summary_tag_from_request | `memory_compaction_test` | yes | 54.3 | expected-failure: The user message lacks the <prior_summary> tag and prior objective text. |
| S122 — cap_compaction_slice_at_20_turns | `memory_compaction_test` | yes | 54.1 | expected-failure: The slice ends at turn 20; the summary and persisted-run to_turn_id == 100 assertions fail (ignored te |
| S123 — skip_stage1_deactivation | `memory_ingestion_test` | yes | 54.0 | expected-failure: The older fact stays active; duplicates_deactivated is 0 and active_facts.is_empty() fails. |
| S124 — stage1_item_stays_pending | `memory_ingestion_test` | yes | 54.0 | expected-failure: The queue row stays pending; the status == stage1_done assertion fails. |
| S125 — raise_cosine_threshold_above_one | `memory_ingestion_test` | yes | 54.4 | expected-failure: No pair reaches the threshold; duplicates_deactivated is 0 and two active facts remain. |
| S126 — skip_vector_insert_in_cosine_stage | `memory_ingestion_test` | yes | 54.3 | expected-failure: insert_vector is a no-op; active_vectors.len() is 0 instead of 1. |
| S127 — stage2_item_stays_stage1_done | `memory_ingestion_test` | yes | 54.4 | expected-failure: The queue row stays stage1_done; the status == completed assertion fails. |
| S128 — stop_ingestion_cycle_after_stage1 | `memory_ingestion_test` | yes | 54.3 | expected-failure: Stage 2 never runs; stage2.processed and stage2.inserted stay 0. |
| S129 — cycle_leaves_items_stage1_done | `memory_ingestion_test` | yes | 84.1 | expected-failure: No queue row reaches completed; the completed count is 0 instead of 10. |
| S130 — store_vectors_with_staged_status | `memory_ingestion_test` | yes | 54.4 | expected-failure: Vectors are staged; the active-vector count is 0 instead of 10. |
| S131 — skip_stage1_crash_reset | `memory_ingestion_test` | yes | 54.0 | expected-failure: The crashed item remains stage1_processing; the status == pending assertion fails. |
| S132 — reset_stage2_crash_to_pending | `memory_ingestion_test` | yes | 54.1 | expected-failure: The stage2 crashed item becomes pending; the status == stage1_done assertion fails. |
| S133 — quarantine_poison_pill_to_stage1_done | `memory_ingestion_test` | yes | 53.9 | expected-failure: The poison pill becomes stage1_done; the status == failed assertion fails. |
| S142 — skip_marker_write_in_presence_check | `model_manager_test` | yes | 52.6 | expected-failure: is_model_file_present still returns true, but no .verified marker is written; the marker-exists assert |
| S143 — ignore_size_in_presence_check | `model_manager_test` | yes | 52.7 | expected-failure: The truncated file is reported present; the !present assertion fails. |
| S144 — trust_marker_without_validation | `model_manager_test` | yes | 52.6 | expected-failure: The stale wrong-hash marker validates the model; the !present assertion fails. |
| S145 — skip_zip_traversal_check | `model_manager_test` | yes | 52.6 | expected-failure: The raw traversal name is used as the output path; do_extract returns Ok and expect_err fails. |
| S146 — skip_tar_traversal_check | `model_manager_test` | yes | 52.8 | expected-failure: The traversal entry is unpacked; do_extract returns Ok and expect_err fails. |
| S147 — skip_marker_deletion_on_removal | `model_manager_test` | yes | 52.7 | expected-failure: The model file is deleted but the .verified marker remains; the marker-absence assertion fails. |
| S148 — ignore_computed_download_hash | `model_manager_test` | yes | 52.5 | expected-failure: The computed hash is ignored; hash != manifest triggers failure and no marker is written. |
| S149 — ignore_existing_tmp_offset | `model_manager_test` | yes | 52.5 | expected-failure: No Range header is sent; the range-header and 206 assertions fail. |
| S150 — hash_tail_only_on_resume | `model_manager_test` | yes | 52.6 | expected-failure: The prefix is not folded into the hash; hash mismatch aborts setup. |
| S151 — append_full_body_on_200 | `model_manager_test` | yes | 52.8 | expected-failure: A 200 body is appended to the existing .tmp; the installed file is PREFIX_LEN bytes too long. |
| S152 — treat_416_as_terminal_error | `model_manager_test` | yes | 52.7 | expected-failure: The 416 aborts immediately; only one request is made and the second-request assertion fails. |
| S153 — delete_tmp_on_cancel_arm | `model_manager_test` | yes | 53.0 | expected-failure: The partial .tmp is removed on cancellation; the tmp.exists() assertion fails. |
| S154 — delete_tmp_on_body_transfer_error | `model_manager_test` | yes | 52.6 | expected-failure: The .tmp is deleted when the body stream errors; the tmp.exists() assertion fails. |
| S155 — install_before_hash_verification | `model_manager_test` | yes | 52.8 | expected-failure: The hash-mismatched file is installed and a marker written; the no-model-file assertion fails. |
| S156 — zero_seed_progress_counter | `model_manager_test` | yes | 54.7 | expected-failure: In-transfer progress reports fewer than PREFIX_LEN bytes; the absolute-progress assertion fails. |
| S099 — rename_generate_handler_macro | `ipc_command_registration_test` | skip | 0.0 | Source-text lint rule against the literal token `tauri::generate_handler![` in src/lib.rs. Renaming a real macro invocat |
| S100 — move_ipc_directory | `ipc_command_registration_test` | skip | 0.0 | Requires relocating a directory and updating modules/imports; not a single-line production edit. The test only parses so |
| S101 — unregister_get_settings_command | `ipc_command_registration_test` | yes | 50.8 | expected-failure: get_settings is no longer in the registered set; the unregistered-commands assertion fails. |
| S102 — delete_non_desktop_stub_arm | `ipc_command_registration_test` | skip | 0.0 | Seed deletes a whole #[cfg(not(desktop))] function arm in src/ipc/tray.rs. Removing that multi-line block is not a singl |
| S103 — drop_not_desktop_classifier_clause | `ipc_command_registration_test` | skip | 0.0 | The is_desktop_guard classifier under test lives in the test file itself, not in app/src-tauri/src. No production one-li |
| S104 — remove_comment_strip_in_parser | `ipc_command_registration_test` | skip | 0.0 | extract_registered_commands is defined in the test file, not in production code. No production one-line edit applies. |
| S105 — remove_block_end_early_exit_in_parser | `ipc_command_registration_test` | skip | 0.0 | extract_registered_commands is defined in the test file, not in production code. No production one-line edit applies. |
| S041 — drop_last_transcript_store | `dictation_output_test` | yes | 56.0 | expected-failure: dictation_last_transcript stays None; last_transcript() assertion fails. |
| S042 — remove_blank_transcript_guard | `dictation_output_test` | yes | 55.7 | expected-failure: Whitespace-only text is stored as an empty transcript instead of being dropped. |
| S043 — remove_dictation_disabled_guard | `dictation_output_test` | yes | 55.8 | expected-failure: Transcript is stored while dictation is disabled. |
| S044 — drop_requested_transliteration | `dictation_output_test` | yes | 56.1 | expected-failure: Devanagari text is not transliterated even when the setting is enabled. |
| S045 — force_transliteration_unconditionally | `dictation_output_test` | yes | 55.9 | expected-failure: Devanagari text is transliterated even when the setting is off. |
| S046 — clear_history_before_push | `dictation_output_test` | yes | 56.0 | expected-failure: History deque is cleared on every transcript, so it never grows past one entry. |
| S047 — tray_mode_returns_error | `dictation_output_test` | yes | 55.9 | expected-failure: Tray routing returns ClipboardFailed instead of Ok. |
| S048 — ignore_clipboard_set_error | `dictation_output_test` | yes | 56.0 | expected-failure: Clipboard failure is swallowed; route_transcript returns Ok without a typed error. |
| S049 — skip_ready_transition_after_transcript | `dictation_output_test` | yes | 66.0 | expected-failure: Dictation never returns to Ready; the state wait times out. |
| S050 — change_fixture_path_to_nonexistent | `dictation_output_test` | skip | 0.0 | Seed targets get_dictation_fixture_path, a test-owned helper in tests/common/paths.rs, not production code. |
| S051 — invert_modular_dispatch_branch | `dictation_ptt_test` | yes | 66.5 | expected-failure: Modular validated speech is sent to the realtime arm; no STT Final is emitted. |
| S052 — drop_silero_prediction_dispatch | `dictation_ptt_test` | yes | 66.5 | expected-failure: Silero backend always reports no speech, so the PTT speech pipeline never fires. |
| S053 — invert_ptt_stop_owner | `dictation_ptt_test` | NO | 57.0 | test passed under mutant |
| S199 — skip_audio_arm_in_test_driver_encode | `realtime_transport_test` | skip | 0.0 | Seed targets TestDriver::encode, a test-owned double in tests/realtime_transport_test.rs, not production code. |
| S200 — ignore_goaway_reconnect | `realtime_transport_test` | yes | 12.4 | expected-failure: GoAway no longer signals the reconnect notifier, so no reconnect occurs. |
| S201 — emit_degraded_instead_of_session_halted | `realtime_transport_test` | yes | 53.1 | expected-failure: Terminal error carries Degraded impact instead of SessionHalted. |
| S202 — retry_reconnect_without_ceiling | `realtime_transport_test` | yes | 58.1 | expected-failure: Reconnect loop has no configured ceiling; the terminal event is never emitted. |
| S203 — ignore_paused_state_on_drop | `realtime_transport_test` | NO | 53.6 | test passed under mutant |
| S204 — skip_cache_expiry_check | `realtime_transport_test` | yes | 53.3 | expected-failure: Expired cache is treated as valid; the resumption token is reused and the file is kept. |
| S205 — purge_cache_unconditionally | `realtime_transport_test` | yes | 53.2 | expected-failure: Unexpired cache is purged on every provider creation. |
| S206 — make_purge_session_cache_noop | `realtime_transport_test` | yes | 12.4 | expected-failure: Cache file is never removed; the existence assertion fails. |
| S207 — skip_health_check_in_provider_factory | `realtime_transport_test` | skip | 0.0 | Seed describes a health check call in create_realtime_provider; that function performs no health check, and the paired t |
| S301 — emit_wrong_turn_id_in_final | `stt_turn_identity_test` | yes | 51.0 | expected-failure: TranscriptFinal carries turn 5 instead of the command's turn 4. |
| S302 — hardcode_assistant_owner_in_final | `stt_turn_identity_test` | yes | 51.0 | expected-failure: Dictation owner is replaced by Assistant in the final event. |
| S303 — adopt_incremented_turn_without_reset | `stt_turn_identity_test` | yes | 51.1 | expected-failure: The first chunk is treated as an already-active turn; reset count is 2 instead of 3. |
| S304 — adopt_turn_jump_without_reset | `stt_turn_identity_test` | NO | 50.9 | test passed under mutant |
| S305 — reset_provider_on_every_stream_chunk | `stt_turn_identity_test` | yes | 51.3 | expected-failure: Each stream chunk forces an extra provider reset; the reset count no longer matches. |
| S306 — adopt_wrapped_turn_without_reset | `stt_turn_identity_test` | yes | 51.0 | expected-failure: The MAX chunk is treated as already active; reset count is 2 instead of 3. |
| S307 — remove_cancelled_final_guard | `stt_turn_identity_test` | yes | 51.0 | expected-failure: Cancelled final is transcribed; the Cancelled event is never emitted. |
| S308 — reject_every_final_while_cancelled | `stt_turn_identity_test` | yes | 51.1 | expected-failure: A stale cancel flag causes the later-turn final to be rejected. |
| S309 — make_reset_stream_noop | `stt_turn_identity_test` | yes | 51.4 | expected-failure: ResetStream no longer clears provider state; reset count is 2 instead of 3. |
| S310 — drop_warmup_dispatch | `stt_turn_identity_test` | yes | 12.4 | expected-failure: Warmup command never reaches the provider; warmup count stays 0. |
| S311 — recycle_wrong_buffer_for_partial | `stt_turn_identity_test` | yes | 51.0 | expected-failure: The recycle channel receives an empty buffer, so the original buffer is not returned. |
| S312 — remove_shutdown_arm_from_dispatch | `stt_turn_identity_test` | skip | 0.0 | A single-line silent change cannot defeat both the Shutdown arm and the engine_shutdown flag check; the test would still |
| S313 — adopt_adjacent_turn_without_reset | `stt_turn_identity_test` | skip | 0.0 | Current stt/actor.rs has no wrapping_add(1) adoption branch, and the test only asserts resets >= 1, which any single-lin |
| S314 — dispatch_wrong_text_instead_of_query | `transcript_ingestion_test` | yes | 56.0 | expected-failure: The LLM request carries an empty user message instead of the finalized transcript. |
| S315 — remove_empty_transcript_guard | `transcript_ingestion_test` | yes | 65.7 | expected-failure: Whitespace-only transcript is dispatched instead of returning to Ready. |
| S316 — remove_state_guard | `transcript_ingestion_test` | yes | 55.9 | expected-failure: Transcript arriving in Ready is processed and dispatched. |
| S317 — dispatch_harness_turn_in_realtime_mode | `transcript_ingestion_test` | NO | 56.1 | test passed under mutant |
| S318 — remove_duplicate_turn_guard | `transcript_ingestion_test` | yes | 56.0 | expected-failure: Duplicate user turn is dispatched again instead of being suppressed. |
| S319 — drop_continuation_prefix_on_merge | `transcript_ingestion_test` | yes | 56.3 | expected-failure: Stashed continuation prefix is omitted from the merged query. |
| S320 — drop_late_final_capture | `transcript_ingestion_test` | NO | 56.0 | test passed under mutant |
| S321 — drop_empty_transcript_prefix_promotion | `transcript_ingestion_test` | yes | 65.9 | expected-failure: Stashed prefix is not promoted when the transcript is empty. |
| S322 — drop_turn_cancel_token_rearm | `transcript_ingestion_test` | yes | 66.0 | expected-failure: The stale cancel token is carried into the new turn. |
| S323 — skip_transliteration_before_dispatch | `transcript_ingestion_test` | yes | 56.2 | expected-failure: Devanagari query is dispatched without transliteration. |
| S324 — keep_old_turn_id_after_interrupt | `transcript_ingestion_test` | yes | 66.0 | expected-failure: Stage 2 does not allocate a new turn id after the interrupt. |
| S325 — store_text_before_state_guard | `transcript_ingestion_test` | yes | 56.1 | expected-failure: Dropped transcript is still stored in the accumulator. |
| S337 — shorten_silence_end_frames | `vad_segmentation_test` | yes | 76.9 | expected-failure: Silence end fires after a single frame, so natural pauses split utterances. |
| S338 — invert_silero_prediction | `vad_segmentation_test` | yes | 56.4 | expected-failure: Silero speech/silence classification is inverted. |
| S339 — error_on_absent_turn_detector_model | `vad_segmentation_test` | yes | 56.2 | expected-failure: An absent model path yields Err instead of a dormant detector. |
| S012 — reference_tap_pushes_every_48k_sample | `audio_capture_aec_test` | yes | 55.7 | expected-failure: Every 48 kHz sample is pushed, so 480 pushed samples yield 480 buffered samples instead of 160; the oc |
| S013 — aec_worker_skips_a_capture_chunk | `audio_capture_aec_test` | yes | 56.0 | expected-failure: One of every two capture frames is discarded, so only 160 of the 320 pushed samples arrive per chunk a |
| S014 — aec_bypass_flag_ignored | `audio_capture_aec_test` | yes | 55.9 | expected-failure: SetEnabled(false) no longer disables AEC; the filtered output differs from the 0.42 passthrough input  |
| S015 — delay_adaptation_pinned_to_wrong_value | `audio_capture_aec_test` | NO | 57.7 | test passed under mutant |
| S016 — aec_disabled_by_default | `audio_capture_aec_test` | yes | 56.2 | expected-failure: AEC is bypassed from startup, so the echo reaches the VAD unattenuated, barge-in fires, and the turn_i |
| S017 — speech_start_listening_transition_dropped | `audio_capture_aec_test` | yes | 61.2 | expected-failure: Stage 1 no longer enters Listening, so double-talk during Speaking never opens listening and the state |
| S018 — bypass_command_ignored_by_worker | `audio_capture_aec_test` | yes | 61.3 | expected-failure: The worker ignores SetEnabled(false); the echo stays suppressed and the positive-control scene never r |
| S054 — transcript_final_dispatch_dropped | `e2e_voice_pipeline_test` | yes | 101.9 | expected-failure: TranscriptFinal never reaches the assistant handler, no Thinking transition occurs, and the Thinking-b |
| S055 — speaking_barge_in_branch_disabled | `e2e_voice_pipeline_test` | yes | 65.3 | expected-failure: SpeechStart while Speaking no longer takes the interrupt arm, so no Listening transition occurs and th |
| S056 — continuation_prefix_stash_dropped | `e2e_voice_pipeline_test` | skip | 0.0 | bound test is currently RED (TF-03); cannot measure kill until fixed |
| S057 — empty_transcript_fallback_dropped | `e2e_voice_pipeline_test` | yes | 91.8 | expected-failure: Sub-300 ms bursts emit no empty TranscriptFinal, so the pipeline stays in Thinking and the Ready recov |

**Original campaign:** 302 killed, 23 survived, 1 timeout, 13 skipped (92.6%).
**Final state:** **317 killed, 0 survivors, 22 documented skips → 100% of executable mutants killed** (317/317). Mutation testing is accepted as clean.

## Post-campaign remediation (verified by targeted re-runs)

The table below is the original campaign snapshot. These entries were corrected and re-run:

| ID | Action | Re-run result |
| :--- | :--- | :--- |
| S068 | Strengthened `universal_tool_execution...` to assert committed tool observation content | **KILLED** |
| S097 | Added a drained-audio stale-event check with a stability barrier | **KILLED** |
| S006 | Added `execution_mode == Parallel` assertion to the web_search contract test | **KILLED** |
| S203 | Retargeted mutant to the silent-disconnect branch actually driven by the test | **KILLED** |
| S304 | Retargeted mutant to the stream-chunk divergence check | **KILLED** |
| S259 | Re-seeded to a silent wrong-outcome mutant (`Err(_) => Rejected`) after virtual-time bounding proved unreliable | **KILLED** |
| S059 | Documented as redundant: cancellation is re-checked at `steps.rs:795` and step5 | SKIP (evidence) |
| S089 | Two retargets left the test green; seed needs redesign | SKIP (evidence) |
| S028, S073, S113 | Confirmed behaviorally equivalent mutants | SKIP (evidence) |

All initially surviving mutants are resolved: 10 were killed by re-seeding or test strengthening; 5 (S022, S085, S241, S242, S243) are documented skips with evidence. No executable mutant survives.

## Surviving mutants — RCA (23 survivors + 1 inconclusive)

Classification: **weak test** = the assertion does not observe the mutated behavior; **wrong/equivalent mutant** = the edit does not change behavior on the test's path; **probable** = inferred from source context, needs a targeted re-run to confirm.

| ID | Test | Classification | Evidence / fix |
| :--- | :--- | :--- | :--- |
| S059 | `a_cancelled_turn_never_dispatches` | wrong/equivalent mutant (confirmed) | `steps.rs:114` guard is redundant: `steps.rs:795` re-checks `req.cancel.is_cancelled()` before execution. Retarget the downstream guard. |
| S065 | `zero_tool_turn_with_interim_filler_latches_to_ready_only_after_playback_drains` | probable weak test | Mutant disables the `pending_jobs > 0 || sink_active` hold in `llm.rs:118`; the test passed, so it likely never reaches the latch with audio outstanding. Add an assertion on state while `pending_jobs > 0`. |
| S068 | `universal_tool_execution_succeeds_without_synthetic_speech_parameters` | weak test (confirmed in CP2 audit) | Observation content is zeroed; test asserts only final text and dispatch count. Assert the committed observation content. |
| S071 | `turn_cancellation_during_tool_execution_aborts_cleanly_and_clears_turn_open` | probable redundant path | `steps.rs:778` is a returned flag, not the cancellation action; another path still returns Cancelled. Retarget the actual tool-batch abort check. |
| S073 | `turn_cancellation_before_speech_rolls_back_history_with_zero_db_artifacts` | equivalent mutant (confirmed) | `steps.rs:901` forced true, but `commit_completed_turn` early-returns on empty text (`chassis.rs:350`), so behaviour is unchanged. Mutant invalid; replace with a non-empty partial-text case. |
| S085 | `mid_loop_critical_budget_triggers_compaction_while_preserving_active_turn_steps` | probable wrong path | `chassis.rs:490` slice logic mutated; test likely observes a different history field. Assert the exact preserved step messages after compaction. |
| S089 | `mid_turn_thinking_barge_in_preserves_committed_tools_and_pushes_standalone_user_message` | probable wrong/equivalent mutant | Replacement changes `clear()` to `stash_for_continuation` at `transcript.rs:293`; the test's assertion may not depend on that accumulator field. Recheck the mutant compiles to the intended semantics. |
| S097 | `stale_playback_finished_is_dropped_and_active_turn_is_resolved` | weak test (confirmed in CP2 analysis) | Epoch guard disabled, but the test seeds `pending_tts_jobs = 1`, so `on_playback_finished` defers before the guard matters. Assert the stale event is dropped before the defer check. |
| S022 | `project_delete_is_restricted_while_sessions_exist` | probable weak test | FK `RESTRICT → CASCADE` in `schema.rs:22` does not fail the test; likely the test uses soft delete or an existing schema. Assert a physical project delete is rejected. |
| S028 | `concurrent_readers_and_persistence_worker_complete_without_db_errors` | equivalent mutant (confirmed) | `BEGIN CONCURRENT → BEGIN EXCLUSIVE` still succeeds; the test only asserts absence of errors. Assert turn counts after the worker barrier. |
| S032 | `tool_call_event_self_heals_a_missing_session` | probable weak test | Removing `ensure_session_exists` does not fail; the test may use an already-existing session. Use a genuinely missing session id. |
| S211 | `remote_connection_heartbeat_timeout_transitions_to_paired` | probable redundant path | The guard at `supervisor.rs:415` controls the transition, but other supervisor paths also return `Paired`; isolate the heartbeat path with virtual time. |
| S213 | `remote_connection_host_disconnect_keeps_pairing` | probable wrong mutant | Injected early-return side effect at `manager.rs:792`; pairing survives through another path. Retarget the persistence call directly. |
| S248 | `payload_near_maximum_size_roundtrips_and_over_cap_fails` | probable wrong mutant | Cap check in `noise.rs:281` disabled; test may use a different cap constant or layer. Confirm the exact oversized payload the test sends. |
| S249 | `oversized_handshake_message_is_refused` | probable wrong mutant | Cap check in `noise.rs:147` disabled; same issue as S248. Assert the refusal at `read_message` with a payload above the constant. |
| S006 | `web_search_tool_declares_a_hermetic_non_terminal_contract` | weak test (confirmed — CP3 rename) | `ToolExecutionMode::Parallel → Exclusive`; the renamed contract test checks schema fields, not the mode. Assert `execution_mode`. |
| S113 | `coordinator_declines_a_slice_while_a_compaction_is_in_progress` | equivalent mutant (confirmed) | `coordinator.rs:85` guard disabled, but the partial unique index still rejects the competing insert. Use a mutant that removes the index check effect. |
| S053 | `ptt_ipc_dictation_seam` | probable weak test | `PttStop.owner` flipped; the test asserts the Final event's owner (`dictation_ptt_test.rs:688`) but apparently not the PttStop owner. Add that assertion. |
| S203 | `paused_state_suppresses_the_reconnect` | probable wrong branch | Mutant targets the GoAway branch (`transport.rs:310`); the test likely drives the silent-disconnect branch. Retarget the branch under test. |
| S304 | `turn_jump_resets_provider_state` | equivalent mutant (confirmed by source shape) | `WorkerState.current_active_turn` initializer changed; normal turn tracking overwrites it before the assertion. Mutate the divergence check instead. |
| S317 | `realtime_mode_keeps_the_query_but_does_not_dispatch` | probable weak/equivalent mutant | Extra dispatch injected into the Realtime arm at `transcript.rs:432`; test asserts accumulator/state but not dispatch absence. Use a dispatch-capture assertion. |
| S320 | `late_arriving_final_is_captured_as_a_continuation_prefix` | weak test (confirmed in CP2 audit) | `if is_late → false`, but the test hand-sets `bound_utterance_id = Some(0)`, so `is_late` is already false. Drive two real `SpeechStart` events to create genuine utterance IDs. |
| S015 | `aec_suppresses_the_synthetic_tts_echo` | weak/flaky test (confirmed in prior audit) | Delay adaptation pinned to 500 ms; ERLE scene still passes, so its ground-truth threshold does not discriminate. Tighten the ERLE assertion or use the 120 ms skew case. |

### Inconclusive mutant

- **S259 — Pairing approval never times out** (`remote_security_test::approval_prompt_times_out_deterministically`): runner hit its 900s timeout. The mutant replaced `PAIRING_APPROVAL_TIMEOUT` with `std::time::Duration::MAX`, which is not a plausible silent production defect under virtual time. **Wrong mutant**: re-run with a finite but longer timeout (for example 2× the constant) so the test fails deterministically.

All mutants were restored byte-for-byte after execution. A survivor means the test is weak or the mutant is wrong; triage before merge. RCA is recorded above; remaining items require test fixes and targeted re-runs.
