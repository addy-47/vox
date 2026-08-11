use crate::core::constants::{PM_RELATION_SUPERSEDES, PM_SOURCE_USER};
use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use crate::persistence::encode_f32_blob;
use std::sync::atomic::Ordering;
use tauri::State;

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

    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    // Ensure embedder is loaded
    crate::services::memory::ensure_embedder_loaded(true)
        .map_err(|e| format!("Embedder loading failed: {}", e))?;

    let embedding = crate::services::memory::generate_embedding(trimmed)
        .map_err(|e| format!("Embedding generation failed: {}", e))?
        .ok_or_else(|| "Failed to generate embedding vector".to_string())?;

    let blob_bytes = encode_f32_blob(&embedding);

    // Query existing collection
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

    conn.execute("BEGIN TRANSACTION;", ())
        .await
        .map_err(|e| e.to_string())?;

    let result: Result<(), String> = async {
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
    }
    .await;

    if result.is_ok() {
        let _ = conn.execute("COMMIT;", ()).await;
        state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
        Ok(())
    } else {
        let _ = conn.execute("ROLLBACK;", ()).await;
        result
    }
}

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

    conn.execute("BEGIN TRANSACTION;", ())
        .await
        .map_err(|e| e.to_string())?;

    let result: Result<(), String> = async {
        // Mark target fact status as superseded
        conn.execute(
            "UPDATE memory_facts SET status = 'superseded' WHERE id = ?",
            (fact_id.clone(),),
        )
        .await
        .map_err(|e| e.to_string())?;

        // Insert tombstone empty fact record
        conn.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at) VALUES (?, 'foundational', 'Identity', '', ?, 'active', ?)",
            (tombstone_id.clone(), PM_SOURCE_USER.to_string(), now),
        )
        .await
        .map_err(|e| e.to_string())?;

        // Insert SUPERSEDES edge from tombstone -> target fact
        conn.execute(
            "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, 'USER', ?)",
            (tombstone_id, fact_id, PM_RELATION_SUPERSEDES.to_string(), now),
        )
        .await
        .map_err(|e| e.to_string())?;

        Ok(())
    }
    .await;

    if result.is_ok() {
        let _ = conn.execute("COMMIT;", ()).await;
        state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
        Ok(())
    } else {
        let _ = conn.execute("ROLLBACK;", ()).await;
        result
    }
}

#[tauri::command]
pub async fn user_edit_memory(
    state: State<'_, std::sync::Arc<AppState>>,
    old_fact_id: String,
    new_fact: String,
    collection: String,
) -> Result<String, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let res = crate::persistence::mutations::supersede_user_fact(&conn, &old_fact_id, &new_fact, &collection)
        .await
        .map_err(|e| e.to_string())?;

    state.memory.graph_version.fetch_add(1, Ordering::SeqCst);
    Ok(res)
}

#[tauri::command]
pub async fn user_delete_memory(
    state: State<'_, std::sync::Arc<AppState>>,
    fact_id: String,
) -> Result<(), String> {
    soft_delete_fact(state, fact_id).await
}
