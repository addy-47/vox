//! ============================================================================
//! tests/memory_ingestion_test.rs — Memory Ingestion Queue & 2-Stage Deduplication v2 Integration Tests
//! ============================================================================
//! Category     : Integration Test (Seam 13)
//! Component    : services/memory/ingestion, persistence/queue, persistence/facts, services/memory/ml/embedder
//! Prerequisites: Local MiniLM-L12 ONNX model (~/.vox/models/embedding/minilm-l12-v2), Turso SQLite engine (vox.db), real dataset files in sandbox/datasets/
//! Execution    : cargo nextest run --test memory_ingestion_test --release --nocapture --test-threads=1
//! Metrics      : Jaccard exact match, 384-dim ONNX embedding generation, cosine semantic dedup, queue status lifecycle
//! ============================================================================

mod common;

use std::{path::PathBuf, time::Duration};

use common::{harness::get_test_app_and_state, paths::TempPathsGuard};
use serde::Deserialize;
use vox_lib::{
    persistence::{
        compactions::record_compaction_start,
        facts::{
            fetch_active_facts_by_type, fetch_active_vectors_by_type, insert_fact, insert_vector,
            FactRecord,
        },
        queue::{claim_pending_queue_batch, enqueue_fact},
        sessions::create_session,
    },
    services::memory::{
        ensure_embedder_loaded,
        ingestion::{
            reconcile_crashed_queue_on_boot, run_ingestion_cycle, run_stage1_exact_dedup,
            run_stage2_cosine_dedup,
        },
        ml::embedder::unload_embedder,
    },
};

#[derive(Debug, Deserialize)]
struct DedupPair {
    id: usize,
    label: String,
    fact1: String,
    fact2: String,
}

#[derive(Debug, Deserialize)]
struct DatasetFact {
    id: String,
    #[serde(rename = "type")]
    fact_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct LlmFactsDataset {
    facts: Vec<DatasetFact>,
}

fn load_dedup_pairs() -> Vec<DedupPair> {
    let candidates = [
        "sandbox/datasets/dedup_500_pairs.json",
        "../../sandbox/datasets/dedup_500_pairs.json",
        "../sandbox/datasets/dedup_500_pairs.json",
    ];
    let path = candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .expect("dedup_500_pairs.json must exist in candidate paths");
    let content = std::fs::read_to_string(&path).expect("Failed to read dedup_500_pairs.json");
    serde_json::from_str(&content).expect("Failed to parse dedup_500_pairs.json")
}

fn load_llm_facts(limit: usize) -> Vec<DatasetFact> {
    let candidates = [
        "sandbox/datasets/pure_llm_facts_dataset.json",
        "../../sandbox/datasets/pure_llm_facts_dataset.json",
        "../sandbox/datasets/pure_llm_facts_dataset.json",
    ];
    let path = candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .expect("pure_llm_facts_dataset.json must exist in candidate paths");
    let content =
        std::fs::read_to_string(&path).expect("Failed to read pure_llm_facts_dataset.json");
    let dataset: LlmFactsDataset =
        serde_json::from_str(&content).expect("Failed to parse pure_llm_facts_dataset.json");
    dataset.facts.into_iter().take(limit).collect()
}

// ============================================================================
// Subtest 1: Stage 1 Exact Jaccard Deduplication & Winner-Takes-All from Dataset
// ============================================================================
#[tokio::test]
async fn test_stage1_exact_dedup_winner_takes_all() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        // Load real fact from pure_llm_facts_dataset.json
        let dataset_facts = load_llm_facts(5);
        let sample = &dataset_facts[0];

        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        // 1. Seed existing active fact using real dataset text
        let old_fact = FactRecord {
            id: format!("fact_old_{}", sample.id),
            session_id: Some(session_id),
            compaction_id,
            fact_type: sample.fact_type.clone(),
            text: sample.text.clone(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_fact(&conn, &old_fact).await.unwrap();

        // 2. Enqueue incoming fact with identical normalized text (case & punctuation variation)
        let modified_text = format!("{}!!!", sample.text.to_uppercase());
        let q_id = enqueue_fact(
            &conn,
            Some(session_id),
            compaction_id,
            &sample.fact_type,
            &modified_text,
        )
        .await
        .unwrap();

        // 3. Execute Stage 1 exact Jaccard dedup
        let summary = run_stage1_exact_dedup(&conn).await.unwrap();
        assert_eq!(summary.processed, 1, "Must process exactly 1 pending item");
        assert_eq!(
            summary.duplicates_deactivated, 1,
            "Must deactivate older matching fact from dataset"
        );
        assert_eq!(summary.errors, 0, "No errors expected during Stage 1 dedup");

        // 4. Assert older fact was deactivated in memory_facts
        let active_facts = fetch_active_facts_by_type(&conn, &sample.fact_type)
            .await
            .unwrap();
        assert!(
            active_facts.is_empty(),
            "Older active fact must be deactivated (incoming item still in queue)"
        );

        // 5. Assert queue item status transitioned to 'stage1_done'
        let mut rows = conn
            .query(
                "SELECT status, retry_count FROM memory_ingestion_queue WHERE id = ?",
                (q_id,),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("Queue row must exist");
        let status: String = row.get(0).unwrap();
        let retry_count: i64 = row.get(1).unwrap();
        assert_eq!(status, "stage1_done");
        assert_eq!(retry_count, 0);
    })
    .await
    .expect("test_stage1_exact_dedup_winner_takes_all timed out");
}

// ============================================================================
// Subtest 2: Stage 2 Semantic Cosine Deduplication with Real Dataset Pair & Real MiniLM ONNX
// ============================================================================
#[tokio::test]
async fn test_stage2_semantic_cosine_dedup_real_embedder() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        unload_embedder();
        let loaded = ensure_embedder_loaded(true).expect("Failed to ensure embedder loaded");
        assert!(loaded, "MiniLM ONNX embedder model must load successfully");

        // Load real duplicate pair from dedup_500_pairs.json that exercises semantic similarity
        let pairs = load_dedup_pairs();
        let duplicate_pair = pairs
            .into_iter()
            .find(|p| p.label == "duplicate" && p.id == 1) // Pair 1: "User's preferred programming language is Python." vs "Python is the programming language the user prefers."
            .expect("Must have duplicate pairs in dedup_500_pairs.json");

        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        // 1. Seed existing active fact using fact1 from real dataset pair
        let old_fact = FactRecord {
            id: format!("fact_pair_{}_1", duplicate_pair.id),
            session_id: Some(session_id),
            compaction_id,
            fact_type: "personal".to_string(),
            text: duplicate_pair.fact1.clone(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_fact(&conn, &old_fact).await.unwrap();

        let old_embeddings =
            vox_lib::services::memory::generate_embeddings_batch(&[&old_fact.text])
                .unwrap()
                .expect("Embedding generation must return vector");
        assert_eq!(
            old_embeddings[0].len(),
            384,
            "MiniLM embeddings must be 384 dimensions"
        );

        insert_vector(
            &conn,
            &old_fact.id,
            "personal",
            "active",
            Some("default"),
            &old_embeddings[0],
        )
        .await
        .unwrap();

        // 2. Enqueue incoming semantic duplicate using fact2 from real dataset pair
        let q_id = enqueue_fact(
            &conn,
            Some(session_id),
            compaction_id,
            "personal",
            &duplicate_pair.fact2,
        )
        .await
        .unwrap();

        conn.execute(
            "UPDATE memory_ingestion_queue SET status = 'stage1_done' WHERE id = ?",
            (q_id,),
        )
        .await
        .unwrap();

        // 3. Execute Stage 2 semantic cosine dedup
        let summary = run_stage2_cosine_dedup(&conn).await.unwrap();
        assert_eq!(summary.processed, 1, "Must process 1 item in Stage 2");
        assert_eq!(summary.inserted, 1, "Must insert 1 new active fact");
        assert_eq!(
            summary.duplicates_deactivated, 1,
            "Must deactivate older semantic duplicate from dataset (>0.95 cosine)"
        );
        assert_eq!(summary.errors, 0, "No errors expected during Stage 2 dedup");

        // 4. Assert older fact deactivated and new fact from dataset is active
        let active_facts = fetch_active_facts_by_type(&conn, "personal").await.unwrap();
        assert_eq!(active_facts.len(), 1, "Exactly 1 active fact must remain");
        assert_eq!(active_facts[0].text, duplicate_pair.fact2);

        // 5. Assert 384-dimensional vector exists for new active fact
        let active_vectors = fetch_active_vectors_by_type(&conn, "personal")
            .await
            .unwrap();
        assert_eq!(
            active_vectors.len(),
            1,
            "Exactly 1 active vector must remain"
        );
        assert_eq!(active_vectors[0].0, active_facts[0].id);
        assert_eq!(active_vectors[0].1.len(), 384);

        // 6. Assert queue item status transitioned to 'completed'
        let mut rows = conn
            .query(
                "SELECT status, processed_at FROM memory_ingestion_queue WHERE id = ?",
                (q_id,),
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("Queue row must exist");
        let status: String = row.get(0).unwrap();
        let processed_at: Option<i64> = row.get(1).ok();
        assert_eq!(status, "completed");
        assert!(processed_at.is_some());

        unload_embedder();
    })
    .await
    .expect("test_stage2_semantic_cosine_dedup_real_embedder timed out");
}

// ============================================================================
// Subtest 3: Full Ingestion Cycle End-to-End Across Real Dataset Facts
// ============================================================================
#[tokio::test]
async fn test_ingestion_cycle_end_to_end() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        unload_embedder();
        ensure_embedder_loaded(true).expect("Failed to load MiniLM embedder");

        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "manual", 1, 10)
            .await
            .unwrap();

        // Load 10 real facts from pure_llm_facts_dataset.json across multiple categories
        let dataset_facts = load_llm_facts(10);
        assert_eq!(dataset_facts.len(), 10, "Must load 10 facts from dataset");

        for fact in &dataset_facts {
            enqueue_fact(
                &conn,
                Some(session_id),
                compaction_id,
                &fact.fact_type,
                &fact.text,
            )
            .await
            .unwrap();
        }

        // Execute full ingestion cycle (Stage 1 exact dedup -> Stage 2 cosine dedup)
        let cycle_summary = run_ingestion_cycle(&conn).await.unwrap();

        assert_eq!(cycle_summary.stage1.processed, 10);
        assert_eq!(cycle_summary.stage1.errors, 0);
        assert_eq!(cycle_summary.stage2.processed, 10);
        assert_eq!(cycle_summary.stage2.inserted, 10);
        assert_eq!(cycle_summary.stage2.errors, 0);

        // Verify all 10 queue items are now 'completed'
        let mut q_rows = conn
            .query(
                "SELECT COUNT(*) FROM memory_ingestion_queue WHERE status = 'completed'",
                (),
            )
            .await
            .unwrap();
        let completed_count: i64 = q_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(completed_count, 10);

        // Verify vectors generated and stored for all 10 facts
        let mut v_rows = conn
            .query(
                "SELECT COUNT(*) FROM memory_facts_vectors WHERE status = 'active'",
                (),
            )
            .await
            .unwrap();
        let vector_count: i64 = v_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(vector_count, 10);

        unload_embedder();
    })
    .await
    .expect("test_ingestion_cycle_end_to_end timed out");
}

// ============================================================================
// Subtest 4: Boot Crash Reconciliation & Poison-Pill Quarantine with Real Dataset Text
// ============================================================================
#[tokio::test]
async fn test_crash_reconciliation_and_poison_pill() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("Failed to connect to test database");

        let dataset_facts = load_llm_facts(3);
        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        // 1. Seed item in 'stage1_processing' with retry_count = 0 (simulating in-flight crash)
        let id_s1 = enqueue_fact(
            &conn,
            Some(session_id),
            compaction_id,
            &dataset_facts[0].fact_type,
            &dataset_facts[0].text,
        )
        .await
        .unwrap();
        claim_pending_queue_batch(&conn, "pending", "stage1_processing", 10)
            .await
            .unwrap();

        // 2. Seed item in 'stage2_processing' with retry_count = 1 (simulating in-flight crash)
        let id_s2 = enqueue_fact(
            &conn,
            Some(session_id),
            compaction_id,
            &dataset_facts[1].fact_type,
            &dataset_facts[1].text,
        )
        .await
        .unwrap();
        conn.execute(
            "UPDATE memory_ingestion_queue SET status = 'stage2_processing', retry_count = 1 WHERE id = ?",
            (id_s2,),
        )
        .await
        .unwrap();

        // 3. Seed poison pill item with retry_count = 3 in 'stage2_processing'
        let id_poison = enqueue_fact(
            &conn,
            Some(session_id),
            compaction_id,
            &dataset_facts[2].fact_type,
            &dataset_facts[2].text,
        )
        .await
        .unwrap();
        conn.execute(
            "UPDATE memory_ingestion_queue SET status = 'stage2_processing', retry_count = 3 WHERE id = ?",
            (id_poison,),
        )
        .await
        .unwrap();

        // 4. Run startup crash reconciliation
        let reconciled_count = reconcile_crashed_queue_on_boot(&conn).await.unwrap();
        assert_eq!(reconciled_count, 3, "Must reconcile all 3 crashed items");

        // 5. Verify Stage 1 crashed item was reset to 'pending' with retry_count = 1
        let mut s1_row = conn
            .query("SELECT status, retry_count FROM memory_ingestion_queue WHERE id = ?", (id_s1,))
            .await
            .unwrap();
        let r1 = s1_row.next().await.unwrap().unwrap();
        let s1_status: String = r1.get(0).unwrap();
        let s1_retry: i64 = r1.get(1).unwrap();
        assert_eq!(s1_status, "pending");
        assert_eq!(s1_retry, 1);

        // 6. Verify Stage 2 crashed item was reset to 'stage1_done' with retry_count = 2
        let mut s2_row = conn
            .query("SELECT status, retry_count FROM memory_ingestion_queue WHERE id = ?", (id_s2,))
            .await
            .unwrap();
        let r2 = s2_row.next().await.unwrap().unwrap();
        let s2_status: String = r2.get(0).unwrap();
        let s2_retry: i64 = r2.get(1).unwrap();
        assert_eq!(s2_status, "stage1_done");
        assert_eq!(s2_retry, 2);

        // 7. Verify Poison pill item was permanently quarantined to 'failed'
        let mut poison_row = conn
            .query("SELECT status, retry_count, error_msg FROM memory_ingestion_queue WHERE id = ?", (id_poison,))
            .await
            .unwrap();
        let rp = poison_row.next().await.unwrap().unwrap();
        let p_status: String = rp.get(0).unwrap();
        let p_retry: i64 = rp.get(1).unwrap();
        let p_err: Option<String> = rp.get(2).ok();
        assert_eq!(p_status, "failed", "Poison pill must transition to 'failed'");
        assert_eq!(p_retry, 3, "Retry count must be preserved");
        assert!(
            p_err.unwrap_or_default().contains("max retries exceeded"),
            "Error message must indicate max retries exceeded"
        );
    })
    .await
    .expect("test_crash_reconciliation_and_poison_pill timed out");
}
