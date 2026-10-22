//! LAN discovery of remote peers.
//!
//! **Owns:** the typed address-hint representation, the mDNS advertisement and
//! browsing of the `_vox._tcp.local` service, and the ordering of dial
//! candidates.
//!
//! **Must never contain:** session state. Discovery answers *where* a peer
//! can be reached; it never answers *whether* the peer may be reached.
//!
//! The persistence layer stores address hints as opaque JSON so that an
//! unrecognised hint kind survives a round trip. The typed representation
//! lives here.
//!
//! Workspace rule: `AGENTS.md` §7.1, `discovery/`.

pub use super::REMOTE_MDNS_SERVICE_TYPE;
