//! ============================================================================
//! evals/common/compaction_eval.rs — Compaction Stage Runner & Judge Evaluator
//! ============================================================================

use std::path::Path;

use anyhow::{anyhow, Result};
use vox_lib::{
    persistence::compactions::{commit_compaction_output, record_compaction_start},
    services::{
        harness::{ChatMessage, Role},
        memory::compaction::run_compaction,
    },
};

use super::{
    datasets::SessionTurn,
    db::EvalDbGuard,
    llm_client::{NvidiaJudgeClient, RecordingLlmProvider},
    reporting::write_markdown_report,
};

/// Summary metrics resulting from a single session compaction evaluation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CompactionEvalSummary {
    pub session_id: i64,
    pub facts_extracted: usize,
    pub session_context_len: usize,
    pub report_path: std::path::PathBuf,
}

/// Runs compaction and commits output to the database without invoking the LLM Judge.
pub async fn run_compaction_and_persist(
    eval_db: &EvalDbGuard,
    session_id: i64,
    case_id: &str,
    turns: &[SessionTurn],
    provider: &RecordingLlmProvider,
) -> Result<vox_lib::services::memory::compaction::CompactionResult> {
    provider.set_context(case_id, "compaction");
    let conn = eval_db.conn()?;

    // 1. Seed conversation turns
    eval_db
        .seed_turns(session_id, &format!("Eval {}", case_id), turns, 0)
        .await?;

    let from_turn_id = turns.first().map(|t| t.turn).unwrap_or(1);
    let to_turn_id = turns.last().map(|t| t.turn).unwrap_or(1);

    // 2. Build conversation history messages
    let mut history_messages = Vec::new();
    for t in turns {
        history_messages.push(ChatMessage::new(Role::User, t.user.clone()));
        history_messages.push(ChatMessage::new(Role::Assistant, t.assistant.clone()));
    }

    // 3. Record compaction start
    let run_id = record_compaction_start(&conn, session_id, "eval", from_turn_id, to_turn_id)
        .await
        .map_err(|e| anyhow!("Failed to record compaction start: {}", e))?;

    // 4. Run compaction via recording LLM provider
    let compaction_res = run_compaction(provider, &history_messages, None, None)
        .await
        .map_err(|e| anyhow!("Compaction run failed for session {}: {}", session_id, e))?;

    // 5. Commit compaction output and enqueue facts into memory_ingestion_queue
    commit_compaction_output(
        &conn,
        run_id,
        &compaction_res.raw_json,
        &compaction_res.facts,
        session_id,
    )
    .await
    .map_err(|e| anyhow!("Failed to commit compaction output: {}", e))?;

    Ok(compaction_res)
}

/// Executes compaction for a session, persists results into the eval DB, and runs the LLM Judge.
pub async fn evaluate_compaction_stage(
    eval_db: &EvalDbGuard,
    session_id: i64,
    case_id: &str,
    turns: &[SessionTurn],
    provider: &RecordingLlmProvider,
    judge: &NvidiaJudgeClient,
    case_dir: &Path,
) -> Result<CompactionEvalSummary> {
    let compaction_res =
        run_compaction_and_persist(eval_db, session_id, case_id, turns, provider).await?;

    // 6. Assemble Judge prompt
    let mut turns_rendered = String::new();
    for t in turns {
        turns_rendered.push_str(&format!("Turn {} [User]: {}\n", t.turn, t.user));
        turns_rendered.push_str(&format!("Turn {} [Assistant]: {}\n", t.turn, t.assistant));
    }

    let mut facts_rendered = String::new();
    for (idx, (category, fact_text)) in compaction_res.facts.iter().enumerate() {
        facts_rendered.push_str(&format!(
            "[FACT-{:02}] ({}) {}\n",
            idx + 1,
            category,
            fact_text
        ));
    }

    let judge_prompt = format!(
        r#"You are the Vox Senior Compaction Evaluation Judge.
Analyze the following session turns and the resulting LLM compaction output.

<session_turns>
{}
</session_turns>

<compaction_output>
Extracted Categorized Facts (Count: {}):
{}
</compaction_output>

CRITICAL INSTRUCTION: When referencing extracted facts in your report, you MUST ALWAYS cite their explicit identifier exactly as given (e.g. `[FACT-01]`, `[FACT-02]`). NEVER invent or use alternate numbering schemes.

Produce a comprehensive evaluation report in clean Markdown format with the following exact sections:

# Compaction Evaluation Report — {}

## 1. Executive Scorecard
*(Note: Every percentage score MUST explicitly state its formula with exact counts: `X / Y = Z%`. Never output an ungrounded percentage).*
- **Fact Coverage / Recall**: [X / Y = Z%] (Denominator Y = count of distinct user factual declarations in dialogue turns; Numerator X = successfully captured declarations. NEVER divide extracted facts by extracted facts).
- **Fact Precision**: [X / Y = Z%]
- **Category Routing Accuracy**: [X / Y = Z%]
- **Hallucination / Stale Fact Rate**: [X / Y = Z%]
- **Noise / Chit-chat Rejection**: [High / Medium / Low]

## 2. Fact Coverage & Completeness Analysis
List all durable factual declarations made by the user in the session turns (preferences, project statuses, decisions, personal background).
- Identify which user declarations were successfully captured, citing the matching `[FACT-XX]` identifier.
- Identify which user declarations were missed (false negatives).

## 3. Category Classification Audit
Verify whether facts were routed into their correct schema buckets (`personal`, `objective`, `workdone`, `blocker`, `next_step`, `pitfall`):
- Highlight any misclassified facts (e.g. personal preferences categorized as workdone or vice versa).

## 4. Atomic Granularity & Information Density
- Identify whether facts are individual atomic assertions or rambling composite sentences.
- Flag any fragmented or incomplete statements.

## 5. Hallucination, Stale Facts & Grounding Check
- **Ungrounded Facts Check**: Explicitly flag any hallucinated statements or ungrounded claims in the extracted facts. For each extracted `personal` fact, cite the dialogue turn number that grounds it. Flag any attribute not explicitly stated by the user (e.g. inventing housing type or residence proximity from an activity/visit).
- **Temporal Resolution Check**: Verify whether any extracted `blocker` or `next_step` was already resolved/fixed by later dialogue turns. If a resolved bug or obstacle is extracted as an active blocker, flag it as a Stale Fact defect.

## 6. Duplicate Facts & Noise Filtering Audit
- Check if identical or redundant facts were emitted multiple times in this slice.
- Verify whether conversational filler (greetings, acknowledgements, transient breaks) was properly filtered.

## 7. Final Verdict & Architectural Recommendations
Provide 2-3 concise, actionable improvements for the compaction prompt or pipeline.
"#,
        turns_rendered,
        compaction_res.facts.len(),
        facts_rendered,
        case_id
    );

    // 7. Execute Judge evaluation via NVIDIA NIM Judge
    let judge_report = judge
        .evaluate_with_trace(&judge_prompt, case_dir, "compaction")
        .await
        .map_err(|e| anyhow!("Compaction Judge evaluation failed: {}", e))?;

    // 8. Write Markdown report
    let report_path = write_markdown_report(case_dir, "compaction.md", &judge_report)?;

    Ok(CompactionEvalSummary {
        session_id,
        facts_extracted: compaction_res.facts.len(),
        session_context_len: compaction_res.session_context.len(),
        report_path,
    })
}
