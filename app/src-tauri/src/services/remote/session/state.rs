use serde::{Deserialize, Serialize};

/// Typed classification of remote connection failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteFailureKind {
    /// Connection dial or response timed out.
    Timeout,
    /// Host port unreachable, connection refused, or no route to host.
    Unreachable,
    /// Pairing request was explicitly rejected by the host user.
    RejectedByHost,
    /// Pairing token was expired, already consumed, or invalid.
    InvalidToken,
    /// Protocol or handshake version mismatch.
    VersionMismatch,
    /// Peer pairing was revoked (terminal, no retry).
    Revoked,
    /// General or unknown OS/network transport error.
    Unknown,
}

/// The canonical connection state of the remote control link.
///
/// Remote-control spec §9.1 and §5.4: two strictly orthogonal state machines.
/// `PairingState` (Tier 1, ephemeral) is separate from `RemoteLinkState`
/// (Tier 2, persistent transport). Zero boolean flags may shadow it anywhere.
///
/// `Disconnected` is intentionally absent. It conflated two distinct conditions
/// and caused the UI to infer "connected" from DB row presence rather than
/// live transport state. Use `Unpaired` (0 rows in paired_devices) or `Paired`
/// (≥1 row, link idle) instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteLinkState {
    /// Remote control switched off (host only).
    Disabled,
    /// No device in `paired_devices`. Pre-pairing. Spec §6.2.
    Unpaired,
    /// At least one device in `paired_devices`; transport link is idle.
    /// Spec §6.2: the Rocket button from this state starts Tier 2.
    Paired,
    /// Handshake and Hello negotiation in progress.
    Connecting,
    /// Link is up, authenticated, and active. Audio / IPC may flow.
    Connected,
    /// Link dropped unexpectedly; Controller is reconnecting with backoff.
    Reconnecting,
    /// Terminal failure with detailed message and classified kind.
    Failed {
        reason: String,
        kind: RemoteFailureKind,
    },
}

impl RemoteLinkState {
    /// Convenience constructor for failed states.
    pub fn failed(reason: impl Into<String>, kind: RemoteFailureKind) -> Self {
        Self::Failed {
            reason: reason.into(),
            kind,
        }
    }

    /// Returns the canonical state string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Unpaired => "Unpaired",
            Self::Paired => "Paired",
            Self::Connecting => "Connecting",
            Self::Connected => "Connected",
            Self::Reconnecting => "Reconnecting",
            Self::Failed { .. } => "Failed",
        }
    }

    /// Converts this state into a serializable DTO for IPC and frontend hydration.
    pub fn to_dto(&self, device_id: Option<&str>, display_name: Option<&str>) -> RemoteStateDto {
        RemoteStateDto {
            link: self.as_str().to_string(),
            failure: match self {
                Self::Failed { reason, .. } => Some(reason.clone()),
                _ => None,
            },
            failure_kind: match self {
                Self::Failed { kind, .. } => Some(*kind),
                _ => None,
            },
            device_id: device_id.map(|s| s.to_string()),
            display_name: display_name.map(|s| s.to_string()),
        }
    }
}

/// DTO representing remote connection state for IPC and frontend hydration.
///
/// Matches `ipc-spec.md` §4.8 and remote-control spec §13.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStateDto {
    /// Canonical link state name.
    pub link: String,
    /// Detailed failure reason when `link == "Failed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    /// Typed classification of failure when `link == "Failed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<RemoteFailureKind>,
    /// Connected peer device id. Present when `link == "Paired"` or `"Connected"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// Connected peer human-readable display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

// ─── Tier 1: Ephemeral Pairing State Machine ────────────────────────────────
//
// Remote-control spec §5.5: PairingState is strictly orthogonal to
// RemoteLinkState. It exists only during first-time device setup and resets
// to Idle once the flow resolves (Approved or Denied).

/// Ephemeral pairing workflow state. Spec §5.5.
///
/// Lifecycle: Idle → AwaitingApproval → Approved | Denied → (reset) Idle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingState {
    /// No pairing flow active.
    Idle,
    /// Token verified; host user approval prompt active (30s deadline).
    AwaitingApproval {
        device_id: String,
        display_name: String,
        /// Unix millisecond timestamp of the approval deadline.
        expires_at_ms: i64,
    },
    /// Human approved; peer written to DB. `RemoteLinkState` → `Paired`.
    Approved { device_id: String },
    /// Explicitly rejected, token invalid, or timed out.
    Denied { reason: String },
}

impl PairingState {
    /// Returns the canonical state string for IPC serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::AwaitingApproval { .. } => "AwaitingApproval",
            Self::Approved { .. } => "Approved",
            Self::Denied { .. } => "Denied",
        }
    }

    /// Converts into a serializable DTO for `remote_pairing_state_changed`.
    pub fn to_dto(&self) -> PairingStateDto {
        PairingStateDto {
            state: self.as_str().to_string(),
            device_id: match self {
                Self::AwaitingApproval { device_id, .. } | Self::Approved { device_id } => {
                    Some(device_id.clone())
                }
                _ => None,
            },
            display_name: match self {
                Self::AwaitingApproval { display_name, .. } => Some(display_name.clone()),
                _ => None,
            },
            expires_at_ms: match self {
                Self::AwaitingApproval { expires_at_ms, .. } => Some(*expires_at_ms),
                _ => None,
            },
            failure_reason: match self {
                Self::Denied { reason } => Some(reason.clone()),
                _ => None,
            },
        }
    }
}

/// DTO for the `remote_pairing_state_changed` IPC event. Spec §13.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingStateDto {
    /// "Idle" | "AwaitingApproval" | "Approved" | "Denied"
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Unix millisecond timestamp of approval deadline (AwaitingApproval only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}
