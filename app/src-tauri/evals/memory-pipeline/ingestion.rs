//! ============================================================================
//! evals/memory-pipeline/ingestion.rs — Ingestion Cycle Runner & Ingestion Judge
//! ============================================================================

use std::path::Path;

use anyhow::{anyhow, Result};
use vox_lib::services::memory::ingestion::{
    run_ingestion_cycle, DedupDecision, DedupNearMiss, IngestionCycleSummary,
};

use crate::common::{
    db::EvalDbGuard,
    llm_client::JudgeClient,
    verdicts::{count_ingestion, parse_verdict, IngestionCounts, IngestionVerdict, JudgeStatus},
};

/// Per-case ingestion outcome.
#[derive(Debug, Clone)]
pub struct IngestionEvalSummary {
    pub stage1_processed: usize,
    pub stage2_processed: usize,
    pub stage2_inserted: usize,
    pub stage1_merges: usize,
    pub stage2_merges: usize,
    pub judge_parsed: bool,
    pub counts: IngestionCounts,
}

/// Runs the ingestion cycle and judges the deduplication decisions it made.
///
/// The judge receives every merge and every near-miss with the similarity that
/// produced it, so it can judge both directions: whether what was merged should
/// have been, and whether what was left alone should have been merged.
pub async fn evaluate_ingestion_stage(
    eval_db: &EvalDbGuard,
    case_id: &str,
    judge: &JudgeClient,
    case_dir: &Path,
) -> Result<IngestionEvalSummary> {
    let conn = eval_db.conn()?;

    let cycle: IngestionCycleSummary = run_ingestion_cycle(&conn)
        .await
        .map_err(|e| anyhow!("Failed to run ingestion cycle: {}", e))?;

    let stage1_merges = cycle.stage1.decisions.len();
    let stage2_merges = cycle.stage2.decisions.len();

    let decisions: Vec<DedupDecision> = cycle
        .stage1
        .decisions
        .iter()
        .chain(cycle.stage2.decisions.iter())
        .cloned()
        .collect();
    let all_near_misses: Vec<DedupNearMiss> = cycle
        .stage1
        .near_misses
        .iter()
        .chain(cycle.stage2.near_misses.iter())
        .cloned()
        .collect();

    let verdict: JudgeStatus<IngestionVerdict> = if decisions.is_empty() && all_near_misses.is_empty()
    {
        JudgeStatus::Invalid {
            reason: "Ingestion cycle produced no merges and no near-misses to audit. \
                     An empty case cannot confirm deduplication accuracy."
                .to_string(),
        }
    } else {
        let prompt = build_judge_prompt(case_id, &decisions, &all_near_misses);
        let raw = judge
            .evaluate_with_trace(&prompt, case_dir, "ingestion")
            .await?;
        parse_verdict::<IngestionVerdict>(&raw)
    };

    let counts = match &verdict {
        JudgeStatus::Parsed(v) => count_ingestion(v),
        JudgeStatus::Invalid { .. } => IngestionCounts::default(),
    };

    // Persist the verdict so the QA pass can verify the counts against the trace.
    let verdict_path = case_dir.join("ingestion_verdict.json");
    let payload = match &verdict {
        JudgeStatus::Parsed(v) => serde_json::json!({ "status": "parsed", "verdict": v, "counts": counts }),
        JudgeStatus::Invalid { reason } => {
            serde_json::json!({ "status": "invalid", "reason": reason })
        }
    };
    std::fs::write(&verdict_path, serde_json::to_string_pretty(&payload)?)
        .map_err(|e| anyhow!("Failed to write ingestion verdict: {}", e))?;

    Ok(IngestionEvalSummary {
        stage1_processed: cycle.stage1.processed,
        stage2_processed: cycle.stage2.processed,
        stage2_inserted: cycle.stage2.inserted,
        stage1_merges,
        stage2_merges,
        judge_parsed: !matches!(verdict, JudgeStatus::Invalid { .. }),
        counts,
    })
}

fn build_judge_prompt(
    case_id: &str,
    decisions: &[DedupDecision],
    near_misses: &[DedupNearMiss],
) -> String {
    let stage_name = |s: vox_lib::services::memory::ingestion::DedupStage| match s {
        vox_lib::services::memory::ingestion::DedupStage::Stage1Exact => {
            "STAGE 1 (exact token Jaccard, threshold 1.0)"
        }
        vox_lib::services::memory::ingestion::DedupStage::Stage2Cosine => {
            "STAGE 2 (MiniLM embedding cosine, threshold 0.95)"
        }
    };

    let mut decision_lines = String::new();
    for d in decisions {
        decision_lines.push_str(&format!(
            "  queue_id={}  [{}]\n    INCOMING : {}\n    DEACTIVATED: {}\n    similarity={:.4}\n",
            d.incoming_queue_id,
            stage_name(d.stage),
            d.incoming_text,
            d.deactivated_text,
            d.similarity
        ));
    }
    if decision_lines.is_empty() {
        decision_lines.push_str("  (no merges occurred)\n");
    }

    let mut near_lines = String::new();
    for n in near_misses {
        near_lines.push_str(&format!(
            "  queue_id={}  [{}]\n    INCOMING : {}\n    KEPT SEPARATE: {}\n    similarity={:.4}\n",
            n.incoming_queue_id,
            stage_name(n.stage),
            n.incoming_text,
            n.active_text,
            n.similarity
        ));
    }
    if near_lines.is_empty() {
        near_lines.push_str("  (no near-miss pairs in the reported band)\n");
    }

    format!(
        r#"You are auditing the deduplication decisions of the Vox memory ingestion pipeline.

<case>{}</case>

<merges_that_occurred>
Each entry is a pair the pipeline collapsed into one stored fact. The incoming
fact won; the older one was deactivated. Judge whether collapsing them was right.
{}
</merges_that_occurred>

<near_miss_pairs_not_merged>
Each entry is a pair that scored at or above 0.70 but below the merge threshold,
so they were deliberately kept as two separate stored facts. Judge whether that
was right.
{}
</near_miss_pairs_not_merged>

For every entry in <merges_that_occurred>, return:
  "correct"    - these two say the same thing, so one stored fact is right.
  "incorrect"  - these are genuinely different facts and one of them is now lost.

For every entry in <near_miss_pairs_not_merged>, return:
  "should_have_merged" - these say the same thing and should be one fact.
  "distinct"           - these are different and staying separate is right.

Return ONLY this JSON object, with no text before or after it:

{{
  "decisions": [
    {{"queue_id": 17, "verdict": "correct", "reason": "why"}}
  ],
  "near_misses": [
    {{"queue_id": 28, "verdict": "distinct", "reason": "why"}}
  ],
  "summary": "one paragraph"
}}

Rules:
- Every queue_id from <merges_that_occurred> must appear exactly once in "decisions".
- Every queue_id from <near_miss_pairs_not_merged> must appear exactly once in "near_misses".
- A pair that differs only in a swapped parameter (allergic to shellfish vs
  allergic to dairy; uses Pop!_OS vs uses Ubuntu) is "distinct", not a duplicate.
- Judge each pair on its own text. Do not reward or penalise the similarity value
  itself; it is provided as context, not as the answer.
"#,
        case_id, decision_lines, near_lines
    )
}

