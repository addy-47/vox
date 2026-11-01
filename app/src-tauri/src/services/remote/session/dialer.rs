//! Outbound Tier 2 dialer, candidate resolution, and Noise handshake.

use std::{sync::Arc, time::Duration};

use super::{
    manager::RemoteSessionManager,
    state::{RemoteFailureKind, RemoteLinkState},
    supervisor::{classify_dial_error, controller_display_name},
};
use crate::{
    persistence::{paired_devices, VoxDb},
    services::remote::{
        discovery,
        link::{
            ws::{dial_with_timeout, WsLink},
            LinkError, PeerRejection,
        },
        protocol::{
            framing::StreamFormat,
            messages::{LinkRefusedReason, SenderIdentity},
        },
        security::{default_noise_prologue, derive_device_id, KeyStore, NoiseHandshake},
        DIAL_CANDIDATE_TIMEOUT, REMOTE_LISTEN_PORT,
    },
};

/// Dials a candidate and runs the Noise handshake to completion.
///
/// Returns the sealed link paired with our local identity, or the
/// classified transport error (spec §6.1 candidate ordering).
pub(crate) async fn dial_link(
    candidate: &str,
    host_device: &paired_devices::PairedDevice,
) -> Result<(WsLink, SenderIdentity), LinkError> {
    let url = discovery::dial_url(candidate, REMOTE_LISTEN_PORT);
    let ws_link = dial_with_timeout(&url, DIAL_CANDIDATE_TIMEOUT).await?;
    let local_identity = drive_noise_handshake(&ws_link, &host_device.public_key).await?;
    Ok((ws_link, local_identity))
}

/// Drives the Noise initiator handshake over a freshly dialed link.
pub(crate) async fn drive_noise_handshake(
    ws_link: &WsLink,
    host_public_key: &[u8],
) -> Result<SenderIdentity, LinkError> {
    let key_store = KeyStore::default_path().map_err(|e| LinkError::Transport(e.to_string()))?;
    let device_key = key_store
        .load_or_create()
        .map_err(|e| LinkError::Transport(e.to_string()))?;

    let prologue = default_noise_prologue();
    let handshake = NoiseHandshake::initiator(&device_key, Some(host_public_key), &prologue)
        .map_err(|e| LinkError::Transport(e.to_string()))?;

    let transport =
        tokio::time::timeout(Duration::from_secs(5), ws_link.drive_handshake(handshake))
            .await
            .map_err(|_| LinkError::Timeout)?
            .map_err(|e| LinkError::Transport(e.to_string()))?;

    ws_link.set_cipher(transport).await;

    let controller_id = derive_device_id(device_key.public_key());
    let controller_name = controller_display_name();
    Ok(SenderIdentity {
        device_id: controller_id.as_str().to_string(),
        display_name: controller_name,
    })
}

/// Attempts to dial each candidate in order until one succeeds.
pub(crate) async fn try_dial_candidates(
    candidates: &[String],
    host_device: &paired_devices::PairedDevice,
    tag: &str,
) -> Result<(WsLink, SenderIdentity, String), Option<LinkError>> {
    let mut last_error = None;
    for candidate in candidates {
        match dial_link(candidate, host_device).await {
            Ok((link, identity)) => return Ok((link, identity, candidate.clone())),
            Err(e) => {
                log::warn!("[Remote] {tag} candidate {candidate} failed: {e}");
                last_error = Some(e);
            }
        }
    }
    Err(last_error)
}

/// Dial candidate resolution. Spec §6.1 (in order): explicit provided
/// address (QR / manual entry) → stored `paired_devices.hints` → mDNS
/// browse for the host `device_id` (the fallback that survives network
/// changes; refreshed hints are persisted before the first mDNS dial).
pub(crate) async fn perform_dial_handshake(
    host_device: &paired_devices::PairedDevice,
    address: Option<String>,
) -> Result<(WsLink, SenderIdentity, String), LinkError> {
    let mut candidates: Vec<String> = Vec::new();
    if let Some(addr) = address {
        candidates.push(addr);
    }
    candidates.extend(discovery::lan_hint_addrs(&host_device.hints));

    log::info!(
        "[Remote] perform_dial_handshake for {}: stored candidates={:?}",
        host_device.device_id,
        candidates
    );

    match try_dial_candidates(&candidates, host_device, "Dial").await {
        Ok(res) => return Ok(res),
        Err(e) => {
            let _ = e;
        }
    }

    // mDNS fallback: the stored hints are IPs captured at pairing time —
    // stale after any network change. Browse the LAN by host device id.
    log::info!(
        "[Remote] Stored candidates exhausted. Triggering mDNS browse for {} (timeout={:?})",
        host_device.device_id,
        discovery::MDNS_BROWSE_TIMEOUT
    );
    let mdns_candidates =
        discovery::discover_host(&host_device.device_id, discovery::MDNS_BROWSE_TIMEOUT)
            .await
            .unwrap_or_else(|e| {
                log::warn!("[Remote] mDNS browse failed: {e}");
                Vec::new()
            });

    log::info!(
        "[Remote] mDNS browse returned {} candidate(s): {:?}",
        mdns_candidates.len(),
        mdns_candidates
    );

    try_dial_candidates(&mdns_candidates, host_device, "mDNS")
        .await
        .map_err(|err| err.unwrap_or_else(|| LinkError::Transport("No dial candidates".into())))
}

pub(crate) async fn handle_dial_peer_rejection(
    manager: &RemoteSessionManager,
    db: &VoxDb,
    device_id: &str,
    rejection: &PeerRejection,
) {
    match rejection {
        PeerRejection::UnknownPeer(_) => {
            if let Ok(conn) = db.connect() {
                let _ = paired_devices::delete_paired_device(&conn, device_id).await;
            }
            manager.set_peer_identity(None);
            *manager.event_forwarder.lock() = None;
            manager.set_state(RemoteLinkState::failed(
                "Device pairing was revoked.".to_string(),
                RemoteFailureKind::Revoked,
            ));
        }
        PeerRejection::LinkRefused(reason) => {
            let (msg, kind) = match reason {
                LinkRefusedReason::ControllerAttached => (
                    "Another controller is already connected".to_string(),
                    RemoteFailureKind::HostBusy,
                ),
                LinkRefusedReason::NotAccepting => (
                    "Host is not accepting controllers".to_string(),
                    RemoteFailureKind::RejectedByHost,
                ),
                LinkRefusedReason::HostBusy => (
                    "Host is currently busy with local session".to_string(),
                    RemoteFailureKind::HostBusy,
                ),
            };
            manager.set_state(RemoteLinkState::failed(msg, kind));
        }
        PeerRejection::RoleNotPermitted(r) => {
            manager.set_state(RemoteLinkState::failed(
                format!("Role not permitted by host: {r}"),
                RemoteFailureKind::RejectedByHost,
            ));
        }
        PeerRejection::VersionMismatch {
            version,
            supported_version,
        } => {
            manager.set_state(RemoteLinkState::failed(
                format!("Protocol version mismatch: peer {version}, supported {supported_version}"),
                RemoteFailureKind::VersionMismatch,
            ));
        }
    }
}

pub(crate) async fn handle_dial_rejection(
    manager: &RemoteSessionManager,
    db: &VoxDb,
    device_id: &str,
    error: LinkError,
    was_reconnecting: bool,
) -> LinkError {
    match &error {
        LinkError::PeerRejected(rejection) => {
            handle_dial_peer_rejection(manager, db, device_id, rejection).await;
        }
        LinkError::Timeout if !was_reconnecting => {
            manager.set_state(RemoteLinkState::failed(
                format!("Handshake timed out connecting to {device_id}"),
                RemoteFailureKind::Timeout,
            ));
        }
        _ if !was_reconnecting => {
            manager.set_state(RemoteLinkState::failed(
                format!("Connection error connecting to {device_id}: {error}"),
                RemoteFailureKind::Unknown,
            ));
        }
        _ => {
            log::debug!(
                "[Remote] Transient reconnect error: {error}; maintaining Reconnecting state"
            );
        }
    }
    error
}

/// Controller-side: dial a paired host, run Noise handshake, exchange
/// Hello, and spawn the Link Supervisor. Assumes device is already in
/// `paired_devices`. Does NOT perform pairing; call `pair_device` first.
///
/// Spec §4.4 Tier 2: explicitly initiated from the Home Rocket button.
pub async fn dial_and_connect(
    manager: Arc<RemoteSessionManager>,

    db: Arc<VoxDb>,
    device_id: String,
    address: Option<String>,
) -> Result<(), LinkError> {
    manager.set_db(Arc::clone(&db));

    let host_device = if let Ok(conn) = db.connect() {
        paired_devices::get_paired_device(&conn, &device_id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };
    let host_device = host_device.ok_or_else(|| {
        LinkError::Transport(format!("Device {device_id} not found in paired_devices"))
    })?;

    manager.set_peer_identity(Some(SenderIdentity {
        device_id: host_device.device_id.clone(),
        display_name: host_device.display_name.clone(),
    }));

    let was_reconnecting = manager.state() == RemoteLinkState::Reconnecting;
    if !was_reconnecting {
        manager.set_state(RemoteLinkState::Connecting);
    }

    let (ws_link, local_identity, chosen_candidate) = perform_dial_handshake(&host_device, address)
        .await
        .map_err(|e| {
            if !was_reconnecting {
                let kind = classify_dial_error(&e.to_string());
                manager.set_state(RemoteLinkState::failed(format!("Dial failed: {e}"), kind));
            }
            e
        })?;

    *manager.active_host_addr.write() = Some(chosen_candidate.clone());

    // Spec §5.7: hints update on successful connection (source: Observed).
    // The dial candidate that worked is the freshest known address, placed at index 0
    // while preserving existing hints for forward compatibility.
    if let Ok(conn) = db.connect() {
        let hints = discovery::update_observed_hint(&host_device.hints, &chosen_candidate);
        let _ = paired_devices::update_paired_device_hints(&conn, &device_id, &hints).await;
    }

    let peer_hello = match manager
        .exchange_hello(
            &ws_link,
            local_identity,
            StreamFormat::UPLINK,
            StreamFormat::DOWNLINK,
        )
        .await
    {
        Ok(h) => h,
        Err(e) => {
            return Err(
                handle_dial_rejection(&manager, &db, &device_id, e, was_reconnecting).await,
            );
        }
    };

    manager
        .attach_link(
            Arc::new(ws_link),
            peer_hello.sender_identity,
            Some(Arc::clone(&manager)),
        )
        .await;
    Ok(())
}
