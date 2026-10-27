//! Host-side inbound connection listener.
//!
//! Accepts incoming controller sockets. Two paths:
//! - `PairRequest` (Tier 1): validates token, prompts user, writes DB, closes socket.
//! - `Hello` first message (legacy path): REMOVED per spec §4.4 ZBC. Tier 2 connections
//!   from a reconnecting controller arrive via explicit `connect_host` IPC, not by
//!   sending a bare `Hello` on a new socket.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

use tauri::{AppHandle, Runtime};

use crate::{
    core::{
        events::{emit_ipc, IpcEvent, RemotePairingPromptPayload},
        state::{AppState, PendingPairingState},
    },
    persistence::paired_devices::{
        insert_paired_device, NewPairedDevice, PairedDeviceRole,
    },
    services::remote::{
        link::{
            ws::{bind, WsLink, WsListener},
            LinkCloseReason, RemoteLink,
        },
        protocol::{
            framing::StreamFormat,
            messages::{
                ControlMessage, Hello, LinkRefused, LinkRefusedReason, PairApproved, PairDenied,
                PairRequest, RoleNotPermitted, SenderIdentity, UnknownPeer,
            },
        },
        security::{
            default_noise_prologue, derive_device_id, DeviceId, DeviceKey, KeyStore, NoiseHandshake,
            PairingOutcome, PairingRequest, TrustDecision, TrustRefusal,
        },
        session::state::{PairingState, RemoteLinkState},
        PAIRING_APPROVAL_TIMEOUT,
    },
};

/// Spawns the background host listener loop accepting incoming controller connections.
pub fn spawn_host_listener<R: Runtime>(app_handle: AppHandle<R>, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        run_host_listener_loop(app_handle, state).await;
    });
}

async fn run_host_listener_loop<R: Runtime>(app_handle: AppHandle<R>, state: Arc<AppState>) {
    let bind_addr: SocketAddr = match "0.0.0.0:7700".parse() {
        Ok(a) => a,
        Err(e) => {
            log::error!("[Remote] Invalid host bind address 0.0.0.0:7700: {e}");
            return;
        }
    };

    loop {
        let accepts = state
            .settings
            .read()
            .map(|s| s.remote.accepts_controllers)
            .unwrap_or(false);

        if !accepts {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            continue;
        }

        let listener = match bind(bind_addr).await {
            Ok(l) => {
                log::info!("[Remote] Inbound host listener bound on {bind_addr}");
                l
            }
            Err(e) => {
                log::warn!("[Remote] Failed to bind host listener on {bind_addr}: {e}");
                tokio::time::sleep(Duration::from_millis(2000)).await;
                continue;
            }
        };

        accept_controller_connections(&listener, &app_handle, &state).await;
    }
}

async fn accept_controller_connections<R: Runtime>(
    listener: &WsListener,
    app_handle: &AppHandle<R>,
    state: &Arc<AppState>,
) {
    while state
        .settings
        .read()
        .map(|s| s.remote.accepts_controllers)
        .unwrap_or(false)
    {
        let accept_fut = listener.accept();
        tokio::pin!(accept_fut);

        let ws_link = loop {
            tokio::select! {
                res = &mut accept_fut => match res {
                    Ok(link) => break Some(link),
                    Err(e) => {
                        log::warn!("[Remote] Accept error: {e}");
                        break None;
                    }
                },
                _ = tokio::time::sleep(Duration::from_millis(1000)) => {
                    let still_accepts = state
                        .settings
                        .read()
                        .map(|s| s.remote.accepts_controllers)
                        .unwrap_or(false);
                    if !still_accepts {
                        break None;
                    }
                }
            }
        };

        let link = match ws_link {
            Some(l) => l,
            None => break,
        };

        let state_arc = Arc::clone(state);
        let app_handle_clone = app_handle.clone();
        tokio::spawn(async move {
            handle_inbound_connection(link, state_arc, app_handle_clone).await;
        });
    }
}

async fn handle_inbound_connection<R: Runtime>(
    ws_link: WsLink,
    state: Arc<AppState>,
    app_handle: AppHandle<R>,
) {
    let initial_bytes = match tokio::time::timeout(
        Duration::from_secs(10),
        ws_link.recv_handshake_message(),
    )
    .await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => {
            log::warn!("[Remote] Failed to receive initial message: {e}");
            return;
        }
        Err(_) => {
            log::warn!("[Remote] Initial message timed out");
            return;
        }
    };

    if let Ok(crate::services::remote::protocol::messages::DecodeOutcome::Known(
        ControlMessage::PairRequest(claimed_req),
    )) = crate::services::remote::protocol::messages::decode_control(&initial_bytes)
    {
        // Tier 1 path: pairing request claim, followed by Noise_XX, user prompt, and DB storage
        handle_inbound_pairing(ws_link, claimed_req, state, app_handle).await;
    } else {
        // Tier 2 path: initial_bytes is Noise_XX message 1 from the connecting controller
        handle_inbound_tier2(ws_link, initial_bytes, state).await;
    }
}

async fn drive_tier2_handshake(
    ws_link: &WsLink,
    first_msg: &[u8],
    device_key: &DeviceKey,
) -> Option<[u8; 32]> {
    let prologue = default_noise_prologue();
    let mut handshake = match NoiseHandshake::responder(device_key, None, &prologue) {
        Ok(h) => h,
        Err(e) => {
            log::warn!("[Remote] Could not initialize Noise responder: {e}");
            return None;
        }
    };

    if let Err(e) = handshake.read_message(first_msg) {
        log::warn!("[Remote] Failed to read initial Noise handshake message: {e}");
        return None;
    }

    let transport = match ws_link.drive_handshake(handshake).await {
        Ok(t) => t,
        Err(e) => {
            log::warn!("[Remote] Tier 2 Noise handshake failed: {e}");
            return None;
        }
    };

    let handshake_remote_static = *transport.remote_static();
    ws_link.set_cipher(transport).await;
    Some(handshake_remote_static)
}

async fn check_host_admission(
    state: &AppState,
    peer_id: &str,
    handshake_remote_static: &[u8; 32],
    ws_link: &WsLink,
) -> Option<()> {
    let accepts = state
        .settings
        .read()
        .map(|s| s.remote.accepts_controllers)
        .unwrap_or(false);
    if !accepts {
        log::warn!("[Remote] Inbound Hello refused: remote.accepts_controllers is false");
        let _ = ws_link
            .send_control(ControlMessage::LinkRefused(LinkRefused {
                reason: LinkRefusedReason::NotAccepting,
            }))
            .await;
        return None;
    }

    if state.remote_manager.state() == RemoteLinkState::Connected {
        log::warn!("[Remote] Inbound Hello refused: another controller is already connected");
        let _ = ws_link
            .send_control(ControlMessage::LinkRefused(LinkRefused {
                reason: LinkRefusedReason::ControllerAttached,
            }))
            .await;
        return None;
    }

    if state.pipeline.turn_open.load(Ordering::Relaxed) {
        log::warn!("[Remote] Inbound Hello refused: host is currently busy with local turn");
        let _ = ws_link
            .send_control(ControlMessage::LinkRefused(LinkRefused {
                reason: LinkRefusedReason::HostBusy,
            }))
            .await;
        return None;
    }

    let conn = match state.db.connect() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[Remote] Database connection failed: {e}");
            return None;
        }
    };

    let decision = match state
        .trust_store
        .lookup(&conn, peer_id, PairedDeviceRole::Controller)
        .await
    {
        Ok(d) => d,
        Err(e) => {
            log::warn!("[Remote] Error looking up peer {peer_id}: {e}");
            return None;
        }
    };

    let device = match decision {
        TrustDecision::Trusted(dev) => dev,
        TrustDecision::Refused(TrustRefusal::RoleMismatch { .. }) => {
            log::warn!("[Remote] Refusing peer {peer_id}: role mismatch");
            let _ = ws_link
                .send_control(ControlMessage::RoleNotPermitted(RoleNotPermitted {
                    role: "Controller".into(),
                }))
                .await;
            return None;
        }
        TrustDecision::Refused(refusal) => {
            log::warn!("[Remote] Refusing peer {peer_id}: {refusal:?}");
            let _ = ws_link
                .send_control(ControlMessage::UnknownPeer(UnknownPeer {
                    device_id: peer_id.to_string(),
                }))
                .await;
            return None;
        }
    };

    if device.public_key.as_slice() != handshake_remote_static.as_slice() {
        log::warn!(
            "[Remote] Refusing peer {peer_id}: handshake public key does not match stored key"
        );
        let _ = ws_link
            .send_control(ControlMessage::UnknownPeer(UnknownPeer {
                device_id: peer_id.to_string(),
            }))
            .await;
        return None;
    }

    Some(())
}

async fn respond_hello_and_attach(
    ws_link: WsLink,
    device_key: &DeviceKey,
    peer_identity: SenderIdentity,
    state: &AppState,
) {
    let host_id = derive_device_id(device_key.public_key());
    let host_name = sysinfo::System::host_name().unwrap_or_else(|| "Vox Host".to_string());
    let local_identity = SenderIdentity {
        device_id: host_id.as_str().to_string(),
        display_name: host_name,
    };

    let hello_msg = Hello {
        version: crate::services::remote::PROTOCOL_VERSION,
        sender_identity: local_identity,
        capabilities: crate::services::remote::protocol::capabilities::CAP_CONTROL_MESSAGES,
        uplink_format: StreamFormat::DOWNLINK,
        downlink_format: StreamFormat::UPLINK,
    };

    if ws_link
        .send_control(ControlMessage::Hello(hello_msg))
        .await
        .is_err()
    {
        return;
    }

    state
        .remote_manager
        .attach_link(Arc::new(ws_link), peer_identity)
        .await;
}

async fn receive_inbound_hello(
    ws_link: &WsLink,
    handshake_remote_static: &[u8; 32],
    state: &AppState,
) -> Option<Hello> {
    let initial_msg =
        match tokio::time::timeout(Duration::from_secs(5), ws_link.recv_control()).await {
            Ok(Ok(m)) => m,
            Ok(Err(e)) => {
                log::warn!("[Remote] Failed to receive encrypted Hello: {e}");
                return None;
            }
            Err(_) => {
                log::warn!("[Remote] Encrypted Hello timed out");
                return None;
            }
        };

    match initial_msg {
        ControlMessage::Hello(h) => Some(h),
        ControlMessage::LinkLoss(loss)
            if loss.reason
                == crate::services::remote::protocol::messages::LinkLossReason::Revoked =>
        {
            let peer_id = derive_device_id(handshake_remote_static).as_str().to_string();
            log::info!("[Remote] Received idle LinkLoss(Revoked) from peer {peer_id}");
            if let Ok(conn) = state.db.connect() {
                let _ = state.trust_store.revoke(&conn, &peer_id).await;
                state.remote_manager.hydrate_from_db(&state.db).await;
            }
            let _ = ws_link.close(LinkCloseReason::Peer).await;
            None
        }
        other => {
            log::warn!("[Remote] Expected Hello, got {:?}", other.message_type());
            None
        }
    }
}

async fn handle_inbound_tier2(ws_link: WsLink, first_msg: Vec<u8>, state: Arc<AppState>) {
    let device_key = match KeyStore::default_path().and_then(|ks| ks.load_or_create()) {
        Ok(k) => k,
        Err(e) => {
            log::warn!("[Remote] Could not load host key: {e}");
            return;
        }
    };

    let Some(handshake_remote_static) =
        drive_tier2_handshake(&ws_link, &first_msg, &device_key).await
    else {
        return;
    };

    let Some(peer_hello) =
        receive_inbound_hello(&ws_link, &handshake_remote_static, &state).await
    else {
        return;
    };

    let peer_id = &peer_hello.sender_identity.device_id;
    if check_host_admission(&state, peer_id, &handshake_remote_static, &ws_link)
        .await
        .is_none()
    {
        return;
    }

    respond_hello_and_attach(ws_link, &device_key, peer_hello.sender_identity, &state).await;
}

async fn drive_tier1_handshake(ws_link: &WsLink, device_key: &DeviceKey) -> Option<[u8; 32]> {
    let prologue = default_noise_prologue();
    let handshake = match NoiseHandshake::responder(device_key, None, &prologue) {
        Ok(h) => h,
        Err(e) => {
            log::warn!("[Remote] Could not initialize Noise responder for pairing: {e}");
            return None;
        }
    };

    let transport = match ws_link.drive_handshake(handshake).await {
        Ok(t) => t,
        Err(e) => {
            log::warn!("[Remote] Tier 1 Noise handshake failed: {e}");
            return None;
        }
    };

    let handshake_static = *transport.remote_static();
    ws_link.set_cipher(transport).await;
    Some(handshake_static)
}

async fn settle_pairing_coordinator<R: Runtime>(
    ws_link: WsLink,
    pending: crate::services::remote::security::pairing::PendingPairing,
    claimed_req: PairRequest,
    handshake_static: [u8; 32],
    host_id: DeviceId,
    state: Arc<AppState>,
    app_handle: AppHandle<R>,
) {
    match pending.settle().await {
        PairingOutcome::Approved { peer: _ } => {
            complete_pairing(
                ws_link,
                claimed_req,
                handshake_static.to_vec(),
                host_id,
                state,
                app_handle,
            )
            .await;
        }
        PairingOutcome::Rejected | PairingOutcome::TimedOut => {
            let _ = ws_link
                .send_control(ControlMessage::PairDenied(PairDenied {
                    reason: "Rejected or timed out".into(),
                }))
                .await;

            let _ = emit_ipc(
                &app_handle,
                IpcEvent::RemotePairingStateChanged(
                    PairingState::Denied {
                        reason: "Rejected or timed out".into(),
                    }
                    .to_dto(),
                ),
            );
        }
    }
}

async fn handle_inbound_pairing<R: Runtime>(
    ws_link: WsLink,
    claimed_req: PairRequest,
    state: Arc<AppState>,
    app_handle: AppHandle<R>,
) {
    let device_key = match KeyStore::default_path().and_then(|ks| ks.load_or_create()) {
        Ok(k) => k,
        Err(e) => {
            log::warn!("[Remote] Could not load host key: {e}");
            return;
        }
    };
    let host_id = derive_device_id(device_key.public_key());

    let pairing_req = match PairingRequest::from_wire(
        &claimed_req.token,
        &claimed_req.device_id,
        &claimed_req.public_key,
        &claimed_req.display_name,
    ) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("[Remote] Malformed pair request: {e}");
            return;
        }
    };

    let source_ip = ws_link
        .peer_addr()
        .map(|a| a.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));

    let Some(handshake_static) = drive_tier1_handshake(&ws_link, &device_key).await else {
        return;
    };

    let pending = match state.pairing_coordinator.begin(
        pairing_req,
        &handshake_static,
        source_ip,
        Instant::now(),
    ) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("[Remote] Pairing rejected by coordinator: {e}");
            let _ = ws_link
                .send_control(ControlMessage::PairDenied(PairDenied {
                    reason: e.to_string(),
                }))
                .await;
            return;
        }
    };

    let expires_at_ms = (Instant::now() + PAIRING_APPROVAL_TIMEOUT)
        .duration_since(Instant::now())
        .as_millis() as i64
        + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

    let _ = emit_ipc(
        &app_handle,
        IpcEvent::RemotePairingStateChanged(
            PairingState::AwaitingApproval {
                device_id: claimed_req.device_id.clone(),
                display_name: claimed_req.display_name.clone(),
                expires_at_ms,
            }
            .to_dto(),
        ),
    );

    let _ = ws_link.send_control(ControlMessage::PairPrompting).await;

    *state.pending_pairing.lock() = Some(PendingPairingState {
        device_id: claimed_req.device_id.clone(),
        display_name: claimed_req.display_name.clone(),
        prompt: pending.prompt(),
    });

    let _ = emit_ipc(
        &app_handle,
        IpcEvent::RemotePairingRequested(RemotePairingPromptPayload {
            device_id: claimed_req.device_id.clone(),
            display_name: claimed_req.display_name.clone(),
        }),
    );

    settle_pairing_coordinator(
        ws_link,
        pending,
        claimed_req,
        handshake_static,
        host_id,
        state,
        app_handle,
    )
    .await;
}

/// Tier 1 completion: write DB, send PairApproved, CLOSE socket.
/// No exchange_hello, no attach_link — spec §4.4 + §5.3.
async fn complete_pairing<R: Runtime>(
    ws_link: WsLink,
    claimed_req: PairRequest,
    pubkey_bytes: Vec<u8>,
    _host_id: DeviceId,
    state: Arc<AppState>,
    app_handle: AppHandle<R>,
) {
    let hints = if let Some(addr) = ws_link.peer_addr() {
        vec![serde_json::json!({ "addr": addr.to_string() })]
    } else {
        vec![]
    };

    if let Ok(conn) = state.db.connect() {
        let _ = insert_paired_device(
            &conn,
            &NewPairedDevice {
                device_id: claimed_req.device_id.clone(),
                public_key: pubkey_bytes,
                display_name: claimed_req.display_name.clone(),
                role: PairedDeviceRole::Controller,
                trust_level: "full".to_string(),
                hints,
            },
        )
        .await;
    }

    // Send PairApproved then CLOSE the pairing socket (spec §5.3 step 12)
    let host_name = sysinfo::System::host_name().unwrap_or_else(|| "Vox Host".to_string());
    let host_identity = SenderIdentity {
        device_id: _host_id.as_str().to_string(),
        display_name: host_name,
    };
    let _ = ws_link
        .send_control(ControlMessage::PairApproved(PairApproved { host_identity }))
        .await;
    let _ = ws_link.close(LinkCloseReason::Local).await;

    // Transition Host to Paired with peer_identity populated
    state.remote_manager.set_peer_identity(Some(SenderIdentity {
        device_id: claimed_req.device_id.clone(),
        display_name: claimed_req.display_name.clone(),
    }));
    state.remote_manager.set_state(RemoteLinkState::Paired);

    // Emit PairingState::Approved (spec §5.5)
    let _ = emit_ipc(
        &app_handle,
        IpcEvent::RemotePairingStateChanged(
            PairingState::Approved {
                device_id: claimed_req.device_id.clone(),
            }
            .to_dto(),
        ),
    );

    log::info!(
        "[Remote] Pairing complete for controller {}. Tier 1 socket closed. \
         Host is now Paired. Connect via Rocket button to start Tier 2.",
        claimed_req.device_id
    );
}
