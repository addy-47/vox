//! LAN discovery of remote peers.
//!
//! **Owns:** module declarations, re-exports, and domain constants for LAN discovery.
//!
//! **Must never contain:** session state or business logic.
//!
//! Spec: `docs/specs/remote-control-spec.md` §5.7 (Address Hints), §6.1
//! (Discovery — QR hint first, mDNS fallback, manual entry tertiary).

pub mod advertiser;
pub mod browser;
pub mod hints;
pub mod multicast;

pub use super::{MDNS_BROWSE_TIMEOUT, REMOTE_MDNS_SERVICE_TYPE};
pub use advertiser::HostAnnouncement;
pub use browser::discover_host;
pub use hints::{lan_hint, lan_hint_addrs, update_observed_hint};
pub(crate) use hints::dial_url;
pub use multicast::MulticastLockGuard;
