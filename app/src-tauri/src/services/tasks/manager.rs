use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{mpsc::Sender, Arc},
    time::{SystemTime, UNIX_EPOCH},
};

use parking_lot::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::{
    core::events::{TaskDto, TaskStatus, VoxEvent},
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

/// Handle returned on spawning a supervised background child process.
pub struct SupervisedTaskHandle {
    pub task_id: String,
    pub session_id: i64,
    pub command: String,
    pub log_path: PathBuf,
    pub completion_rx: tokio::sync::oneshot::Receiver<TaskStatus>,
}

/// Resolves the canonical shell and argument vectors across supported platforms.
pub fn resolve_shell_command(command: &str) -> (String, Vec<String>) {
    #[cfg(target_os = "android")]
    {
        (
            "/system/bin/sh".to_string(),
            vec!["-c".to_string(), command.to_string()],
        )
    }
    #[cfg(all(unix, not(target_os = "android")))]
    {
        (
            "bash".to_string(),
            vec!["-c".to_string(), command.to_string()],
        )
    }
    #[cfg(windows)]
    {
        (
            "cmd".to_string(),
            vec!["/C".to_string(), command.to_string()],
        )
    }
}

/// Sends SIGKILL to the entire process group identified by negative PGID.
#[cfg(unix)]
pub fn kill_process_group(pid: u32) {
    if pid > 0 {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

/// Kills process tree on Windows via taskkill.
#[cfg(windows)]
pub fn kill_process_group(pid: u32) {
    if pid > 0 {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
    }
}

/// Asynchronous background process group manager and supervisor.
pub struct BackgroundTaskManager {
    active_tasks: Arc<RwLock<HashMap<String, ActiveTaskRecord>>>,
    db: Arc<VoxDb>,
    event_tx: Arc<Mutex<Option<Sender<VoxEvent>>>>,
}

impl BackgroundTaskManager {
    /// Initializes a new BackgroundTaskManager.
    pub fn new(db: Arc<VoxDb>, event_tx: Option<Sender<VoxEvent>>) -> Self {
        Self {
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            db,
            event_tx: Arc::new(Mutex::new(event_tx)),
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

    /// Spawns an isolated child process group and attaches a Tokio supervisor task.
    pub fn spawn_supervised_task(
        &self,
        session_id: i64,
        tool_call_id: Option<String>,
        command: String,
    ) -> anyhow::Result<SupervisedTaskHandle> {
        let task_id = format!(
            "task_{}_{}",
            session_id,
            &uuid::Uuid::new_v4().to_string()[..8]
        );
        let tasks_dir = crate::utils::paths::session_tasks_dir(session_id);
        std::fs::create_dir_all(&tasks_dir)?;

        let log_path = crate::utils::paths::task_log_path(session_id, &task_id);
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)?;
        let log_file_err = log_file.try_clone()?;

        let (shell, args) = resolve_shell_command(&command);
        let mut cmd = tokio::process::Command::new(shell);
        cmd.args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::from(log_file))
            .stderr(std::process::Stdio::from(log_file_err));

        #[cfg(unix)]
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }
        #[cfg(windows)]
        {
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
        }

        let mut child = cmd.spawn()?;
        let pid = child.id().unwrap_or(0);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let cancel_token = CancellationToken::new();

        let record = ActiveTaskRecord {
            task_id: task_id.clone(),
            session_id,
            tool_call_id: tool_call_id.clone(),
            command: command.clone(),
            pid,
            log_path: log_path.clone(),
            started_at: now,
            is_promoted: false,
            cancel_token: cancel_token.clone(),
        };
        self.active_tasks.write().insert(task_id.clone(), record);

        let (completion_tx, completion_rx) = tokio::sync::oneshot::channel();
        let active_tasks_clone = Arc::clone(&self.active_tasks);
        let db_clone = Arc::clone(&self.db);
        let event_tx_clone = Arc::clone(&self.event_tx);
        let task_id_clone = task_id.clone();
        let started_at = now;

        tauri::async_runtime::spawn(async move {
            let status = tokio::select! {
                _ = cancel_token.cancelled() => {
                    kill_process_group(pid);
                    let _ = child.wait().await;
                    let killed_at = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    TaskStatus::Killed { killed_at }
                }
                res = child.wait() => {
                    let completed_at = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let duration_ms = completed_at.saturating_sub(started_at);
                    match res {
                        Ok(exit_status) => {
                            let exit_code = exit_status.code().unwrap_or(-1);
                            TaskStatus::Completed {
                                exit_code,
                                duration_ms,
                                completed_at,
                            }
                        }
                        Err(e) => {
                            TaskStatus::Failed {
                                error: e.to_string(),
                                duration_ms,
                                failed_at: completed_at,
                            }
                        }
                    }
                }
            };

            let is_promoted = {
                let mut tasks = active_tasks_clone.write();
                if let Some(t) = tasks.remove(&task_id_clone) {
                    t.is_promoted
                } else {
                    false
                }
            };

            if is_promoted {
                if let Ok(conn) = db_clone.connect() {
                    let _ = crate::persistence::tasks::record_task_finished(
                        &conn,
                        &task_id_clone,
                        session_id,
                        &status,
                    )
                    .await;
                }

                let tx_opt = event_tx_clone.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(VoxEvent::TaskFinished {
                        task_id: task_id_clone,
                        session_id,
                        status: status.clone(),
                    });
                }
            }

            let _ = completion_tx.send(status);
        });

        Ok(SupervisedTaskHandle {
            task_id,
            session_id,
            command,
            log_path,
            completion_rx,
        })
    }

    /// Terminates a specific background task by ID.
    pub async fn kill_task(&self, task_id: &str) -> anyhow::Result<()> {
        let task_opt = self.active_tasks.write().remove(task_id);
        if let Some(task) = task_opt {
            task.cancel_token.cancel();
            kill_process_group(task.pid);
        }
        if let Ok(conn) = self.db.connect() {
            let _ = crate::persistence::tasks::mark_task_killed(&conn, task_id).await;
        }
        Ok(())
    }

    /// Terminates all active background tasks associated with a session.
    pub fn kill_all(&self, session_id: i64) {
        let mut to_kill = Vec::new();
        {
            let mut tasks = self.active_tasks.write();
            tasks.retain(|_, task| {
                if task.session_id == session_id {
                    to_kill.push(task.clone());
                    false
                } else {
                    true
                }
            });
        }
        for task in to_kill {
            task.cancel_token.cancel();
            kill_process_group(task.pid);
        }
        let db = Arc::clone(&self.db);
        tauri::async_runtime::spawn(async move {
            if let Ok(conn) = db.connect() {
                let _ = crate::persistence::tasks::mark_session_tasks_killed(&conn, session_id).await;
            }
        });
    }

    /// Marks an active task as promoted to the background and persists its initial running state.
    pub async fn mark_promoted(&self, task_id: &str) {
        let record_opt = {
            let mut tasks = self.active_tasks.write();
            if let Some(task) = tasks.get_mut(task_id) {
                task.is_promoted = true;
                Some((
                    task.task_id.clone(),
                    task.session_id,
                    task.tool_call_id.clone(),
                    task.command.clone(),
                    task.log_path.to_string_lossy().to_string(),
                    task.started_at as i64,
                ))
            } else {
                None
            }
        };

        if let Some((task_id_str, session_id, tool_call_id, command, log_path, started_at)) =
            record_opt
        {
            if let Ok(conn) = self.db.connect() {
                let _ = crate::persistence::tasks::record_task_promoted(
                    &conn,
                    &task_id_str,
                    session_id,
                    tool_call_id.as_deref(),
                    &command,
                    &log_path,
                    started_at,
                )
                .await;
            }
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

    /// Lists active background tasks, optionally filtered by session ID.
    pub fn list_tasks(&self, session_id: Option<i64>) -> Vec<TaskDto> {
        self.active_tasks
            .read()
            .values()
            .filter(|t| session_id.is_none_or(|sid| t.session_id == sid))
            .map(|t| TaskDto {
                task_id: t.task_id.clone(),
                session_id: t.session_id,
                command: t.command.clone(),
                status: "running".to_string(),
                exit_code: None,
                error: None,
                log_path: t.log_path.to_string_lossy().to_string(),
                started_at: t.started_at,
                completed_at: None,
                duration_ms: None,
            })
            .collect()
    }
}
