use crate::monitoring::snapshot::RuntimeSnapshot;
use crate::monitoring::MAX_SNAPSHOT_HISTORY;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

/// Shared thread-safe state for runtime monitoring.
pub struct MonitoringState {
    history: Arc<RwLock<VecDeque<RuntimeSnapshot>>>,
    latest: Arc<RwLock<Option<RuntimeSnapshot>>>,
}

impl MonitoringState {
    /// Creates a new empty MonitoringState instance.
    pub fn new() -> Self {
        Self {
            history: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_SNAPSHOT_HISTORY))),
            latest: Arc::new(RwLock::new(None)),
        }
    }

    /// Adds a new snapshot to the history, evicting the oldest if capacity is exceeded.
    pub fn push(&self, snapshot: RuntimeSnapshot) {
        if let Ok(mut latest) = self.latest.write() {
            *latest = Some(snapshot.clone());
        }

        if let Ok(mut history) = self.history.write() {
            history.push_back(snapshot);
            if history.len() > MAX_SNAPSHOT_HISTORY {
                history.pop_front();
            }
        }
    }

    /// Gets the most recent snapshot if available.
    pub fn get_latest(&self) -> Option<RuntimeSnapshot> {
        self.latest.read().ok().and_then(|guard| guard.clone())
    }

    /// Gets the full history of recorded snapshots.
    pub fn get_history(&self) -> Vec<RuntimeSnapshot> {
        self.history
            .read()
            .map(|guard| guard.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clears all recorded snapshot history and latest state.
    pub fn clear(&self) {
        if let Ok(mut history) = self.history.write() {
            history.clear();
        }
        if let Ok(mut latest) = self.latest.write() {
            *latest = None;
        }
    }
}

impl Default for MonitoringState {
    fn default() -> Self {
        Self::new()
    }
}

