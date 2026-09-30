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

    // 6. Assemble Judge prompt
    let mut turns_rendered = String::new();
    for t in turns {
        turns_rendered.push_str(&format!("Turn {} [User]: {}\n", t.turn, t.user));
        turns_rendered.push_str(&format!("Turn {} [Assistant]: {}\n", t.turn, t.assistant));
    }

    let mut facts_rendered = String::new();
    for (category, fact_text) in &compaction_res.facts {
        facts_rendered.push_str(&format!("- [{}] {}\n", category, fact_text));
    }

    let judge_prompt = format!(
        r#"You are the Vox Senior Compaction Evaluation Judge.
Analyze the following session turns and the resulting LLM compaction output.

<session_turns>
{}
</session_turns>

<compaction_output>
Context Summary:
{}

Extracted Categorized Facts (Count: {}):
{}
</compaction_output>

Produce a comprehensive evaluation report in clean Markdown format with the following exact sections:

# Compaction Evaluation Report — {}

## 1. Executive Scorecard
- **Fact Coverage / Recall**: [0-100%]
- **Fact Precision**: [0-100%]
- **Category Routing Accuracy**: [0-100%]
- **Context Summary Fidelity**: [0-100%]
- **Hallucination Rate**: [0-100%]
- **Noise / Chit-chat Rejection**: [High / Medium / Low]

## 2. Fact Coverage & Completeness Analysis
List all durable factual declarations made by the user in the session turns (preferences, project statuses, decisions, personal background).
- Identify which facts were successfully captured.
- Identify which facts were missed (false negatives).

## 3. Category Classification Audit
Verify whether facts were routed into their correct schema buckets (`personal`, `objective`, `workdone`, `blocker`, `next_step`, `pitfall`):
- Highlight any misclassified facts (e.g. personal preferences categorized as workdone).

## 4. Atomic Granularity & Information Density
- Identify whether facts are individual atomic assertions or rambling composite sentences.
- Flag any fragmented or incomplete statements.

## 5. Context Summary Fidelity & Hallucination Check
- Audit the `Context Summary` against the session turns.
- Explicitly flag any hallucinated statements or ungrounded claims.

## 6. Duplicate Facts & Noise Filtering Audit
- Check if identical or redundant facts were emitted multiple times in this slice.
- Verify whether conversational filler (greetings, acknowledgements) was properly filtered.

## 7. Final Verdict & Architectural Recommendations
Provide 2-3 concise, actionable improvements for the compaction prompt or pipeline.
"#,
        turns_rendered,
        compaction_res.session_context,
        compaction_res.facts.len(),
        facts_rendered,
        case_id
    );

    // 7. Execute Judge evaluation via NVIDIA NIM Judge
    let judge_report = judge
        .evaluate(&judge_prompt)
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
