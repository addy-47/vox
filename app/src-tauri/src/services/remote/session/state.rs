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
    /// General or unknown OS/network transport error.
    Unknown,
}

/// The canonical connection state of the remote control link.
///
/// Remote-control spec §9.1: Single source of truth for remote state.
/// Zero boolean flags may shadow it anywhere in the backend codebase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteLinkState {
    /// Remote control switched off (host only).
    Disabled,
    /// No active link.
    Disconnected,
    /// Handshake and Hello negotiation in progress.
    Connecting,
    /// Link is up, authenticated and active.
    Connected,
    /// Link dropped unexpectedly; reconnecting with backoff.
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
            Self::Disconnected => "Disconnected",
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
/// Matches `ipc-spec.md` §4.8.
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
    /// Connected peer device id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// Connected peer human-readable display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}
