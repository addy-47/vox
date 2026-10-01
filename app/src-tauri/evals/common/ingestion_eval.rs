//! ============================================================================
//! evals/common/ingestion_eval.rs — Ingestion Stage Runner & Judge Evaluator
//! ============================================================================

use std::path::Path;

use anyhow::{anyhow, Result};
use vox_lib::{
    persistence::facts::fetch_all_observations,
    services::memory::ingestion::{run_ingestion_cycle, IngestionCycleSummary},
};

use super::{db::EvalDbGuard, llm_client::NvidiaJudgeClient, reporting::write_markdown_report};

/// Summary metrics resulting from an ingestion cycle evaluation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IngestionEvalSummary {
    pub stage1_processed: usize,
    pub stage2_processed: usize,
    pub stage2_inserted: usize,
    pub total_active_observations: usize,
    pub report_path: std::path::PathBuf,
}

#[derive(Debug, Clone)]
struct QueueSnapshotItem {
    id: i64,
    fact_type: String,
    text: String,
    status: String,
}

/// Executes the ingestion deduplication cycle, records telemetry, and runs the LLM Judge.
pub async fn evaluate_ingestion_stage(
    eval_db: &EvalDbGuard,
    case_id: &str,
    judge: &NvidiaJudgeClient,
    case_dir: &Path,
) -> Result<IngestionEvalSummary> {
    let conn = eval_db.conn()?;

    // 1. Snapshot pending queue items before ingestion
    let mut queue_stmt = conn
        .query(
            "SELECT id, type, text, status FROM memory_ingestion_queue WHERE status = 'pending' ORDER BY id ASC",
            (),
        )
        .await
        .map_err(|e| anyhow!("Failed to query pending queue items: {}", e))?;

    let mut pending_items = Vec::new();
    while let Some(row) = queue_stmt.next().await? {
        pending_items.push(QueueSnapshotItem {
            id: row.get(0)?,
            fact_type: row.get(1)?,
            text: row.get(2)?,
            status: row.get(3)?,
        });
    }

    // 2. Snapshot existing active observations before ingestion across all types
    let existing_active = fetch_all_observations(&conn, None, Some("active"), None, None, None)
        .await
        .unwrap_or_default();

    // 3. Execute production ingestion cycle (Stage 1 Jaccard + Stage 2 ONNX Cosine Dedup)
    let cycle_summary: IngestionCycleSummary = run_ingestion_cycle(&conn)
        .await
        .map_err(|e| anyhow!("Failed to run ingestion cycle: {}", e))?;

    // 4. Snapshot active observations after ingestion across all types
    let after_active = fetch_all_observations(&conn, None, Some("active"), None, None, None)
        .await
        .unwrap_or_default();

    // 5. Query updated queue items
    let mut post_queue_stmt = conn
        .query(
            "SELECT id, type, text, status FROM memory_ingestion_queue ORDER BY id ASC",
            (),
        )
        .await
        .map_err(|e| anyhow!("Failed to query post-ingestion queue items: {}", e))?;

    let mut post_queue_items = Vec::new();
    while let Some(row) = post_queue_stmt.next().await? {
        post_queue_items.push(QueueSnapshotItem {
            id: row.get(0)?,
            fact_type: row.get(1)?,
            text: row.get(2)?,
            status: row.get(3)?,
        });
    }

    // 6. Build Ingestion Judge Prompt
    let mut pending_rendered = String::new();
    for item in &pending_items {
        pending_rendered.push_str(&format!(
            "- [ID {} | {}] {}\n",
            item.id, item.fact_type, item.text
        ));
    }
    if pending_rendered.is_empty() {
        pending_rendered = "None (no pending facts in queue)".to_string();
    }

    let mut existing_rendered = String::new();
    for obs in &existing_active {
        existing_rendered.push_str(&format!(
            "- [ID {} | Type: {}] {}\n",
            obs.id, obs.observation_type, obs.text
        ));
    }
    if existing_rendered.is_empty() {
        existing_rendered = "None (database was clean before this cycle)".to_string();
    }

    let mut post_obs_rendered = String::new();
    for obs in &after_active {
        post_obs_rendered.push_str(&format!(
            "- [ID {} | Type: {}] {}\n",
            obs.id, obs.observation_type, obs.text
        ));
    }
    if post_obs_rendered.is_empty() {
        post_obs_rendered = "None (no active observations)".to_string();
    }

    let mut queue_decisions_rendered = String::new();
    for item in &post_queue_items {
        queue_decisions_rendered.push_str(&format!(
            "- [ID {} | Status: {}] {}\n",
            item.id, item.status, item.text
        ));
    }

    let completed_count = post_queue_items
        .iter()
        .filter(|i| i.status == "completed")
        .count();
    let pending_count = post_queue_items
        .iter()
        .filter(|i| i.status == "pending")
        .count();
    let stage1_done_count = post_queue_items
        .iter()
        .filter(|i| i.status == "stage1_done")
        .count();
    let failed_count = post_queue_items
        .iter()
        .filter(|i| i.status == "failed")
        .count();

    let judge_prompt = format!(
        r#"You are the Vox Senior Memory Ingestion & Deduplication Judge.
Analyze the following memory ingestion cycle, which uses:
- Stage 1: Exact token Jaccard similarity (Threshold = 1.0)
- Stage 2: Dense ONNX MiniLM vector embedding cosine similarity (Threshold = 0.95)

<architecture_invariant>
CRITICAL: In Vox, Stage 2 vector deduplication is strictly partitioned by observation category type (`WHERE status = 'active' AND type = ?`).
Candidate observations are compared ONLY against existing observations of the EXACT SAME TYPE (e.g. personal vs personal, workdone vs workdone).
Cross-type conceptual overlap (e.g. between an `objective` task and a `workdone` completion, or between a `personal` preference and a `workdone` action) is INTENTIONAL by design — they are stored as separate category nodes in the memory graph and are NEVER merged, deactivated, or suppressed by Stage 2. Do NOT treat cross-type conceptual overlap as duplicate pollution or deduplication failure.
</architecture_invariant>

<pre_existing_active_observations>
{}
</pre_existing_active_observations>

<incoming_queue_facts>
{}
</incoming_queue_facts>

<post_cycle_queue_status>
{}
</post_cycle_queue_status>

<post_cycle_active_observations>
{}
</post_cycle_active_observations>

Cycle Telemetry:
- Stage 1 Processed: {} | Errors: {}
- Stage 2 Processed: {} | Inserted: {} | Duplicates Deactivated: {} | Errors: {}
- Queue Breakdown: Total Ingested Items={}, Completed={}, Remaining Pending={}, Stranded Stage1 Done={}, Failed={}
- Total Active Observations In Database: {}

Produce a comprehensive evaluation report in clean Markdown format with the following exact sections:

# Ingestion Evaluation Report — {}

## 1. Executive Scorecard
*(Note: Every percentage score MUST explicitly state its formula with exact counts: `X / Y = Z%`. If 0 candidates were evaluated or merged, report 'N/A (0 candidates)' rather than a vacuous 100% or 0%.)*
- **Deduplication Precision**: [X / Y = Z% or N/A (0 merged)] (Did merged/deactivated facts truly represent duplicates?)
- **Deduplication Recall**: [X / Y = Z% or N/A (0 same-type duplicates exist)] (Were all genuine same-type duplicates caught?)
- **False Merge Rate**: [X / Y = Z%] (Distinct same-type facts incorrectly suppressed as duplicates)
- **Duplicate Pollution Rate**: [X / Y = Z%] (Duplicate same-type facts erroneously inserted as novel observations)
- **Queue Pipeline Integrity**: [Pass / Fail]

## 2. Stage 1 Exact Match Audit
Evaluate lexical/token matching:
- Were any facts inappropriately marked as exact duplicates?

## 3. Stage 2 Semantic Vector Deduplication Audit
Analyze same-type facts marked as duplicates vs. inserted:
- **False Merges Audit**: List any distinct same-type facts that were improperly merged or suppressed because of semantic proximity.
- **Duplicate Pollution Audit**: List any incoming facts that were inserted as novel but were actually synonymous with pre-existing same-type observations.

## 4. Near-Miss Region & Boundary Analysis
Inspect borderline same-type candidate pairs (near the 0.85-0.95 similarity cutoff):
- Identify where the cosine threshold succeeded or struggled to separate subtle nuances.

## 5. Contradiction & Evolution Handling
- Were evolving facts (e.g. status changes, location changes, preference shifts) handled appropriately, or do contradictory observations now co-exist?

## 6. Final Verdict & Threshold Calibration
Provide concise feedback on whether the 0.95 cosine threshold is optimal or requires calibration based on same-type candidate observations.
"#,
        existing_rendered,
        pending_rendered,
        queue_decisions_rendered,
        post_obs_rendered,
        cycle_summary.stage1.processed,
        cycle_summary.stage1.errors,
        cycle_summary.stage2.processed,
        cycle_summary.stage2.inserted,
        cycle_summary.stage2.duplicates_deactivated,
        cycle_summary.stage2.errors,
        post_queue_items.len(),
        completed_count,
        pending_count,
        stage1_done_count,
        failed_count,
        after_active.len(),
        case_id
    );

    // 7. Run Judge evaluation via NVIDIA NIM Judge
    let judge_report = judge
        .evaluate(&judge_prompt)
        .await
        .map_err(|e| anyhow!("Ingestion Judge evaluation failed: {}", e))?;

    // 8. Write Markdown report
    let report_path = write_markdown_report(case_dir, "ingestion.md", &judge_report)?;

    Ok(IngestionEvalSummary {
        stage1_processed: cycle_summary.stage1.processed,
        stage2_processed: cycle_summary.stage2.processed,
        stage2_inserted: cycle_summary.stage2.inserted,
        total_active_observations: after_active.len(),
        report_path,
    })
}
