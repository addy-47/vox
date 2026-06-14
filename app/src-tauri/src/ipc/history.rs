use crate::core::state::AppState;
use serde::Serialize;
use tauri::State;

/// Retrieves the current in-memory transcript history (tray ephemeral buffer).
#[tauri::command]
pub async fn get_transcript_history(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<String>, String> {
    let history = state.pipeline.transcript_history.lock().unwrap();
    Ok(history.iter().cloned().collect())
}

/// Commits a completed session's full text to the ephemeral history buffer.
#[tauri::command]
pub async fn commit_session_to_history(
    text: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<String>, String> {
    if !text.trim().is_empty() {
        let mut history = state.pipeline.transcript_history.lock().unwrap();
        // Prevent duplicate consecutive entries
        if history.front() != Some(&text) {
            history.push_front(text);
            let limit = {
                let settings = state.settings.read().unwrap();
                settings.ui.tray_history_limit as usize
            };
            while history.len() > limit {
                history.pop_back();
            }
        }
    }
    let history = state.pipeline.transcript_history.lock().unwrap();
    Ok(history.iter().cloned().collect())
}

// ─── Persistence-Backed History Commands ─────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct SessionRow {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub turn_count: i64,
    pub first_message: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct TurnRow {
    pub id: i64,
    pub session_id: i64,
    pub turn_id: i32,
    pub user_text: String,
    pub assistant_text: String,
    pub stt_latency_ms: Option<i64>,
    pub ttft_ms: Option<i64>,
    pub created_at: i64,
}

/// Returns all sessions ordered by most recent first.
#[tauri::command]
pub async fn get_sessions(
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<SessionRow>, String> {
    let db_path = crate::utils::paths::get().db.clone();

    tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| format!("DB open failed: {}", e))?;

        let mut stmt = conn.prepare(
            "SELECT s.id, s.started_at, s.ended_at, s.turn_count,
                    (SELECT user_text FROM turns t WHERE t.session_id = s.id ORDER BY t.turn_id ASC LIMIT 1) as first_message
             FROM sessions s
             ORDER BY s.started_at DESC LIMIT 100"
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([], |row| {
            Ok(SessionRow {
                id:            row.get(0)?,
                started_at:    row.get(1)?,
                ended_at:      row.get(2)?,
                turn_count:    row.get(3)?,
                first_message: row.get(4)?,
            })
        }).map_err(|e| e.to_string())?
          .filter_map(|r| r.ok())
          .collect();

        Ok(rows)
    }).await.map_err(|e| e.to_string())?
}

/// Returns all turns for a given session, oldest first.
#[tauri::command]
pub async fn get_turns(
    session_id: i64,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<TurnRow>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let _ = state; // holds the managed state lifetime

    tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| format!("DB open failed: {}", e))?;

        let mut stmt = conn.prepare(
            "SELECT id, session_id, turn_id, user_text, assistant_text, stt_latency_ms, ttft_ms, created_at
             FROM turns WHERE session_id = ?1 ORDER BY created_at ASC"
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map(rusqlite::params![session_id], |row| {
            Ok(TurnRow {
                id:             row.get(0)?,
                session_id:     row.get(1)?,
                turn_id:        row.get(2)?,
                user_text:      row.get(3)?,
                assistant_text: row.get(4)?,
                stt_latency_ms: row.get(5)?,
                ttft_ms:        row.get(6)?,
                created_at:     row.get(7)?,
            })
        }).map_err(|e| e.to_string())?
          .filter_map(|r| r.ok())
          .collect();

        Ok(rows)
    }).await.map_err(|e| e.to_string())?
}

/// Deletes a session and all its turns (CASCADE).
#[tauri::command]
pub async fn delete_session(
    id: i64,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    let db_path = crate::utils::paths::get().db.clone();
    let _ = state; // holds the managed state lifetime

    tokio::task::spawn_blocking(move || {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("DB open failed: {}", e))?;

        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| e.to_string())?;

        conn.execute("DELETE FROM sessions WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
