# Remote Control Connection Flow — Execution Checklist

## Phase 1: Full-Stack Implementation

### Batch 1.1: Physical Transport (WsLink)
- [x] `app/src-tauri/src/services/remote/link/ws.rs` [NEW]
  - `pub struct WsLink`: Wraps `WebSocketStream<TcpStream>` for control channel.
  - `impl RemoteLink for WsLink`: Implements `send_control`, `recv_control`, `close`, `close_reason`. Stub `send_audio`/`recv_audio` for deferred audio slice.
  - `pub async fn bind(addr: SocketAddr) -> Result<WsListener, LinkError>`: Binds TCP and accepts incoming WebSocket upgrades.
  - `pub async fn dial(url: &str) -> Result<WsLink, LinkError>`: Dials `ws://` endpoint and returns upgraded `WsLink`.
- [x] `app/src-tauri/src/services/remote/link/mod.rs`
  - Re-export `WsLink`.
- Depends on: None.

### Batch 1.2: Session State & Manager
- [x] `app/src-tauri/src/services/remote/session/state.rs` [NEW]
  - `pub enum RemoteLinkState`: Canonical enum (`Disabled`, `Disconnected`, `Connecting`, `Connected`, `Reconnecting`, `Failed(String)`).
  - `pub struct RemoteStateDto`: DTO serializable to `{ link, failure, deviceId, displayName }`.
- [x] `app/src-tauri/src/services/remote/session/manager.rs` [NEW]
  - `pub struct RemoteSessionManager`: Holds `Arc<RwLock<RemoteLinkState>>`, active `RemoteLink`, and session role.
  - `pub async fn run_host_handshake(...)`: Executes Noise_XX handshake as responder.
  - `pub async fn run_controller_handshake(...)`: Executes Noise_XX handshake as initiator.
  - `pub async fn exchange_hello(...)`: Exchanges `Hello` and checks version and capabilities.
  - `pub fn start_heartbeat(...)`: Background task sending `Ping` every 2s, expecting `Pong` within 1s, timing out after 3 missed pongs.
  - `pub async fn disconnect(...)`: Sends `Disconnect` or `LinkLoss` and resets state to `Disconnected`.
- [x] `app/src-tauri/src/services/remote/session/mod.rs`
  - Re-export `RemoteLinkState`, `RemoteStateDto`, and `RemoteSessionManager`.
- Depends on: Batch 1.1.

### Batch 1.3: Static Command Routing & Guard
- [x] `app/src-tauri/src/services/remote/control/routing.rs` [NEW]
  - `pub enum CommandClass`: `Remote`, `Local`, `Split`, `Denied`.
  - `pub fn get_routing_table() -> &'static HashMap<&'static str, CommandClass>`: Maps all IPC commands and settings domains/keys.
- [x] `app/src-tauri/src/services/remote/control/guard.rs` [NEW]
  - `pub fn is_desktop_command_permitted(command: &str, domain: Option<&str>, state: &RemoteLinkState) -> bool`: Enforces `RULE-RC-GUARD`.
- [x] `app/src-tauri/src/services/remote/control/mod.rs`
  - Re-export routing and guard functions.
- Depends on: Batch 1.2.

### Batch 1.4: Pairing Coordinator & QR Minting
- [x] `app/src-tauri/src/services/remote/security/pairing.rs`
  - `pub fn mint_pairing_qr_svg(host_id: &DeviceId, public_key: &[u8], hint_addr: &str, hint_port: u16, token: &PairingToken) -> Result<(String, String), SecurityError>`: Generates payload string and SVG string via `qrcode` crate.
  - `pub fn get_local_lan_ip() -> Option<String>`: Discovers primary outgoing IPv4 address.
- [x] `app/src-tauri/src/services/remote/security/mod.rs`
  - Re-export QR minting and IP discovery.
- Depends on: Batch 1.2.

### Batch 1.5: Tauri IPC Handlers & Event Bridge
- [x] `app/src-tauri/src/ipc/remote.rs` [NEW]
  - `pair_new_device`: Host mints token, returns payload, QR data URL/SVG, and expiration.
  - `approve_pairing`: Host answers inbound pairing prompt with `"Approve"` or `"Reject"`.
  - `list_paired_devices`: Host returns list from Turso `paired_devices`.
  - `revoke_device`: Host deletes peer row and closes open link.
  - `disconnect_device`: Host closes open link with `host_disconnected` reason, keeping pairing.
  - `connect_host`: Controller dials host by deviceId or address.
  - `disconnect_host`: Controller sends intentional `Disconnect`.
  - `get_link_state`: Controller reads current state for hydration.
  - `get_command_routing`: Returns static routing table.
- [x] `app/src-tauri/src/ipc/mod.rs`
  - Export `pub mod remote;`.
- [x] `app/src-tauri/src/lib.rs`
  - Register all 9 commands in `generate_handler!`.
  - Emit `remote_state_changed` on transitions and `remote_pairing_requested` on inbound pairing attempts.
- Depends on: Batches 1.1, 1.2, 1.3, 1.4.

### Batch 1.6: Frontend Services, Store & Remote Guard
- [x] `app/src/services/remoteService.ts` [NEW]
  - Invoke wrappers for all 9 IPC commands.
- [x] `app/src/store/remoteStore.ts` [NEW]
  - Zustand store managing `linkState`, `connectedPeer`, `pendingPairingPrompt`, `pairedDevices`.
  - Listens to `remote_state_changed` and `remote_pairing_requested`.
- [x] `app/src/shared/hooks/useRemoteGuard.ts` [NEW]
  - Hook returning `{ isBlocked, guardProps, tooltip }`.
- [x] `app/src/shared/components/home/HomeControls.tsx`
  - Integrate `useRemoteGuard` into Engage button.
- Depends on: Batch 1.5.

### Batch 1.7: Host UI (Settings & Modals)
- [x] `app/src/shared/components/settings/interaction/RemoteDeskPanel.tsx` [NEW]
  - Toggle `remote.accepts_controllers`, `[+ Pair New Device]` button, and paired controller card.
- [x] `app/src/shared/components/remote/PairingModal.tsx` [NEW]
  - Displays SVG QR, 120s countdown timer, IP hint, auto-refresh.
- [x] `app/src/shared/components/remote/PairingPromptModal.tsx` [NEW]
  - Inbound approval modal ("Approve [Device Name] to connect?"), 30s timeout, Approve/Deny.
- [x] `app/src/shared/components/settings/interaction/InteractionCard.tsx`
  - Mount `RemoteDeskPanel` on desktop when `activeDeskTab === "remote"`.
- [x] `app/src/layout/AppLayout.tsx`
  - Mount `PairingPromptModal` globally.
- Depends on: Batch 1.6.

### Batch 1.8: Controller UI (Settings & Scanner)
- [x] `app/src/shared/components/remote/QrScannerModal.tsx` [NEW]
  - In-app camera viewfinder via WebRTC + `jsQR` canvas decoder at 20fps. Reticle overlay.
  - Graceful fallback to manual IP/port entry on camera denial. Cleanup tracks on unmount.
- [x] `app/src/shared/components/settings/interaction/ControllerRemoteDeskPanel.tsx` [NEW]
  - Controller Remote Desk view: "No Host Connected", **[Scan Host QR]** button, Paired Host card.
- [x] `app/src/shared/components/settings/interaction/InteractionCard.tsx`
  - Render `ControllerRemoteDeskPanel` on mobile viewports.
- Depends on: Batch 1.6.

### Batch 1.9: Rocket Button & Ambient Visuals
- [x] `app/src/shared/components/remote/RocketButton.tsx` [NEW]
  - Controller: always visible; dials or disconnects.
  - Host: visible only when `Connected`; two-step inline disconnect matching `RestoreDefaultsButton.tsx`.
- [x] `app/src/layout/AppHeader.tsx`
  - Mount `RocketButton.tsx` next to ghost/incognito toggle.
- [x] `app/src/shared/components/orb/` (or orb wrapper)
  - Inward ripples and compact `● Remote` status badge on Host.
- [x] `app/src/layout/AppLayout.tsx`
  - Viewport inset accent rim glow on Controller when `Connected`.
- Depends on: Batches 1.7, 1.8.

---

## Phase 2: Test Engineering

### Batch 2.1: Rust Backend Integration Tests
- [x] `app/src-tauri/tests/remote_link_test.rs`
  - `ws_link_control_roundtrip_delivers_payload`
  - `ws_link_oversized_message_closes_with_protocol_error`
  - `ws_link_clean_close_notifies_peer`
- [x] `app/src-tauri/tests/remote_connection_test.rs` [NEW]
  - `remote_connection_loopback_e2e_full_flow`
  - `remote_connection_heartbeat_timeout_transitions_to_disconnected`
  - `remote_connection_rule_rc_guard_blocks_desktop_mutating_writes`
  - `remote_connection_host_disconnect_keeps_pairing`
  - `remote_connection_revocation_permanently_removes_device`
- Depends on: Phase 1.

### Batch 2.2: Frontend Unit & DOM Tests
- [x] `app/src/test/unit/remoteStore.test.ts` [NEW]
- [x] `app/src/test/unit/useRemoteGuard.test.ts` [NEW]
- [x] `app/src/test/dom/PairingModal.dom.test.tsx` [NEW]
- [x] `app/src/test/dom/RocketButton.dom.test.tsx` [NEW]
- Depends on: Phase 1.

---

## Phase 3: Code & Spec Review

- [x] Audit against `.agents/rules/backend-style-guide.md` (no boolean shadows, file grammar, error propagation).
- [x] Audit against `.agents/rules/frontend-style-guide.md` (React 19 hook order, memory leak guards, theme tokens).
- [x] Audit against `.agents/rules/testing-style-guide.md` (mandatory intent blocks, sequential release execution).
- [x] Run `./scripts/verify-targets.sh` (0 warnings / 0 errors).

---

## Phase 4: Lead Agent UI/UX Scenario Walkthrough

- [x] Trace US-RC-01 (First-time pairing).
- [x] Trace US-RC-05 & 06 (Disconnect & reconnection).
- [x] Trace US-RC-07 & 08 (Host passive observer state & control guard).
- [x] Trace US-RC-09 (Revocation).
- [x] Physical Android verification via USB adb.
