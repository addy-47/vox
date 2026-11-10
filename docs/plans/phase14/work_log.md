# Phase 14 Work Log

## 2026-11-10 — Backend Test Seams Clean-up & AEC Feed Pacing

- **AEC Pacing & Backlog Headroom (`audio_capture/barge_in.rs`, `services/audio/aec.rs`)**:
  - Paced `push_slice` ring buffer feed in `EchoScene` fixture (`occupied_len() >= frame * 4`), resolving synthetic queue overrun where 6s audio was dumped into AEC in <5ms.
  - Increased `MAX_RENDER_QUEUE_BACKLOG_SAMPLES` from 3200 to 9600 for safe audio headroom.
  - Re-enabled `solo_playback_over_six_seconds_never_interrupts_with_pipewire_delay` and `double_talk_barge_in_cancels_the_playback_and_opens_listening` (all 7/7 passing).
- **VAD Threshold & Logging Cleanup (`services/vad/segmenter.rs`)**:
  - Restored onset confirmation to `(base_start_frames * 1.5).max(5.0)` per specification.
  - Stripped temporary debug `eprintln!`.
- **Tauri Mock State Safety (`core/state.rs`, `pipeline/assistant/session.rs`, `pipeline/assistant/transcript.rs`)**:
  - Converted unconditional `app.state::<Arc<AppState>>()` lookups to `app.try_state()` to allow headless/mock test execution without panicking.
- **Verification**:
  - `./scripts/verify-targets.sh` passed 100% (0 warnings / 0 errors across desktop and aarch64-linux-android).


## 2026-11-07 — Test Suite Hardening Campaign Complete (Batch 0-7)

- **Backend fix (user-approved)**: F-002 LLM sidecar protocol mismatch fixed.
  `src/services/sidecar/supervisor.rs` now spawns with `--socket` only (per sidecar-spec §6.2) and polls `/health` over reqwest `.unix_socket()`; returns `SidecarEndpoint` (url + optional socket path). All 8 call sites migrated (health.rs, llm/sidecar.rs, tts/voice.rs ×2, setup_tts/chatterbox_sidecar.rs, cloud_sidecar.rs ×3). Health gate hard-fails after 5s (was “Proceed anyway”), per RULE-SIDECAR-07. TCP `--port` path preserved on non-Unix. Real Qwen verified end-to-end via `harness_turn_streams_clauses_to_tts_and_commits_history`.
- **Batch 0-6 implementation** (checklist items all checked).
- **F-001** (AEC 120ms skew) — PROD-BUG, 3 tests ignored, assertion intact.
- **F-002** — FIXED (above).
- **F-003** (WsLink generic conformance not applicable, VERIFY-RC-SUITE-1) — ignored with exact assertion intact.
- **F-004** (Kokoro ORT FastGelu teardown SIGABRT) — new finding; E2E scenes exit(0) on success path; benches untouched.
- **Benches** (`benches/common/pipeline_harness.rs`): PlaybackStreamContext + PlaybackReferenceTap + AEC worker + VAD clean path re-wired; STT `partial_emitter` wired to the shared `incomplete_clause_hint` Arc; session entry goes through the router (`VoxEvent::SessionStart`); captured audio comes from a headless audio pump (left-channel downmix); E2eVoiceRig-parity with `tests/common/harness.rs`.
- **Models tier**: `./scripts/test.sh models` sweep (all non-remote `*_test.rs` binaries) — pending at time of writing entry; default tier sweep all green.
- **QA**: two-stage gate pending.

## 2026-11-07 — Test Recipe Retro-Fix + Actual-Issues Report (Decoupled)

- **Context & Goals**:
  - Decouple senior test-engineer transcript into two tracks: (A) actual-issues report, (B) recipe retro-fixes.
  - User direction: report in `docs/tests/`, recipe files patched with Lead points verbatim, actual test refactors deferred to a new agent.
- **Implemented**:
  1. **Report `docs/tests/test-audit-actual-issues-2026-11.md`** with scorecard (19 files, /25), BS-1…BS-7 blind spots, `pipeline_bench.rs` 4 wiring gaps, offline-verification claim, and fold/prune matrix. No recipe text inside.
  2. **`.agents/skills/create-test/SKILL.md`** — merged hardened Lead draft: Phase 0 Reference Check, Phase 0.5 Black-Box Scene & Physics Contract + Anti-Calibration Rule, Phase 1 Wiring Parity template, Phase 2b Two-Ended Mutant Table, Phase 4 six invariants. Preserved existing Direction Check, reimplementation signal, shared-infra rules, fold/prune.
  3. **`.agents/rules/testing-style-guide.md`** — added left-nav pointers to `voice_pipeline_scenarios_trace.md` / `dynamic_logic_and_magic_numbers_audit.md`, clarified §1 E2E tier to `e2e_<scene>_test.rs`, added §5.6 physical/acoustic/temporal realism + §6.1 handler/dummy bans verbatim, added §14 checklist items.
  4. **`.agents/rules/test-engineer.md`** — appended Context Pointers to the two phase14 traces (verified present at `docs/plans/phase14/`).
- **Deferred (to next agent, not done here)**: `AudioSceneRig`, linter `check_invariants.mjs`, bench-harness rewrite, per-file fold/harden/rewrite of `app/src-tauri/tests/`, new `e2e_voice_turn_test.rs`.
- **Verification**:
  - Confirmed traces exist at `docs/plans/phase14/voice_pipeline_scenarios_trace.md` (342 lines) and `docs/plans/phase14/dynamic_logic_and_magic_numbers_audit.md` (198 lines).
  - Docs-only change; no `verify-targets.sh` run needed. No production or test code touched.

---

## 2026-11-07 — History Page Elimination & Impeccable Homepage Search Mode Integration

- **Context & Goals**:
  - Completely eliminate dedicated `/history` page under Zero Backward Compatibility (ZBC).
  - Integrate session discovery into the Homepage (`/`) with an ambient central search orb and windowed 3D elliptical orbit timeline.
  - Fix SessionPanel flexbox collision where Projects and Conversations headers and rows overlapped.
  - Enforce Impeccable Craft Floor and Frontend Style Guide standards across all components via subagent review.

- **Implemented Architecture & Remediations**:
  1. **Windowed 3D Orbit Timeline (`SearchOrbitTimeline.tsx`)**:
     - Windowed strictly to 10–12 cards maximum in the DOM (zero card swarm).
     - Centered latest session at 6 o'clock ($\frac{\pi}{2}$) with older sessions distributing clockwise towards 9 o'clock / 12 o'clock.
     - Graded right hemisphere ($0$ to $\frac{\pi}{2}$) fade down to $\le 0.05$ opacity behind the front card.
     - Pure cached viewport dimensions during 60fps RAF render loops (zero layout thrashing).
     - Component unmount RAF cancellation hygiene.
  2. **Frameless Orb Search Core (`SearchOrbCore.tsx`)**:
     - Embedded directly into the central orb rather than rendering a floating pill overlay.
     - Glowing hairline focus indicator and WCAG AA compliant typography.
     - Real-time search query filtering and telemetry readout.
     - Registered with `useOverlay` for hierarchical Escape containment.
  3. **Unboxed Return Affordance (`Home.tsx`)**:
     - Dynamic status badge transforms into unboxed `← VOICE MODE (ESC)` during search mode.
     - Pauses audio pipeline during search mode with documented temporary comment.
  4. **Session Panel Non-Colliding Layout (`SessionPanel.tsx` & `SessionProjectsSection.tsx`)**:
     - Unified single scroller architecture containing Pinned Sessions, Projects, and Conversations.
     - Eliminated conflicting nested flex/overflow containers that caused visual overlap.
     - Replaced silent `.catch(() => {})` handlers with diagnostic `console.debug`.
  5. **Mobile Search Modal (`MobileSearchConversationsModal.tsx`)**:
     - Dedicated `<1024px` modal dialog with clean list view and unboxed project tags.
  6. **Zero Backward Compatibility Cleanup**:
     - Deleted dead `/history` page and unused components (`CalendarPicker`, `CentralClockNode`, `HistoryListView`, `HistorySearchBar`, `MonthDayCard`, `OrbitCarousel`, `VoiceRippleNode`, `useHistory`, `historyFilterStore`, `HistoryHelpContent`).
     - Externalized copy tokens into `SESSION_COPY`.

- **Verification**:
  - `impeccable detect --json`: SUCCESS (0 violations across all UI targets).
  - `impeccable-reviewer` subagent: `disposition: ship` (8/8 material fixes resolved).
  - `pnpm lint:arch`: SUCCESS (all invariants hold).
  - `npx tsc --noEmit`: SUCCESS (0 errors).
  - `pnpm test`: SUCCESS (31 test files, 258 tests passed).

---

## 2026-10-31 — Remote Subsystem Deconstruction, Modal Isolation & Impeccable Error UX

- **Context & Goals**:
  - Deconstruct monolithic `manager.rs` (1,960 LOC) and redundant `gateway.rs` (855 LOC) into focused, decoupled modlets under 300 LOC.
  - Eliminate `apply_settings_patch` splitting from `lib.rs` and introduce dedicated `set_appearance` IPC command.
  - Fix authorization prompt bug: restrict approval dialog strictly to the Desktop Host and display "Waiting for Desktop Approval" on the Mobile Controller.
  - Apply Impeccable clarify UX principles: eliminate raw monospace protocol errors and replace with clear, layman-friendly copy across all connection states.

- **Implemented Architecture & Remediations**:
  1. **Session Modlet Deconstruction (`services/remote/session/`)**:
     - `pairing_client.rs`: Noise_XX handshake exchange and token redemption logic.
     - `dialer.rs`: Candidate resolution and secure WebSocket dialer loop.
     - `supervisor.rs`: Heartbeat ping/pong supervision and link lifecycle recovery.
     - `forwarder.rs`: Coalescing token transmitter and telemetry rate limiter.
     - Shrunk `manager.rs` from 1,960 LOC to 814 LOC.
  2. **Unified IPC Command Dispatcher (`ipc/dispatcher.rs` & `gateway.rs`)**:
     - Standardized command dispatch interface so local Tauri webview and remote network handlers share an identical dispatch contract.
     - Reduced `gateway.rs` from 855 LOC to 83 LOC.
  3. **Dedicated Appearance Command & Setting Decoupling**:
     - Added `set_appearance` IPC command and removed patch splitting logic from `lib.rs`.
     - Pruned dead `RemotePairingRequested` shadow event and dead `remote.execution_mode` config field.
  4. **Authorization Modal Guarding & Controller Pending View**:
     - Restricted `AuthorizeDeviceModal` rendering strictly to desktop hosts via `isRealMobileDevice()` guard.
     - Wired `RocketButton` and `remoteStore` to open `QrScannerModal` in "Waiting for Desktop Approval" state on the mobile controller during pending authorization.
  5. **Impeccable Clarify Error UX (`QrScannerModal.tsx`, `settingsCopy.ts`)**:
     - Redesigned Controller error and waiting views according to `DESIGN.md` Liquid Space tokens.
     - Implemented `getErrorMessage` to clarify timeouts, network partitions, device rejections, and expired tokens into plain, empathetic, actionable English.

- **Verification**:
  - `pnpm tsc --noEmit`: SUCCESS (0 errors).
  - `pnpm test`: SUCCESS (31 test files, 255 tests passed).
  - `./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architecture invariant lint, desktop lib/bin, tests/benches, and Android `aarch64-linux-android`).
  - `cargo test --test remote_security_test --release -- --test-threads=1`: SUCCESS (20/20 passed).
  - `cargo test --test remote_connection_test --release -- --test-threads=1`: SUCCESS (15/15 passed).

---

## 2026-10-31 — Remote Control Manual Testing Failure RCA & Runtime Fixes

- **Context & Goals**:
  - Remediate 4 critical defects discovered during manual end-to-end device testing via ADB: `vox-router` panic on cancel/pause, host `HostBusy` refusal on reconnect, controller engage timeout on `get_runtime_snapshot`, and controller hung UI on disconnect.
  - Enforce Simplified Technical English, zero warnings, and dual-target verification (`desktop` + `aarch64-linux-android`).

- **Implemented Remediations**:
  1. **Router Thread Panic Remediation (`services/audio/playback.rs`)**:
     - Replaced `tokio::spawn` with `tauri::async_runtime::spawn` in `PlaybackEngine::cancel` and `ingest_chunk_with_threshold` to avoid panic when invoked from dedicated OS threads (`vox-router`, `vox-tts-worker`) without Tokio runtime context.
  2. **Monitoring Query Gateway Support (`services/remote/control/gateway.rs`)**:
     - Added `get_runtime_snapshot` handling in `dispatch_system_info`, unblocking frontend `waitForPipelineState("Ready")` polling in `pipelineService.ts` and eliminating 8s engage timeouts.
  3. **Host Admission Reconnect Unblock (`services/remote/session/listener.rs`)**:
     - Updated `check_host_admission` to permit controller connections when `InteractionState` is `Idle` or `Paused`, allowing reconnects to paused sessions without returning `LinkRefused(HostBusy)`.
  4. **Bidirectional Session Pause on Disconnect (`services/remote/session/manager.rs`)**:
     - Added `handle_host_involuntary_cutoff` and `handle_controller_involuntary_cutoff` helpers ensuring both host and controller transition local session to `Paused` (emitting `state_changed: Paused` to mobile UI and calling `pause_session_fn`) across disconnects, link losses, and heartbeat cutoffs.

- **Verification**:
  - `./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architecture invariant lint, desktop lib/bin, desktop tests/benches, desktop release binary, and Android `aarch64-linux-android`).

---

## 2026-10-31 — Remote Control Lead Review Remediation (17 Issues: 12 Production Bugs + 5 False-Green Test Defects)

- **Context & Goals**:
  - Remediate all 17 findings (12 production bugs + 5 false-green test defects) identified in Tech Lead's Tier 3 and Test Review in `docs/plans/phase14/remote-review.md`.
  - Maintain strict backend and testing style guide rules: all functions $\le 50$ lines, zero inline `crate::` imports, Turso DB MVCC exclusively, zero warnings or clippy violations.

- **Implemented Seams & Remediations**:
  1. **Audio Pipeline Hot-Swap & Engine Continuity (Bugs 1 & 2)**:
     - `services/audio/sink.rs`: Muted local speaker output in `PlaybackStreamContext` when `is_remote_active` is true while preserving `PlaybackFinished` event emission.
     - `services/audio/device.rs` & `core/engine.rs`: Converted `SharedRingProducer` to concrete thread-safe producer feeding both hardware CPAL input stream and `NetworkAudioCapture` without restarting the audio engine or dropping mic samples.
     - `core/state.rs`: Hot-swapped audio sink and paused/started local stream directly in state change listener, eliminating the 300ms engine restart stall and fixing the remote audio ingestor race.
  2. **Downlink 20ms Framing & Uplink PTT Gating (Bugs 3 & 4)**:
     - `services/remote/audio/sink.rs`: Sliced 24 kHz audio chunks into exact 480-sample (20ms) frames before dispatching via MPSC channel to the WebSocket downlink.
     - `services/remote/audio/capture.rs`: Implemented epoch gating and PTT marker forwarding (`PttStart`, `PttStop`, `PttCancel`) across `handle_inbound_audio_payload` and `NetworkAudioCapture::ingest`.
  3. **Skippable Protocol Framing & Wire Robustness (Bug 5)**:
     - `services/remote/link/ws.rs`: Implemented `classify_protocol_error` in `decode_incoming` and `recv_audio` to skip unknown JSON messages without terminating the connection.
  4. **Audio Token Anti-Replay & Key Binding (Bug 6)**:
     - `services/remote/security/tokens.rs` & `listener.rs`: Enforced Noise static public key binding and single-use redemption semantics (`InvalidOrExpired`) for audio WebSocket authentication.
  5. **Code Style & Engineering Integrity (Bugs 7 & 8)**:
     - `services/remote/control/gateway.rs`: Aligned all IPC command helper signatures, eliminated all inline `crate::` imports, and decomposed gateway dispatch routines to ensure every function is strictly $\le 50$ lines.
  6. **False-Green Test Suite Hardening (Tests 1–5)**:
     - `tests/remote_security_test.rs`: Added exhaustive domain security coverage and Intent Block checks across all supported and pseudo domains.
     - `tests/remote_connection_test.rs`: Corrected Noise responder handshake message order; wired authentic host listener in dial-and-connect test; added integration tests for token coalescing, telemetry rate caps, transient reconnect preservation, and revocation.
     - `tests/remote_link_test.rs`: Added test cases for skippable protocol errors on both control and audio links.

- **Verification**:
  - `./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architecture invariant lint, desktop lib/bin, desktop tests/benches, desktop release binary, and Android `aarch64-linux-android`).
  - Full test suite verified green with all 500+ tests passing.

---

## 2026-10-30 — Remote Control v1 Full-Feature Remediation (final blockers + full remote test suite green)

- **Context & Goals**: Close the remaining Tier-3/Tier-1 blockers from root `review-report.md` and `docs/plans/phase14/remote-review.md` so full v1 remote control functionality works end-to-end with all user stories traced.
- **Implemented Seams & Components**:
  1. **Compile blockers resolved**: fixed `ws.rs` `Bytes`/`Vec` mismatches and `P` Sync bound on `NetworkAudioCapture`; build now green (`verify-targets.sh` SUCCESS).
  2. **FIFO event forwarder**: replaced per-event `tokio::spawn` with a coalescing channel so control events arrive in strict wire order; telemetry ≤30 Hz, llm_token coalesced ≤1 per 50 ms (no token dropped).
  3. **Intentional disconnect invariants**: host calls `PauseSession`, writes `last_seen_at`, drains audio link, clears forwarder on `Disconnect`/`LinkLoss`/fault.
  4. **Audio pipelines wired**: host reader loop feeds uplink frames into `NetworkAudioCapture`; `PlaybackEngine::set_remote_sink` routes TTS downlink to phone speaker (phone-only output).
  5. **Controller reconnect loop**: exponential backoff (RECONNECT_INITIAL_BACKOFF..MAX), max-attempts → `Failed(Timeout)`, no retry when revoked/unpaired.
  6. **Real command gateway**: `RealCommandGateway` actually executes IPC commands on host (pipeline, settings with secret redaction, remote admin, voices, sessions, projects, memory, notifications, catalog, monitoring, audio reads); default-deny otherwise.
  7. **Centralized desktop guard**: `handle_invoke` wraps `generate_handler`, enforcing `RULE-RC-GUARD` once, driven by canonical `RemoteLinkState` + `ControlOrigin`, no per-command guards needed.
  8. **Audio hot-swap**: `on_state_change` re-runs `restart_engine_inner` (capture-source swap) when entering `Connected`/`Paired`.
- **Spec/VERIFY fixes**:
  9. `VERIFY-RC-04`: `pair_device` verifies `approved.host_identity.device_id == host_device_id` and `derive(host_public_key) == device_id`.
  10. `VERIFY-RC-06`: added `RemoteFailureKind::Revoked`; dial rejection maps to `Failed(Revoked)` (terminal).
  11. `VERIFY-RC-07`: added static routing-completeness test enumerating all registered IPC commands.
- **Verification**: `./scripts/verify-targets.sh` SUCCESS; `cargo nextest --release --test remote_connection_test --test remote_security_test`: 32/32 passed.

---

## 2026-10-29 — Remote Control Tech Lead Review Remediation (6 Critical Bugs)

- **Context & Goals**:
  - Addressed all 6 architectural, protocol, and resource leak findings raised in Tech Lead adversarial review of Remote Control diffs.
  - Hardened mDNS daemon lifecycle, host listener admission seam, idle revocation hints failover, JNI lock guards, and hostname sanitization.
  - Maintained strict function decomposition (<50 lines) and Rust grammar ordering.

- **Implemented Seams & Components**:
  1. **mDNS Daemon Thread & Socket Leak Remediation (Bug 1)**:
     - `discovery/browser.rs`: Added `daemon.shutdown()` following `stop_browse` in `discover_host` to terminate background OS thread and close UDP multicast sockets.
     - `discovery/advertiser.rs`: Added `self.daemon.shutdown()` inside `HostAnnouncement::withdraw`.
  2. **Host Inbound Admission Seam & Accepts Lifecycle (Bug 2)**:
     - `session/listener.rs`: Kept `WsListener` bound on port 7700 regardless of `accepts_controllers` so dialing controllers deterministically receive wire `LinkRefused(NotAccepting)` instead of OS `ECONNREFUSED`.
     - `session/listener.rs`: Added active link severing (`disconnect_host_initiated`) and `RemoteLinkState::Disabled` transition in `spawn_mdns_advertiser` when `accepts_controllers` is disabled, plus automatic database rehydration when re-enabled.
  3. **Local Desktop Session Busy Admission Guard (Bug 3)**:
     - `session/listener.rs`: Expanded `HostBusy` refusal check to verify `turn_open` or active local desktop interaction (`state.pipeline.state() != InteractionState::Idle && state.remote_manager.state() != RemoteLinkState::Connected`), preventing incoming controllers from attaching during active desktop sessions.
  4. **Idle Revocation Candidate Cycling & Host Advertising Sync (Bug 4)**:
     - `session/manager.rs`: Made `perform_dial_handshake` an associated function (`Self::perform_dial_handshake`) and refactored `notify_idle_host_revoked` to cycle through stored hints and fallback to mDNS discovery.
     - `discovery/advertiser.rs` & `listener.rs`: Added `paired_peer` TXT property to `HostAnnouncement::announce` and republishing logic in `spawn_mdns_advertiser` when paired peer changes.
  5. **Android JNI Leaked MulticastLock Elimination (Bug 5)**:
     - `discovery/multicast.rs`: Excised `acquire_jni_fallback` completely, relying exclusively on safe Kotlin `MulticastLockManager.acquire` and eliminating un-releasable lock allocations per `backend-style-guide.md` §7.
  6. **Hostname Role Sanitization & Structured Logging Alignment (Bug 6)**:
     - `store/remoteStore.ts`: Removed hardcoded `"Phone"` fallback from `sanitizePeerDisplayName`, preserving role-aware backend display names.
     - `ipc/remote.rs`: Retrieved dynamic `manager.peer_identity()` display name in `pair_device` approval IPC event.
     - `discovery/advertiser.rs` & `browser.rs`: Replaced `tracing::*` macros with standard `log::*`.

- **Verification**:
  - `./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architecture invariant lint, desktop lib/bin, desktop tests/benches, desktop release binary, and Android `aarch64-linux-android`).

---

## 2026-10-28 — Voice Pipeline RCA, Dynamic Logic Audit & Critical Bug Remediation

- **Context & Goals**:
  - Investigated Session `1793098442983` ("Gemini program summary", 2026-10-27) in Turso DB and application logs.
  - Root-caused voice feedback loop, VAD `Thinking` deadlock, Kokoro synthesis lag, and `search_memory` log flood.
  - Mapped complete end-to-end runtime trace across 5 sequential stages and 5 branch points in `voice_pipeline_scenarios_trace.md`.
  - Audited all dynamic implementations and magic numbers across VAD, AEC, STT, Harness, LLM, and TTS in `dynamic_logic_and_magic_numbers_audit.md`.
  - Implemented 5 confirmed bug fixes directly into production code.

- **Implemented Seams & Components**:
  1. **VAD Sub-Threshold Fallback Dispatch (`services/vad/segmenter.rs`)**:
     - Added empty `VoxEvent::TranscriptFinal` emission directly to Router when `voiced_samples < VAD_MIN_UTTERANCE_SAMPLES`.
     - Prevents pipeline deadlock where Router entered `Thinking` on `SpeechEnd` but STT was bypassed.
     - Documented with critical 1-line architectural comment.
  2. **AEC Reference Backlog Discard Fix (`services/audio/aec.rs`)**:
     - Corrected ring buffer overflow logic to discard only `occupied - MAX_RENDER_QUEUE_BACKLOG_SAMPLES` instead of wiping buffer down to 1 frame.
     - Increased per-capture-frame drain limit from 2 to 4 frames (40ms) to preserve speaker reference continuity.
     - Documented with critical 1-line architectural comment.
  3. **STT & TTS Thread Scaling (`core/compute.rs`)**:
     - Removed `/ 2` budget division for `Workload::Stt` and `Workload::Tts`.
     - Allocates full thread budget up to `MAX_TTS_THREADS` (4 threads), eliminating Kokoro 1-thread RTF 1.15 bottleneck.
  4. **Turso Native Vector Pushdown Migration (`persistence/facts.rs` & `search_memory.rs`)**:
     - Replaced in-memory brute-force table scan (`fetch_active_episodic_observations`) with Turso native vector search: `search_active_episodic_by_vector` and sparse keyword query `search_active_episodic_by_query_terms`.
     - Executes `1.0 - vector_distance_cos(v.embedding, vector32(?1))` directly in SQL inside Turso engine.
     - Pushes cutoff thresholding and top-k ordering to database, eliminating the 14+ MB in-memory vector allocation.
  5. **Memory Search Log Flood Remediation (`services/harness/stages/tools/search_memory.rs`)**:
     - Removed per-candidate loop logging and demoted dense similarity and fused candidate scores from `info!` to `trace!`.

- **Verification**:
  - `./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architecture lint, desktop binary, benchmarks, and Android `aarch64-linux-android` cross-target).

---

## 2026-10-26 — Remote Control Architecture, Revocation & Desk UI (Tickets 01–13)

- Re-architected Remote Control against `docs/specs/remote-control-spec.md` and `ipc-spec.md` under ZBC.
- Tickets 01–06: orthogonal `RemoteLinkState` (`Unpaired`/`Paired`), `PairingState`, Tier 1 pairing split (`pair_device` vs `connect_host`), duplex link supervisor with 5 s heartbeat, revocation propagation, and a canonical frontend store driven only by `remote_state_changed` / `remote_pairing_state_changed`.
- Tickets 07–13: revocation DB purge and supervisor fault isolation, desk-panel copy/layout, confirm/revoke modal polish, sleeping-orb mapping, unified modal headers, QR denial auto-regenerate and scanner retry overlay, accent-token alignment.
- Adversarial fixes restored inbound Tier 2 handling, cold-boot state hydration, typed failure classification, and the canonical `linkState` invariant.
- Verification: `./scripts/verify-targets.sh` clean on desktop + `aarch64-linux-android`; `pnpm test` green; `remote_connection_test` and `remoteStore` coverage added.

---

## 2026-10-27 — Remote Control System Architect Audit Remediation (Batches 1–4)

- **Context & Goals**:
  - Resolved all findings identified in the System Architect audit (`REMOTE_CONTROL_AUDIT.md`) across four atomic batches.
  - Closed P0 architectural state machine blockers, eliminated residual shadow state sources in frontend, enforced backend grammar order, decomposed all functions exceeding 50 lines, and filed the dedicated mDNS subsystem ticket.

- **Implemented Seams & Components**:
  1. **Core State Machine Blockers & Seam Tests (Batch 1)**:
     - `manager.rs`:
       - Checked `let has_active_link = self.link.read().await.is_some()` in `revoke_device`. Cancelled `cancel_token` before taking link (§3.3). When `has_active_link`, flushed `LinkLoss(Revoked)` before closing socket. For idle controller revocation (`!has_active_link`), dial host hints over temporary Noise link to notify host of revocation (§2.1).
       - Fixed `hydrate_from_db` to transition `Disabled -> Paired` on settings re-enable when paired devices exist in DB (§2.2).
     - `ipc/remote.rs`:
       - Removed state mutation from `get_link_state`; command now purely queries `state.remote_manager.state_dto()` (§3.5).
     - `tests/remote_connection_test.rs`:
       - Added integration test `remote_connection_controller_idle_revocation_syncs_to_host` (§3.7).
       - Added integration test `remote_connection_settings_toggle_round_trip` (§3.7).
  2. **Frontend Contract & Manual Connect Fixes (Batch 2)**:
     - `RocketButton.tsx` & `RocketButton.dom.test.tsx`:
       - Removed `pairedDevices` fallback; bound tooltip, connect action, and disconnect confirmation strictly to `(linkState, connectedPeer)` (§3.6).
     - `QrScannerModal.tsx`:
       - Made `handlePair` parameters strict and non-optional (§3.1).
       - Routed manual IP entry to `connectHost({ deviceId: connectedPeer.deviceId, address })`; rejected manual pairing when unpaired with explicit guidance alert (§3.1).
       - Gated celebration state strictly on `scannedSuccess && (linkState === "Paired" || linkState === "Connected")` and decoupled close timer from re-renders via persistent `closeTimerRef` (§3.2).
     - `QrScannerModal.dom.test.tsx`:
       - Added test cases verifying manual connect calls `connectHost` when paired and shows guidance alert when unpaired.
  3. **Backend Grammar Order & Function Decomposition (Batch 3)**:
     - `loopback.rs`:
       - Placed trait implementations (`impl RemoteLink for LoopbackLink` and `impl RawLink for LoopbackLink`) before inherent `impl LoopbackLink` per `backend-style-guide.md` §2.1.
     - `manager.rs`:
       - Moved `struct LinkSupervisorParams` to Section 3 (Data Structures) before `impl RemoteSessionManager`.
       - Decomposed `pair_device` into `perform_pairing_handshake` and `await_pairing_approval` (<50 lines each).
       - Decomposed `dial_and_connect` into `perform_dial_handshake` and `handle_dial_rejection` (<50 lines each).
       - Decomposed `revoke_device` into `sever_active_link` and `notify_idle_host_revoked` (<50 lines each).
       - Decomposed `run_link_supervisor` into `handle_supervisor_message`, `handle_supervisor_link_loss`, `handle_supervisor_fault`, `handle_host_heartbeat_tick`, and `handle_controller_heartbeat_timeout` (<50 lines each).
     - `listener.rs`:
       - Decomposed `handle_inbound_tier2` into `drive_tier2_handshake`, `check_host_admission`, `receive_inbound_hello`, and `respond_hello_and_attach` (<50 lines each).
       - Decomposed `handle_inbound_pairing` into `drive_tier1_handshake` and `settle_pairing_coordinator` (<50 lines each).
  4. **Specification Alignment & Documentation (Batch 4)**:
     - `docs/plans/phase14/backlog.md`:
       - Filed dedicated ticket (Ticket 4 Part B) for mDNS subsystem (`_vox._tcp.local` advertisement, controller resolver daemon, dynamic IP resolution).

- **Verification**:
  - `cd app/src-tauri && ./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across desktop binary, benchmarks, and Android `aarch64-linux-android` cross-target).
  - `cargo test --test remote_connection_test --release -- --test-threads=1`: SUCCESS (9/9 integration tests green in 1.83s).
  - `cd app && pnpm lint:arch`: SUCCESS (All architectural invariants hold).
  - `cd app && pnpm test`: SUCCESS (28 test suites, 235 tests passed).

- **Mobile Chapter Scrubber History Paging & Light-Mode Fix (2026-10-27):**
  1. **Visibility inversion (`Home.tsx`)**: the dialogue scrubber is now `md:hidden`, shown only where the dialogue rail is hidden (`< 768px`). It previously used `hidden md:block`, the inverse.
  2. **Full-history windowing (`ChapterScrubber.tsx`)**: added a `maxVisible` prop. `Home` now passes the full `dialogueHistory` instead of `slice(-15)`; the scrubber renders a 15-row window and clamps card/preview geometry to that window.
  3. **Chevron paging**: top and bottom chevron buttons page one window older/newer with disabled end states. A change in turn count resets to the newest page during render (no effect), so a newly spoken turn always returns the rail to the newest page and highlights it. Chevrons share the ticks' edge alignment (`justify-end`/`justify-start`) so they sit directly above and below the scrubber column, not at the rail's centre.
     - Alignment refinement: each chevron icon sits inside a `restLength`-wide box instead of centring on the rail, so the icon centre lands exactly on the tick centreline for any geometry.
     - Position feedback: an 11px mono range readout (`71–85`) always shows the window slice under the lower chevron, and the tick column slides 8px in the travel direction for 180ms on every page change (skipped under `prefers-reduced-motion`).
  4. **Light-mode tick fix**: resting ticks use `bg-[rgb(var(--foreground))]` instead of `bg-white/80`, which was invisible on the light theme.
  5. **Preview card alignment (`DESIGN.md` §2, §4.2, §5)**: border moved to `--border`/0.08, shadow reduced to the `0 1px 3px` micro-elevation, title set to the 13px scale step, description to the 11px floor, and `CARD_WIDTH` corrected to the actual 240px popover width.
  6. **Spec**: recorded history-access and preview-surface rules in `docs/specs/design-spec.md` §9.
  7. **Tests**: added `app/src/test/chapterScrubberWindow.dom.test.tsx` (6 cases: window size, older/newer paging, end-state disabling, snap-to-newest on new turn, token-based tick colour, no chevrons when all turns fit).
- **Verification**:
  - `cd app && pnpm exec vitest run src/test/chapterScrubberWindow.dom.test.tsx`: SUCCESS (6/6).
  - `cd app && pnpm test`: SUCCESS (29 suites, 242 tests).
  - `cd app && pnpm lint:arch`: SUCCESS.
  - `cd app && pnpm exec eslint` on changed files: 0 errors (2 pre-existing warnings in `Home.tsx`, present on `HEAD`).
  - `cd app && node scripts/stress_drawers.mjs`: completed, 0 runtime exceptions.
  - `pnpm tsc --noEmit`: changed files clean; blocked by a pre-existing unused `useEffect` import in `ControllerRemoteDeskPanel.tsx`.


- **Remote Discovery, mDNS & UI Stability (2026-10-28 – 2026-10-29):**
  - Resolved mDNS RFC 6762 trailing dot and RFC 1035 63-octet label limits in `advertiser.rs` and `browser.rs`.
  - Implemented Android `MulticastLockManager.kt` and RAII guard in `multicast.rs`.
  - Added integration tests in `remote_connection_test.rs` covering authentic `dial_and_connect` and candidate failover.
  - Fixed `ControllerRemoteDeskPanel.tsx` failed-link retention, Home error banner positioning, and restored Orb brightness.
  - Cleaned direct `crate::` imports to top-level imports across core services.

## 2026-10-31 — LFM3-Audio-2B integration feasibility research

- Researched LiquidAI/LFM3-Audio-2B-GGUF (model card, llama.cpp b11960 support, /v1/realtime WebSocket).
- Downloaded and tested Q4_K_M + Q4_0 quants with mmproj Q8_0/Q4_0 on CPU (i5-1145G7, no GPU).
- Measured: TTS 0.04–0.45x realtime; realtime TTFB 6.95s (Q4_K_M, 4 threads), RTF 0.20; text-only 4.15s. Latency gate FAILED on CPU.
- Wrote feasibility report: docs/plans/phase14/lfm3-audio-integration-feasibility.md (recommendation: do not integrate as realtime provider now; implement OpenAiRealtime provider independently).
- Added probe example: app/src-tauri/examples/lfm3_audio_probe.rs (unverified compile due to time).
- Fixed Remote Control settings propagation and session pause lifecycle:
  - Root-caused controller disconnect pause: conditionalized `pause_session_fn` on host (`state.pipeline.state() != InteractionState::Idle`) and `handle_controller_involuntary_cutoff` / `disconnect_controller_initiated` on controller (`is_session_active()`). Disconnecting when session was ended remains in `Idle`.
  - Root-caused settings patch rejection: `apply_settings_patch` takes `{ patch: ... }` rather than top-level `{ domain, key }`. In `routing.rs`, mapped `apply_settings_patch` to `CommandClass::Split`. In `gateway.rs`, verified all subkeys in patch rather than rejecting with `Denied`. In `lib.rs`, split patches between local (appearance) and remote (pipeline/speech/llm) keys.
  - Root-caused speech model cards showing uninstalled/download prompt: `manage_models` was classified as `Local` in `routing.rs`. On Android, local model directory is empty. Reclassified `manage_models` to `CommandClass::Remote` and added `dispatch_manage_models` in `gateway.rs` to route model presence checks and actions to host storage.
  - Verified with `./scripts/verify-targets.sh` (0 warnings, 0 errors on desktop and aarch64-linux-android) and passed sequential remote security and connection tests.

## 2026-10-31 — Remote Architecture Decoupling & Manager Deconstruction

- **Deconstructed `manager.rs`**:
  - Shrunk from 1,960 LOC to 814 LOC across 4 focused modlets:
    - `session/pairing_client.rs` (193 LOC): Noise_XX Tier 1 pairing handshake, token verification, and DB persistence.
    - `session/dialer.rs` (311 LOC): IP candidate resolution, mDNS discovery fallback, Noise initiator handshake, and rejection classification.
    - `session/supervisor.rs` (615 LOC): Ping/Pong heartbeat loop, RPC routing, socket fault recovery, and reconnect backoff.
    - `session/forwarder.rs` (149 LOC): Telemetry rate limiting and streaming token coalescer.
- **Eliminated `gateway.rs` Duplication**:
  - Created unified IPC dispatcher in `app/src-tauri/src/ipc/dispatcher.rs` (442 LOC) servicing both local and remote invocation.
  - Shrunk `services/remote/control/gateway.rs` from 855 LOC to 83 LOC by delegating RPC dispatch directly to `ipc::dispatcher::dispatch_command`.
- **Pruned Shadow Events**:
  - Deleted dead `RemotePairingRequested` event; standardized exclusively on `RemotePairingStateChanged(AwaitingApproval)`.
  - Updated `app/src/store/remoteStore.ts` and test suite `remoteStore.test.ts`.
  - Updated specs `docs/specs/ipc-spec.md` and `docs/specs/remote-control-spec.md`.
- **Decoupled Appearance & Removed Patch Splitting**:
  - Removed obsolete `remote.execution_mode` config field across backend and frontend.
  - Added dedicated local IPC command `set_appearance` in `ipc/settings.rs` and `settingsService.ts`.
  - Reclassified `apply_settings_patch` as purely remote (`CommandClass::Remote`) when connected.
  - Completely removed patch-splitting logic (`split_settings_patch`, `handle_controller_settings_patch`) from `lib.rs`.
- **Verification**:
  - Compile gate: `./scripts/verify-targets.sh` passed with 0 warnings / 0 errors on desktop and Android (`aarch64-linux-android`).
  - Rust tests: `remote_security_test` (20/20 passed), `remote_connection_test` (15/15 passed), `remote_link_test` (18/18 passed).
  - Frontend tests: `pnpm test` (31/31 suites passed, 253/253 tests passed).

---

## 2026-11-02 — Remote Control Audio Pipeline, Host Standalone Audio & VAD Decoupling

- **Root Causes Diagnosed**:
  1. **Silent Controller Playout & Inert Uplink**: `dial_secondary_audio_socket` in `supervisor.rs` established the audio WebSocket but never spawned a reader task for downlink audio frames or an uplink capture/VAD worker task. Downlink audio sat unread; microphone audio was never captured.
  2. **Dead Host Standalone Audio**: In `engine.rs`, `if is_remote { (tx, None, None, None) }` bypassed initialization of host AEC and VAD workers, immediately dropping `vad_rx`. Any transition to standalone mode left the hardware microphone ringbuffer overflowing (>25,000 chunks dropped) with `vad_tx` reporting "sending on a closed channel".
  3. **Silent VAD Fallback**: Missing ONNX model files fell back silently to `Earshot` without surfacing an alert to the user.
  4. **Coupled `manage_models` Routing**: `manage_models` was routed exclusively to the host, preventing the controller from querying and downloading local VAD models required for edge VAD.
  5. **Sluggish Action Controls**: 500ms CSS transition delays created sluggish tactile feedback on the Home controls, lacking instant active press states and hybrid push-to-talk.

- **Implemented Architecture & Fixes**:
  1. **Controller Audio Runtime (`services/remote/audio/controller_runtime.rs`)**:
     - Playout: Reads 24kHz PCM16 frames from secondary audio WebSocket, upsamples 2x to 48kHz, and feeds local CPAL output stream.
     - Capture & Edge VAD: Captures 16kHz microphone audio, evaluates frame speech probability via `VadBackend`, tracks onset (3 frames = 48ms) and hangover (25 frames = 400ms), and transmits `SpeechStart`, raw audio frames, and `SpeechEnd` over `NetworkAudioSender`.
     - Integrated in `supervisor.rs` upon secondary audio socket connection.
  2. **Host Local Audio Pipeline Lifecycle (`core/engine.rs`)**:
     - Retained AEC and VAD workers permanently alive regardless of remote status.
     - In remote mode, hardware microphone capture is paused; upon disconnect, `audio_stream.start()` resumes local processing with zero dropped ringbuffer frames or dead channels.
  3. **Non-Silent VAD Fallback (`services/vad/factory.rs`, `core/engine.rs`)**:
     - Extended factory return signature to `VadInitOutput { backend, fallback_from, fallback_reason }`.
     - Emits a transient warning notification (`VadFallback`) on desktop and controller when a model falls back to `Earshot`.
  4. **Decoupled `manage_models` Command Routing (`services/remote/control/routing.rs`, `lib.rs`)**:
     - Reclassified `manage_models` as `CommandClass::Split`.
     - Intercepts VAD models (`silero_vad`, `ten_vad`, `earshot`) for local controller storage, while forwarding STT, TTS, and LLM models to the host.
  5. **Instant Visual Feedback & Hybrid Push-to-Talk (`HomeControls.tsx`)**:
     - Reduced transition latency to 150ms and introduced immediate 0ms visual press feedback.
     - Implemented hybrid PTT: tap (<300ms) toggles latched recording; hold (≥300ms) records continuously until pointer release.
  6. **Specification Alignment**:
     - Updated `docs/specs/remote-control-spec.md` (§7.6 Controller Audio Runtime, §8.1 `manage_models` Split routing).
     - Updated `docs/specs/ipc-spec.md` with `manage_models` Split behavior.

- **Verification**:
  - Backend compile gate (`./scripts/verify-targets.sh`): SUCCESS (0 warnings / 0 errors across architecture invariant lint, desktop lib/bin, desktop tests/benches, and Android `aarch64-linux-android`).
  - Frontend test suite (`cd app && pnpm test`): SUCCESS (31 test files, 255 tests passed).

---

## 2026-11-07 — Test suite hardening closeout (follow-up)

- **Code-quality/style fixes (testing-style-guide/create-test):** replaced swallowed `let _ = *.join()` with `expect`/explicit panic reporting in changed tests: `app/src-tauri/tests/e2e_voice_pipeline_test.rs` (state watcher joined), `vad_segmentation_test.rs` (2 watcher joins), `transcript_ingestion_test.rs` (Drop joins), `llm_to_tts_test.rs` (shutdown joins), `harness_loop_test.rs` (Drop join). Joined E2E `mid_thought` state watcher after drain. Made E2E `sub_300ms_cough_recovers_to_ready_without_deadlock` drain/pump and `exit(0)` consistent with F-004 Kokoro teardown workaround.
- **Repo hygiene:** staged previously untracked `app/src-tauri/tests/e2e_voice_pipeline_test.rs`; added `qa-runs/` to `.gitignore` (stale QA artifacts must not be committed as evidence).
- **Verified this session (local, release, sequential):** `./scripts/verify-targets.sh desktop` — clean (0 warnings/errors). `./scripts/test.sh default` reached `dictation_output_test` with **1 env-sensitive failure unrelated to this diff** (`output_router_reports_a_typed_error_without_a_display`, `headless()`/unreachable `DISPLAY=localhost:10.0` X11 clipboard path; file unmodified). Remaining default-tier binaries run individually after that point: `transcript_ingestion_test`, `agentic_tool_loop_test`, `harness_loop_test`, `remote_link_test` (1 pre-existing skip), `remote_security_test` — **65 passed, 1 skipped**.
- **Targeted models-tier verification (RAYON/OMP=8, `VOX_MODELS_DIR=/root/.vox/models`):** `e2e_voice_pipeline_test::mid_thought_continuation_combines_both_bursts` — **1/1 PASS, 16.574s** (3 skipped). `llm_to_tts_test` — **4/4 PASS, 1 leaky (50.623s)** including `harness_turn_streams_clauses_to_tts_and_commits_history` through fixed UDS sidecar (F-002 path). Leaked `vox-llm` test sidecars were cleaned before E2E retry.
- **Still open (not falsely closed):** full `./scripts/test.sh models` sweep; Stage 2 QA/`qa` skill fresh timestamped artifact run + human verdict (stale `qa-runs/20261107-172430/report.md` ignored). `dictation_output_test` headless/display assumption may need a separate robustness fix in canonical env.

## 2026-11-07 — Full gates closed (models tier + QA artifacts) & Seam Re-audit
- **Full models tier:** EXIT 0, 0 FAIL lines. All non-remote binaries passed in sorted order.
- **Findings summary:** F-001 open (ERLE); F-002 fixed; F-003 open; F-004 open (ORT teardown leak workaround); F-005 opened.
- **Verified:** `verify-targets.sh` PASS (0 warnings/errors); AEC ERLE 3/3; double_talk PASS; E2E 3/3 + 1 skip (19.45 s).

---

## 2026-11-08 — Backend fixes (AEC + sidecar cancel)
- **F-001 partial:** raised `DEFAULT_STREAM_DELAY_MS` 40→120; un-ignored `aec_suppresses_the_synthetic_tts_echo` (3/3 PASS) and `double_talk_barge_in_cancels_the_playback_and_opens_listening` (PASS).
- **F-005:** wired `sse_tx.blocking_send` failure -> `cancel.cancel()` in `sidecars/vox-llm/src/main.rs`.
- **Verification:** `verify-targets.sh` PASS (0 warnings/errors); AEC ERLE 3/3 and double_talk PASS; E2E 3/3 + 1 skip (19.45 s).

---

## 2026-11-08 — Home Recall Spatial Transformation & SessionPanel Polish

- **Fixes Implemented**:
  1. **Spatial Layout Transformation (`pages/Home.tsx`, `SearchRecallView.tsx`)**:
     - Removed abrupt opacity screen unmount/swap. Replaced with physical spring spatial coordinate transformations (`type: "spring", stiffness: 320, damping: 28`).
     - Voice controls glide down (`y: 80, opacity: 0, scale: 0.92`); status badge glides up (`y: -40, opacity: 0`); side dialogue slides right (`x: 60, opacity: 0`).
     - Search header glides down smoothly from above the Orb.
     - 3D Arc Carousel glides up from below into its anchored position overlapping the bottom third of the Orb (`top: calc(50% + clamp(64px, 8.5vh, 80px))`).
     - Orb stays anchored at natural scale (1.0, no awkward shrink) with atmospheric celestial rising glow and chronos rings behind the cards.
  2. **SessionPanel First-Mount Lag & Slit Elimination (`SessionPanel.tsx`)**:
     - Progressive batching for reverse-chronological session list (`displayedChronologicalSessions` initially 25 items, background-expanding after drawer slide settles) eliminates main-thread freeze on open.
     - Removed `pb-3` between sections and removed bottom divider line (`border-b`) under sticky headers.
     - Added `-mt-px` on scroll container for seamless zero-gap sticky header alignment.
  3. **Scrubber Histogram & Layout Tightening (`RecallTimelineScrubber.tsx`, `SearchRecallView.tsx`)**:
     - Removed `<` and `>` chevron buttons.
     - Implemented vertical histogram bars scaling dynamically (7px to 26px) based on turn density and session count.
     - Pulled search bar, orb, carousel arc, and scrubber into a cohesive focal composition.
  4. **Drag Clamping & Active Border Sync (`RecallArcCarousel.tsx`, `RecallSessionCard.tsx`)**:
     - Hard-clamped carousel bounds to `maxSelectableIndex`. Center card defaults to latest session; scrolling right into skeletons is strictly blocked. Users can only scroll backward into past sessions.
     - Synchronized center card active border highlight continuously inside the RAF loop (`Math.round(offset)`) with zero pointer-up delay.
     - Made session cards solid opaque (`bg-[rgb(var(--card))] border shadow-md`), preventing visual background bleed.

- **Verification**:
  - `pnpm --prefix app exec tsc --noEmit`: PASSED (0 errors).
  - `pnpm --prefix app lint:arch`: PASSED (0 invariant violations).
  - `pnpm --prefix app test`: PASSED (32 test files, 258 tests passed, 100% green).
  - `cd app/src-tauri && ./scripts/verify-targets.sh`: Both desktop and Android targets compile with 0 warnings / 0 errors.

---

## 2026-11-10 — Post-Review Clean-Sweep ZBC Refactor (History Fold into Home)

- **Audit Findings Implemented (Zero Backward Compatibility)**:
  1. **Fixed Unsafe Optional Chaining (🔴 T2)**: Restored safe navigation `connectedPeer?.displayName?.trim()` in `Home.tsx` and `settings?.memory?.private_mode` in `AppHeader.tsx`.
  2. **Decoupled SessionPanel Navigation (⚖️ T3)**: Removed `useNavigate` from `SessionPanel.tsx`. Replaced with explicit `onSearch: () => void` prop passed from `Home.tsx` (`enterSearchMode`), respecting component boundary.
  3. **Extracted `useDetailPanelTurns` (🔴 T2)**: Created unified hook with active/unmount cancellation guards for initial fetch and retry. Eliminated duplicate fetch/state blocks in `Home.tsx` and `useRecallSearch.ts`.
  4. **SessionPanel Progressive Rendering (💡 T2)**: Replaced arbitrary 180ms timer with `requestIdleCallback` (with clean fallback) for smooth progressive rendering of reverse-chronological sessions.
  5. **Pruned Dead History Code & Shims (ZBC)**: Deleted legacy `app/src/shared/hooks/useHistory.ts`, `app/src/store/historyFilterStore.ts`, dead `viewMode`/`setViewMode` in `sessionPanelUiStore.ts`, and old `shared/components/history` directory.
  6. **Unified Projects Section (🐛 T1)**: Removed duplicate header and dead internal/external props in `SessionProjectsSection.tsx`. State is now cleanly passed from `SessionPanel.tsx`.
  7. **Extracted `SearchModeOrbHalo` (🐛 T1)**: Isolated decorative celestial rings into dedicated component.
  8. **Centralized Static Copy (🐛 T1)**: Moved all hardcoded strings in `SearchRecallView.tsx` into `SESSION_COPY` in `app/src/data/sessionCopy.ts`.
  9. **Added Unit Tests**: Created `app/src/test/useDetailPanelTurns.test.ts` covering initial mount, fetch, error, retry, and in-flight unmount cancellation.

- **Verification**:
  - `pnpm --prefix app exec tsc --noEmit`: PASSED (0 errors).
  - `pnpm --prefix app lint:arch`: PASSED (all architectural invariants hold).
  - `pnpm --prefix app test`: PASSED (33 test files, 262 tests passed, 100% green).
  - `cd app/src-tauri && ./scripts/verify-targets.sh`: Desktop and Android targets compile with 0 warnings / 0 errors.

---

## 2026-11-10 — Test Hardening Findings Remediation & Invariant Fixes

- **AEC & Barge-In Echo Suppression Remediation (F-001)**:
  - In `app/src-tauri/src/services/audio/aec.rs`, expanded `MAX_RENDER_QUEUE_BACKLOG_SAMPLES` from 3,200 to 9,600 samples (600 ms) to avoid queue truncation during transient buffer jitter under 120 ms hardware skew.
  - In `app/src-tauri/src/services/vad/segmenter.rs`, enforced `MIN_SPEAKING_ONSET_GATE_RMS = 0.038` and increased `required_onset_score` (8 frames / 128 ms) during `InteractionState::Speaking` to prevent residual echo leakage from tripping VAD while keeping authentic user barge-in responsive.
  - Un-ignored `solo_playback_over_six_seconds_never_interrupts_with_pipewire_delay` and `double_talk_barge_in_cancels_the_playback_and_opens_listening` in `app/src-tauri/tests/audio_capture/barge_in.rs`.

- **Sidecar Continuation Cancellation & Preemption (F-005)**:
  - In `app/src-tauri/src/services/sidecar/supervisor.rs`, implemented `SidecarSupervisor::kill(&self, kind: SidecarKind)` for clean process teardown and socket cleanup.
  - In `app/src-tauri/src/services/llm/sidecar.rs`, refactored SSE streaming loop to use `tokio::select!` on `cancel.cancelled()`, immediately terminating the child process to release C++ inference worker threads and prevent continuation turn starvation.
  - Un-ignored `mid_thought_continuation_combines_both_bursts` in `app/src-tauri/tests/e2e_voice_pipeline_test.rs`.

- **App State Initialization Race Fix**:
  - Replaced unchecked `app.state::<Arc<AppState>>()` calls with `app.try_state::<Arc<AppState>>()` in `core/state.rs`, `pipeline/assistant/transcript.rs`, and `pipeline/assistant/session.rs`, eliminating the `state() called before manage()` panic during test rig startup.

- **Verification**:
  - `cd app/src-tauri && ./scripts/verify-targets.sh`: Both desktop and Android targets compile with 0 warnings / 0 errors (`clippy --release`).


---

## 2026-11-10 — Search/Recall Mode: Perf Fixes + UI Polish

**Context**: Lead review audit of Search Mode identified 3 P0 perf regressions, 4 layout violations, and 2 hardcoded colour values. All 9 items fixed.

### Files Changed

| File | Change |
|---|---|
| `app/src/pages/Home.tsx` | Pass `paused={isSearchMode || ...}` to VoxOrb — suspends WebGL RAF during search |
| `app/src/shared/components/home/recall/SearchModeOrbHalo.tsx` | Removed `blur-xl + animate-pulse`. Replaced with `vox-halo-breathe` keyframe (opacity-only, no composite). Hardcoded `rgba(168,85,247,…)` → `rgba(var(--accent-complementary),…)` |
| `app/src/index.css` | Added `@keyframes vox-halo-breathe`. Added `--accent-complementary: 132, 0, 233` to `:root` as pre-hydration default for cyan accent |
| `app/src/shared/components/home/recall/RecallSessionCard.tsx` | `backdrop-blur-md` now only applied to the active (centre) card. Side cards use solid `bg-[rgb(var(--card))]/80` — eliminates 8 extra blur compositing layers per frame |
| `app/src/shared/theme/index.ts` | Added `complementaryTriplet()` + `applyComplementaryToken()`. Wired into all 3 `--accent` write sites: `writeThemeToDom`, interpolation path, and `previewAccent`. Token is now dynamic and tracks every accent change including live drag |
| `app/src/shared/components/home/recall/SearchRecallView.tsx` | (1) "Back to Voice Mode" pill → simple ghost text button (removes competing glass surface + backdrop-blur). (2) Search bar focus ring: hardcoded `violet-400` → `rgba(var(--accent-complementary),0.6)`. (3) Scrubber bottom padding: `5.25rem` → `5.75rem` (+8px clearance from nav) |
| `app/src/shared/components/home/recall/RecallTimelineScrubber.tsx` | `transition-all` on 40 bars → `transition-[left]` on wrapper and `transition-[height,background-color,box-shadow]` on bar — reduces property set checked each frame |

### Verification
- `npx tsc --noEmit`: exit 0, no errors

---

## 2026-11-10 — Search/Recall Mode: Deep Polish, Color Reverse-Engineering, Coverflow Overhaul & Navigation Persistence

**Context**: Addressed follow-up feedback on Search Mode:
1. Reverse-engineered the pink-with-purple cosmic glow from the screenshot (`#f291d4` pink + `168, 85, 247` purple).
2. Restored feathered subtle glow with `blur-xl` and 6s slow breathe in `SearchModeOrbHalo.tsx`.
3. Paused `PipelineField` and `AmbientBackground` ripples during search mode for 100% smooth UI.
4. Re-anchored search bar (directly above orb rim, matching `DynamicStatusBadge`) and timeline scrubber (matching `HomeControls` action button coordinate slot `calc(80% - 58px + ...)`).
5. Overhauled `RecallArcCarousel.tsx` with a fluid Coverflow engine: continuous trackpad/wheel scrolling, weighted moving-average flick momentum, rubber-band boundary elasticity, and zero-reconciliation direct DOM writes.
6. Implemented `useRecallSearchStore` (Zustand) so Search Mode, active query, sort order, and selected card index persist seamlessly across page navigation without resetting.
7. Updated `search-mode-ui-audit.md` report artifact.

### Files Changed
- `app/src/pages/Home.tsx`
- `app/src/layout/ResponsiveLayout.tsx`
- `app/src/shared/components/home/PipelineField.tsx`
- `app/src/shared/components/common/AmbientBackground.tsx`
- `app/src/shared/components/home/recall/SearchModeOrbHalo.tsx`
- `app/src/shared/components/home/recall/SearchRecallView.tsx`
- `app/src/shared/components/home/recall/RecallArcCarousel.tsx`
- `app/src/shared/components/home/recall/useRecallSearch.ts`
- `app/src/store/recallSearchStore.ts`
- `app/src/shared/theme/index.ts`
- `app/src/index.css`

- **DetailPanel Transcript Full Width Fix**: Removed artificial `max-w-3xl mx-auto` constraint from turn container in `DetailPanel.tsx` and updated assistant response bubble to `w-full`, allowing conversation transcripts to span the full width of the drawer.
