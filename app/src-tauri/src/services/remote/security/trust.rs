use std::collections::HashSet;

use parking_lot::Mutex;
use turso::Connection;

use super::identity::derive_device_id;
use crate::{
    core::error::PersistenceError,
    persistence::paired_devices::{
        delete_paired_device, get_paired_device, PairedDevice, PairedDeviceRole,
    },
};

/// The decision the trust store reached about a peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustDecision {
    /// The peer is paired, and it may connect in the role it was looked up
    /// for.
    Trusted(PairedDevice),
    /// The peer may not connect. Every refusal is terminal.
    Refused(TrustRefusal),
}

/// Why a peer was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustRefusal {
    /// The peer has never been paired with this device.
    UnknownPeer,
    /// The pairing was revoked. Terminal: the connection is not retried.
    Revoked,
    /// The peer's stored role contradicts the direction of the connection.
    RoleMismatch {
        /// The role the row stores.
        stored_role: PairedDeviceRole,
    },
    /// The stored row's device id is not the derivation of its public key.
    ///
    /// db-spec RULE-DB-05: the pair is stored verbatim, so a row that
    /// disagrees is rejected at the trust boundary rather than trusted.
    KeyMismatch,
}

/// The trust store over the `paired_devices` registry.
///
/// A device is trusted if and only if it has a row in `paired_devices`. There
/// is no second source of truth: this store reads the table, it does not
/// cache it.
pub struct TrustStore {
    /// The device ids this process has revoked.
    ///
    /// Revocation *is* the deletion of the row, so after the deletion the
    /// peer is unknown and the table cannot say why. The tombstone records
    /// the reason for this process only. It can never make a peer trusted —
    /// it can only make a peer refused — so it cannot disagree with the
    /// database in the unsafe direction.
    ///
    /// The tombstone is **process-scoped**: after a restart the row is gone
    /// and the peer reports `UnknownPeer` instead of `Revoked`. Both are
    /// terminal, so the behaviour is equivalent; only the message differs.
    revoked: Mutex<HashSet<String>>,
}

impl Default for TrustStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TrustStore {
    /// An empty trust store.
    pub fn new() -> Self {
        Self {
            revoked: Mutex::new(HashSet::new()),
        }
    }

    /// Looks a peer up and decides whether it may connect.
    ///
    /// `connecting_role` is the role the peer is connecting *as*. A peer
    /// whose stored role contradicts it is refused.
    pub async fn lookup(
        &self,
        conn: &Connection,
        device_id: &str,
        connecting_role: PairedDeviceRole,
    ) -> Result<TrustDecision, PersistenceError> {
        let Some(device) = get_paired_device(conn, device_id).await? else {
            let refusal = if self.is_revoked(device_id) {
                TrustRefusal::Revoked
            } else {
                TrustRefusal::UnknownPeer
            };
            return Ok(TrustDecision::Refused(refusal));
        };

        // The key binding: the row's id must be the derivation of its key.
        if derive_device_id(&device.public_key).as_str() != device.device_id {
            return Ok(TrustDecision::Refused(TrustRefusal::KeyMismatch));
        }

        // The role: the stored role must match the direction of the
        // connection.
        if device.role != connecting_role {
            return Ok(TrustDecision::Refused(TrustRefusal::RoleMismatch {
                stored_role: device.role,
            }));
        }

        Ok(TrustDecision::Trusted(device))
    }

    /// Revokes a peer: deletes the row and records the revocation.
    ///
    /// The deletion is the revocation. After it the peer is not trusted, and
    /// the connection attempt is terminal.
    pub async fn revoke(
        &self,
        conn: &Connection,
        device_id: &str,
    ) -> Result<bool, PersistenceError> {
        let deleted = delete_paired_device(conn, device_id).await?;
        if deleted {
            self.revoked.lock().insert(device_id.to_string());
        }
        Ok(deleted)
    }

    /// Whether this process revoked the device.
    fn is_revoked(&self, device_id: &str) -> bool {
        self.revoked.lock().contains(device_id)
    }
}
