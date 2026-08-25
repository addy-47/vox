use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use serde::Serialize;
use tauri::State;

/// Retrieves the current in-memory transcript history (tray ephemeral buffer).
#[tauri::command]
pub async fn get_transcript_history(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<String>, String> {
    let history = state.pipeline.transcript_history.lock();
    Ok(history.iter().cloned().collect())
}

/// Commits a completed session's full text to the ephemeral history buffer.
#[tauri::command]
pub async fn commit_session_to_history(
    text: String,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<String>, String> {
    if !text.trim().is_empty() {
        let mut history = state.pipeline.transcript_history.lock();
        if history.front() != Some(&text) {
            history.push_front(text);
            let limit = {
                let settings = state.settings.read().unwrap();
                settings.history.tray_history_limit as usize
            };
            while history.len() > limit {
                history.pop_back();
            }
        }
    }
    let history = state.pipeline.transcript_history.lock();
    Ok(history.iter().cloned().collect())
}

/// Representation of a stored conversation session.
#[derive(Debug, Serialize, Clone)]
pub struct SessionRow {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub turn_count: i64,
    pub first_message: Option<String>,
}

/// Representation of a single conversation turn in a session.
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
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT s.id, s.started_at, s.ended_at, s.turn_count,
                    (SELECT t.user_text FROM turns t WHERE t.session_id = s.id ORDER BY t.turn_id ASC LIMIT 1) as first_message
             FROM sessions s
             ORDER BY s.started_at DESC LIMIT 100",
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut sessions = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        sessions.push(SessionRow {
            id: row.get(0).map_err(|e| e.to_string())?,
            started_at: row.get(1).map_err(|e| e.to_string())?,
            ended_at: row.get(2).map_err(|e| e.to_string())?,
            turn_count: row.get(3).map_err(|e| e.to_string())?,
            first_message: row.get(4).map_err(|e| e.to_string())?,
        });
    }

    Ok(sessions)
}

/// Returns all turns for a given session, oldest first.
#[tauri::command]
pub async fn get_turns(
    session_id: i64,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<TurnRow>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let _ = state;

    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT id, session_id, turn_id, user_text, assistant_text, stt_latency_ms, ttft_ms, created_at
             FROM turns WHERE session_id = ? ORDER BY created_at ASC",
            (session_id,),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut turns = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        turns.push(TurnRow {
            id: row.get(0).map_err(|e| e.to_string())?,
            session_id: row.get(1).map_err(|e| e.to_string())?,
            turn_id: row.get(2).map_err(|e| e.to_string())?,
            user_text: row.get(3).map_err(|e| e.to_string())?,
            assistant_text: row.get(4).map_err(|e| e.to_string())?,
            stt_latency_ms: row.get(5).map_err(|e| e.to_string())?,
            ttft_ms: row.get(6).map_err(|e| e.to_string())?,
            created_at: row.get(7).map_err(|e| e.to_string())?,
        });
    }

    Ok(turns)
}

/// Deletes a session and all its turns (CASCADE).
#[tauri::command]
pub async fn delete_session(
    id: i64,
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<(), String> {
    let db_path = crate::utils::paths::get().db.clone();
    let _ = state;

    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    conn.execute("DELETE FROM sessions WHERE id = ?", (id,))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
