use serde::{Deserialize, Serialize};

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
    /// Terminal failure (e.g. Revoked, UnknownPeer, VersionMismatch, Timeout).
    Failed(String),
}

impl RemoteLinkState {
    /// Returns the canonical state string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::Connected => "Connected",
            Self::Reconnecting => "Reconnecting",
            Self::Failed(_) => "Failed",
        }
    }

    /// Converts this state into a serializable DTO for IPC and frontend hydration.
    pub fn to_dto(&self, device_id: Option<&str>, display_name: Option<&str>) -> RemoteStateDto {
        RemoteStateDto {
            link: self.as_str().to_string(),
            failure: match self {
                Self::Failed(reason) => Some(reason.clone()),
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
    /// Connected peer device id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    /// Connected peer human-readable display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}
