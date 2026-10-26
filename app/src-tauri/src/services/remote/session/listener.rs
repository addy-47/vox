//! Host-side inbound connection listener.
//!
//! Accepts incoming controller sockets. Two paths:
//! - `PairRequest` (Tier 1): validates token, prompts user, writes DB, closes socket.
//! - `Hello` first message (legacy path): REMOVED per spec §4.4 ZBC. Tier 2 connections
//!   from a reconnecting controller arrive via explicit `connect_host` IPC, not by
//!   sending a bare `Hello` on a new socket.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Runtime};

use crate::{
    core::{
        events::{emit_ipc, IpcEvent, RemotePairingPromptPayload},
        state::{AppState, PendingPairingState},
    },
    persistence::paired_devices::{
        get_paired_device, insert_paired_device, NewPairedDevice, PairedDeviceRole,
    },
    services::remote::{
        link::{
            ws::{bind, WsLink, WsListener},
            LinkCloseReason, RemoteLink,
        },
        protocol::{
            framing::StreamFormat,
            messages::{
                ControlMessage, Hello, LinkRefused, LinkRefusedReason, PairDenied, PairRequest,
                SenderIdentity, UnknownPeer,
            },
        },
        security::{derive_device_id, DeviceId, KeyStore, PairingOutcome, PairingRequest},
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
    // Spec §6.5: refuse if another controller is already connected
    if state.remote_manager.state() == RemoteLinkState::Connected {
        log::warn!("[Remote] Refusing connection: controller already connected");
        return;
    }

    let initial_msg =
        match tokio::time::timeout(Duration::from_secs(10), ws_link.recv_control()).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(e)) => {
                log::warn!("[Remote] Failed to receive initial control message: {e}");
                return;
            }
            Err(_) => {
                log::warn!("[Remote] Initial message timed out");
                return;
            }
        };

    match initial_msg {
        ControlMessage::PairRequest(req) => {
            // Tier 1 path: validate token, prompt user, write DB, close socket
            handle_inbound_pairing(ws_link, req, state, app_handle).await;
        }
        ControlMessage::Hello(peer_hello) => {
            // Tier 2 path: verify paired_devices, reply Hello, attach streaming link
            handle_inbound_hello(ws_link, peer_hello, state).await;
        }
        other => {
            log::warn!(
                "[Remote] Unexpected opening message: {:?}",
                other.message_type()
            );
        }
    }
}

async fn handle_inbound_hello(ws_link: WsLink, peer_hello: Hello, state: Arc<AppState>) {
    // 1. Check if remote accepts controllers (§6.5)
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
        return;
    }

    // 2. Check if another controller is attached (§6.5)
    if state.remote_manager.state() == RemoteLinkState::Connected {
        log::warn!("[Remote] Inbound Hello refused: another controller is already connected");
        let _ = ws_link
            .send_control(ControlMessage::LinkRefused(LinkRefused {
                reason: LinkRefusedReason::ControllerAttached,
            }))
            .await;
        return;
    }

    // 3. Verify peer is in paired_devices (§6.5)
    let peer_id = &peer_hello.sender_identity.device_id;
    let is_paired = if let Ok(conn) = state.db.connect() {
        get_paired_device(&conn, peer_id)
            .await
            .ok()
            .flatten()
            .is_some()
    } else {
        false
    };

    if !is_paired {
        log::warn!("[Remote] Unknown peer attempted Tier 2 connection: {peer_id}");
        let _ = ws_link
            .send_control(ControlMessage::UnknownPeer(UnknownPeer {
                device_id: peer_id.clone(),
            }))
            .await;
        return;
    }

    // 4. Send host Hello response and attach link
    let key_store = match KeyStore::default_path().and_then(|ks| ks.load_or_create()) {
        Ok(k) => k,
        Err(e) => {
            log::warn!("[Remote] Could not load host key: {e}");
            return;
        }
    };
    let host_id = derive_device_id(key_store.public_key());

    let local_identity = SenderIdentity {
        device_id: host_id.as_str().to_string(),
        display_name: "Vox Host".to_string(),
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
        .attach_link(Arc::new(ws_link), peer_hello.sender_identity)
        .await;
}

async fn handle_inbound_pairing<R: Runtime>(
    ws_link: WsLink,
    claimed_req: PairRequest,
    state: Arc<AppState>,
    app_handle: AppHandle<R>,
) {
    let key_store = match KeyStore::default_path().and_then(|ks| ks.load_or_create()) {
        Ok(k) => k,
        Err(e) => {
            log::warn!("[Remote] Could not load host key: {e}");
            return;
        }
    };
    let host_id = derive_device_id(key_store.public_key());

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

    let pubkey_bytes = match hex::decode(&claimed_req.public_key) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("[Remote] Malformed public key: {e}");
            return;
        }
    };

    let source_ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let pending =
        match state
            .pairing_coordinator
            .begin(pairing_req, &pubkey_bytes, source_ip, Instant::now())
        {
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

    // Emit PairingState::AwaitingApproval to frontend (spec §5.5)
    let expires_at_ms = (Instant::now() + PAIRING_APPROVAL_TIMEOUT)
        .duration_since(Instant::now())
        .as_millis() as i64
        + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

    let pairing_state_dto = PairingState::AwaitingApproval {
        device_id: claimed_req.device_id.clone(),
        display_name: claimed_req.display_name.clone(),
        expires_at_ms,
    }
    .to_dto();

    let _ = emit_ipc(
        &app_handle,
        IpcEvent::RemotePairingStateChanged(pairing_state_dto),
    );

    // Inform controller the prompt is active
    let _ = ws_link.send_control(ControlMessage::PairPrompting).await;

    let (responder_tx, responder_rx) = tokio::sync::oneshot::channel();
    *state.pending_pairing.lock() = Some(PendingPairingState {
        device_id: claimed_req.device_id.clone(),
        display_name: claimed_req.display_name.clone(),
        responder: responder_tx,
    });

    let _ = emit_ipc(
        &app_handle,
        IpcEvent::RemotePairingRequested(RemotePairingPromptPayload {
            device_id: claimed_req.device_id.clone(),
            display_name: claimed_req.display_name.clone(),
        }),
    );

    let approved = match tokio::time::timeout(PAIRING_APPROVAL_TIMEOUT, responder_rx).await {
        Ok(Ok(decision)) => decision,
        _ => false,
    };

    if approved {
        pending.prompt().approve();
    } else {
        pending.prompt().reject();
    }

    if let PairingOutcome::Approved { peer: _ } = pending.settle().await {
        complete_pairing(ws_link, claimed_req, pubkey_bytes, host_id, state, app_handle).await;
    } else {
        let _ = ws_link
            .send_control(ControlMessage::PairDenied(PairDenied {
                reason: "Rejected or timed out".into(),
            }))
            .await;

        // Emit PairingState::Denied (spec §5.5)
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
    if let Ok(conn) = state.db.connect() {
        let _ = insert_paired_device(
            &conn,
            &NewPairedDevice {
                device_id: claimed_req.device_id.clone(),
                public_key: pubkey_bytes,
                display_name: claimed_req.display_name.clone(),
                role: PairedDeviceRole::Controller,
                trust_level: "full".to_string(),
                hints: vec![],
            },
        )
        .await;
    }

    // Send PairApproved then CLOSE the pairing socket (spec §5.3 step 12)
    let _ = ws_link.send_control(ControlMessage::PairApproved).await;
    let _ = ws_link.close(LinkCloseReason::Local).await;

    // Transition Host to Paired (spec §5.3: both devices enter Paired after pairing)
    state.remote_manager.set_state(RemoteLinkState::Paired);

    // Emit remote_state_changed { link: "Paired", deviceId, displayName }
    // Patch the dto to carry the new peer's identity since set_state clears peer_identity
    let dto_with_peer = crate::services::remote::session::state::RemoteStateDto {
        link: "Paired".to_string(),
        failure: None,
        failure_kind: None,
        device_id: Some(claimed_req.device_id.clone()),
        display_name: Some(claimed_req.display_name.clone()),
    };
    let _ = emit_ipc(
        &app_handle,
        IpcEvent::RemoteStateChanged(dto_with_peer),
    );

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
