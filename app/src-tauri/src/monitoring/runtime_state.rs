use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use crate::monitoring::snapshot::RuntimeSnapshot;

/// Maximum number of snapshots to retain in memory (~60 seconds at 10Hz).
const MAX_SNAPSHOT_HISTORY: usize = 600;

/// Shared thread-safe state for runtime monitoring.
pub struct MonitoringState {
    /// Bounded ringbuffer of recent snapshots.
    /// Protected by RwLock: Collector acquires Write lock, IPC acquires Read lock.
    history: Arc<RwLock<VecDeque<RuntimeSnapshot>>>,
    /// The most recent snapshot for fast access.
    latest: Arc<RwLock<Option<RuntimeSnapshot>>>,
}

impl MonitoringState {
    pub fn new() -> Self {
        Self {
            history: Arc::new(RwLock::new(VecDeque::with_capacity(MAX_SNAPSHOT_HISTORY))),
            latest: Arc::new(RwLock::new(None)),
        }
    }

    /// Add a new snapshot to the history, evicting the oldest if necessary.
    pub fn push(&self, snapshot: RuntimeSnapshot) {
        // Update latest snapshot
        {
            let mut latest = self.latest.write().unwrap();
            *latest = Some(snapshot.clone());
        }

        // Append to history
        {
            let mut history = self.history.write().unwrap();
            history.push_back(snapshot);
            if history.len() > MAX_SNAPSHOT_HISTORY {
                history.pop_front();
            }
        }
    }

    /// Get the most recent snapshot.
    pub fn get_latest(&self) -> Option<RuntimeSnapshot> {
        self.latest.read().unwrap().clone()
    }

    /// Get the full history of snapshots.
    pub fn get_history(&self) -> Vec<RuntimeSnapshot> {
        self.history.read().unwrap().iter().cloned().collect()
    }

    /// Clear all history.
    pub fn clear(&self) {
        let mut history = self.history.write().unwrap();
        history.clear();
        let mut latest = self.latest.write().unwrap();
        *latest = None;
    }
}

impl Default for MonitoringState {
    fn default() -> Self {
        Self::new()
    }
}
