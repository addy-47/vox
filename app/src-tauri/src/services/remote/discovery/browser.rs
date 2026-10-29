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
pub async fn discover_host(device_id: &str, max_wait: Duration) -> Result<Vec<String>, String> {
    let _multicast_lock = MulticastLockGuard::acquire();
    log::info!(
        "[Remote::Discovery] Starting mDNS browse for '{}' (target device_id={}, max_wait={:?})",
        REMOTE_MDNS_SERVICE_TYPE,
        device_id,
        max_wait
    );
    let daemon = ServiceDaemon::new().map_err(|e| format!("mDNS daemon: {e}"))?;
    let receiver = daemon
        .browse(REMOTE_MDNS_SERVICE_TYPE)
        .map_err(|e| format!("mDNS browse: {e}"))?;

    let deadline = tokio::time::Instant::now() + max_wait;
    let mut candidates: Vec<String> = Vec::new();

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            log::info!(
                "[Remote::Discovery] mDNS browse deadline elapsed (found {} candidate(s))",
                candidates.len()
            );
            break;
        }
        match tokio::time::timeout(remaining, receiver.recv_async()).await {
            Err(_) => {
                log::info!(
                    "[Remote::Discovery] mDNS browse timeout reached (found {} candidate(s))",
                    candidates.len()
                );
                break;
            }
            Ok(Err(e)) => {
                log::warn!("[Remote::Discovery] mDNS receiver error: {e}");
                let _ = daemon.stop_browse(REMOTE_MDNS_SERVICE_TYPE);
                let _ = daemon.shutdown();
                return Err(format!("mDNS receiver: {e}"));
            }
            Ok(Ok(event)) => handle_service_event(event, device_id, &mut candidates),
        }
    }

    let _ = daemon.stop_browse(REMOTE_MDNS_SERVICE_TYPE);
    let _ = daemon.shutdown();
    log::info!(
        "[Remote::Discovery] mDNS browse complete. Candidates: {:?}",
        candidates
    );
    Ok(candidates)
}

/// Handles an mDNS event, collecting candidate endpoints when a matching service is resolved.
fn handle_service_event(event: ServiceEvent, device_id: &str, candidates: &mut Vec<String>) {
    match event {
        ServiceEvent::SearchStarted(ty) => {
            log::debug!("[Remote::Discovery] mDNS browse search started for {ty}");
        }
        ServiceEvent::ServiceFound(ty, fullname) => {
            log::info!(
                "[Remote::Discovery] mDNS service discovered: type={}, fullname={}",
                ty,
                fullname
            );
        }
        ServiceEvent::ServiceResolved(info) => {
            log::info!(
                "[Remote::Discovery] mDNS service resolved: fullname={}, host={}, port={}, addrs={:?}, props={:?}",
                info.get_fullname(),
                info.get_hostname(),
                info.get_port(),
                info.get_addresses(),
                info.get_properties()
            );
            collect_candidate(&info, device_id, candidates);
        }
        ServiceEvent::ServiceRemoved(ty, fullname) => {
            log::debug!(
                "[Remote::Discovery] mDNS service removed: type={}, fullname={}",
                ty,
                fullname
            );
        }
        ServiceEvent::SearchStopped(ty) => {
            log::debug!("[Remote::Discovery] mDNS browse search stopped for {ty}");
        }
        _ => {}
    }
}

/// Matches a resolved service against a device id and appends its IPv4
/// `ip:port` candidates.
fn collect_candidate(info: &ResolvedService, device_id: &str, out: &mut Vec<String>) {
    let prop_device_id = info.get_property_val_str(TXT_DEVICE_ID);
    let matches_id = prop_device_id == Some(device_id);

    if !matches_id {
        log::debug!(
            "[Remote::Discovery] Ignored service {}: device_id property {:?} != expected {}",
            info.get_fullname(),
            prop_device_id,
            device_id
        );
        return;
    }
    let port = info.get_port();
    for ip in info.get_addresses() {
        if let IpAddr::V4(v4) = ip.to_ip_addr() {
            if !v4.is_loopback() {
                let candidate = format!("{}:{}", v4, port);
                if !out.contains(&candidate) {
                    log::info!(
                        "[Remote::Discovery] Matched host candidate for {device_id}: {candidate}"
                    );
                    out.push(candidate);
                }
            }
        }
    }
}
