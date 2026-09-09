use std::time::Duration;

pub const PERSISTENCE_CHANNEL_CAPACITY: usize = 128;
pub const WORKER_EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(100);
pub const PERSISTENCE_RATE_INTERVAL: Duration = Duration::from_secs(1);
pub const MAX_QUEUE_RETRY_ATTEMPTS: u32 = 3;
pub const SQLITE_BUSY_TIMEOUT_MS: u32 = 5000;

pub mod compactions;
pub mod db;
pub mod facts;
pub mod notifications;
pub mod personal_memory;
pub mod projects;
pub mod queue;
pub mod schema;
pub mod sessions;
pub mod voices;
pub mod worker;

pub use compactions::{commit_compaction_output, has_in_progress_compaction, record_compaction_start, resolve_uncompacted_range, CompactionRecord};
pub use facts::{
    deactivate_fact, deactivate_facts_batch, fetch_active_facts_by_type, fetch_all_active_facts,
    FactRecord,
};
pub use notifications::{NewNotification, NotificationRecord};
pub use personal_memory::PersonalMemoryRecord;
pub use projects::ProjectRow;
pub use queue::{enqueue_fact, has_unfinished_items, record_queue_item_failure, update_queue_item_status, QueueItem};
pub use sessions::{fetch_session_project_id, SessionRow, TurnRow};

/// Asynchronous pipeline events offloaded from the voice hot-path to the persistence worker.
#[derive(Debug, Clone)]
pub enum PersistenceEvent {
    SessionStarted {
        session_id: i64,
        timestamp_ms: u64,
    },
    SessionEnded {
        session_id: i64,
        timestamp_ms: u64,
    },
    TurnCompleted {
        session_id: i64,
        turn_id: u32,
        user_text: String,
        assistant_text: String,
    },
    UpdateSessionMetadata {
        session_id: i64,
        key: String,
        value: String,
    },
    Shutdown,
}

/// Floating-point vector byte-blob encoding helper for Turso F32_BLOB columns.
pub fn encode_f32_blob(floats: &[f32]) -> Vec<u8> {
    floats.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Decodes byte-blob into a float vector. Returns empty vector and logs warning if misaligned.
pub fn decode_f32_blob(bytes: &[u8]) -> Vec<f32> {
    if !bytes.len().is_multiple_of(4) {
        log::warn!(
            "[Persistence] Misaligned f32 blob length {} (not a multiple of 4)",
            bytes.len()
        );
        return Vec::new();
    }
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or_default()))
        .collect()
}

#[cfg(test)]
mod tests {
    use turso::Builder;

    use crate::persistence::{
        compactions::{
            commit_compaction_output, fetch_latest_compaction_run, record_compaction_start,
        },
        facts::{
            deactivate_fact, fetch_active_facts_by_type, fetch_active_vectors_by_type, insert_fact,
            insert_vector, FactRecord,
        },
        personal_memory::{get_personal_memory, save_personal_memory, update_consolidated_memory},
        projects::{
            create_project, delete_project, get_project_by_id, get_projects, rename_project,
        },
        queue::{
            claim_pending_queue_batch, reconcile_crashed_queue_on_boot, update_queue_item_status,
        },
        schema::run_migrations,
        sessions::{
            create_session, create_session_with_id, delete_session, fetch_session_by_id,
            fetch_sessions, fetch_turns, update_session_metadata,
        },
    };

    async fn create_test_conn() -> turso::Connection {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let db_path = temp_dir.path().join("persistence_test.db");
        let db = Builder::new_local(db_path.to_string_lossy().as_ref())
            .experimental_index_method(true)
            .build()
            .await
            .expect("build db");
        let conn = db.connect().expect("connect");
        conn.execute("PRAGMA foreign_keys = ON;", ())
            .await
            .expect("enable fk");
        run_migrations(&conn).await.expect("migrations");
        conn
    }

    #[tokio::test]
    async fn test_projects_crud() {
        let conn = create_test_conn().await;

        // Default project exists
        let default_proj = get_project_by_id(&conn, "default").await.unwrap();
        assert!(default_proj.is_some());

        // Create new project
        let p1 = create_project(&conn, "work", "Work Project").await.unwrap();
        assert_eq!(p1.id, "work");
        assert_eq!(p1.name, "Work Project");

        // Rename
        rename_project(&conn, "work", "Vox Work").await.unwrap();
        let p1_renamed = get_project_by_id(&conn, "work").await.unwrap().unwrap();
        assert_eq!(p1_renamed.name, "Vox Work");

        // List
        let list = get_projects(&conn).await.unwrap();
        assert_eq!(list.len(), 2);

        // Cannot delete default
        assert!(delete_project(&conn, "default").await.is_err());

        // Attach session to work project -> delete should fail
        create_session_with_id(&conn, 1001, Some("work"))
            .await
            .unwrap();
        assert!(delete_project(&conn, "work").await.is_err());

        // Remove session, now delete succeeds
        delete_session(&conn, 1001, true).await.unwrap();
        delete_project(&conn, "work").await.unwrap();
        assert!(get_project_by_id(&conn, "work").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sessions_and_turns_crud() {
        let conn = create_test_conn().await;

        let session_id = create_session(&conn, None).await.unwrap();
        assert!(session_id > 0);

        let s = fetch_session_by_id(&conn, session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(s.project_id, "default");
        assert_eq!(s.turn_count, 0);
        assert!(!s.is_pinned);

        // Add turns
        conn.execute(
            "INSERT INTO turns (session_id, turn_id, user_text, assistant_text, created_at)
             VALUES (?, 1, 'Hello', 'Hi there!', 1000),
                    (?, 2, 'How are you?', 'I am doing well.', 2000)",
            (session_id, session_id),
        )
        .await
        .unwrap();

        let turns = fetch_turns(&conn, session_id).await.unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].user_text, "Hello");
        assert_eq!(turns[1].turn_id, 2);

        // Fetch session again -> verify turn_count and first_message
        let s_updated = fetch_session_by_id(&conn, session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(s_updated.turn_count, 2);
        assert_eq!(s_updated.first_message.as_deref(), Some("Hello"));

        // Update metadata
        update_session_metadata(&conn, session_id, Some("Greetings"), Some(true), None)
            .await
            .unwrap();
        let s_meta = fetch_session_by_id(&conn, session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(s_meta.title.as_deref(), Some("Greetings"));
        assert!(s_meta.is_pinned);

        // Soft delete
        delete_session(&conn, session_id, false).await.unwrap();
        let active = fetch_sessions(&conn, None).await.unwrap();
        assert!(active.is_empty());

        // Hard delete cascades
        delete_session(&conn, session_id, true).await.unwrap();
        assert!(fetch_session_by_id(&conn, session_id)
            .await
            .unwrap()
            .is_none());
        assert!(fetch_turns(&conn, session_id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_personal_memory_optimistic_locking() {
        let conn = create_test_conn().await;

        // Fetches global personal memory (initialized in migration)
        let mem = get_personal_memory(&conn, None).await.unwrap();
        assert_eq!(mem.version, 1);

        // Save with valid version
        let updated = save_personal_memory(&conn, None, "# Preferences\n- Rust", 1)
            .await
            .unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(updated.content, "# Preferences\n- Rust");

        // Save with stale version 1 -> must fail
        let conflict = save_personal_memory(&conn, None, "Stale content", 1).await;
        assert!(
            conflict.is_err(),
            "Optimistic lock must reject stale version"
        );

        // Save with correct version 2 -> succeeds
        let updated2 = save_personal_memory(&conn, None, "# Preferences\n- Rust\n- Tauri", 2)
            .await
            .unwrap();
        assert_eq!(updated2.version, 3);
        assert!(updated2.content.contains("Tauri"));

        // Background consolidation update
        let consolidated = update_consolidated_memory(&conn, None, "# Consolidated\n- All good")
            .await
            .unwrap();
        assert_eq!(consolidated.version, 4);
        assert_eq!(consolidated.content, "# Consolidated\n- All good");
    }

    #[tokio::test]
    async fn test_compactions_and_queue_flow() {
        let conn = create_test_conn().await;

        let session_id = create_session(&conn, None).await.unwrap();
        let run_id = record_compaction_start(&conn, session_id, "soft", 1, 5)
            .await
            .unwrap();
        assert!(run_id > 0);

        let latest = fetch_latest_compaction_run(&conn, session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.status, "in_progress");

        // Commit compaction output with extracted facts
        let facts = vec![
            ("personal".to_string(), "User likes clean code".to_string()),
            (
                "workdone".to_string(),
                "Implemented v2 persistence".to_string(),
            ),
        ];
        commit_compaction_output(&conn, run_id, "{\"summary\":\"test\"}", &facts, session_id)
            .await
            .unwrap();

        let finished = fetch_latest_compaction_run(&conn, session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(finished.status, "completed");

        // Verify items arrived in queue
        let claimed = claim_pending_queue_batch(&conn, "pending", "stage1_processing", 10)
            .await
            .unwrap();
        assert_eq!(claimed.len(), 2);
        assert_eq!(claimed[0].status, "stage1_processing");

        // Update queue item
        update_queue_item_status(&conn, claimed[0].id, "completed", None)
            .await
            .unwrap();

        // Simulate crash on claimed[1]
        let reconciled = reconcile_crashed_queue_on_boot(&conn).await.unwrap();
        assert_eq!(reconciled, 1);
    }

    #[tokio::test]
    async fn test_facts_and_vectors_lifecycle() {
        let conn = create_test_conn().await;

        let session_id = create_session(&conn, None).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        let fact = FactRecord {
            id: "fact_1001".to_string(),
            session_id: Some(session_id),
            compaction_id,
            fact_type: "objective".to_string(),
            text: "Achieve sub-200ms voice pipeline".to_string(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_fact(&conn, &fact).await.unwrap();

        let embedding = vec![0.5f32; 384];
        insert_vector(&conn, "fact_1001", "objective", "active", None, &embedding)
            .await
            .unwrap();

        let active_facts = fetch_active_facts_by_type(&conn, "objective")
            .await
            .unwrap();
        assert_eq!(active_facts.len(), 1);
        assert_eq!(active_facts[0].id, "fact_1001");

        let active_vectors = fetch_active_vectors_by_type(&conn, "objective")
            .await
            .unwrap();
        assert_eq!(active_vectors.len(), 1);
        assert_eq!(active_vectors[0].0, "fact_1001");
        assert_eq!(active_vectors[0].1.len(), 384);

        // Deactivate
        deactivate_fact(&conn, "fact_1001").await.unwrap();
        assert!(fetch_active_facts_by_type(&conn, "objective")
            .await
            .unwrap()
            .is_empty());
        assert!(fetch_active_vectors_by_type(&conn, "objective")
            .await
            .unwrap()
            .is_empty());
    }
}
