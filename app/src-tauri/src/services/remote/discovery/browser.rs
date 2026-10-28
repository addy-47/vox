//! Controller-side mDNS browsing and host discovery.
//!
//! Spec: `docs/specs/remote-control-spec.md` §6.1 (Discovery — mDNS fallback).

use std::{net::IpAddr, time::Duration};

use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent};

use super::multicast::MulticastLockGuard;
use crate::services::remote::REMOTE_MDNS_SERVICE_TYPE;

// ─── Constants & Type Aliases ────────────────────────────────────────────────

/// TXT property carrying the host device id (spec §6.1: the phone matches the
/// discovered service against its `paired_devices` table by this identity).
const TXT_DEVICE_ID: &str = "device_id";

// ─── Data Structures ─────────────────────────────────────────────────────────

// ─── Trait Implementations ───────────────────────────────────────────────────

// ─── Inherent Implementations ────────────────────────────────────────────────

// ─── Helper Functions ────────────────────────────────────────────────────────

/// Browses `_vox._tcp.local` for `max_wait` duration and returns every `ip:port`
/// advertising the requested host `device_id`. LAN v1 is IPv4; v6 responders
/// are skipped.
///
/// On Android, acquires a `MulticastLock` for the duration of the browse to prevent
/// the Wi-Fi radio from filtering mDNS multicast frames.
pub async fn discover_host(
    device_id: &str,
    max_wait: Duration,
) -> Result<Vec<String>, String> {
    let _multicast_lock = MulticastLockGuard::acquire();
    let daemon = ServiceDaemon::new().map_err(|e| format!("mDNS daemon: {e}"))?;
    let receiver = daemon
        .browse(REMOTE_MDNS_SERVICE_TYPE)
        .map_err(|e| format!("mDNS browse: {e}"))?;

    let deadline = tokio::time::Instant::now() + max_wait;
    let mut candidates: Vec<String> = Vec::new();

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, receiver.recv_async()).await {
            Err(_) => break, // deadline reached
            Ok(Err(e)) => return Err(format!("mDNS receiver: {e}")),
            Ok(Ok(ServiceEvent::ServiceResolved(info))) => {
                collect_candidate(&info, device_id, &mut candidates);
            }
            Ok(Ok(_)) => {}
        }
    }

    let _ = daemon.stop_browse(REMOTE_MDNS_SERVICE_TYPE);
    Ok(candidates)
}

/// Matches a resolved service against a device id and appends its IPv4
/// `ip:port` candidates.
fn collect_candidate(info: &ResolvedService, device_id: &str, out: &mut Vec<String>) {
    let matches_id = info
        .get_property_val_str(TXT_DEVICE_ID)
        .map(|id| id == device_id)
        .unwrap_or(false);

    if !matches_id {
        return;
    }
    let port = info.get_port();
    for ip in info.get_addresses() {
        if let IpAddr::V4(v4) = ip.to_ip_addr() {
            if !v4.is_loopback() {
                let candidate = format!("{}:{}", v4, port);
                if !out.contains(&candidate) {
                    out.push(candidate);
                }
            }
        }
    }
}

