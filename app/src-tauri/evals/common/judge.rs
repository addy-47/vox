//! ============================================================================
//! judge.rs — Single-call Nvidia hosted judge client for the memory ladder
//! ============================================================================
//! Category     : Evaluation (shared harness, not a runnable eval)
//! Component    : evals/common (Nvidia OpenAI-compatible API)
//! Prerequisites: NVIDIA_API_KEY env var at eval runtime
//! Execution    : Included via #[path] from evals/memory_*_eval.rs
//! Metrics      : Judge latency + raw prompt/response passthrough for audit
//! ============================================================================

use std::time::Instant;

use anyhow::{Context, Result};
use serde_json::{json, Value};

const DEFAULT_JUDGE_URL: &str = "https://integrate.api.nvidia.com/v1/chat/completions";

/// Outcome of one judge call: fluid markdown report plus audit trail.
/// The verdict is read from the judge's closing `VERDICT: PASS|FAIL` line.
#[derive(Debug)]
pub struct JudgeOutput {
    pub report_markdown: String,
    pub verdict: JudgeVerdict,
    pub latency_s: f64,
}

/// Machine-readable verdict extracted from the report's closing line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudgeVerdict {
    Pass,
    Fail,
    Unknown,
}

impl JudgeVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            JudgeVerdict::Pass => "PASS",
            JudgeVerdict::Fail => "FAIL",
            JudgeVerdict::Unknown => "UNKNOWN",
        }
    }
}

/// Loads a judge prompt template from evals assets.
pub fn load_prompt(name: &str) -> Result<String> {
    let path = format!("{}/evals/assets/prompts/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).with_context(|| format!("Failed to read judge prompt at {path}"))
}

/// Calls the Nvidia-hosted judge model once (temperature 0) and returns the
/// markdown report as-is. The closing `VERDICT: PASS|FAIL` line is extracted
/// for loop control; a missing line yields `Unknown` (treated as FAIL by
/// callers — a judge that won't commit to a verdict proves nothing).
/// The full report is returned inside the eval report for audit; this function
/// never grades — it only transports the judge's words.
pub async fn run_judge(
    base_url: &str,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_content: &str,
    max_tokens: u32,
) -> Result<JudgeOutput> {
    let url = if base_url.trim().is_empty() {
        DEFAULT_JUDGE_URL.to_string()
    } else {
        resolve_chat_completions_url(base_url)
    };
    let started = Instant::now();
    let client = reqwest::Client::new();
    // Retry transient failures (transport errors, 429/5xx) up to 3 attempts;
    // a 4xx other than 429 is a real request error and fails immediately.
    let mut attempt = 0;
    let content = loop {
        attempt += 1;
        let resp = client
            .post(&url)
            .bearer_auth(api_key)
            .json(&json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_content},
                ],
                "temperature": 0,
                "max_tokens": max_tokens,
            }))
            .send()
            .await;
        let (status, body) = match resp {
            Err(e) if attempt < 3 => {
                eprintln!("[judge] attempt {attempt} transport error ({e}); retrying...");
                tokio::time::sleep(std::time::Duration::from_secs(10 * attempt as u64)).await;
                continue;
            }
            Err(e) => return Err(e).context("Judge HTTP request failed"),
            Ok(resp) => {
                let status = resp.status();
                let body: Value = resp.json().await.context("Judge response was not JSON")?;
                if !status.is_success()
                    && (status.as_u16() == 429 || status.is_server_error())
                    && attempt < 3
                {
                    eprintln!("[judge] attempt {attempt} HTTP {status}; retrying...");
                    tokio::time::sleep(std::time::Duration::from_secs(10 * attempt as u64)).await;
                    continue;
                }
                (status, body)
            }
        };
        if !status.is_success() {
            anyhow::bail!(
                "Judge HTTP {status}: {}",
                body.to_string().chars().take(500).collect::<String>()
            );
        }
        let content = body
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .context("Judge response had no choices[0].message.content")?
            .to_string();
        // A completed round-trip whose report stops before the verdict
        // (model emitted EOS early) is also retried — without a verdict line
        // the report cannot drive the iteration loop.
        if extract_verdict(&content) == JudgeVerdict::Unknown && attempt < 3 {
            eprintln!("[judge] attempt {attempt} report has no VERDICT line; retrying...");
            tokio::time::sleep(std::time::Duration::from_secs(10 * attempt as u64)).await;
            continue;
        }
        break content;
    };
    let latency_s = started.elapsed().as_secs_f64();

    Ok(JudgeOutput {
        verdict: extract_verdict(&content),
        report_markdown: content,
        latency_s,
    })
}

/// Resolves a bare base URL to the canonical chat-completions endpoint.
fn resolve_chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

/// Reads the last `VERDICT: <PASS|FAIL>` line (case-insensitive, tolerating
/// markdown emphasis like `**VERDICT: FAIL**` or `## VERDICT: PASS`) from the report.
fn extract_verdict(report: &str) -> JudgeVerdict {
    for line in report.lines().rev() {
        let t = line
            .trim()
            .trim_start_matches(['#', '*', '_', ' '])
            .trim_end_matches(['*', '_', ' ']);
        if t.len() < 13 || !t.to_ascii_lowercase().starts_with("verdict") {
            continue;
        }
        let value = t[7..]
            .trim_start_matches([':', ' ', '*'])
            .trim()
            .to_ascii_uppercase();
        if value.starts_with("PASS") {
            return JudgeVerdict::Pass;
        }
        if value.starts_with("FAIL") {
            return JudgeVerdict::Fail;
        }
    }
    JudgeVerdict::Unknown
}
