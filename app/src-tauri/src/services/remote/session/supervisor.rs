//! Duplex link supervisor managing ping/pong heartbeats, socket faults, and auto-reconnect backoff.

use std::{sync::Arc, time::Instant};

use parking_lot::RwLock;
use tokio::sync::RwLock as AsyncRwLock;
use tokio_util::sync::CancellationToken;

use super::{
    forwarder::EventForwarderSender,
    manager::{
        CommandGateway, PauseSessionCallback, PendingRpcMap, RemoteEventCallback,
        RemoteSessionManager, SessionRole, StateChangeCallback,
    },
    state::{RemoteFailureKind, RemoteLinkState},
};
use crate::{
    persistence::{paired_devices, VoxDb},
    services::remote::{
        link::{LinkCloseReason, LinkError, RemoteLink},
        protocol::messages::{
            AttachSnapshot, ControlMessage, ErrResponse, LinkLoss, LinkLossReason, Ping, Pong, Req,
            Res, SenderIdentity, SessionToken,
        },
        HEARTBEAT_INTERVAL, HEARTBEAT_MAX_MISSED_PONGS, HEARTBEAT_PING_TIMEOUT,
        RECONNECT_INITIAL_BACKOFF, RECONNECT_MAX_ATTEMPTS, RECONNECT_MAX_BACKOFF,
    },
};

/// Control flow decision for duplex supervisor message handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupervisorFlow {
    Continue,
    Break,
}

/// Parameters for supervising link health via the full duplex recv_control loop.
pub(crate) struct LinkSupervisorParams {
    pub(crate) manager: Option<Arc<RemoteSessionManager>>,
    pub(crate) role: SessionRole,
    pub(crate) link: Arc<dyn RemoteLink>,
    pub(crate) cancel: CancellationToken,
    pub(crate) state: Arc<RwLock<RemoteLinkState>>,
    pub(crate) peer_identity: Arc<RwLock<Option<SenderIdentity>>>,
    pub(crate) on_change: Option<StateChangeCallback>,
    pub(crate) link_slot: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
    pub(crate) audio_link_slot: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
    pub(crate) active_host_addr: Arc<parking_lot::RwLock<Option<String>>>,
    pub(crate) db_opt: Option<Arc<VoxDb>>,
    pub(crate) pending_rpcs: PendingRpcMap,
    pub(crate) gateway: Arc<dyn CommandGateway>,
    pub(crate) last_snapshot: Arc<parking_lot::RwLock<Option<AttachSnapshot>>>,
    pub(crate) event_forwarder: Arc<parking_lot::Mutex<Option<EventForwarderSender>>>,
    pub(crate) pause_session_fn: parking_lot::Mutex<Option<PauseSessionCallback>>,
    pub(crate) on_remote_event: Arc<parking_lot::Mutex<Option<RemoteEventCallback>>>,
    pub(crate) current_pipeline_state: Arc<parking_lot::RwLock<String>>,
}

fn handle_supervisor_rpc_res(params: &LinkSupervisorParams, res: Res) {
    if let Some(tx) = params.pending_rpcs.lock().remove(&res.request_id) {
        let _ = tx.send(Ok(res.payload));
    }
}

fn handle_supervisor_rpc_err(params: &LinkSupervisorParams, err: ErrResponse) {
    if let Some(tx) = params.pending_rpcs.lock().remove(&err.request_id) {
        let _ = tx.send(Err(err));
    }
}

fn handle_supervisor_rpc_req(params: &LinkSupervisorParams, req: Req) {
    let link = Arc::clone(&params.link);
    let gateway = params.gateway.clone();
    tokio::spawn(async move {
        let reply = gateway.dispatch(req).await;
        let _ = link.send_control(reply).await;
    });
}

fn handle_supervisor_session_token(params: &LinkSupervisorParams, token: SessionToken) {
    if params.role != SessionRole::Controller {
        return;
    }
    log::info!("[Remote Supervisor] Controller received SessionToken: dialing audio link");
    let audio_slot = Arc::clone(&params.audio_link_slot);
    let db_opt = params.db_opt.clone();
    let peer_id = params
        .peer_identity
        .read()
        .as_ref()
        .map(|p| p.device_id.clone());
    let host_addr = params.active_host_addr.read().clone();
    let cancel = params.cancel.clone();
    tokio::spawn(async move {
        dial_secondary_audio_socket(audio_slot, db_opt, peer_id, host_addr, token, cancel).await;
    });
}

async fn dial_secondary_audio_socket(
    audio_slot: Arc<AsyncRwLock<Option<Arc<dyn RemoteLink>>>>,
    db_opt: Option<Arc<VoxDb>>,
    peer_id: Option<String>,
    host_addr: Option<String>,
    token: SessionToken,
    cancel: CancellationToken,
) {
    let (Some(id), Some(addr), Some(db)) = (peer_id, host_addr, db_opt) else {
        return;
    };
    let Ok(conn) = db.connect() else {
        return;
    };
    let Ok(Some(device)) = paired_devices::get_paired_device(&conn, &id).await else {
        return;
    };
    match super::dialer::dial_link(&addr, &device).await {
        Ok((ws_link, _)) => {
            let msg = ControlMessage::SessionToken(token);
            if let Err(e) = ws_link.send_control(msg).await {
                log::warn!("[Remote] Audio link send SessionToken failed: {e}");
                let _ = ws_link.close(LinkCloseReason::Local).await;
            } else {
                log::info!("[Remote] Controller audio link established");
                let link_arc: Arc<dyn RemoteLink> = Arc::new(ws_link);
                *audio_slot.write().await = Some(Arc::clone(&link_arc));

                let vad_settings = crate::config::VoxSettings::load().vad;
                if let Err(e) = crate::services::remote::audio::spawn_controller_audio_runtime(
                    link_arc,
                    cancel,
                    vad_settings,
                ) {
                    log::error!("[Remote] Failed to spawn controller audio runtime: {e}");
                }
            }
        }
        Err(e) => {
            log::warn!("[Remote] Controller audio link dial failed: {e}");
        }
    }
}

async fn handle_host_involuntary_cutoff(params: &LinkSupervisorParams) {
    if params.role != SessionRole::Host {
        return;
    }
    if let Some(cb) = params.pause_session_fn.lock().clone() {
        cb();
    }
    let peer_id = params
        .peer_identity
        .read()
        .as_ref()
        .map(|p| p.device_id.clone());
    if let (Some(id), Some(db)) = (peer_id, &params.db_opt) {
        if let Ok(conn) = db.connect() {
            let _ = paired_devices::mark_paired_device_seen(&conn, &id).await;
        }
    }
}

fn handle_controller_involuntary_cutoff(params: &LinkSupervisorParams) {
    if params.role != SessionRole::Controller {
        return;
    }
    let st = params.current_pipeline_state.read().clone();
    let is_active = st != "Idle" && st != "None" && !st.is_empty();
    if is_active {
        if let Some(cb) = params.on_remote_event.lock().clone() {
            cb(
                "state_changed",
                serde_json::json!({
                    "owner": "Assistant",
                    "state": "Paused",
                    "turn_id": 0,
                }),
            );
        }
        if let Some(cb) = params.pause_session_fn.lock().clone() {
            cb();
        }
    }
}

async fn handle_supervisor_disconnect(params: &LinkSupervisorParams) -> SupervisorFlow {
    log::info!("[Remote Supervisor] Received Disconnect from controller — transitioning to Paired");
    params.pending_rpcs.lock().clear();
    *params.event_forwarder.lock() = None;
    let _ = params.link.close(LinkCloseReason::Local).await;
    params.link_slot.write().await.take();

    // Spec §6.4: host pauses the session and writes last_seen_at.
    handle_host_involuntary_cutoff(params).await;

    transition_supervisor_state(
        &params.state,
        &params.peer_identity,
        &params.on_change,
        RemoteLinkState::Paired,
    );
    SupervisorFlow::Break
}

async fn handle_supervisor_link_loss_msg(
    params: &LinkSupervisorParams,
    loss: LinkLoss,
) -> SupervisorFlow {
    log::info!("[Remote Supervisor] Received LinkLoss({:?})", loss.reason);
    params.pending_rpcs.lock().clear();
    *params.event_forwarder.lock() = None;
    let _ = params.link.close(LinkCloseReason::Local).await;
    params.link_slot.write().await.take();
    handle_controller_involuntary_cutoff(params);
    let new_state =
        handle_supervisor_link_loss(&params.peer_identity, &params.db_opt, loss.reason).await;
    transition_supervisor_state(
        &params.state,
        &params.peer_identity,
        &params.on_change,
        new_state,
    );
    SupervisorFlow::Break
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
            let _ = params
                .link
                .send_control(ControlMessage::Pong(Pong { seq: p.seq }))
                .await;
            SupervisorFlow::Continue
        }
        ControlMessage::Pong(p) => {
            if p.seq == seq {
                *missed_pongs = 0;
            }
            SupervisorFlow::Continue
        }
        ControlMessage::Res(res) => {
            handle_supervisor_rpc_res(params, res);
            SupervisorFlow::Continue
        }
        ControlMessage::Err(err) => {
            handle_supervisor_rpc_err(params, err);
            SupervisorFlow::Continue
        }
        ControlMessage::Req(req) if params.role == SessionRole::Host => {
            handle_supervisor_rpc_req(params, req);
            SupervisorFlow::Continue
        }
        ControlMessage::SessionToken(token) => {
            handle_supervisor_session_token(params, token);
            SupervisorFlow::Continue
        }
        ControlMessage::Event(ev) => {
            if ev.event_name == "state_changed" {
                if let Some(st) = ev.payload.get("state").and_then(|v| v.as_str()) {
                    *params.current_pipeline_state.write() = st.to_string();
                }
                if let Some(snap) = params.last_snapshot.write().as_mut() {
                    snap.state = ev.payload.clone();
                }
            }
            if let Some(cb) = params.on_remote_event.lock().clone() {
                cb(&ev.event_name, ev.payload);
            }
            SupervisorFlow::Continue
        }
        ControlMessage::AttachSnapshot(snapshot) => {
            log::info!(
                "[Remote Supervisor] Controller received AttachSnapshot: session_id={:?}",
                snapshot.session_id
            );
            if let Some(st) = snapshot.state.get("state").and_then(|v| v.as_str()) {
                *params.current_pipeline_state.write() = st.to_string();
            }
            params.last_snapshot.write().replace(snapshot.clone());
            if let Some(cb) = params.on_remote_event.lock().clone() {
                cb("state_changed", snapshot.state.clone());
                cb(
                    "remote_attach_snapshot",
                    serde_json::json!({
                        "sessionId": snapshot.session_id,
                        "turns": snapshot.turns,
                        "settings": snapshot.settings,
                    }),
                );
            }
            SupervisorFlow::Continue
        }
        ControlMessage::Disconnect => handle_supervisor_disconnect(params).await,
        ControlMessage::LinkLoss(loss) => handle_supervisor_link_loss_msg(params, loss).await,
        _ => SupervisorFlow::Continue,
    }
}

async fn handle_supervisor_link_loss(
    peer_identity: &Arc<RwLock<Option<SenderIdentity>>>,
    db_opt: &Option<Arc<VoxDb>>,
    reason: LinkLossReason,
) -> RemoteLinkState {
    if reason != LinkLossReason::Revoked {
        return RemoteLinkState::Paired;
    }
    log::warn!("[Remote Supervisor] Link revoked by peer — deleting from DB");
    let peer_id = peer_identity.read().as_ref().map(|p| p.device_id.clone());
    if let (Some(id), Some(db)) = (peer_id, db_opt) {
        if let Ok(conn) = db.connect() {
            let _ = paired_devices::delete_paired_device(&conn, &id).await;
            let remaining = paired_devices::list_paired_devices(&conn)
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
    params.pending_rpcs.lock().clear();
    *params.event_forwarder.lock() = None;
    params.link_slot.write().await.take();
    if params.cancel.is_cancelled() {
        log::debug!("[Remote Supervisor] Cancelled during shutdown — ignoring socket fault");
        return;
    }

    let is_still_paired = if let Some(db) = &params.db_opt {
        let peer_id = params
            .peer_identity
            .read()
            .as_ref()
            .map(|p| p.device_id.clone());
        if let (Some(id), Ok(conn)) = (peer_id, db.connect()) {
            paired_devices::get_paired_device(&conn, &id)
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

    handle_host_involuntary_cutoff(params).await;
    handle_controller_involuntary_cutoff(params);

    let reconnect = params.role == SessionRole::Controller && is_still_paired;
    let new_state = if !is_still_paired {
        log::warn!(
            "[Remote Supervisor] Peer is no longer in paired_devices — transitioning to Unpaired"
        );
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
    if reconnect {
        spawn_controller_reconnect(params);
    }
}

async fn handle_host_heartbeat_tick(
    params: &LinkSupervisorParams,
    seq: &mut u64,
    missed_pongs: &mut u32,
) -> SupervisorFlow {
    *seq = seq.wrapping_add(1);
    let _ = params
        .link
        .send_control(ControlMessage::Ping(Ping { seq: *seq }))
        .await;
    *missed_pongs += 1;

    if *missed_pongs >= HEARTBEAT_MAX_MISSED_PONGS {
        log::warn!(
            "[Remote Supervisor] Host: {} missed pongs — link dead, transitioning to Paired",
            *missed_pongs
        );
        params.pending_rpcs.lock().clear();
        let _ = params
            .link
            .close(LinkCloseReason::Transport(
                "Heartbeat: 3 missed pongs".into(),
            ))
            .await;
        params.link_slot.write().await.take();

        // Host-side involuntary cutoff: pause session and update last_seen_at (Bug 10a).
        handle_host_involuntary_cutoff(params).await;

        transition_supervisor_state(
            &params.state,
            &params.peer_identity,
            &params.on_change,
            RemoteLinkState::Paired,
        );
        SupervisorFlow::Break
    } else {
        SupervisorFlow::Continue
    }
}

async fn handle_controller_heartbeat_timeout(params: &LinkSupervisorParams) -> SupervisorFlow {
    log::warn!(
        "[Remote Supervisor] Controller: no Ping for {:?} — transitioning to Reconnecting",
        HEARTBEAT_PING_TIMEOUT
    );
    params.pending_rpcs.lock().clear();
    *params.event_forwarder.lock() = None;
    let _ = params
        .link
        .close(LinkCloseReason::Transport(
            "Heartbeat: no ping received".into(),
        ))
        .await;
    params.link_slot.write().await.take();
    handle_controller_involuntary_cutoff(params);
    transition_supervisor_state(
        &params.state,
        &params.peer_identity,
        &params.on_change,
        RemoteLinkState::Reconnecting,
    );

    // Spawn reconnect loop (exponential backoff).
    spawn_controller_reconnect(params);
    SupervisorFlow::Break
}

fn spawn_controller_reconnect(params: &LinkSupervisorParams) {
    let Some(manager) = params.manager.clone() else {
        return;
    };
    let db_opt = params.db_opt.clone();
    let peer_id = params
        .peer_identity
        .read()
        .as_ref()
        .map(|p| p.device_id.clone());
    let on_change = params.on_change.clone();
    tokio::spawn(async move {
        let Some(id) = peer_id else { return };
        let Some(db) = db_opt else { return };
        let mut delay = RECONNECT_INITIAL_BACKOFF;
        for _ in 0..RECONNECT_MAX_ATTEMPTS {
            tokio::time::sleep(delay).await;
            if manager.state() != RemoteLinkState::Reconnecting {
                return;
            }
            match manager
                .clone()
                .dial_and_connect(db.clone(), id.clone(), None)
                .await
            {
                Ok(()) => return,
                Err(_) => {
                    if manager.state() != RemoteLinkState::Reconnecting {
                        return;
                    }
                }
            }
            delay = (delay * 2).min(RECONNECT_MAX_BACKOFF);
        }
        // max attempts exceeded
        if manager.state() == RemoteLinkState::Reconnecting {
            manager.set_state(RemoteLinkState::failed(
                "Reconnect failed after maximum attempts".to_string(),
                RemoteFailureKind::Timeout,
            ));
        }
        let _ = on_change;
    });
}

pub(crate) async fn run_link_supervisor(params: LinkSupervisorParams) {
    let mut seq: u64 = 0;
    let mut missed_pongs: u32 = 0;
    let mut last_ping_at = Instant::now();

    let mut ping_timer = tokio::time::interval(HEARTBEAT_INTERVAL);
    ping_timer.tick().await;

    loop {
        tokio::select! {
            _ = params.cancel.cancelled() => {
                log::debug!("[Remote Supervisor] cancelled — exiting");
                params.pending_rpcs.lock().clear();
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
    if matches!(
        new_state,
        RemoteLinkState::Unpaired | RemoteLinkState::Disabled
    ) {
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
pub(crate) fn classify_dial_error(err: &str) -> RemoteFailureKind {
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
