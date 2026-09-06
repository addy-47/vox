use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
};

use crate::monitoring::{snapshot::RuntimeSnapshot, MAX_SNAPSHOT_HISTORY};

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
        let mut latest = self.latest.write().unwrap_or_else(|e| e.into_inner());
        *latest = Some(snapshot.clone());

        let mut history = self.history.write().unwrap_or_else(|e| e.into_inner());
        history.push_back(snapshot);
        if history.len() > MAX_SNAPSHOT_HISTORY {
            history.pop_front();
        }
    }

    /// Gets the most recent snapshot if available.
    pub fn get_latest(&self) -> Option<RuntimeSnapshot> {
        let guard = self.latest.read().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    }

    /// Gets the full history of recorded snapshots.
    pub fn get_history(&self) -> Vec<RuntimeSnapshot> {
        let guard = self.history.read().unwrap_or_else(|e| e.into_inner());
        guard.iter().cloned().collect()
    }

    /// Clears all recorded snapshot history and latest state.
    pub fn clear(&self) {
        let mut history = self.history.write().unwrap_or_else(|e| e.into_inner());
        history.clear();
        let mut latest = self.latest.write().unwrap_or_else(|e| e.into_inner());
        *latest = None;
    }
}

impl Default for MonitoringState {
    fn default() -> Self {
        Self::new()
    }
}
