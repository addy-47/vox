//! The capability bitmask a peer announces in its `Hello`.
//!
//! Remote-control spec §7.2: the mask is negotiated per connection and MUST
//! NOT be baked into a stored credential, because it changes across app
//! versions. A peer that sets a bit this build does not know about is
//! ignored, not rejected — an older build must be able to talk to a newer
//! one.
//!
//! v1 defines one capability: the peer speaks the control protocol at all.
//! Further bits are added here, in one place, when a feature becomes
//! genuinely optional. Until then a plain `u64` carries the mask and the
//! wire format is unchanged.

/// The capability bitmask a peer announces in its `Hello`.
pub type Capabilities = u64;

/// The peer speaks the control protocol at all.
pub const CAP_CONTROL_MESSAGES: Capabilities = 1 << 0;

/// Every bit this build understands.
///
/// A bit outside this mask is a capability a future build added. It is
/// ignored on receipt rather than treated as an error.
pub const KNOWN_MASK: Capabilities = CAP_CONTROL_MESSAGES;
