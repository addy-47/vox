use anyhow::Result;
use std::collections::HashMap;
use vox_lib::persistence::schema::run_migrations;
use vox_lib::core::settings::MemorySettings;
use vox_lib::persistence::memory_worker::{
    enqueue_personal_facts, process_one_queue_item, session_end_consolidation
};
use vox_lib::core::constants::{
    PM_QUEUE_STATUS_COMPLETED, PM_TYPE_FOUNDATIONAL, PM_TYPE_OPERATIONAL
};

#[tokio::test]
async fn test_v3_worker_type_aware_processing() -> Result<()> {
    let db = turso::Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    let session_id = "session_999";

    // 1. Enqueue facts of different collections
    let mut facts = HashMap::new();
    facts.insert("Identity".to_string(), vec!["User is named Alex".to_string()]); // Foundational (pending)
    facts.insert("Tasks".to_string(), vec!["Call doctor (pending)".to_string()]); // Operational (staged)

    enqueue_personal_facts(&conn, facts, session_id).await?;

    // Verify Identity is enqueued as pending
    let mut rows = conn.query("SELECT count(*) FROM personal_memory_queue WHERE status = 'pending'", ()).await?;
    assert_eq!(rows.next().await?.unwrap().get::<i64>(0)?, 1);

    // Verify Tasks is enqueued as staged
    let mut rows = conn.query("SELECT count(*) FROM personal_memory_queue WHERE status = 'staged'", ()).await?;
    assert_eq!(rows.next().await?.unwrap().get::<i64>(0)?, 1);

    // 2. Process pending items (which is Identity)
    let settings = MemorySettings::default();
    let processed = process_one_queue_item(&conn, &settings).await?;
    assert!(processed, "Must process one pending item");

    // Identity should now be inserted directly into memory_facts (it will generate embedding/NLI)
    let mut rows = conn.query("SELECT fact, type, status FROM memory_facts WHERE collection = 'Identity'", ()).await?;
    let row = rows.next().await?.unwrap();
    assert_eq!(row.get::<String>(0)?, "User is named Alex");
    assert_eq!(row.get::<String>(1)?, PM_TYPE_FOUNDATIONAL);
    assert_eq!(row.get::<String>(2)?, "active");

    // Queue status for Identity should be completed
    let mut rows = conn.query("SELECT status FROM personal_memory_queue WHERE collection = 'Identity'", ()).await?;
    assert_eq!(rows.next().await?.unwrap().get::<String>(0)?, PM_QUEUE_STATUS_COMPLETED);

    Ok(())
}

#[tokio::test]
async fn test_v3_session_end_consolidation() -> Result<()> {
    let db = turso::Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    let session_id = "session_123";

    // Seed staged items in personal_memory_queue
    conn.execute(
        "INSERT INTO personal_memory_queue (fact, collection, status, session_id, created_at)
         VALUES ('Active task A', 'Tasks', 'staged', ?, 1000)",
        (session_id,),
    ).await?;
    conn.execute(
        "INSERT INTO personal_memory_queue (fact, collection, status, session_id, created_at)
         VALUES ('Long term goal B', 'Goals', 'staged', ?, 1100)",
        (session_id,),
    ).await?;

    // Run consolidation sweep
    let summary = "Discussed various task plans and defined goals.";
    session_end_consolidation(&conn, session_id, summary).await?;

    // 1. Verify staged items are deleted/updated (i.e. status is no longer 'staged')
    let mut rows = conn.query("SELECT count(*) FROM personal_memory_queue WHERE session_id = ? AND status = 'staged'", (session_id,)).await?;
    assert_eq!(rows.next().await?.unwrap().get::<i64>(0)?, 0);

    // Verify Tasks and Goals are now 'pending' in the queue
    let mut rows = conn.query("SELECT count(*) FROM personal_memory_queue WHERE session_id = ? AND status = 'pending'", (session_id,)).await?;
    assert_eq!(rows.next().await?.unwrap().get::<i64>(0)?, 2);

    // 2. Verify Context fact was written directly to memory_facts
    let mut rows = conn.query("SELECT fact, collection, type, status FROM memory_facts WHERE collection = 'Context'", ()).await?;
    let row = rows.next().await?.unwrap();
    assert_eq!(row.get::<String>(0)?, summary);
    assert_eq!(row.get::<String>(1)?, "Context");
    assert_eq!(row.get::<String>(2)?, PM_TYPE_OPERATIONAL);
    assert_eq!(row.get::<String>(3)?, "active");

    // 3. Process the pending queue items (which are Tasks and Goals)
    let settings = MemorySettings::default();
    let processed1 = process_one_queue_item(&conn, &settings).await?;
    assert!(processed1, "Must process first pending item");
    let processed2 = process_one_queue_item(&conn, &settings).await?;
    assert!(processed2, "Must process second pending item");

    // 4. Verify operational facts are now in memory_facts
    let mut rows = conn.query("SELECT fact, collection, type, status FROM memory_facts WHERE session_id = ? AND collection IN ('Tasks', 'Goals') ORDER BY collection ASC", (session_id,)).await?;
    
    let mut facts_list = Vec::new();
    while let Some(row) = rows.next().await? {
        facts_list.push((row.get::<String>(0)?, row.get::<String>(1)?, row.get::<String>(2)?, row.get::<String>(3)?));
    }
    
    assert_eq!(facts_list.len(), 2);
    
    // Check Goals fact
    assert_eq!(facts_list[0].0, "Long term goal B");
    assert_eq!(facts_list[0].1, "Goals");
    assert_eq!(facts_list[0].2, PM_TYPE_OPERATIONAL);
    assert_eq!(facts_list[0].3, "active");

    // Check Tasks fact
    assert_eq!(facts_list[1].0, "Active task A");
    assert_eq!(facts_list[1].1, "Tasks");
    assert_eq!(facts_list[1].2, PM_TYPE_OPERATIONAL);
    assert_eq!(facts_list[1].3, "active");

    Ok(())
}
