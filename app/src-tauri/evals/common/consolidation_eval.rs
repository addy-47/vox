//! ============================================================================
//! evals/common/consolidation_eval.rs — Consolidation Stage Runner & Judge Evaluator
//! ============================================================================

use std::path::Path;

use anyhow::{anyhow, Result};
use vox_lib::{
    config::PersonalMemorySettings,
    persistence::{
        facts::fetch_active_observations_by_type,
        personal_memory::{fetch_pending_revisions, get_personal_memory},
    },
    services::memory::personal::{
        consolidate_personal_memory, ConsolidateOutcome, ConsolidationRequest, PersonalMemory,
    },
};

use super::{
    db::EvalDbGuard,
    llm_client::{NvidiaJudgeClient, RecordingLlmProvider},
    reporting::write_markdown_report,
};

/// Summary metrics resulting from a consolidation evaluation pass.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ConsolidationEvalSummary {
    pub mode: String,
    pub prior_version: i64,
    pub new_version: i64,
    pub active_observations_count: usize,
    pub sections_count: usize,
    pub blocks_count: usize,
    pub report_path: std::path::PathBuf,
}

/// Executes consolidation with auto_apply, logs raw traces, and invokes the LLM Judge.
pub async fn evaluate_consolidation_stage(
    eval_db: &EvalDbGuard,
    case_id: &str,
    provider: &RecordingLlmProvider,
    judge: &NvidiaJudgeClient,
    case_dir: &Path,
) -> Result<ConsolidationEvalSummary> {
    provider.set_context(case_id, "consolidation");
    let conn = eval_db.conn()?;

    // 1. Fetch active observations
    let candidate_observations = fetch_active_observations_by_type(&conn, "personal")
        .await
        .map_err(|e| anyhow!("Failed to fetch candidate personal observations: {}", e))?;

    // 2. Fetch current personal memory prior to consolidation
    let prior_record = get_personal_memory(&conn, None)
        .await
        .map_err(|e| anyhow!("Failed to get personal memory record: {}", e))?;

    let prior_model = PersonalMemory::from_json(&prior_record.content).unwrap_or_default();
    let prior_sections_count = prior_model.sections.len();
    let prior_markdown = prior_model.render_to_markdown();

    let mode = if prior_sections_count == 0 {
        "Cold Generation (Synthesizing new v1 personal memory)".to_string()
    } else {
        format!(
            "Incremental Delta (Integrating into existing v{} with {} sections)",
            prior_record.version, prior_sections_count
        )
    };

    // 3. Configure auto_apply memory settings
    let memory_settings = PersonalMemorySettings {
        suggestion_policy: "auto_apply".to_string(),
        ..Default::default()
    };

    // 4. Execute consolidation pass
    let request = ConsolidationRequest {
        conn: &conn,
        llm_provider: provider,
        comments: None,
        project_id: None,
        memory_settings: &memory_settings,
        llm_settings: None,
        forced: true,
    };

    let outcome: ConsolidateOutcome = consolidate_personal_memory(request)
        .await
        .map_err(|e| anyhow!("Personal memory consolidation failed: {}", e))?;

    // 5. Fetch updated personal memory record
    let post_record = get_personal_memory(&conn, None)
        .await
        .map_err(|e| anyhow!("Failed to fetch post-consolidation memory: {}", e))?;

    let post_model = PersonalMemory::from_json(&post_record.content).unwrap_or_default();
    let post_sections_count = post_model.sections.len();
    let post_blocks_count: usize = post_model.sections.iter().map(|s| s.blocks.len()).sum();
    let post_markdown = post_model.render_to_markdown();

    let pending_revisions = fetch_pending_revisions(&conn, None).await.unwrap_or_default();

    // 6. Build Consolidation Judge Prompt
    let mut obs_rendered = String::new();
    for obs in &candidate_observations {
        obs_rendered.push_str(&format!("- [{}] {}\n", obs.id, obs.text));
    }
    if obs_rendered.is_empty() {
        obs_rendered = "None (no active observations)".to_string();
    }

    let prior_md_rendered = if prior_markdown.trim().is_empty() {
        "*(Empty - initial clean state)*".to_string()
    } else {
        prior_markdown
    };

    let post_md_rendered = if post_markdown.trim().is_empty() {
        "*(Empty)*".to_string()
    } else {
        post_markdown
    };

    let outcome_details = match &outcome {
        ConsolidateOutcome::Completed { record } => {
            format!("Completed with memory version v{}", record.version)
        }
        ConsolidateOutcome::ConfirmationRequired { reason, pending_count } => {
            format!("Confirmation required: {:?} (pending: {})", reason, pending_count)
        }
    };

    let judge_prompt = format!(
        r#"You are the Vox Senior Personal Memory Consolidation Judge.
Analyze the following personal memory consolidation pass:

<consolidation_mode>
{}
</consolidation_mode>

<prior_personal_memory>
{}
</prior_personal_memory>

<input_candidate_observations>
{}
</input_candidate_observations>

<consolidation_outcome>
{} | Pending Revisions In DB: {}
</consolidation_outcome>

<resulting_personal_memory>
{}
</resulting_personal_memory>

Produce a comprehensive evaluation report in clean Markdown format with the following exact sections:

# Consolidation Evaluation Report — {}

## 1. Executive Scorecard
- **Observation Coverage / Retention**: [0-100%]
- **Taxonomy & Domain Structure Quality**: [1-10]
- **Prose Coherence & Context Density**: [1-10]
- **Delta Operation Fidelity**: [0-100%] (or N/A for cold start)
- **Temporal Consistency**: [High / Medium / Low]

## 2. Semantic Loss & Omission Audit
Inspect every input candidate observation:
- List which observations were successfully incorporated into the memory model.
- Explicitly list any **dropped observations** (observations left unrepresented in the memory).

## 3. Taxonomy & Section Emergence Assessment
- Audit the section titles (e.g. `Career & Technical Stack`, `Dietary Preferences & Health`).
- Penalize generic dump buckets (like `General`, `User Info`, `Notes`, `Miscellaneous`).
- Evaluate whether related blocks are clustered logically.

## 4. Prose Coherence & Block Quality
- Audit the prose quality of each block. Blocks should be coherent 1-3 sentence statements providing clear context rather than fragmented bullet scraps.
- Highlight any awkward phrasing, truncated statements, or low-information content.

## 5. Delta Operation & Preservation Audit
(If cold generation, assess structure generation; if incremental delta, assess delta operations):
- Verify whether operations appropriately selected `creates` for existing sections, `new_sections` for novel domains, and `updates` for evolving facts.
- Check whether stable existing knowledge was preserved or unnecessarily rewritten/deleted.

## 6. Contradiction & Temporal Resolution
- Check if updated facts superseded older ones cleanly, or if contradictory information is present in the final memory.

## 7. Actionable Architectural Feedback
Provide 2-3 specific improvements for consolidation prompts or schema rules.
"#,
        mode,
        prior_md_rendered,
        obs_rendered,
        outcome_details,
        pending_revisions.len(),
        post_md_rendered,
        case_id
    );

    // 7. Run Judge evaluation via NVIDIA NIM Judge
    let judge_report = judge
        .evaluate(&judge_prompt)
        .await
        .map_err(|e| anyhow!("Consolidation Judge evaluation failed: {}", e))?;

    // 8. Write Markdown report
    let report_path = write_markdown_report(case_dir, "consolidation.md", &judge_report)?;

    Ok(ConsolidationEvalSummary {
        mode,
        prior_version: prior_record.version,
        new_version: post_record.version,
        active_observations_count: candidate_observations.len(),
        sections_count: post_sections_count,
        blocks_count: post_blocks_count,
        report_path,
    })
}
