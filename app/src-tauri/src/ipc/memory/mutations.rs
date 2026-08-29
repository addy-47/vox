use crate::core::constants::{PM_RELATION_SUPERSEDES, PM_SOURCE_USER};
use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use crate::persistence::encode_f32_blob;
use std::sync::atomic::Ordering;
use tauri::State;

/// Update the text content and vector embedding of an existing memory fact.
#[tauri::command]
pub async fn edit_fact_content(
    state: State<'_, std::sync::Arc<AppState>>,
    fact_id: String,
    new_content: String,
) -> Result<(), String> {
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
            (fact_id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;

    let row = rows
        .next()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Fact not found: {}", fact_id))?;
    let collection: String = row.get(0).map_err(|e| e.to_string())?;

    VoxDb::with_transaction(&conn, async {
        conn.execute(
            "UPDATE memory_facts SET fact = ? WHERE id = ?",
            (trimmed.to_string(), fact_id.clone()),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)
             ON CONFLICT(fact_id) DO UPDATE SET embedding = excluded.embedding",
            (fact_id.clone(), collection, blob_bytes),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

/// Reassign a memory fact to a new collection by staging it into the ingestion queue.
#[tauri::command]
pub async fn reassign_fact_collection(
    state: State<'_, std::sync::Arc<AppState>>,
    fact_id: String,
    new_collection: String,
) -> Result<(), String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT fact, source, session_id FROM memory_facts WHERE id = ?",
            (fact_id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;

    let row = rows
        .next()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Fact not found: {}", fact_id))?;

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
    Ok(())
}

/// Soft-delete a memory fact by creating a tombstone fact with a SUPERSEDES edge.
#[tauri::command]
pub async fn soft_delete_fact(
    state: State<'_, std::sync::Arc<AppState>>,
    fact_id: String,
) -> Result<(), String> {
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
            (fact_id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at) VALUES (?, 'foundational', 'Identity', '', ?, 'active', ?)",
            (tombstone_id.clone(), PM_SOURCE_USER.to_string(), now),
        )
        .await
        .map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, 'USER', ?)",
            (tombstone_id, fact_id, PM_RELATION_SUPERSEDES.to_string(), now),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    })
    .await?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

/// Supersede an existing user memory fact with a newly edited version and audit trail.
#[tauri::command]
pub async fn user_edit_memory(
    state: State<'_, std::sync::Arc<AppState>>,
    old_fact_id: String,
    new_fact: String,
    collection: String,
) -> Result<String, String> {
    let trimmed = new_fact.trim();
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
            &old_fact_id,
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
        old_fact_id,
        new_id
    );
    Ok(new_id)
}

/// Delete a user memory fact via soft-deletion and tombstone relation edge.
#[tauri::command]
pub async fn user_delete_memory(
    state: State<'_, std::sync::Arc<AppState>>,
    fact_id: String,
) -> Result<(), String> {
    let trimmed = fact_id.trim();
    if trimmed.is_empty() {
        return Err("Fact ID cannot be empty".to_string());
    }
    soft_delete_fact(state, trimmed.to_string()).await
}
