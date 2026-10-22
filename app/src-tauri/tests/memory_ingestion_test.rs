//! ============================================================================
//! memory_ingestion_test.rs — queued facts are deduplicated in two stages (exact then semantic), embedded, and the queue survives a crash
//! ============================================================================
//! Category  : Integration (Seam 13 — memory ingestion)
//! Component : src/services/memory/ingestion.rs, src/services/memory/ml/embedder.rs, src/persistence/queue.rs, src/persistence/facts.rs
//! Binary    : memory_ingestion_test
//! Prereqs   : VOX_MODELS_DIR with the MiniLM-L12 ONNX embedder; datasets under sandbox/datasets/
//! Run       : cargo nextest run --test memory_ingestion_test --release --test-threads=1
//! ============================================================================

mod common;

use std::time::Duration;

use common::{harness::get_test_app_and_state, paths::TempPathsGuard};
use serde::Deserialize;
use vox_lib::{
    persistence::{
        compactions::record_compaction_start,
        facts::{
            fetch_active_observations_by_type, fetch_active_vectors_by_type, insert_observation,
            insert_vector, ObservationRecord,
        },
        queue::{claim_pending_queue_batch, enqueue_observation},
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
    observation_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct LlmFactsDataset {
    facts: Vec<DatasetFact>,
}

fn load_dedup_pairs() -> Vec<DedupPair> {
    common::paths::load_json_dataset("dedup_500_pairs.json")
}

fn load_llm_facts(limit: usize) -> Vec<DatasetFact> {
    let dataset: LlmFactsDataset = common::paths::load_json_dataset("pure_llm_facts_dataset.json");
    dataset.facts.into_iter().take(limit).collect()
}

/// Seeds a semantically duplicated pair and leaves the incoming copy queued
/// in `stage1_done`, ready for the cosine stage.
async fn seed_semantic_duplicate_pair(
    conn: &turso::Connection,
    pair: &DedupPair,
) -> i64 {
    let session_id = create_session(conn, Some("default")).await.unwrap();
    let compaction_id = record_compaction_start(conn, session_id, "soft", 0, 5)
        .await
        .unwrap();

    let old_fact = ObservationRecord {
        id: format!("fact_pair_{}_1", pair.id),
        session_id: Some(session_id),
        compaction_id,
        observation_type: "personal".to_string(),
        text: pair.fact1.clone(),
        status: "active".to_string(),
        created_at: 1000,
        updated_at: 1000,
    };
    insert_observation(conn, &old_fact).await.unwrap();

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
        conn,
        &old_fact.id,
        "personal",
        "active",
        Some("default"),
        &old_embeddings[0],
    )
    .await
    .unwrap();

    let q_id = enqueue_observation(
        conn,
        Some(session_id),
        compaction_id,
        "personal",
        &pair.fact2,
    )
    .await
    .unwrap();

    conn.execute(
        "UPDATE memory_ingestion_queue SET status = 'stage1_done' WHERE id = ?",
        (q_id,),
    )
    .await
    .unwrap();

    q_id
}

/// PURPOSE:  An incoming fact whose normalised text matches an existing fact
///           must deactivate the older one; keeping both duplicates the
///           user's memory and doubles the retrieval noise.
/// SCENARIO: Seed an active fact from the dataset, enqueue a case- and
///           punctuation-variant of it and run the exact dedup stage.
/// ASSERTS:  One item is processed, one duplicate is deactivated, no errors
///           occur and no active fact of that type remains.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Skip the deactivation in `run_stage1_exact_dedup` — the active
///           fact count assertion would fail.
#[tokio::test]
async fn stage1_exact_dedup_deactivates_the_older_matching_fact() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        let dataset_facts = load_llm_facts(5);
        let sample = &dataset_facts[0];

        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        let old_fact = ObservationRecord {
            id: format!("fact_old_{}", sample.id),
            session_id: Some(session_id),
            compaction_id,
            observation_type: sample.observation_type.clone(),
            text: sample.text.clone(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_observation(&conn, &old_fact).await.unwrap();

        let modified_text = format!("{}!!!", sample.text.to_uppercase());
        enqueue_observation(
            &conn,
            Some(session_id),
            compaction_id,
            &sample.observation_type,
            &modified_text,
        )
        .await
        .unwrap();

        let summary = run_stage1_exact_dedup(&conn).await.unwrap();
        assert_eq!(summary.processed, 1, "Must process exactly 1 pending item");
        assert_eq!(
            summary.duplicates_deactivated, 1,
            "Must deactivate older matching fact from dataset"
        );
        assert_eq!(summary.errors, 0, "No errors expected during Stage 1 dedup");

        let active_facts = fetch_active_observations_by_type(&conn, &sample.observation_type)
            .await
            .unwrap();
        assert!(
            active_facts.is_empty(),
            "Older active fact must be deactivated (incoming item still in queue)"
        );
    })
    .await
    .expect("stage1_exact_dedup_deactivates_the_older_matching_fact timed out");
}

/// PURPOSE:  The exact stage must park the processed item in `stage1_done`
///           with no retries; a wrong status makes the next stage skip the
///           item or reprocess it forever.
/// SCENARIO: Run the exact dedup stage over one queued item and read the
///           queue row back.
/// ASSERTS:  The row's status is `stage1_done` and its retry count is zero.
/// CLASS:    happy-path
/// REQUIRES: none
/// MUTANT:   Leave the status at `pending` after stage 1 — the status
///           assertion would fail.
#[tokio::test]
async fn stage1_exact_dedup_marks_the_queue_item_stage1_done() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state
            .db
            .connect()
            .expect("Failed to connect to test database");

        let dataset_facts = load_llm_facts(5);
        let sample = &dataset_facts[0];

        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        let old_fact = ObservationRecord {
            id: format!("fact_old_{}", sample.id),
            session_id: Some(session_id),
            compaction_id,
            observation_type: sample.observation_type.clone(),
            text: sample.text.clone(),
            status: "active".to_string(),
            created_at: 1000,
            updated_at: 1000,
        };
        insert_observation(&conn, &old_fact).await.unwrap();

        let modified_text = format!("{}!!!", sample.text.to_uppercase());
        let q_id = enqueue_observation(
            &conn,
            Some(session_id),
            compaction_id,
            &sample.observation_type,
            &modified_text,
        )
        .await
        .unwrap();

        run_stage1_exact_dedup(&conn).await.unwrap();

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
    .expect("stage1_exact_dedup_marks_the_queue_item_stage1_done timed out");
}

/// PURPOSE:  A semantically equivalent but differently worded fact must
///           deactivate the older fact and become the only active one.
/// SCENARIO: Seed the real duplicate pair from the dataset with its vector,
///           queue the second wording and run the cosine dedup stage.
/// ASSERTS:  One item is processed, one fact is inserted, one duplicate is
///           deactivated, no errors occur and the surviving fact is the
///           incoming wording.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR with the MiniLM-L12 ONNX embedder
/// MUTANT:   Lower the cosine threshold so the pair is not detected — the
///           duplicate count assertion would fail.
#[tokio::test]
async fn stage2_cosine_dedup_inserts_the_new_fact_and_deactivates_the_old_one() {
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

        let pairs = load_dedup_pairs();
        let duplicate_pair = pairs
            .into_iter()
            .find(|p| p.label == "duplicate" && p.id == 1)
            .expect("Must have duplicate pairs in dedup_500_pairs.json");

        seed_semantic_duplicate_pair(&conn, &duplicate_pair).await;

        let summary = run_stage2_cosine_dedup(&conn).await.unwrap();
        assert_eq!(summary.processed, 1, "Must process 1 item in Stage 2");
        assert_eq!(summary.inserted, 1, "Must insert 1 new active fact");
        assert_eq!(
            summary.duplicates_deactivated, 1,
            "Must deactivate older semantic duplicate from dataset (>0.95 cosine)"
        );
        assert_eq!(summary.errors, 0, "No errors expected during Stage 2 dedup");

        let active_facts = fetch_active_observations_by_type(&conn, "personal")
            .await
            .unwrap();
        assert_eq!(active_facts.len(), 1, "Exactly 1 active fact must remain");
        assert_eq!(active_facts[0].text, duplicate_pair.fact2);

        unload_embedder();
    })
    .await
    .expect("stage2_cosine_dedup_inserts_the_new_fact_and_deactivates_the_old_one timed out");
}

/// PURPOSE:  The fact accepted by the semantic stage must carry its own
///           embedding; a fact without a vector is invisible to semantic
///           retrieval.
/// SCENARIO: Run the cosine stage over the duplicate pair and list the active
///           vectors.
/// ASSERTS:  Exactly one active vector exists, it belongs to the surviving
///           fact and has 384 dimensions.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR with the MiniLM-L12 ONNX embedder
/// MUTANT:   Skip the vector insert in the cosine stage — the vector count
///           assertion would fail.
#[tokio::test]
async fn stage2_cosine_dedup_stores_a_384_dimension_vector_for_the_new_fact() {
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

        let pairs = load_dedup_pairs();
        let duplicate_pair = pairs
            .into_iter()
            .find(|p| p.label == "duplicate" && p.id == 1)
            .expect("Must have duplicate pairs in dedup_500_pairs.json");

        seed_semantic_duplicate_pair(&conn, &duplicate_pair).await;

        run_stage2_cosine_dedup(&conn).await.unwrap();

        let active_facts = fetch_active_observations_by_type(&conn, "personal")
            .await
            .unwrap();
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

        unload_embedder();
    })
    .await
    .expect("stage2_cosine_dedup_stores_a_384_dimension_vector_for_the_new_fact timed out");
}

/// PURPOSE:  A fact that survived both dedup stages must leave the queue in
///           the `completed` state with a processing timestamp; a queue item
///           stuck in flight is picked up again on every boot.
/// SCENARIO: Run the cosine stage over the duplicate pair and read the queue
///           row back.
/// ASSERTS:  The row's status is `completed` and `processed_at` is set.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR with the MiniLM-L12 ONNX embedder
/// MUTANT:   Leave the status at `stage1_done` — the status assertion would
///           fail.
#[tokio::test]
async fn stage2_cosine_dedup_marks_the_queue_item_completed() {
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

        let pairs = load_dedup_pairs();
        let duplicate_pair = pairs
            .into_iter()
            .find(|p| p.label == "duplicate" && p.id == 1)
            .expect("Must have duplicate pairs in dedup_500_pairs.json");

        let q_id = seed_semantic_duplicate_pair(&conn, &duplicate_pair).await;

        run_stage2_cosine_dedup(&conn).await.unwrap();

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
    .expect("stage2_cosine_dedup_marks_the_queue_item_completed timed out");
}

/// PURPOSE:  A full ingestion cycle must process every queued fact through
///           both stages without losing or erroring on any of them.
/// SCENARIO: Enqueue ten dataset facts and run the full ingestion cycle.
/// ASSERTS:  Stage 1 processed ten items with no errors, stage 2 processed
///           ten items, inserted ten facts and reported no errors.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR with the MiniLM-L12 ONNX embedder
/// MUTANT:   Stop the cycle after stage 1 — the stage 2 counters would be
///           zero.
#[tokio::test]
async fn ingestion_cycle_processes_every_queued_fact() {
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

        let dataset_facts = load_llm_facts(10);
        assert_eq!(dataset_facts.len(), 10, "Must load 10 facts from dataset");

        for fact in &dataset_facts {
            enqueue_observation(
                &conn,
                Some(session_id),
                compaction_id,
                &fact.observation_type,
                &fact.text,
            )
            .await
            .unwrap();
        }

        let cycle_summary = run_ingestion_cycle(&conn).await.unwrap();

        assert_eq!(cycle_summary.stage1.processed, 10);
        assert_eq!(cycle_summary.stage1.errors, 0);
        assert_eq!(cycle_summary.stage2.processed, 10);
        assert_eq!(cycle_summary.stage2.inserted, 10);
        assert_eq!(cycle_summary.stage2.errors, 0);

        unload_embedder();
    })
    .await
    .expect("ingestion_cycle_processes_every_queued_fact timed out");
}

/// PURPOSE:  After a full cycle every queue item must be closed; items left
///           open are re-ingested on the next boot and duplicate the facts.
/// SCENARIO: Enqueue ten dataset facts, run the ingestion cycle and count the
///           completed queue rows.
/// ASSERTS:  Ten queue rows carry the `completed` status.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR with the MiniLM-L12 ONNX embedder
/// MUTANT:   Leave the final status as `stage1_done` — the count assertion
///           would fail.
#[tokio::test]
async fn ingestion_cycle_marks_every_queue_item_completed() {
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

        for fact in load_llm_facts(10) {
            enqueue_observation(
                &conn,
                Some(session_id),
                compaction_id,
                &fact.observation_type,
                &fact.text,
            )
            .await
            .unwrap();
        }

        run_ingestion_cycle(&conn).await.unwrap();

        let mut q_rows = conn
            .query(
                "SELECT COUNT(*) FROM memory_ingestion_queue WHERE status = 'completed'",
                (),
            )
            .await
            .unwrap();
        let completed_count: i64 = q_rows.next().await.unwrap().unwrap().get(0).unwrap();
        assert_eq!(completed_count, 10);

        unload_embedder();
    })
    .await
    .expect("ingestion_cycle_marks_every_queue_item_completed timed out");
}

/// PURPOSE:  Every ingested fact must get its own active embedding; missing
///           vectors make the fact unreachable by semantic retrieval.
/// SCENARIO: Enqueue ten dataset facts, run the ingestion cycle and count the
///           active vector rows.
/// ASSERTS:  Ten active vector rows exist.
/// CLASS:    happy-path
/// REQUIRES: VOX_MODELS_DIR with the MiniLM-L12 ONNX embedder
/// MUTANT:   Store the vectors with status `staged` instead of `active` — the
///           count assertion would fail.
#[tokio::test]
async fn ingestion_cycle_stores_one_active_vector_per_fact() {
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

        for fact in load_llm_facts(10) {
            enqueue_observation(
                &conn,
                Some(session_id),
                compaction_id,
                &fact.observation_type,
                &fact.text,
            )
            .await
            .unwrap();
        }

        run_ingestion_cycle(&conn).await.unwrap();

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
    .expect("ingestion_cycle_stores_one_active_vector_per_fact timed out");
}

/// PURPOSE:  A queue item that was in flight when the app crashed must be
///           returned to `pending` with an incremented retry count; leaving
///           it in `stage1_processing` strands the fact forever.
/// SCENARIO: Claim a pending item into `stage1_processing`, then run the boot
///           reconciliation.
/// ASSERTS:  The item is back at `pending` with retry count 1.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Skip the `stage1_processing` reset in
///           `reconcile_crashed_queue_on_boot` — the status assertion would
///           fail.
#[tokio::test]
async fn crash_reconciliation_resets_a_stage1_crashed_item_to_pending() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("Failed to connect to test database");

        let dataset_facts = load_llm_facts(3);
        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        let id_s1 = enqueue_observation(
            &conn,
            Some(session_id),
            compaction_id,
            &dataset_facts[0].observation_type,
            &dataset_facts[0].text,
        )
        .await
        .unwrap();
        claim_pending_queue_batch(&conn, "pending", "stage1_processing", 10)
            .await
            .unwrap();

        let reconciled_count = reconcile_crashed_queue_on_boot(&conn).await.unwrap();
        assert_eq!(reconciled_count, 1, "Must reconcile the single crashed item");

        let mut s1_row = conn
            .query("SELECT status, retry_count FROM memory_ingestion_queue WHERE id = ?", (id_s1,))
            .await
            .unwrap();
        let r1 = s1_row.next().await.unwrap().unwrap();
        let s1_status: String = r1.get(0).unwrap();
        let s1_retry: i64 = r1.get(1).unwrap();
        assert_eq!(s1_status, "pending");
        assert_eq!(s1_retry, 1);
    })
    .await
    .expect("crash_reconciliation_resets_a_stage1_crashed_item_to_pending timed out");
}

/// PURPOSE:  A queue item that crashed during the semantic stage must return
///           to `stage1_done`, not to `pending`; re-running the exact stage
///           would be wasted work.
/// SCENARIO: Put a queued item into `stage2_processing` with one retry, then
///           run the boot reconciliation.
/// ASSERTS:  The item is back at `stage1_done` with retry count 2.
/// CLASS:    regression
/// REQUIRES: none
/// MUTANT:   Reset `stage2_processing` items to `pending` — the status
///           assertion would fail.
#[tokio::test]
async fn crash_reconciliation_resets_a_stage2_crashed_item_to_stage1_done() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("Failed to connect to test database");

        let dataset_facts = load_llm_facts(3);
        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        let id_s2 = enqueue_observation(
            &conn,
            Some(session_id),
            compaction_id,
            &dataset_facts[1].observation_type,
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

        // Two further items so the reconciliation has the same crashed-item
        // population as the original scenario.
        let id_s1 = enqueue_observation(
            &conn,
            Some(session_id),
            compaction_id,
            &dataset_facts[0].observation_type,
            &dataset_facts[0].text,
        )
        .await
        .unwrap();
        claim_pending_queue_batch(&conn, "pending", "stage1_processing", 10)
            .await
            .unwrap();

        let id_poison = enqueue_observation(
            &conn,
            Some(session_id),
            compaction_id,
            &dataset_facts[2].observation_type,
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

        let reconciled_count = reconcile_crashed_queue_on_boot(&conn).await.unwrap();
        assert_eq!(reconciled_count, 3, "Must reconcile all 3 crashed items");

        let mut s2_row = conn
            .query("SELECT status, retry_count FROM memory_ingestion_queue WHERE id = ?", (id_s2,))
            .await
            .unwrap();
        let r2 = s2_row.next().await.unwrap().unwrap();
        let s2_status: String = r2.get(0).unwrap();
        let s2_retry: i64 = r2.get(1).unwrap();
        assert_eq!(s2_status, "stage1_done");
        assert_eq!(s2_retry, 2);

        // The other two items must have been reconciled too.
        let mut s1_row = conn
            .query(
                "SELECT status FROM memory_ingestion_queue WHERE id = ?",
                (id_s1,),
            )
            .await
            .unwrap();
        let r1 = s1_row.next().await.unwrap().unwrap();
        let s1_status: String = r1.get(0).unwrap();
        assert_eq!(s1_status, "pending");
    })
    .await
    .expect("crash_reconciliation_resets_a_stage2_crashed_item_to_stage1_done timed out");
}

/// PURPOSE:  A fact that keeps failing must be quarantined, not retried
///           forever; a poison pill blocks the whole ingestion queue.
/// SCENARIO: Put a queue item into `stage2_processing` with the maximum retry
///           count and run the boot reconciliation.
/// ASSERTS:  The item is `failed`, keeps its retry count and records a
///           "max retries exceeded" error.
/// CLASS:    edge-case
/// REQUIRES: none
/// MUTANT:   Reset the poison pill to `stage1_done` instead of `failed` — the
///           status assertion would fail.
#[tokio::test]
async fn crash_reconciliation_quarantines_a_poison_pill_item() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let _guard = TempPathsGuard::new();
        let (_app, state) = get_test_app_and_state().await;
        let conn = state.db.connect().expect("Failed to connect to test database");

        let dataset_facts = load_llm_facts(3);
        let session_id = create_session(&conn, Some("default")).await.unwrap();
        let compaction_id = record_compaction_start(&conn, session_id, "soft", 0, 5)
            .await
            .unwrap();

        let _id_s1 = enqueue_observation(
            &conn,
            Some(session_id),
            compaction_id,
            &dataset_facts[0].observation_type,
            &dataset_facts[0].text,
        )
        .await
        .unwrap();
        claim_pending_queue_batch(&conn, "pending", "stage1_processing", 10)
            .await
            .unwrap();

        let id_s2 = enqueue_observation(
            &conn,
            Some(session_id),
            compaction_id,
            &dataset_facts[1].observation_type,
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

        let id_poison = enqueue_observation(
            &conn,
            Some(session_id),
            compaction_id,
            &dataset_facts[2].observation_type,
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

        let reconciled_count = reconcile_crashed_queue_on_boot(&conn).await.unwrap();
        assert_eq!(reconciled_count, 3, "Must reconcile all 3 crashed items");

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
    .expect("crash_reconciliation_quarantines_a_poison_pill_item timed out");
}
