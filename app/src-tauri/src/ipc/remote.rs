use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};

use crate::{
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent, RemotePairingPromptPayload},
        state::{AppState, PendingPairingState},
    },
    persistence::paired_devices::{
        delete_paired_device, get_paired_device, insert_paired_device,
        list_paired_devices as db_list_paired_devices, NewPairedDevice, PairedDeviceRole,
    },
    services::remote::{
        control::{get_routing_table, CommandClass},
        link::{
            ws::{bind, dial, WsLink, WsListener},
            RemoteLink,
        },
        protocol::{
            framing::StreamFormat,
            messages::{
                ControlMessage, Hello, LinkLossReason, PairDenied, PairRequest, SenderIdentity,
                UnknownPeer,
            },
            CAP_CONTROL_MESSAGES, PROTOCOL_VERSION,
        },
        security::{
            derive_device_id, get_local_lan_ip, mint_pairing_qr_svg, DeviceId, DeviceKey,
            KeyStore, PairingOutcome, PairingRequest,
        },
        session::{RemoteLinkState, RemoteStateDto},
        PAIRING_APPROVAL_TIMEOUT, PAIRING_TOKEN_TTL,
    },
};

/// Response payload from `pair_new_device`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairNewDeviceDto {
    pub payload: String,
    pub qr_data_url: String,
    pub expires_at: i64,
}

/// A row from `paired_devices` serialized for IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedDeviceDto {
    pub device_id: String,
    pub display_name: String,
    pub role: String,
    pub paired_at: i64,
    pub last_seen_at: Option<i64>,
    pub hints: Vec<serde_json::Value>,
}

/// Host initiates pairing: generates ephemeral token and SVG QR data URL.
#[tauri::command]
pub async fn pair_new_device(
    state: State<'_, Arc<AppState>>,
) -> Result<PairNewDeviceDto, VoxIpcError> {
    let accepts = {
        let settings = state
            .settings
            .read()
            .map_err(|_| VoxIpcError::Internal("Settings lock poisoned".into()))?;
        settings.remote.accepts_controllers
    };
    if !accepts {
        return Err(VoxIpcError::InvalidState(
            "Remote control is disabled. Enable remote.accepts_controllers first.".into(),
        ));
    }

    let key_store = KeyStore::default_path()
        .map_err(|e| VoxIpcError::Internal(format!("Failed to open keystore: {e}")))?;
    let device_key = key_store
        .load_or_create()
        .map_err(|e| VoxIpcError::Internal(format!("Failed to load device key: {e}")))?;

    generate_pairing_dto(&state, &device_key)
}

/// Host answers inbound pairing approval modal.
#[tauri::command]
pub async fn approve_pairing(
    decision: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let pending = state
        .pending_pairing
        .lock()
        .take()
        .ok_or_else(|| VoxIpcError::NotFound("No pending pairing request to answer".into()))?;

    let approved = decision.eq_ignore_ascii_case("approve");
    let _ = pending.responder.send(approved);
    Ok(())
}

/// Returns the list of devices paired with this Host from Turso DB.
#[tauri::command]
pub async fn list_paired_devices(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<PairedDeviceDto>, VoxIpcError> {
    let conn = state.db.connect()?;
    let devices = db_list_paired_devices(&conn).await?;

    let dtos = devices
        .into_iter()
        .map(|d| PairedDeviceDto {
            device_id: d.device_id,
            display_name: d.display_name,
            role: d.role.as_str().to_string(),
            paired_at: d.paired_at,
            last_seen_at: d.last_seen_at,
            hints: d.hints,
        })
        .collect();

    Ok(dtos)
}

/// Permanently removes a device from `paired_devices` and disconnects if active.
#[tauri::command]
pub async fn revoke_device(
    device_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let conn = state.db.connect()?;
    let _ = delete_paired_device(&conn, &device_id).await?;

    let is_current = state
        .remote_manager
        .peer_identity()
        .map(|p| p.device_id == device_id)
        .unwrap_or(false);

    if is_current {
        let _ = state
            .remote_manager
            .disconnect(LinkLossReason::HostDisconnected)
            .await;
    }

    Ok(())
}

/// Host ends the active connection while keeping device pairing.
#[tauri::command]
pub async fn disconnect_device(
    _device_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    state
        .remote_manager
        .disconnect(LinkLossReason::HostDisconnected)
        .await
        .map_err(|e| VoxIpcError::Network(e.to_string()))
}

/// Controller connects to a host by address or stored candidate.
#[tauri::command]
pub async fn connect_host(
    device_id: Option<String>,
    address: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let target = address.or(device_id).ok_or_else(|| {
        VoxIpcError::InvalidArgument("Target host address or deviceId must be provided".into())
    })?;

    let url = parse_target_url(&target);
    let local_identity = resolve_local_device_identity()?;

    state.remote_manager.set_state(RemoteLinkState::Connecting);

    let manager = Arc::clone(&state.remote_manager);
    tokio::spawn(async move {
        match dial(&url).await {
            Ok(link) => {
                match manager
                    .exchange_hello(
                        &link,
                        local_identity,
                        StreamFormat::UPLINK,
                        StreamFormat::DOWNLINK,
                    )
                    .await
                {
                    Ok(peer_hello) => {
                        manager
                            .attach_link(Arc::new(link), peer_hello.sender_identity)
                            .await;
                    }
                    Err(err) => {
                        manager.set_state(RemoteLinkState::Failed(err.to_string()));
                    }
                }
            }
            Err(err) => {
                manager.set_state(RemoteLinkState::Failed(err.to_string()));
            }
        }
    });

    Ok(())
}

/// Controller cleanly terminates the active connection.
#[tauri::command]
pub async fn disconnect_host(state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError> {
    state
        .remote_manager
        .disconnect(LinkLossReason::HostDisconnected)
        .await
        .map_err(|e| VoxIpcError::Network(e.to_string()))
}

/// Returns the canonical remote link state DTO for UI mount hydration.
#[tauri::command]
pub async fn get_link_state(
    state: State<'_, Arc<AppState>>,
) -> Result<RemoteStateDto, VoxIpcError> {
    Ok(state.remote_manager.state_dto())
}

/// Returns the static command routing classification table.
#[tauri::command]
pub async fn get_command_routing() -> Result<HashMap<String, CommandClass>, VoxIpcError> {
    Ok(get_routing_table().clone())
}

/// Spawns the background listener for incoming controller connections on desktop.
pub fn spawn_remote_host_listener<R: Runtime>(
    app_handle: AppHandle<R>,
    state: Arc<AppState>,
) {
    tauri::async_runtime::spawn(async move {
        run_remote_host_listener_loop(app_handle, state).await;
    });
}

async fn run_remote_host_listener_loop<R: Runtime>(
    app_handle: AppHandle<R>,
    state: Arc<AppState>,
) {
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

    let initial_msg = match tokio::time::timeout(Duration::from_secs(10), ws_link.recv_control()).await {
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
            log::warn!("[Remote] Unexpected initial message: {:?}", other.message_type());
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
    let pending = match state.pairing_coordinator.begin(
        pairing_req,
        &pubkey_bytes,
        source_ip,
        Instant::now(),
    ) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("[Remote] Pairing rejected by coordinator: {e}");
            let _ = ws_link.send_control(ControlMessage::PairDenied(PairDenied {
                reason: e.to_string(),
            })).await;
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

async fn handle_inbound_hello(
    ws_link: WsLink,
    peer_hello: Hello,
    state: Arc<AppState>,
) {
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
        log::warn!("[Remote] Unknown device {peer_id} attempted to connect");
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

    let host_hello = ControlMessage::Hello(Hello {
        version: PROTOCOL_VERSION,
        sender_identity: SenderIdentity {
            device_id: host_id.as_str().to_string(),
            display_name: "Vox Host".to_string(),
        },
        uplink_format: StreamFormat::UPLINK,
        downlink_format: StreamFormat::DOWNLINK,
        capabilities: CAP_CONTROL_MESSAGES,
    });

    if ws_link.send_control(host_hello).await.is_ok() {
        state
            .remote_manager
            .attach_link(Arc::new(ws_link), peer_hello.sender_identity)
            .await;
    }
}

fn parse_target_url(target: &str) -> String {
    if target.starts_with("ws://") || target.starts_with("wss://") {
        target.to_string()
    } else {
        format!("ws://{target}")
    }
}

fn resolve_local_device_identity() -> Result<SenderIdentity, VoxIpcError> {
    let key_store = KeyStore::default_path()
        .map_err(|e| VoxIpcError::Internal(format!("Failed to open keystore: {e}")))?;
    let device_key = key_store
        .load_or_create()
        .map_err(|e| VoxIpcError::Internal(format!("Failed to load device key: {e}")))?;
    let device_id = derive_device_id(device_key.public_key());

    Ok(SenderIdentity {
        device_id: device_id.as_str().to_string(),
        display_name: "Vox Controller".into(),
    })
}

fn generate_pairing_dto(
    state: &AppState,
    device_key: &DeviceKey,
) -> Result<PairNewDeviceDto, VoxIpcError> {
    let host_id = derive_device_id(device_key.public_key());
    let token = state
        .pairing_coordinator
        .issue_token(Instant::now())
        .map_err(|e| VoxIpcError::Internal(format!("Failed to issue pairing token: {e}")))?;

    let hint_addr = get_local_lan_ip().unwrap_or_else(|| "127.0.0.1".to_string());
    let hint_port: u16 = 7700;

    let (payload, qr_data_url) = mint_pairing_qr_svg(
        &host_id,
        device_key.public_key(),
        &hint_addr,
        hint_port,
        &token,
    )
    .map_err(|e| VoxIpcError::Internal(format!("Failed to mint QR code: {e}")))?;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let expires_at = now_ms + PAIRING_TOKEN_TTL.as_millis() as i64;

    Ok(PairNewDeviceDto {
        payload,
        qr_data_url,
        expires_at,
    })
}
