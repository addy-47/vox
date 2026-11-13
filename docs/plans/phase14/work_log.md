# Phase 14 Work Log

## 2026-10-26 – 2026-10-31 — Remote Control Subsystem Architecture, IPC Dispatcher & Hardening (Compacted)

- **Architecture & Modlet Deconstruction (`services/remote/`)**:
  - Deconstructed monolithic `manager.rs` (1,960 LOC) into focused submodules: `pairing_client.rs`, `dialer.rs`, `supervisor.rs`, and `forwarder.rs`.
  - Created unified IPC command dispatcher in `ipc/dispatcher.rs` and shrunk `services/remote/control/gateway.rs` from 855 LOC to 83 LOC.
  - Standardized on canonical `RemoteLinkState` (`Unpaired`/`Paired`) and `PairingState`, pruning shadow events.
  - Dedicated `set_appearance` IPC command; removed patch-splitting logic from `lib.rs`.
- **Protocol, Security & Mobile UI**:
  - Enforced Noise_XX mutual authentication, token replay protection, and candidate IP failover in `remote_connection_test.rs` and `remote_security_test.rs`.
  - Restricted authorization prompts strictly to desktop host; mobile controller displays waiting state (`QrScannerModal.tsx`).
  - Added Android multicast lock management in `MulticastLockManager.kt` and `discovery/multicast.rs`.
- **Verification**: `./scripts/verify-targets.sh` clean across desktop and `aarch64-linux-android`; 255+ frontend tests and 44+ remote integration tests green.

---

## 2026-10-26 – 2026-10-30 — Remote Control v1 Foundation, Wire Protocol & Discovery (Compacted)

- **Foundation & Protocol (`services/remote/`)**:
  - Implemented Noise_XX handshake, ephemeral audio token validation, and candidate IP failover in `discovery/browser.rs` and `discovery/advertiser.rs`.
  - Added Android multicast lock management via Kotlin `MulticastLockManager.kt`.
  - Wired real command gateway with default-deny policy, FIFO event forwarder, and centralized `RULE-RC-GUARD` desktop invoke wrappers.
- **Voice Pipeline Audit & Seams**:
  - Mapped runtime trace across 5 sequential stages (`voice_pipeline_scenarios_trace.md`) and audited magic constants (`dynamic_logic_and_magic_numbers_audit.md`).
  - Implemented VAD sub-threshold empty final transcript fallback to resolve `Thinking` deadlock.
  - Migrated vector search to Turso native vector pushdown in `persistence/facts.rs`.
- **Verification**: `./scripts/verify-targets.sh` clean on desktop + `aarch64-linux-android`; 32/32 remote connection/security tests passed.

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

## 2026-11-10 to 2026-11-12 — Search Mode Halo Masking, Underline Search Bar & Coverflow Physics (Compacted)

- **Halo Masking & Subtle Aura (`SearchModeOrbHalo.tsx`)**: Replaced square container mask with unconstrained `rounded-full` fragment layers. Shifted radial-gradient origin to `50% 36%` with soft falloff at `62%`. Applied vertical alpha gradient mask to terminate glow cleanly above the scrubber.
- **Underline Search Bar & Session Compaction (`SearchRecallView.tsx`, `RecallSessionCard.tsx`)**: Aligned search input with frameless Memory page underline style. Embedded compact variant button for uncompacted turn counts; added `"uncompacted"` sort and filter menu option.
- **Bidirectional Scrubber Sync & Physics (`RecallArcCarousel.tsx`, `RecallTimelineScrubber.tsx`)**: Added `onLiveIndexChange` with gesture locks (`draggingRef`, `isWheelingRef`). Replaced exact-index matching with nearest-bar resolution.
- **Verification**: `pnpm lint:arch` PASSED; Vitest 11/11 tests passed; `stress_drawers.mjs` (0 exceptions, 60 FPS); `soak_test.mjs --quick` (0 DOM delta).

## 2026-11-10 — Recall Coverflow 60 FPS Physics, Glow Feathering & Lead Review Hardening

- **Celestial Rising Semi-Sphere Visual Transformation (`Home.tsx`)**:
  - Implemented dynamic vertical gradient alpha mask (`linear-gradient(to bottom, black 0%, black 35%, rgba(0,0,0,0.65) 50%, rgba(0,0,0,0.2) 65%, transparent 80%)`) on `OrbitField` & `VoxOrb` during Search Mode.
  - The Orb visualizes as a rising celestial object from behind the horizon of session cards, eliminating the jarring appearance of cards placed over a solid background sphere.
- **Search & Drawer Transition Decoupling (`SessionPanel.tsx`, `Home.tsx`, `projectsService.ts`, `useSessionPanel.ts`)**:
  - Prewarmed `getSessions()` and `getProjects()` in `Home.tsx` during idle (50ms), ensuring caches are hot before the user opens any panel.
  - Exported `getCachedProjects()` from `projectsService.ts` and seeded `useSessionPanel` initial state synchronously.
  - Deferred `refresh()` in `useSessionPanel` until 240ms (after the 220ms drawer slide rests) when cache is warm, preventing mid-slide layout thrashing.
  - Decoupled `onClose()` and `onSearch()` in `SessionPanel.tsx` via `requestAnimationFrame()`, eliminating the mini-freeze when entering search mode.
- **Deterministic UI Suite Execution (`review-ui`)**:
  - Updated `stress_drawers.mjs` and `soak_test.mjs` to target `/?mode=search` instead of legacy `/history`.
  - `pnpm lint:arch`: PASSED (all architectural invariants hold).
  - ESLint on `shared/components/home/recall/`: 0 errors / 0 warnings.
  - `stress_drawers.mjs`: PASSED (0 exceptions, Net Heap +1.21 MB, DOM nodes -84).
  - `soak_test.mjs --quick`: PASSED (0 net heap growth, -0.357 MB/min slope, DOM delta 0, listener delta -4).
- **Coverflow Scroll Physics & Memo Stabilization (`RecallArcCarousel.tsx`, `RecallSessionCard.tsx`)**:
  - Decoupled `handleWheel` from immediate integer snapping (`isWheelingRef`), allowing fluid trackpad and wheel momentum with snapping only on rest.
  - Wrapped `onSelect` in referentially stable `handleCardSelect` callback, protecting `React.memo` across all non-active cards on integer steps.
  - Replaced layout-thrashing `display: none` / `block` toggling inside RAF with `visibility: hidden` and static `will-change: transform, opacity`.
  - Replaced `transition-all` on cards with targeted `transition-[border-color,box-shadow,background-color] duration-150`.
  - Preserved the existing 3D arc curvature (`recallArcMath.ts`).
- **Glow Blur Optimization & Atmospheric Halo (`SearchModeOrbHalo.tsx`)**:
  - Preserved user-requested `blur-xl` feathered cosmic glow (`--accent` + `--accent-complementary`).
  - Removed infinite opacity breathe animation and continuous spin keyframe during scroll, isolating the glow to a static hardware layer (`transform-gpu`, `translateZ(0)`).
- **Natural Timeline Sorting & Skeleton Removal (`useRecallSearch.ts`)**:
  - Removed fake pulsing skeleton cards.
  - "Newest first" now sorts newest-first (`b.created_at - a.created_at`) with index 0 (newest session) centered, and older conversations progressing naturally to the right.
  - Connected existing backend Turso `searchHistory()` with 150ms debounce for deep search across titles and turns.
- **SessionPanel Lag & Drawer Stabilization (`useSessionPanel.ts`, `SessionPanel.tsx`, `Home.tsx`)**:
  - Seeded `useSessionPanel` immediately with `getCachedSessions()`.
  - Deferred chronological session expansion to 260ms (after the 220ms drawer slide completes) to maintain 60 FPS during open gestures.
  - Added `overscroll-contain` to session scroll container.
  - Guarded `<DetailPanel>` in `Home.tsx` with `!isSearchMode` to eliminate redundant drawer mounts.
- **Scrubber & Design Floor Alignment (`RecallTimelineScrubber.tsx`, `DetailPanel.tsx`, `SearchRecallView.tsx`)**:
  - Fixed duplicate month keys across years (`${m.year}-${m.label}`).
  - Decoupled bar geometry from `activeIndex` in scrubber memo.
  - Aligned all `10px` and `10.5px` micro-copy to the Vox design system 11px floor (`text-2xs` / `text-[11px]`).
- **Verification**:
  - `pnpm exec tsc --noEmit` passed with 0 errors across entire workspace.
  - Vitest passed 11/11 tests (`DetailPanel.dom.test.tsx`, `historyEmptyState.regression.test.ts`, `useDetailPanelTurns.test.ts`).
  - `./scripts/verify-targets.sh` passed 100% (0 warnings / 0 errors across desktop and Android).

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

- **Search Mode Lead Review Audit (2026-11-10)**: Addressed 9 items: suspended WebGL RAF during search, replaced `blur-xl` with opacity-only `vox-halo-breathe`, added dynamic `--accent-complementary`, isolated card `backdrop-blur-md` to center active card, anchored search bar above orb rim, and decimated scrubber to 40 bars max.
- **DetailPanel Full Width Fix**: Removed artificial `max-w-3xl mx-auto` constraint, allowing conversation transcripts to span the full drawer width.

---

## 2026-11-10 — Test Suite Update: Voice Pipeline Simplification & Smart Turn-Taking

**Role:** Test Engineer  
**Context:** Synchronized integration test suite with backend voice pipeline refactoring (removal of heuristic noise floor tracking and conjunction string checking; addition of `SmartTurnDetector` and Tier 1A graceful fallback).

### Changes
1. **`app/src-tauri/tests/common/harness.rs`**:
   - Stripped `incomplete_clause_hint` from `setup_vad_actor`, `wire_audio_scene`, and `VadActorHandles`.
   - Removed `incomplete_clause_hint: Arc<AtomicBool>` from `AudioSceneRig`.
   - Removed `incomplete_clause_hint` and conjunction emitter closure from `E2eVoiceRig`.
   - Removed `incomplete_clause_hint_emitter` helper function.
2. **`app/src-tauri/tests/vad_segmentation_test.rs`**:
   - Removed all references to `incomplete_clause_hint` and `shared_hint` in `RecordingStt` and `spawn_vad`.
   - Updated Subtest C: Replaced obsolete `EARLY_UTTERANCE_MIN_END_FRAMES` (800ms) with standard silence duration testing (400ms default), asserting that an intra-utterance pause of 250ms is retained within a single segment.
   - Removed trailing hint argument in `silero_vad_segments_speech_and_triggers_auto_silence_stop`.
   - Added `smart_turn_detector_gracefully_degrades_when_model_absent` to verify Tier 1A graceful degradation to `predict_eou -> Ok(None)` when model weights are not present.
3. **Callsite Parity**:
   - Updated `setup_vad_actor` callsites in `dictation_ptt_test.rs` and `playback_test.rs`.
4. **Findings Recorded (`docs/tests/test-findings.md`)**:
   - F-006: Type conversion & tuple shape errors in `turn_detector.rs` (fixed by backend engineer).
   - F-007: 3 clippy lints in `src/services/vad/{segmenter,turn_detector}.rs` pending backend cleanup.

### Verification
- `cargo test --test vad_segmentation_test --release`: **3/3 passed (49.89s)** (graceful degradation, auto silence stop, passive streaming suppression).

---

## 2026-11-11 — Voice Pipeline Simplification, Smart Turn-Taking & Echo Onset Stabilization

- **Voice Pipeline Simplification (`services/vad/`, `core/engine.rs`)**:
  - Eliminated ~185 lines of brittle heuristic logic, conjunction string matching (`" and"`, `" or"`, `" but"`, `"और"`, `"के"`), and 50+ manual constants across the VAD and STT pipeline.
  - Integrated `SmartTurnDetector` (`services/vad/turn_detector.rs`) loading the `smart_turn.onnx` acoustic EOU classifier (pipecat-ai/smart-turn-v3 INT8) with zero-cost Tier 1A graceful fallback when the model file is not present.
  - Registered `smart_turn` in `manifests/models_manifest.json` under the `vad` model family.
  - Removed `incomplete_clause_hint` atomic flag across engine, benchmarks, actors, and test harnesses.
- **AEC Speaking Onset Floor & Barge-In Stabilization (`services/vad/segmenter.rs`)**:
  - Remediated residual echo leakage during `InteractionState::Speaking` while WebRTC AEC3 converges.
  - Enforced a minimum speaking onset gate floor of `0.038` RMS: cleanly rejects residual loudspeaker echo transients (0.020–0.035 RMS) while preserving fast barge-in response for real user speech (0.063–0.207 RMS).
  - Aligned speaking onset confirmation score requirement: `(base_start_frames * 2.0).max(4.0)`.
- **Verification**:
  - Dual-target compile gate (`./scripts/verify-targets.sh`): Desktop clippy/tests/binary + Android (`aarch64-linux-android`) clippy passed with **0 warnings / 0 errors**.
  - Architectural lint (`./scripts/lint-arch.sh`): PASSED (all backend architectural invariants hold).
  - `audio_capture_aec_test`: 7/7 tests passed 3 times consecutively (0 flakes: suppression, bypass, chunk pass-through, tap downsampling, echo interruption, barge-in cancellation, 6s solo playback).
  - Full test suite (`cargo nextest run --release --test-threads=1`): **510 tests run, 510 passed, 0 failed, 4 skipped (100% green)**.

---

## 2026-11-12 — Fix Search Render Crash, Frosted Glass Capsule & Sphere Glow (Compacted)

- **Fix Search Render Crash (`RecallTimelineScrubber.tsx`)**: Resolved `0 / 0 = NaN` division on single-match filters. Added safe fallback guards for 0-result states.
- **Frosted Glass Command Capsule (`SearchRecallView.tsx`)**: Replaced bare floating underline with a prominent frosted glass capsule. Lifted scrubber stage into the golden ratio pocket.
- **Middle Sphere Secondary Accent Glow (`SearchModeOrbHalo.tsx`)**: Extended atmosphere glow through the middle up to ~38% height.
- **Verification**: `tsc`, `lint:arch`, ESLint, and Vitest (11/11 tests) passed.

- **Temporal Calendar Scrubber (`RecallTimelineScrubber.tsx`)**: Replaced index-based bar placement with true calendar month segmentation. Active month accent illumination with zero label overlap.
- **SessionPanel Seam & Slivers Elimination (`SessionPanel.tsx`)**: Gated pinned section strictly behind `pinnedSessions.length > 0`. Reduced `pb-2` to `pb-1.5` on actions container.
- **Verification**: `tsc`, `lint:arch`, ESLint, and Vitest (11/11 tests) passed.

## 2026-11-12 — Disappearing Orb RCA, Horizon Glow Breathe & Freeze-Free Search Transition

- **Disappearing Orb Bug RCA & Fix (`AdvancedOrb.tsx`, `Home.tsx`)**:
  - **Root Cause Confirmed**: When mounting `Home.tsx` while in Search Mode (`?mode=search`), `Home.tsx` previously passed `paused={isSearchMode || ...}` to `VoxOrb`. This forced `useDynamicFPS` to pause rendering immediately upon component mount before `tickFn` could run even once. Consequently, `renderer.render(scene, camera)` and `setEntered(true)` were never called, leaving the WebGL canvas unpainted and styled at `opacity: 0`. Furthermore, after 3 seconds of calm idle in Search Mode, `isSettledIdle` timed out to `true`; because `interactionState` remained `"Idle"` when switching back to Voice Mode, `isSettledIdle` never reset to `false`, permanently blocking the RAF loop until a fresh route navigation in Voice Mode remounted the component.
  - **Remediation**:
    - Removed `isSearchMode` from `paused` on `VoxOrb` in `Home.tsx`. In Search Mode, the Orb runs at calm `fpsIdle: 12` (sub-1% CPU) and naturally settles after 3 seconds.
    - Added an immediate static frame render (`renderer.render(scene, camera)`) and `setEntered(true)` immediately following scene context instantiation in `AdvancedOrb.tsx`, guaranteeing the canvas is never blank or transparent on initial mount regardless of initial pause state.
    - Updated `isSettledIdle` effect in `AdvancedOrb.tsx` to guard on `paused`, ensuring that resuming from paused state clears `isSettledIdle` and permits 3 seconds of active rendering.
    - Added a `webglcontextrestored` listener to `renderer.domElement` to re-render the static scene upon WebGL context restoration.
- **Atmospheric Celestial Halo Glow & Breathe Pulse (`SearchModeOrbHalo.tsx`, `index.css`)**:
  - Centered halo radial gradient at `50% 75%` (`ellipse 85% 55%`), focusing the ethereal ambient glow tightly on the horizon between the lower hemisphere of the Orb and the session cards.
  - Extinguished glow above the 50% line (transparent from Y <= 35%), keeping the top half, search bar, and header completely clear of purple light spill.
  - Upgraded `@keyframes vox-halo-breathe` in `index.css` to a 6s ease-in-out atmospheric pulse (cycling opacity 0.50 to 0.95 and scale 0.96 to 1.03) for vivid organic depth.
- **Freeze-Free Session Panel Search Toggle (`SessionPanel.tsx`)**:
  - **Root Cause Confirmed**: When closing `SessionPanel`, `panelActive` flipped to `false`, which immediately invoked `setChronologicalRenderLimit(25)`. This synchronously destroyed and unmounted 70+ DOM session rows on the exact frame the drawer slide-out animation began, locking the main thread and freezing the page.
  - **Remediation**: Deferred collapsing `chronologicalRenderLimit` by 300ms in `SessionPanel.tsx`, ensuring the 220ms drawer slide completes with zero DOM unmounting mutations.
  - Enhanced dynamic toggle button in `SessionPanel.tsx`: renders `<Search /> Search conversations` when in Voice Mode and `<Mic /> Back to Voice Mode` when in Search Mode with high-resolution performance tracing (`[trace-search]`).
- **Verification**:
  - `cd app && pnpm exec tsc --noEmit`: PASSED (0 errors).
  - ESLint `shared/components/home/`: PASSED (0 errors).
  - `pnpm lint:arch`: PASSED (all architectural invariants hold).
  - Vitest: 11/11 tests passed (`DetailPanel.dom.test.tsx`, `historyEmptyState.regression.test.ts`, `useDetailPanelTurns.test.ts`).

## 2026-11-12 — Appearance Card Redesign & Review Remediation

- **Appearance Card Redesign (`AppearanceCard.tsx`)**:
  - Replaced tabs with quiet breadcrumb navigation (`CardTitleBar` with `Customise >` and `<`).
  - Added native full color picker (`HexColorPicker`) in a 2-column studio layout (compact picker left, mode switch & swatches right).
  - Constrained card height to match `PersonaCard` (`h-[280px] max-h-[280px]` in full-max, `h-[230px]` in full-min), preventing collision with the bottom dock (`EdgeNav`).
  - Placed `ThemeToggleButton` into the header action slot in Custom view.
  - Enforced `design-spec.md` §2.1 hairline boundaries on preset pills (`border-[rgba(var(--border),0.08)]` and `border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.08)]`); removed rings, glow shadows, and opaque inline styles.
  - Normalized all preset theme borders in `themePresets.ts` to neutral hairlines (`"255, 255, 255"` dark, `"0, 0, 0"` light).
- **Backend & Frontend Review Remediations**:
  - Backend hex validation (`config/mutation.rs`, `ipc/settings.rs`) rejecting invalid color hex inputs.
  - Persistence round-trip tests (`settings_persistence_test.rs`): 17/17 passed sequentially.
  - ZBC cleanup: atomically eliminated legacy `accent_seed` across backend, frontend types, `WizardRoot.tsx`, and `AdvancedOrb.tsx`.
  - Fixed mobile overlay stacking in `SearchRecallView.tsx` and bound `MemorySearchBar` shortcut hint to SSOT.
- **Verification**:
  - Compile gate (`./scripts/verify-targets.sh`): Desktop clippy/bin/tests + Android (`aarch64`) passed with **0 warnings / 0 errors**.
  - Vitest test suite (`pnpm vitest run`): **34 test files, 269 tests passed (100% green)**.


---

## 2026-11-13 — Appearance Card Redesign, Preset Catalog Overhaul & Full Review Remediation

- **Appearance Card Complete Redesign (`AppearanceCard.tsx`)**:
  - **Breadcrumb Navigation**: Replaced the underline tab strip with `CardTitleBar` breadcrumb action (`Customise >` in Presets view, and `<` back arrow returning to Presets from Custom view).
  - **Full Native Color Picker**: Restored `HexColorPicker` (`react-colorful`) in the Custom view with segmented selection between `Accent Color` and `Background Tone`. Supported by rAF live previews (`previewAccent` / `previewBackground`) and drag gate management (`beginAccentPreview` / `endAccentPreview` / `beginBackgroundPreview` / `endBackgroundPreview`).
  - **Eliminated In-Card Theme Toggle**: Removed duplicate `ThemeToggleButton` from the card header (already provided at the page header level).
  - **Multi-Layer Background Tone Chips**: Replaced flat, indistinguishable near-black circular swatches with multi-layered surface preview chips showing distinct border, card layer, and accent dot contrast.
- **Preset Catalog & Contrast Overhaul (`themePresets.ts`, `surfaceTokens.ts`)**:
  - Calibrated all presets (`Tokyo Night`, `Catppuccin`, `One Dark`, `Dracula`, `Nord`, `Night Owl`, `Vox Default`) for high contrast against background, card, and text tokens in both Dark and Light modes. Fixed the inverted card luminance bug in Tokyo Night light mode.
  - Added two new presets: `Rosé Pine` (Moon & Dawn) and `Solarized` (Dark & Light).
  - Consolidated palette SSOT: Moved `ACCENT_SWATCHES`, `DARK_BG_PRESETS`, and `LIGHT_BG_PRESETS` into `themePresets.ts`.
- **Backend Review & ZBC Hardening (`config/mutation.rs`, `ipc/settings.rs`, `settings_persistence_test.rs`)**:
  - **ZBC Cleanup**: Completely pruned legacy `accent_seed` across backend schemas, types, and callsites in favor of the canonical 5-field appearance shape.
  - **Hex Validation**: Implemented `is_valid_hex_color(&str) -> bool` and enforced it in `apply_appearance_mutation` and `ipc::settings::set_appearance`.
  - **Persistence Round-Trip Assertions**: Expanded `settings_reload_preserves_every_mutated_field` to assert all 5 appearance keys (`preset`, `dark_accent`, `light_accent`, `dark_bg`, `light_bg`) and invalid hex string rejection.
- **Frontend Review Remediation (`index.html`, `SearchRecallView.tsx`, `MemorySearchBar.tsx`, `settingsLayout.ts`)**:
  - **Boot Script Hoisting**: Renamed `br, bg, bb` in `index.html` to `lightR/lightG/lightB` and `darkR/darkG/darkB`.
  - **Overlay Stacking Context**: Removed redundant `z-50` wrapper in `SearchRecallView.tsx` on mobile to align with global `z-70` overlay stack; unified `isMobile` screen detection with `Home.tsx`.
  - **Shortcut SSOT**: Replaced hardcoded `"Ctrl+K"` in `MemorySearchBar.tsx` with `getShortcutById("memory.search")?.keys`.
  - **Card Width**: Calibrated `getDomainCardWidth("appearance")` in `settingsLayout.ts`.
- **Verification**:
  - `pnpm lint:arch`: PASSED (all architectural invariants hold).
  - `pnpm vitest run src/test/dom/AppearanceCard.dom.test.tsx src/test/themeTransition.regression.test.ts`: PASSED (8/8 tests).
  - `verify-targets.sh`: PASSED (desktop release and aarch64 Android compile with 0 warnings/0 errors).
  - `cargo test --release --test settings_persistence_test -- --test-threads=1`: PASSED (17/17 tests).

## 2026-11-13 — Custom Palette Card OKLCH Donut Carousel & Swatches Overhaul

**Task:** Ground-up redesign of the Custom Palette view into a standard 2-ring OKLCH Donut Picker Carousel with curated pairs, chevron navigation, text labels below the donut, and header copy button within the exact card height constraint.

### Changes
- `app/src/shared/components/settings/appearance/OklchDonutPicker.tsx`
  - Built a standard two-ring concentric OKLCH Donut Color Picker:
    - Ring 1 (Outer): 360° OKLCH Hue sweep with draggable handle.
    - Ring 2 (Inner): Lightness/Luminance sweep with draggable handle.
    - Center Core: Live color swatch circle with one-click copy.
  - Eliminated sparkles icon in favor of clean color preview disc.
  - Full touch/pointer radial hit testing with real-time OKLCH perceptual color math.
- `app/src/shared/theme/oklchColor.ts`
  - Added OKLCH conversion utilities (`hexToOklch`, `oklchToHex`) and precomputed conic-gradient stops for accent, dark background, and light background.
- `app/src/shared/theme/themePresets.ts` & `index.ts`
  - Added `CURATED_ACCENT_PRESETS` with the 4 user-specified unique pairs:
    1. Astral Mauve (`#CBA6F7` dark, `#3B1D62` light)
    2. Sakura Bloom (`#F7A6D2` dark, `#611D47` light)
    3. Neo Mint (`#A6F7D3` dark, `#22573C` light)
    4. Solar Amber (`#F9BA5F` dark, `#5A2E05` light)
- `app/src/shared/components/settings/appearance/AppearanceCard.tsx`
  - Positioned the standard 2-ring donut prominently on the left.
  - Placed the carousel footer with chevrons (`ChevronLeft` / `ChevronRight`) and simple text label (`Accent Color` / `Background Tone`) directly below the donut.
  - Right column: 4 clean, tactile preset rows showing either the 4 curated accent swatches or the 4 background presets.
  - Header (`CardTitleBar`): Active color hex readout with one-click copy button + `<ThemeToggleButton />`.
- `app/src/test/dom/AppearanceCard.dom.test.tsx`
  - Updated swatch assertion to test the new `#CBA6F7` curated accent.

### Verification
- `pnpm vitest run src/test/dom/AppearanceCard.dom.test.tsx`: 6/6 tests passed.
- `pnpm lint:arch`: passed with zero violations.
- `pnpm eslint src/shared/components/settings/appearance/ src/shared/theme/oklchColor.ts`: 0 errors, 0 warnings.
- `pnpm build`: clean compilation and production build.
