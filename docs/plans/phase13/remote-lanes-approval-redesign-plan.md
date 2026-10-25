# Remote Control 3-Lane Architecture & UI Overhaul Plan

## 1. Executive Summary & Target End-State

This plan resolves the architectural conflation between device pairing and live remote streaming, fixes the visual flicker and layout defects across host and controller surfaces, implements a pixel-perfect Host Authorization dialog matching user-provided reference (Image 3), creates a shared `<ConfirmModal>` primitive matching the Antigravity confirmation reference (Image 2), and provides clean, mutual revocation synchronization across LAN peers.

### Target End-State:
1. **Three Completely Decoupled Lanes:**
   - **Lane 1: Device Pairing (Mutual Cryptographic Trust Setup):** QR scan $\to$ Handshake $\to$ Host Authorization Dialog $\to$ Mutual storage in `paired_devices`. Upon approval, the pairing link cleanly terminates. The devices are paired, but **not** actively streaming.
   - **Lane 2: Remote Connection (Live Session):** Initiated explicitly via the Home Screen **Rocket button** (or explicit connect action). Uses established `paired_devices` credentials to start live bidirectional audio and control streaming. Clicking Disconnect ends the stream while preserving the device pairing.
   - **Lane 3: Revocation (Mutual Cryptographic Tear-down):** Revoking a device removes it from `paired_devices`. If currently connected, the peer is notified with `LinkLoss(Revoked)` so the peer immediately clears its stored pairing and updates UI state.
2. **Elimination of `PairingModal` Flicker:** Immediate initialization to loading/skeleton states on mount; no multi-frame empty content flashing.
3. **High-Fidelity Host Authorization Dialog:** Pixel-accurate 2-column layout matching reference Image 3 (`media_1792919404089_35994bd8.png`) with permission itemization, phone-to-laptop wireless illustration, and auto-expire countdown.
4. **Shared `<ConfirmModal>` Component:** Minimal, elegant destructive action modal matching reference Image 2 (`media_1792919276119_af3bcede.png`), eliminating ugly in-place inline layout gaps in settings.
5. **Redesigned Mobile Controller Panel:** Clean host identity display on left; interactive SVG stub with connected status and subtle red `Click to revoke` triggering `<ConfirmModal>` on right.

---

## 2. Root Cause Analysis & Empirical Evidence

### RCA 1: Pairing Conflated with Live Remote Streaming (Lane 1 vs Lane 2)
- **Confirmed Evidence (`manager.rs` lines 422–463 & `listener.rs` lines 281–304):**
  When first pairing via QR code (`PairApproved`), both the mobile controller in `dial_and_connect` and the desktop host in `complete_pairing_and_attach` immediately run:
  ```rust
  exchange_hello(...)
  attach_link(...)
  ```
  This immediately attached the live link, started the heartbeat supervisor, and transitioned `linkState` to `"Connected"`.
- **Consequence:**
  1. Once paired, the controller's home screen Rocket button appeared active immediately.
  2. Clicking the Rocket button called `disconnectHost()`, which severed the connection, but because `ControllerRemoteDeskPanel.tsx` only checked `linkState === "Connected"`, disconnecting made the settings screen look like the phone was no longer paired!
- **Resolution:**
  Pairing (Lane 1) must terminate upon saving `paired_devices` and exchanging `PairApproved`. It must close the handshake socket cleanly without calling `attach_link`. Live connection (Lane 2) is solely attached when dialing an already-paired device via the Rocket button.

### RCA 2: Jarring `PairingModal` Double Flicker
- **Confirmed Evidence (`PairingModal.tsx` lines 74–114):**
  `loading` state was initialized to `false` with `details = null`. On render tick 1, the modal displayed the fallback empty container ("No pairing code generated"). On tick 2, `useEffect` called `generatePairing()`, flipping `loading` to `true` (rendering the spinner). On tick 3, the IPC resolved, setting `details` and flipping `loading` to `false`.
- **Resolution:**
  Initialize `loading` to `true` when `open && !details`. Render the container skeleton immediately, avoiding the 3-frame layout shift. Decouple authorization logic out of `PairingModal.tsx`.

### RCA 3: Persistent Layout Gap & In-Place Confirmation
- **Confirmed Evidence (`RemoteDeskPanel.tsx` & `ControllerRemoteDeskPanel.tsx`):**
  Both panels rendered an inline hover-expandable container (`max-w-[150px]`) next to the host/device name that shifted surrounding text and left a dead gap. Clicking it replaced the text in-place with inline tick/cross buttons.
- **Resolution:**
  Remove inline expansion. The revoke action triggers the shared `<ConfirmModal>` matching Image 2.

### RCA 4: Peer Blindness on Revocation (Lane 3)
- **Confirmed Evidence (`manager.rs` lines 271–287):**
  When `revoke_device(device_id)` is invoked, the host deletes the row from Turso and calls `disconnect(LinkLossReason::HostDisconnected)`. It never notifies the peer that it was *revoked*. If the controller was connected, it treated the disconnect as a normal link drop and tried to reconnect.
- **Resolution:**
  Introduce `LinkLossReason::Revoked`. When revoked while connected, host sends `LinkLoss(Revoked)` so controller immediately deletes the host from its local `paired_devices` and halts reconnection backoff. Emits IPC event `remote_paired_devices_changed` so local frontends refresh immediately.

---

## 3. Specifications Alignment & UI Reference Contracts

### 3.1 Host Authorization Modal (Reference: Image 3 `media_1792919404089_35994bd8.png`)
The component `AuthorizeDeviceModal.tsx` will be extracted from `PairingModal.tsx` and structured with:
- **Header:**
  - Cyan shield outline icon + uppercase tracking label `REMOTE CONTROL` (`text-[10px] font-mono tracking-widest text-[rgb(var(--accent))]`).
  - Modal title: `Authorize Remote Device` (`text-2xl font-bold tracking-tight text-white`).
  - Subtitle: `A companion device is requesting permission to control Vox over your local Wi-Fi network.`
  - Dismiss button: Circular dark glass close button on top right (`size={16}`).
- **Two-Column Body:**
  - **Left Column (`THIS DEVICE CAN`):**
    - 4 vertical permission items with rounded badge icons:
      1. *Microphone (`Mic`):* **Stream microphone audio** — *Send live audio from this device into Vox's speech pipeline.*
      2. *Play (`Play`):* **Start, pause, and trigger voice sessions** — *Control real-time voice interactions and remote tools.*
      3. *PTT (`Command` / `Radio`):* **Trigger push-to-talk** — *Send speech dictation input remotely.*
      4. *Telemetry (`Activity` / `BarChart3`):* **Read session state** — *View real-time session telemetry and pipeline status over local Wi-Fi.*
  - **Right Column (Hardware & Wireless Status):**
    - High-fidelity vector graphic: Mobile phone outline with cyan glowing orb $\leftrightarrow$ Dotted wireless beam $\leftrightarrow$ Laptop display with radial radar rings and Vox center orb.
    - Countdown timer: Clock icon + `Request expires in` + Monospace countdown `00:12`.
    - Microcopy: `If you don't approve, this request will expire automatically and the device won't be able to connect.`
- **Footer Actions:**
  - Left: Dark glass `✕ Deny` button (`bg-white/[0.04] hover:bg-white/[0.08] border border-white/10 text-white rounded-2xl py-3`).
  - Right: Cyan pill `✓ Approve Connection` button (`bg-[#52f3f1] hover:bg-[#6ef5f3] text-black font-semibold rounded-2xl py-3 shadow-[0_0_20px_rgba(82,243,241,0.35)]`).
- **Affordances:** Keyboard shortcuts: `Enter` to approve, `Escape` to deny.

### 3.2 Shared Confirm Modal (Reference: Image 2 `media_1792919276119_af3bcede.png`)
Component `ConfirmModal.tsx` in `app/src/shared/ui/ConfirmModal.tsx`:
- Centered dialog container with dark glass surface (`bg-[#1e1e24]` / `bg-[rgb(var(--surface))]`), rounded corners (`rounded-2xl`), subtle border (`border border-white/10`).
- Header: Left-aligned title + Right-aligned close 'X' button.
- Body: Informative description text.
- Footer: Right-aligned button group:
  - `Cancel` ghost button.
  - Action button (`Revoke` / `Delete`): Solid danger pill (`bg-[rgb(var(--danger))] text-white hover:opacity-90 rounded-xl px-4 py-2`).

### 3.3 Controller Settings Panel
- **Left Column:**
  - When paired: Host Name (e.g. `Vox Desktop Host`), Local IP / status badge.
  - When unpaired: Pairing instructions.
- **Right Column:**
  - When paired: SVG stub trigger with `Connected` / `Paired` label and subtle red `Click to revoke` label opening `<ConfirmModal>`.
  - When unpaired: QR Scanner glyph button.

---

## 4. Blast Radius & Dependency Matrix

| Area | Component / File | Blast Radius / Callers | Dependency |
| :--- | :--- | :--- | :--- |
| **Backend Protocol** | `protocol/messages.rs` | `LinkLossReason::Revoked` variant | None |
| **Backend Listener** | `session/listener.rs` | Decouple pairing approval from session attach | `protocol/messages.rs` |
| **Backend Manager** | `session/manager.rs` | Decouple `dial_and_connect` pairing path from hello/attach; add `LinkLoss(Revoked)` | `session/listener.rs` |
| **Shared Primitives** | `ConfirmModal.tsx` | New reusable modal component | None |
| **Host UI** | `AuthorizeDeviceModal.tsx` | Extracted from `PairingModal.tsx`, implements Image 3 | None |
| **Host UI** | `PairingModal.tsx` | Remove authorization clutter, fix flicker | `AuthorizeDeviceModal.tsx` |
| **Host Settings** | `RemoteDeskPanel.tsx` | Wire `<ConfirmModal>` for revoke; cleanup gaps | `ConfirmModal.tsx` |
| **Controller UI** | `ControllerRemoteDeskPanel.tsx`| Redesign paired state, wire `<ConfirmModal>` | `ConfirmModal.tsx` |
| **Store & State** | `remoteStore.ts` | Listen for `remote_paired_devices_changed`, decouple pairing from connection | Backend IPC events |
| **Home Screen** | `RocketButton.tsx` | Guard connection toggle strictly to Lane 2 | `remoteStore.ts` |

---

## 5. Execution Batches

### Batch 1: Shared UI Primitives (`ConfirmModal` & Decoupled `AuthorizeDeviceModal`)
*State during batch:* Fully additive; zero compilation breaks.
1. Create `app/src/shared/ui/ConfirmModal.tsx` matching Image 2 (`media_1792919276119_af3bcede.png`).
2. Create `app/src/shared/components/remote/AuthorizeDeviceModal.tsx` matching Image 3 (`media_1792919404089_35994bd8.png`).
3. Extract inline authorization logic from `PairingModal.tsx` into `AuthorizeDeviceModal.tsx`.
4. Fix `PairingModal.tsx` initial state flicker (`loading: true` on initial mount).

### Batch 2: Backend 3-Lane Decoupling (Pairing vs Connection vs Revocation)
*State during batch:* Atomic backend update. User will compile and verify Rust.
1. `app/src-tauri/src/services/remote/protocol/messages.rs`:
   - Add `LinkLossReason::Revoked`.
2. `app/src-tauri/src/services/remote/session/listener.rs`:
   - Split `complete_pairing_and_attach` into `complete_pairing`: sends `PairApproved`, inserts peer into `paired_devices`, emits `RemotePairedDevicesChanged`, and cleanly closes pairing link with `LinkCloseReason::Local`.
3. `app/src-tauri/src/services/remote/session/manager.rs`:
   - In `dial_and_connect`: when `token` is present, await `PairApproved`, save host to `paired_devices`, close link cleanly, set state to `Disconnected`. Do NOT call `attach_link`.
   - In `revoke_device`: if peer is currently connected, send `ControlMessage::LinkLoss(LinkLoss { reason: LinkLossReason::Revoked })` before closing.
4. `app/src-tauri/src/core/events.rs`:
   - Add `IpcEvent::RemotePairedDevicesChanged` to notify frontends when devices are paired or revoked.

### Batch 3: Controller on Receiving Revocation & Frontend Store Sync
*State during batch:* Frontend TypeScript and tests stay green.
1. In `app/src/store/remoteStore.ts`:
   - Listen to `remote_paired_devices_changed` $\to$ re-fetch `listPairedDevices()`.
   - Update `linkState` handler: if `failureKind === "rejected_by_host"` or `linkState === "Failed"` due to revocation, re-fetch `listPairedDevices()` to purge local host record.
2. In `app/src-tauri/src/services/remote/session/manager.rs`:
   - When controller receives `LinkLoss(Revoked)`, automatically remove host from its local `paired_devices` table.

### Batch 4: Host & Controller Settings Panels Redesign
*State during batch:* Zero-gap layout, `<ConfirmModal>` integration.
1. In `RemoteDeskPanel.tsx`:
   - Eliminate 18px layout hole and inline tick/cross.
   - Revoke button opens `<ConfirmModal>` with title `"Revoke Controller"` and body copy.
2. In `ControllerRemoteDeskPanel.tsx`:
   - Check `pairedDevices.length > 0` (or host paired) rather than only `isConnected`.
   - Left side: Display Host Name and IP/connection status.
   - Right side: SVG stub icon with status and red `Click to revoke` label triggering `<ConfirmModal>`.
3. In `RocketButton.tsx`:
   - Decoupled from pairing logic. Handles solely Lane 2 (connecting / disconnecting active session).
   - If not paired, clicking informs user to pair first.

### Batch 5: Automated Verification & Review
1. Run `pnpm tsc --noEmit` to verify complete type safety.
2. Run `pnpm test:unit` and `pnpm test:dom` (ensure all tests pass).
3. Run `pnpm lint:arch` to guarantee zero architecture violations.
4. User compiles and validates Rust compile gate (`./scripts/verify-targets.sh`).

---

## 6. Invariants & Guard Rails
- **Zero Backward Compatibility (ZBC):** Direct replacement of old inline confirmation and conflated pairing paths. No legacy fallback paths.
- **Style Guides:** Strictly follow `.agents/rules/backend-style-guide.md` and `.agents/rules/android-style-guide.md`.
- **Compile Gate:** User executes `./scripts/verify-targets.sh`. No bare cargo commands run by the assistant.
