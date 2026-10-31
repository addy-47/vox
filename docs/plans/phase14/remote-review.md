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


---

# Tier 3 and test review 


### 1. Acknowledgment: Full 4,000-Line `changes_2.diff` Received Intact

I received the entire `changes_2.diff` from start (`AGENTS.md`) to finish (`app/src-tauri/tests/remote_security_test.rs`, line +115) with **zero truncation** across all 31 modified/added files. You do not need to re-send individual files from this diff.

---

### 2. What Landed Cleanly in This Diff

* **Wire Protocol & Security Primitives (`messages.rs`, `redaction.rs`, `tokens.rs`):** `ControlMessage::SessionToken` (19 variants), `redact_settings_value` (covering maps, server arrays, `tts.azure_tts.key`, and suffixes), and single-use `AudioSessionTokenStore` with static-key binding and anti-replay tests are all clean and well-tested.


* **Centralized Desktop Write Guard Hook (`lib.rs`, `ipc/pipeline.rs`, `ipc/settings.rs`):** Moving `is_desktop_command_permitted` out of individual IPC functions and into a single `handle_invoke` interceptor in `lib.rs` (enforcing `RULE-RC-GUARD` in one place) is much cleaner than scattered checks.


* **FIFO Event Coalescer (`manager.rs::run_event_forwarder`) & Reconnect Loop (`spawn_controller_reconnect`):** The 30Hz `telemetry` throttle and 50ms `llm_token` string accumulator with pre-event flushing preserve wire ordering, and exponential backoff reconnection (`1s` $\to$ `30s`, max 10 attempts) is wired into the supervisor fault and timeout paths.


* **Dictation Hotkey Gating (`hotkey.rs:142`):** Properly ignores the desktop dictation hotkey while `RemoteLinkState == Connected` per Spec §11.



---

### 3. Seven Critical Bugs & Missing Controller Seams in `changes_2.diff`

While the **Host** half of Tier 3 is ~85% wired, the **Controller** half of Tier 3 and several Host audio/gateway seams have ship-breaking bugs that will prevent a real remote voice session from working end-to-end:

#### 🔴 Bug 1 (P0 Downlink Audio Crash & Pipeline Deadlock): `PlaybackEngine::push_samples` Violates 64KB Noise Limit, Scrambles Frame Order, and Never Fires `PlaybackFinished` (`services/audio/playback.rs:168-177`)

Look at what your agent wrote in `playback.rs:168-177`:

```rust
if let Some(sink) = self.remote_sink.read().clone() {
    let i16_samples: Vec<i16> = chunk_24khz
        .iter()
        .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
        .collect();
    tokio::spawn(async move {
        sink.send_samples(i16_samples).await;
    });
    return;
}

```

Three things break here on the very first TTS response:

1. **Exceeds 65,519-Byte Noise Payload Cap $\to$ Dropped Audio:** TTS synthesizes full clause buffers (`chunk_24khz` is often 35,000–60,000 samples = 70 KB–120 KB of `i16` bytes). `NetworkAudioSink::send_samples` wraps the entire `Vec<i16>` into a single `AudioFrame` and passes it to `NoiseTransport::encrypt`, which **rejects any payload > 65,519 bytes** (`NOISE_MAX_PAYLOAD_BYTES`)! Spec §7.4 mandates **20 ms frames (480 samples = 960 bytes at 24 kHz)**.


2. **Unordered `tokio::spawn` Scrambles Audio Chunks:** Spawning an independent async task per `push_samples` call without an ordered MPSC queue means Tokio worker threads can acquire `WsLink::writer.lock().await` out of order, scrambling speech syllables.


3. **Early `return;` Prevents `VoxEvent::PlaybackFinished` (Pipeline Deadlocks in `Speaking`):** Returning early at line 176 skips `turn_metrics.record_tts_first_audio()`, skips updating the playback state atomics, and **never emits `VoxEvent::PlaybackFinished**`—leaving the Host pipeline permanently stuck in `InteractionState::Speaking` after the first sentence!



* **The Fix:** In `NetworkAudioSink::send_samples`, slice `samples` into `chunks(480)` (20 ms @ 24 kHz) and send them sequentially over a bounded MPSC channel owned by `NetworkAudioSink` (not raw `tokio::spawn` per call). And in `PlaybackEngine::push_samples`, record `turn_metrics` and pace/emit `PlaybackFinished` (or let `AudioSink` handle the network sink so `PlaybackEngine` lifecycle logic runs unchanged).

---

#### 🔴 Bug 2 (P0 Uplink Audio Drop): Race Between `handle_inbound_audio_socket` and `restart_engine_inner` (`listener.rs:533-543` & `core/state.rs:247-259`)

Look at `handle_inbound_audio_socket` in `listener.rs:533-543`:

```rust
let ingestor = state.remote_audio_ingestor.lock().clone();
let link = state.remote_manager.clone().get_audio_link().await;
if let Some(link) = link {
    tokio::spawn(async move {
        while let Ok(payload) = link.recv_audio().await {
            if let Some(ing) = &ingestor {
                let _ = ing.ingest(payload);
            }
        }
    });
}

```

1. **Stale `ingestor` Snapshot:** When the control channel transitions to `Connected`, `core/state.rs:252` spawns `restart_engine_inner(&app, &st, false)` asynchronously (which stops the engine, sets `*state.remote_audio_ingestor.lock() = None`, and then recreates `NetworkAudioCapture` a few hundred milliseconds later in `engine.rs:186`). Meanwhile, the Controller immediately dials the audio socket and hits `listener.rs:533`, cloning `state.remote_audio_ingestor` **once before the `while` loop** while it is still `None` (or holds the old pre-restart capture instance)! As a result, `ing` is `None` or dead for the entire session, and **zero microphone audio from the phone ever reaches VAD/STT**.


2. **Active Session Wiped on Connect (`reengage_session: false`):** Calling `restart_engine_inner(&app, &st, false)` in `core/state.rs:256` with `reengage_session = false` kills any active session on the Host down to `InteractionState::Idle` when the phone connects or disconnects.



* **The Fix:** Pass `state_arc = Arc::clone(state)` into the audio reader task in `listener.rs:536` and read `state_arc.remote_audio_ingestor.lock().clone()` inside the `while` loop (or only on `None` / epoch change), and pass `reengage_session: true` (or pause on disconnect) so connecting doesn't destroy the active session.



---

#### 🔴 Bug 3 (P0 Security & Spec Violation): `RealCommandGateway` Never Calls `classify_command` and Rejects ~50 Valid `Remote` Commands (`control/gateway.rs:38-183`)

In `lib.rs:667`, `state_arc.remote_manager.set_gateway(gw)` replaces `DefaultCommandGateway` with `RealCommandGateway`. Look inside `RealCommandGateway::dispatch` (`gateway.rs:39-183`):

1. **Routing Table Bypass (`VERIFY-RC-07` & Spec §8.1 / §10.4):** `RealCommandGateway::dispatch` **never calls `classify_command(&req.command, domain, key)**`!
* If the Controller sends `update_setting` for `appearance.theme` or `remote.accepts_controllers` (both classified as `CommandClass::Local` in `routing.rs:163-167`), `RealCommandGateway:131-144` executes them directly on the Host!


* Lines 156–175 of `gateway.rs` also expose `list_paired_devices`, `revoke_device`, `disconnect_device`, `get_link_state`, and `get_command_routing`—all of which are classified as `CommandClass::Local` in `routing.rs:134-143`!




2. **Missing `Remote` Command Handlers (`gateway.rs:177`):** Every History, Session, Project, Memory, Model Catalog, Voice, and Notification command classified as `CommandClass::Remote` in `routing.rs` falls through to line 177 (`_ => "Command '...' is not implemented on the remote gateway"`), breaking the phone's History, Memory, Voices, and Settings panels (US-RC-03).



* **The Fix:**
1. At the very top of `RealCommandGateway::dispatch`, run:
```rust
let domain = req.args.get("domain").and_then(|v| v.as_str());
let key = req.args.get("key").and_then(|v| v.as_str());
if matches!(
    crate::services::remote::control::classify_command(&req.command, domain, key),
    CommandClass::Local | CommandClass::Denied
) {
    return ControlMessage::Err(ErrResponse {
        request_id: req.request_id,
        code: RemoteErrorCode::Denied,
        message: format!("Command '{}' is not permitted remotely", req.command),
    });
}

```


2. Wire the remaining `CommandClass::Remote` IPC handlers (`ipc::history::*`, `ipc::memory::*`, `ipc::voices::*`, `ipc::models::*`, `ipc::notifications::*`) into `RealCommandGateway::dispatch`.





---

#### 🔴 Bug 4 (P0 Controller Missing Seam): Controller Silently Drops All Forwarded `ControlMessage::Event`s and Never Applies `AttachSnapshot` (`manager.rs:1509-1519`)

Look at `handle_supervisor_message` in `manager.rs:1509-1519`:

1. **`ControlMessage::Event(ev)` Dropped:** When the Host forwards `state_changed`, `transcript_partial`, `transcript_final`, `llm_token`, `turn_metrics`, or `telemetry` over the wire as `ControlMessage::Event(ev)`, the Controller's `handle_supervisor_message` hits `_ => SupervisorFlow::Continue` at line 1519 and **throws the event away**! The Controller's local React UI never receives a single pipeline event from the Host.


2. **`ControlMessage::AttachSnapshot(snapshot)` Never Emitted to UI:** Line 1514 writes `params.last_snapshot.write().replace(snapshot)`, and nothing in the codebase ever reads `last_snapshot` or hydrates the Controller's frontend stores (`sessionStore`, `settingsStore`) with the snapshot's `state`, `session_id`, `settings`, and `turns`.



* **The Fix:** Give `RemoteSessionManager` an `on_remote_event: Option<Arc<dyn + Fn(&str, Send Sync serde_json::Value)>>` callback wired in `AppState::new` to `app_handle.emit(event_name, payload)`. When `ControlMessage::Event(ev)` arrives on the Controller, invoke that callback. When `ControlMessage::AttachSnapshot(snap)` arrives on the Controller, emit `state_changed` with `snap.state` and an event/callback to hydrate `session_id`, `turns`, and redacted `settings`.



---

#### 🔴 Bug 5 (P0 Controller Missing Seam): Controller Has Zero Audio Uplink/Downlink Workers and Zero Outbound RPC Forwarding

Look at the Controller side of `dial_secondary_audio_socket` (`manager.rs:1387-1418`) and `handle_invoke` (`lib.rs:211-240`):

1. **Controller Audio Socket Sits Idle:** After `dial_secondary_audio_socket` stores `ws_link` in `*audio_slot.write().await = Some(Arc::new(ws_link))` on line 1412, **nothing on the Controller ever calls `send_audio` (to stream the phone's microphone + AEC 16kHz PCM16 frames to the Host) or `recv_audio` (to play the Host's 24kHz TTS frames on the phone's speaker)**!


2. **Controller Local `invoke` Never Calls `remote_manager.call_remote()`:** When the phone is `Connected` to the Host and the phone UI invokes `start_session`, `submit_text_input`, `ptt_start`, or `update_setting`, `handle_invoke` in `lib.rs` executes them against the **phone's local `AppState**` instead of routing `CommandClass::Remote | CommandClass::Split` commands through `state.remote_manager.call_remote(cmd, args)`!



---

#### 🔴 Bug 6 (P1 Event Fanout Bug): Host Forwards Its Own `remote_state_changed` Event Over the Wire to the Controller (`core/events.rs:335-353`)

Look at `event_to_value` and `should_suppress_and_forward` in `core/events.rs:335-353`:

* Because `event_to_value` serializes `IpcEvent::RemoteStateChanged`, `RemotePairingStateChanged`, and `RemotePairingRequested`, whenever `RemoteLinkState` is `Connected` on the Host, `should_suppress_and_forward` forwards `remote_state_changed` (carrying the **Host's** view of `peer`: the phone's `deviceId` and `displayName`) over the wire to the Controller!


* **The Fix:** In `event_to_value` (`core/events.rs:335-337`), return `None` for `IpcEvent::RemoteStateChanged`, `IpcEvent::RemotePairingStateChanged`, and `IpcEvent::RemotePairingRequested` so local transport/pairing lifecycle events are never forwarded across the wire as pipeline/system events.



---

#### 🟡 Bug 7 (P1 Guard & Lint Hygiene): `apply_settings_patch` Blocked in `lib.rs:225-233` & Three Banned `#[allow(...)]` Attributes in `manager.rs`

1. **`apply_settings_patch` Guard Bug (`lib.rs:225-233`):** `apply_settings_patch` passes `{ "patch": { "<domain>": { "<key>": ... } } }`, not top-level `"domain"` and `"key"`. In `handle_invoke`, `domain` and `key` evaluate to `None`, so `is_desktop_command_permitted("apply_settings_patch", None, None, &Connected)` returns `false`, blocking desktop theme changes (`appearance`) and toggling `remote.accepts_controllers` off via patch. Extract `(domain, key)` from `v.get("patch")` when `cmd == "apply_settings_patch"`.


2. **Banned `#[allow(...)]` Attributes (`manager.rs:92, 116, 1228`):** `backend-style-guide.md` §6 enforces zero `#[allow(...)]` attributes in `src/`. Replace the complex inline types on lines 92 and 116 with type aliases (`type EventForwarderTx = ...; type PauseSessionCallback = ...;`) and remove `#[allow(clippy::while_let_loop, clippy::await_holding_lock)]` on line 1228.

---

Tracing every user story (`US-RC-01` through `US-RC-11`) and verification criterion (`VERIFY-RC-01` through `VERIFY-RC-22`) end-to-end across your full files (`manager_3.rs`, `listener_3.rs`, `sink.rs`, `capture.rs`, `gateway.rs`, and `changes_2.diff`) plus your three test suites (`remote_security_test.rs`, `remote_link_test.rs`, `remote_connection_test.rs`) uncovered **5 additional production bugs** (beyond the 7 from last turn) and **5 false-green / bypassed-seam flaws in the tests**.

---

### 1. Five Additional Production Bugs Found via User-Story Tracing

#### 🔴 Bug 8 (`US-RC-07` / `US-RC-08` / `US-RC-02`): `handle_invoke` in `lib.rs` Blocks the Mobile Controller From Running Any Commands When Connected!

* **Where:** `app/src-tauri/src/lib.rs:211-238` in `changes_2.diff`.


* **User Story Trace:** The user connects their phone to the laptop (`RemoteLinkState::Connected` on both devices) and taps Engage (`start_session`), speaks/types (`submit_text_input`), or toggles a setting (`update_setting`) on the **phone**.
* **What Actually Happens in Code:**
```rust
// lib.rs:233
if !is_desktop_command_permitted(cmd, domain, key, &state.remote_manager.state()) {
    resolver.respond(Err::<(), _>(tauri::ipc::InvokeError::from(VoxIpcError::InvalidState(
        "Remote session active: desktop write command blocked".to_string(),
    ))));
    return true;
}

```


`lib.rs` runs on **both** Desktop (`SessionRole::Host`) and Android (`SessionRole::Controller`). When the phone connects to the laptop, `state.remote_manager.state()` on the phone is `RemoteLinkState::Connected`. Because `handle_invoke` never checks `state.remote_manager.role()`, **the phone's own Tauri backend blocks every write command invoked by the phone UI** with `"Remote session active: desktop write command blocked"`!


* **The Fix:** Branch on `state.remote_manager.role()` inside `handle_invoke`:


* If `SessionRole::Host`: enforce `is_desktop_command_permitted(...)`.


* If `SessionRole::Controller` and `state == RemoteLinkState::Connected`: check `classify_command(cmd, domain, key)`. If `CommandClass::Remote` or `CommandClass::Split`, forward the command over the control socket via `state.remote_manager.call_remote(cmd, json_args).await` and respond via `resolver`! If `CommandClass::Local`, let it execute locally on the phone.





---

#### 🔴 Bug 9 (`US-RC-06` — Auto-Reconnect on Wi-Fi Hiccup): `spawn_controller_reconnect` Aborts After Attempt #1 Because `dial_and_connect` Overwrites `RemoteLinkState::Reconnecting`

* **Where:** `manager_3.rs:995-1003` and `manager_3.rs:1697-1713`.


* **User Story Trace:** Wi-Fi drops for 5 seconds. The Controller transitions to `RemoteLinkState::Reconnecting` and spawns `spawn_controller_reconnect` (`RECONNECT_MAX_ATTEMPTS = 10`, exponential backoff 1s $\to$ 30s).


* **What Actually Happens in Code:**
1. After the first 1-second sleep, `spawn_controller_reconnect` calls `manager.clone().dial_and_connect(...)` (`manager_3.rs:1703`).


2. At line 995, `dial_and_connect` immediately overwrites the state: `self.set_state(RemoteLinkState::Connecting)`.


3. Because Wi-Fi is still recovering at $t = 1\text{s}$, `perform_dial_handshake` fails, and line 1001 sets:
`self.set_state(RemoteLinkState::failed(format!("Dial failed: {e}"), kind))`.


4. Control returns `Err(_)` to `spawn_controller_reconnect` at line 1706, which checks:
```rust
if manager.state() != RemoteLinkState::Reconnecting {
    return;
}

```


5. Because `dial_and_connect` mutated `manager.state()` to `Failed`, `manager.state() != RemoteLinkState::Reconnecting` is **always true** on the very first failed retry, killing the reconnect loop after 1 attempt!




* **The Fix:** Extract an internal `try_dial_and_connect(&self, db, device_id, address, is_reconnect: bool)` helper (or check `let was_reconnecting = self.state() == RemoteLinkState::Reconnecting;`). When `is_reconnect` is true:
* Stay in `RemoteLinkState::Reconnecting` during the dial attempt (do not flip to `Connecting`).


* On transport/timeout error during a reconnect attempt, do **not** transition to `Failed`—keep state as `Reconnecting` and only transition to `Failed` when `handle_dial_rejection` encounters a terminal auth error (`UnknownPeer`, `RoleNotPermitted`, `VersionMismatch`) or after all `RECONNECT_MAX_ATTEMPTS` are exhausted.





---

#### 🔴 Bug 10 (`US-RC-05` & `VERIFY-RC-11` / `VERIFY-RC-14`): Host Heartbeat Timeout Forgets to Pause Session, and `on_state_change` Engine Restart Wipes `Paused` State Back to `Idle`

* **Where:** `manager_3.rs:1622-1647`, `core/state.rs:246-259`, and `ipc/pipeline.rs:40-85`.


* **User Story Trace:**
1. **Case A (3 Missed Pongs — `VERIFY-RC-14`):** In `handle_host_heartbeat_tick` (`manager_3.rs:1622-1647`), when `*missed_pongs >= HEARTBEAT_MAX_MISSED_PONGS`, the Host closes the link and transitions to `Paired`, **but forgets to invoke `params.pause_session_fn` and `mark_paired_device_seen**` (which were added to `handle_supervisor_disconnect` and `handle_supervisor_fault`, but omitted from `handle_host_heartbeat_tick`!).


2. **Case B (Intentional Disconnect — `US-RC-05` & `VERIFY-RC-11`):** When the Controller disconnects, `handle_supervisor_disconnect` calls `pause_session_fn()` (emitting `VoxEvent::PauseSession`) and transitions to `RemoteLinkState::Paired`. Transitioning to `Paired` fires `on_state_change` in `core/state.rs:249`, which calls `restart_engine_inner(&app, &st, false)`. `restart_engine_inner` calls `stop_audio_engine(state)`, which **resets `state.pipeline` to `InteractionState::Idle**` and, because `reengage_session` is `false`, leaves the pipeline in `Idle` instead of `Paused`!




* **The Fix:**
1. Call `pause_session_fn` and `mark_paired_device_seen` inside `handle_host_heartbeat_tick` (`manager_3.rs:1630`).


2. Do **not** tear down and restart the entire audio engine (`stop_audio_engine` / `start_audio_engine`) on every remote connect/disconnect in `core/state.rs:249`! Instead, keep `AudioStream` running and hot-swap or gate the input/output: feed `NetworkAudioCapture` directly into the existing capture ring buffer producer (`cap_prod`) when `RemoteLinkState == Connected`, and route `PlaybackEngine` to `remote_sink` when `RemoteLinkState == Connected`. That eliminates the 300ms engine restart stall, fixes the `remote_audio_ingestor` race (Bug 2 from last turn), and preserves `InteractionState::Paused` on disconnect!





---

#### 🔴 Bug 11 (`US-RC-04` & `VERIFY-RC-12`): `NetworkAudioCapture::ingest` Drops `Flush` and `PttMarker` Whenever `ingestion_gate == false`, and `PttMarker` Is a No-Op

* **Where:** `services/remote/audio/capture.rs:94-140`.


* **User Story Trace:**
1. While the Host is in `Speaking` or `Thinking` (or `Ready` in PTT mode), `self.ingestion_gate` is `false`.


2. Look at lines 95 and 116 of `capture.rs`:


```rust
pub fn ingest(&self, payload: AudioPayload) -> Result<(), ProtocolError> {
    if !self.active.load(Ordering::Relaxed) || !self.ingestion_gate.load(Ordering::Relaxed) {
        return Ok(());
    }
    match payload {
        AudioPayload::Frame(frame) => { ... }
        AudioPayload::Control(AudioControlMessage::Flush(flush)) => {
            self.active_epoch.fetch_max(flush.epoch, Ordering::AcqRel);
            Ok(())
        }
        AudioPayload::Control(AudioControlMessage::PttMarker(_)) => Ok(()),
    }
}

```


3. Because the `!self.ingestion_gate.load(...)` early return sits **above** `match payload`, any `Flush(epoch)` or `PttMarker(Start)` that arrives while `ingestion_gate` is closed is **silently ignored**! In PTT mode, `ingestion_gate` is closed *until* `PttStart` opens it—so gating `PttMarker` and `Flush` behind `ingestion_gate == true` makes PTT impossible and drops `Flush` epoch updates during TTS playback!


4. Furthermore, `AudioPayload::Control(AudioControlMessage::PttMarker(_)) => Ok(())` (line 136) and `|_ptt| {}` (line 109) are empty no-ops that never dispatch `VoxEvent::PttStart` / `PttStop` / `PttCancel`.




* **The Fix:** Check `self.active` and `self.ingestion_gate` **only inside the `AudioPayload::Frame(frame)` arm** (after checking `frame.epoch < min_epoch`), so `Flush` always advances `active_epoch` and `PttMarker` always dispatches to `event_tx` regardless of whether the mic ingestion gate is currently open.



---

#### 🟡 Bug 12 (`US-RC-09` / `US-RC-11`): `RemoteFailureKind::Revoked` Added to Rust `state.rs`, But Missing From TypeScript `remoteService.ts` and `QrScannerModal.tsx`

* **Where:** `state.rs:18` in `changes_2.diff` vs. `remoteService.ts:14-21` and `QrScannerModal.tsx:33-74`.


* **What Happens:** When the Host revokes the Controller and the Controller dials, `handle_dial_rejection` (`manager_3.rs:909`) emits `RemoteLinkState::Failed { reason: "Device pairing was revoked.", kind: RemoteFailureKind::Revoked }`, which serializes as `failureKind: "revoked"`.


* Update `RemoteFailureKind` in `remoteService.ts` to include `| "revoked"` (and `"host_busy"`), and handle `"revoked"` in `QrScannerModal.tsx::getErrorMessage`.



---

### 2. Test Suite Audit: 5 False Greens & Bypassed Production Seams

Your test files (`remote_security_test.rs`, `remote_link_test.rs`, `remote_connection_test.rs`) have strong unit coverage on low-level primitives (Noise handshake, token TTL/rate-limits, loopback framing), **but 5 integration tests are "false greens" that bypass the production code they claim to test**:

#### 🚨 False Green 1: `remote_connection_dial_and_connect_reaches_connected` Fakes the Host Instead of Running `listener.rs` (`remote_connection_test.rs:818-849, 928-959`

)

* What the work log claimed (`changes.diff` line 1225): *"Identified that `remote_connection_test.rs` previously faked the host listener inside the test body instead of testing the authentic production seam (`handle_inbound_connection`)... Added `remote_connection_dial_and_connect_reaches_connected`: Full controller `dial_and_connect` with real host listener."*


* What the test actually does (`remote_connection_test.rs:818-849`):
It spawns a custom `tokio::spawn` block that manually calls `listener.accept()`, manually runs `NoiseHandshake::responder`, manually reads `Hello`, manually sends `Hello`, and calls `hm.attach_link(...)`.


* **Why this is a False Green:** It **never calls a single function in `listener.rs**` (`handle_inbound_connection`, `dispatch_tier2_control_msg`, `check_host_admission`, `respond_hello_and_attach`, or `handle_inbound_audio_socket`)! Because the fake host never sends `SessionToken` or `AttachSnapshot`, the test completely misses whether the Controller's `dial_secondary_audio_socket` or `AttachSnapshot` handling works.


* **The Fix:** Extract `pub async fn handle_inbound_tier2_with_key(...)` or pass test dependencies into `listener.rs` so `remote_connection_test.rs` calls the real Host listener logic.

#### 🚨 False Green 2: `remote_connection_pairing_handshake_approved_reaches_connected` Fakes the Host & Runs Noise in the Wrong Order (`remote_connection_test.rs:135-190`

)

* Look at lines 135–183 of `remote_connection_test.rs`:


1. The test manually receives `PairRequest` in plaintext.
2. At line 149, the test calls `coordinator.begin(pairing_req, &pubkey_bytes, ...)` **before** running the Noise handshake (line 178), passing the unverified plaintext `claimed_req.public_key` as `handshake_static`!


3. At line 161, it inserts the peer into the DB *before* running the Noise handshake at line 178!


4. And its doc comment says `ASSERTS: Both Controller and Host reach RemoteLinkState::Connected`, while line 200 asserts `RemoteLinkState::Paired` and never instantiates a `host_manager`.




* **The Fix:** In the test's host task, run `NoiseHandshake::responder` immediately after receiving `PairRequest`, pass `*transport.remote_static()` to `coordinator.begin(...)`, and update the doc comment to `RemoteLinkState::Paired`.



#### 🚨 False Green 3: `WsLink` Never Runs the Link Conformance Suite (`VERIFY-RC-15`) — And Would Fail Case 17 & 18 Today (`remote_link_test.rs:480`

 & `ws_2.rs:334-345`)

* Spec Requirement (`RAIL-RC-05` & `VERIFY-RC-15`): *"Every `RemoteLink` implementation passes the conformance suite."*


* What the test does (`remote_link_test.rs:480`): Calls `run_link_conformance(LoopbackLink::pair).await`, and **never** calls `run_link_conformance` on a `WsLink` pair!


* **Proof It Caught a Real Bug:** Compare `LoopbackLink::recv_control` (`loopback_2.rs:112-123`) with `WsLink::decode_incoming` (`ws_2.rs:334-345`) and `WsLink::recv_audio` (`ws_2.rs:183-189`):


* `LoopbackLink` passes decode errors to `classify_protocol_error(&error)`: if `ProtocolResponse::Skippable` (such as `ProtocolError::MissingTypeTag` on a tagless JSON message), it logs a warning and **continues the receive loop** (which is tested by conformance cases `"skippable_violation_does_not_end_the_receive"` and `"skippable_audio_violation_does_not_end_the_receive"`).


* `WsLink` **never calls `classify_protocol_error(&err)**` in `decode_incoming` or `recv_audio`—it treats every `ProtocolError` (including `MissingTypeTag`) as fatal and closes the link!


* Calling `classify_protocol_error(&err)` in `WsLink::decode_incoming` and `WsLink::recv_audio` (matching `LoopbackLink`) fixes `WsLink` so it can pass `run_link_conformance`.





#### 🚨 False Green 4: `routing_table_contains_all_registered_ipc_commands` (`VERIFY-RC-07`) Checks a Copy-Pasted Array Instead of `supported_keys_for_domain` (`remote_security_test.rs:648-760`

)

* In `remote_security_test.rs:648-760`, the test hardcodes a 77-element string array copied from `routing.rs` and **does not check a single `domain.key` setting entry** (violating Spec §8.1: *"The test enumerates the command registry and the settings key table and fails if any entry is missing"*).


* **The Fix:** In `routing_table_contains_all_registered_ipc_commands`, iterate over every domain in `config::mutation` (`supported_keys_for_domain`) plus `"working_memory"`, `"personal_memory"`, and `"model_parameters"`, asserting that `classify_command("update_setting", Some(domain), Some(key)) != CommandClass::Denied`.



#### 🚨 False Green 5: Zero Integration Subtests for the New Tier 3 Seams

Add deterministic integration tests in `remote_connection_test.rs` covering:

1. **`VERIFY-RC-13` & `21` (Audio Socket Token & Key Binding):** Mint token via `host_manager.audio_tokens().mint(...)`, verify valid token + matching static key succeeds, replay of same token fails, and valid token + different static key fails.


2. **`VERIFY-RC-09` & `10` (Event Forwarder Coalescing):** Push 10 `telemetry` events in 10ms and 5 `llm_token` events (`"Hel"`, `"lo "`, `"wor"`, `"ld!"`) for `turn_id = 1` followed by `state_changed` into `forward_event`, and assert over `LoopbackLink` that only 1 `telemetry` event arrived and the 4 `llm_token` chunks were coalesced and flushed *before* `state_changed`.


3. **`VERIFY-RC-06` & `US-RC-06` (Reconnect Loop):** Verify that a transient dial failure during `Reconnecting` stays in `Reconnecting` and succeeds on attempt 2, whereas an `UnknownPeer` rejection immediately halts retries and enters `Failed { kind: Revoked }`.



---

### 3. Complete Summary Checklist Before You Ship Tier 3

If your agent resolves the **7 items from the previous turn** + the **5 user-story bugs + 5 test fixes from this turn**, every single User Story (`US-RC-01`–`11`) and Verification Criterion (`VERIFY-RC-01`–`22`) will be genuinely green:

| # | Category | File(s) | Fix Summary |
| --- | --- | --- | --- |
| **1** | **Downlink Audio** | `sink.rs`, `playback.rs` | Chunk 24kHz PCM16 into 480-sample (20ms) frames over an ordered MPSC queue; don't skip `PlaybackFinished` or `turn_metrics` in `PlaybackEngine`.

 |
| **2** | **Uplink Audio &PTT** | `capture.rs`, `listener.rs`, `state.rs` | Don't gate `Flush` / `PttMarker` behind `ingestion_gate == true`; wire `PttMarker` to `VoxEvent::Ptt*`; avoid restarting the whole engine on connect/disconnect so `Paused` state and `remote_audio_ingestor` aren't wiped.

 |
| **3** | **Controller Audio & RPC** | `lib.rs`, `manager.rs` | In `lib.rs::handle_invoke`, only enforce `is_desktop_command_permitted` when `role == SessionRole::Host`; when `role == Controller && state == Connected`, forward `Remote`/`Split` commands via `call_remote` and bridge phone mic/speaker to `audio_link`.

 |
| **4** | **Gateway Completeness** | `gateway.rs` | Call `classify_command` at the top of `RealCommandGateway::dispatch` to block `Local`/`Denied` commands (`appearance.*`, `remote.*`), and wire the remaining `Remote` IPC handlers.

 |
| **5** | **Controller Event & Snapshot** | `manager.rs`, `events.rs` | Emit incoming `ControlMessage::Event` and `AttachSnapshot` to the Controller's local frontend; do not forward `RemoteStateChanged` / `RemotePairing*` in `events.rs::event_to_value`.

 |
| **6** | **Reconnect & Heartbeat** | `manager.rs` | Don't overwrite `Reconnecting` with `Connecting`/`Failed` on transient retry failures inside `spawn_controller_reconnect`; call `pause_session_fn` and `mark_paired_device_seen` in `handle_host_heartbeat_tick`.

 |
| **7** | **Types, Lints & Tests** | `remoteService.ts`, `ws.rs`, `tests/*.rs` | Add `"revoked"` to TS `RemoteFailureKind`; remove the 3 `#[allow(...)]` in `manager.rs`; add `classify_protocol_error` to `WsLink`; and fix the 5 false-green test seams.

 |