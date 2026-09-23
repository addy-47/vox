use anyhow::Result;
use serde::{Deserialize, Serialize};
use turso::Connection;

pub mod stage1_dedup;
pub mod stage2_embed;

pub use stage1_dedup::{jaccard_similarity, run_stage1_exact_dedup, Stage1Summary};
pub use stage2_embed::{
    run_stage2_cosine_dedup, run_stage2_cosine_dedup_with_embedder, Stage2Summary,
};

use crate::persistence::queue::reconcile_crashed_queue_on_boot as persistence_reconcile;

pub const JACCARD_EXACT_MATCH_THRESHOLD: f32 = 1.0;
pub const SOFT_VECTOR_DEDUP_THRESHOLD: f32 = 0.95;

pub const STAGE1_BATCH_CEILING: usize = 128;
pub const STAGE2_BATCH_SIZE: usize = 16;

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

/// Composite metrics summarizing an entire 2-stage ingestion deduplication cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestionCycleSummary {
    pub stage1: Stage1Summary,
    pub stage2: Stage2Summary,
}

/// Executes a complete deduplication cycle: Stage 1 exact Jaccard dedup followed by Stage 2 semantic cosine dedup.
pub async fn run_ingestion_cycle(conn: &Connection) -> Result<IngestionCycleSummary> {
    let stage1 = run_stage1_exact_dedup(conn).await?;
    let stage2 = run_stage2_cosine_dedup(conn).await?;

    Ok(IngestionCycleSummary { stage1, stage2 })
}

/// Executes an ingestion cycle using an injected embedding function for deterministic testing.
pub async fn run_ingestion_cycle_with_embedder<F>(
    conn: &Connection,
    embed_fn: F,
) -> Result<IngestionCycleSummary>
where
    F: Fn(&str) -> Result<Option<Vec<f32>>> + Send + Sync + 'static,
{
    let stage1 = run_stage1_exact_dedup(conn).await?;
    let stage2 = run_stage2_cosine_dedup_with_embedder(conn, embed_fn).await?;

    Ok(IngestionCycleSummary { stage1, stage2 })
}

/// Reconciles crashed queue items on application boot.
pub async fn reconcile_crashed_queue_on_boot(conn: &Connection) -> Result<usize> {
    persistence_reconcile(conn).await
}

#[cfg(test)]
mod tests {
    use turso::Builder;

    use super::*;
    use crate::persistence::{
        compactions::record_compaction_start, schema::recreate_schema, sessions::create_session,
    };

    #[tokio::test]
    async fn test_crash_reconciliation_flow() {
        let db = Builder::new_local(":memory:").build().await.unwrap();
        let conn = db.connect().unwrap();
        recreate_schema(&conn).await.unwrap();

        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        conn.execute(
            "INSERT INTO memory_ingestion_queue (session_id, compaction_id, type, text, status, retry_count, created_at)
             VALUES (?, ?, 'personal', 'fact 1', 'stage1_processing', 0, 100),
                    (?, ?, 'personal', 'fact 2', 'stage2_processing', 1, 100),
                    (?, ?, 'personal', 'fact 3', 'stage1_processing', 3, 100)",
            (
                session_id,
                compaction_id,
                session_id,
                compaction_id,
                session_id,
                compaction_id,
            ),
        )
        .await
        .unwrap();

        let reconciled = reconcile_crashed_queue_on_boot(&conn).await.unwrap();
        assert_eq!(reconciled, 3);

        let mut rows = conn
            .query(
                "SELECT id, status, retry_count FROM memory_ingestion_queue ORDER BY id ASC",
                (),
            )
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
