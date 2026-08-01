use anyhow::Result;
use turso::Builder;
use vox_lib::persistence::schema::run_migrations;
use vox_lib::services::memory::pipeline::{
    run_stage1_dedup, run_stage2_embed, run_stage3_eval, run_stage4_commit,
};

#[tokio::test]
async fn test_layer2_memory_pipeline_4stage_rigorous() -> Result<()> {
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // 1. Seed existing active Identity fact in memory_facts
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
         VALUES ('mem_identity_v1', 'special_state', 'Identity', 'User is a Junior AI Developer.', 'LLM', 'active', 'sess_1', ?)",
        (now,),
    ).await?;

    // 2. Enqueue synthetic facts into personal_memory_queue
    let facts = vec![
        ("User is a Senior AI Engineer specializing in Rust.", "Identity"), // Near-duplicate / entailing -> should trigger SUPERSEDES
        ("Always use dark mode for UI components.", "Directives"),
        ("Prefers Linux Ubuntu OS and Tauri desktop stack.", "Profile"),
        ("Vox is a real-time voice assistant app.", "Entities"),
        ("Never use raw pointers in safe Rust modules.", "Constraints"),
        ("", "Identity"), // Empty string -> should be superseded in Stage 1
    ];

    for (idx, (fact_text, coll)) in facts.iter().enumerate() {
        conn.execute(
            "INSERT INTO personal_memory_queue (id, fact, collection, source, session_id, status, created_at)
             VALUES (?, ?, ?, 'LLM', 'sess_1', 'staged_pending', ?)",
            ((idx + 1) as i64, fact_text.to_string(), coll.to_string(), now + idx as i64),
        ).await?;
    }

    // 3. Stage 1: Dedup
    let t1 = std::time::Instant::now();
    let n1 = run_stage1_dedup(&conn).await?;
    let d1 = t1.elapsed();
    assert_eq!(n1, 6, "Stage 1 must process all 6 items");
    println!("[Stage 1 Dedup] Processed {} items in {:?}", n1, d1);

    // Verify empty fact was marked 'superseded'
    let mut row_empty = conn.query("SELECT status FROM personal_memory_queue WHERE id = 6", ()).await?;
    if let Some(r) = row_empty.next().await? {
        let status: String = r.get(0)?;
        assert_eq!(status, "superseded");
    }

    // 4. Stage 2: Embed
    let t2 = std::time::Instant::now();
    let n2 = run_stage2_embed(&conn).await?;
    let d2 = t2.elapsed();
    assert_eq!(n2, 5, "Stage 2 must embed all 5 deduped items");
    println!("[Stage 2 Embed] Embedded {} items in {:?}", n2, d2);

    // Verify vectors were populated
    let mut row_vec = conn.query("SELECT vector FROM personal_memory_queue WHERE id = 1", ()).await?;
    if let Some(r) = row_vec.next().await? {
        let vec_blob: Option<Vec<u8>> = r.get(0)?;
        assert!(vec_blob.is_some() && !vec_blob.unwrap().is_empty(), "Vector blob must be populated");
    }

    // 5. Stage 3: Eval (ONNX Inference)
    let t3 = std::time::Instant::now();
    let n3 = run_stage3_eval(&conn).await?;
    let d3 = t3.elapsed();
    assert_eq!(n3, 5, "Stage 3 must evaluate all 5 embedded items");
    println!("[Stage 3 Eval] Evaluated {} items in {:?}", n3, d3);

    // 6. Stage 4: Commit & Prune
    let t4 = std::time::Instant::now();
    let n4 = run_stage4_commit(&conn).await?;
    let d4 = t4.elapsed();
    assert_eq!(n4, 6, "Stage 4 must commit and prune all 6 queue items");
    println!("[Stage 4 Commit] Committed and pruned {} items in {:?}", n4, d4);

    // Verify queue is completely empty after Stage 4
    let mut count_row = conn.query("SELECT COUNT(*) FROM personal_memory_queue", ()).await?;
    let queue_count: i64 = count_row.next().await?.unwrap().get(0)?;
    assert_eq!(queue_count, 0, "Queue must be empty after Stage 4 commit");

    // Verify active facts count in memory_facts
    let mut fact_count_row = conn.query("SELECT COUNT(*) FROM memory_facts WHERE status = 'active'", ()).await?;
    let active_facts_count: i64 = fact_count_row.next().await?.unwrap().get(0)?;
    assert!(active_facts_count >= 5, "At least 5 active facts must exist in memory_facts");

    println!("[Integration Test Success] Layer 2 4-stage memory pipeline test passed with metrics!");
    Ok(())
}
