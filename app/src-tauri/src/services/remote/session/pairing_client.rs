//! Outbound Tier 1 Noise_XX pairing handshake and token consumption.

use std::{sync::Arc, time::Duration};

use super::{
    manager::RemoteSessionManager, state::RemoteLinkState, supervisor::controller_display_name,
};
use crate::{
    persistence::{paired_devices, VoxDb},
    services::remote::{
        link::{
            ws::{dial, WsLink},
            LinkCloseReason, LinkError, PeerRejection, RemoteLink,
        },
        protocol::{
            errors::ProtocolError,
            messages::{ControlMessage, PairApproved, PairRequest, SenderIdentity},
        },
        security::{default_noise_prologue, derive_device_id, KeyStore, NoiseHandshake},
    },
};

pub async fn perform_pairing_handshake(
    address: &str,
    token: &str,
    host_public_key: &str,
) -> Result<(WsLink, Vec<u8>), LinkError> {
    let url = if address.starts_with("ws://") || address.starts_with("wss://") {
        address.to_string()
    } else {
        format!("ws://{address}")
    };
    let ws_link = dial(&url).await?;

    let key_store = KeyStore::default_path().map_err(|e| LinkError::Transport(e.to_string()))?;
    let device_key = key_store
        .load_or_create()
        .map_err(|e| LinkError::Transport(e.to_string()))?;

    let controller_id = derive_device_id(device_key.public_key());
    let controller_pubkey_hex = hex::encode(device_key.public_key());
    let controller_name = controller_display_name();

    let pair_req = ControlMessage::PairRequest(PairRequest {
        token: token.to_string(),
        device_id: controller_id.as_str().to_string(),
        public_key: controller_pubkey_hex,
        display_name: controller_name,
    });
    ws_link.send_control(pair_req).await?;

    let h_pk_bytes = hex::decode(host_public_key).map_err(|e| {
        let msg = format!("Invalid host public key hex: {e}");
        LinkError::Transport(msg)
    })?;
    let prologue = default_noise_prologue();
    let handshake = NoiseHandshake::initiator(&device_key, Some(&h_pk_bytes), &prologue)
        .map_err(|e| LinkError::Transport(e.to_string()))?;

    let transport =
        tokio::time::timeout(Duration::from_secs(5), ws_link.drive_handshake(handshake))
            .await
            .map_err(|_| LinkError::Timeout)?
            .map_err(|e| LinkError::Transport(e.to_string()))?;

    ws_link.set_cipher(transport).await;
    Ok((ws_link, h_pk_bytes))
}

pub async fn await_pairing_approval(
    ws_link: &WsLink,
    on_prompting: Option<Box<dyn FnOnce() + Send>>,
) -> Result<PairApproved, LinkError> {
    let mut resp = match tokio::time::timeout(Duration::from_secs(35), ws_link.recv_control()).await
    {
        Ok(Ok(msg)) => msg,
        Ok(Err(e)) => return Err(e),
        Err(_) => {
            return Err(LinkError::Timeout);
        }
    };

    if let ControlMessage::PairPrompting = resp {
        if let Some(cb) = on_prompting {
            cb();
        }
        resp = match tokio::time::timeout(Duration::from_secs(35), ws_link.recv_control()).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(LinkError::Timeout);
            }
        };
    }

    match resp {
        ControlMessage::PairApproved(approved) => Ok(approved),
        ControlMessage::PairDenied(denied) => {
            let msg = format!("Pairing denied by host: {}", denied.reason);
            let _ = ws_link.close(LinkCloseReason::Local).await;
            Err(LinkError::Transport(msg))
        }
        other => {
            let msg = format!("Expected PairApproved, got {:?}", other.message_type());
            let _ = ws_link.close(LinkCloseReason::Local).await;
            Err(LinkError::Protocol(ProtocolError::MalformedJson(msg)))
        }
    }
}

/// Controller-side: dial the host, perform the Tier 1 pairing handshake,
/// write the host into `paired_devices`, close the pairing socket, and
/// transition `RemoteLinkState` to `Paired`.
///
/// Spec §5.3 + §4.4: pairing approval MUST NOT trigger an automatic Tier 2
/// connection. The Rocket button (connect_host) starts Tier 2 explicitly.
pub async fn pair_device(
    manager: Arc<RemoteSessionManager>,

    db: Arc<VoxDb>,
    address: String,
    token: String,
    host_public_key: String,
    host_device_id: String,
    on_prompting: Option<Box<dyn FnOnce() + Send>>,
) -> Result<(), LinkError> {
    manager.set_db(Arc::clone(&db));

    let (ws_link, h_pk_bytes) =
        perform_pairing_handshake(&address, &token, &host_public_key).await?;

    let approved = await_pairing_approval(&ws_link, on_prompting).await?;

    // VERIFY-RC-04: host identity must match the QR claim.
    if approved.host_identity.device_id != host_device_id {
        let _ = ws_link.close(LinkCloseReason::Local).await;
        return Err(LinkError::PeerRejected(PeerRejection::UnknownPeer(
            approved.host_identity.device_id,
        )));
    }
    // RULE-DB-05: derive(public_key) == device_id
    if let Ok(pk) = hex::decode(&host_public_key) {
        if pk.len() == 32 {
            let derived = crate::services::remote::security::derive_device_id(&pk);
            if derived.as_str() != host_device_id {
                let _ = ws_link.close(LinkCloseReason::Local).await;
                return Err(LinkError::PeerRejected(PeerRejection::UnknownPeer(
                    host_device_id.clone(),
                )));
            }
        }
    }

    let host_display_name = if !approved.host_identity.display_name.is_empty() {
        approved.host_identity.display_name
    } else {
        "Vox Host".to_string()
    };

    if let Ok(conn) = db.connect() {
        let _ = paired_devices::insert_paired_device(
            &conn,
            &paired_devices::NewPairedDevice {
                device_id: host_device_id.clone(),
                public_key: h_pk_bytes,
                display_name: host_display_name.clone(),
                role: paired_devices::PairedDeviceRole::Host,
                trust_level: "full".to_string(),
                hints: vec![serde_json::json!({ "addr": address })],
            },
        )
        .await;
    }

    let _ = ws_link.close(LinkCloseReason::Local).await;
    manager.set_peer_identity(Some(SenderIdentity {
        device_id: host_device_id.clone(),
        display_name: host_display_name,
    }));
    manager.set_state(RemoteLinkState::Paired);
    log::info!("[Remote] Pairing complete with host {host_device_id}. Tier 1 socket closed.");
    Ok(())
}
