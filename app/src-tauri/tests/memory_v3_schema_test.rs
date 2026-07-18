use anyhow::Result;
use vox_lib::persistence::schema::run_migrations;
use vox_lib::core::constants::{collection_type, PM_TYPE_FOUNDATIONAL, PM_TYPE_OPERATIONAL, PM_TYPE_SEMANTIC};

#[tokio::test]
async fn test_v3_schema_idempotency_and_crud() -> Result<()> {
    let db = turso::Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;

    // 1. Run migrations twice (idempotency check)
    run_migrations(&conn).await?;
    run_migrations(&conn).await?;

    // 2. Validate tables exist
    for table in &["memory_facts", "memory_facts_vectors", "memory_relations", "personal_memory_queue"] {
        let mut rows = conn
            .query(
                &format!("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='{}'", table),
                (),
            )
            .await?;
        let count: i64 = rows.next().await?.unwrap().get(0)?;
        assert_eq!(count, 1, "Table '{}' must be created", table);
    }

    // 3. Test CRUD on memory_facts
    let fact_id = "test_fact_123";
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, created_at) 
         VALUES (?, ?, 'Identity', 'Alex is a system engineer', 'LLM', 'active', 1000)",
        (fact_id, PM_TYPE_FOUNDATIONAL),
    ).await?;

    let mut rows = conn
        .query("SELECT fact, status FROM memory_facts WHERE id = ?", (fact_id,))
        .await?;
    let row = rows.next().await?.unwrap();
    let fact: String = row.get(0)?;
    let status: String = row.get(1)?;
    assert_eq!(fact, "Alex is a system engineer");
    assert_eq!(status, "active");

    // Update status to superseded
    conn.execute("UPDATE memory_facts SET status = 'superseded' WHERE id = ?", (fact_id,)).await?;
    let mut rows = conn
        .query("SELECT status FROM memory_facts WHERE id = ?", (fact_id,))
        .await?;
    let status: String = rows.next().await?.unwrap().get(0)?;
    assert_eq!(status, "superseded");

    Ok(())
}

#[tokio::test]
async fn test_v3_cascading_deletes() -> Result<()> {
    let db = turso::Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    conn.execute("PRAGMA foreign_keys = ON;", ()).await?;
    run_migrations(&conn).await?;

    let fact_id_1 = "fact_1";
    let fact_id_2 = "fact_2";

    // Insert 2 facts
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES (?, 'semantic', 'Projects', 'Vox is cool', 'active', 1000)",
        (fact_id_1,),
    ).await?;
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES (?, 'semantic', 'Projects', 'Limbo is fast', 'active', 2000)",
        (fact_id_2,),
    ).await?;

    // Insert vector
    let mock_emb = vec![0.5f32; 1024];
    let blob = vox_lib::persistence::memory_worker::encode_f32_blob(&mock_emb);
    conn.execute(
        "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES (?, 'Projects', ?)",
        (fact_id_1, blob),
    ).await?;

    // Insert relation
    conn.execute(
        "INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES (?, ?, 'SUPPORTS', 1500)",
        (fact_id_1, fact_id_2),
    ).await?;

    // Verify vector & relation exist
    let mut rows = conn.query("SELECT count(*) FROM memory_facts_vectors WHERE fact_id = ?", (fact_id_1,)).await?;
    assert_eq!(rows.next().await?.unwrap().get::<i64>(0)?, 1);

    let mut rows = conn.query("SELECT count(*) FROM memory_relations WHERE from_id = ? AND to_id = ?", (fact_id_1, fact_id_2)).await?;
    assert_eq!(rows.next().await?.unwrap().get::<i64>(0)?, 1);

    // Delete fact_1
    conn.execute("DELETE FROM memory_facts WHERE id = ?", (fact_id_1,)).await?;

    // Verify cascading deletes worked (vector & relation removed)
    let mut rows = conn.query("SELECT count(*) FROM memory_facts_vectors WHERE fact_id = ?", (fact_id_1,)).await?;
    assert_eq!(rows.next().await?.unwrap().get::<i64>(0)?, 0);

    let mut rows = conn.query("SELECT count(*) FROM memory_relations WHERE from_id = ? AND to_id = ?", (fact_id_1, fact_id_2)).await?;
    assert_eq!(rows.next().await?.unwrap().get::<i64>(0)?, 0);

    Ok(())
}

#[test]
fn test_collection_to_type_mappings() {
    assert_eq!(collection_type("Identity"), PM_TYPE_FOUNDATIONAL);
    assert_eq!(collection_type("Constraints"), PM_TYPE_FOUNDATIONAL);
    assert_eq!(collection_type("Context"), PM_TYPE_OPERATIONAL);
    assert_eq!(collection_type("Tasks"), PM_TYPE_OPERATIONAL);
    assert_eq!(collection_type("Goals"), PM_TYPE_OPERATIONAL);
    assert_eq!(collection_type("Preferences"), PM_TYPE_SEMANTIC);
    assert_eq!(collection_type("Relationships"), PM_TYPE_SEMANTIC);
    assert_eq!(collection_type("Skills"), PM_TYPE_SEMANTIC);
    assert_eq!(collection_type("Projects"), PM_TYPE_SEMANTIC);
}
