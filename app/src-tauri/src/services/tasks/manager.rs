use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{mpsc::Sender, Arc},
};

use parking_lot::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{
    core::events::VoxEvent,
    persistence::VoxDb,
};

/// Active task record tracking runtime process group metadata and cancellation tokens.
#[derive(Debug, Clone)]
pub struct ActiveTaskRecord {
    pub task_id: String,
    pub session_id: i64,
    pub tool_call_id: Option<String>,
    pub command: String,
    pub pid: u32,
    pub log_path: PathBuf,
    pub started_at: u64,
    pub is_promoted: bool,
    pub cancel_token: CancellationToken,
}

/// Asynchronous background process group manager and supervisor.
pub struct BackgroundTaskManager {
    active_tasks: Arc<RwLock<HashMap<String, ActiveTaskRecord>>>,
    db: Arc<VoxDb>,
    event_tx: Mutex<Option<Sender<VoxEvent>>>,
}

impl BackgroundTaskManager {
    /// Initializes a new BackgroundTaskManager.
    pub fn new(db: Arc<VoxDb>, event_tx: Option<Sender<VoxEvent>>) -> Self {
        Self {
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            db,
            event_tx: Mutex::new(event_tx),
        }
    }

    /// Accessor for the underlying VoxDb reference.
    pub fn db(&self) -> &Arc<VoxDb> {
        &self.db
    }

    /// Sets or updates the pipeline event transmitter.
    pub fn set_event_tx(&self, event_tx: Sender<VoxEvent>) {
        *self.event_tx.lock() = Some(event_tx);
    }

    /// Returns the number of active background tasks for the specified session.
    pub fn active_tasks_count(&self, session_id: i64) -> usize {
        self.active_tasks
            .read()
            .values()
            .filter(|t| t.session_id == session_id)
            .count()
    }

    /// Returns whether any running background tasks exist for the specified session.
    pub fn has_running_tasks(&self, session_id: i64) -> bool {
        self.active_tasks
            .read()
            .values()
            .any(|t| t.session_id == session_id)
    }

    /// Terminates a specific background task by ID.
    pub fn kill_task(&self, task_id: &str) -> anyhow::Result<()> {
        if let Some(task) = self.active_tasks.write().remove(task_id) {
            task.cancel_token.cancel();
        }
        Ok(())
    }

    /// Terminates all active background tasks associated with a session.
    pub fn kill_all(&self, session_id: i64) {
        let mut tasks = self.active_tasks.write();
        tasks.retain(|_, task| {
            if task.session_id == session_id {
                task.cancel_token.cancel();
                false
            } else {
                true
            }
        });
    }

    /// Marks an active task as promoted to the background.
    pub fn mark_promoted(&self, task_id: &str) {
        if let Some(task) = self.active_tasks.write().get_mut(task_id) {
            task.is_promoted = true;
        }
    }

    /// Reads up to `max_lines` from the tail of a task log file.
    pub fn read_log_tail(&self, log_path: &Path, max_lines: usize) -> anyhow::Result<String> {
        if !log_path.exists() {
            return Ok(String::new());
        }
        let content = std::fs::read_to_string(log_path)?;
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(max_lines);
        Ok(lines[start..].join("\n"))
    }

    /// Prunes an unpromoted task from the active registry without firing TaskFinished.
    pub fn prune_unpromoted(&self, task_id: &str) {
        let mut tasks = self.active_tasks.write();
        if let Some(task) = tasks.get(task_id) {
            if !task.is_promoted {
                tasks.remove(task_id);
            }
        }
    }
}
