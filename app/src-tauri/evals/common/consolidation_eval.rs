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

    let prior_model = PersonalMemory::from_json(&prior_record.content).unwrap_or_default();
    let post_model = PersonalMemory::from_json(&post_record.content).unwrap_or_default();
    let post_sections_count = post_model.sections.len();
    let post_blocks_count: usize = post_model.sections.iter().map(|s| s.blocks.len()).sum();
    let post_markdown = post_model.render_to_markdown();

    // Compute block diff between prior and post models
    let mut deleted_blocks = Vec::new();
    for p_sec in &prior_model.sections {
        for p_blk in &p_sec.blocks {
            let still_exists = post_model
                .sections
                .iter()
                .any(|s| s.blocks.iter().any(|b| b.id == p_blk.id));
            if !still_exists {
                deleted_blocks.push(format!(
                    "- [Block {} | Section '{}'] {}",
                    p_blk.id, p_sec.title, p_blk.text
                ));
            }
        }
    }
    let deletions_count = deleted_blocks.len();
    let deletions_rendered = if deleted_blocks.is_empty() {
        "None (0 blocks deleted)".to_string()
    } else {
        deleted_blocks.join("\n")
    };

    let pending_revisions = fetch_pending_revisions(&conn, None)
        .await
        .unwrap_or_default();

    // 6. Build Consolidation Judge Prompt
    let mut obs_rendered = String::new();
    for (idx, obs) in candidate_observations.iter().enumerate() {
        obs_rendered.push_str(&format!(
            "- [O{} | Fact {}] {}\n",
            idx + 1,
            obs.id,
            obs.text
        ));
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
        ConsolidateOutcome::ConfirmationRequired {
            reason,
            pending_count,
        } => {
            format!(
                "Confirmation required: {:?} (pending: {})",
                reason, pending_count
            )
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
*(Note: Personal memory consolidates facts strictly scoped to the 'personal' domain. These are the active personal facts from the database.)*

<consolidation_outcome>
{} | Pending Revisions In DB: {}
</consolidation_outcome>

<memory_structural_changes>
- Prior Memory: v{} ({} sections, {} blocks)
- Resulting Memory: v{} ({} sections, {} blocks)
- Blocks Deleted from Prior Memory (N={}):
{}
</memory_structural_changes>

<resulting_personal_memory>
{}
</resulting_personal_memory>

Produce a comprehensive evaluation report in clean Markdown format with the following exact sections:

# Consolidation Evaluation Report — {}

## 1. Executive Scorecard
*(Note: Every percentage score MUST explicitly state its formula with exact counts: `X / Y = Z%`)*
- **Observation Coverage / Retention**: [X / Y = Z%] (Denominator Y = {} candidate observations. X = count of candidates demonstrably represented in resulting memory blocks. If any candidate observation is listed under 'Dropped observations' below, X MUST BE STRICTLY LESS THAN Y. Never report 100% when observations are dropped.)
- **Deletions Audited**: [N = {}] (Count of deleted blocks. Must state whether each deletion was justified by an invalidating observation or was an unjustified deletion).
- **Ungrounded Hallucinations / Extrapolations**: [None / Count with severity]
- **Taxonomy & Domain Structure Quality**: [1-10]
- **Prose Coherence & Block Fidelity**: [1-10] (Penalize heavily if ungrounded rationales are invented)
- **Delta Operation Fidelity**: [X / Y = Z%] (or N/A for cold start)
- **Temporal Consistency**: [High / Medium / Low]

## 2. Provenance & Hallucination Audit (CRITICAL)
Inspect every sentence in `<resulting_personal_memory>` against `<input_candidate_observations>`:
- **Zero Extrapolation Check**: Does the memory introduce any technical motivations, architectural rationale, or philosophical goals absent from the input facts?
  *(CRITICAL: Stating that a project 'reflects a focus on robust architecture where control over resource lifetimes is paramount' when input facts only stated 'working on Rust ownership logic' is a CRITICAL HALLUCINATION. Do NOT praise extrapolation as 'a layer of interpretation' or 'insight'. Penalize it as an ungrounded hallucination).*
- **Provenance Breakdown**: Map each emitted memory block to the specific input candidate observations that ground it. Quote any fabricated or extrapolated phrases.

## 3. Semantic Loss & Omission Audit
Inspect every input candidate observation:
- List which observations were successfully incorporated into the memory model.
- Explicitly list any **dropped observations** (observations left unrepresented in the memory).

## 4. Taxonomy & Section Emergence Assessment
- Audit the section titles (e.g. `Career`, `About Them`, `Habits & Routine`, `Interests`, `Plans`, `Health`).
- Penalize generic dump buckets (like `General`, `User Info`, `Notes`, `Miscellaneous`).
- Evaluate whether related blocks are clustered logically under umbrellas.

## 5. Prose Coherence & Block Quality
- Audit the prose quality of each block. Blocks should be coherent 1-3 sentence statements providing clear context rather than fragmented bullet scraps.
- Highlight any awkward phrasing, truncated statements, or low-information content.

## 6. Delta Operation & Preservation Audit
(If cold generation, assess structure generation; if incremental delta, assess delta operations):
- Verify whether operations appropriately selected `add` for existing umbrella sections, `new` for novel umbrellas, `update` for evolving facts, and `delete` ONLY when an observation directly invalidated an older statement.
- Deletions count: N = {}. Check whether stable existing knowledge was preserved or unnecessarily deleted/rewritten.

## 7. Contradiction & Temporal Resolution
- Check if updated facts superseded older ones cleanly, or if contradictory information is present in the final memory.

## 8. Actionable Architectural Feedback
Provide 2-3 specific improvements for consolidation prompts or schema rules.
"#,
        mode,
        prior_md_rendered,
        obs_rendered,
        outcome_details,
        pending_revisions.len(),
        prior_record.version,
        prior_model.sections.len(),
        prior_model
            .sections
            .iter()
            .map(|s| s.blocks.len())
            .sum::<usize>(),
        post_record.version,
        post_sections_count,
        post_blocks_count,
        deletions_count,
        deletions_rendered,
        post_md_rendered,
        case_id,
        candidate_observations.len(),
        deletions_count,
        deletions_count
    );

    // 7. Run Judge evaluation via NVIDIA NIM Judge
    let judge_report = judge
        .evaluate_with_trace(&judge_prompt, case_dir, "consolidation")
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
