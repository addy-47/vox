//! Host-side mDNS advertisement.
//!
//! Spec: `docs/specs/remote-control-spec.md` §6.1 (Host Advertisement).

use std::{collections::HashMap, time::Duration};

use mdns_sd::{ServiceDaemon, ServiceInfo};

use super::hints::local_ips;
use crate::services::remote::REMOTE_MDNS_SERVICE_TYPE;

// ─── Constants & Type Aliases ────────────────────────────────────────────────

/// How often the host advertiser re-checks the `accepts_controllers` flag and
/// refreshes its announcement when the flag changes.
const ADVERTISER_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// TXT property carrying the host device id (spec §6.1: the phone matches the
/// discovered service against its `paired_devices` table by this identity).
const TXT_DEVICE_ID: &str = "device_id";

/// TXT property distinguishing "host offline" from "host not accepting"
/// (spec §6.1 acceptance note). Values: `"0"` / `"1"`.
const TXT_ACCEPTS: &str = "accepts";

// ─── Data Structures ─────────────────────────────────────────────────────────

/// A live host mDNS announcement. Dropping the struct does not stop the
/// daemon; [`Self::withdraw`] tears the record down explicitly on acceptance
/// flag changes so controllers never see a stale state.
pub struct HostAnnouncement {
    daemon: ServiceDaemon,
    fullname: String,
}

// ─── Trait Implementations ───────────────────────────────────────────────────

// ─── Inherent Implementations ────────────────────────────────────────────────

impl HostAnnouncement {
    /// Publishes (or re-publishes with a new `accepts` flag) the host service.
    pub fn announce(
        device_id: &str,
        port: u16,
        accepts: bool,
    ) -> Result<Self, String> {
        let daemon = ServiceDaemon::new().map_err(|e| format!("mDNS daemon: {e}"))?;

        let accepts_value = if accepts { "1" } else { "0" };
        let mut props = HashMap::new();
        props.insert(TXT_DEVICE_ID.to_string(), device_id.to_string());
        props.insert(TXT_ACCEPTS.to_string(), accepts_value.to_string());

        let instance = instance_name(device_id);
        let host = host_name(device_id);
        let ips = local_ips();
        tracing::info!(
            "[Remote::Advertiser] Announcing mDNS: type={}, instance={}, host={}, port={}, ips={:?}, device_id={}",
            REMOTE_MDNS_SERVICE_TYPE,
            instance,
            host,
            port,
            ips,
            device_id
        );
        let info = ServiceInfo::new(
            REMOTE_MDNS_SERVICE_TYPE,
            &instance,
            &host,
            ips.join(","),
            port,
            props,
        )
        .map_err(|e| format!("mDNS service info: {e}"))?;

        let fullname = info.get_fullname().to_string();

        daemon
            .register(info)
            .map_err(|e| format!("mDNS register: {e}"))?;

        tracing::info!("[Remote::Advertiser] Registered mDNS service: {}", fullname);
        Ok(Self { daemon, fullname })
    }

    /// Withdraws the announcement. Fire-and-forget: the daemon owns the
    /// goodbye packet.
    pub fn withdraw(self) {
        let _ = self.daemon.unregister(&self.fullname);
    }

    /// Interval at which the advertiser polls the acceptance flag. Exposed for
    /// the listener advertiser loop.
    pub(crate) fn poll_interval() -> Duration {
        ADVERTISER_POLL_INTERVAL
    }
}

// ─── Helper Functions ────────────────────────────────────────────────────────

/// Stable mDNS instance name for a host device.
/// RFC 1035 / RFC 6763 limits any single DNS label to 63 bytes.
/// A full SHA-256 device_id is 64 hex characters, so we truncate the label prefix
/// to 32 characters ("Vox-<32 chars>" = 36 bytes <= 63). Controllers match
/// discovered hosts purely by the full 64-char `device_id` in the TXT property.
fn instance_name(device_id: &str) -> String {
    let prefix = if device_id.len() > 32 { &device_id[..32] } else { device_id };
    format!("Vox-{prefix}")
}

/// Stable mDNS host name for address records (RFC 1035 label limit: 63 bytes).
fn host_name(device_id: &str) -> String {
    let prefix = if device_id.len() > 32 { &device_id[..32] } else { device_id };
    format!("vox-{prefix}.local.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_announcement_registers_with_rfc_compliant_service_syntax() {
        let device_id = "390cc0ea95c8390b0f39512bd8156dc8e80942919df397c64dc269aeb3329fb2";
        let announcement = HostAnnouncement::announce(device_id, 7700, true);
        assert!(
            announcement.is_ok(),
            "Host announcement failed RFC registration check: {:?}",
            announcement.err()
        );
        let announcement = announcement.unwrap();
        announcement.withdraw();
    }
}


