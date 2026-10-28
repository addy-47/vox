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
        let host = format!("vox-{device_id}.local.");
        let info = ServiceInfo::new(
            REMOTE_MDNS_SERVICE_TYPE,
            &instance,
            &host,
            local_ips().join(","),
            port,
            props,
        )
        .map_err(|e| format!("mDNS service info: {e}"))?;

        daemon
            .register(info)
            .map_err(|e| format!("mDNS register: {e}"))?;

        Ok(Self {
            daemon,
            fullname: format!("{}.{}", instance_name(device_id), REMOTE_MDNS_SERVICE_TYPE),
        })
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

/// Stable mDNS instance name for a host device. Controllers match services to
/// their `paired_devices` rows purely by the `device_id` TXT property, so the
/// instance name only needs to be unique per LAN; embedding the device id satisfies
/// both requirements.
fn instance_name(device_id: &str) -> String {
    format!("Vox-{device_id}")
}

