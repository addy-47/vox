//! ============================================================================
//! db.rs — Fresh eval-database setup + turn seeding for the memory ladder
//! ============================================================================
//! Category     : Evaluation (shared harness, not a runnable eval)
//! Component    : evals/common (vox_lib persistence API)
//! Prerequisites: None
//! Execution    : Included via #[path] from evals/memory_*_eval.rs
//! Metrics      : N/A
//! ============================================================================

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use turso::Connection;
use vox_lib::persistence::{schema::run_migrations, sessions::TurnRow, VoxDb};

use super::turns::DatasetTurn;

/// Opens a fresh Turso eval database at `path` and runs schema migrations.
/// The file must not exist yet; each rung writes its own DB file.
pub async fn open_fresh_eval_db(path: &std::path::Path) -> Result<(VoxDb, Connection)> {
    if path.exists() {
        anyhow::bail!(
            "Eval DB already exists at {} — refusing to overwrite",
            path.display()
        );
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create eval results dir {}", parent.display()))?;
    }
    let db = VoxDb::open(path).await.context("VoxDb::open failed")?;
    let conn = db.connect().context("VoxDb::connect failed")?;
    run_migrations(&conn)
        .await
        .context("run_migrations failed")?;
    Ok((db, conn))
}

/// Opens an existing rung DB file produced by a previous rung (ladder input).
pub async fn create_session_at_timestamp(
    conn: &Connection,
    timestamp_ms: i64,
    project_id: Option<&str>,
) -> Result<i64> {
    let project_id = project_id.unwrap_or("default");
    let mut rows = conn
        .query(
            "INSERT INTO sessions (project_id, is_pinned, created_at, updated_at)
             VALUES (?, 0, ?, ?) RETURNING id;",
            (project_id, timestamp_ms, timestamp_ms),
        )
        .await?;
    let row = rows
        .next()
        .await?
        .context("No session id returned after creating session")?;
    Ok(row.get(0)?)
}

pub async fn persist_turn_at_timestamp(
    conn: &Connection,
    session_id: i64,
    turn_id: u32,
    user_text: &str,
    assistant_text: &str,
    timestamp_ms: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at)
         VALUES (?, ?, ?, ?, ?);",
        (
            session_id,
            i64::from(turn_id),
            user_text,
            assistant_text,
            timestamp_ms,
        ),
    )
    .await?;
    conn.execute(
        "UPDATE sessions SET updated_at = ? WHERE id = ?;",
        (timestamp_ms, session_id),
    )
    .await?;
    Ok(())
}

pub async fn open_existing_eval_db(path: &std::path::Path) -> Result<(VoxDb, Connection)> {
    if !path.exists() {
        anyhow::bail!(
            "Ladder input DB missing at {} — run the previous rung first",
            path.display()
        );
    }
    let db = VoxDb::open(path).await.context("VoxDb::open failed")?;
    let conn = db.connect().context("VoxDb::connect failed")?;
    Ok((db, conn))
}

/// Flushes hot journal content into the main database file so a plain file
/// copy carries the full state. Must run before any ladder handoff copy —
/// Turso keeps recent writes in sidecar files that `fs::copy` would miss.
pub async fn checkpoint_source_db(path: &std::path::Path) -> Result<()> {
    let (db, conn) = open_existing_eval_db(path).await?;
    let mut rows = conn
        .query("PRAGMA wal_checkpoint(TRUNCATE);", ())
        .await
        .context("wal_checkpoint failed")?;
    while rows
        .next()
        .await
        .context("wal_checkpoint row failed")?
        .is_some()
    {}
    drop(conn);
    drop(db);
    Ok(())
}

/// Inserts a session row plus one `turns` row per fixture turn (mirrors the
/// production persistence path where turns land in the DB as they complete).
/// Returns the session id and the `TurnRow`s for `seed_continuation`.
pub async fn seed_session_with_turns(
    conn: &Connection,
    turns: &[DatasetTurn],
) -> Result<(i64, Vec<TurnRow>)> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    // Scope the read cursor: Turso refuses a write on a connection with a
    // statement still in progress, so `rows` must drop before the inserts.
    let session_id: i64 = {
        let mut rows = conn
            .query(
                "INSERT INTO sessions (project_id, title, created_at, updated_at) VALUES ('default', 'Memory ladder eval', ?, ?) RETURNING id;",
                (now, now),
            )
            .await
            .context("Failed to insert eval session")?;
        rows.next()
            .await?
            .context("No session id returned")?
            .get(0)?
    };

    let mut turn_rows = Vec::with_capacity(turns.len());
    for t in turns {
        conn.execute(
            "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at) VALUES (?, ?, ?, ?, ?)",
            (
                session_id,
                t.turn as i64,
                t.user.clone(),
                t.assistant.clone(),
                now,
            ),
        )
        .await
        .with_context(|| format!("Failed to insert turn {}", t.turn))?;
        turn_rows.push(TurnRow {
            id: 0,
            session_id,
            turn_id: t.turn,
            user_text: t.user.clone(),
            assistant_text: t.assistant.clone(),
            created_at: now,
        });
    }
    Ok((session_id, turn_rows))
}
