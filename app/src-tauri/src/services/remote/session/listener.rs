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
            RemoteLink,
        },
        protocol::{
            framing::StreamFormat,
            messages::{
                ControlMessage, Hello, PairDenied, PairRequest, SenderIdentity, UnknownPeer,
            },
        },
        security::{derive_device_id, DeviceId, KeyStore, PairingOutcome, PairingRequest},
        session::state::RemoteLinkState,
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
            handle_inbound_pairing(ws_link, req, state, app_handle).await;
        }
        ControlMessage::Hello(peer_hello) => {
            handle_inbound_hello(ws_link, peer_hello, state).await;
        }
        other => {
            log::warn!(
                "[Remote] Unexpected initial message: {:?}",
                other.message_type()
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
        complete_pairing_and_attach(ws_link, claimed_req, pubkey_bytes, host_id, state).await;
    } else {
        let _ = ws_link
            .send_control(ControlMessage::PairDenied(PairDenied {
                reason: "Rejected or timed out".into(),
            }))
            .await;
    }
}

async fn complete_pairing_and_attach(
    ws_link: WsLink,
    claimed_req: PairRequest,
    pubkey_bytes: Vec<u8>,
    host_id: DeviceId,
    state: Arc<AppState>,
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

    let _ = ws_link.send_control(ControlMessage::PairApproved).await;

    let local_identity = SenderIdentity {
        device_id: host_id.as_str().to_string(),
        display_name: "Vox Host".to_string(),
    };

    if let Ok(peer_hello) = state
        .remote_manager
        .exchange_hello(
            &ws_link,
            local_identity,
            StreamFormat::DOWNLINK,
            StreamFormat::UPLINK,
        )
        .await
    {
        state
            .remote_manager
            .attach_link(Arc::new(ws_link), peer_hello.sender_identity)
            .await;
    }
}

async fn handle_inbound_hello(ws_link: WsLink, peer_hello: Hello, state: Arc<AppState>) {
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
        log::warn!("[Remote] Unknown peer attempted connection: {peer_id}");
        let _ = ws_link
            .send_control(ControlMessage::UnknownPeer(UnknownPeer {
                device_id: peer_id.clone(),
            }))
            .await;
        return;
    }

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
