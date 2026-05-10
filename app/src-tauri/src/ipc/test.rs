use tauri::{AppHandle, State, Manager};
use crate::core::state::{AppState, InteractionOwner};
use crate::services::stt::SttCommand;
use std::sync::atomic::Ordering;
use hound::WavReader;
use std::path::Path;

#[tauri::command]
pub async fn debug_harden_test(app: AppHandle, wav_path: String) -> Result<serde_json::Value, String> {
    log::info!("[HardenTest] Starting E2E persistence hardening test with: {}", wav_path);
    
    let state: State<'_, AppState> = app.state();
    
    // 1. Verify DB existence and schema
    let db_path = crate::utils::paths::get().db.clone();
    if !db_path.exists() {
        return Err("vox.db does not exist".to_string());
    }
    
    let initial_sessions = get_session_count(&db_path)?;
    log::info!("[HardenTest] Initial session count: {}", initial_sessions);

    // 2. Load WAV data
    let mut reader = WavReader::open(&wav_path).map_err(|e| format!("Failed to open WAV: {}", e))?;
    let spec = reader.spec();
    if spec.sample_rate != 16000 || spec.channels != 1 {
        return Err(format!("Test WAV must be 16kHz mono. Got: {}Hz, {}ch", spec.sample_rate, spec.channels));
    }
    
    let samples: Vec<f32> = reader.samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();
    
    log::info!("[HardenTest] Loaded {} samples from WAV", samples.len());

    // 3. Trigger engagement if not already
    if !state.pipeline.is_engaged.load(Ordering::Relaxed) {
        crate::ipc::pipeline::engage(state.clone(), app.clone()).await?;
    }

    let turn_id = state.pipeline.turn_id.load(Ordering::Relaxed);
    let conversation_id = state.conversation_id.load(Ordering::Relaxed);
    log::info!("[HardenTest] Injected turn_id: {}, conversation_id: {}", turn_id, conversation_id);

    // 4. Inject audio into STT
    {
        let lock = state.engine.lock().await;
        if let Some(engine) = lock.as_ref() {
            let _ = engine.stt_tx.send(SttCommand::Final(turn_id, InteractionOwner::MainWindow, samples));
        } else {
            return Err("Engine not running".to_string());
        }
    }

    // 5. Wait for turn completion
    let start_wait = std::time::Instant::now();
    let mut actual_turn_id = 0u32;
    let mut found = false;

    while start_wait.elapsed().as_secs() < 120 {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        
        // The orchestrator bumps the ID, so we check for both injected and injected+1
        if check_turn_exists(&db_path, conversation_id, turn_id).unwrap_or(false) {
            found = true; actual_turn_id = turn_id; break;
        }
        if check_turn_exists(&db_path, conversation_id, turn_id + 1).unwrap_or(false) {
            found = true; actual_turn_id = turn_id + 1; break;
        }
    }

    if !found {
        return Err("Test failed: Turn not found in DB after timeout.".to_string());
    }

    // 6. Verify data integrity
    let (user_text, assistant_text) = get_turn_data(&db_path, conversation_id, actual_turn_id)?;
    log::info!("[HardenTest] Verified turn in DB (ID: {}). User: {:?}, Assistant: {:?}", actual_turn_id, user_text, assistant_text);

    Ok(serde_json::json!({
        "status": "success",
        "conversation_id": conversation_id.to_string(),
        "turn_id": actual_turn_id,
        "user_text": user_text,
        "assistant_text": assistant_text,
    }))
}

fn get_session_count(db_path: &Path) -> Result<i64, String> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    Ok(count)
}

fn check_turn_exists(db_path: &Path, conv_id: u64, turn_id: u32) -> Result<bool, String> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM turns WHERE session_id = ?1 AND turn_id = ?2",
        rusqlite::params![conv_id as i64, turn_id],
        |r| r.get(0)
    ).map_err(|e| e.to_string())?;
    Ok(count > 0)
}

fn get_turn_data(db_path: &Path, conv_id: u64, turn_id: u32) -> Result<(String, String), String> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
    let (user, assistant): (String, String) = conn.query_row(
        "SELECT user_text, assistant_text FROM turns WHERE session_id = ?1 AND turn_id = ?2",
        rusqlite::params![conv_id as i64, turn_id],
        |r| Ok((r.get(0)?, r.get(1)?))
    ).map_err(|e| e.to_string())?;
    Ok((user, assistant))
}
