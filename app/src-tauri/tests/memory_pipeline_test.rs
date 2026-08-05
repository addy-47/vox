use anyhow::Result;
use turso::Builder;
use vox_lib::persistence::encode_f32_blob;
use vox_lib::persistence::queries::{fetch_inter_subfloor_candidates, fetch_intra_subfloor_candidates};
use vox_lib::persistence::schema::run_migrations;
use vox_lib::services::memory::embedder::{ensure_embedder_loaded, generate_embedding, l2_normalize};
use vox_lib::services::memory::pipeline::batch_result::{CandidateAuditLog, DedupAuditLog};
use vox_lib::services::memory::pipeline::{
    run_stage1_dedup, run_stage2_embed, run_stage3_eval_with_metrics_seq, run_stage4_commit,
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
    let run_id = "test_run_pipeline_001";

    let _ = ensure_embedder_loaded(true);
    let seed_fact = "User is a Junior AI Developer.";
    let blob_bytes = if let Ok(Some(vec)) = generate_embedding(seed_fact) {
        encode_f32_blob(&vec)
    } else {
        let mut raw_vec = vec![0.0f32; 384];
        raw_vec[0] = 1.0;
        encode_f32_blob(&l2_normalize(&raw_vec))
    };

    let mut norm_vec = vec![0.0f32; 384];
    norm_vec[0] = 1.0;
    let norm_vec = l2_normalize(&norm_vec);

    // 1. Seed existing active Identity fact in memory_facts & vector DB
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
         VALUES ('mem_identity_v1', 'special_state', 'Identity', 'User is a Junior AI Developer.', 'LLM', 'active', 'sess_1', ?)",
        (now,),
    ).await?;
    conn.execute(
        "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES ('mem_identity_v1', 'Identity', ?)",
        (blob_bytes.clone(),),
    ).await?;

    // 2. Enqueue synthetic facts into personal_memory_queue
    let facts = vec![
        ("User is a Senior AI Engineer specializing in Rust.", "Identity"), // item 1
        ("Always use dark mode for UI components.", "Directives"),           // item 2
        ("Prefers Linux Ubuntu OS and Tauri desktop stack.", "Profile"),       // item 3
        ("Vox is a real-time voice assistant app.", "Entities"),             // item 4
        ("Never use raw pointers in safe Rust modules.", "Constraints"),       // item 5
        ("", "Identity"),                                                      // item 6: Empty string -> Stage 1 drop
        ("User is a Senior AI Engineer specializing in Rust.", "Identity"), // item 7: Duplicate of item 1 -> Stage 1 Jaccard drop
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
    assert_eq!(n1, 7, "Stage 1 must process all 7 items");
    println!("[Stage 1 Dedup] Processed {} items in {:?}", n1, d1);

    // Verify empty fact (item 6) was marked 'superseded'
    let mut row_empty = conn.query("SELECT status FROM personal_memory_queue WHERE id = 6", ()).await?;
    if let Some(r) = row_empty.next().await? {
        let status: String = r.get(0)?;
        assert_eq!(status, "superseded");
    }

    // Verify Stage 1 Jaccard duplicate (item 7) was marked 'superseded' with dedup_match_json
    let mut row_jaccard = conn.query("SELECT status, dedup_match_json FROM personal_memory_queue WHERE id = 7", ()).await?;
    if let Some(r) = row_jaccard.next().await? {
        let status: String = r.get(0)?;
        let match_json: Option<String> = r.get(1)?;
        assert_eq!(status, "superseded");
        assert!(match_json.is_some(), "Stage 1 dropped duplicate must populate dedup_match_json");
        let dedup_log: DedupAuditLog = serde_json::from_str(&match_json.unwrap())?;
        assert_eq!(dedup_log.stage, "stage1_jaccard");
        assert_eq!(dedup_log.action, "duplicate_dropped");
        assert!(!dedup_log.matched_fact_id.is_empty(), "Matched fact ID must be non-empty");
    }

    // 4. Stage 2: Embed
    let t2 = std::time::Instant::now();
    let n2 = run_stage2_embed(&conn).await?;
    let d2 = t2.elapsed();
    assert_eq!(n2, 5, "Stage 2 must embed all 5 active deduped items");
    println!("[Stage 2 Embed] Embedded {} items in {:?}", n2, d2);

    // Verify vectors were populated
    let mut row_vec = conn.query("SELECT vector FROM personal_memory_queue WHERE id = 1", ()).await?;
    if let Some(r) = row_vec.next().await? {
        let vec_blob: Option<Vec<u8>> = r.get(0)?;
        assert!(vec_blob.is_some() && !vec_blob.unwrap().is_empty(), "Vector blob must be populated");
    }

    // 5. Stage 3: Eval (ONNX Inference)
    let t3 = std::time::Instant::now();
    let n3 = run_stage3_eval_with_metrics_seq(&conn, run_id, 1).await?;
    let d3 = t3.elapsed();
    assert_eq!(n3, 5, "Stage 3 must evaluate all 5 embedded items");
    println!("[Stage 3 Eval] Evaluated {} items in {:?}", n3, d3);

    // Verify audit_json on evaluated items that matched candidates
    let mut audit_row = conn.query("SELECT audit_json FROM personal_memory_queue WHERE audit_json IS NOT NULL", ()).await?;
    let mut found_audit = false;
    while let Some(r) = audit_row.next().await? {
        found_audit = true;
        let audit_json_raw: String = r.get(0)?;
        let candidate_logs: Vec<CandidateAuditLog> = serde_json::from_str(&audit_json_raw)?;
        for log in &candidate_logs {
            assert!(!log.engine.is_empty(), "Engine field must be populated");
            assert!(!log.candidate_source.is_empty(), "Candidate source must be memory_facts or queue_in_flight");
        }
    }
    assert!(found_audit, "At least one evaluated queue item must populate audit_json when candidate matches exist");

    // 6. Subfloor Candidate Query Verification
    let intra_sub = fetch_intra_subfloor_candidates(&conn, "Identity", &norm_vec, 0.25, 0.80, None).await?;
    println!("Intra subfloor candidate search returned {} candidates", intra_sub.len());
    let inter_sub = fetch_inter_subfloor_candidates(&conn, &["Profile", "Entities"], &norm_vec, 0.25, 0.70, None).await?;
    println!("Inter subfloor candidate search returned {} candidates", inter_sub.len());

    // 7. Verify Operational Metrics Population in memory_pipeline_metrics
    let mut metrics_row = conn.query(
        "SELECT COUNT(*) FROM memory_pipeline_metrics WHERE run_id = ?",
        (run_id,),
    ).await?;
    let metrics_count: i64 = metrics_row.next().await?.unwrap().get(0)?;
    assert!(metrics_count >= 1, "At least 1 stage metrics record must exist for run_id");

    // 8. Stage 4: Commit & Prune
    let t4 = std::time::Instant::now();
    let n4 = run_stage4_commit(&conn).await?;
    let d4 = t4.elapsed();
    assert_eq!(n4, 7, "Stage 4 must commit and prune all 7 queue items");
    println!("[Stage 4 Commit] Committed and pruned {} items in {:?}", n4, d4);

    // Verify queue is completely empty after Stage 4
    let mut count_row = conn.query("SELECT COUNT(*) FROM personal_memory_queue", ()).await?;
    let queue_count: i64 = count_row.next().await?.unwrap().get(0)?;
    assert_eq!(queue_count, 0, "Queue must be empty after Stage 4 commit");

    // Verify active facts count in memory_facts
    let mut fact_count_row = conn.query("SELECT COUNT(*) FROM memory_facts WHERE status = 'active'", ()).await?;
    let active_facts_count: i64 = fact_count_row.next().await?.unwrap().get(0)?;
    assert!(active_facts_count >= 5, "At least 5 active facts must exist in memory_facts");

    println!("[Integration Test Success] Layer 2 4-stage memory pipeline test passed with metrics and audit assertions!");
    Ok(())
}
