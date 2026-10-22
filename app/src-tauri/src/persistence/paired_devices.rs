use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use turso::Connection;

use crate::core::error::PersistenceError;

pub type Result<T> = std::result::Result<T, PersistenceError>;

/// Columns selected by every read in this module, in decode order.
const SELECT_COLUMNS: &str =
    "device_id, public_key, display_name, role, trust_level, hints, paired_at, last_seen_at";

/// The role a paired peer holds relative to this device.
///
/// Stored as the exact strings `controller` and `host` so the schema `CHECK`
/// constraint and this enum cannot drift apart. The laptop stores its
/// controllers (`Controller`); the phone stores its hosts (`Host`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairedDeviceRole {
    Controller,
    Host,
}

impl PairedDeviceRole {
    /// Returns the stored `role` string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Controller => "controller",
            Self::Host => "host",
        }
    }
}

/// A device that has completed remote-control pairing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairedDevice {
    /// Lowercase hex of `SHA-256("vox/device-id/v1" ‖ x25519_public_key)` —
    /// the derivation domain string is prepended to the 32 public key bytes
    /// before hashing (remote-control spec §5.1, db-spec §5.14).
    ///
    /// The identity layer owns the derivation. The persistence layer stores the
    /// pair verbatim and does not re-derive it; the trust boundary is where a
    /// mismatch is detected and rejected (RULE-DB-05).
    pub device_id: String,
    /// The peer's 32-byte X25519 static public key. The private half never
    /// enters the database.
    pub public_key: Vec<u8>,
    /// Human-readable peer name shown in the device list.
    pub display_name: String,
    /// The role this peer holds relative to this device.
    pub role: PairedDeviceRole,
    /// The trust tier granted to this peer.
    ///
    /// `full` is the only tier in v1, and the schema's `CHECK` constraint is
    /// what enforces it. The column exists so a reduced-trust tier can be
    /// added later without a schema migration; a second Rust enum would only
    /// be a second, weaker copy of that constraint.
    pub trust_level: String,
    /// Address hints, stored as opaque JSON objects.
    ///
    /// The persistence layer never interprets a hint. An unrecognised `kind`
    /// therefore survives a round trip untouched, so a newer transport's hints
    /// are not destroyed by an older build reading them. The typed
    /// representation belongs to `services::remote::discovery`.
    pub hints: Vec<serde_json::Value>,
    /// Pairing completion time, millisecond epoch.
    pub paired_at: i64,
    /// Last connect or disconnect time, millisecond epoch. `None` until the
    /// peer connects for the first time.
    pub last_seen_at: Option<i64>,
}

/// Input for registering a completed pairing.
#[derive(Debug, Clone)]
pub struct NewPairedDevice {
    /// The peer's device id, derived by the identity layer as
    /// `SHA-256("vox/device-id/v1" ‖ x25519_public_key)`. This access layer
    /// stores it verbatim and never re-derives it (RULE-DB-05).
    pub device_id: String,
    pub public_key: Vec<u8>,
    pub display_name: String,
    pub role: PairedDeviceRole,
    pub trust_level: String,
    pub hints: Vec<serde_json::Value>,
}

/// Returns the current wall-clock time in millisecond epoch.
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Decodes the stored `role` string into the typed role.
fn parse_role(raw: &str) -> Result<PairedDeviceRole> {
    match raw {
        "controller" => Ok(PairedDeviceRole::Controller),
        "host" => Ok(PairedDeviceRole::Host),
        other => Err(PersistenceError::InvalidRecord {
            message: format!("Unknown paired_devices.role: {}", other),
        }),
    }
}

/// Decodes the stored `hints` JSON array, preserving unknown hint kinds.
fn parse_hints(raw: &str) -> Result<Vec<serde_json::Value>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str::<Vec<serde_json::Value>>(trimmed).map_err(|e| {
        PersistenceError::InvalidRecord {
            message: format!("Invalid paired_devices.hints JSON: {}", e),
        }
    })
}

/// Decodes one `paired_devices` row.
fn decode_row(row: &turso::Row) -> Result<PairedDevice> {
    let role_raw: String = row.get(3)?;
    let trust_raw: String = row.get(4)?;
    let hints_raw: String = row.get(5)?;
    Ok(PairedDevice {
        device_id: row.get(0)?,
        public_key: row.get(1)?,
        display_name: row.get(2)?,
        role: parse_role(&role_raw)?,
        trust_level: trust_raw,
        hints: parse_hints(&hints_raw)?,
        paired_at: row.get(6)?,
        last_seen_at: row.get(7)?,
    })
}

/// Inserts a completed pairing and returns the stored row.
///
/// The pairing time is stamped by the access layer so it cannot drift from
/// the insert. Inserting a `device_id` that is already present fails: a peer
/// is either paired once or revoked and paired again.
pub async fn insert_paired_device(
    conn: &Connection,
    device: &NewPairedDevice,
) -> Result<PairedDevice> {
    let hints_json =
        serde_json::to_string(&device.hints).map_err(|e| PersistenceError::InvalidRecord {
            message: format!("Failed to encode paired_devices.hints: {}", e),
        })?;
    let paired_at = now_ms();

    conn.execute(
        "INSERT INTO paired_devices \
         (device_id, public_key, display_name, role, trust_level, hints, paired_at, last_seen_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, NULL);",
        (
            device.device_id.clone(),
            device.public_key.clone(),
            device.display_name.clone(),
            device.role.as_str(),
            device.trust_level.clone(),
            hints_json,
            paired_at,
        ),
    )
    .await?;

    get_paired_device(conn, &device.device_id)
        .await?
        .ok_or_else(|| PersistenceError::InvalidRecord {
            message: format!(
                "Paired device {} vanished immediately after insert",
                device.device_id
            ),
        })
}

/// Lists every paired peer, oldest pairing first.
pub async fn list_paired_devices(conn: &Connection) -> Result<Vec<PairedDevice>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {} FROM paired_devices ORDER BY paired_at ASC;",
                SELECT_COLUMNS
            ),
            (),
        )
        .await?;

    let mut devices = Vec::new();
    while let Some(row) = rows.next().await? {
        devices.push(decode_row(&row)?);
    }
    Ok(devices)
}

/// Looks up one paired peer by device id.
pub async fn get_paired_device(conn: &Connection, device_id: &str) -> Result<Option<PairedDevice>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {} FROM paired_devices WHERE device_id = ? LIMIT 1;",
                SELECT_COLUMNS
            ),
            (device_id.to_string(),),
        )
        .await?;

    match rows.next().await? {
        Some(row) => Ok(Some(decode_row(&row)?)),
        None => Ok(None),
    }
}

/// Replaces the address-hint list of a paired peer.
///
/// Returns the number of rows updated. This is the only mutable column
/// besides `last_seen_at`.
pub async fn update_paired_device_hints(
    conn: &Connection,
    device_id: &str,
    hints: &[serde_json::Value],
) -> Result<u64> {
    let hints_json = serde_json::to_string(hints).map_err(|e| PersistenceError::InvalidRecord {
        message: format!("Failed to encode paired_devices.hints: {}", e),
    })?;

    let affected = conn
        .execute(
            "UPDATE paired_devices SET hints = ? WHERE device_id = ?;",
            (hints_json, device_id.to_string()),
        )
        .await?;
    Ok(affected)
}

/// Writes `last_seen_at` for a paired peer.
///
/// RULE-DB-04: `last_seen_at` is written on connect and on disconnect only.
/// This is the only write path to the column, and there is deliberately no
/// "touch" entry point, so a heartbeat cannot rewrite the column and turn a
/// connect-time field into a hot column.
pub async fn mark_paired_device_seen(conn: &Connection, device_id: &str) -> Result<u64> {
    let affected = conn
        .execute(
            "UPDATE paired_devices SET last_seen_at = ? WHERE device_id = ?;",
            (now_ms(), device_id.to_string()),
        )
        .await?;
    Ok(affected)
}

/// Deletes a pairing. This is the complete and immediate revocation of a peer.
///
/// There is no soft delete and no disabled flag: the row is gone, and the peer
/// is unknown to the trust store from that moment on.
pub async fn delete_paired_device(conn: &Connection, device_id: &str) -> Result<bool> {
    let affected = conn
        .execute(
            "DELETE FROM paired_devices WHERE device_id = ?;",
            (device_id.to_string(),),
        )
        .await?;
    Ok(affected > 0)
}
