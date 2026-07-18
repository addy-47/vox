use anyhow::Result;
use std::collections::{HashMap, HashSet};
use vox_lib::persistence::schema::run_migrations;
use vox_lib::core::settings::MemorySettings;
use vox_lib::services::memory::personal_memory::{
    retrieve_personal_context, resolve_edges, MemoryFact
};
use vox_lib::core::constants::{
    PM_TYPE_FOUNDATIONAL, PM_TYPE_OPERATIONAL, PM_TYPE_SEMANTIC,
    PM_RELATION_USER_SUPERSEDES, PM_RELATION_CONFLICTS
};

#[tokio::test]
async fn test_v3_tier1_budget_and_timeline() -> Result<()> {
    let db = turso::Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Seed foundational facts
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES ('id_1', ?, 'Identity', 'User name is Alex', 'active', ?)",
        (PM_TYPE_FOUNDATIONAL, now),
    ).await?;
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES ('con_1', ?, 'Constraints', 'No python backends allowed', 'active', ?)",
        (PM_TYPE_FOUNDATIONAL, now),
    ).await?;

    // Seed operational facts
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES ('task_1', ?, 'Tasks', 'Complete memory spec (active)', 'active', ?)",
        (PM_TYPE_OPERATIONAL, now),
    ).await?;

    // Seed context facts (within 12h window)
    let five_mins_ago = now - (5 * 60 * 1000);
    let two_hours_ago = now - (2 * 3600 * 1000);
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES ('ctx_1', ?, 'Context', 'Discussed calculus derivatives', 'active', ?)",
        (PM_TYPE_OPERATIONAL, five_mins_ago),
    ).await?;
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES ('ctx_2', ?, 'Context', 'Discussed movie Interstellar', 'active', ?)",
        (PM_TYPE_OPERATIONAL, two_hours_ago),
    ).await?;

    // Call retrieve_personal_context with a tiny mock embedding vector (all zeros)
    let settings = MemorySettings::default();
    let query_emb = vec![0.0f32; 1024];

    // Context size = 2048 tokens. 7% budget = 143 tokens.
    let ctx_block = retrieve_personal_context(&conn, &query_emb, &settings, 2048, None).await?;

    println!("Tier 1 Context Block:\n{}", ctx_block);

    assert!(ctx_block.contains("[Identity]"));
    assert!(ctx_block.contains("User name is Alex"));
    assert!(ctx_block.contains("[Constraints]"));
    assert!(ctx_block.contains("No python backends allowed"));
    assert!(ctx_block.contains("[Active Tasks]"));
    assert!(ctx_block.contains("Complete memory spec"));
    assert!(ctx_block.contains("[Past Contexts within the Last 12 Hours]"));
    assert!(ctx_block.contains("Discussed calculus derivatives"));
    assert!(ctx_block.contains("Discussed movie Interstellar"));

    Ok(())
}

#[tokio::test]
async fn test_v3_distant_memory_fallback() -> Result<()> {
    let db = turso::Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Seed one very old context (10 days ago = 240 hours ago)
    let ten_days_ago = now - (10 * 24 * 3600 * 1000);
    conn.execute(
        "INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES ('ctx_old', 'operational', 'Context', 'Discussed quantum physics', 'active', ?)",
        (ten_days_ago,),
    ).await?;

    let settings = MemorySettings::default();
    let query_emb = vec![0.0f32; 1024];

    let ctx_block = retrieve_personal_context(&conn, &query_emb, &settings, 2048, None).await?;
    println!("Distant Fallback Block:\n{}", ctx_block);

    assert!(ctx_block.contains("[Recollection (Distant Memory)]"));
    assert!(ctx_block.contains("Discussed quantum physics"));

    Ok(())
}

#[tokio::test]
async fn test_v3_edge_resolution_supersedes_and_conflicts() -> Result<()> {
    let db = turso::Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // 1. SUPERSESSION: fact_1 (old preferences) superseded by fact_2 (new preferences)
    let fact_1 = MemoryFact {
        id: "f_1".to_string(),
        fact_type: PM_TYPE_SEMANTIC.to_string(),
        collection: "Preferences".to_string(),
        fact: "User likes milk chocolate".to_string(),
        source: "LLM".to_string(),
        status: "active".to_string(),
        created_at: now - 10000,
    };
    let fact_2 = MemoryFact {
        id: "f_2".to_string(),
        fact_type: PM_TYPE_SEMANTIC.to_string(),
        collection: "Preferences".to_string(),
        fact: "User likes dark chocolate".to_string(),
        source: "LLM".to_string(),
        status: "active".to_string(),
        created_at: now - 5000,
    };

    // Insert facts
    conn.execute("INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES ('f_1', 'semantic', 'Preferences', 'User likes milk chocolate', 'active', ?)", (now - 10000,)).await?;
    conn.execute("INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES ('f_2', 'semantic', 'Preferences', 'User likes dark chocolate', 'active', ?)", (now - 5000,)).await?;
    
    // Write supersedes relation
    conn.execute("INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES ('f_2', 'f_1', ?, ?)", (PM_RELATION_USER_SUPERSEDES.to_string(), now)).await?;

    // 2. CONFLICT: fact_3 conflicts with fact_4. fact_4 is newer so fact_3 should be shadowed.
    let fact_3 = MemoryFact {
        id: "f_3".to_string(),
        fact_type: PM_TYPE_SEMANTIC.to_string(),
        collection: "Preferences".to_string(),
        fact: "User prefers macOS".to_string(),
        source: "LLM".to_string(),
        status: "active".to_string(),
        created_at: now - 8000,
    };
    let fact_4 = MemoryFact {
        id: "f_4".to_string(),
        fact_type: PM_TYPE_SEMANTIC.to_string(),
        collection: "Preferences".to_string(),
        fact: "User prefers Linux".to_string(),
        source: "LLM".to_string(),
        status: "active".to_string(),
        created_at: now - 2000,
    };

    conn.execute("INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES ('f_3', 'semantic', 'Preferences', 'User prefers macOS', 'active', ?)", (now - 8000,)).await?;
    conn.execute("INSERT INTO memory_facts (id, type, collection, fact, status, created_at) VALUES ('f_4', 'semantic', 'Preferences', 'User prefers Linux', 'active', ?)", (now - 2000,)).await?;

    // Write conflict relation
    conn.execute("INSERT INTO memory_relations (from_id, to_id, relation, created_at) VALUES ('f_3', 'f_4', ?, ?)", (PM_RELATION_CONFLICTS.to_string(), now)).await?;

    // Map candidate facts to run resolve_edges
    let mut candidate_map = HashMap::new();
    candidate_map.insert(fact_1.id.clone(), fact_1);
    candidate_map.insert(fact_2.id.clone(), fact_2);
    candidate_map.insert(fact_3.id.clone(), fact_3);
    candidate_map.insert(fact_4.id.clone(), fact_4);

    let mut direct_hits = HashSet::new();
    direct_hits.insert("f_1".to_string());
    direct_hits.insert("f_3".to_string());

    let settings = MemorySettings::default();
    let resolved = resolve_edges(&conn, candidate_map, &direct_hits, &settings, None).await?;

    let resolved_ids: HashSet<String> = resolved.iter().map(|f| f.id.clone()).collect();
    
    // f_1 should be replaced by f_2
    assert!(!resolved_ids.contains("f_1"), "f_1 must be superseded by f_2");
    assert!(resolved_ids.contains("f_2"), "f_2 must be present in resolved list");

    // f_3 conflicts with f_4. f_4 is newer (now - 2000 vs now - 8000) so f_3 should be shadowed/suppressed.
    assert!(!resolved_ids.contains("f_3"), "f_3 must be suppressed due to newer conflict f_4");
    assert!(resolved_ids.contains("f_4"), "f_4 must survive conflict");

    Ok(())
}
