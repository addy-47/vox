//! Remote session manager — Tier 1 (pairing) and Tier 2 (transport) lifecycle.
//!
//! Strict separation per spec §4.4:
//! - `pair_device`: Tier 1 — ephemeral token handshake, writes DB, closes socket.
//! - `dial_and_connect`: Tier 2 — assumes device is already in paired_devices; dials,
//!   runs Noise handshake, exchanges Hello, spawns Link Supervisor.
//! - `spawn_link_supervisor`: Active duplex reader. The ONLY reader of recv_control()
//!   while a link is alive. Dispatches Ping/Pong, Disconnect, LinkLoss, and socket faults.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use parking_lot::RwLock;
use tokio::sync::{oneshot, RwLock as AsyncRwLock};
use tokio_util::sync::CancellationToken;

use super::state::{RemoteLinkState, RemoteStateDto};
pub(crate) use super::supervisor::sanitize_peer_display_name;
use crate::{
    core::error::PersistenceError,
    persistence::{paired_devices, VoxDb},
    services::remote::{
        control,
        link::{LinkCloseReason, LinkError, PeerRejection, RemoteLink},
        protocol::{
            errors::ProtocolError,
            framing::StreamFormat,
            messages::{
                AttachSnapshot, ControlMessage, ErrResponse, Hello, LinkLoss, LinkLossReason, Req,
                Res, SenderIdentity, VersionMismatch,
            },
            RemoteErrorCode, CAP_CONTROL_MESSAGES,
        },
        security::{AudioSessionTokenStore, TrustStore},
        PROTOCOL_VERSION,
    },
};

/// Type alias for state change subscriber callbacks.
pub type StateChangeCallback = Arc<dyn Fn(&RemoteLinkState, Option<&SenderIdentity>) + Send + Sync>;

/// Asynchronous RPC pending map keyed by request_id.
pub type PendingRpcMap = Arc<
    parking_lot::Mutex<
        std::collections::HashMap<u64, oneshot::Sender<Result<serde_json::Value, ErrResponse>>>,
    >,
>;

/// Role played in the remote session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    Host,
    Controller,
}

/// Trait for dispatching inbound IPC command requests on the host.
#[async_trait::async_trait]
pub trait CommandGateway: Send + Sync {
    /// Dispatches an inbound command request and produces a response message (`Res` or `Err`).
    async fn dispatch(&self, req: Req) -> ControlMessage;
}

/// Default host command gateway enforcing routing table constraints.
#[derive(Debug, Default)]
pub struct DefaultCommandGateway;

pub use super::forwarder::EventForwarderSender;
use super::{
    forwarder::run_event_forwarder,
    supervisor::{run_link_supervisor, LinkSupervisorParams},
};

/// Callback invoked on host session pause upon link severance.
pub type PauseSessionCallback = Arc<dyn Fn() + Send + Sync>;

/// Callback invoked when remote events or attach snapshots arrive on controller.
pub type RemoteEventCallback = Arc<dyn Fn(&str, serde_json::Value) + Send + Sync>;

/// A function-based command gateway adapter.
pub struct FnCommandGateway<F>(pub F);

/// Manages the remote control session lifecycle, Noise_XX handshake,
/// capabilities exchange, and link supervision.
pub struct RemoteSessionManager {
    pub(crate) role: SessionRole,
    pub(crate) state: Arc<RwLock<RemoteLinkState>>,
    pub(crate) peer_identity: Arc<RwLock<Option<SenderIdentity>>>,
    pub(crate) link: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
    pub(crate) audio_link: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
    pub(crate) active_host_addr: Arc<parking_lot::RwLock<Option<String>>>,
    pub(crate) cancel_token: Arc<parking_lot::Mutex<Option<CancellationToken>>>,
    pub(crate) on_state_change: Option<StateChangeCallback>,
    pub(crate) db: Arc<parking_lot::RwLock<Option<Arc<VoxDb>>>>,
    pub(crate) trust_store: Arc<parking_lot::RwLock<Option<Arc<TrustStore>>>>,
    pub(crate) pending_rpcs: PendingRpcMap,
    pub(crate) next_request_id: AtomicU64,
    pub(crate) gateway: Arc<parking_lot::RwLock<Arc<dyn CommandGateway>>>,
    pub(crate) audio_tokens: Arc<AudioSessionTokenStore>,
    pub(crate) last_snapshot: Arc<parking_lot::RwLock<Option<AttachSnapshot>>>,
    pub(crate) event_forwarder: Arc<parking_lot::Mutex<Option<EventForwarderSender>>>,
    pub(crate) pause_session_fn: parking_lot::Mutex<Option<PauseSessionCallback>>,
    pub(crate) on_remote_event: Arc<parking_lot::Mutex<Option<RemoteEventCallback>>>,
    pub(crate) current_pipeline_state: Arc<parking_lot::RwLock<String>>,
}

#[async_trait::async_trait]
impl CommandGateway for DefaultCommandGateway {
    async fn dispatch(&self, req: Req) -> ControlMessage {
        if req.command == "apply_settings_patch" {
            return ControlMessage::Res(Res {
                request_id: req.request_id,
                ok: true,
                payload: serde_json::json!({ "status": "acknowledged" }),
            });
        }
        let domain = req.args.get("domain").and_then(|v| v.as_str());
        let key = req.args.get("key").and_then(|v| v.as_str());
        let classification = control::classify_command(&req.command, domain, key);
        match classification {
            control::CommandClass::Local | control::CommandClass::Denied => {
                ControlMessage::Err(ErrResponse {
                    request_id: req.request_id,
                    code: RemoteErrorCode::Denied,
                    message: format!("Command '{}' is not permitted remotely", req.command),
                })
            }
            _ => ControlMessage::Res(Res {
                request_id: req.request_id,
                ok: true,
                payload: serde_json::json!({ "status": "acknowledged" }),
            }),
        }
    }
}

#[async_trait::async_trait]
impl<F, Fut> CommandGateway for FnCommandGateway<F>
where
    F: Fn(Req) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = ControlMessage> + Send,
{
    async fn dispatch(&self, req: Req) -> ControlMessage {
        (self.0)(req).await
    }
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
            audio_link: Arc::new(AsyncRwLock::new(None)),
            active_host_addr: Arc::new(parking_lot::RwLock::new(None)),
            cancel_token: Arc::new(parking_lot::Mutex::new(None)),
            on_state_change: None,
            db: Arc::new(parking_lot::RwLock::new(None)),
            trust_store: Arc::new(parking_lot::RwLock::new(None)),
            pending_rpcs: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            next_request_id: AtomicU64::new(1),
            gateway: Arc::new(parking_lot::RwLock::new(Arc::new(DefaultCommandGateway))),
            audio_tokens: Arc::new(AudioSessionTokenStore::new()),
            last_snapshot: Arc::new(parking_lot::RwLock::new(None)),
            event_forwarder: Arc::new(parking_lot::Mutex::new(None)),
            pause_session_fn: parking_lot::Mutex::new(None),
            on_remote_event: Arc::new(parking_lot::Mutex::new(None)),
            current_pipeline_state: Arc::new(parking_lot::RwLock::new("Idle".into())),
        }
    }

    /// Creates a new `RemoteSessionManager` with an attached database and state transition listener.
    pub fn with_callback(role: SessionRole, db: Arc<VoxDb>, callback: StateChangeCallback) -> Self {
        Self {
            role,
            state: Arc::new(RwLock::new(RemoteLinkState::Unpaired)),
            peer_identity: Arc::new(RwLock::new(None)),
            link: Arc::new(AsyncRwLock::new(None)),
            audio_link: Arc::new(AsyncRwLock::new(None)),
            active_host_addr: Arc::new(parking_lot::RwLock::new(None)),
            cancel_token: Arc::new(parking_lot::Mutex::new(None)),
            on_state_change: Some(callback),
            db: Arc::new(parking_lot::RwLock::new(Some(db))),
            trust_store: Arc::new(parking_lot::RwLock::new(None)),
            pending_rpcs: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            next_request_id: AtomicU64::new(1),
            gateway: Arc::new(parking_lot::RwLock::new(Arc::new(DefaultCommandGateway))),
            audio_tokens: Arc::new(AudioSessionTokenStore::new()),
            last_snapshot: Arc::new(parking_lot::RwLock::new(None)),
            event_forwarder: Arc::new(parking_lot::Mutex::new(None)),
            pause_session_fn: parking_lot::Mutex::new(None),
            on_remote_event: Arc::new(parking_lot::Mutex::new(None)),
            current_pipeline_state: Arc::new(parking_lot::RwLock::new("Idle".into())),
        }
    }

    /// Returns true if a voice pipeline session is currently active (not Idle).
    pub fn is_session_active(&self) -> bool {
        let st = self.current_pipeline_state.read().clone();
        st != "Idle" && st != "None" && !st.is_empty()
    }

    /// Attaches an event listener for receiving remote pipeline events and attach snapshots.
    pub fn set_on_remote_event(&self, callback: RemoteEventCallback) {
        *self.on_remote_event.lock() = Some(callback);
    }

    /// Attaches the trust store for cryptographic revocation and refusal tracking.
    pub fn set_trust_store(&self, trust_store: Arc<TrustStore>) {
        *self.trust_store.write() = Some(trust_store);
    }

    /// Attaches the shared database handle for supervisor revocation sync.
    pub fn set_db(&self, db: Arc<VoxDb>) {
        *self.db.write() = Some(db);
    }

    /// Attaches a custom command gateway for dispatching inbound remote commands.
    pub fn set_gateway(&self, gateway: Arc<dyn CommandGateway>) {
        *self.gateway.write() = gateway;
    }

    /// Attaches a pause-session callback invoked on host-side disconnects.
    pub fn set_pause_session_fn(&self, f: Arc<dyn Fn() + Send + Sync>) {
        *self.pause_session_fn.lock() = Some(f);
    }

    /// Returns the active command gateway.
    pub fn gateway(&self) -> Arc<dyn CommandGateway> {
        self.gateway.read().clone()
    }

    /// Returns a reference to the pending RPC map.
    pub fn pending_rpcs(&self) -> PendingRpcMap {
        Arc::clone(&self.pending_rpcs)
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
    pub async fn hydrate_from_db(&self, db: &VoxDb) {
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
        if matches!(
            new_state,
            RemoteLinkState::Unpaired | RemoteLinkState::Disabled
        ) {
            let mut guard = self.peer_identity.write();
            *guard = None;
            *self.active_host_addr.write() = None;
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
    pub async fn attach_link(
        &self,
        link: Arc<dyn RemoteLink>,
        peer: SenderIdentity,
        manager: Option<Arc<RemoteSessionManager>>,
    ) {
        {
            let mut guard = self.peer_identity.write();
            *guard = Some(peer);
        }
        {
            let mut guard = self.link.write().await;
            *guard = Some(link.clone());
        }
        // FIFO event forwarder (preserves wire order; coalesces high-freq events).
        let (tx, rx) = tokio::sync::mpsc::channel::<(String, serde_json::Value)>(256);
        *self.event_forwarder.lock() = Some(tx.clone());
        let link_slot = Arc::clone(&self.link);
        tokio::spawn(run_event_forwarder(rx, link_slot));
        // Host writes last_seen_at on connect (RULE-DB-04).
        if self.role == SessionRole::Host {
            let peer_id = self.peer_identity().map(|p| p.device_id);
            if let Some(id) = peer_id {
                let db_opt = self.db.read().clone();
                if let Some(db) = db_opt {
                    if let Ok(conn) = db.connect() {
                        let _ =
                            crate::persistence::paired_devices::mark_paired_device_seen(&conn, &id)
                                .await;
                    }
                }
            }
        }
        self.set_state(RemoteLinkState::Connected);
        self.spawn_link_supervisor(link, manager);
    }

    /// Returns a reference to the active link if one is open.
    pub async fn get_link(&self) -> Option<Arc<dyn RemoteLink>> {
        self.link.read().await.clone()
    }

    /// Attaches an active secondary audio link to the session.
    pub async fn attach_audio_link(&self, link: Arc<dyn RemoteLink>) {
        let mut guard = self.audio_link.write().await;
        *guard = Some(link);
    }

    /// Returns a reference to the active audio link if one is open.
    pub async fn get_audio_link(&self) -> Option<Arc<dyn RemoteLink>> {
        self.audio_link.read().await.clone()
    }

    /// Returns the shared audio link slot for sinks/readers.
    pub fn audio_link_slot(&self) -> Arc<tokio::sync::RwLock<Option<Arc<dyn RemoteLink>>>> {
        Arc::clone(&self.audio_link)
    }

    /// Returns a reference to the audio session token store.
    pub fn audio_tokens(&self) -> Arc<AudioSessionTokenStore> {
        Arc::clone(&self.audio_tokens)
    }

    /// Non-blocking event forwarder to the active controller over the control link.
    ///
    /// Spec §9.2 & §9.3: Telemetry is capped to <= 30 Hz (drop oldest), and LLM tokens
    /// are capped to <= 1 per 50 ms. Other events are forwarded immediately.
    pub fn forward_event(&self, event_name: &str, payload: serde_json::Value) {
        let tx = self.event_forwarder.lock().clone();
        if let Some(tx) = tx {
            let _ = tx.try_send((event_name.to_string(), payload));
        }
    }

    /// Returns the most recent AttachSnapshot received by this controller, if any.
    pub fn last_snapshot(&self) -> Option<AttachSnapshot> {
        self.last_snapshot.read().clone()
    }

    /// Stores an AttachSnapshot received on the controller.
    pub fn set_last_snapshot(&self, snapshot: AttachSnapshot) {
        let mut guard = self.last_snapshot.write();
        *guard = Some(snapshot);
    }

    /// Controller-side: sends an IPC command over the control link and awaits the correlated response.
    pub async fn call_remote(
        &self,
        command: impl Into<String>,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, LinkError> {
        let link = self
            .get_link()
            .await
            .ok_or(LinkError::Closed(LinkCloseReason::Local))?;

        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending_rpcs.lock().insert(request_id, tx);

        let req = ControlMessage::Req(Req {
            request_id,
            command: command.into(),
            args,
        });

        if let Err(e) = link.send_control(req).await {
            self.pending_rpcs.lock().remove(&request_id);
            return Err(e);
        }

        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(Ok(Ok(payload))) => Ok(payload),
            Ok(Ok(Err(err_resp))) => Err(LinkError::Transport(format!(
                "Remote command error ({:?}): {}",
                err_resp.code, err_resp.message
            ))),
            Ok(Err(_closed)) => {
                self.pending_rpcs.lock().remove(&request_id);
                Err(LinkError::Closed(LinkCloseReason::Peer))
            }
            Err(_timeout) => {
                self.pending_rpcs.lock().remove(&request_id);
                Err(LinkError::Timeout)
            }
        }
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
                Err(LinkError::PeerRejected(PeerRejection::VersionMismatch {
                    version: mismatch.version,
                    supported_version: mismatch.supported_version,
                }))
            }
            ControlMessage::UnknownPeer(peer) => Err(LinkError::PeerRejected(
                PeerRejection::UnknownPeer(peer.device_id),
            )),
            ControlMessage::RoleNotPermitted(role) => Err(LinkError::PeerRejected(
                PeerRejection::RoleNotPermitted(role.role),
            )),
            ControlMessage::LinkRefused(refused) => Err(LinkError::PeerRejected(
                PeerRejection::LinkRefused(refused.reason),
            )),
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
    fn spawn_link_supervisor(
        &self,
        link: Arc<dyn RemoteLink>,
        manager: Option<Arc<RemoteSessionManager>>,
    ) {
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
        let audio_link_slot = Arc::clone(&self.audio_link);
        let active_host_addr = Arc::clone(&self.active_host_addr);
        let db_opt = self.db.read().clone();
        let pending_rpcs = Arc::clone(&self.pending_rpcs);
        let gateway = self.gateway.read().clone();
        let last_snapshot = Arc::clone(&self.last_snapshot);
        let event_forwarder = Arc::clone(&self.event_forwarder);
        let pause_session_fn = parking_lot::Mutex::new(self.pause_session_fn.lock().clone());
        let on_remote_event = Arc::clone(&self.on_remote_event);
        let current_pipeline_state = Arc::clone(&self.current_pipeline_state);

        tokio::spawn(async move {
            run_link_supervisor(LinkSupervisorParams {
                manager,
                role,
                link,
                cancel,
                state: state_arc,
                peer_identity: peer_arc,
                on_change,
                link_slot,
                audio_link_slot,
                active_host_addr,
                db_opt,
                pending_rpcs,
                gateway,
                last_snapshot,
                event_forwarder,
                pause_session_fn,
                on_remote_event,
                current_pipeline_state,
            })
            .await;
        });
    }

    // ─── Ticket 02: Tier 1 — pair_device ────────────────────────────────────

    /// Controller-side: dial the host, perform the Tier 1 pairing handshake,
    /// write the host into `paired_devices`, close the pairing socket, and
    /// transition `RemoteLinkState` to `Paired`.
    pub async fn pair_device(
        self: Arc<Self>,
        db: Arc<VoxDb>,
        address: String,
        token: String,
        host_public_key: String,
        host_device_id: String,
        on_prompting: Option<Box<dyn FnOnce() + Send>>,
    ) -> Result<(), LinkError> {
        super::pairing_client::pair_device(
            self,
            db,
            address,
            token,
            host_public_key,
            host_device_id,
            on_prompting,
        )
        .await
    }

    // ─── Ticket 02: Tier 2 — dial_and_connect ───────────────────────────────

    /// Controller-side: dial a paired host, run Noise handshake, exchange
    /// Hello, and spawn the Link Supervisor. Assumes device is already in
    /// `paired_devices`. Does NOT perform pairing; call `pair_device` first.
    pub async fn dial_and_connect(
        self: Arc<Self>,
        db: Arc<VoxDb>,
        device_id: String,
        address: Option<String>,
    ) -> Result<(), LinkError> {
        super::dialer::dial_and_connect(self, db, device_id, address).await
    }

    // ─── Ticket 04: Revocation & Disconnect ─────────────────────────────────

    /// Lists all paired devices from the Turso database.
    pub async fn list_paired_devices(
        db: &VoxDb,
    ) -> Result<Vec<paired_devices::PairedDevice>, PersistenceError> {
        let conn = db.connect()?;
        paired_devices::list_paired_devices(&conn).await
    }

    async fn sever_active_link(&self, device_id: &str, has_active_link: bool) {
        let is_current = self
            .peer_identity()
            .map(|p| p.device_id == device_id)
            .unwrap_or(false);

        if has_active_link && is_current {
            if let Some(audio) = self.audio_link.write().await.take() {
                let _ = audio.close(LinkCloseReason::Local).await;
            }
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
            *self.active_host_addr.write() = None;
        }
    }

    async fn notify_idle_host_revoked(&self, conn: &turso::Connection, device_id: &str) {
        if self.role != SessionRole::Controller {
            return;
        }
        let device = paired_devices::get_paired_device(conn, device_id)
            .await
            .ok()
            .flatten();
        let Some(device) = device else {
            return;
        };

        tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_secs(4), async move {
                if let Ok((ws, _identity, _candidate)) =
                    super::dialer::perform_dial_handshake(&device, None).await
                {
                    let _ = ws
                        .send_control(ControlMessage::LinkLoss(LinkLoss {
                            reason: LinkLossReason::Revoked,
                        }))
                        .await;
                    let _ = ws.close(LinkCloseReason::Local).await;
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
        db: &VoxDb,
        device_id: &str,
    ) -> Result<bool, PersistenceError> {
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
            paired_devices::delete_paired_device(&conn, device_id).await?
        };

        // Determine new state from remaining count (spec §5.8)
        let remaining = paired_devices::list_paired_devices(&conn)
            .await
            .map(|v| v.len())
            .unwrap_or(0);
        *self.event_forwarder.lock() = None;
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
        self.pending_rpcs.lock().clear();

        if let Some(audio) = self.audio_link.write().await.take() {
            let _ = audio.close(LinkCloseReason::Local).await;
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
        if let Some(cb) = self.pause_session_fn.lock().clone() {
            cb();
        }
        let peer_id = self.peer_identity().map(|p| p.device_id);
        if let Some(id) = peer_id {
            let db_opt = self.db.read().clone();
            if let Some(db) = db_opt {
                if let Ok(conn) = db.connect() {
                    let _ = crate::persistence::paired_devices::mark_paired_device_seen(&conn, &id)
                        .await;
                }
            }
        }
        *self.event_forwarder.lock() = None;
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
        self.pending_rpcs.lock().clear();

        if let Some(audio) = self.audio_link.write().await.take() {
            let _ = audio.close(LinkCloseReason::Local).await;
        }

        let link_opt = self.link.write().await.take();
        if let Some(link) = link_opt {
            let _ = link.send_control(ControlMessage::Disconnect).await;
            let _ = link.close(LinkCloseReason::Local).await;
        }

        // Keep pairing: transition to Paired (spec §6.4)
        // peer_identity is retained because the device remains in paired_devices DB.
        *self.event_forwarder.lock() = None;
        if self.is_session_active() {
            if let Some(cb) = self.on_remote_event.lock().clone() {
                cb(
                    "state_changed",
                    serde_json::json!({
                        "owner": "Assistant",
                        "state": "Paused",
                        "turn_id": 0,
                    }),
                );
            }
            if let Some(cb) = self.pause_session_fn.lock().clone() {
                cb();
            }
        }
        self.set_state(RemoteLinkState::Paired);
        Ok(())
    }
}

pub(crate) use super::supervisor::host_display_name;
