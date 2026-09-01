use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use crate::persistence::encode_f32_blob;
use crate::services::memory::{FactSource, Relation};
use std::sync::atomic::Ordering;
use tauri::State;

#[derive(Debug, serde::Deserialize)]
pub struct ManageFactPayload {
    pub action: String,
    pub fact_id: String,
    pub new_content: Option<String>,
    pub new_collection: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum ManageFactResult {
    NewId(String),
    Done,
}

/// Unified command for modifying, superseding, reassigning, and deleting memory facts.
#[tauri::command]
pub async fn manage_memory_fact(
    state: State<'_, std::sync::Arc<AppState>>,
    payload: ManageFactPayload,
) -> Result<ManageFactResult, String> {
    match payload.action.to_lowercase().as_str() {
        "edit_in_place" | "edit" => edit_fact_in_place_internal(&state, payload).await,
        "supersede" | "user_edit" => supersede_fact_internal(&state, payload).await,
        "reassign" => reassign_fact_internal(&state, payload).await,
        "delete" | "soft_delete" => delete_fact_internal(&state, payload).await,
        _ => Err(format!(
            "Unknown manage_memory_fact action: {}",
            payload.action
        )),
    }
}

async fn edit_fact_in_place_internal(
    state: &State<'_, std::sync::Arc<AppState>>,
    payload: ManageFactPayload,
) -> Result<ManageFactResult, String> {
    let new_content = payload
        .new_content
        .ok_or("new_content required for edit action")?;
    let trimmed = new_content.trim();
    if trimmed.is_empty() {
        return Err("Fact content cannot be empty".to_string());
    }

    let memory_enabled = state
        .settings
        .read()
        .map(|s| s.memory.pipeline_processing_enabled || s.memory.context_retrieval_enabled)
        .unwrap_or(false);
    if !memory_enabled {
        return Err("Memory subsystem is disabled".to_string());
    }

    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let trimmed_clone = trimmed.to_string();
    let embedding = tokio::task::spawn_blocking(move || {
        crate::services::memory::ensure_embedder_loaded(true)
            .map_err(|e| format!("Embedder loading failed: {}", e))?;
        crate::services::memory::generate_embedding(&trimmed_clone)
            .map_err(|e| format!("Embedding generation failed: {}", e))?
            .ok_or_else(|| "Failed to generate embedding vector".to_string())
    })
    .await
    .map_err(|e| format!("Task panicked: {}", e))??;

    let blob_bytes = encode_f32_blob(&embedding);

    let mut rows = conn
        .query(
            "SELECT collection FROM memory_facts WHERE id = ?",
            (payload.fact_id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;

    let row = rows
        .next()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Fact not found: {}", payload.fact_id))?;
    let collection: String = row.get(0).map_err(|e| e.to_string())?;

    VoxDb::with_transaction(&conn, async {
        conn.execute(
            "UPDATE memory_facts SET fact = ? WHERE id = ?",
            (trimmed.to_string(), payload.fact_id.clone()),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)
             ON CONFLICT(fact_id) DO UPDATE SET embedding = excluded.embedding",
            (payload.fact_id.clone(), collection, blob_bytes),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(ManageFactResult::Done)
}

async fn supersede_fact_internal(
    state: &State<'_, std::sync::Arc<AppState>>,
    payload: ManageFactPayload,
) -> Result<ManageFactResult, String> {
    let new_content = payload
        .new_content
        .ok_or("new_content required for supersede action")?;
    let collection = payload
        .new_collection
        .unwrap_or_else(|| "Identity".to_string());
    let trimmed = new_content.trim();
    if trimmed.is_empty() {
        return Err("Fact content cannot be empty".to_string());
    }

    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let new_id = VoxDb::with_transaction(&conn, async {
        let res = crate::persistence::mutations::supersede_user_fact(
            &conn,
            &payload.fact_id,
            trimmed,
            &collection,
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok(res)
    })
    .await?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    log::info!(
        "[Memory] Successfully superseded fact {} -> new_id={}",
        payload.fact_id,
        new_id
    );
    Ok(ManageFactResult::NewId(new_id))
}

async fn reassign_fact_internal(
    state: &State<'_, std::sync::Arc<AppState>>,
    payload: ManageFactPayload,
) -> Result<ManageFactResult, String> {
    let new_collection = payload
        .new_collection
        .ok_or("new_collection required for reassign action")?;
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT fact, source, session_id FROM memory_facts WHERE id = ?",
            (payload.fact_id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;

    let row = rows
        .next()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Fact not found: {}", payload.fact_id))?;

    let fact_text: String = row.get(0).map_err(|e| e.to_string())?;
    let source_str: String = row.get(1).map_err(|e| e.to_string())?;
    let session_id: String = row.get(2).unwrap_or_default();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "INSERT INTO personal_memory_queue (fact, collection, source, session_id, status, created_at)
         VALUES (?, ?, ?, ?, 'staged_pending', ?)",
        (fact_text, new_collection, source_str, session_id, now),
    )
    .await
    .map_err(|e| e.to_string())?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(ManageFactResult::Done)
}

async fn delete_fact_internal(
    state: &State<'_, std::sync::Arc<AppState>>,
    payload: ManageFactPayload,
) -> Result<ManageFactResult, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let tombstone_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());

    VoxDb::with_transaction(&conn, async {
        conn.execute(
            "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
            (payload.fact_id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at) VALUES (?, 'foundational', 'Identity', '', ?, 'active', ?)",
            (tombstone_id.clone(), FactSource::User.as_str().to_string(), now),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, 'USER', ?)",
            (tombstone_id, payload.fact_id, Relation::Supersedes.as_str().to_string(), now),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(ManageFactResult::Done)
}
