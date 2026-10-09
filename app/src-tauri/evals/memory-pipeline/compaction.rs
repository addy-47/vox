//! ============================================================================
//! evals/memory-pipeline/compaction.rs — Slice Planner, Runtime Pass, Compaction Judge
//! ============================================================================

use std::path::Path;

use anyhow::{anyhow, Result};
use vox_lib::{
    persistence::compactions::{commit_compaction_output, record_compaction_start},
    services::{llm::LlmSettings, memory::compaction::run_compaction},
};

use crate::common::{
    db::EvalDbGuard,
    llm_client::{JudgeClient, RecordingLlmProvider},
    slices::{dump_slices, plan_slices, slice_to_chat_messages, CompactionSlice},
    verdicts::{flatten_compaction, parse_verdict, CompactionVerdict, JudgeStatus},
};

/// Plans slice boundaries for a case and writes them to disk. Makes no LLM calls.
pub fn dump_case_slices(
    turns: &[crate::common::datasets::SessionTurn],
    case_dir: &Path,
) -> Result<usize> {
    let slices = plan_slices(turns);
    dump_slices(&slices, case_dir).map_err(|e| anyhow!("Failed to write slices: {}", e))?;
    Ok(slices.len())
}

/// Per-case telemetry surfaced for the run summary alongside judge verdicts.
#[derive(Debug, Clone, Default)]
pub struct SliceTelemetry {
    pub attribution_flags: usize,
    pub personal_cited: usize,
    pub personal_total: usize,
}

/// Runs every planned slice against the pipeline provider, persisting each into the
/// shared database, and chaining each slice's session context into the next.
///
/// Each slice's runtime document is also written to `case_dir/runtime/slice_NN.json`
/// so fact counts never depend on whether the judge parsed its verdict.
pub async fn run_slices_and_persist(
    eval_db: &EvalDbGuard,
    session_id: i64,
    case_id: &str,
    slices: &[CompactionSlice],
    provider: &RecordingLlmProvider,
    case_dir: &Path,
    llm_settings: &LlmSettings,
) -> Result<(Vec<(serde_json::Value, bool)>, SliceTelemetry)> {
    provider.set_context(case_id, "compaction");
    let conn = eval_db.conn()?;
    let runtime_dir = case_dir.join("runtime");
    std::fs::create_dir_all(&runtime_dir)
        .map_err(|e| anyhow!("Failed to create runtime dir: {}", e))?;

    let mut prior_summary: Option<String> = None;
    let mut documents = Vec::new();
    let mut attribution_flag_count = 0usize;
    let mut personal_cited = 0usize;
    let mut personal_total = 0usize;

    for slice in slices {
        let history = slice_to_chat_messages(slice, prior_summary.as_deref());

        let run_id = record_compaction_start(
            &conn,
            session_id,
            "eval",
            slice.from_turn,
            slice.to_turn,
        )
        .await
        .map_err(|e| anyhow!("Failed to record compaction start: {}", e))?;

        let result = run_compaction(provider, &history, Some(llm_settings), None).await.map_err(|e| {
            anyhow!(
                "Compaction failed for session {} slice {}: {}",
                session_id, slice.slice_index, e
            )
        })?;

        commit_compaction_output(
            &conn,
            run_id,
            &result.raw_json,
            &result.facts,
            session_id,
        )
        .await
        .map_err(|e| anyhow!("Failed to commit compaction output: {}", e))?;

        // The next slice sees this slice's summary as `<prior_summary>`, exactly as
        // production does. Each side of the baseline comparison chains its own.
        prior_summary = Some(result.session_context.clone());

        // Flag-only attribution telemetry: counted here so the eval can measure
        // whether the deterministic flags predict what the judge finds.
        attribution_flag_count += result.attribution_flags.len();
        personal_cited += result.personal_cited;
        personal_total += result.personal_total;

        // A lenient parse fallback is a real runtime outcome: production preserves
        // the raw text with zero staged facts (spec 3.75) rather than failing.
        // The eval mirrors that. The slice is recorded with empty buckets and a
        // fallback marker so the summary can distinguish "produced nothing" from
        // "judge failed to parse".
        match serde_json::from_str::<serde_json::Value>(&result.raw_json) {
            Ok(mut doc) => {
                // Judge the stored form: production strips citations before the
                // facts reach the database, so the comparison uses clean text.
                if let Some(personal) = doc.get_mut("personal").and_then(|v| v.as_array_mut()) {
                    for fact in personal.iter_mut() {
                        if let Some(text) = fact.as_str() {
                            let clean = vox_lib::services::memory::compaction::strip_citation(text);
                            *fact = serde_json::Value::String(clean);
                        }
                    }
                }
                std::fs::write(
                    runtime_dir.join(format!("slice_{:02}.json", slice.slice_index)),
                    serde_json::to_string_pretty(&doc).unwrap_or_default(),
                )
                .map_err(|e| anyhow!("Failed to write runtime doc: {}", e))?;
                documents.push((doc, false));
            }
            Err(e) => {
                log::warn!(
                    "Slice {} produced unparseable compaction output ({}); recording fallback with zero facts.",
                    slice.slice_index,
                    e
                );
                let fallback = serde_json::json!({
                    "personal": [], "objective": [], "workdone": [],
                    "blocker": [], "next_step": [], "pitfall": [],
                    "_eval_fallback": true,
                    "_eval_raw": result.raw_json,
                });
                std::fs::write(
                    runtime_dir.join(format!("slice_{:02}.json", slice.slice_index)),
                    serde_json::to_string_pretty(&fallback).unwrap_or_default(),
                )
                .map_err(|e| anyhow!("Failed to write fallback doc: {}", e))?;
                documents.push((fallback, true));
            }
        }
    }

    Ok((
        documents,
        SliceTelemetry {
            attribution_flags: attribution_flag_count,
            personal_cited,
            personal_total,
        },
    ))
}

/// Judges each slice's runtime output against the baseline for the same slice.
///
/// The judge receives two documents plus the slice turns. The turns exist for
/// one purpose only: classifying runtime facts absent from the baseline as
/// supported or ungrounded. The baseline itself is never re-judged. All counting
/// happens in [`count_compaction`], never by reading the judge's prose.
pub async fn judge_compaction(
    judge: &JudgeClient,
    case_dir: &Path,
    case_id: &str,
    slice: &CompactionSlice,
    runtime_doc: &serde_json::Value,
    baseline_doc: Option<&serde_json::Value>,
) -> Result<JudgeStatus<CompactionVerdict>> {
    let runtime_facts = flatten_compaction(runtime_doc);
    let baseline_facts = baseline_doc
        .map(flatten_compaction)
        .unwrap_or_default();

    if baseline_facts.is_empty() {
        return Ok(JudgeStatus::Invalid {
            reason: format!(
                "No baseline document for slice {}. Run generate_baseline.py, then pass its run directory with --baseline-dir.",
                slice.slice_index
            ),
        });
    }

    let prompt = build_judge_prompt(case_id, slice, &runtime_facts, &baseline_facts);
    let raw = judge
        .evaluate_with_trace(&prompt, case_dir, &format!("compaction_slice_{:02}", slice.slice_index))
        .await?;
    Ok(parse_verdict::<CompactionVerdict>(&raw))
}

fn build_judge_prompt(
    case_id: &str,
    slice: &CompactionSlice,
    runtime_facts: &[crate::common::verdicts::FlatFact],
    baseline_facts: &[crate::common::verdicts::FlatFact],
) -> String {
    let render = |facts: &[crate::common::verdicts::FlatFact]| -> String {
        if facts.is_empty() {
            return "  (none)".to_string();
        }
        facts
            .iter()
            .map(|f| format!("  [{}] ({}) {}\n", f.index, f.category, f.text))
            .collect::<String>()
    };

    let mut turns_rendered = String::new();
    for m in &slice.messages {
        if m.role == "system" {
            continue;
        }
        turns_rendered.push_str(&format!("  {}: {}\n", m.role, m.content));
    }

    format!(
        r#"You are auditing one compaction pass of the Vox memory pipeline.

<case>{}</case>
<slice>{} — turns {} through {}</slice>

Two documents are given below. Both were produced by the same model from the same
conversation, using the same extraction prompt. They differ only in how much
internal reasoning the model was allowed to spend.

<baseline_facts>
The best available extraction. Acts as the reference ceiling. Every entry is
numbered. You do NOT judge this document. It is the ruler, not the subject.
{}
</baseline_facts>

<runtime_facts>
What the pipeline actually produced under its normal operating settings.
{}
</runtime_facts>

<slice_turns>
The conversation both documents were extracted from. Use it for ONE purpose only:
deciding whether a runtime fact absent from the baseline is supported by what
somebody actually said.
{}
</slice_turns>

Your task is to classify every runtime fact, then report what the runtime missed.

Classify each runtime fact into exactly one category:

  "matched_baseline"   - The baseline contains a fact with the same meaning.
                         Set "baseline_index" to that baseline fact's number.
                         You need nothing else for this verdict.
  "novel_but_valid"    - The baseline does not contain it, BUT <slice_turns> shows
                         a speaker establishing it. Quote the supporting turn in
                         "reason". Leave "baseline_index" null.
  "ungrounded"         - The baseline does not contain it AND <slice_turns> does not
                         support it: invented, extrapolated, or an attribute no
                         speaker established. Quote the closest turn and explain the
                         gap in "reason". Leave "baseline_index" null.

Attribution rules, applied exactly as the extraction prompt states them:
- Only the USER's explicit statements establish facts. Assistant suggestions,
  recommendations, examples, and hypothetical answers are not user facts.
- A question, topic, or location the user mentions does not establish residence,
  identity, ownership, preference, or habit.
- Do not convert an assistant claim into a completed action unless the dialogue
  records it happening.

Also list every baseline fact the runtime did not produce, by number.

Return ONLY this JSON object, with no text before or after it:

{{
  "runtime_facts": [
    {{"index": 1, "verdict": "matched_baseline", "baseline_index": 2, "reason": "why"}},
    {{"index": 2, "verdict": "novel_but_valid", "baseline_index": null, "reason": "why, quoting turn N"}},
    {{"index": 3, "verdict": "ungrounded", "baseline_index": null, "reason": "why, quoting turn N"}}
  ],
  "baseline_missed": [
    {{"index": 7, "reason": "why the runtime omitted it"}}
  ],
  "summary": "one paragraph"
}}

Rules:
- "index" in runtime_facts must be every number from 1 to {}, each exactly once.
- "index" in baseline_missed must refer only to baseline fact numbers, never to runtime numbers.
- Use null for "baseline_index" whenever the verdict is not "matched_baseline".
- Never invent a "No mention" claim. Every novel/ungrounded verdict must quote a
  turn number from <slice_turns>.
"#,
        case_id,
        slice.slice_index,
        slice.from_turn,
        slice.to_turn,
        render(baseline_facts),
        render(runtime_facts),
        turns_rendered,
        runtime_facts.len(),
    )
}