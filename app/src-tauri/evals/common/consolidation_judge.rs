use std::time::Duration;

use super::{
    judge,
    pipeline_report::{JudgeObservation, SuggestionObservation},
    report,
};

pub const JUDGE_MAX_TOKENS: u32 = 6000;
/// Ollama truncates a long judge payload to its default context window unless
/// told otherwise, silently removing the evidence the judge must grade.
pub const JUDGE_NUM_CTX: u32 = 32768;

#[derive(Debug, Clone)]
pub struct JudgeSettings {
    pub enabled: bool,
    pub url: String,
    pub model: String,
}

/// One judge pass over the accepted document. Report-only: the verdict never
/// gates the case, it is evidence for and against the mechanical assertions.
pub async fn judge_consolidation(
    settings: &JudgeSettings,
    candidate_facts: &[String],
    base_document: &str,
    operations: &[SuggestionObservation],
    accepted_document: &str,
    stage_timeout: Duration,
) -> JudgeObservation {
    let system_prompt = match judge::load_prompt("judge_pipeline_anchor_survival.md") {
        Ok(prompt) => prompt,
        Err(error) => {
            return JudgeObservation {
                model: settings.model.clone(),
                verdict: "JUDGE_UNAVAILABLE".to_string(),
                latency_s: 0.0,
                coverage_score: None,
                quality_score: None,
                groundedness_score: None,
                anchor_survival_score: None,
                claimed_anchor_losses: Vec::new(),
                claimed_inventions: Vec::new(),
                report_markdown: format!("Failed to load judge prompt: {error:#}"),
            }
        }
    };
    let user_content = format!(
        "NEW_FACTS:\n{facts}\n\nBASE_DOCUMENT:\n{base}\n\nOPERATIONS:\n{ops}\n\nDOCUMENT:\n{accepted}",
        facts = serde_json::to_string_pretty(candidate_facts).unwrap_or_default(),
        base = base_document,
        ops = serde_json::to_string_pretty(
            &operations
                .iter()
                .map(|operation| serde_json::json!({
                    "op": operation.op,
                    "target_index": operation.target_index,
                    "content": operation.content,
                }))
                .collect::<Vec<_>>()
        )
        .unwrap_or_default(),
        accepted = accepted_document,
    );
    match tokio::time::timeout(
        stage_timeout,
        judge::run_judge_with_transport(
            &settings.url,
            "",
            &settings.model,
            &system_prompt,
            &user_content,
            JUDGE_MAX_TOKENS,
            judge::JudgeTransport::ollama(JUDGE_NUM_CTX),
        ),
    )
    .await
    {
        Ok(Ok(output)) => JudgeObservation {
            model: settings.model.clone(),
            verdict: output.verdict.as_str().to_string(),
            latency_s: output.latency_s,
            coverage_score: report::extract_score(&output.report_markdown, "SCORE_COVERAGE"),
            quality_score: report::extract_score(&output.report_markdown, "SCORE_QUALITY"),
            groundedness_score: report::extract_score(
                &output.report_markdown,
                "SCORE_GROUNDEDNESS",
            ),
            anchor_survival_score: report::extract_score(
                &output.report_markdown,
                "SCORE_ANCHOR_SURVIVAL",
            ),
            claimed_anchor_losses: report::extract_tagged_items(
                &output.report_markdown,
                "ANCHOR_LOSS",
            ),
            claimed_inventions: report::extract_tagged_items(&output.report_markdown, "INVENTION"),
            report_markdown: output.report_markdown,
        },
        Ok(Err(error)) => JudgeObservation {
            model: settings.model.clone(),
            verdict: "JUDGE_ERROR".to_string(),
            latency_s: 0.0,
            coverage_score: None,
            quality_score: None,
            groundedness_score: None,
            anchor_survival_score: None,
            claimed_anchor_losses: Vec::new(),
            claimed_inventions: Vec::new(),
            report_markdown: format!("{error:#}"),
        },
        Err(_) => JudgeObservation {
            model: settings.model.clone(),
            verdict: "JUDGE_TIMEOUT".to_string(),
            latency_s: 0.0,
            coverage_score: None,
            quality_score: None,
            groundedness_score: None,
            anchor_survival_score: None,
            claimed_anchor_losses: Vec::new(),
            claimed_inventions: Vec::new(),
            report_markdown: "Judge call timed out".to_string(),
        },
    }
}
