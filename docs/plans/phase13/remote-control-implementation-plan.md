# Remote Control Connection Flow Vertical Slice — Master Execution Plan & Specification

## 1. Goal & Strict Scope Boundaries

This vertical slice implements the **Remote Control Connection Flow End-to-End**.
- **In Scope:**
  1. Backend WebSocket transport (`WsLink`) over `tokio-tungstenite`.
  2. Noise_XX mutual authentication, single-use pairing token QR minting, and in-memory approval flow.
  3. Canonical session state machine (`RemoteLinkState`, heartbeat loop, `Hello` negotiation, and disconnect handling).
  4. 9 Tauri IPC commands and the `remote_state_changed` event push.
  5. Complete Frontend UI:
     - Desktop Host: Settings Remote Desk toggle, SVG QR pairing modal with 2-min countdown, Paired Controller card with Revoke action, Inbound Approval prompt modal (`PairingPromptModal`).
     - Android Controller: Empty state, In-app WebRTC camera QR scanner modal with manual IP fallback, Paired Host view.
     - Header Rocket Button 🚀: Controller dial trigger; Host active session indicator with two-step unboxed inline disconnect matching `RestoreDefaultsButton.tsx`.
     - Ambient Visuals: Host motionless dimmed orb with inward ripples and compact `● Remote` status badge; Controller screen rim glow.
  6. Automated loopback integration tests and physical Android APK build verification.
- **Strictly Out of Scope:**
  - Microphone audio capture streaming and speaker playback streaming (parked for subsequent audio pipeline slice).
  - Off-LAN transports (QUIC, relays).
  - Multiple simultaneous active controllers (v1 is strictly 1:1 host-to-controller).

---

## 2. All Settled Architectural & UI Decisions (ADRs)

| Area | Settled Decision | Behavioral Contract |
| :--- | :--- | :--- |
| **Role Coupling** | Desktop is always **Host**; Android/Mobile is always **Controller**. | Decoupling is deferred to v2. Simplifies state and eliminates asymmetric confusion in v1. |
| **Enablement (Lane 1)** | Host-only persistent switch (`remote.accepts_controllers: bool`). | Controller has NO enablement switch in settings. Controller is a client that connects on demand. |
| **Header Rocket 🚀 (Lane 2)** | **Controller:** Always visible next to ghost/incognito in `AppHeader`. Clicking dials host or opens scanner modal.<br>**Host:** Hidden when idle; appears only when `Connected`. | Host disconnect uses a **two-step unboxed inline transition** identical to `RestoreDefaultsButton.tsx`: text *"Disconnect remote"*, red checkmark (`Check`), grey cross (`X`), auto-reverting after 5s. |
| **Pairing & Approval** | Single-use 128-bit token (2-min TTL, in memory). Host displays *"Approve [Device Name]?"* modal with 30s timeout and [Approve] / [Deny] buttons. | **No secondary 4-to-8 digit PIN code.** Proximity is cryptographically proven by scanning the ephemeral QR code and completing Noise_XX. |
| **Controller In-App Scanner** | Pure in-app camera viewfinder via WebRTC `getUserMedia` + `jsQR` parser, plus manual IP/port entry fallback. | Zero native plugin dependency overhead; 100% portable on Android WebView. Camera tracks are strictly stopped on unmount. |
| **Paired Device Management** | **Host:** Shows single paired controller card with Revoke (trash) button calling `revoke_device`.<br>**Controller:** Shows paired host card with status ("Online"). | **NO Controller-side unpair command exists in `ipc-spec.md`.** Switching hosts on the controller is done simply by scanning a new QR code or entering a new target address. |
| **Host Control Guard** | Backend enforces `RULE-RC-GUARD`: local mutating writes are rejected during a remote session. | Frontend implements `useRemoteGuard()` hook: disables Engage button and mutating triggers with `disabled={true}`, `cursor-not-allowed`, and tooltip *"Remote session active"*. |
| **Host Visuals** | Dimmed motionless orb (sleeping style) with **ripples flowing inward**; compact status pill `● Remote` above orb. | Provides clear, calm visual feedback that the laptop is a passive listener while phone is authoritative. |
| **Controller Visuals** | Inset accent rim glow around viewport edges (`box-shadow: inset 0 0 24px rgba(var(--accent), 0.15)`) + compact `Remote` sub-pill. | Subtle ambient indicator that the mobile surface is controlling a remote host. |

---

## 3. End-to-End Testing & Physical Device Questions Answered

### Q1: Can an agent fully test the connection logic end-to-end without a phone?
**Yes.**  
In Vox, the wire protocol and session state machine run independently of physical hardware:
- We author a dedicated integration test binary: [`tests/remote_connection_test.rs`](file:///home/addy/projects/apps/vox/app/src-tauri/tests/remote_connection_test.rs).
- This test spins up a live Host instance on a local OS loopback port (`127.0.0.1:0`), mints a pairing token, and simulates a Controller instance over real TCP/WebSocket.
- It validates the entire flow programmatically:
  1. TCP connection and Noise_XX handshake.
  2. Public key derivation and identity verification (`DeviceId == SHA-256(domain || pubkey)`).
  3. Ephemeral pairing token validation and consumption.
  4. Inbound approval channel resolution (`approve_pairing`).
  5. Hello exchange and capability negotiation.
  6. Heartbeat loop (bidirectional 2s/1s ping/pong exchange).
  7. Clean disconnect and state transition back to `Disconnected`.
- For the frontend, Vitest DOM tests (`pnpm test:dom`) verify the React components, countdown timer, SVG QR rendering, and `useRemoteGuard` hook.

### Q2: At what point in the plan should the user connect their phone to the laptop?
**At Ticket 09 (The Final Verification Phase).**
- During Tickets 01–08, all backend code is validated using unit tests, seam tests, and the cross-compilation compile gate (`./scripts/verify-targets.sh`).
- Once Ticket 09 is reached:
  1. Connect your Android phone to your laptop via USB.
  2. Ensure USB Debugging is enabled (`adb devices` shows the phone).
  3. The agent/user runs:
     ```bash
     cd app/src-tauri && cargo tauri android dev
     # OR builds debug apk:
     cargo tauri android build --debug
     adb install -r gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
     ```
  4. Open Vox on your laptop (Host) and Vox on your phone (Controller).
  5. Laptop: Settings -> Remote Control -> click `[+ Pair New Device]`.
  6. Phone: Header Rocket Button or Settings -> tap `[Scan Host QR]`, point camera at laptop screen.
  7. Laptop: Click `[Approve]` on the inbound prompt modal.
  8. Verify: Header Rocket icon appears on laptop, inward ripples activate, and phone takes control.

---

## 4. Ticket Breakdown (`.scratch/remote-connection-slice/issues/`)

All 9 tickets have been created as standalone markdown work items:

1. **[01-ws-link-transport.md](file:///home/addy/projects/apps/vox/.scratch/remote-connection-slice/issues/01-ws-link-transport.md)**: Implement `WsLink` over `tokio-tungstenite` implementing `RemoteLink` (Host listener & Controller dialer).
2. **[02-session-manager-state.md](file:///home/addy/projects/apps/vox/.scratch/remote-connection-slice/issues/02-session-manager-state.md)**: Implement `RemoteLinkState` enum, `RemoteSessionManager`, Noise_XX integration, Hello exchange, and 2s/1s heartbeat loop.
3. **[03-command-routing-guard.md](file:///home/addy/projects/apps/vox/.scratch/remote-connection-slice/issues/03-command-routing-guard.md)**: Implement static `CommandClass` table and `RULE-RC-GUARD` blocking desktop mutating writes during remote session.
4. **[04-pairing-coordinator-qr.md](file:///home/addy/projects/apps/vox/.scratch/remote-connection-slice/issues/04-pairing-coordinator-qr.md)**: Ephemeral 128-bit pairing token minting, SVG QR string output via `qrcode` crate, and `PendingPairing` with 30s approval timeout.
5. **[05-ipc-handlers-event-push.md](file:///home/addy/projects/apps/vox/.scratch/remote-connection-slice/issues/05-ipc-handlers-event-push.md)**: Register the 9 simplified Tauri remote commands and wire `remote_state_changed` event push across transitions.
6. **[06-frontend-store-remote-guard.md](file:///home/addy/projects/apps/vox/.scratch/remote-connection-slice/issues/06-frontend-store-remote-guard.md)**: Implement `remoteService.ts`, Zustand `remoteStore.ts`, and `useRemoteGuard()` hook.
7. **[07-settings-remote-desk-host-pairing.md](file:///home/addy/projects/apps/vox/.scratch/remote-connection-slice/issues/07-settings-remote-desk-host-pairing.md)**: Host Remote Desk panel, `PairingModal.tsx` with SVG QR and live countdown timer, and Host `PairingPromptModal.tsx`.
8. **[08-controller-settings-in-app-scanner.md](file:///home/addy/projects/apps/vox/.scratch/remote-connection-slice/issues/08-controller-settings-in-app-scanner.md)**: Controller Remote Desk panel, In-App WebRTC camera viewfinder modal (`getUserMedia` + `jsQR`), manual IP fallback, and connected host view.
9. **[09-header-rocket-visuals-apk-test.md](file:///home/addy/projects/apps/vox/.scratch/remote-connection-slice/issues/09-header-rocket-visuals-apk-test.md)**: AppHeader `RocketButton.tsx` with two-step inline disconnect matching `RestoreDefaultsButton.tsx`, Host inward ripples, Controller edge glow, `tests/remote_connection_test.rs`, and physical device APK verification.

---

## 5. Implementation Agent Instructions

When the implementation agent is launched to execute these tickets:
1. **Work Frontier Order:** Follow tickets strictly from `01` to `09`. Each ticket declares its blockers.
2. **Style Guides:**
   - Backend: Adhere to `backend-style-guide.md` and `AGENTS.md` §8 module map.
   - Frontend: Adhere to `impeccable` design rules and `frontend-style-guide.md` (no raw color literals, use theme variables, Framer Motion).
3. **Compile Gate Rule:**
   - After any change touching Rust or Tauri configs, always verify with:
     ```bash
     cd app/src-tauri && ./scripts/verify-targets.sh
     ```
