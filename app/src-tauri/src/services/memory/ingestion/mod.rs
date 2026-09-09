use serde::{Deserialize, Serialize};

pub mod runner;
pub mod stage1_dedup;
pub mod stage2_embed;

pub use runner::{
    reconcile_crashed_queue_on_boot, run_ingestion_cycle, run_ingestion_cycle_with_embedder,
    IngestionCycleSummary,
};
pub use stage1_dedup::{jaccard_similarity, run_stage1_exact_dedup, Stage1Summary};
pub use stage2_embed::{
    run_stage2_cosine_dedup, run_stage2_cosine_dedup_with_embedder, Stage2Summary,
};

/// Strongly-typed lifecycle state for items in `memory_ingestion_queue`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueStatus {
    Pending,
    Stage1Processing,
    Stage1Done,
    Stage2Processing,
    Completed,
    Failed,
}

impl QueueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Stage1Processing => "stage1_processing",
            Self::Stage1Done => "stage1_done",
            Self::Stage2Processing => "stage2_processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{
        compactions::record_compaction_start,
        facts::{fetch_active_facts_by_type, fetch_active_vectors_by_type, insert_fact, insert_vector, FactRecord},
        queue::enqueue_fact,
        schema::recreate_schema,
        sessions::create_session,
    };
    use turso::Builder;

    #[test]
    fn test_jaccard_similarity_calculation() {
        assert_eq!(jaccard_similarity("", ""), 1.0);
        assert_eq!(jaccard_similarity("hello world", "HELLO WORLD!"), 1.0);
        assert_eq!(jaccard_similarity("User likes Rust", "user likes rust."), 1.0);
        assert_eq!(jaccard_similarity("apples", "oranges"), 0.0);
        let sim = jaccard_similarity("apple orange banana", "apple orange pear");
        assert!((sim - 0.5).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_stage1_exact_dedup_winner_takes_all() {
        let db = Builder::new_local(":memory:").build().await.unwrap();
        let conn = db.connect().unwrap();
        recreate_schema(&conn).await.unwrap();

        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        let old_fact = FactRecord {
            id: "fact_old_1".to_string(),
            session_id: Some(session_id),
            compaction_id,
            fact_type: "objective".to_string(),
            text: "Build realtime audio transcription".to_string(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_fact(&conn, &old_fact).await.unwrap();

        let q_id = enqueue_fact(
            &conn,
            Some(session_id),
            compaction_id,
            "objective",
            "build realtime audio transcription!",
        )
        .await
        .unwrap();

        let summary = run_stage1_exact_dedup(&conn).await.unwrap();
        assert_eq!(summary.processed, 1);
        assert_eq!(summary.duplicates_deactivated, 1);
        assert_eq!(summary.errors, 0);

        let active_facts = fetch_active_facts_by_type(&conn, "objective").await.unwrap();
        assert!(active_facts.is_empty(), "Older fact should have been deactivated");

        let mut rows = conn
            .query("SELECT status FROM memory_ingestion_queue WHERE id = ?", (q_id,))
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let status: String = row.get(0).unwrap();
        assert_eq!(status, "stage1_done");
    }

    #[tokio::test]
    async fn test_stage2_cosine_dedup_winner_takes_all() {
        let db = Builder::new_local(":memory:").build().await.unwrap();
        let conn = db.connect().unwrap();
        recreate_schema(&conn).await.unwrap();

        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        let base_vector = vec![0.5f32; 384];
        let old_fact = FactRecord {
            id: "fact_old_vec".to_string(),
            session_id: Some(session_id),
            compaction_id,
            fact_type: "workdone".to_string(),
            text: "Refactored audio ring buffer".to_string(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_fact(&conn, &old_fact).await.unwrap();
        insert_vector(
            &conn,
            &old_fact.id,
            "workdone",
            "active",
            Some("default"),
            &base_vector,
        )
        .await
        .unwrap();

        let q_id = enqueue_fact(
            &conn,
            Some(session_id),
            compaction_id,
            "workdone",
            "Rewrote audio ring buffer implementation",
        )
        .await
        .unwrap();

        conn.execute(
            "UPDATE memory_ingestion_queue SET status = 'stage1_done' WHERE id = ?",
            (q_id,),
        )
        .await
        .unwrap();

        let mock_vec = base_vector.clone();
        let summary = run_stage2_cosine_dedup_with_embedder(&conn, move |_| {
            Ok(Some(mock_vec.clone()))
        })
        .await
        .unwrap();

        assert_eq!(summary.processed, 1);
        assert_eq!(summary.inserted, 1);
        assert_eq!(summary.duplicates_deactivated, 1);
        assert_eq!(summary.errors, 0);

        let active_facts = fetch_active_facts_by_type(&conn, "workdone").await.unwrap();
        assert_eq!(active_facts.len(), 1);
        assert_ne!(active_facts[0].id, "fact_old_vec");
        assert_eq!(active_facts[0].text, "Rewrote audio ring buffer implementation");

        let active_vecs = fetch_active_vectors_by_type(&conn, "workdone").await.unwrap();
        assert_eq!(active_vecs.len(), 1);
        assert_eq!(active_vecs[0].0, active_facts[0].id);

        let mut rows = conn
            .query("SELECT status FROM memory_ingestion_queue WHERE id = ?", (q_id,))
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let status: String = row.get(0).unwrap();
        assert_eq!(status, "completed");
    }

    #[tokio::test]
    async fn test_crash_reconciliation_flow() {
        let db = Builder::new_local(":memory:").build().await.unwrap();
        let conn = db.connect().unwrap();
        recreate_schema(&conn).await.unwrap();

        conn.execute(
            "INSERT INTO memory_ingestion_queue (session_id, compaction_id, type, text, status, retry_count, created_at)
             VALUES (NULL, 1, 'personal', 'fact 1', 'stage1_processing', 0, 100),
                    (NULL, 1, 'personal', 'fact 2', 'stage2_processing', 1, 100),
                    (NULL, 1, 'personal', 'fact 3', 'stage1_processing', 3, 100)",
            (),
        )
        .await
        .unwrap();

        let reconciled = reconcile_crashed_queue_on_boot(&conn).await.unwrap();
        assert_eq!(reconciled, 3);

        let mut rows = conn
            .query("SELECT id, status, retry_count FROM memory_ingestion_queue ORDER BY id ASC", ())
            .await
            .unwrap();

        let r1 = rows.next().await.unwrap().unwrap();
        let s1: String = r1.get(1).unwrap();
        let rc1: i64 = r1.get(2).unwrap();
        assert_eq!(s1, "pending");
        assert_eq!(rc1, 1);

        let r2 = rows.next().await.unwrap().unwrap();
        let s2: String = r2.get(1).unwrap();
        let rc2: i64 = r2.get(2).unwrap();
        assert_eq!(s2, "stage1_done");
        assert_eq!(rc2, 2);

        let r3 = rows.next().await.unwrap().unwrap();
        let s3: String = r3.get(1).unwrap();
        assert_eq!(s3, "failed");
    }
}
