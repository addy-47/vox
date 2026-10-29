//! IPC command handlers for the Remote Control feature.
//!
//! Spec §13 IPC contract:
//! - `pair_new_device`:   Host generates ephemeral token + QR.
//! - `pair_device`:       Controller performs Tier 1 pairing handshake.
//! - `approve_pairing`:   Host approves/rejects pending pairing prompt.
//! - `connect_host`:      Controller starts Tier 2 transport session.
//! - `disconnect_host`:   Controller ends Tier 2 session (keeps pairing).
//! - `disconnect_device`: Host ends Tier 2 session (keeps pairing).
//! - `revoke_device`:     Permanent Tier 1 trust revocation.
//! - `list_paired_devices`, `get_link_state`, `get_command_routing`: queries.

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime, State};

use crate::{
    core::{
        error::VoxIpcError,
        events::{emit_ipc, IpcEvent},
        state::AppState,
    },
    persistence::paired_devices::PairedDevice,
    services::remote::{
        control::{get_routing_table, CommandClass},
        security::mint_pairing_invitation,
        session::{PairingStateDto, RemoteSessionManager, RemoteStateDto},
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

// ─── Host-side Tier 1 ───────────────────────────────────────────────────────

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
    if approved {
        pending.prompt.approve();
    } else {
        pending.prompt.reject();
    }
    Ok(())
}

// ─── Controller-side Tier 1 ─────────────────────────────────────────────────

/// Controller performs the full Tier 1 pairing handshake against a Host.
///
/// Spec §4.4: Tier 1 only — writes host into paired_devices, closes the
/// pairing socket. Does NOT start a Tier 2 transport session.
/// Use `connect_host` to start Tier 2 after pairing completes.
#[tauri::command]
pub async fn pair_device(
    address: String,
    token: String,
    public_key: String,
    device_id: String,
    state: State<'_, Arc<AppState>>,
    app_handle: tauri::AppHandle,
) -> Result<(), VoxIpcError> {
    let (manager, db) = (Arc::clone(&state.remote_manager), Arc::clone(&state.db));
    let app = app_handle.clone();
    let on_prompting = Box::new(move || {
        let _ = emit_ipc(
            &app,
            IpcEvent::RemotePairingStateChanged(PairingStateDto {
                state: "AwaitingApproval".to_string(),
                device_id: None,
                display_name: None,
                expires_at_ms: None,
                failure_reason: None,
            }),
        );
    });
    let app_done = app_handle.clone();
    let dev_id_clone = device_id.clone();
    tokio::spawn(async move {
        match manager
            .clone()
            .pair_device(
                db,
                address,
                token,
                public_key,
                dev_id_clone.clone(),
                Some(on_prompting),
            )
            .await
        {
            Ok(()) => {
                let display_name = manager
                    .peer_identity()
                    .map(|p| p.display_name)
                    .unwrap_or_else(|| "Vox Host".to_string());
                let _ = emit_ipc(
                    &app_done,
                    IpcEvent::RemotePairingStateChanged(PairingStateDto {
                        state: "Approved".to_string(),
                        device_id: Some(dev_id_clone),
                        display_name: Some(display_name),
                        expires_at_ms: None,
                        failure_reason: None,
                    }),
                );
            }
            Err(e) => {
                log::error!("[Remote] pair_device failed: {e}");
                let _ = emit_ipc(
                    &app_done,
                    IpcEvent::RemotePairingStateChanged(PairingStateDto {
                        state: "Denied".to_string(),
                        device_id: None,
                        display_name: None,
                        expires_at_ms: None,
                        failure_reason: Some(e.to_string()),
                    }),
                );
            }
        }
    });
    Ok(())
}

// ─── Tier 2 — transport session ─────────────────────────────────────────────

/// Controller connects to an already-paired host.
///
/// Spec §4.4 Tier 2: device must already be in paired_devices.
/// Runs Noise handshake + Hello, spawns Link Supervisor.
/// `address` is optional — falls back to paired_devices hints if absent.
#[tauri::command]
pub async fn connect_host(
    device_id: String,
    address: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    let (manager, db) = (Arc::clone(&state.remote_manager), Arc::clone(&state.db));
    tokio::spawn(async move {
        if let Err(e) = manager.dial_and_connect(db, device_id, address).await {
            log::error!("[Remote] connect_host failed: {e}");
        }
    });
    Ok(())
}

/// Controller cleanly terminates the active Tier 2 session.
///
/// Spec §6.4: sends `Disconnect`, transitions both sides to `Paired`.
#[tauri::command]
pub async fn disconnect_host(state: State<'_, Arc<AppState>>) -> Result<(), VoxIpcError> {
    state
        .remote_manager
        .disconnect_controller_initiated()
        .await
        .map_err(|e| VoxIpcError::Network(e.to_string()))
}

/// Host cleanly terminates the active Tier 2 session.
///
/// Spec §6.6: sends `LinkLoss(HostDisconnected)`, transitions both sides to `Paired`.
#[tauri::command]
pub async fn disconnect_device(
    _device_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    state
        .remote_manager
        .disconnect_host_initiated()
        .await
        .map_err(|e| VoxIpcError::Network(e.to_string()))
}

// ─── Tier 1 trust management ─────────────────────────────────────────────────

/// Permanently removes a device from `paired_devices`.
///
/// Spec §5.8: sends `LinkLoss(Revoked)` if active, deletes from DB,
/// transitions to `Unpaired` (0 remaining) or `Paired` (>0 remaining).
/// Emits `remote_state_changed` via the state callback.
#[tauri::command]
pub async fn revoke_device(
    device_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), VoxIpcError> {
    state
        .remote_manager
        .revoke_device(&state.db, &device_id)
        .await
        .map_err(VoxIpcError::from)?;
    // No RemotePairedDevicesChanged emission: state is communicated via
    // remote_state_changed only (spec §13, single canonical event rule).
    Ok(())
}

// ─── Queries ─────────────────────────────────────────────────────────────────

/// Returns the list of devices paired with this Host from Turso DB.
#[tauri::command]
pub async fn list_paired_devices(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<PairedDeviceDto>, VoxIpcError> {
    let devices = RemoteSessionManager::list_paired_devices(&state.db).await?;
    let dtos = devices.into_iter().map(PairedDeviceDto::from).collect();
    Ok(dtos)
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
