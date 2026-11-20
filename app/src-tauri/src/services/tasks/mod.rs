//! Background task execution, process group supervision, and lifecycle persistence.

pub mod manager;

pub use manager::{ActiveTaskRecord, BackgroundTaskManager, PromotionOutcome};
