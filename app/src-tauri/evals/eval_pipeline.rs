//! ============================================================================
//! eval_pipeline.rs — Ladder Eval 2: 4-Stage Ingestion Pipeline Evaluation
//! ============================================================================
//! Category     : Evaluation Script
//! Component    : services::memory / evals
//! Prerequisites: evals/results/stage_1_compaction.db (or JSON input)
//! Execution    : cargo run --example eval_pipeline
//! Metrics      : Stage Latencies (ms), Deduplication Rate, Relations Created, DB Write Count
//! ============================================================================

mod llm_judge;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use turso::Builder;
use vox_lib::services::memory::pipeline::drain_pipeline_queue;

fn resolve_path(rel: &str) -> PathBuf {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if base.ends_with("app/src-tauri") {
        base.join(rel)
    } else {
        base.join("app/src-tauri").join(rel)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Vox Memory Subsystem: Ladder Eval 2 (4-Stage Pipeline & Ingestion) ===");

    let input_db_path = resolve_path("evals/results/stage_1_compaction.db");
    let output_db_path = resolve_path("evals/results/stage_2_pipeline.db");

    if !input_db_path.exists() {
        return Err(anyhow::anyhow!(
            "Input DB at {:?} not found. Please run eval_compaction first.",
            input_db_path
        ));
    }

    if let Some(parent) = output_db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Copy Eval 1 output database to stage_2_pipeline.db
    let _ = std::fs::copy(&input_db_path, &output_db_path)?;

    let abs_db_path = std::fs::canonicalize(&output_db_path)?;
    let db_path_str = abs_db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid output DB path {:?}", abs_db_path))?;
    let db = Builder::new_local(db_path_str).build().await?;
    let conn = db.connect()?;

    println!("[Eval 2] Running 4-stage memory pipeline on {:?}", output_db_path);

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let start_time = std::time::Instant::now();

    // Execute 4-stage pipeline until queue is completely drained
    let processed_count = drain_pipeline_queue(&conn, &cancel_flag).await?;
    let total_duration = start_time.elapsed();

    println!("[Eval 2] Pipeline finished. Processed {} items in {:?}", processed_count, total_duration);

    // Query operational stage metrics from memory_pipeline_metrics table
    let mut metrics_rows = conn
        .query(
            "SELECT stage_name, items_claimed, items_processed, items_superseded, relations_created, duration_ms 
             FROM memory_pipeline_metrics ORDER BY id ASC",
            (),
        )
        .await?;

    let mut stage_metrics_list = Vec::new();
    println!("\n--- Operational Pipeline Stage Metrics ---");
    while let Some(row) = metrics_rows.next().await? {
        let stage: String = row.get(0)?;
        let claimed: i64 = row.get(1)?;
        let processed: i64 = row.get(2)?;
        let superseded: i64 = row.get(3)?;
        let relations: i64 = row.get(4)?;
        let duration: i64 = row.get(5)?;
        println!(
            "Stage: {:<15} | Claimed: {:<3} | Processed: {:<3} | Superseded: {:<3} | Relations: {:<3} | Duration: {} ms",
            stage, claimed, processed, superseded, relations, duration
        );
        stage_metrics_list.push(serde_json::json!({
            "stage_name": stage,
            "items_claimed": claimed,
            "items_processed": processed,
            "items_superseded": superseded,
            "relations_created": relations,
            "duration_ms": duration,
        }));
    }

    // Query Queue Final Status Breakdown
    let mut queue_status_counts = std::collections::HashMap::new();
    let mut q_rows = conn.query("SELECT status, COUNT(*) FROM personal_memory_queue GROUP BY status", ()).await?;
    while let Some(row) = q_rows.next().await? {
        let status: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        queue_status_counts.insert(status, count);
    }

    // Query Memory Facts Breakdown
    let mut fact_status_counts = std::collections::HashMap::new();
    let mut f_rows = conn.query("SELECT status, COUNT(*) FROM memory_facts GROUP BY status", ()).await?;
    while let Some(row) = f_rows.next().await? {
        let status: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        fact_status_counts.insert(status, count);
    }

    // Query Memory Relations Breakdown by Type
    let mut relation_type_counts = std::collections::HashMap::new();
    let mut r_rows = conn.query("SELECT relation, COUNT(*) FROM memory_relations GROUP BY relation", ()).await?;
    while let Some(row) = r_rows.next().await? {
        let rel_type: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        relation_type_counts.insert(rel_type, count);
    }

    // Query Sample Superseded Facts
    let mut sample_superseded = Vec::new();
    let mut s_rows = conn.query("SELECT fact, collection FROM personal_memory_queue WHERE status = 'superseded' LIMIT 15", ()).await?;
    while let Some(row) = s_rows.next().await? {
        let fact: String = row.get(0)?;
        let col: String = row.get(1)?;
        sample_superseded.push(serde_json::json!({ "fact": fact, "collection": col }));
    }

    let report_path = resolve_path("evals/results/eval_pipeline_results.json");
    let report = serde_json::json!({
        "processed_count": processed_count,
        "total_duration_ms": total_duration.as_millis(),
        "avg_ms_per_fact": if processed_count > 0 { total_duration.as_millis() as f64 / processed_count as f64 } else { 0.0 },
        "output_db_path": output_db_path.to_string_lossy().to_string(),
        "stage_metrics": stage_metrics_list,
        "queue_status_counts": queue_status_counts,
        "fact_status_counts": fact_status_counts,
        "relation_type_counts": relation_type_counts,
        "sample_superseded_facts": sample_superseded,
    });
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    println!("\n=========================================================================");
    println!("[Eval 2 Deterministic & Semantic Metrics Summary]");
    println!("  Total Items Processed     : {}", processed_count);
    println!("  Total Execution Time      : {:?} ({:.2} ms/fact)", total_duration, if processed_count > 0 { total_duration.as_millis() as f64 / processed_count as f64 } else { 0.0 });
    println!("  Queue Status Breakdown    : {:?}", queue_status_counts);
    println!("  Committed Facts Breakdown : {:?}", fact_status_counts);
    println!("  Relations Breakdown       : {:?}", relation_type_counts);
    println!("  Full JSON Report Saved To : {:?}", report_path);
    println!("=========================================================================\n");

    Ok(())
}
