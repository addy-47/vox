use std::{future::Future, sync::Arc};

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
            ControlMessage, Hello, LinkLoss, LinkLossReason, Ping, SenderIdentity,
            VersionMismatch,
        },
    },
    security::{
        default_noise_prologue, DeviceKey, NoiseHandshake, NoiseTransport, SecurityError,
    },
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
