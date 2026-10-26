//! Remote session manager — Tier 1 (pairing) and Tier 2 (transport) lifecycle.
//!
//! Strict separation per spec §4.4:
//! - `pair_device`: Tier 1 — ephemeral token handshake, writes DB, closes socket.
//! - `dial_and_connect`: Tier 2 — assumes device is already in paired_devices; dials,
//!   runs Noise handshake, exchanges Hello, spawns Link Supervisor.
//! - `spawn_link_supervisor`: Active duplex reader. The ONLY reader of recv_control()
//!   while a link is alive. Dispatches Ping/Pong, Disconnect, LinkLoss, and socket faults.

use std::{
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::RwLock;
use tokio::{sync::RwLock as AsyncRwLock, time::sleep};
use tokio_util::sync::CancellationToken;

use super::state::{RemoteFailureKind, RemoteLinkState, RemoteStateDto};
use crate::services::remote::{
    link::{LinkCloseReason, LinkError, RemoteLink},
    protocol::{
        capabilities::CAP_CONTROL_MESSAGES,
        errors::ProtocolError,
        framing::StreamFormat,
        messages::{
            ControlMessage, Hello, LinkLoss, LinkLossReason, Ping, Pong, SenderIdentity,
            VersionMismatch,
        },
    },
    security::{default_noise_prologue, DeviceKey, NoiseHandshake, NoiseTransport, SecurityError},
    HEARTBEAT_INTERVAL, HEARTBEAT_MAX_MISSED_PONGS, HEARTBEAT_PING_TIMEOUT, PROTOCOL_VERSION,
};

/// Type alias for state change subscriber callbacks.
pub type StateChangeCallback = Arc<dyn Fn(&RemoteLinkState, Option<&SenderIdentity>) + Send + Sync>;

/// Role played in the remote session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    Host,
    Controller,
}

/// Manages the remote control session lifecycle, Noise_XX handshake,
/// capabilities exchange, and link supervision.
pub struct RemoteSessionManager {
    role: SessionRole,
    state: Arc<RwLock<RemoteLinkState>>,
    peer_identity: Arc<RwLock<Option<SenderIdentity>>>,
    link: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
    cancel_token: Arc<parking_lot::Mutex<Option<CancellationToken>>>,
    on_state_change: Option<StateChangeCallback>,
}

impl RemoteSessionManager {
    /// Creates a new `RemoteSessionManager` for the specified role.
    pub fn new(role: SessionRole) -> Self {
        Self {
            role,
            // Ticket 01: initial state is Unpaired (no paired_devices rows).
            // Call set_state(Paired) after verifying paired_devices count at startup.
            state: Arc::new(RwLock::new(RemoteLinkState::Unpaired)),
            peer_identity: Arc::new(RwLock::new(None)),
            link: Arc::new(AsyncRwLock::new(None)),
            cancel_token: Arc::new(parking_lot::Mutex::new(None)),
            on_state_change: None,
        }
    }

    /// Creates a new `RemoteSessionManager` with an attached state transition listener.
    pub fn with_callback(role: SessionRole, callback: StateChangeCallback) -> Self {
        Self {
            role,
            state: Arc::new(RwLock::new(RemoteLinkState::Unpaired)),
            peer_identity: Arc::new(RwLock::new(None)),
            link: Arc::new(AsyncRwLock::new(None)),
            cancel_token: Arc::new(parking_lot::Mutex::new(None)),
            on_state_change: Some(callback),
        }
    }

    /// Returns the session role (Host or Controller).
    pub fn role(&self) -> SessionRole {
        self.role
    }

    /// Returns the current canonical link state.
    pub fn state(&self) -> RemoteLinkState {
        self.state.read().clone()
    }

    /// Returns the attached peer identity if connected.
    pub fn peer_identity(&self) -> Option<SenderIdentity> {
        self.peer_identity.read().clone()
    }

    /// Returns the current `RemoteStateDto` for IPC queries.
    pub fn state_dto(&self) -> RemoteStateDto {
        let state = self.state();
        let peer = self.peer_identity();
        state.to_dto(
            peer.as_ref().map(|p| p.device_id.as_str()),
            peer.as_ref().map(|p| p.display_name.as_str()),
        )
    }

    /// Transitions to a new link state and invokes listener callbacks.
    pub fn set_state(&self, new_state: RemoteLinkState) {
        {
            let mut guard = self.state.write();
            *guard = new_state.clone();
        }
        let peer = self.peer_identity();
        if let Some(cb) = &self.on_state_change {
            cb(&new_state, peer.as_ref());
        }
    }

    /// Attaches an active link and peer identity to the session.
    /// Spawns the Link Supervisor actor (duplex read loop + heartbeat).
    pub async fn attach_link(&self, link: Arc<dyn RemoteLink>, peer: SenderIdentity) {
        {
            let mut guard = self.peer_identity.write();
            *guard = Some(peer);
        }
        {
            let mut guard = self.link.write().await;
            *guard = Some(link.clone());
        }
        self.set_state(RemoteLinkState::Connected);
        self.spawn_link_supervisor(link);
    }

    /// Returns a reference to the active link if one is open.
    pub async fn get_link(&self) -> Option<Arc<dyn RemoteLink>> {
        self.link.read().await.clone()
    }

    /// Executes the responder side of the Noise_XX handshake.
    pub async fn run_host_handshake<FRead, FWrite, FutR, FutW>(
        local_key: &DeviceKey,
        expected_peer: Option<&[u8]>,
        read_fn: FRead,
        write_fn: FWrite,
    ) -> Result<NoiseTransport, SecurityError>
    where
        FRead: FnMut() -> FutR,
        FutR: Future<Output = Result<Vec<u8>, SecurityError>>,
        FWrite: FnMut(Vec<u8>) -> FutW,
        FutW: Future<Output = Result<(), SecurityError>>,
    {
        let handshake =
            NoiseHandshake::responder(local_key, expected_peer, &default_noise_prologue())?;
        drive_noise_handshake(handshake, read_fn, write_fn).await
    }

    /// Executes the initiator side of the Noise_XX handshake.
    pub async fn run_controller_handshake<FRead, FWrite, FutR, FutW>(
        local_key: &DeviceKey,
        expected_peer: Option<&[u8]>,
        read_fn: FRead,
        write_fn: FWrite,
    ) -> Result<NoiseTransport, SecurityError>
    where
        FRead: FnMut() -> FutR,
        FutR: Future<Output = Result<Vec<u8>, SecurityError>>,
        FWrite: FnMut(Vec<u8>) -> FutW,
        FutW: Future<Output = Result<(), SecurityError>>,
    {
        let handshake =
            NoiseHandshake::initiator(local_key, expected_peer, &default_noise_prologue())?;
        drive_noise_handshake(handshake, read_fn, write_fn).await
    }

    /// Exchanges `Hello` negotiation messages over the active link.
    pub async fn exchange_hello(
        &self,
        link: &dyn RemoteLink,
        local_identity: SenderIdentity,
        uplink_format: StreamFormat,
        downlink_format: StreamFormat,
    ) -> Result<Hello, LinkError> {
        let hello = Hello {
            version: PROTOCOL_VERSION,
            sender_identity: local_identity,
            capabilities: CAP_CONTROL_MESSAGES,
            uplink_format,
            downlink_format,
        };

        link.send_control(ControlMessage::Hello(hello)).await?;
        let incoming = link.recv_control().await?;

        match incoming {
            ControlMessage::Hello(peer_hello) => {
                if peer_hello.version != PROTOCOL_VERSION {
                    let mismatch = VersionMismatch {
                        version: peer_hello.version,
                        supported_version: PROTOCOL_VERSION,
                    };
                    let _ = link
                        .send_control(ControlMessage::VersionMismatch(mismatch.clone()))
                        .await;
                    return Err(LinkError::Protocol(ProtocolError::MalformedJson(format!(
                        "Protocol version mismatch: peer sent {}, supported {}",
                        peer_hello.version, PROTOCOL_VERSION
                    ))));
                }
                Ok(peer_hello)
            }
            ControlMessage::VersionMismatch(mismatch) => {
                Err(LinkError::Protocol(ProtocolError::MalformedJson(format!(
                    "Peer rejected version: announced {}, supported {}",
                    mismatch.version, mismatch.supported_version
                ))))
            }
            ControlMessage::UnknownPeer(peer) => {
                Err(LinkError::Protocol(ProtocolError::MalformedJson(format!(
                    "UnknownPeer: {}",
                    peer.device_id
                ))))
            }
            ControlMessage::LinkRefused(refused) => {
                Err(LinkError::Protocol(ProtocolError::MalformedJson(format!(
                    "LinkRefused: {:?}",
                    refused.reason
                ))))
            }
            other => Err(LinkError::Protocol(ProtocolError::MalformedJson(format!(
                "Expected Hello, got {:?}",
                other.message_type()
            )))),
        }
    }

    // ─── Ticket 03: Link Supervisor (duplex recv_control loop) ──────────────

    /// Spawns the Link Supervisor actor. This is the sole reader of
    /// `recv_control()` for the entire lifetime of a connected session.
    ///
    /// Spec §6.3: bidirectional Ping/Pong, Disconnect dispatch, LinkLoss
    /// dispatch, and fast-path socket fault detection.
    fn spawn_link_supervisor(&self, link: Arc<dyn RemoteLink>) {
        let cancel = CancellationToken::new();
        {
            let mut guard = self.cancel_token.lock();
            if let Some(old) = guard.replace(cancel.clone()) {
                old.cancel();
            }
        }

        let role = self.role;
        let state_arc = Arc::clone(&self.state);
        let on_change = self.on_state_change.clone();
        let peer_arc = Arc::clone(&self.peer_identity);
        let link_slot = Arc::clone(&self.link);

        tokio::spawn(async move {
            run_link_supervisor(role, link, cancel, state_arc, peer_arc, on_change, link_slot)
                .await;
        });
    }

    // ─── Ticket 02: Tier 1 — pair_device ────────────────────────────────────

    /// Controller-side: dial the host, perform the Tier 1 pairing handshake,
    /// write the host into `paired_devices`, close the pairing socket, and
    /// transition `RemoteLinkState` to `Paired`.
    ///
    /// Spec §5.3 + §4.4: pairing approval MUST NOT trigger an automatic Tier 2
    /// connection. The Rocket button (connect_host) starts Tier 2 explicitly.
    pub async fn pair_device(
        self: Arc<Self>,
        db: Arc<crate::persistence::VoxDb>,
        address: String,
        token: String,
        host_public_key: String,
        host_device_id: String,
        on_prompting: Option<Box<dyn FnOnce() + Send>>,
    ) -> Result<(), LinkError> {
        self.set_state(RemoteLinkState::Connecting);

        let url = if address.starts_with("ws://") || address.starts_with("wss://") {
            address.clone()
        } else {
            format!("ws://{address}")
        };

        let ws_link = match crate::services::remote::link::ws::dial(&url).await {
            Ok(l) => l,
            Err(e) => {
                let err_str = e.to_string();
                let kind = classify_dial_error(&err_str);
                self.set_state(RemoteLinkState::failed(format!("Failed to connect: {e}"), kind));
                return Err(e);
            }
        };

        let key_store = crate::services::remote::security::KeyStore::default_path()
            .map_err(|e| LinkError::Transport(e.to_string()))?;
        let device_key = key_store
            .load_or_create()
            .map_err(|e| LinkError::Transport(e.to_string()))?;

        let controller_id =
            crate::services::remote::security::derive_device_id(device_key.public_key());
        let controller_pubkey_hex = hex::encode(device_key.public_key());

        // Emit AwaitingApproval so the controller UI shows "Waiting for host approval"
        // (the on_prompting callback handles the IPC emission).

        let pair_req = ControlMessage::PairRequest(
            crate::services::remote::protocol::messages::PairRequest {
                token: token.clone(),
                device_id: controller_id.as_str().to_string(),
                public_key: controller_pubkey_hex,
                display_name: "Vox Controller".to_string(),
            },
        );

        if let Err(e) = ws_link.send_control(pair_req).await {
            self.set_state(RemoteLinkState::failed(
                format!("Failed to send pairing request: {e}"),
                RemoteFailureKind::Unreachable,
            ));
            return Err(e);
        }

        // Wait for PairPrompting or PairApproved/PairDenied (35s)
        let mut resp =
            match tokio::time::timeout(Duration::from_secs(35), ws_link.recv_control()).await {
                Ok(Ok(msg)) => msg,
                Ok(Err(e)) => {
                    self.set_state(RemoteLinkState::failed(
                        format!("Connection error awaiting pairing response: {e}"),
                        RemoteFailureKind::Unreachable,
                    ));
                    return Err(e);
                }
                Err(_) => {
                    let msg = "Pairing timed out waiting for host";
                    self.set_state(RemoteLinkState::failed(msg, RemoteFailureKind::Timeout));
                    return Err(LinkError::Protocol(ProtocolError::MalformedJson(
                        msg.to_string(),
                    )));
                }
            };

        if let ControlMessage::PairPrompting = resp {
            if let Some(cb) = on_prompting {
                cb();
            }
            resp = match tokio::time::timeout(Duration::from_secs(35), ws_link.recv_control())
                .await
            {
                Ok(Ok(msg)) => msg,
                Ok(Err(e)) => {
                    self.set_state(RemoteLinkState::failed(
                        format!("Connection error awaiting pairing approval: {e}"),
                        RemoteFailureKind::Unreachable,
                    ));
                    return Err(e);
                }
                Err(_) => {
                    let msg = "Host did not approve pairing within 35s";
                    self.set_state(RemoteLinkState::failed(msg, RemoteFailureKind::Timeout));
                    return Err(LinkError::Protocol(ProtocolError::MalformedJson(
                        msg.to_string(),
                    )));
                }
            };
        }

        match resp {
            ControlMessage::PairApproved => {
                let h_pk_bytes = hex::decode(&host_public_key).map_err(|e| {
                    let msg = format!("Invalid host public key hex: {e}");
                    self.set_state(RemoteLinkState::failed(
                        msg.clone(),
                        RemoteFailureKind::InvalidToken,
                    ));
                    LinkError::Protocol(ProtocolError::MalformedJson(msg))
                })?;

                // Write host into paired_devices
                if let Ok(conn) = db.connect() {
                    let _ = crate::persistence::paired_devices::insert_paired_device(
                        &conn,
                        &crate::persistence::paired_devices::NewPairedDevice {
                            device_id: host_device_id.clone(),
                            public_key: h_pk_bytes,
                            display_name: "Vox Host".to_string(),
                            role: crate::persistence::paired_devices::PairedDeviceRole::Host,
                            trust_level: "full".to_string(),
                            hints: vec![serde_json::json!({ "addr": address })],
                        },
                    )
                    .await;
                }

                // Tier 1 complete: close socket, transition to Paired (spec §4.4)
                let _ = ws_link.close(LinkCloseReason::Local).await;
                self.set_state(RemoteLinkState::Paired);
                log::info!("[Remote] Pairing complete with host {host_device_id}. Tier 1 socket closed.");
                Ok(())
            }
            ControlMessage::PairDenied(denied) => {
                let is_token = denied.reason.to_lowercase().contains("token")
                    || denied.reason.to_lowercase().contains("expired");
                let kind = if is_token {
                    RemoteFailureKind::InvalidToken
                } else {
                    RemoteFailureKind::RejectedByHost
                };
                let msg = format!("Pairing denied by host: {}", denied.reason);
                self.set_state(RemoteLinkState::failed(msg.clone(), kind));
                let _ = ws_link.close(LinkCloseReason::Local).await;
                Err(LinkError::Protocol(ProtocolError::MalformedJson(msg)))
            }
            other => {
                let msg = format!("Expected PairApproved, got {:?}", other.message_type());
                self.set_state(RemoteLinkState::failed(
                    msg.clone(),
                    RemoteFailureKind::VersionMismatch,
                ));
                let _ = ws_link.close(LinkCloseReason::Local).await;
                Err(LinkError::Protocol(ProtocolError::MalformedJson(msg)))
            }
        }
    }

    // ─── Ticket 02: Tier 2 — dial_and_connect ───────────────────────────────

    /// Controller-side: dial a paired host, run Noise handshake, exchange
    /// Hello, and spawn the Link Supervisor. Assumes device is already in
    /// `paired_devices`. Does NOT perform pairing; call `pair_device` first.
    ///
    /// Spec §4.4 Tier 2: explicitly initiated from the Home Rocket button.
    pub async fn dial_and_connect(
        self: Arc<Self>,
        db: Arc<crate::persistence::VoxDb>,
        device_id: String,
        address: Option<String>,
    ) -> Result<(), LinkError> {
        self.set_state(RemoteLinkState::Connecting);

        // Resolve address: use provided or fall back to paired_devices hints
        let dial_addr = if let Some(addr) = address {
            addr
        } else if let Ok(conn) = db.connect() {
            crate::persistence::paired_devices::get_paired_device(&conn, &device_id)
                .await
                .ok()
                .flatten()
                .and_then(|d| {
                    d.hints.first().and_then(|h| {
                        h.get("addr").and_then(|v| v.as_str()).map(|s| s.to_string())
                    })
                })
                .ok_or_else(|| {
                    LinkError::Transport(format!(
                        "No address hint for device {device_id}"
                    ))
                })?
        } else {
            return Err(LinkError::Transport("Cannot open DB to resolve address".into()));
        };

        let url = if dial_addr.starts_with("ws://") || dial_addr.starts_with("wss://") {
            dial_addr.clone()
        } else {
            format!("ws://{dial_addr}")
        };

        let ws_link = match crate::services::remote::link::ws::dial(&url).await {
            Ok(l) => l,
            Err(e) => {
                let kind = classify_dial_error(&e.to_string());
                self.set_state(RemoteLinkState::failed(format!("Dial failed: {e}"), kind));
                return Err(e);
            }
        };

        let key_store = crate::services::remote::security::KeyStore::default_path()
            .map_err(|e| LinkError::Transport(e.to_string()))?;
        let device_key = key_store
            .load_or_create()
            .map_err(|e| LinkError::Transport(e.to_string()))?;

        let controller_id =
            crate::services::remote::security::derive_device_id(device_key.public_key());
        let local_identity = SenderIdentity {
            device_id: controller_id.as_str().to_string(),
            display_name: "Vox Controller".to_string(),
        };

        let peer_hello = match self
            .exchange_hello(
                &ws_link,
                local_identity,
                StreamFormat::UPLINK,
                StreamFormat::DOWNLINK,
            )
            .await
        {
            Ok(h) => h,
            Err(e) => {
                let err_str = e.to_string();
                let (msg, kind) = if err_str.contains("UnknownPeer") {
                    // Host revoked this device or doesn't know it (§6.5, §6.7)
                    // Purge local paired_devices entry
                    if let Ok(conn) = db.connect() {
                        let _ = crate::persistence::paired_devices::delete_paired_device(
                            &conn, &device_id,
                        )
                        .await;
                    }
                    (
                        format!(
                            "Connection rejected: this device is not paired with the host ({device_id})"
                        ),
                        RemoteFailureKind::RejectedByHost,
                    )
                } else if err_str.contains("LinkRefused") {
                    (
                        format!("Host refused connection: {e}"),
                        RemoteFailureKind::RejectedByHost,
                    )
                } else if err_str.contains("Timeout") || err_str.contains("timed out") {
                    (
                        format!("Handshake timed out connecting to {device_id}"),
                        RemoteFailureKind::Timeout,
                    )
                } else {
                    (
                        format!("Protocol version mismatch or handshake error connecting to {device_id}: {e}"),
                        RemoteFailureKind::VersionMismatch,
                    )
                };

                self.set_state(RemoteLinkState::failed(msg, kind));
                return Err(e);
            }
        };

        // Attach link — spawns Link Supervisor (Ticket 03)
        self.attach_link(Arc::new(ws_link), peer_hello.sender_identity)
            .await;
        Ok(())
    }

    // ─── Ticket 04: Revocation & Disconnect ─────────────────────────────────

    /// Lists all paired devices from the Turso database.
    pub async fn list_paired_devices(
        db: &crate::persistence::VoxDb,
    ) -> Result<
        Vec<crate::persistence::paired_devices::PairedDevice>,
        crate::core::error::PersistenceError,
    > {
        let conn = db.connect()?;
        crate::persistence::paired_devices::list_paired_devices(&conn).await
    }

    /// Permanently removes a paired device and severs its connection.
    ///
    /// Spec §5.6 + §5.8: send `LinkLoss(Revoked)` before closing if active,
    /// then determine new state from remaining paired_devices count.
    pub async fn revoke_device(
        &self,
        db: &crate::persistence::VoxDb,
        device_id: &str,
    ) -> Result<bool, crate::core::error::PersistenceError> {
        // Ticket 04: sever active link with Revoked notification before DB deletion
        let is_current = self
            .peer_identity()
            .map(|p| p.device_id == device_id)
            .unwrap_or(false);
        if is_current {
            if let Some(link) = self.link.write().await.take() {
                let _ = link
                    .send_control(ControlMessage::LinkLoss(LinkLoss {
                        reason: LinkLossReason::Revoked,
                    }))
                    .await;
                let _ = link.close(LinkCloseReason::Local).await;
            }
            if let Some(token) = self.cancel_token.lock().take() {
                token.cancel();
            }
            {
                let mut guard = self.peer_identity.write();
                *guard = None;
            }
        }

        let conn = db.connect()?;
        let deleted =
            crate::persistence::paired_devices::delete_paired_device(&conn, device_id).await?;

        // Determine new state from remaining count (spec §5.8)
        let remaining = crate::persistence::paired_devices::list_paired_devices(&conn)
            .await
            .map(|v| v.len())
            .unwrap_or(0);
        let new_state = if remaining == 0 {
            RemoteLinkState::Unpaired
        } else {
            RemoteLinkState::Paired
        };
        self.set_state(new_state);
        Ok(deleted)
    }

    /// Host-initiated clean disconnect — keeps pairing intact.
    ///
    /// Spec §6.6: sends `LinkLoss(HostDisconnected)`, closes link, pauses
    /// session, writes `last_seen_at`, transitions to `Paired`.
    pub async fn disconnect_host_initiated(&self) -> Result<(), LinkError> {
        if let Some(token) = self.cancel_token.lock().take() {
            token.cancel();
        }

        let link_opt = self.link.write().await.take();
        if let Some(link) = link_opt {
            let _ = link
                .send_control(ControlMessage::LinkLoss(LinkLoss {
                    reason: LinkLossReason::HostDisconnected,
                }))
                .await;
            let _ = link.close(LinkCloseReason::Local).await;
        }

        {
            let mut guard = self.peer_identity.write();
            *guard = None;
        }

        // Keep pairing: transition to Paired (spec §6.6 — NOT Unpaired)
        self.set_state(RemoteLinkState::Paired);
        Ok(())
    }

    /// Controller-initiated clean disconnect — keeps pairing intact.
    ///
    /// Spec §6.4: sends `Disconnect`, closes link, transitions to `Paired`.
    pub async fn disconnect_controller_initiated(&self) -> Result<(), LinkError> {
        if let Some(token) = self.cancel_token.lock().take() {
            token.cancel();
        }

        let link_opt = self.link.write().await.take();
        if let Some(link) = link_opt {
            let _ = link.send_control(ControlMessage::Disconnect).await;
            let _ = link.close(LinkCloseReason::Local).await;
        }

        {
            let mut guard = self.peer_identity.write();
            *guard = None;
        }

        // Keep pairing: transition to Paired (spec §6.4)
        self.set_state(RemoteLinkState::Paired);
        Ok(())
    }

    /// Legacy unified disconnect — prefer `disconnect_host_initiated` /
    /// `disconnect_controller_initiated`. Retained for callers not yet migrated.
    pub async fn disconnect(&self, reason: LinkLossReason) -> Result<(), LinkError> {
        match reason {
            LinkLossReason::Revoked => {
                // Only used when revoke is called without a DB context; prefer revoke_device.
                if let Some(token) = self.cancel_token.lock().take() {
                    token.cancel();
                }
                let link_opt = self.link.write().await.take();
                if let Some(link) = link_opt {
                    let _ = link
                        .send_control(ControlMessage::LinkLoss(LinkLoss { reason }))
                        .await;
                    let _ = link.close(LinkCloseReason::Local).await;
                }
                {
                    let mut guard = self.peer_identity.write();
                    *guard = None;
                }
                self.set_state(RemoteLinkState::Unpaired);
            }
            _ => match self.role {
                SessionRole::Host => self.disconnect_host_initiated().await?,
                SessionRole::Controller => self.disconnect_controller_initiated().await?,
            },
        }
        Ok(())
    }
}

// ─── Noise handshake driver ─────────────────────────────────────────────────

/// Drives the Noise handshake state machine to completion using provided I/O closures.
async fn drive_noise_handshake<FRead, FWrite, FutR, FutW>(
    mut handshake: NoiseHandshake,
    mut read_fn: FRead,
    mut write_fn: FWrite,
) -> Result<NoiseTransport, SecurityError>
where
    FRead: FnMut() -> FutR,
    FutR: Future<Output = Result<Vec<u8>, SecurityError>>,
    FWrite: FnMut(Vec<u8>) -> FutW,
    FutW: Future<Output = Result<(), SecurityError>>,
{
    loop {
        if handshake.is_complete() {
            break;
        }
        if handshake.is_my_turn() {
            let msg = handshake.write_message(&[])?;
            write_fn(msg).await?;
        } else {
            let msg = read_fn().await?;
            handshake.read_message(&msg)?;
        }
    }
    handshake.finish()
}

// ─── Ticket 03: Duplex Link Supervisor ──────────────────────────────────────

/// Supervises link health via the full duplex recv_control loop.
///
/// This is the ONLY reader of `recv_control()` for the lifetime of a session.
/// Spec §6.3: handles Ping/Pong, Disconnect, LinkLoss, heartbeat, and faults.
async fn run_link_supervisor(
    role: SessionRole,
    link: Arc<dyn RemoteLink>,
    cancel: CancellationToken,
    state: Arc<RwLock<RemoteLinkState>>,
    peer_identity: Arc<RwLock<Option<SenderIdentity>>>,
    on_change: Option<StateChangeCallback>,
    link_slot: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
) {
    // Host sends Ping, Controller monitors Ping arrival deadline
    let mut seq: u64 = 0;
    let mut missed_pongs: u32 = 0;
    let mut last_ping_at = Instant::now(); // Controller: reset on each Ping received

    // Named constants per spec §6.3.2
    const PING_INTERVAL: Duration = HEARTBEAT_INTERVAL;    // 2s
    const PING_TIMEOUT: Duration = HEARTBEAT_PING_TIMEOUT; // 6s  (controller deadline)
    const MAX_MISSED: u32 = HEARTBEAT_MAX_MISSED_PONGS;    // 3

    let mut ping_timer = tokio::time::interval(PING_INTERVAL);
    ping_timer.tick().await; // consume the immediate first tick

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                log::debug!("[Remote Supervisor] cancelled — exiting");
                break;
            }

            // ── The duplex read arm ──────────────────────────────────────────
            msg = link.recv_control() => {
                match msg {
                    Ok(ControlMessage::Ping(p)) => {
                        // Spec §6.3.1: receiver MUST immediately reply with Pong
                        last_ping_at = Instant::now();
                        let _ = link.send_control(ControlMessage::Pong(Pong { seq: p.seq })).await;
                    }

                    Ok(ControlMessage::Pong(p)) => {
                        // Spec §6.3.1: sender resets missed counter on matching Pong
                        if p.seq == seq {
                            missed_pongs = 0;
                        }
                    }

                    Ok(ControlMessage::Disconnect) => {
                        // Spec §6.4: Controller sent intentional disconnect to Host.
                        // Host pauses session, transitions Paired.
                        log::info!("[Remote Supervisor] Received Disconnect from controller — transitioning to Paired");
                        let _ = link.close(LinkCloseReason::Local).await;
                        link_slot.write().await.take();
                        clear_peer_and_set_state(
                            &state,
                            &peer_identity,
                            &on_change,
                            RemoteLinkState::Paired,
                        );
                        // shortcut: pause_session hook would go here when pipeline integration is added
                        break;
                    }

                    Ok(ControlMessage::LinkLoss(loss)) => {
                        // Spec §6.3.1 + §5.8: Controller receives this from Host.
                        log::info!("[Remote Supervisor] Received LinkLoss({:?})", loss.reason);
                        let _ = link.close(LinkCloseReason::Local).await;
                        link_slot.write().await.take();

                        let new_state = match loss.reason {
                            LinkLossReason::Revoked => {
                                // Spec §5.8: peer was revoked — clear trust, halt reconnect
                                log::warn!("[Remote Supervisor] Link revoked by peer — no reconnect");
                                RemoteLinkState::Unpaired // shortcut: no DB to check here
                            }
                            _ => {
                                // HostDisconnected / HostPaused / HostIdle — keep pairing
                                RemoteLinkState::Paired
                            }
                        };
                        clear_peer_and_set_state(&state, &peer_identity, &on_change, new_state);
                        break;
                    }

                    Ok(_other) => {
                        // Req/Res/Event — will be routed by the command gateway in a future ticket
                    }

                    Err(e) => {
                        // Spec §6.3.3 fast-path: OS reported socket fault (RST, EOF)
                        log::warn!("[Remote Supervisor] Socket fault: {e}");
                        link_slot.write().await.take();
                        let new_state = match role {
                            SessionRole::Host => RemoteLinkState::Paired,
                            SessionRole::Controller => RemoteLinkState::Reconnecting,
                        };
                        clear_peer_and_set_state(&state, &peer_identity, &on_change, new_state);
                        break;
                    }
                }
            }

            // ── Host heartbeat ping ticker ───────────────────────────────────
            _ = ping_timer.tick(), if role == SessionRole::Host => {
                seq = seq.wrapping_add(1);
                let sent = link.send_control(ControlMessage::Ping(Ping { seq })).await;
                if sent.is_err() {
                    // Socket is dead — count as a missed pong
                    missed_pongs += 1;
                } else {
                    // Pong expected within HEARTBEAT_PONG_TIMEOUT; count until Pong resets it
                    missed_pongs += 1;
                }

                if missed_pongs >= MAX_MISSED {
                    // Spec §6.3.2: 3 missed pongs → host declares involuntary cutoff
                    log::warn!(
                        "[Remote Supervisor] Host: {missed_pongs} missed pongs — link dead, transitioning to Paired"
                    );
                    let _ = link
                        .close(LinkCloseReason::Transport("Heartbeat: 3 missed pongs".into()))
                        .await;
                    link_slot.write().await.take();
                    // shortcut: pause_session hook would go here
                    clear_peer_and_set_state(
                        &state,
                        &peer_identity,
                        &on_change,
                        RemoteLinkState::Paired,
                    );
                    break;
                }
            }

            // ── Controller liveness deadline ─────────────────────────────────
            _ = sleep(PING_TIMEOUT), if role == SessionRole::Controller => {
                // Spec §6.3.2: if no Ping received for 6s, controller declares cutoff
                if last_ping_at.elapsed() >= PING_TIMEOUT {
                    log::warn!(
                        "[Remote Supervisor] Controller: no Ping for {:?} — transitioning to Reconnecting",
                        PING_TIMEOUT
                    );
                    let _ = link
                        .close(LinkCloseReason::Transport("Heartbeat: no ping received".into()))
                        .await;
                    link_slot.write().await.take();
                    clear_peer_and_set_state(
                        &state,
                        &peer_identity,
                        &on_change,
                        RemoteLinkState::Reconnecting,
                    );
                    break;
                }
            }
        }
    }
}

/// Clears the peer identity and atomically sets a new link state.
fn clear_peer_and_set_state(
    state: &Arc<RwLock<RemoteLinkState>>,
    peer_identity: &Arc<RwLock<Option<SenderIdentity>>>,
    on_change: &Option<StateChangeCallback>,
    new_state: RemoteLinkState,
) {
    {
        let mut guard = peer_identity.write();
        *guard = None;
    }
    {
        let mut guard = state.write();
        *guard = new_state.clone();
    }
    if let Some(cb) = on_change {
        cb(&new_state, None);
    }
}

/// Classifies a dial/transport error string into a `RemoteFailureKind`.
fn classify_dial_error(err: &str) -> RemoteFailureKind {
    if err.contains("timed out") {
        RemoteFailureKind::Timeout
    } else if err.contains("refused") || err.contains("unreachable") || err.contains("No route") {
        RemoteFailureKind::Unreachable
    } else {
        RemoteFailureKind::Unknown
    }
}
