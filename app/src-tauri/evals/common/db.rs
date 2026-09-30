//! ============================================================================
//! evals/common/db.rs — Isolated Evaluation Database Guard & Seeder
//! ============================================================================

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use turso::Connection;
use vox_lib::persistence::{schema::run_migrations, VoxDb};

use super::datasets::SessionTurn;

/// Database guard wrapping a dedicated SQLite database file for an evaluation run.
#[allow(dead_code)]
pub struct EvalDbGuard {
    pub db_path: PathBuf,
    db: Arc<VoxDb>,
}

impl EvalDbGuard {
    /// Opens or creates an isolated evaluation database at `db_path` and executes schema migrations.
    pub async fn new(db_path: &Path) -> Result<Self> {
        let vox_db = VoxDb::open(db_path)
            .await
            .map_err(|e| anyhow!("Failed to open eval database at {:?}: {}", db_path, e))?;

        let conn = vox_db
            .connect()
            .map_err(|e| anyhow!("Failed to connect to eval database: {}", e))?;

        run_migrations(&conn)
            .await
            .map_err(|e| anyhow!("Failed to run schema migrations on eval database: {}", e))?;

        Ok(Self {
            db_path: db_path.to_path_buf(),
            db: Arc::new(vox_db),
        })
    }

    /// Obtains a new connection to the evaluation database.
    pub fn conn(&self) -> Result<Connection> {
        self.db
            .connect()
            .map_err(|e| anyhow!("Failed to vend eval connection: {}", e))
    }

    /// Seeds conversation turns into the `sessions` and `turns` tables.
    pub async fn seed_turns(
        &self,
        session_id: i64,
        title: &str,
        turns: &[SessionTurn],
        simulated_start_ms: i64,
    ) -> Result<()> {
        let conn = self.conn()?;
        let now = if simulated_start_ms > 0 {
            simulated_start_ms
        } else {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        };

        conn.execute(
            "INSERT OR REPLACE INTO sessions (id, project_id, title, created_at, updated_at) \
             VALUES (?, 'default', ?, ?, ?)",
            (session_id, title, now, now),
        )
        .await
        .map_err(|e| anyhow!("Failed to insert session row {}: {}", session_id, e))?;

        for (idx, turn) in turns.iter().enumerate() {
            let turn_ts = now + (idx as i64 * 5_000); // 5-second simulated spacing per turn
            conn.execute(
                "INSERT OR REPLACE INTO turns (session_id, turn_id, user_text, assistant_text, created_at) \
                 VALUES (?, ?, ?, ?, ?)",
                (
                    session_id,
                    turn.turn as i64,
                    turn.user.clone(),
                    turn.assistant.clone(),
                    turn_ts,
                ),
            )
            .await
            .map_err(|e| anyhow!("Failed to insert turn {} for session {}: {}", turn.turn, session_id, e))?;
        }

        Ok(())
    }
}
