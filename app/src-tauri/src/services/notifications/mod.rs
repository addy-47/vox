pub mod actions;
pub mod service;

pub use actions::execute_notification_action;
pub use service::notify;
use crate::core::{error::PipelineImpact, events::Severity};

use serde::{Deserialize, Serialize};

/// Closed domain category taxonomy for system notifications and alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationCategory {
    SessionCompaction,
    MemoryConsolidation,
    Pipeline,
    Dictation,
    Hardware,
    Models,
    Storage,
}

impl NotificationCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SessionCompaction => "session_compaction",
            Self::MemoryConsolidation => "memory_consolidation",
            Self::Pipeline => "pipeline",
            Self::Dictation => "dictation",
            Self::Hardware => "hardware",
            Self::Models => "models",
            Self::Storage => "storage",
        }
    }
}

impl From<&str> for NotificationCategory {
    fn from(s: &str) -> Self {
        match s {
            "session_compaction" => Self::SessionCompaction,
            "memory_consolidation" => Self::MemoryConsolidation,
            "pipeline" => Self::Pipeline,
            "dictation" => Self::Dictation,
            "hardware" => Self::Hardware,
            "models" => Self::Models,
            "storage" => Self::Storage,
            _ => Self::Pipeline,
        }
    }
}

/// Strongly-typed action parameters for polymorphic interactive tasks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ActionPayload {
    CompactSession {
        session_id: i64,
    },
    ConsolidateMemory,
    Retry {
        operation: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        resource_id: Option<String>,
    },
    Navigate {
        target: String,
    },
}

/// Retention and remediation contract governing storage and UI interaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Ephemeral feedback; never committed to SQLite; auto-dismisses after 3s.
    Transient,
    /// Passive historical record; committed to SQLite; zero floating HUD overlay.
    Receipt,
    /// Actionable task card with interactive button; dual-emits to HUD on Critical/TurnAborted.
    Interactive(ActionPayload),
}

/// Input parameters for dispatching a notification through the universal front door.
#[derive(Debug, Clone)]
pub struct NotificationParams<'a> {
    pub group_key: Option<&'a str>,
    pub category: NotificationCategory,
    pub severity: Severity,
    pub impact: Option<PipelineImpact>,
    pub action: Action,
    pub title: &'a str,
    pub message: &'a str,
    pub session_id: Option<i64>,
    pub metadata: Option<&'a str>,
    pub duration_ms: Option<u64>,
}

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