use std::time::Duration;

pub mod audio;
pub mod control;
pub mod discovery;
pub mod link;
pub mod protocol;
pub mod security;
pub mod session;

// ─── Domain constants ───────────────────────────────────────────────────────
//
// Single definition site for every remote-control timing, size cap, token
// lifetime and rate limit. The module that owns each quantity re-exports it
// (for example `session::HEARTBEAT_INTERVAL`). Later waves consume these
// names; no wave may introduce a literal for the same quantity.

// --- Wire framing caps (owned by `protocol/`) ---

/// Maximum size, in bytes, of a control message before the Noise handshake
/// completes. Remote-control spec §7.1.
pub const MAX_PREAUTH_MESSAGE_BYTES: usize = 4 * 1024;

/// The size of the Noise authentication tag, in bytes.
///
/// One Noise transport message is one payload plus this tag.
pub const NOISE_TAG_BYTES: usize = 16;

/// The largest Noise transport message, in bytes.
pub const NOISE_MAX_MESSAGE_BYTES: usize = 65535;

/// The largest plaintext that fits into one Noise message.
pub const NOISE_MAX_PAYLOAD_BYTES: usize = NOISE_MAX_MESSAGE_BYTES - NOISE_TAG_BYTES;

/// Maximum size, in bytes, of a control message after the Noise handshake
/// completes. Remote-control spec §7.1 says 64 KB; the Noise transport is
/// the tighter constraint, so the cap is the transport's. One number, not
/// two that can drift.
pub const MAX_CONTROL_MESSAGE_BYTES: usize = NOISE_MAX_PAYLOAD_BYTES;

/// The wire protocol version this build speaks. Remote-control spec §7.2.
pub const PROTOCOL_VERSION: u16 = 1;

// --- Link lifecycle (owned by `link/`) ---/// Deadline for completing the Noise handshake and the Hello exchange.
/// Remote-control spec §7.1.
pub const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(5);

/// The capacity of each in-process loopback channel.
///
/// The channel is bounded, so a send into a full channel awaits rather than
/// dropping the message: the backpressure is real, not simulated.
pub const LOOPBACK_CHANNEL_CAPACITY: usize = 64;

// --- Session heartbeat (owned by `session/`) ---

/// Interval at which the host sends a heartbeat ping. Remote-control spec §6.3.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);

/// Deadline for the controller to answer a ping with a pong.
/// Remote-control spec §6.3.
pub const HEARTBEAT_PONG_TIMEOUT: Duration = Duration::from_secs(1);

/// Number of missed pongs after which the host declares the link dead and
/// pauses the session. Remote-control spec §6.3.
pub const HEARTBEAT_MAX_MISSED_PONGS: u32 = 3;

/// Deadline after which the controller, having seen no ping, enters the
/// reconnecting state. Remote-control spec §6.3.
pub const HEARTBEAT_PING_TIMEOUT: Duration = Duration::from_secs(6);

// --- Reconnect (controller only) ---
pub const RECONNECT_MAX_ATTEMPTS: u32 = 10;
pub const RECONNECT_INITIAL_BACKOFF: Duration = Duration::from_secs(1);
pub const RECONNECT_MAX_BACKOFF: Duration = Duration::from_secs(30);

// --- Pairing (owned by `security/`) ---

/// Lifetime of a pairing token. Remote-control spec §5.3.
pub const PAIRING_TOKEN_TTL: Duration = Duration::from_secs(120);

/// Minimum entropy of a pairing token, in bits. Remote-control spec §10.3.
pub const PAIRING_TOKEN_MIN_ENTROPY_BITS: usize = 128;

/// Maximum number of pairing attempts per source IP inside the rate-limit
/// window. Remote-control spec §10.3.
pub const PAIRING_ATTEMPTS_PER_WINDOW: u32 = 5;

/// Window over which the per-source-IP pairing attempt limit is evaluated.
/// Remote-control spec §10.3.
pub const PAIRING_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// Number of failed validation attempts from one source IP after which the
/// pairing token is invalidated. Remote-control spec §10.3.
pub const PAIRING_MAX_FAILED_ATTEMPTS: u32 = 3;

/// Deadline for the host user to approve or reject a pairing request.
/// Remote-control spec §5.3.
pub const PAIRING_APPROVAL_TIMEOUT: Duration = Duration::from_secs(30);

// --- Discovery (owned by `discovery/`) ---

/// mDNS service type advertised by a host that accepts controllers.
/// Remote-control spec §6.1. Must end with '._tcp.local.' per RFC 6762 / mdns-sd.
pub const REMOTE_MDNS_SERVICE_TYPE: &str = "_vox._tcp.local.";

/// Dial attempt deadline applied when cycling candidate addresses. Remote-
/// control spec §6.1: the connector must not block one dead candidate for the
/// full transport dial timeout while a discovery fallback waits behind it.
pub const DIAL_CANDIDATE_TIMEOUT: Duration = Duration::from_secs(4);

/// Deadline for one mDNS browse session (controller fallback dial).
/// Remote-control spec §7.1.
pub const MDNS_BROWSE_TIMEOUT: Duration = Duration::from_secs(6);

/// Port the host inbound listener binds (owned by `session/listener.rs`).
/// Spec §6.1 wants a dynamic port eventually; Lan v1 decision table keeps the
/// fixed port and advertises it in the mDNS record and the QR hint.
pub const REMOTE_LISTEN_PORT: u16 = 7700;
