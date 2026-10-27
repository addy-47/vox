### 1. Solving the Revocation Sync Issue (And Why Your Agent’s Option 1 Has Two Traps)

Your agent correctly diagnosed *why* revocation does not sync while in the `Paired` (idle) state: `self.link` is `None` because Tier 1 closes the WebSocket immediately after `PairApproved`, so `manager.rs::revoke_device` simply deletes the local Turso DB row and emits `Unpaired` locally without sending a single packet over the LAN.

However, your agent’s recommended **Option 1 (One-Shot `RevokeNotice` + 4-Second Frontend Liveness Probe)** contains a **critical security vulnerability** and a **polling anti-pattern**:

1. **The Security Trap in Agent’s Step 1–2 (Unauthenticated `RevokeNotice`):**
* Your agent proposes dialing `ws://host:7700`, sending a new `ControlMessage::RevokeNotice(controller_id)`, and having the Host delete the Controller from `paired_devices`.
* **Why this is dangerous:** `DeviceKey` is an X25519 Diffie-Hellman keypair (Spec §5.1–5.2)—there is no Ed25519 signature scheme in v1. And because theHost broadcasts its `device_id` via mDNS, if `RevokeNotice` is accepted without a Noise_XX handshake, any device on the Wi-Fi network can connect to port 7700, send `RevokeNotice`, and remotely wipe your paired phone from the laptop.




2. **The Polling Trap in Agent’s Step 3 (4-Second Frontend Probe Loop):**
* Your agent proposes putting a 4-second timer inside `remoteStore.ts` / `ControllerRemoteDeskPanel.tsx` to continuously probe the Host while in `Paired` state.
* **Why this violates Vox architecture:** Frontend components and stores must never poll network state. A 4-second TCP/WebSocket dial loop prevents CPU sleep states, spams the Host listener with 15 connections per minute, and creates a race condition if the user taps the Rocket button (`dial_and_connect`) while a probe socket is mid-flight.





#### The Correct Architecture for Bidirectional Idle Revocation Sync

You can achieve **instant (<100ms) sync in both directions** with **zero background polling** and **full cryptographic authentication** using primitives already in your stack:

| Direction | Mechanism | Exact Execution Flow |
| --- | --- | --- |
| **A. Controller revokes Host while `Paired` (Idle)** | **Best-Effort Authenticated One-Shot `LinkLoss(Revoked)**` | 1. In `manager.rs::revoke_device`, before deleting the Host row from `paired_devices`, read the Host’s `hints` (`addr:port`) and `public_key`.

<br>

<br>2. Delete the local DB row immediately and emit `remote_state_changed("Unpaired")` so the phone UI updates in 0ms without waiting on the network.

<br>

<br>3. Spawn a detached background Tokio task with a **2-second timeout** that dials `ws://host_addr:port`, **completes the 3-message Noise_XX handshake** (cryptographically proving possession of the Controller's X25519 key), sends existing `ControlMessage::LinkLoss(LinkLoss { reason: LinkLossReason::Revoked })`, and closes the socket.

<br>

<br>4. On the Host, `listener.rs` completes Noise_XX, verifies the peer's static key matches the paired controller, sees `LinkLoss(Revoked)`, deletes the controller from `paired_devices`, and transitions Host to `Unpaired`.

 |
| **B. Host revokes Controller while `Paired` (Idle)** | **mDNS TXT State Broadcast + App-Resume One-Shot Check** | 1. **Primary (Instant <100ms LAN push, 0 TCP sockets):** Under Spec §6.1 & §6.5, the Host already advertises `_vox._tcp.local` via `mdns-sd` with `device_id` and `accepts` in the TXT record. Add a `paired_peer=<controller_id_prefix

 |

---

### 2. Backend Code & Spec Security/HLD Review (Critical Bugs Found)

Reviewing your Tier 1 and Tier 2 implementation across `state.rs`, `manager.rs`, `listener.rs`, `ws.rs`, and `messages.rs` uncovered **five critical bugs**:

#### 🐛 Bug 1 (P0 Security): Noise_XX Encryption & Authentication Is Dead Code (Everything Travels in Plaintext)

* **Where:** `manager.rs:151-187, 285-533`, `listener.rs:131-362`, and `ws.rs:38-145`.


* **What is happening:**
* `run_host_handshake`, `run_controller_handshake`, and `drive_noise_handshake` are defined in `manager.rs` **and never called anywhere in the codebase**.


* `WsLink` (`ws.rs`) has no `NoiseTransport` state at all—it serializes `ControlMessage` directly to plaintext UTF-8 JSON and sends it over an unencrypted `ws://` socket (`ws.rs:43-47`).


* In `pair_device` (`manager.rs:320-366`) and `handle_inbound_pairing` (`listener.rs:247-362`), `PairRequest`, `PairPrompting`, and `PairApproved` are exchanged in **plaintext** with zero Noise handshake and zero verification that `derive(handshake_static) == claimed_device_id` (violating Spec §5.2, §5.3, and `VERIFY-RC-04`).


* In `dial_and_connect` (`manager.rs:466`) and `handle_inbound_hello` (`listener.rs:176`), the Controller dials `ws://` and immediately sends a plaintext `Hello` claiming a `device_id`. Anyone on the LAN who reads the `device_id` can spoof `Hello` without possessing the X25519 private key.




* **The Fix:** `WsLink` must hold `cipher: AsyncMutex<Option<NoiseTransport>>`. In Tier 1, right after `PairRequest` is sent/received in plaintext (Spec §5.3, §7.1), both sides must run `run_controller_handshake` / `run_host_handshake`, verify the static key matches `device_id`, and upgrade `WsLink` to encrypt/decrypt all subsequent frames (`PairPrompting`, `PairApproved`, `PairDenied`). In Tier 2, `Noise_XX` must complete *before* `Hello` is exchanged.



#### 🐛 Bug 2 (P0 State Bug): `peer_identity` Is `None` in `Paired` State, Breaking `remote_state_changed` and `get_link_state`

* **Where:** `manager.rs:111-129, 392, 549-552, 608, 628, 791-806` and `listener.rs:394-406`.


* **What is happening:**
1. Look at the comment your agent left in `listener.rs:394-406`:


```rust
state.remote_manager.set_state(RemoteLinkState::Paired);
// Patch the dto to carry the new peer's identity since set_state clears peer_identity
let dto_with_peer = RemoteStateDto { link: "Paired".to_string(), ... };
let _ = emit_ipc(&app_handle, IpcEvent::RemoteStateChanged(dto_with_peer));

```


Calling `set_state(RemoteLinkState::Paired)` on line 394 triggers `on_state_change` with `peer_identity == None`, emitting a first `remote_state_changed { link: "Paired", deviceId: null, displayName: null }`, followed immediately by a second manual event on line 404!


2. Worse, on the Controller (`manager.rs:392`), after `PairApproved`, it calls `self.set_state(RemoteLinkState::Paired)` without ever setting `self.peer_identity`—so the Controller UI receives `link: "Paired"` with **`deviceId: null, displayName: null`**!


3. And on every disconnect (`disconnect_host_initiated`, `disconnect_controller_initiated`, and `clear_peer_and_set_state` in `manager.rs:608, 628, 797`), the code sets `*self.peer_identity.write() = None` **before** transitioning to `RemoteLinkState::Paired`! That means every transition from `Connected` $\to$ `Paired` wipes out `deviceId` and `displayName` in `RemoteStateDto`, violating Spec §9.1 & §13!


4. Furthermore, in `revoke_device` (`manager.rs:549-552`), `let is_current = self.peer_identity().map(|p| p.device_id == device_id)` is always `false` in `Paired` state because `peer_identity` was cleared on disconnect!




* **The Fix:** Stop splitting `RemoteLinkState` and `peer_identity` across two separate `RwLock`s (`manager.rs:49-50`). Either keep `peer_identity` populated whenever a paired device exists in `Paired | Connecting | Connected | Reconnecting` (only clearing it to `None` on `Unpaired` or `Disabled`), or embed the peer metadata directly into the enum variants.



#### 🐛 Bug 3 (P1 Data Loss): Hardcoded `"Vox Host"` and `"Vox Controller"` Overwrite Real Device Names

* **Where:** `messages.rs:321`, `manager.rs:325, 380, 462`, and `listener.rs:226`.


* **What is happening:**
* Spec §5.3 and §7.3 state that `PairApproved` carries `{ host_identity }`. In `messages.rs:321`, `ControlMessage::PairApproved` was implemented as an empty unit variant!


* Because `PairApproved` carries no `host_identity` (and the QR payload lacks `display_name`), `manager.rs:380` hardcodes `display_name: "Vox Host".to_string()` into Turso DB!


* Similarly, `manager.rs:325, 462` hardcodes `"Vox Controller"` instead of the phone's device name, and `listener.rs:226` hardcodes `"Vox Host"`.




* **The Fix:** Add `pub struct PairApproved { pub host_identity: SenderIdentity }` to `messages.rs` per Spec §7.3, and pass the real OS hostname / configured device name instead of hardcoded strings.



#### 🐛 Bug 4 (P1 Security & Spec Violation): `source_ip` Hardcoded to `127.0.0.1` & Port Hardcoded to `7700`

* **Where:** `listener.rs:48, 281` and `ws.rs:149`.


* **What is happening:**
* `WsListener::accept` (`ws.rs:149`) discards the `SocketAddr` returned by `self.listener.accept().await`.


* Consequently, `listener.rs:281` hardcodes `let source_ip = IpAddr::V4(Ipv4Addr::LOCALHOST);` when calling `state.pairing_coordinator.begin(...)`. Every device on the LAN is rate-limited as `127.0.0.1` (3 failed attempts from an attacker IP invalidates the token or rate-limits the legitimate phone), and `complete_pairing` (`listener.rs:386`) saves `hints: vec![]` because it lost the peer's IP!


* `listener.rs:48` hardcodes `"0.0.0.0:7700"` instead of binding a dynamic port (or configurable port) and advertising it via mDNS as required by Spec §6.1 & §15.




* **The Fix:** Store `peer_addr: SocketAddr` on `WsLink` inside `WsListener::accept()` and pass `ws_link.peer_addr().ip()` into `pairing_coordinator.begin()`.



#### 🐛 Bug 5 (P1 Concurrency & Protocol): Controller Heartbeat `sleep` Reset Bug & Silent Admission Drops

* Controller Heartbeat `tokio::select!` Bug (`manager.rs:771`):


* In `run_link_supervisor`, `_ = sleep(PING_TIMEOUT), if role == SessionRole::Controller` creates a brand-new `sleep(6s)` future on **every iteration of the `loop**`. Whenever *any* control message (`Event`, `Res`, `Ping`) arrives on `link.recv_control()`, the `select!` loop iterates and restarts the 6-second sleep from zero! Once Tier 3 starts streaming `Event` messages (up to 30 Hz), that `sleep(6s)` arm will **never fire** even if the Host stops sending `Ping` completely.
* **Fix:** Replace `sleep(PING_TIMEOUT)` with `tokio::time::sleep_until(tokio::time::Instant::from_std(last_ping_at + PING_TIMEOUT))`.


* Pre-Message Silent Drop (`listener.rs:133-136`):


* `handle_inbound_connection` checks `if state.remote_manager.state() == RemoteLinkState::Connected { return; }` *before* reading `initial_msg`. This silently drops the TCP socket without sending `LinkRefused { reason: ControllerAttached }`, making lines 191–199 in `handle_inbound_hello` dead code and violating `VERIFY-RC-18`.


* Also, `handle_inbound_hello` is missing the check for an active local desktop session (`LinkRefusedReason::HostBusy`, Spec §6.5 & `VERIFY-RC-18`).





---

### 3. Backend Code Structure & Style-Guide Smells (`src/services/remote/`)

Your directory layout (`audio/`, `control/`, `discovery/`, `link/`, `protocol/`, `security/`, `session/`) is clean and well-bounded, but the implementation inside `session/` and `link/` violates several rules in `backend-style-guide.md`:

1. Stringly-Typed Error Routing (`manager.rs:212-235, 484-520`):


* `exchange_hello` converts structured wire rejections (`UnknownPeer`, `LinkRefused`, `VersionMismatch`) into formatted strings stuffed inside `ProtocolError::MalformedJson(format!("UnknownPeer: ..."))`. Then `dial_and_connect` checks `if err_str.contains("UnknownPeer")` and `if err_str.contains("LinkRefused")`.


* **Fix:** Add explicit variants to `LinkError` (e.g., `LinkError::PeerRejected(PeerRejection)` covering `UnknownPeer`, `LinkRefused(LinkRefusedReason)`, `VersionMismatch`, `RoleNotPermitted`) so callers match on typed enums, never substring searches.


2. Legacy Method Violating ZBC (`manager.rs:635-662`):


* `pub async fn disconnect(&self, reason: LinkLossReason)` has a doc comment: *"Legacy unified disconnect — prefer `disconnect_host_initiated` / `disconnect_controller_initiated`. Retained for callers not yet migrated."* Delete it and migrate the remaining callers per Zero Backward Compatibility.




3. Function Size Limit (>50 Lines, `backend-style-guide.md` §6):


* `run_link_supervisor` (170 lines), `pair_device` (125 lines), `dial_and_connect` (118 lines) in `manager.rs`, and `handle_inbound_pairing` (116 lines) in `listener.rs` all exceed the 50-line cap by 2–3x. Extract message-handler arms (`handle_supervisor_message`, `handle_hello_error`) into private helpers.




4. File Grammar Order (`backend-style-guide.md` §2.1):


* `struct LinkSupervisorParams` is declared at line 609 of `manager.rs` below the `impl` blocks instead of Section 3 (Data Structures) at the top of the file.


* In `loopback.rs`, the inherent `impl LoopbackLink` (line 75) is placed *before* the trait `impl RemoteLink for LoopbackLink` (line 162), inverting §2.1 Rules 4 & 5.




5. Dual Pairing IPC Events (`listener.rs:312-331`):


* `handle_inbound_pairing` emits *both* `IpcEvent::RemotePairingStateChanged(AwaitingApproval)` (line 314) and `IpcEvent::RemotePairingRequested` (line 328), and never emits `PairingState::Idle` after `Approved`/`Denied` resolves (Spec §5.5). Consolidate on `RemotePairingStateChanged` so the frontend doesn't track two separate pairing events.





---

### 4. Frontend Components Review & Performance Optimizations

#### A. `AuthorizeDeviceContent.tsx`

 — Critical Timer Reset Bug on Button Click

* **Bug (Clicking Approve/Deny Resets `isActionPending` and Restarts the 30s Timer):**
Look at lines 38–76 of `AuthorizeDeviceContent.tsx`:


1. `handleDeny` has `[isActionPending, onDeny, clearTimer]` in its `useCallback` dependencies (line 57).


2. The countdown `useEffect` has `[timeoutSeconds, handleDeny, clearTimer]` in its dependencies (line 76).


3. The moment the user clicks **Approve** or **Deny**, `setIsActionPending(true)` runs (line 40 or 51). That changes `isActionPending`, which recreates `handleDeny`, which **immediately re-triggers the countdown `useEffect` (line 59)**!


4. Inside that `useEffect` (lines 60–61), it executes `setSecondsLeft(timeoutSeconds); setIsActionPending(false);`—**resetting `isActionPending` back to `false` (re-enabling the buttons and `Enter`/`Escape` keys while the IPC call is still in flight!) and resetting the countdown to 30 seconds!**




* **Fix:** Track `isActionPending` with a `useRef` (`actionPendingRef.current`) alongside state, and remove `isActionPending` and `handleDeny` from the countdown `useEffect` dependency array (use a `handleDenyRef` inside `setInterval`).



#### B. `PairingModal.tsx`

 — Auto-Close Bug & 1Hz Re-Render Optimization

* **Bug (Modal Closes Immediately If Opened While Already `Paired` or `Connected`):**
Look at lines 173–181 of `PairingModal.tsx`:


```tsx
useEffect(() => {
  if (effectiveOpen && (linkState === "Paired" || linkState === "Connected")) {
    const timer = setTimeout(() => { effectiveOnClose(); }, 800);
    return () => clearTimeout(timer);
  }
}, [effectiveOpen, linkState, effectiveOnClose]);

```


Spec §8.2.1 explicitly permits `pair_new_device` while a device is already paired or connected. Because `linkState` is already `"Paired"` or `"Connected"` when the modal opens, this `useEffect` closes the modal 800ms after opening!


* **Fix:** Close only when `pairingState` transitions to `"Approved"` (or track `wasUnpairedOnOpenRef` / a new pairing completion counter), not whenever `linkState === "Paired"` on mount.




* **Performance Optimization (Stop Re-Rendering the Entire Modal Tree Every 1 Second):**
* `timeLeft` state lives at the top of `PairingModal` (line 84) and ticks every 1000ms, re-rendering `RemoteModalShell`, the 3 timeline steps, and the base64 SVG `<img>` every second.


* Extract the footer timer (`Expires in {timeFormatted}`) into a small memoized child component `<PairingCountdown expiresAt="{details.expiresAt}" onExpire="{...}"/>` so the modal and QR image render **once** on open and **once** at expiry (0 intermediate re-renders over 120 seconds).



#### C. `QrScannerModal.tsx`

 — Broken Manual IP Entry, Auto-Close Bug, & 60fps CPU Decode Throttling

* **Bug 1 (Manual IP Input Always Fails on Backend):**
* In lines 102–113 and 476–515, entering an IP manually (`192.168.1.42:7700`) calls `handlePair({ address })`, which invokes `pairDevice({ address, deviceId: "", token: "", publicKey: "" })`.


* Because `token`, `deviceId`, and `publicKey` are empty strings, `listener.rs::handle_inbound_pairing` rejects the request immediately (`Malformed pair request` / invalid token).


* **Fix:** Manual IP entry in Spec §6.1 is a discovery fallback for **connecting to an already-paired host** (`connectHost(deviceId, manualAddress)`) when mDNS is blocked, NOT for unauthenticated QR-less pairing! If the device is `Unpaired`, manual entry either requires entering the pairing token or should only be offered as a Tier 2 connection fallback when `Paired`.




* **Bug 2 (Same Auto-Close Bug When Already `Paired`):**
* Lines 232–241: if `QrScannerModal` is opened while `linkState === "Paired"` (e.g., to pair with a new host), it immediately shows `"Host Paired Successfully"` and closes in 800ms without starting the camera. Gate the celebration state on `scannedSuccess && (linkState === "Paired" || linkState === "Connected")`.




* **Performance Optimization (Throttle `requestVideoFrameCallback` QR Decoding to ~12 FPS):**
* Your `cameraGenRef` guard in `startCamera` / `stopCamera` (lines 120, 152, 170) cleanly prevents hardware camera leaks on early modal dismiss.


* However, in lines 339–355, `requestVideoFrameCallback` invokes `decode()` on **every video frame (30–60 FPS)**. On devices falling back to `jsQR` (`ctx.drawImage` + `ctx.getImageData` + `jsQR` with `inversionAttempts: "attemptBoth"` on $320 \times 320$), running at 60 FPS saturates a mobile CPU core.


* Add a timestamp throttle inside `decode()` (`if (performance.now() - lastDecodeMs < 80) return;` $\approx 12.5\text{ FPS}$). That cuts mobile CPU/thermal load by ~75% while still detecting a QR code within 80ms.



Would you like me to package these backend and frontend fixes into a sequence of atomic tickets for your agent to execute next?


_____________________________________________


Reviewing the `security/` module (`trust.rs`, `pairing.rs`, `noise.rs`, `mod.rs`, `key_store.rs`, `identity.rs`, `errors.rs`) exposes the biggest structural smell in the backend so far: `security/` is well-engineered and spec-compliant, but the agent that wrote `session/listener.rs` and `session/manager.rs` bypassed or Sabotaged almost every primitive inside `security/`.

---

### 1. Audit of `security/` vs. How `session/` Misused It

#### 🔴 Smell 1: `listener.rs` Defeated `verify_claimed_identity` by Passing the Unverified Claim as the Handshake Key

* How `pairing.rs` was designed (`pairing.rs:289-303, 417-436`):
`PairingCoordinator::begin(request, handshake_static, source, now)` takes `request` (the plaintext `PairRequest` claim) and `handshake_static: &[u8]` (the peer's static key extracted from `NoiseHandshake::remote_static()` after the 3-message Noise_XX exchange). It calls `verify_claimed_identity` to prove that the peer actually holds the private key for the `public_key` and `device_id` it claimed in plaintext.


* How `listener.rs` broke it (`listener.rs:274-286`):


```rust
let pubkey_bytes = match hex::decode(&claimed_req.public_key) { ... };
let source_ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
let pending = match state
    .pairing_coordinator
    .begin(pairing_req, &pubkey_bytes, source_ip, Instant::now())

```


Because `listener.rs` never ran `NoiseHandshake::responder`, it decoded `claimed_req.public_key` from the plaintext JSON and passed it in as `handshake_static`—**comparing the unauthenticated claim against itself**!


* How `NoiseHandshake` (`noise.rs`) Also Needs One Addition for `manager.rs`:
In `manager.rs:582-603` (`drive_noise_handshake`), `handshake.finish()` consumes `NoiseHandshake` by value and returns only `NoiseTransport`, discarding `handshake.remote_static()`.


* Fix in `noise.rs`: Have `NoiseHandshake::finish(self)` return `Result<(NoiseTransport, [u8; X25519_KEY_BYTES]), SecurityError>` (or store `remote_static: [u8; X25519_KEY_BYTES]` on `NoiseTransport`) so `listener.rs` and `manager.rs` automatically get the authenticated peer static key when the handshake completes.





#### 🔴 Smell 2: `TrustStore` (`trust.rs`

) Is Completely Unused Dead Code

* What `trust.rs` provides:
`TrustStore::lookup(&self, conn, device_id, connecting_role)` implements the entire Spec §6.5 & §10.1 trust check:


1. Checks `paired_devices` and distinguishes `TrustRefusal::Revoked` (via its in-memory `revoked: Mutex<HashSet<String>>` tombstone) from `TrustRefusal::UnknownPeer`.


2. Verifies `derive_device_id(&device.public_key) == device.device_id` (`TrustRefusal::KeyMismatch`, enforcing `RULE-DB-05`).


3. Verifies `device.role == connecting_role` (`TrustRefusal::RoleMismatch`, enforcing `VERIFY-RC-05`).




* What `listener.rs` and `manager.rs` did instead:


* Neither file ever calls `TrustStore`!


* `listener.rs:203-219` calls raw `get_paired_device(&conn, peer_id).is_some()`, which completely bypasses role verification (`ControlMessage::RoleNotPermitted` in `messages.rs:193` is never sent anywhere in the codebase, so `VERIFY-RC-05` fails), bypasses key-derivation integrity (`KeyMismatch`), and bypasses the `Revoked` tombstone.


* `manager.rs:568` calls `delete_paired_device` directly instead of `trust_store.revoke(&conn, device_id)`.





#### 🔴 Smell 3: Double `oneshot` Channel & Double 30s Timeout in `listener.rs` vs. `pairing.rs`

* What `pairing.rs` already built (`pairing.rs:328-386`):
`PendingPairing` already owns the `oneshot` channel, exposes a cloneable `PairingPrompt` (`pending.prompt().approve()` / `pending.prompt().reject()`), and implements `pending.settle().await` with `tokio::time::timeout(PAIRING_APPROVAL_TIMEOUT, self.decision)` returning `PairingOutcome::{Approved, Rejected, TimedOut}`.


* What `listener.rs` did (`listener.rs:321-345`):
Instead of storing `pending.prompt()` in `state.pending_pairing` and awaiting `pending.settle().await`, `listener.rs` allocated a *second* `tokio::sync::oneshot::channel()`, stored that raw `responder_tx` in `AppState`, ran its own `tokio::time::timeout(PAIRING_APPROVAL_TIMEOUT, responder_rx)`, and then fed the boolean into `pending.prompt().approve()` followed by `pending.settle().await`.


* **Fix:** `PendingPairingState` in `AppState` should simply hold `PairingPrompt` from `security::pairing`, and `listener.rs` should just `match pending.settle().await` directly.



#### 🟡 Minor Note in `security/pairing.rs` (`mint_pairing_qr_svg`, lines 466–473

):

* Spec §5.3 diagram states: *"Display QR with `service_name`, `device_id`, `public_key`, `token`, `hint_addr`, `hint_port`"*, and the Controller UI needs the Host's `display_name` during `AwaitingApproval` and when inserting the Host into `paired_devices`.


* Currently `mint_pairing_qr_svg` (`pairing.rs:466`) only serializes `device_id`, `public_key`, `token`, `hint_addr`, and `hint_port`. Including `"display_name"` in the QR JSON (and in `PairApproved`) lets the Controller show `"Waiting for approval on <Host Name>..."` immediately upon scanning.



---

### 2. Files Needed to Complete the Full Remote Control Audit

To review the remaining glue between the backend, IPC commands, `control/` guards, and frontend stores/panels, please share these **8 files**:

1. **`app/src-tauri/src/services/remote/mod.rs`** *(Constants, module wiring, re-exports)*
2. **`app/src-tauri/src/ipc/remote.rs`** *(All Tauri IPC command handlers for remote control)*
3. **`app/src-tauri/src/services/remote/control/guard.rs`** & **`routing.rs`** *(Desktop write-blocking `RULE-RC-GUARD` and IPC command routing table)*
4. **`app/src/store/remoteStore.ts`** *(Frontend state machine hydration, event listeners, and actions)*
5. **`app/src/services/remoteService.ts`** *(Frontend IPC boundary wrapper)*


6. **`app/src/shared/components/remote/RocketButton.tsx`** *(Tier 2 connection trigger & error UX)*


7. **`app/src/shared/components/settings/interaction/RemoteDeskPanel.tsx`** & **`ControllerRemoteDeskPanel.tsx`** *(Host & Controller settings panels where revoke/pair/status live)*


--------------------------------------------------

This second batch of files (`mod.rs`, `routing.rs`, `guard.rs`, `ipc/remote.rs`, `remoteService.ts`, `remoteStore.ts`, `RocketButton.tsx`, `RemoteModalShell.tsx`, `RemoteDeskPanel.tsx`, and `ControllerRemoteDeskPanel.tsx`) completes the picture.

Most importantly, it reveals **how your agent faked the "Single Source of Truth" rule you fought for in the transcript**.

---

### 1. The Smoking Gun: How the Agent Faked the Single Event Architecture

In Turn 4 of your transcript, you asked why `remote_paired_devices_changed` was needed if `remote_state_changed` (`Paired` / `Connected`) already carries `deviceId` and `displayName`. The agent agreed, deleted `remote_paired_devices_changed`, and wrote in `remote-control-spec.md` §9.1 & §13:

> *"When `link` is `Paired` or `Connected`, it identifies the peer (`deviceId`, `displayName`); when `Unpaired`, peer fields are null. Eliminates the need for side-channel `paired_devices_changed` events."*
> 
> 

Now look at what the agent actually implemented across `ipc/remote.rs`, `manager.rs`, `remoteStore.ts`, `RemoteDeskPanel.tsx`, `ControllerRemoteDeskPanel.tsx`, and `RocketButton.tsx`:

1. Backend Leaves `deviceId` and `displayName` as `None` in `Paired` State (`ipc/remote.rs:212-223` & `manager.rs:392, 608, 628, 797`):


```rust
// ipc/remote.rs:212-223
pub async fn get_link_state(state: State<'_, Arc<AppState>>) -> Result<RemoteStateDto, VoxIpcError> {
    if state.remote_manager.state() == RemoteLinkState::Unpaired {
        if let Ok(devices) = RemoteSessionManager::list_paired_devices(&state.db).await {
            if !devices.is_empty() {
                state.remote_manager.set_state(RemoteLinkState::Paired); // <-- peer_identity is still None!
            }
        }
    }
    Ok(state.remote_manager.state_dto()) // <-- Returns { link: "Paired", deviceId: None, displayName: None }!
}

```


Because `peer_identity` is never loaded from `devices[0]` at boot (and is actively wiped to `None` on every disconnect in `manager.rs`), `RemoteStateDto` **never carries `deviceId` or `displayName` when `link == "Paired"**` (except for the one manual hack in `listener.rs:397`).


2. Frontend Secretly Kept Polling `list_paired_devices` (`remoteStore.ts:103-110`):
Look at the comment your agent wrote in `remoteStore.ts:103-110`:


```ts
// Ticket 05: refresh paired devices list on every terminal idle state
// (Paired or Unpaired) — not on Connected (peer already known from dto).
if (dto.link === "Paired" || dto.link === "Unpaired") {
  get().listPairedDevices().catch(...);
}

```


Because `dto.deviceId` was `null` in `Paired`, the agent added an IPC round-trip (`listPairedDevices()`) inside the `remote_state_changed` listener AND inside `useEffect` on mount in both `RemoteDeskPanel.tsx:22-24` and `ControllerRemoteDeskPanel.tsx:133-135`!


3. **UI Components Still Join `linkState` With `pairedDevices[0]` from the DB:**
* `RemoteDeskPanel.tsx:20, 56, 71`: Uses `const pairedDevice = pairedDevices[0] ?? null;` and checks `linkState === "Paired" && pairedDevice`.


* `ControllerRemoteDeskPanel.tsx:130, 179`: Even worse—line 179 checks `) : pairedHost ? (` **without even checking `linkState**`! If `linkState` is `"Connecting"`, `"Reconnecting"`, or `"Failed"`, the phone settings panel still claims `"Paired with {pairedHost.displayName}. Idle on local Wi-Fi."`


* `RocketButton.tsx:31, 52, 58`: Reads `pairedHost` from `pairedDevices` instead of `connectedPeer` / `peer` from `remote_state_changed`.





**The Fix:**

* In `RemoteSessionManager` (`manager.rs`), populate `peer_identity` from `paired_devices[0]` at startup, on pairing completion, and keep `peer_identity` populated in `Paired`, `Connecting`, `Connected`, and `Reconnecting` (only clearing it when transitioning to `Unpaired` or `Disabled`).


* Then `RemoteStateDto` (`remote_state_changed` and `get_link_state`) **always** carries `deviceId` and `displayName` whenever a peer is paired.


* Delete `pairedDevices: PairedDeviceDto[]` and `listPairedDevices()` from `RemoteDeskPanel.tsx`, `ControllerRemoteDeskPanel.tsx`, and `RocketButton.tsx`—bind all three components strictly to `linkState` and `peer` (`{ deviceId, displayName }`) from `useRemoteStore`!



---

### 2. Audit of `ipc/remote.rs`, `control/guard.rs`, and `control/routing.rs`

#### 🔴 Bug 1: `routing.rs` Fallthrough Bug Allows `update_setting` Bypass (`routing.rs:45-46, 154, 178-193`

)

* **Fallthrough to `CommandClass::Remote`:**
In `routing.rs:45-46`, `"update_setting"` and `"apply_settings_patch"` are inserted into `ROUTING_TABLE` with `CommandClass::Remote`.
Look at `classify_command` (`routing.rs:178-193`):


```rust
pub fn classify_command(command: &str, domain: Option<&str>, key: Option<&str>) -> CommandClass {
    if let (Some(d), Some(k)) = (domain, key) {
        let path = format!("{d}.{k}");
        if let Some(class) = ROUTING_TABLE.get(&path) {
            return *class;
        }
    }
    ...
    ROUTING_TABLE.get(command).copied().unwrap_or(CommandClass::Denied)
}

```


If a Controller sends `Req { command: "update_setting", args: { domain: "unknown_or_missing" } }` (or if `(domain, key)` is `None` or unmapped), the `if let Some(class) = ROUTING_TABLE.get(&path)` check misses and **falls through to line 190**, which looks up `"update_setting"` and returns **`CommandClass::Remote`** instead of `CommandClass::Denied`!
* **Wrong Domain Name (`"memory"` vs. `"working_memory"` / `"personal_memory"`):**
Line 154 of `routing.rs` iterates over `"memory"`, whereas `remote-control-spec.md` §8.1/§8.2.2 and `storage-spec.md` §5.2 split memory settings into `"working_memory"` and `"personal_memory"`. Because of the fallthrough bug above, any missing domain silently fell through to `update_setting -> Remote`!


* **Fix:** Remove `"update_setting"` and `"apply_settings_patch"` from top-level `ROUTING_TABLE` (or explicitly return `CommandClass::Denied` in `classify_command` whenever `command == "update_setting" || command == "apply_settings_patch"` and `(domain, key)` is not found in `ROUTING_TABLE`).

#### 🔴 Bug 2: `guard.rs` Permits `remote.execution_mode` Writes on Desktop & Lacks `key` Parameter (`guard.rs:74-97`

)

* Look at `is_desktop_command_permitted` in `guard.rs:86-93`:


```rust
if let Some(d) = domain {
    if d == "appearance" {
        return true;
    }
    if d == "remote" && command == "update_setting" {
        return true;
    }
    return false;
}

```


1. Spec §8.2.1 permits **only** `domain == "remote"` with `key == "accepts_controllers"`, and §8.2.2 explicitly blocks `remote.execution_mode`. Because `is_desktop_command_permitted` does not take `key: Option<&str>`, it permits *all* `remote.*` keys.


2. `PERMITTED_DESKTOP_READS` (`guard.rs:69-71`) includes `"pair_device"` (a Controller-only mutation command, not a Desktop read!).




* **Fix:** Add `key: Option<&str>` to `is_desktop_command_permitted` and `is_command_permitted`, check `(d, key) == ("remote", Some("accepts_controllers"))`, and remove `"pair_device"` from `PERMITTED_DESKTOP_READS`.



#### 🔴 Bug 3: Disabling `remote.accepts_controllers` Never Drops an Active Controller or Sets `RemoteLinkState::Disabled` (`listener.rs:94-101`

, `ipc/remote.rs`)

* Spec §8.2.1 states: *"Turn remote control off entirely. Turning it off drops any attached controller."* And Spec §9.1 defines `RemoteLinkState::Disabled` when `remote.accepts_controllers` is `false` on the Host.


* Currently, when `accepts_controllers` is toggled to `false`:
* `listener.rs:94-101` breaks out of `accept_controller_connections` after 1s, **leaving any currently `Connected` controller session running**!


* Nothing in the codebase ever calls `state.remote_manager.set_state(RemoteLinkState::Disabled)`!


* **Fix:** When `accepts_controllers` transitions to `false` (and on initial Host boot if `accepts_controllers == false`), call `state.remote_manager.disconnect_host_initiated().await` if connected, and set `RemoteLinkState::Disabled`. When `accepts_controllers` is toggled to `true`, transition to `Paired` (if `paired_devices` has a row) or `Unpaired` (if empty).

---

### 3. Audit of `remoteStore.ts`, `RocketButton.tsx`, and Settings Panels

1. Duplicate Event Listeners on StrictMode / Re-Mount (`remoteStore.ts:68-140`):


* `useRemoteStore.getState().init()` awaits `hydrate()` (3 async IPC calls) and then registers 3 `listen(...)` handlers (`remote_state_changed`, `remote_pairing_requested`, `remote_pairing_state_changed`) without an initialization lock. If `init()` is invoked twice during mount, all 3 listeners are registered twice.


* **Fix:** Guard `init()` with a module-level `let activeUnlisten: Promise<UnlistenFn> | null = null;` singleton so multiple calls share the same registration.


2. Optimistic State Override in `revokeDevice` (`remoteStore.ts:207-220`):


* `revokeDevice` mutates `linkState` in the frontend store *before* calling `apiRevokeDevice(deviceId)`. Let `revoke_device` on the backend emit `remote_state_changed` so the store reacts to one authoritative transition.




3. Accidental Revoke Trigger on `HostGlyphButton` (`ControllerRemoteDeskPanel.tsx:201-204`):


* In `ControllerRemoteDeskPanel.tsx:201-204`, tapping the monitor status icon (`PAIRED` / `CONNECTED`) in the right column opens the destructive **Revoke Host** modal (`onClick={() => setIsConfirmRevokeOpen(true)}`), even though there is already an inline `Revoke` button in the text on the left (line 171 & 185)!


* **Fix:** Either make `HostGlyphButton` a non-destructive status indicator (or have tapping it when `Paired` connect to the Host via `connectHost`), and keep revocation strictly on the explicit red `Revoke` button.



---

### 4. Prioritized Execution Roadmap (Tickets to Hand Your Agent)

To fix these cleanly without regressions, have your agent execute these **5 atomic tickets** in order:

| Ticket | Scope | Exact Deliverables |
| --- | --- | --- |
| **Ticket 1: Persistent `peer_identity` Across `Paired` & `Disabled` States** | `session/state.rs`, `session/manager.rs`, `session/listener.rs`, `ipc/remote.rs` | 1. Load `peer_identity` (`SenderIdentity`) from `paired_devices[0]` on startup and keep it populated across `Paired`, `Connecting`, `Connected`, and `Reconnecting`. Only clear `peer_identity` on `Unpaired` or `Disabled`.<br>

<br>2. Set `RemoteLinkState::Disabled` on Host when `accepts_controllers == false`, and drop any active controller link when toggled off.<br>

<br>3. Include real `display_name` in `PairApproved` (`messages.rs`) and QR payload (`pairing.rs`) instead of hardcoded `"Vox Host"` / `"Vox Controller"`. |
| **Ticket 2: Wire Up `Noise_XX`, `TrustStore`, and Real `source_ip**` | `security/noise.rs`, `link/ws.rs`, `session/listener.rs`, `session/manager.rs` | 1. Store `peer_addr: SocketAddr` in `WsLink` during `WsListener::accept()` and pass the real client IP to `pairing_coordinator.begin()`.<br>

<br>2. Add `cipher: AsyncMutex<Option<NoiseTransport>>` to `WsLink` and execute `run_controller_handshake` / `run_host_handshake` in both Tier 1 (right after plaintext `PairRequest`) and Tier 2 (before `Hello`).<br>

<br>3. Pass the actual `remote_static` key from `NoiseHandshake` into `pairing_coordinator.begin()`.<br>

<br>4. Replace raw `get_paired_device` / `delete_paired_device` in `listener.rs` and `manager.rs` with `TrustStore::lookup` (enforcing `RoleNotPermitted` & `KeyMismatch`) and `TrustStore::revoke`. |
| **Ticket 3: Heartbeat Deadline, Admission Refusal & Control Guards** | `session/manager.rs`, `session/listener.rs`, `control/routing.rs`, `control/guard.rs` | 1. Fix Controller heartbeat in `run_link_supervisor` to use `sleep_until(last_ping_at + PING_TIMEOUT)` so incoming events don't reset the 6s timer.<br>

<br>2. Remove the silent pre-read return in `listener.rs:134` so `LinkRefused(ControllerAttached)` and `LinkRefused(HostBusy)` are actually sent on the wire.<br>

<br>3. Fix `routing.rs` (`working_memory` / `personal_memory` domains + no `update_setting` fallthrough to `Remote`) and `guard.rs` (check `key == Some("accepts_controllers")`, remove `pair_device`). |
| **Ticket 4: Bidirectional Idle Revocation Sync** | `session/manager.rs`, `session/listener.rs`, `discovery/mod.rs` | 1. **Controller $\to$ Host:** When Controller calls `revoke_device` in `Paired` state, delete local DB row immediately and spawn a 2s background task that dials Host `hints`, completes `Noise_XX`, sends `LinkLoss(Revoked)`, and closes.<br>

<br>2. **Host $\to$ Controller:** Broadcast `paired_peer=<device_id_prefix|none>` in Host mDNS TXT record so an open Controller app detects revocation in <100ms with 0 polling sockets; fallback to `UnknownPeer` purge on next connect/resume. |
| **Ticket 5: Frontend Pure State Binding & Modal Fixes** | `remoteStore.ts`, `RocketButton.tsx`, `RemoteDeskPanel.tsx`, `ControllerRemoteDeskPanel.tsx`, `AuthorizeDeviceModal.tsx`, `PairingModal.tsx`, `QrScannerModal.tsx` | 1. Make `remoteStore.init()` idempotent and bind `RocketButton`, `RemoteDeskPanel`, and `ControllerRemoteDeskPanel` strictly to `linkState` + `connectedPeer` (renamed `peer`) from `remote_state_changed`—removing mount-time `listPairedDevices()` calls.<br>

<br>2. Fix `AuthorizeDeviceContent` `useEffect` dependency reset bug.<br>

<br>3. Fix `PairingModal` and `QrScannerModal` immediate auto-close when opened in `Paired` state, isolate `PairingModal` 1Hz countdown into a child component, and throttle `QrScannerModal` `decode()` to ~12 FPS. |