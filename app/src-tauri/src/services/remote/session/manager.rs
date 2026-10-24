use std::{future::Future, sync::Arc, time::Duration};

use parking_lot::RwLock;
use tokio::{sync::RwLock as AsyncRwLock, time::sleep};
use tokio_util::sync::CancellationToken;

use super::state::{RemoteLinkState, RemoteStateDto};
use crate::services::remote::{
    link::{LinkCloseReason, LinkError, RemoteLink},
    protocol::{
        capabilities::CAP_CONTROL_MESSAGES,
        errors::ProtocolError,
        framing::StreamFormat,
        messages::{
            ControlMessage, Hello, LinkLoss, LinkLossReason, Ping, SenderIdentity, VersionMismatch,
        },
    },
    security::{default_noise_prologue, DeviceKey, NoiseHandshake, NoiseTransport, SecurityError},
    HEARTBEAT_INTERVAL, HEARTBEAT_MAX_MISSED_PONGS, PROTOCOL_VERSION,
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
/// capabilities exchange, and heartbeat monitoring.
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
            state: Arc::new(RwLock::new(RemoteLinkState::Disconnected)),
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
            state: Arc::new(RwLock::new(RemoteLinkState::Disconnected)),
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
        self.start_heartbeat(link);
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
            other => Err(LinkError::Protocol(ProtocolError::MalformedJson(format!(
                "Expected Hello, got {:?}",
                other.message_type()
            )))),
        }
    }

    /// Starts the background heartbeat ping/pong supervisor task.
    pub fn start_heartbeat(&self, link: Arc<dyn RemoteLink>) {
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

        tokio::spawn(async move {
            run_heartbeat_loop(role, link, cancel, state_arc, peer_arc, on_change).await;
        });
    }

    /// Cleanly closes the active session.
    pub async fn disconnect(&self, reason: LinkLossReason) -> Result<(), LinkError> {
        if let Some(token) = self.cancel_token.lock().take() {
            token.cancel();
        }

        let link_opt = self.link.write().await.take();
        if let Some(link) = link_opt {
            match self.role {
                SessionRole::Host => {
                    let _ = link
                        .send_control(ControlMessage::LinkLoss(LinkLoss { reason }))
                        .await;
                }
                SessionRole::Controller => {
                    let _ = link.send_control(ControlMessage::Disconnect).await;
                }
            }
            let _ = link.close(LinkCloseReason::Local).await;
        }

        {
            let mut guard = self.peer_identity.write();
            *guard = None;
        }

        self.set_state(RemoteLinkState::Disconnected);
        Ok(())
    }

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

    /// Revokes a paired device from the Turso database and severs connection if currently active.
    pub async fn revoke_device(
        &self,
        db: &crate::persistence::VoxDb,
        device_id: &str,
    ) -> Result<bool, crate::core::error::PersistenceError> {
        let conn = db.connect()?;
        let deleted =
            crate::persistence::paired_devices::delete_paired_device(&conn, device_id).await?;
        let is_current = self
            .peer_identity()
            .map(|p| p.device_id == device_id)
            .unwrap_or(false);
        if is_current {
            let _ = self.disconnect(LinkLossReason::HostDisconnected).await;
        }
        Ok(deleted)
    }

    /// Dials a remote host from the controller and either initiates first-time pairing
    /// (if `token` and `public_key` are provided) or completes authenticated connection.
    pub async fn dial_and_connect(
        self: Arc<Self>,
        db: Arc<crate::persistence::VoxDb>,
        target: String,
        host_device_id: Option<String>,
        token: Option<String>,
        host_public_key: Option<String>,
    ) -> Result<(), LinkError> {
        self.set_state(RemoteLinkState::Connecting);

        let url = if target.starts_with("ws://") || target.starts_with("wss://") {
            target
        } else {
            format!("ws://{target}")
        };

        let ws_link = match crate::services::remote::link::ws::dial(&url).await {
            Ok(l) => l,
            Err(e) => {
                let err_msg = format!("Failed to connect to host: {e}");
                self.set_state(RemoteLinkState::Failed(err_msg.clone()));
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
        let display_name = "Vox Controller".to_string();

        if let (Some(tok), Some(h_pk)) = (token.as_ref(), host_public_key.as_ref()) {
            let pair_req = ControlMessage::PairRequest(
                crate::services::remote::protocol::messages::PairRequest {
                    token: tok.clone(),
                    device_id: controller_id.as_str().to_string(),
                    public_key: controller_pubkey_hex,
                    display_name: display_name.clone(),
                },
            );

            if let Err(e) = ws_link.send_control(pair_req).await {
                let err_msg = format!("Failed to send pairing request: {e}");
                self.set_state(RemoteLinkState::Failed(err_msg.clone()));
                return Err(e);
            }

            let resp =
                match tokio::time::timeout(Duration::from_secs(35), ws_link.recv_control()).await {
                    Ok(Ok(msg)) => msg,
                    Ok(Err(e)) => {
                        let err_msg = format!("Connection error awaiting pairing response: {e}");
                        self.set_state(RemoteLinkState::Failed(err_msg.clone()));
                        return Err(e);
                    }
                    Err(_) => {
                        let err_msg = "Pairing approval timed out after 35 seconds".to_string();
                        self.set_state(RemoteLinkState::Failed(err_msg.clone()));
                        return Err(LinkError::Protocol(ProtocolError::MalformedJson(err_msg)));
                    }
                };

            match resp {
                ControlMessage::PairApproved => {
                    let h_pk_bytes = hex::decode(h_pk).map_err(|e| {
                        let err_msg = format!("Invalid host public key hex: {e}");
                        self.set_state(RemoteLinkState::Failed(err_msg.clone()));
                        LinkError::Protocol(ProtocolError::MalformedJson(err_msg))
                    })?;
                    let h_id = host_device_id.unwrap_or_else(|| {
                        crate::services::remote::security::derive_device_id(&h_pk_bytes)
                            .as_str()
                            .to_string()
                    });

                    if let Ok(conn) = db.connect() {
                        let _ = crate::persistence::paired_devices::insert_paired_device(
                            &conn,
                            &crate::persistence::paired_devices::NewPairedDevice {
                                device_id: h_id,
                                public_key: h_pk_bytes,
                                display_name: "Vox Host".to_string(),
                                role: crate::persistence::paired_devices::PairedDeviceRole::Host,
                                trust_level: "full".to_string(),
                                hints: vec![],
                            },
                        )
                        .await;
                    }

                    let local_identity = SenderIdentity {
                        device_id: controller_id.as_str().to_string(),
                        display_name,
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
                            let err_msg = format!("Protocol negotiation error after pairing: {e}");
                            self.set_state(RemoteLinkState::Failed(err_msg.clone()));
                            return Err(e);
                        }
                    };

                    self.attach_link(Arc::new(ws_link), peer_hello.sender_identity)
                        .await;
                    Ok(())
                }
                ControlMessage::PairDenied(denied) => {
                    let err_msg = format!("Pairing denied by host: {}", denied.reason);
                    self.set_state(RemoteLinkState::Failed(err_msg.clone()));
                    let _ = ws_link.close(LinkCloseReason::Local).await;
                    Err(LinkError::Protocol(ProtocolError::MalformedJson(err_msg)))
                }
                other => {
                    let err_msg = format!("Expected PairApproved, got {:?}", other.message_type());
                    self.set_state(RemoteLinkState::Failed(err_msg.clone()));
                    let _ = ws_link.close(LinkCloseReason::Local).await;
                    Err(LinkError::Protocol(ProtocolError::MalformedJson(err_msg)))
                }
            }
        } else {
            let local_identity = SenderIdentity {
                device_id: controller_id.as_str().to_string(),
                display_name,
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
                    let err_msg = format!("Handshake error: {e}");
                    self.set_state(RemoteLinkState::Failed(err_msg.clone()));
                    return Err(e);
                }
            };

            self.attach_link(Arc::new(ws_link), peer_hello.sender_identity)
                .await;
            Ok(())
        }
    }
}

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

/// Supervises link health via bidirectional Ping/Pong probes.
async fn run_heartbeat_loop(
    role: SessionRole,
    link: Arc<dyn RemoteLink>,
    cancel: CancellationToken,
    state: Arc<RwLock<RemoteLinkState>>,
    peer_identity: Arc<RwLock<Option<SenderIdentity>>>,
    on_change: Option<StateChangeCallback>,
) {
    let mut seq: u64 = 0;
    let mut missed_pongs: u32 = 0;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = sleep(HEARTBEAT_INTERVAL) => {
                if role == SessionRole::Host {
                    seq = seq.wrapping_add(1);
                    let ping = ControlMessage::Ping(Ping { seq });
                    if link.send_control(ping).await.is_err() {
                        missed_pongs += 1;
                    }

                    if missed_pongs >= HEARTBEAT_MAX_MISSED_PONGS {
                        let _ = link.close(LinkCloseReason::Transport("Heartbeat missed 3 pongs".into())).await;
                        {
                            let mut s = state.write();
                            *s = RemoteLinkState::Disconnected;
                        }
                        if let Some(cb) = &on_change {
                            let p = peer_identity.read().clone();
                            cb(&RemoteLinkState::Disconnected, p.as_ref());
                        }
                        break;
                    }
                }
            }
        }
    }
}
