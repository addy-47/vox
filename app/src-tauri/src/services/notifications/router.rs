use serde::{Deserialize, Serialize};

use crate::core::{error::PipelineImpact, events::Severity};
use super::Action;

/// Resolved destination surface for a notification event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryChannel {
    /// Delivered exclusively to the floating HUD overlay window for 3s; zero SQLite persistence.
    ToastOnly,
    /// Persisted silently to the SQLite drawer; zero floating HUD overlay window.
    NotificationOnly,
    /// Dispatched simultaneously to the floating HUD overlay and persisted to SQLite.
    ToastAndNotification,
}

/// Resolves the delivery channel using the deterministic 3D truth table:
/// (PipelineImpact, Severity, Action) -> DeliveryChannel.
pub fn resolve_channel(
    impact: Option<PipelineImpact>,
    severity: Severity,
    action: &Action,
) -> DeliveryChannel {
    match action {
        Action::Transient => DeliveryChannel::ToastOnly,
        Action::Receipt => DeliveryChannel::NotificationOnly,
        Action::Interactive(_) => match (impact, severity) {
            (Some(PipelineImpact::SessionHalted), _) | (_, Severity::Critical) => {
                DeliveryChannel::ToastAndNotification
            }
            (Some(PipelineImpact::TurnAborted), Severity::Warning) => {
                DeliveryChannel::ToastAndNotification
            }
            _ => DeliveryChannel::NotificationOnly,
        },
    }
}
