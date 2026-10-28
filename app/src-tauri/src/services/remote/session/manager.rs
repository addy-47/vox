//! Remote session manager — Tier 1 (pairing) and Tier 2 (transport) lifecycle.
//!
//! Strict separation per spec §4.4:
//! - `pair_device`: Tier 1 — ephemeral token handshake, writes DB, closes socket.
//! - `dial_and_connect`: Tier 2 — assumes device is already in paired_devices; dials,
//!   runs Noise handshake, exchanges Hello, spawns Link Supervisor.
//! - `spawn_link_supervisor`: Active duplex reader. The ONLY reader of recv_control()
//!   while a link is alive. Dispatches Ping/Pong, Disconnect, LinkLoss, and socket faults.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::RwLock;
use tokio::sync::RwLock as AsyncRwLock;
use tokio_util::sync::CancellationToken;

use super::state::{RemoteFailureKind, RemoteLinkState, RemoteStateDto};
use crate::services::remote::{
    discovery,
    link::{
        ws::{dial_with_timeout, WsLink},
        LinkCloseReason, LinkError, RemoteLink,
    },
    protocol::{
        capabilities::CAP_CONTROL_MESSAGES,
        errors::ProtocolError,
        framing::StreamFormat,
        messages::{
            ControlMessage, Hello, LinkLoss, LinkLossReason, Ping, Pong, SenderIdentity,
            VersionMismatch,
        },
    },
    DIAL_CANDIDATE_TIMEOUT, HEARTBEAT_INTERVAL, HEARTBEAT_MAX_MISSED_PONGS, HEARTBEAT_PING_TIMEOUT,
    PROTOCOL_VERSION, REMOTE_LISTEN_PORT,
};

/// Type alias for state change subscriber callbacks.
pub type StateChangeCallback = Arc<dyn Fn(&RemoteLinkState, Option<&SenderIdentity>) + Send + Sync>;

/// Role played in the remote session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    Host,
    Controller,
}

/// Parameters for supervising link health via the full duplex recv_control loop.
/// Spec §6.3: handles Ping/Pong, Disconnect, LinkLoss, heartbeat, and faults.
struct LinkSupervisorParams {
    role: SessionRole,
    link: Arc<dyn RemoteLink>,
    cancel: CancellationToken,
    state: Arc<RwLock<RemoteLinkState>>,
    peer_identity: Arc<RwLock<Option<SenderIdentity>>>,
    on_change: Option<StateChangeCallback>,
    link_slot: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
    db_opt: Option<Arc<crate::persistence::VoxDb>>,
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
    db: Arc<parking_lot::RwLock<Option<Arc<crate::persistence::VoxDb>>>>,
    trust_store: Arc<parking_lot::RwLock<Option<Arc<crate::services::remote::security::TrustStore>>>>,
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
            db: Arc::new(parking_lot::RwLock::new(None)),
            trust_store: Arc::new(parking_lot::RwLock::new(None)),
        }
    }

    /// Creates a new `RemoteSessionManager` with an attached database and state transition listener.
    pub fn with_callback(
        role: SessionRole,
        db: Arc<crate::persistence::VoxDb>,
        callback: StateChangeCallback,
    ) -> Self {
        Self {
            role,
            state: Arc::new(RwLock::new(RemoteLinkState::Unpaired)),
            peer_identity: Arc::new(RwLock::new(None)),
            link: Arc::new(AsyncRwLock::new(None)),
            cancel_token: Arc::new(parking_lot::Mutex::new(None)),
            on_state_change: Some(callback),
            db: Arc::new(parking_lot::RwLock::new(Some(db))),
            trust_store: Arc::new(parking_lot::RwLock::new(None)),
        }
    }

    /// Attaches the trust store for cryptographic revocation and refusal tracking.
    pub fn set_trust_store(&self, trust_store: Arc<crate::services::remote::security::TrustStore>) {
        *self.trust_store.write() = Some(trust_store);
    }

    /// Attaches the shared database handle for supervisor revocation sync.
    pub fn set_db(&self, db: Arc<crate::persistence::VoxDb>) {
        *self.db.write() = Some(db);
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

    /// Sets or clears the cached peer identity.
    pub fn set_peer_identity(&self, peer: Option<SenderIdentity>) {
        let mut guard = self.peer_identity.write();
        *guard = peer.map(|mut p| {
            p.display_name = sanitize_peer_display_name(&p.display_name);
            p
        });
    }

    /// Hydrates peer identity from local database on startup or settings reload.
    pub async fn hydrate_from_db(&self, db: &crate::persistence::VoxDb) {
        if let Ok(devices) = Self::list_paired_devices(db).await {
            let target = if devices.is_empty() {
                RemoteLinkState::Unpaired
            } else {
                RemoteLinkState::Paired
            };

            if let Some(first) = devices.first() {
                self.set_peer_identity(Some(SenderIdentity {
                    device_id: first.device_id.clone(),
                    display_name: first.display_name.clone(),
                }));
            } else {
                self.set_peer_identity(None);
            }

            if matches!(
                self.state(),
                RemoteLinkState::Unpaired | RemoteLinkState::Paired | RemoteLinkState::Disabled
            ) && self.state() != target
            {
                self.set_state(target);
            }
        }
    }

    /// Transitions to a new link state and invokes listener callbacks.
    pub fn set_state(&self, new_state: RemoteLinkState) {
        if matches!(new_state, RemoteLinkState::Unpaired | RemoteLinkState::Disabled) {
            let mut guard = self.peer_identity.write();
            *guard = None;
        }
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

    // ─── Ticket 02: Hello Exchange ──────────────────────────────────────────

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
                Err(LinkError::PeerRejected(crate::services::remote::link::PeerRejection::VersionMismatch {
                    version: mismatch.version,
                    supported_version: mismatch.supported_version,
                }))
            }
            ControlMessage::UnknownPeer(peer) => {
                Err(LinkError::PeerRejected(crate::services::remote::link::PeerRejection::UnknownPeer(
                    peer.device_id,
                )))
            }
            ControlMessage::RoleNotPermitted(role) => {
                Err(LinkError::PeerRejected(crate::services::remote::link::PeerRejection::RoleNotPermitted(
                    role.role,
                )))
            }
            ControlMessage::LinkRefused(refused) => {
                Err(LinkError::PeerRejected(crate::services::remote::link::PeerRejection::LinkRefused(
                    refused.reason,
                )))
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
        let db_opt = self.db.read().clone();

        tokio::spawn(async move {
            run_link_supervisor(LinkSupervisorParams {
                role,
                link,
                cancel,
                state: state_arc,
                peer_identity: peer_arc,
                on_change,
                link_slot,
                db_opt,
            })
            .await;
        });
    }

    // ─── Ticket 02: Tier 1 — pair_device ────────────────────────────────────

    async fn perform_pairing_handshake(
        address: &str,
        token: &str,
        host_public_key: &str,
    ) -> Result<(crate::services::remote::link::ws::WsLink, Vec<u8>), LinkError> {
        let url = if address.starts_with("ws://") || address.starts_with("wss://") {
            address.to_string()
        } else {
            format!("ws://{address}")
        };
        let ws_link = crate::services::remote::link::ws::dial(&url).await?;

        let key_store = crate::services::remote::security::KeyStore::default_path()
            .map_err(|e| LinkError::Transport(e.to_string()))?;
        let device_key = key_store
            .load_or_create()
            .map_err(|e| LinkError::Transport(e.to_string()))?;

        let controller_id =
            crate::services::remote::security::derive_device_id(device_key.public_key());
        let controller_pubkey_hex = hex::encode(device_key.public_key());
        let controller_name = controller_display_name();

        let pair_req = ControlMessage::PairRequest(
            crate::services::remote::protocol::messages::PairRequest {
                token: token.to_string(),
                device_id: controller_id.as_str().to_string(),
                public_key: controller_pubkey_hex,
                display_name: controller_name,
            },
        );
        ws_link.send_control(pair_req).await?;

        let h_pk_bytes = hex::decode(host_public_key).map_err(|e| {
            let msg = format!("Invalid host public key hex: {e}");
            LinkError::Protocol(ProtocolError::MalformedJson(msg))
        })?;
        let prologue = crate::services::remote::security::default_noise_prologue();
        let handshake = crate::services::remote::security::NoiseHandshake::initiator(
            &device_key,
            Some(&h_pk_bytes),
            &prologue,
        )
        .map_err(|e| LinkError::Protocol(ProtocolError::MalformedJson(e.to_string())))?;

        let transport = tokio::time::timeout(
            Duration::from_secs(5),
            ws_link.drive_handshake(handshake),
        )
        .await
        .map_err(|_| LinkError::Timeout)?
        .map_err(|e| LinkError::Protocol(ProtocolError::MalformedJson(e.to_string())))?;

        ws_link.set_cipher(transport).await;
        Ok((ws_link, h_pk_bytes))
    }

    async fn await_pairing_approval(
        ws_link: &crate::services::remote::link::ws::WsLink,
        on_prompting: Option<Box<dyn FnOnce() + Send>>,
    ) -> Result<crate::services::remote::protocol::messages::PairApproved, LinkError> {
        let mut resp =
            match tokio::time::timeout(Duration::from_secs(35), ws_link.recv_control()).await {
                Ok(Ok(msg)) => msg,
                Ok(Err(e)) => return Err(e),
                Err(_) => {
                    let msg = "Pairing timed out waiting for host";
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
                Ok(Err(e)) => return Err(e),
                Err(_) => {
                    let msg = "Host did not approve pairing within 35s";
                    return Err(LinkError::Protocol(ProtocolError::MalformedJson(
                        msg.to_string(),
                    )));
                }
            };
        }

        match resp {
            ControlMessage::PairApproved(approved) => Ok(approved),
            ControlMessage::PairDenied(denied) => {
                let msg = format!("Pairing denied by host: {}", denied.reason);
                let _ = ws_link.close(LinkCloseReason::Local).await;
                Err(LinkError::Protocol(ProtocolError::MalformedJson(msg)))
            }
            other => {
                let msg = format!("Expected PairApproved, got {:?}", other.message_type());
                let _ = ws_link.close(LinkCloseReason::Local).await;
                Err(LinkError::Protocol(ProtocolError::MalformedJson(msg)))
            }
        }
    }

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
        self.set_db(Arc::clone(&db));

        let (ws_link, h_pk_bytes) =
            Self::perform_pairing_handshake(&address, &token, &host_public_key).await?;

        let approved = Self::await_pairing_approval(&ws_link, on_prompting).await?;

        let host_display_name = if !approved.host_identity.display_name.is_empty() {
            approved.host_identity.display_name
        } else {
            "Vox Host".to_string()
        };

        if let Ok(conn) = db.connect() {
            let _ = crate::persistence::paired_devices::insert_paired_device(
                &conn,
                &crate::persistence::paired_devices::NewPairedDevice {
                    device_id: host_device_id.clone(),
                    public_key: h_pk_bytes,
                    display_name: host_display_name.clone(),
                    role: crate::persistence::paired_devices::PairedDeviceRole::Host,
                    trust_level: "full".to_string(),
                    hints: vec![serde_json::json!({ "addr": address })],
                },
            )
            .await;
        }

        let _ = ws_link.close(LinkCloseReason::Local).await;
        self.set_peer_identity(Some(SenderIdentity {
            device_id: host_device_id.clone(),
            display_name: host_display_name,
        }));
        self.set_state(RemoteLinkState::Paired);
        log::info!("[Remote] Pairing complete with host {host_device_id}. Tier 1 socket closed.");
        Ok(())
    }

    // ─── Ticket 02: Tier 2 — dial_and_connect ───────────────────────────────

    /// Dials a candidate and runs the Noise handshake to completion.
    ///
    /// Returns the sealed link paired with our local identity, or the
    /// classified transport error (spec §6.1 candidate ordering).
    async fn dial_link(
        &self,
        candidate: &str,
        host_device: &crate::persistence::paired_devices::PairedDevice,
    ) -> Result<(WsLink, SenderIdentity), LinkError> {
        let url = discovery::dial_url(candidate, REMOTE_LISTEN_PORT);
        let ws_link = dial_with_timeout(&url, DIAL_CANDIDATE_TIMEOUT).await?;
        let local_identity = self
            .drive_noise_handshake(&ws_link, &host_device.public_key)
            .await?;
        Ok((ws_link, local_identity))
    }

    /// Drives the Noise initiator handshake over a freshly dialed link.
    async fn drive_noise_handshake(
        &self,
        ws_link: &WsLink,
        host_public_key: &[u8],
    ) -> Result<SenderIdentity, LinkError> {
        let key_store = crate::services::remote::security::KeyStore::default_path()
            .map_err(|e| LinkError::Transport(e.to_string()))?;
        let device_key = key_store
            .load_or_create()
            .map_err(|e| LinkError::Transport(e.to_string()))?;

        let prologue = crate::services::remote::security::default_noise_prologue();
        let handshake = crate::services::remote::security::NoiseHandshake::initiator(
            &device_key,
            Some(host_public_key),
            &prologue,
        )
        .map_err(|e| LinkError::Protocol(ProtocolError::MalformedJson(e.to_string())))?;

        let transport = tokio::time::timeout(
            Duration::from_secs(5),
            ws_link.drive_handshake(handshake),
        )
        .await
        .map_err(|_| LinkError::Timeout)?
        .map_err(|e| LinkError::Protocol(ProtocolError::MalformedJson(e.to_string())))?;

        ws_link.set_cipher(transport).await;

        let controller_id =
            crate::services::remote::security::derive_device_id(device_key.public_key());
        let controller_name = controller_display_name();
        Ok(SenderIdentity {
            device_id: controller_id.as_str().to_string(),
            display_name: controller_name,
        })
    }

    /// Dial candidate resolution. Spec §6.1 (in order): explicit provided
    /// address (QR / manual entry) → stored `paired_devices.hints` → mDNS
    /// browse for the host `device_id` (the fallback that survives network
    /// changes; refreshed hints are persisted before the first mDNS dial).
    async fn perform_dial_handshake(
        &self,
        host_device: &crate::persistence::paired_devices::PairedDevice,
        address: Option<String>,
    ) -> Result<(WsLink, SenderIdentity, String), LinkError> {
        let mut candidates: Vec<String> = Vec::new();
        if let Some(addr) = address {
            candidates.push(addr);
        }
        candidates.extend(discovery::lan_hint_addrs(&host_device.hints));

        log::info!(
            "[Remote] perform_dial_handshake for {}: stored candidates={:?}",
            host_device.device_id,
            candidates
        );

        let mut last_error: Option<LinkError> = None;
        for candidate in &candidates {
            match self.dial_link(candidate, host_device).await {
                Ok((link, identity)) => return Ok((link, identity, candidate.clone())),
                Err(e) => {
                    log::warn!("[Remote] Dial candidate {candidate} failed: {e}");
                    last_error = Some(e);
                }
            }
        }

        // mDNS fallback: the stored hints are IPs captured at pairing time —
        // stale after any network change. Browse the LAN by host device id.
        log::info!(
            "[Remote] Stored candidates exhausted. Triggering mDNS browse for {} (timeout={:?})",
            host_device.device_id,
            discovery::MDNS_BROWSE_TIMEOUT
        );
        let mdns_candidates =
            discovery::discover_host(&host_device.device_id, discovery::MDNS_BROWSE_TIMEOUT)
                .await
                .unwrap_or_else(|e| {
                    log::warn!("[Remote] mDNS browse failed: {e}");
                    Vec::new()
                });

        log::info!(
            "[Remote] mDNS browse returned {} candidate(s): {:?}",
            mdns_candidates.len(),
            mdns_candidates
        );

        for candidate in &mdns_candidates {
            match self.dial_link(candidate, host_device).await {
                Ok((link, identity)) => return Ok((link, identity, candidate.clone())),
                Err(e) => {
                    log::warn!("[Remote] mDNS candidate {candidate} failed: {e}");
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            LinkError::Transport("No dial candidates".into())
        }))
    }

    async fn handle_dial_rejection(
        &self,
        db: &crate::persistence::VoxDb,
        device_id: &str,
        error: LinkError,
    ) -> LinkError {
        match &error {
            LinkError::PeerRejected(crate::services::remote::link::PeerRejection::UnknownPeer(_)) => {
                if let Ok(conn) = db.connect() {
                    let _ = crate::persistence::paired_devices::delete_paired_device(&conn, device_id).await;
                }
                let remaining = if let Ok(conn) = db.connect() {
                    crate::persistence::paired_devices::list_paired_devices(&conn)
                        .await
                        .map(|v| v.len())
                        .unwrap_or(0)
                } else {
                    0
                };
                let new_state = if remaining == 0 {
                    RemoteLinkState::Unpaired
                } else {
                    RemoteLinkState::Paired
                };
                self.set_state(new_state);
            }
            LinkError::PeerRejected(crate::services::remote::link::PeerRejection::LinkRefused(reason)) => {
                let msg = match reason {
                    crate::services::remote::protocol::messages::LinkRefusedReason::ControllerAttached => {
                        "Another controller is already connected".to_string()
                    }
                    crate::services::remote::protocol::messages::LinkRefusedReason::NotAccepting => {
                        "Host is not accepting controllers".to_string()
                    }
                    crate::services::remote::protocol::messages::LinkRefusedReason::HostBusy => {
                        "Host is currently busy with local session".to_string()
                    }
                };
                self.set_state(RemoteLinkState::failed(msg, RemoteFailureKind::RejectedByHost));
            }
            LinkError::PeerRejected(crate::services::remote::link::PeerRejection::RoleNotPermitted(r)) => {
                self.set_state(RemoteLinkState::failed(
                    format!("Role not permitted by host: {r}"),
                    RemoteFailureKind::RejectedByHost,
                ));
            }
            LinkError::PeerRejected(crate::services::remote::link::PeerRejection::VersionMismatch {
                version,
                supported_version,
            }) => {
                self.set_state(RemoteLinkState::failed(
                    format!("Protocol version mismatch: peer {version}, supported {supported_version}"),
                    RemoteFailureKind::VersionMismatch,
                ));
            }
            LinkError::Timeout => {
                self.set_state(RemoteLinkState::failed(
                    format!("Handshake timed out connecting to {device_id}"),
                    RemoteFailureKind::Timeout,
                ));
            }
            _ => {
                self.set_state(RemoteLinkState::failed(
                    format!("Connection error connecting to {device_id}: {error}"),
                    RemoteFailureKind::Unknown,
                ));
            }
        }
        error
    }

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
        self.set_db(Arc::clone(&db));

        let host_device = if let Ok(conn) = db.connect() {
            crate::persistence::paired_devices::get_paired_device(&conn, &device_id)
                .await
                .ok()
                .flatten()
        } else {
            None
        };
        let host_device = host_device.ok_or_else(|| {
            LinkError::Transport(format!("Device {device_id} not found in paired_devices"))
        })?;

        self.set_peer_identity(Some(SenderIdentity {
            device_id: host_device.device_id.clone(),
            display_name: host_device.display_name.clone(),
        }));
        self.set_state(RemoteLinkState::Connecting);

        let (ws_link, local_identity, chosen_candidate) =
            self.perform_dial_handshake(&host_device, address)
                .await
                .map_err(|e| {
                    let kind = classify_dial_error(&e.to_string());
                    self.set_state(RemoteLinkState::failed(format!("Dial failed: {e}"), kind));
                    e
                })?;

        // Spec §5.7: hints update on successful connection (source: Observed).
        // The dial candidate that worked is the freshest known address, placed at index 0
        // while preserving existing hints for forward compatibility.
        if let Ok(conn) = db.connect() {
            let hints = discovery::update_observed_hint(&host_device.hints, &chosen_candidate);
            let _ = crate::persistence::paired_devices::update_paired_device_hints(
                &conn,
                &device_id,
                &hints,
            )
            .await;
        }

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
                return Err(self.handle_dial_rejection(&db, &device_id, e).await);
            }
        };

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

    async fn sever_active_link(&self, device_id: &str, has_active_link: bool) {
        let is_current = self
            .peer_identity()
            .map(|p| p.device_id == device_id)
            .unwrap_or(false);

        if has_active_link && is_current {
            if let Some(link) = self.link.write().await.take() {
                let _ = link
                    .send_control(ControlMessage::LinkLoss(LinkLoss {
                        reason: LinkLossReason::Revoked,
                    }))
                    .await;
                tokio::time::sleep(Duration::from_millis(150)).await;
                let _ = link.close(LinkCloseReason::Local).await;
            }
            let mut guard = self.peer_identity.write();
            *guard = None;
        }
    }

    async fn notify_idle_host_revoked(
        &self,
        conn: &turso::Connection,
        device_id: &str,
    ) {
        if self.role != SessionRole::Controller {
            return;
        }
        let device = crate::persistence::paired_devices::get_paired_device(conn, device_id)
            .await
            .ok()
            .flatten();
        let Some(device) = device else { return; };
        let Some(addr) = device
            .hints
            .first()
            .and_then(|h| h.get("addr").and_then(|v| v.as_str()).map(|s| s.to_string()))
        else {
            return;
        };
        let host_pk = device.public_key;

        tokio::spawn(async move {
            let url = if addr.starts_with("ws://") || addr.starts_with("wss://") {
                addr
            } else {
                format!("ws://{addr}")
            };
            let _ = tokio::time::timeout(Duration::from_secs(2), async move {
                if let Ok(ws) = crate::services::remote::link::ws::dial(&url).await {
                    if let Ok(ks) = crate::services::remote::security::KeyStore::default_path()
                        .and_then(|k| k.load_or_create())
                    {
                        let prologue = crate::services::remote::security::default_noise_prologue();
                        if let Ok(handshake) =
                            crate::services::remote::security::NoiseHandshake::initiator(
                                &ks,
                                Some(&host_pk),
                                &prologue,
                            )
                        {
                            if let Ok(transport) = ws.drive_handshake(handshake).await {
                                ws.set_cipher(transport).await;
                                let _ = ws
                                    .send_control(ControlMessage::LinkLoss(LinkLoss {
                                        reason: LinkLossReason::Revoked,
                                    }))
                                    .await;
                                let _ = ws.close(LinkCloseReason::Local).await;
                            }
                        }
                    }
                }
            })
            .await;
        });
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
        // Spec §5.8 / §3.3: cancel supervisor first so it does not emit transient state changes
        if let Some(token) = self.cancel_token.lock().take() {
            token.cancel();
        }

        let has_active_link = self.link.read().await.is_some();
        self.sever_active_link(device_id, has_active_link).await;

        let conn = db.connect()?;
        if !has_active_link {
            self.notify_idle_host_revoked(&conn, device_id).await;
        }

        let trust_store_opt = self.trust_store.read().clone();
        let deleted = if let Some(ts) = trust_store_opt {
            ts.revoke(&conn, device_id).await?
        } else {
            crate::persistence::paired_devices::delete_paired_device(&conn, device_id).await?
        };

        // Determine new state from remaining count (spec §5.8)
        let remaining = crate::persistence::paired_devices::list_paired_devices(&conn)
            .await
            .map(|v| v.len())
            .unwrap_or(0);
        if remaining == 0 {
            self.set_peer_identity(None);
            self.set_state(RemoteLinkState::Unpaired);
        } else {
            self.hydrate_from_db(db).await;
        }
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

        // Keep pairing: transition to Paired (spec §6.6 — NOT Unpaired)
        // peer_identity is retained because the device remains in paired_devices DB.
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

        // Keep pairing: transition to Paired (spec §6.4)
        // peer_identity is retained because the device remains in paired_devices DB.
        self.set_state(RemoteLinkState::Paired);
        Ok(())
    }
}

// ─── Ticket 03: Duplex Link Supervisor ──────────────────────────────────────

enum SupervisorFlow {
    Continue,
    Break,
}

async fn handle_supervisor_message(
    params: &LinkSupervisorParams,
    msg: ControlMessage,
    seq: u64,
    last_ping_at: &mut Instant,
    missed_pongs: &mut u32,
) -> SupervisorFlow {
    match msg {
        ControlMessage::Ping(p) => {
            *last_ping_at = Instant::now();
            let _ = params.link.send_control(ControlMessage::Pong(Pong { seq: p.seq })).await;
            SupervisorFlow::Continue
        }
        ControlMessage::Pong(p) => {
            if p.seq == seq {
                *missed_pongs = 0;
            }
            SupervisorFlow::Continue
        }
        ControlMessage::Disconnect => {
            log::info!("[Remote Supervisor] Received Disconnect from controller — transitioning to Paired");
            let _ = params.link.close(LinkCloseReason::Local).await;
            params.link_slot.write().await.take();
            transition_supervisor_state(
                &params.state,
                &params.peer_identity,
                &params.on_change,
                RemoteLinkState::Paired,
            );
            SupervisorFlow::Break
        }
        ControlMessage::LinkLoss(loss) => {
            log::info!("[Remote Supervisor] Received LinkLoss({:?})", loss.reason);
            let _ = params.link.close(LinkCloseReason::Local).await;
            params.link_slot.write().await.take();
            let new_state = handle_supervisor_link_loss(&params.peer_identity, &params.db_opt, loss.reason).await;
            transition_supervisor_state(
                &params.state,
                &params.peer_identity,
                &params.on_change,
                new_state,
            );
            SupervisorFlow::Break
        }
        _ => SupervisorFlow::Continue,
    }
}

async fn handle_supervisor_link_loss(
    peer_identity: &Arc<RwLock<Option<SenderIdentity>>>,
    db_opt: &Option<Arc<crate::persistence::VoxDb>>,
    reason: LinkLossReason,
) -> RemoteLinkState {
    if reason != LinkLossReason::Revoked {
        return RemoteLinkState::Paired;
    }
    log::warn!("[Remote Supervisor] Link revoked by peer — deleting from DB");
    let peer_id = peer_identity.read().as_ref().map(|p| p.device_id.clone());
    if let (Some(id), Some(db)) = (peer_id, db_opt) {
        if let Ok(conn) = db.connect() {
            let _ = crate::persistence::paired_devices::delete_paired_device(&conn, &id).await;
            let remaining = crate::persistence::paired_devices::list_paired_devices(&conn)
                .await
                .map(|d| d.len())
                .unwrap_or(0);
            if remaining == 0 {
                return RemoteLinkState::Unpaired;
            }
            return RemoteLinkState::Paired;
        }
    }
    RemoteLinkState::Unpaired
}

async fn handle_supervisor_fault(params: &LinkSupervisorParams, err: &LinkError) {
    log::warn!("[Remote Supervisor] Socket fault: {err}");
    params.link_slot.write().await.take();
    if params.cancel.is_cancelled() {
        log::debug!("[Remote Supervisor] Cancelled during shutdown — ignoring socket fault");
        return;
    }

    let is_still_paired = if let Some(db) = &params.db_opt {
        let peer_id = params.peer_identity.read().as_ref().map(|p| p.device_id.clone());
        if let (Some(id), Ok(conn)) = (peer_id, db.connect()) {
            crate::persistence::paired_devices::get_paired_device(&conn, &id)
                .await
                .ok()
                .flatten()
                .is_some()
        } else {
            false
        }
    } else {
        true
    };

    let new_state = if !is_still_paired {
        log::warn!("[Remote Supervisor] Peer is no longer in paired_devices — transitioning to Unpaired");
        RemoteLinkState::Unpaired
    } else {
        match params.role {
            SessionRole::Host => RemoteLinkState::Paired,
            SessionRole::Controller => RemoteLinkState::Reconnecting,
        }
    };
    transition_supervisor_state(
        &params.state,
        &params.peer_identity,
        &params.on_change,
        new_state,
    );
}

async fn handle_host_heartbeat_tick(
    params: &LinkSupervisorParams,
    seq: &mut u64,
    missed_pongs: &mut u32,
) -> SupervisorFlow {
    *seq = seq.wrapping_add(1);
    let _ = params.link.send_control(ControlMessage::Ping(Ping { seq: *seq })).await;
    *missed_pongs += 1;

    if *missed_pongs >= HEARTBEAT_MAX_MISSED_PONGS {
        log::warn!("[Remote Supervisor] Host: {} missed pongs — link dead, transitioning to Paired", *missed_pongs);
        let _ = params.link.close(LinkCloseReason::Transport("Heartbeat: 3 missed pongs".into())).await;
        params.link_slot.write().await.take();
        transition_supervisor_state(&params.state, &params.peer_identity, &params.on_change, RemoteLinkState::Paired);
        SupervisorFlow::Break
    } else {
        SupervisorFlow::Continue
    }
}

async fn handle_controller_heartbeat_timeout(params: &LinkSupervisorParams) -> SupervisorFlow {
    log::warn!("[Remote Supervisor] Controller: no Ping for {:?} — transitioning to Reconnecting", HEARTBEAT_PING_TIMEOUT);
    let _ = params.link.close(LinkCloseReason::Transport("Heartbeat: no ping received".into())).await;
    params.link_slot.write().await.take();
    transition_supervisor_state(&params.state, &params.peer_identity, &params.on_change, RemoteLinkState::Reconnecting);
    SupervisorFlow::Break
}

async fn run_link_supervisor(params: LinkSupervisorParams) {
    let mut seq: u64 = 0;
    let mut missed_pongs: u32 = 0;
    let mut last_ping_at = Instant::now();

    let mut ping_timer = tokio::time::interval(HEARTBEAT_INTERVAL);
    ping_timer.tick().await;

    loop {
        tokio::select! {
            _ = params.cancel.cancelled() => {
                log::debug!("[Remote Supervisor] cancelled — exiting");
                break;
            }
            msg = params.link.recv_control() => {
                match msg {
                    Ok(m) => {
                        if let SupervisorFlow::Break = handle_supervisor_message(
                            &params,
                            m,
                            seq,
                            &mut last_ping_at,
                            &mut missed_pongs,
                        ).await {
                            break;
                        }
                    }
                    Err(e) => {
                        handle_supervisor_fault(&params, &e).await;
                        break;
                    }
                }
            }
            _ = ping_timer.tick(), if params.role == SessionRole::Host => {
                if let SupervisorFlow::Break = handle_host_heartbeat_tick(&params, &mut seq, &mut missed_pongs).await {
                    break;
                }
            }
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(last_ping_at + HEARTBEAT_PING_TIMEOUT)), if params.role == SessionRole::Controller => {
                if last_ping_at.elapsed() >= HEARTBEAT_PING_TIMEOUT {
                    handle_controller_heartbeat_timeout(&params).await;
                    break;
                }
            }
        }
    }
}

/// Atomically sets a new link state from supervisor, only clearing peer identity if Unpaired or Disabled.
fn transition_supervisor_state(
    state: &Arc<RwLock<RemoteLinkState>>,
    peer_identity: &Arc<RwLock<Option<SenderIdentity>>>,
    on_change: &Option<StateChangeCallback>,
    new_state: RemoteLinkState,
) {
    if matches!(new_state, RemoteLinkState::Unpaired | RemoteLinkState::Disabled) {
        let mut guard = peer_identity.write();
        *guard = None;
    }
    {
        let mut guard = state.write();
        *guard = new_state.clone();
    }
    let peer = peer_identity.read().clone();
    if let Some(cb) = on_change {
        cb(&new_state, peer.as_ref());
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

/// Computes the controller device display name, falling back from Android's default
/// "localhost" hostname to "Vox Phone".
pub(crate) fn controller_display_name() -> String {
    let name = sysinfo::System::host_name().unwrap_or_default();
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("localhost")
        || trimmed.eq_ignore_ascii_case("localhost.localdomain")
    {
        if cfg!(target_os = "android") {
            "Vox Phone".to_string()
        } else {
            "Vox Controller".to_string()
        }
    } else {
        trimmed.to_string()
    }
}

/// Sanitizes a peer display name, replacing "localhost" and empty names with "Vox Phone".
pub(crate) fn sanitize_peer_display_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("localhost")
        || trimmed.eq_ignore_ascii_case("localhost.localdomain")
    {
        "Vox Phone".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Returns the host machine display name, filtering out generic "localhost".
pub(crate) fn host_display_name() -> String {
    let name = sysinfo::System::host_name().unwrap_or_default();
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("localhost")
        || trimmed.eq_ignore_ascii_case("localhost.localdomain")
    {
        "Vox Host".to_string()
    } else {
        trimmed.to_string()
    }
}
