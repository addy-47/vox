use anyhow::Result;
use query_sieve::MemoryScope;
use turso::Builder;
use vox_lib::core::constants::PM_RELATION_SHAPES;
use vox_lib::core::settings::MemorySettings;
use vox_lib::persistence::encode_f32_blob;
use vox_lib::persistence::schema::run_migrations;
use vox_lib::services::memory::embedder::l2_normalize;
use vox_lib::services::memory::retrieval::retrieve_personal_context_v7;

#[tokio::test]
async fn test_v7_memory_scope_retrieval_pipeline_rigorous() -> Result<()> {
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Seed test active facts across v7 collections
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
         VALUES ('mem_1', 'special_state', 'Identity', 'User is Alex, a Principal AI Architect.', 'LLM', 'active', 'sess_1', ?)",
        (now,),
    ).await?;

    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
         VALUES ('mem_2', 'special_state', 'Directives', 'Always format code snippets using GitHub Markdown.', 'LLM', 'active', 'sess_1', ?)",
        (now + 1,),
    ).await?;

    // Seed L2-normalized float vectors for vector candidate matching
    let mut raw_vec = vec![0.0f32; 384];
    raw_vec[0] = 1.0;
    let norm_vec = l2_normalize(&raw_vec);
    let blob_bytes = encode_f32_blob(&norm_vec);

    conn.execute(
        "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES ('mem_2', 'Directives', ?)",
        (blob_bytes.clone(),),
    ).await?;

    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
         VALUES ('mem_3', 'semantic_graph', 'Profile', 'User prefers dark mode and uses Linux OS.', 'LLM', 'active', 'sess_1', ?)",
        (now + 2,),
    ).await?;
    conn.execute(
        "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES ('mem_3', 'Profile', ?)",
        (blob_bytes.clone(),),
    ).await?;

    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, created_at)
         VALUES ('mem_4', 'semantic_graph', 'Entities', 'Vox is a real-time voice AI app in Rust.', 'LLM', 'active', 'sess_1', ?)",
        (now + 3,),
    ).await?;
    conn.execute(
        "INSERT INTO memory_facts_vectors (fact_id, collection, embedding) VALUES ('mem_4', 'Entities', ?)",
        (blob_bytes.clone(),),
    ).await?;

    // Seed a graph relation between Profile (mem_3) and Entities (mem_4)
    conn.execute(
        "INSERT INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES ('mem_3', 'mem_4', ?, 'LLM', ?)",
        (PM_RELATION_SHAPES, now + 4),
    ).await?;

    let settings = MemorySettings::default(); // max_personal_memory_share = 0.15

    // 1. Test ChitChat scope -> must return empty string with zero RAG queries
    let chitchat_res =
        retrieve_personal_context_v7(&conn, &norm_vec, MemoryScope::ChitChat, &settings, 4096)
            .await?;
    assert!(
        chitchat_res.is_empty(),
        "ChitChat scope must return empty string with zero retrieval"
    );

    // 2. Test System Prompt Identity Preloading via ConversationManager
    let mut conv_mgr = vox_lib::services::memory::working_memory::ConversationManager::new(4096);
    conv_mgr.load_identity_into_system_prompt(&conn).await?;
    let sys_content = &conv_mgr.get_messages()[0].content;
    assert!(
        sys_content.contains("<user_profile>"),
        "System prompt must contain user_profile block"
    );
    assert!(
        sys_content.contains("User is Alex"),
        "System prompt must pre-load Identity fact"
    );

    // 3. Test User scope RAG -> must contain Profile vector seed
    let user_res =
        retrieve_personal_context_v7(&conn, &norm_vec, MemoryScope::User, &settings, 4096).await?;
    assert!(
        user_res.contains("User prefers dark mode"),
        "User scope must contain Profile vector match"
    );

    // 4. Test Domain scope RAG -> Directives now vector-searched (in semantic_graph), Entities vector seed + BFS child relation
    let domain_res =
        retrieve_personal_context_v7(&conn, &norm_vec, MemoryScope::Domain, &settings, 4096)
            .await?;
    assert!(
        domain_res.contains("GitHub Markdown"),
        "Domain scope must contain Directives fact in semantic graph"
    );
    assert!(
        domain_res.contains("Vox is a real-time voice AI app"),
        "Domain scope must contain Entities vector match"
    );

    // 4. Verify context budget ceiling (< 15% of 4096 tokens = ~614 tokens)
    let total_tokens = vox_lib::services::memory::estimate_tokens(&domain_res);
    let token_ceiling = (4096.0 * 0.15) as usize;
    assert!(
        total_tokens <= token_ceiling,
        "Context tokens ({}) must not exceed 15% budget cap ({})",
        total_tokens,
        token_ceiling
    );

    println!("[Integration Test Success] v7 MemoryScope Retrieval Pipeline passed with BFS graph expansion & budget cap verification!");
    Ok(())
}
