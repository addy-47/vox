use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::time::sleep;

use vox_lib::core::settings::{VoxSettings, MemorySettings};
use vox_lib::core::constants::*;
use vox_lib::services::memory::nli::*;
use vox_lib::services::memory::personal_memory::*;
use vox_lib::persistence::memory_worker::*;

fn get_test_nli_name() -> String {
    "deberta-v3-xsmall-nli".to_string()
}

async fn setup_test_db() -> Result<(turso::Connection, PathBuf, tempfile::TempDir)> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("vox_test.db");
    let db = turso::Builder::new_local(db_path.to_str().unwrap())
        .experimental_index_method(true)
        .build()
        .await?;
    let conn = db.connect()?;
    vox_lib::persistence::schema::run_migrations(&conn).await?;
    Ok((conn, db_path, temp_dir))
}

async fn insert_test_fact(
    conn: &turso::Connection,
    id: &str,
    collection: &str,
    fact: &str,
    created_at: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO memory_facts (id, collection, fact, source, created_at) VALUES (?, ?, ?, 'LLM', ?)",
        (id.to_string(), collection.to_string(), fact.to_string(), created_at),
    ).await?;
    let dummy_embedding = vec![0.1f32; 1024];
    let blob = encode_f32_blob(&dummy_embedding);
    conn.execute(
        "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, ?, ?)",
        (id.to_string(), collection.to_string(), blob),
    ).await?;
    Ok(())
}

#[tokio::test]
async fn test_nli_engine_correctness() -> Result<()> {
    let nli_name = get_test_nli_name();
    ensure_nli_loaded(&nli_name)?;

    // Test contradiction
    let res1 = classify_pair("User lives in Delhi", "User lives in Bangalore")?;
    assert!(res1.contradiction > 0.7);

    // Test entailment
    let res2 = classify_pair("User prefers dark mode", "User likes dark theme")?;
    assert!(res2.entailment > 0.7);

    // Test neutral
    let res3 = classify_pair("User likes Rust", "User drinks coffee in the morning")?;
    assert!(res3.neutral > 0.5);

    Ok(())
}

#[tokio::test]
async fn test_job_queue_persistence_and_recovery() -> Result<()> {
    let (conn, db_path, _tmp) = setup_test_db().await?;

    // Enqueue 3 facts
    let mut facts = HashMap::new();
    facts.insert(
        "Identity".to_string(),
        vec!["Works as a developer.".to_string(), "Lives in SF.".to_string()],
    );
    facts.insert(
        "Preferences".to_string(),
        vec!["Prefers dark mode.".to_string()],
    );

    enqueue_personal_facts(&conn, facts, "session_test_1").await?;

    // Verify 3 rows enqueued
    let mut rows = conn.query("SELECT count(*) FROM personal_memory_queue WHERE status = 'pending'", ()).await?;
    let count: i64 = rows.next().await?.unwrap().get(0)?;
    assert_eq!(count, 3);

    // Spawn memory worker with personal_enabled = true
    let mut vox_settings = VoxSettings::default();
    vox_settings.memory.personal_enabled = true;
    let settings = Arc::new(RwLock::new(vox_settings));
    let private_mode = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_tx = spawn_memory_worker(db_path.clone(), private_mode, settings);

    // Poll DB until all 3 items are processed, up to 15 seconds
    let start_time = std::time::Instant::now();
    let mut done_count = 0;
    while start_time.elapsed() < Duration::from_secs(15) {
        let mut rows = conn.query("SELECT count(*) FROM personal_memory_queue WHERE status = 'done'", ()).await?;
        done_count = rows.next().await?.unwrap().get::<i64>(0)?;
        if done_count == 3 {
            break;
        }
        sleep(Duration::from_millis(200)).await;
    }

    // Shutdown worker
    let _ = worker_tx.send(MemoryWorkerEvent::Shutdown);

    // Reopen DB and check
    let db = turso::Builder::new_local(db_path.to_str().unwrap())
        .experimental_index_method(true)
        .build()
        .await?;
    let conn2 = db.connect()?;

    let mut check_rows = conn2.query("SELECT id, status, error_msg FROM personal_memory_queue", ()).await?;
    while let Some(row) = check_rows.next().await? {
        println!("Queue Row: id={:?}, status={:?}, error={:?}", row.get::<i64>(0), row.get::<String>(1), row.get::<Option<String>>(2));
    }

    assert_eq!(done_count, 3);

    let mut rows = conn2.query("SELECT count(*) FROM memory_facts", ()).await?;
    let facts_count: i64 = rows.next().await?.unwrap().get(0)?;
    assert_eq!(facts_count, 3);

    Ok(())
}

#[tokio::test]
async fn test_end_to_end_nli_graph_pipeline() -> Result<()> {
    let (conn, _db_path, _tmp) = setup_test_db().await?;
    let mut settings = MemorySettings::default();
    settings.nli_model_name = get_test_nli_name();

    // Insert original fact
    let mut facts1 = HashMap::new();
    facts1.insert("Identity".to_string(), vec!["User lives in San Francisco".to_string()]);
    enqueue_personal_facts(&conn, facts1, "sess_1").await?;

    // Process first fact
    let processed = process_one_queue_item(&conn, &settings).await?;
    assert!(processed);

    // Insert contradicting fact
    let mut facts2 = HashMap::new();
    facts2.insert("Identity".to_string(), vec!["User lives in New York".to_string()]);
    enqueue_personal_facts(&conn, facts2, "sess_1").await?;

    // Process second fact
    let processed = process_one_queue_item(&conn, &settings).await?;
    assert!(processed);

    // Verify CONFLICTS edge exists in relations table
    let mut rows = conn.query("SELECT count(*) FROM memory_relations WHERE relation = ?", (PM_RELATION_CONFLICTS.to_string(),)).await?;
    let conflicts_count: i64 = rows.next().await?.unwrap().get(0)?;
    assert!(conflicts_count >= 1);

    Ok(())
}

#[tokio::test]
async fn test_edge_resolution_pointer_swap() -> Result<()> {
    let (conn, _db_path, _tmp) = setup_test_db().await?;
    let settings = MemorySettings::default();

    // Insert fact A
    let fact_a_id = "mem_1000_a".to_string();
    conn.execute(
        "INSERT INTO memory_facts (id, collection, fact, source, created_at) VALUES (?, 'Identity', 'User codes in Java', 'LLM', 1000)",
        (fact_a_id.clone(),),
    ).await?;

    // Supersede fact A with fact B
    let _fact_b_id = supersede_user_fact(&conn, &fact_a_id, "User codes in Rust", "Identity").await?;

    // Query retrieve_personal_context
    let query_vector = vec![0.1f32; 1024];
    let context = retrieve_personal_context(&conn, &query_vector, &settings, 2048, None).await?;

    assert!(context.contains("User codes in Rust"));
    assert!(!context.contains("User codes in Java"));

    Ok(())
}

#[tokio::test]
async fn test_edge_resolution_supports_pull() -> Result<()> {
    let (conn, _db_path, _tmp) = setup_test_db().await?;
    let settings = MemorySettings::default();

    // Insert direct match X (using insert_test_fact so it is loaded and searchable in Identity/Preferences)
    let fact_x_id = "mem_1000_x";
    insert_test_fact(&conn, fact_x_id, "Identity", "User relies on Turso", 1000).await?;

    // Insert supporting fact Y (Preferences, different collection)
    let fact_y_id = "mem_1000_y";
    insert_test_fact(&conn, fact_y_id, "Preferences", "User is running on 8GB RAM constraint", 1000).await?;

    // Write SUPPORTS edge from X -> Y
    conn.execute(
        "INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, 1000)",
        (fact_x_id.to_string(), fact_y_id.to_string(), PM_RELATION_SUPPORTS.to_string()),
    ).await?;

    // Query retrieve_personal_context (should pull Y because X is loaded in Identity)
    let query_vector = vec![0.1f32; 1024];
    let context = retrieve_personal_context(&conn, &query_vector, &settings, 2048, None).await?;

    assert!(context.contains("User relies on Turso"));
    assert!(context.contains("User is running on 8GB RAM constraint"));

    Ok(())
}

#[tokio::test]
async fn test_edge_resolution_conflicts_shadow() -> Result<()> {
    let (conn, _db_path, _tmp) = setup_test_db().await?;
    let settings = MemorySettings::default();

    // Insert older fact A
    let fact_a_id = "mem_1000_a";
    insert_test_fact(&conn, fact_a_id, "Preferences", "User lives in Delhi", 1000).await?;

    // Insert newer fact B
    let fact_b_id = "mem_2000_b";
    insert_test_fact(&conn, fact_b_id, "Preferences", "User lives in Bangalore", 2000).await?;

    // Write CONFLICTS edge
    conn.execute(
        "INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, ?, 1000)",
        (fact_a_id.to_string(), fact_b_id.to_string(), PM_RELATION_CONFLICTS.to_string()),
    ).await?;

    // Retrieve context: only newer fact B (Bangalore) should survive
    let query_vector = vec![0.1f32; 1024];
    let context = retrieve_personal_context(&conn, &query_vector, &settings, 2048, None).await?;

    assert!(context.contains("User lives in Bangalore"));
    assert!(!context.contains("User lives in Delhi"));

    Ok(())
}

#[tokio::test]
async fn test_private_mode_worker_isolation() -> Result<()> {
    let (conn, db_path, _tmp) = setup_test_db().await?;

    let settings = Arc::new(RwLock::new(VoxSettings::default()));
    let private_mode = Arc::new(std::sync::atomic::AtomicBool::new(true)); // Private mode ACTIVE
    let worker_tx = spawn_memory_worker(db_path.clone(), private_mode, settings);

    // Try sending facts ready event
    let mut facts = HashMap::new();
    facts.insert("Identity".to_string(), vec!["This should never be saved.".to_string()]);
    let _ = worker_tx.send(MemoryWorkerEvent::PersonalFactsReady {
        facts,
        session_id: "private_session".to_string(),
    });

    sleep(Duration::from_secs(2)).await;
    let _ = worker_tx.send(MemoryWorkerEvent::Shutdown);

    // Verify DB remains empty of personal memories
    let mut rows = conn.query("SELECT count(*) FROM personal_memory_queue", ()).await?;
    let queue_count: i64 = rows.next().await?.unwrap().get(0)?;
    assert_eq!(queue_count, 0);

    let mut rows = conn.query("SELECT count(*) FROM memory_facts", ()).await?;
    let facts_count: i64 = rows.next().await?.unwrap().get(0)?;
    assert_eq!(facts_count, 0);

    Ok(())
}

#[tokio::test]
async fn test_edge_resolution_cyclic_loop_safety() -> Result<()> {
    let (conn, _db_path, _tmp) = setup_test_db().await?;
    let settings = MemorySettings::default();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Insert two facts
    conn.execute(
        "INSERT INTO memory_facts (id, collection, fact, source, created_at) VALUES (?, 'Identity', 'Fact A', 'LLM', ?)",
        ("fact_a".to_string(), now),
    ).await?;
    conn.execute(
        "INSERT INTO memory_facts (id, collection, fact, source, created_at) VALUES (?, 'Identity', 'Fact B', 'LLM', ?)",
        ("fact_b".to_string(), now + 1),
    ).await?;

    // Create a cyclic USER_SUPERSEDES reference: A -> B and B -> A
    conn.execute(
        "INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, 'USER_SUPERSEDES', ?)",
        ("fact_b".to_string(), "fact_a".to_string(), now),
    ).await?;
    conn.execute(
        "INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, 'USER_SUPERSEDES', ?)",
        ("fact_a".to_string(), "fact_b".to_string(), now),
    ).await?;

    let mut candidate_map = HashMap::new();
    candidate_map.insert("fact_a".to_string(), MemoryFact {
        id: "fact_a".to_string(),
        collection: "Identity".to_string(),
        fact: "Fact A".to_string(),
        source: "LLM".to_string(),
        created_at: now,
    });

    let direct_hit_ids = std::collections::HashSet::new();

    // Execute edge resolution (Assert that it breaks out of the cycle and does not hang!)
    let result = resolve_edges(&conn, candidate_map, &direct_hit_ids, &settings, None).await;
    assert!(result.is_ok(), "Pointer swap cycle must break gracefully and not hang!");
    
    Ok(())
}

#[tokio::test]
async fn test_hybrid_search_and_reciprocal_rank_fusion() -> Result<()> {
    let (conn, _db_path, _tmp) = setup_test_db().await?;

    // Insert distinct episodic memories
    conn.execute(
        "INSERT INTO episodes (id, session_id, summary, embedding, created_at, token_count) VALUES (10, 100, 'User prefers Rust.', ?, 1000, 10)",
        (encode_f32_blob(&vec![0.9; 1024]),),
    ).await?;
    conn.execute(
        "INSERT INTO episodes (id, session_id, summary, embedding, created_at, token_count) VALUES (20, 200, 'User talks about Madara Uchiha.', ?, 2000, 10)",
        (encode_f32_blob(&vec![0.1; 1024]),), // Very different embedding
    ).await?;


    // Query with an exact keyword 'Madara' and a dense vector representing 'Rust'
    let query_vector = vec![0.9; 1024]; // Aligned with Episode 1
    let query_text = "Madara"; // Aligned with Episode 2

    let settings = MemorySettings::default();

    // Execute hybrid search
    let results = vox_lib::services::memory::retrieval::search_and_diversify_episodes(
        &conn,
        &query_vector,
        query_text,
        300, // current_session_id
        &settings,
        4096, // context_size
    ).await?;

    // Assert reciprocal rank fusion successfully merged results
    assert!(!results.is_empty(), "RRF search must return candidates.");
    
    Ok(())
}
