# Phase 14 Work Log

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
