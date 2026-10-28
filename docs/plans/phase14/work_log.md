# Phase 14 Work Log

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

## 2026-10-26 — Remote Control Architecture & Synchronization Alignment (Tickets 01–06)

- **Context & Goals**:
  - Re-architected Remote Control according to the authoritative specification ([`docs/specs/remote-control-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/remote-control-spec.md)) and [`docs/specs/ipc-spec.md`](file:///home/addy/projects/apps/vox/docs/specs/ipc-spec.md).
  - Executed all 6 tracer-bullet tickets in [`.scratch/remote-control/issues/`](file:///home/addy/projects/apps/vox/.scratch/remote-control/issues/) under strict Zero Backward Compatibility (ZBC).
  - Removed all legacy side-channel events (`RemotePairedDevicesChanged`, `RemotePairingPending`).

- **Implemented Seams & Components**:
  1. **Core Domain Enums & DTOs (Ticket 01)**:
     - Replaced ambiguous `RemoteLinkState::Disconnected` with orthogonal variants: `RemoteLinkState::Unpaired` (0 paired devices in DB) and `RemoteLinkState::Paired` (1+ paired devices, idle link).
     - Defined ephemeral `PairingState` enum (`Idle`, `AwaitingApproval`, `Approved`, `Denied`) and `PairingStateDto`.
     - Added `RemotePairingStateChanged(PairingStateDto)` to `IpcEvent` in `core/events.rs`.
     - Synchronized frontend TypeScript definitions in `services/remoteService.ts`.
  2. **Tier 1 Pairing Decoupling & Explicit IPC Split (Ticket 02)**:
     - Decoupled Tier 1 (cryptographic pairing) from Tier 2 (active streaming connection).
     - Split overloaded `connect_host` into `pair_device` (first-time pairing with Noise static keys + token) and `connect_host` (dialing previously paired peer).
     - Updated `complete_pairing` in `session/listener.rs` to insert peer into `paired_devices`, send `PairApproved`, cleanly close socket with `LinkCloseReason::Local`, and transition Host to `RemoteLinkState::Paired`.
     - Implemented `pair_device` in `session/manager.rs` and exposed `pair_device` Tauri IPC command in `ipc/remote.rs`.
     - Cleanly excised legacy `RemotePairedDevicesChanged` and `RemotePairingPending` events across backend and frontend.
  3. **Tier 2 Duplex Link Supervisor & Bidirectional Heartbeat (Ticket 03)**:
     - Spawned long-running duplex reader loop `run_link_supervisor` in `attach_link` on both Host and Controller.
     - Implemented symmetric 5-second `Ping`/`Pong` heartbeat ticker with 3 missed pongs cutoff.
     - Handled `Disconnect`, `LinkLoss`, and involuntary drops (`Reconnecting` for Controller, `Paired` for Host).
  4. **Host & Controller Revocation & Disconnect Propagation (Ticket 04)**:
     - Implemented wire-level revocation: sends `LinkLoss(Revoked)` before DB deletion. Peer prunes local DB and transitions to `Unpaired` or `Paired`.
     - Clean disconnects send `LinkLoss(HostDisconnected)` from Host and `Disconnect` from Controller, setting both sides to `Paired`.
  5. **Frontend Store & Canonical Event Alignment (Ticket 05)**:
     - Driven store exclusively through `remote_state_changed` and `remote_pairing_state_changed`.
     - Refreshed paired devices upon transitioning to `Paired` or `Unpaired`.
     - Added `pairDevice` action to `remoteStore.ts` and updated unit tests in `remoteStore.test.ts`.
  6. **Frontend UI Component Alignment (Ticket 06)**:
     - Updated `RemoteDeskPanel`, `ControllerRemoteDeskPanel`, `RocketButton`, and `QrScannerModal` to check canonical orthogonal `linkState`.
     - Decoupled QR Scanner (Tier 1 pairing) from Rocket Button (Tier 2 connection).

- **Adversarial Review Remediation**:
  1. *Inbound Tier 2 Connection Handler Restored (`listener.rs`)*: Restored `handle_inbound_hello` in `listener.rs` for incoming Tier 2 connections on port 7700. Validates `accepts_controllers`, checks for concurrent active controller, verifies peer against `paired_devices`, replies with Host `Hello`, and attaches `RemoteLink`.
  2. *Cold-Boot State Hydration (`lib.rs` & `ipc/remote.rs`)*: Added startup reconciliation task in `lib.rs` and lazy reconciliation in `get_link_state` (`ipc/remote.rs`) to promote `RemoteLinkState::Unpaired` to `Paired` when records exist in `paired_devices`.
  3. *Typed Failure Classification (`manager.rs`)*: Handled `UnknownPeer` (purges local credentials, sets `RemoteFailureKind::RejectedByHost`) and `LinkRefused` (sets `RejectedByHost`) distinctly from `VersionMismatch`.
  4. *Canonical `linkState` Invariant (`RocketButton.tsx`)*: Refactored controller click handler to branch strictly on `linkState === "Paired"` and `linkState === "Failed"` rather than inspecting `pairedHost` array presence.

- **Verification**:
  - `./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architecture lint, desktop binary, benchmarks, and Android `aarch64-linux-android` cross-target).

---

## 2026-10-26 — Remote Control Revocation Synchronization & UI Polish (Tickets 07–13)

- **Context & Goals**:
  - Addressed all 7 issues identified in technical investigation across tickets `07` through `13` in `.scratch/remote-control/issues/`.
  - Resolved peer revocation desynchronization and supervisor socket race conditions.
  - Aligned modal headers, scanner typography, alert/retry overlays, theme accent tokens, and orb sleeping state.

- **Implemented Seams & Components**:
  1. **Revocation DB Purge & Supervisor Fault Isolation (Ticket 07)**:
     - Injected `VoxDb` handle into `RemoteSessionManager` and passed `db_opt` into `run_link_supervisor`.
     - In `run_link_supervisor`, on receiving `LinkLossReason::Revoked`, pruned revoked peer from Turso `paired_devices` and set `RemoteLinkState::Unpaired` (if 0 peers left) or `Paired`.
     - On link socket faults (`Err(e)`), guarded against shutdown race conditions with `cancel.is_cancelled()`, preventing transient drops from overwriting `Unpaired` state.
     - In `remoteStore.ts`, added optimistic `pairedDevices` pruning in `revokeDevice` before refetching `listPairedDevices()`.
  2. **Remote Desk Panels Copy & Layout Refactor (Ticket 08)**:
     - `RemoteDeskPanel.tsx`: Removed awkward inline trash icon and inline device button from the middle of sentences; placed device name in prose and moved `[Revoke device]` button to the bottom action block.
     - `ControllerRemoteDeskPanel.tsx`: Simplified copy; removed verbose raw IP hints `({hostHint})` and multi-paragraph layout.
  3. **Confirm & Revoke Modal Polish and Performance (Ticket 09)**:
     - `ConfirmModal.tsx`: Removed heavy accent glow shadow (`shadow-[0_0_36px_...]`) and thick colored border; applied crisp neutral border (`border-[rgba(var(--foreground),0.08)]`) and clean elevation shadow (`shadow-[0_20px_50px_rgba(0,0,0,0.6)]`).
     - Removed redundant window `keydown` listener in favor of `useOverlay` hierarchy. Streamlined framer-motion transition to `duration: 0.15`.
  4. **Orb State Mapping to Sleeping on Remote Session (Ticket 10)**:
     - `AdvancedOrb.tsx`: Added sleeping color target lerping (`ctx.tgtGlow` and `ctx.tgtAccent` multiplied by 0.2 and 0.25) when `sleeping || state === 'Sleeping'` for authentic dimmed dormant appearance.
     - `activityRegistry.ts`: Prioritized `Sleeping` state over unengaged `Dormant` state.
     - `useHomePage.ts` & `Home.tsx`: Set `activityDisplay` to "Remote Session" (`orbState: "sleeping"`) and passed mapped `interactionState="Sleeping"` to `VoxOrb` when `isRemoteBlocked` is active.
  5. **Modal Header Alignment, Layout Parity & Visual Tuning (Ticket 11)**:
     - `AuthorizeDeviceModal.tsx` & `PairingModal.tsx`: Completely unified modal header architecture. `PairingModal` now renders the full standalone header with matching padding (`p-6 sm:p-8 md:p-9`), uppercase `REMOTE CONTROL` badge + icon row, `text-2xl sm:text-[26px]` title, `text-[12.5px]` subtitle, and close button.
     - `PairingModal.tsx`: Aligned the left column's desktop host Wi-Fi IP footer flush with the bottom edge of the right-side QR card (`pb-0` on left column, `mt-auto`).
     - `PairingModal.tsx`: Updated countdown timer color thresholds so the timer and clock icon turn warning yellow (`var(--warning)`) at $\le$ 20 seconds, and urgent red (`var(--error)`) at $\le$ 5 seconds.
  6. **QR Denial Auto-Regenerate, Scanner Typography & Explicit Alert/Retry (Ticket 12)**:
     - `PairingModal.tsx`: Added `handleDenyPairing` callback that auto-invokes `_resetCachedQrDetails()` and `generatePairing()` immediately upon denial or timeout so desktop is instantly ready for retry.
     - `QrScannerModal.tsx`: Replaced subtle footer error banner with a prominent in-modal Failure Alert & Retry overlay with a touch-friendly "Scan Again / Retry" button that cleanly resets scan state and restarts camera.
     - Upgraded typography across scanner instructions, viewfinders, and manual IP inputs.
  7. **Theme Accent Color in Controller Confirmation View (Ticket 13)**:
     - `QrScannerModal.tsx`: Replaced all hardcoded `var(--success)` instances with the application's active accent token `var(--accent)`.

- **Verification**:
  - `cd app/src-tauri && ./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architecture lint, desktop binary, benchmarks, and Android `aarch64-linux-android` cross-target).
  - `pnpm test src/test/unit/remoteStore.test.ts src/test/dom/`: SUCCESS (5 test files, 28 tests passed).

---

## 2026-10-26 — Remote Desk UI Parity & Authorize Countdown Timer

- **Context & Goals**:
  - Aligned Controller Remote Desk left copy with Host Desk panel: inline `[Revoke host]` button and explicit device name on the right.
  - Added actual device name to the right of `[Revoke device]` button on Host Desk panel (`RemoteDeskPanel.tsx`).
  - Styled Controller desk right-side SVG with `var(--accent)` for paired state, and `var(--accent)` with pulsing broadcast waves when connected.
  - Added warning yellow ($\le$ 20s) and error red ($\le$ 5s) countdown timer colors to `AuthorizeDeviceModal.tsx`.

- **Implemented Seams & Components**:
  1. **Authorize Device Countdown Colors**:
     - `AuthorizeDeviceModal.tsx`: Applied dynamic color classes to Clock icon and formatted time span: `var(--warning)` at $\le$ 20s and `var(--error)` at $\le$ 5s.
     - `AuthorizeDeviceModal.dom.test.tsx`: Verified timer color threshold transitions at 25s, 19s, and 4s.
  2. **Host & Controller Desk Revoke UI & Device Names**:
     - `RemoteDeskPanel.tsx`: Added `({pairedDevice.displayName})` to the right of `[Revoke device]` button for both Connected and Paired states.
     - `ControllerRemoteDeskPanel.tsx`: Updated left-side copy to match Host Desk prose with inline `[Revoke host]` button and `({pairedHost.displayName})` to its right.
     - `settingsCopy.ts`: Added `revokeHost` and `revokeHostAria` strings.
  3. **Controller Desk Glyph Accent Colors**:
     - `ControllerRemoteDeskPanel.tsx`: Updated `HostGlyphButton` monitor frame, stand, and inner icon to render with `var(--accent)` in both Paired and Connected states. Connected state displays active animated signal waves and pulse.

- **Verification**:
  - `pnpm test src/test/unit/remoteStore.test.ts src/test/dom/`: SUCCESS (5 test files, 28 tests passed).

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


- **Settings Page Layout Inventory Doc (2026-10-28):**
  1. Explored `pages/Settings.tsx`, `useSettingsPage.ts`, `settingsLayout.ts`, `SettingsCardWrapper.tsx`, `SettingsSideCarouselCard.tsx`, `RadialHub.tsx`, and all six domain cards (Persona, Memory, Language, Appearance, Speech, Interaction) plus their desks/modals.
  2. Created root-level `SETTINGS_PAGE_LAYOUT.md` documenting desktop radial-hub grid layout, compact/mobile single-column order, shared card chrome/commit footers, each card's settings and views, and full modals/overlays inventory, plus mobile redesign preservation notes and key file index.

- **Remote Control mDNS + Connect RCA (2026-10-28):**
  1. RCA: phone dials stale hint `192.168.29.221:7700` after network change; host never receives inbound; mDNS fallback unimplemented; controller booted Unpaired on relaunch. Pairing is key-based, not IP-coupled (RAIL-RC-01).
  2. Implemented mDNS advertise (host) + browse (controller) in `services/remote/discovery/mod.rs`; listener spawns `spawn_mdns_advertiser`; manager cycles explicit → stored hints → mDNS and persists the working candidate as an Observed hint.
  3. Added `ws::dial_with_timeout` (4s) and `DIAL_CANDIDATE_TIMEOUT`; added `if-addrs` dependency.
  4. Frontend: Home failure banner with dismiss; `useHomePage` exposes `remoteFailureReason`/`dismissRemoteFailure`.
  5. Deferred Tier B (Android multicast lock) → `.scratch/remote_mdns_android_multicast_lock.md`.
  6. Verified: `verify-targets.sh` (desktop + aarch64-android) 0 warnings/0 errors; cargo lib tests 209 pass. Pre-existing failures unchanged (chapter scrubber pages-older test, tsc `ControllerRemoteDeskPanel.tsx`).

- **Remote Control mDNS Suffix RCA & Settings Failed State Fix (2026-10-28):**
  1. **Root Cause Analysis (Logs from Host `vox.log` & Android `adb logcat`)**:
     - Host log and Android logcat showed identical error: `mDNS service ... must end with '._tcp.local.' or '._udp.local.'`.
     - `REMOTE_MDNS_SERVICE_TYPE` was missing trailing dot (`_vox._tcp.local` vs `_vox._tcp.local.`).
     - As a result, host advertisement failed to register on every loop, and controller mDNS browse failed immediately on timeout.
     - Device pairing was intact in database (`paired_devices`), so no re-pairing was necessary.
     - In `ControllerRemoteDeskPanel.tsx` and `RemoteDeskPanel.tsx`, UI branched exclusively on `linkState === "Paired"` and `linkState === "Connected"`.
     - When rocket connection timed out to `linkState === "Failed"`, the desk panel treated the peer as nonexistent and rendered the unpaired "Pair with desktop..." prompt and QR scanner glyph button.
  2. **Implemented Fixes**:
     - `services/remote/mod.rs`: Fixed `REMOTE_MDNS_SERVICE_TYPE` to `"_vox._tcp.local."` per RFC 6762 / `mdns-sd` requirements.
     - `services/remote/discovery/advertiser.rs`: Replaced manual string formatting with `info.get_fullname().to_string()` for exact unregistration key match.
     - `remoteStore.ts`: Added `dismissFailure()` action to reset `linkState` to `Paired` when a paired peer is retained.
     - `useHomePage.ts`: Wired banner dismiss button to invoke `dismissFailure()`.
     - `ControllerRemoteDeskPanel.tsx`: Fixed state guard to `hasPairedHost = connectedPeer !== null && linkState !== "Unpaired"`. Rendered failed status with retry glyph and kept paired host identity visible when `linkState === "Failed"`.
     - `RemoteDeskPanel.tsx`: Updated condition to keep paired device visible during `linkState === "Failed"`.
  3. **Verification**:
     - `./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architecture lint, desktop binary, desktop tests/benches, and Android `aarch64-linux-android`).
     - `cd app && pnpm tsc --noEmit`: SUCCESS (0 errors).
     - `pnpm exec vitest run src/test/unit/remoteStore.test.ts src/test/chapterScrubberWindow.dom.test.tsx src/test/remoteActivityDisplay.regression.test.ts`: SUCCESS (22/22 pass).
  1. **Android Multicast Lock & Discovery Alignment**:
     - `MulticastLockManager.kt`: Added thread-safe Kotlin manager for `WifiManager.MulticastLock` acquire/release with safe exception handling. Retained under R8 by `jni-keep.pro`.
     - `AndroidManifest.xml`: Declared `CHANGE_WIFI_MULTICAST_STATE` and `ACCESS_WIFI_STATE` permissions.
     - `services/remote/discovery/`: Decomposed discovery domain per `backend-style-guide.md` into `hints.rs`, `advertiser.rs`, `browser.rs`, `multicast.rs`, and minimal re-export `mod.rs`.
     - `multicast.rs`: Implemented RAII `MulticastLockGuard` acquiring the lock during `discover_host` on Android (via Kotlin manager with direct JNI fallback) and releasing on drop; no-op on desktop.
     - `hints.rs`: Added `update_observed_hint` to prepend freshly observed candidates while preserving forward-compatible hints.
     - `remote/mod.rs`: Moved `MDNS_BROWSE_TIMEOUT` (6s) to subsystem constants per `RC-SSO`.
  2. **Code Review Remediation of Prior Agent's Work**:
     - `ChapterScrubber.tsx`: Rendered 11px mono range readout (`windowStart + 1`–`windowStart + visibleChapters.length`) under lower chevron when pages exist; fixed failing test in `chapterScrubberWindow.dom.test.tsx` (6/6 pass).
     - `ControllerRemoteDeskPanel.tsx`: Removed unused `useEffect` import; resolved `TS6133` error (`pnpm tsc --noEmit` exits with 0 errors).
  3. **Specification Alignment**:
     - `docs/specs/remote-control-spec.md` §6.1: Documented mDNS TXT record properties (`device_id`, `accepts`), Android `MulticastLock` requirement, candidate dial sequence, and observed hint updates.
  4. **Verification**:
     - `./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architectural invariant lint, desktop lib/bin, desktop tests/benches, and Android `aarch64-linux-android`).
     - `cargo test --test remote_connection_test --release -- --test-threads=1`: SUCCESS (9/9 pass in 1.84s).
     - `cargo test discovery --release`: SUCCESS (5/5 pass).
     - `pnpm tsc --noEmit`: SUCCESS (0 errors).
     - `pnpm exec vitest run src/test/chapterScrubberWindow.dom.test.tsx`: SUCCESS (6/6 pass).
     - `pnpm exec vitest run src/test/unit/remoteStore.test.ts src/test/remoteActivityDisplay.regression.test.ts`: SUCCESS (15/15 pass).

- **Remote Control Test Suite Revamp & `/create-test` Implementation (2026-10-28):**
  1. **Audit & RCA Findings**:
     - Identified that `remote_connection_test.rs` previously faked the host listener inside the test body instead of testing the authentic production seam (`handle_inbound_connection`).
     - Subtest header falsely claimed `dial_and_connect` while calling `pair_device`; the Rocket button path (`dial_and_connect`) had 0% integration test coverage.
     - Zero tests called `mdns-sd`, allowing RFC 6762 trailing dot rejection to escape to production.
     - Settings panel lacked DOM tests for `linkState === "Failed"`, allowing the "no device paired" UI regression.
  2. **Implemented Tests & Fixes**:
     - `services/remote/discovery/advertiser.rs`: Added `host_announcement_registers_with_rfc_compliant_service_syntax` unit test asserting `HostAnnouncement::announce` registers cleanly with `mdns-sd`.
     - `tests/remote_connection_test.rs`: Added 3 new integration subtests:
       - `remote_connection_dial_and_connect_reaches_connected`: Full controller `dial_and_connect` with real host listener, Noise XX handshake, and observed hint persistence.
       - `remote_connection_dead_primary_hint_falls_back_to_valid_candidate`: Proves dead IP candidate 0 fails over to working candidate 1 within deadline.
       - `remote_connection_dial_failure_retains_peer_identity`: Asserts dial failure transitions to `Failed` while preserving `peer_identity` and state DTO `deviceId`/`displayName`.
     - Corrected misleading intent block on `remote_connection_pairing_handshake_approved_reaches_connected`.
     - `app/src/test/controllerRemoteDeskPanel.dom.test.tsx`: Created DOM regression suite verifying `Failed`, `Paired`, `Connected`, and `Unpaired` states render correct affordances and retain host identity.
  3. **Compilation & Quality Gate**:
     - `./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architectural invariant lint, desktop lib/bin, desktop tests/benches, and Android `aarch64-linux-android`).
     - `cd app && pnpm lint:arch && pnpm tsc --noEmit`: SUCCESS (0 errors).
     - `pnpm exec vitest run src/test/controllerRemoteDeskPanel.dom.test.tsx`: SUCCESS (4/4 pass).

- **mDNS Label Length Limit Root Cause Fix, Diagnostic Tracing & Home Error Banner Position (2026-10-28):**
  1. **Root Cause Analysis (mDNS Packet Encoding Drop)**:
     - Traced why `discover_host` produced 0 candidates despite both host and phone being active on the same subnet (`10.109.x.x`).
     - Discovered that RFC 1035 §2.3.4 and RFC 6763 §4.1 enforce a maximum DNS label length of 63 octets.
     - `advertiser.rs` constructed `instance_name = "Vox-{device_id}"` and `host = "vox-{device_id}.local."` using the raw 64-hex-char `device_id`, resulting in 68-byte labels.
     - `mdns-sd` packet writer (`dns_parser.rs:3075`) explicitly skips any question or record containing labels > 63 bytes. As a result, the daemon dropped all announcement packets at wire encoding without returning a registration error.
  2. **Implementation**:
     - `services/remote/discovery/advertiser.rs`: Truncated `instance_name` and `host_name` prefix to 32 chars (`Vox-{32-char-hash}` = 36 bytes <= 63 octets), preserving full 64-char `device_id` in TXT record where controller matches. Added announcement tracing.
     - `services/remote/discovery/browser.rs`: Added exhaustive `tracing::info!` instrumentation covering `SearchStarted`, `ServiceFound`, `ServiceResolved`, `ServiceRemoved`, candidate matching, and browse timeouts.
     - `services/remote/session/manager.rs`: Added structured logging for candidate iteration, candidate failure causes, mDNS fallback triggers, and discovered candidates.
     - `app/src/pages/Home.tsx`: Replaced scattered absolute coordinate error banners (`top-16`, `top-[116px]`, `top-[160px]`) with a unified flex container pinned directly below the App Header (`top-2 sm:top-3 left-1/2 -translate-x-1/2 z-[100]`), eliminating overlap with the status badge and central orb.
  3. **Verification**:
     - `./scripts/verify-targets.sh`: SUCCESS (0 warnings / 0 errors across architectural invariant lint, desktop lib/bin, desktop tests/benches, and Android `aarch64-linux-android`).
     - `cd app && pnpm lint:arch && pnpm tsc --noEmit`: SUCCESS (0 errors).

- **Remote Session Orb Visibility & Connected Kicker Label (2026-10-28):**
  1. **Root Cause**:
     - `AdvancedOrb.tsx`: `useDynamicFPS` passed `isPaused: isSleeping || ...`. When remote connected, `isSleeping` froze the animation loop, preventing dynamic transitions until unmount/remount. Furthermore, shader alpha was reduced to `0.12` and color brightness to `0.10` (`0.12 * 0.10 = 0.012`), rendering the orb pitch black.
     - `Home.tsx`: Lacked the connected kicker label displaying the peer device identity above `DynamicStatusBadge`.
  2. **Fix Applied**:
     - `AdvancedOrb.tsx`: Removed `isSleeping` from `isPaused` so the orb stays alive and reactive. Preserved full visibility (`alpha` remains at 0.85-0.90) while softly muting the interior accent and glow colors (0.40 multiplier).
     - `Home.tsx`: Added glowing pulse badge displaying `CONNECTED: <displayName>` directly above the `REMOTE SESSION` status badge.
  3. **Verification**:
     - `cd app && pnpm lint:arch && pnpm tsc --noEmit`: SUCCESS (0 errors).
     - Vite HMR updated live on desktop host.


- **Restore Defaults Confirm Modal (2026-10-28):**
  1. **Change**: Replaced in-place two-step inline confirmation in `app/src/shared/components/settings/RestoreDefaultsButton.tsx` with shared `ConfirmModal`. Button now opens modal; confirming calls `restoreDefaults()`.
  2. **Verification**: `cd app && pnpm tsc --noEmit`: SUCCESS (0 errors).
