use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};

use crate::{
    core::{error::VoxIpcError, state::AppState},
    persistence::paired_devices::PairedDevice,
    services::remote::{
        control::{get_routing_table, CommandClass},
        protocol::messages::LinkLossReason,
        security::mint_pairing_invitation,
        session::{RemoteSessionManager, RemoteStateDto},
    },
};

/// Response payload from `pair_new_device`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairNewDeviceDto {
    pub payload: String,
    pub qr_data_url: String,
    pub expires_at: i64,
    pub expires_at_ms: i64,
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

impl From<PairedDevice> for PairedDeviceDto {
    fn from(d: PairedDevice) -> Self {
        Self {
            device_id: d.device_id,
            display_name: d.display_name,
            role: d.role.as_str().to_string(),
            paired_at: d.paired_at,
            last_seen_at: d.last_seen_at,
            hints: d.hints,
        }
    }
}

/// Host initiates pairing: generates ephemeral token and SVG QR data URL.
#[tauri::command]
pub async fn pair_new_device(
    state: State<'_, Arc<AppState>>,
) -> Result<PairNewDeviceDto, VoxIpcError> {
    let accepts = state
        .settings
        .read()
        .map_err(|_| VoxIpcError::Internal("Settings lock poisoned".into()))?
        .remote
        .accepts_controllers;
    if !accepts {
        return Err(VoxIpcError::InvalidState(
            "Remote control is disabled".into(),
        ));
    }

    let (payload, qr_data_url, expires_at, expires_at_ms) =
        mint_pairing_invitation(&state.pairing_coordinator, 7700)
            .map_err(|e| VoxIpcError::Internal(e.to_string()))?;
    Ok(PairNewDeviceDto {
        payload,
        qr_data_url,
        expires_at,
        expires_at_ms,
    })
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
    let devices = RemoteSessionManager::list_paired_devices(&state.db).await?;
    let dtos = devices.into_iter().map(PairedDeviceDto::from).collect();
    Ok(dtos)
}

/// Permanently removes a device from `paired_devices` and disconnects if active.
#[tauri::command]
pub async fn revoke_device(
    device_id: String,
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<(), VoxIpcError> {
    state
        .remote_manager
        .revoke_device(&state.db, &device_id)
        .await
        .map_err(VoxIpcError::from)?;
    let _ = crate::core::events::emit_ipc(
        &app_handle,
        crate::core::events::IpcEvent::RemotePairedDevicesChanged,
    );
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

/// Controller connects to a host by address or stored candidate, with optional pairing credentials.
#[tauri::command]
pub async fn connect_host(
    device_id: Option<String>,
    address: Option<String>,
    token: Option<String>,
    public_key: Option<String>,
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<(), VoxIpcError> {
    let target = address.or(device_id.clone()).ok_or_else(|| {
        VoxIpcError::InvalidArgument("Target host address or deviceId must be provided".into())
    })?;
    let (manager, db) = (Arc::clone(&state.remote_manager), Arc::clone(&state.db));
    let app = app_handle.clone();
    let on_prompting = Box::new(move || {
        let _ = crate::core::events::emit_ipc(
            &app,
            crate::core::events::IpcEvent::RemotePairingPending,
        );
    });
    tokio::spawn(async move {
        let _ = manager
            .dial_and_connect(db, target, device_id, token, public_key, Some(on_prompting))
            .await;
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
pub fn spawn_remote_host_listener<R: Runtime>(app_handle: AppHandle<R>, state: Arc<AppState>) {
    crate::services::remote::session::spawn_host_listener(app_handle, state);
}
