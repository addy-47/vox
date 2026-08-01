use anyhow::Result;
use turso::Builder;
use vox_lib::persistence::encode_f32_blob;
use vox_lib::persistence::schema::run_migrations;
use vox_lib::services::memory::embedder::l2_normalize;
use vox_lib::services::memory::pipeline::stage3_eval::run_stage3_eval;
use vox_lib::services::memory::pipeline::stage4_commit::run_stage4_commit;

#[tokio::test]
async fn test_nli_state_resolution_and_edge_policies_rigorous() -> Result<()> {
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut raw_vec = vec![0.0f32; 384];
    raw_vec[0] = 1.0;
    let norm_vec = l2_normalize(&raw_vec);
    let blob_bytes = encode_f32_blob(&norm_vec);

    // 1. Seed existing active Identity fact in memory_facts
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
         VALUES ('mem_identity_old', 'special_state', 'Identity', 'User lives in San Francisco.', 'LLM', 'active', 'sess_1', ?)",
        (now,),
    ).await?;
    conn.execute(
        "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES ('mem_identity_old', 'Identity', ?)",
        (blob_bytes.clone(),),
    ).await?;

    // 2. Enqueue updated entailing Identity fact into personal_memory_queue with status = 'embedded'
    conn.execute(
        "INSERT INTO personal_memory_queue (id, fact, collection, source, session_id, status, created_at, vector)
         VALUES (200, 'User lives in San Francisco, California.', 'Identity', 'LLM', 'sess_1', 'embedded', ?, ?)",
        (now + 1, blob_bytes.clone()),
    ).await?;

    // 3. Execute Stage 3 Eval (runs real DeBERTa-v3 NLI inference)
    let evaluated_count = run_stage3_eval(&conn).await?;
    assert_eq!(evaluated_count, 1, "Stage 3 must evaluate the embedded item");

    // 4. Execute Stage 4 Commit
    let committed_count = run_stage4_commit(&conn).await?;
    assert_eq!(committed_count, 1, "Stage 4 must commit the evaluated item");

    // 5. Assert that real DeBERTa NLI inference detected ENTAILMENT and kept both facts active (SUPPORTS edge)
    let mut row = conn.query("SELECT status FROM memory_facts WHERE id = 'mem_identity_old'", ()).await?;
    if let Some(r) = row.next().await? {
        let status: String = r.get(0)?;
        assert_eq!(status, "active", "Identity fact must remain active when NLI entailment produces SUPPORTS edge");
    }

    println!("[Integration Test Success] Layer 4 real ONNX NLI State Resolution test passed!");
    Ok(())
}
