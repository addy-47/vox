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
use vox_lib::services::memory::pipeline::run_pipeline_cycle;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Vox Memory Subsystem: Ladder Eval 2 (4-Stage Pipeline & Ingestion) ===");

    let input_db_path = PathBuf::from("evals/results/stage_1_compaction.db");
    let output_db_path = PathBuf::from("evals/results/stage_2_pipeline.db");

    if !input_db_path.exists() {
        return Err(anyhow::anyhow!(
            "Input DB at {:?} not found. Please run eval_compaction first.",
            input_db_path
        ));
    }

    // Copy Eval 1 output database to stage_2_pipeline.db
    let _ = std::fs::copy(&input_db_path, &output_db_path)?;

    let db_path_str = output_db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid output DB path {:?}", output_db_path))?;
    let db_str = format!("file:{}", db_path_str);
    let db = Builder::new_local(&db_str).build().await?;
    let conn = db.connect()?;

    println!("[Eval 2] Running 4-stage memory pipeline on {:?}", output_db_path);

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let start_time = std::time::Instant::now();

    // Execute 4-stage pipeline (Dedup -> Embed -> Eval -> Commit)
    let processed_count = run_pipeline_cycle(&conn, &cancel_flag).await?;
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
    }

    let report_path = PathBuf::from("evals/results/eval_pipeline_results.json");
    let report = serde_json::json!({
        "processed_count": processed_count,
        "total_duration_ms": total_duration.as_millis(),
        "output_db_path": output_db_path.to_string_lossy().to_string(),
    });
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;

    println!("\n[Eval 2 Completed] Output DB ready at {:?}", output_db_path);
    Ok(())
}
