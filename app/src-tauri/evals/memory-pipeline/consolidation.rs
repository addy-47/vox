//! ============================================================================
//! evals/memory-pipeline/consolidation.rs — Consolidation Pass & Consolidation Judge
//! ============================================================================

use std::path::Path;

use anyhow::{anyhow, Result};
use vox_lib::{
    config::settings::PersonalMemorySettings,
    persistence::{
        facts::{fetch_active_observations_by_type, ObservationRecord},
        personal_memory::get_personal_memory,
    },
    services::memory::personal::{ConsolidationRequest, ConsolidationTelemetry, PersonalMemory},
};

use crate::common::{
    db::EvalDbGuard,
    llm_client::{JudgeClient, RecordingLlmProvider},
    verdicts::{
        count_consolidation, parse_verdict, ConsolidationCounts, ConsolidationVerdict, JudgeStatus,
    },
};

/// Per-case consolidation outcome.
#[derive(Debug, Clone)]
pub struct ConsolidationEvalSummary {
    pub mode: String,
    pub prior_version: i64,
    pub new_version: i64,
    pub sections_count: usize,
    pub blocks_count: usize,
    pub judge_parsed: bool,
    pub counts: ConsolidationCounts,
}

/// A block as the judge sees it: persistent ID plus text, so a deletion can be
/// joined back to the resulting state.
#[derive(Debug, Clone)]
struct JudgeBlock {
    block_id: String,
    section_title: String,
    text: String,
}

fn collect_blocks(memory: &PersonalMemory) -> Vec<JudgeBlock> {
    memory
        .sections
        .iter()
        .flat_map(|s| {
            s.blocks.iter().map(move |b| JudgeBlock {
                block_id: b.id.clone(),
                section_title: s.title.clone(),
                text: b.text.clone(),
            })
        })
        .collect()
}

fn render_blocks(blocks: &[JudgeBlock]) -> String {
    if blocks.is_empty() {
        return "  (memory is empty)".to_string();
    }
    blocks
        .iter()
        .map(|b| format!("  {}  [{}]  {}\n", b.block_id, b.section_title, b.text))
        .collect()
}

fn render_observations(observations: &[ObservationRecord]) -> String {
    if observations.is_empty() {
        return "  (no active personal observations)".to_string();
    }
    observations
        .iter()
        .map(|o| format!("  {}  {}\n", o.id, o.text))
        .collect()
}

/// Runs one consolidation pass and judges the operations it produced.
///
/// The judge receives the observations it was given, the handle view the model
/// saw, the operations it emitted, the operations as resolved onto persistent
/// IDs together with every refusal and its reason, and the resulting memory with
/// its block IDs. That is enough to audit a `delete` end to end, which the
/// previous harness could not do.
pub async fn evaluate_consolidation_stage(
    eval_db: &EvalDbGuard,
    case_id: &str,
    provider: &RecordingLlmProvider,
    judge: &JudgeClient,
    case_dir: &Path,
    llm_settings: &vox_lib::services::llm::LlmSettings,
) -> Result<ConsolidationEvalSummary> {
    provider.set_context(case_id, "consolidation");
    let conn = eval_db.conn()?;

    let candidates = fetch_active_observations_by_type(&conn, "personal")
        .await
        .map_err(|e| anyhow!("Failed to fetch candidate personal observations: {}", e))?;

    let prior_record = get_personal_memory(&conn, None)
        .await
        .map_err(|e| anyhow!("Failed to get personal memory record: {}", e))?;
    let prior_model = PersonalMemory::from_json(&prior_record.content).unwrap_or_default();
    let prior_blocks = collect_blocks(&prior_model);
    let prior_sections: Vec<String> = prior_model.sections.iter().map(|s| s.id.clone()).collect();

    let mode = if prior_sections.is_empty() {
        "cold generation".to_string()
    } else {
        "incremental delta".to_string()
    };

    // The eval runs auto_apply so a harmful delete actually reaches the committed
    // state and can be measured. Production defaults to manual_review, which
    // stages the same delete instead. Recorded in the run manifest.
    let memory_settings = PersonalMemorySettings {
        suggestion_policy: "auto_apply".to_string(),
        ..Default::default()
    };

    let request = ConsolidationRequest {
        conn: &conn,
        llm_provider: provider,
        comments: None,
        project_id: None,
        memory_settings: &memory_settings,
        llm_settings: Some(llm_settings),
        forced: true,
    };

    let (outcome, telemetry) =
        vox_lib::services::memory::personal::consolidate_personal_memory_with_telemetry(request)
            .await
            .map_err(|e| anyhow!("Personal memory consolidation failed: {}", e))?;

    let post_record = get_personal_memory(&conn, None)
        .await
        .map_err(|e| anyhow!("Failed to fetch post-consolidation memory: {}", e))?;
    let post_model = PersonalMemory::from_json(&post_record.content).unwrap_or_default();
    let post_blocks = collect_blocks(&post_model);
    let post_sections: Vec<String> = post_model.sections.iter().map(|s| s.id.clone()).collect();

    // Section net loss is computed here, from the ID sets. The judge is not
    // consulted: a section legitimately disappears when its last block is deleted
    // and a replacement is created, so only net loss is meaningful.
    let lost_sections: Vec<&String> = prior_sections
        .iter()
        .filter(|id| !post_sections.contains(id))
        .collect();

    let is_cold_generation = telemetry.pass_name == "cold generation";

    // Which blocks this pass created or rewrote. Grounding is scoped to these:
    // a block inherited untouched from a prior pass is reported separately, never
    // charged to this pass as a fresh hallucination.
    let prior_ids: std::collections::HashSet<&str> =
        prior_blocks.iter().map(|b| b.block_id.as_str()).collect();
    let touched_ids: std::collections::HashSet<String> = {
        let mut set: std::collections::HashSet<String> = post_blocks
            .iter()
            .filter(|b| !prior_ids.contains(b.block_id.as_str()))
            .map(|b| b.block_id.clone())
            .collect();
        for op in &telemetry.resolved_ops {
            match op {
                vox_lib::services::memory::personal::ResolvedOp::UpdateBlock {
                    block_id, ..
                } => {
                    set.insert(block_id.clone());
                }
                vox_lib::services::memory::personal::ResolvedOp::DeleteBlock {
                    block_id, ..
                } => {
                    set.insert(block_id.clone());
                }
                _ => {}
            }
        }
        set
    };

    let verdict: JudgeStatus<ConsolidationVerdict> = if candidates.is_empty() {
        JudgeStatus::Invalid {
            reason: "No active personal observations were supplied, so the pass had \
                     nothing to integrate. An empty input proves nothing about \
                     consolidation accuracy."
                .to_string(),
        }
    } else if is_cold_generation {
        let prompt = build_cold_gen_prompt(case_id, &candidates, &post_blocks);
        let raw = judge
            .evaluate_with_trace(&prompt, case_dir, "consolidation")
            .await?;
        parse_verdict::<ConsolidationVerdict>(&raw)
    } else if telemetry.handle_view.is_none() && telemetry.model_output.is_none() {
        JudgeStatus::Invalid {
            reason: format!(
                "Incremental pass produced no operations to audit (outcome: {:?}).",
                outcome
            ),
        }
    } else {
        let prompt = build_judge_prompt(
            case_id,
            &mode,
            &candidates,
            &prior_blocks,
            &telemetry,
            &post_blocks,
            &touched_ids,
        );
        let raw = judge
            .evaluate_with_trace(&prompt, case_dir, "consolidation")
            .await?;
        parse_verdict::<ConsolidationVerdict>(&raw)
    };

    let counts = match &verdict {
        JudgeStatus::Parsed(v) => count_consolidation(v),
        JudgeStatus::Invalid { .. } => ConsolidationCounts::default(),
    };

    let payload = serde_json::json!({
        "status": match &verdict {
            JudgeStatus::Parsed(_) => "parsed",
            JudgeStatus::Invalid { .. } => "invalid",
        },
        "mode": mode,
        "suggestion_policy": "auto_apply",
        "suggestion_policy_is_production_default": false,
        "prior_version": prior_record.version,
        "new_version": post_record.version,
        "prior_section_ids": prior_sections,
        "post_section_ids": post_sections,
        "sections_lost": lost_sections,
        "telemetry": telemetry,
        "verdict": match &verdict {
            JudgeStatus::Parsed(v) => serde_json::to_value(v)?,
            JudgeStatus::Invalid { reason } => serde_json::json!({ "reason": reason }),
        },
        "counts": counts,
    });
    std::fs::write(
        case_dir.join("consolidation_verdict.json"),
        serde_json::to_string_pretty(&payload)?,
    )
    .map_err(|e| anyhow!("Failed to write consolidation verdict: {}", e))?;

    Ok(ConsolidationEvalSummary {
        mode,
        prior_version: prior_record.version,
        new_version: post_record.version,
        sections_count: post_model.sections.len(),
        blocks_count: post_blocks.len(),
        judge_parsed: !verdict.is_invalid(),
        counts,
    })
}

fn build_cold_gen_prompt(
    case_id: &str,
    observations: &[ObservationRecord],
    post_blocks: &[JudgeBlock],
) -> String {
    format!(
        r#"You are auditing a cold-generation pass of the Vox personal-memory pipeline.
The pass received a numbered list of observations and produced a complete memory
from nothing. There are no prior blocks and no edit operations to review.

<case>{}</case>

<observations_supplied>
Every observation the pass was given. Each must end up represented in the
resulting memory, exactly once.
{}
</observations_supplied>

<memory_produced>
The committed result. Every block carries its ID.
{}
</memory_produced>

Produce two things.

1. COVERAGE. For every observation in <observations_supplied>, decide whether the
   resulting memory represents it. Use "represented": true only when you can point
   at a block in <memory_produced> that carries that observation's content.
   Observations saying the same thing should share one block; none may be dropped.

2. GROUNDING. List any phrase in <memory_produced> that no observation supports:
   invented attributes, inferred relationships, proximity claims ("near", "close
   to"), or completion status nobody established. Stating more than the
   observations is worse than stating less.

Return ONLY this JSON object, with no text before or after it:

{{
  "observations": [
    {{"obs_id": "...", "represented": true, "block_id": "...", "reason": "why"}}
  ],
  "deletes": [],
  "updates": [],
  "hallucinations": [
    {{"block_id": "...", "phrase": "invented text", "ungrounded": true, "reason": "why"}}
  ],
  "summary": "one paragraph"
}}

Rules:
- "observations" must contain one entry for every observation supplied, using its
  exact id. "block_id" is null when represented is false.
- "deletes" and "updates" stay empty: cold generation emits no operations.
"#,
        case_id,
        render_observations(observations),
        render_blocks(post_blocks),
    )
}

fn build_judge_prompt(
    case_id: &str,
    mode: &str,
    observations: &[ObservationRecord],
    prior_blocks: &[JudgeBlock],
    telemetry: &ConsolidationTelemetry,
    post_blocks: &[JudgeBlock],
    touched_ids: &std::collections::HashSet<String>,
) -> String {
    let ops = telemetry
        .model_output
        .as_ref()
        .map(|o| serde_json::to_string_pretty(o).unwrap_or_default())
        .unwrap_or_else(|| "(no operations emitted)".to_string());

    let resolved = serde_json::to_string_pretty(&telemetry.resolved_ops).unwrap_or_default();
    let resolve_rejections =
        serde_json::to_string_pretty(&telemetry.resolve_rejections).unwrap_or_default();
    let apply_rejections =
        serde_json::to_string_pretty(&telemetry.apply_rejections).unwrap_or_default();

    let touched_list = if touched_ids.is_empty() {
        "(no block was created or rewritten by this pass)".to_string()
    } else {
        let mut ids: Vec<&String> = touched_ids.iter().collect();
        ids.sort();
        ids.iter().map(|id| format!("  {}\n", id)).collect()
    };

    format!(
        r#"You are auditing one personal-memory consolidation pass of the Vox pipeline.

<case>{}</case>
<mode>{}</mode>
<suggestion_policy>auto_apply (deletions were committed, not staged)</suggestion_policy>

<observations_supplied>
Every observation the pass was given. Each must end up represented in the
resulting memory.
{}
</observations_supplied>

<memory_before>
The memory the model was shown, as handle-labelled blocks.
{}
</memory_before>

<memory_view_shown_to_model>
{}
</memory_view_shown_to_model>

<model_operations_emitted>
Exactly what the model returned, before handle resolution.
{}
</model_operations_emitted>

<operations_resolved>
The same operations mapped onto persistent IDs.
{}
</operations_resolved>

<operations_refused_at_resolution>
{}
</operations_refused_at_resolution>

<operations_refused_at_apply>
{}
</operations_refused_at_apply>

<memory_after>
The committed result. Every block carries its ID.
{}
</memory_after>

<blocks_this_pass_created_or_rewrote>
Only these blocks may be charged to this pass for grounding. Every other block
is inherited untouched: report it under inherited_concerns, never as a fresh
hallucination of this pass.
{}
</blocks_this_pass_created_or_rewrote>

Produce four things.

1. COVERAGE. For every observation in <observations_supplied>, decide whether
   <memory_after> represents it. "Represented" means the content is present in
   <memory_after>, full stop. It does NOT matter which pass created the block,
   whether this pass touched it, or whether it already existed: presence is
   presence. Set "represented": true whenever you can point at a block in
   <memory_after> that carries the observation's content, and name that block.

2. DELETIONS. For every `delete` operation, decide whether it was justified. A
   delete is justified ONLY when an observation states the deleted block is no
   longer true. Anything else is unjustified: unrelated subject, better expressed as
   an update, or no invalidating observation at all. Set "invalidating_obs" to the
   observation id only when one genuinely invalidates it, otherwise null.

3. UPDATES. For every `update`, decide whether the replacement kept the subject
   of the block it replaced.

4. GROUNDING, scoped to this pass. List phrases in blocks from
   <blocks_this_pass_created_or_rewrote> that no observation supports: invented
   attributes, inferred relationships, proximity claims, or completion status
   nobody established. Then, separately, list any inherited block (not in that
   list) whose content no supplied observation supports, under
   "inherited_concerns". Inherited concerns are reported, never counted as this
   pass's hallucinations.

Return ONLY this JSON object, with no text before or after it:

{{
  "observations": [
    {{"obs_id": "...", "represented": true, "block_id": "...", "reason": "why"}}
  ],
  "deletes": [
    {{"block_id": "...", "justified": false, "invalidating_obs": null, "reason": "why"}}
  ],
  "updates": [
    {{"block_id": "...", "kept_subject": true, "reason": "why"}}
  ],
  "hallucinations": [
    {{"block_id": "...", "phrase": "invented text", "ungrounded": true, "reason": "why"}}
  ],
  "inherited_concerns": [
    {{"block_id": "...", "phrase": "inherited text", "reason": "why this predates the pass"}}
  ],
  "summary": "one paragraph"
}}

Rules:
- "observations" must contain one entry for every observation supplied, using its
  exact id. "block_id" is null when represented is false.
- "deletes" must contain one entry for every delete operation resolved. Use the
  persistent block ID from <operations_resolved>, not the handle.
- Blocks created by a `new` operation have no prior text and need no delete entry.
- Do not reward invented detail. A block that says more than its observation is
  worse than a block that says less.
"#,
        case_id,
        mode,
        render_observations(observations),
        render_blocks(prior_blocks),
        telemetry.handle_view.as_deref().unwrap_or("(not shown)"),
        ops,
        resolved,
        resolve_rejections,
        apply_rejections,
        render_blocks(post_blocks),
        touched_list,
    )
}
