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

const NVIDIA_CHAT_URL: &str = "https://integrate.api.nvidia.com/v1/chat/completions";

/// Outcome of one judge call: parsed JSON verdict plus audit trail.
#[derive(Debug)]
pub struct JudgeOutput {
    pub verdict: Value,
    pub raw_content: String,
    pub latency_s: f64,
}

/// Loads a judge prompt template from evals assets.
pub fn load_prompt(name: &str) -> Result<String> {
    let path = format!("{}/evals/assets/prompts/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).with_context(|| format!("Failed to read judge prompt at {path}"))
}

/// Calls the Nvidia-hosted judge model once (temperature 0) and parses the
/// response body as JSON (tolerating ```json fences). The full prompt and raw
/// response are returned inside the report for audit; this function never
/// grades — it only transports the judge's words.
pub async fn run_judge(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_content: &str,
    max_tokens: u32,
) -> Result<JudgeOutput> {
    let started = Instant::now();
    let client = reqwest::Client::new();
    let resp = client
        .post(NVIDIA_CHAT_URL)
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
        .await
        .context("Judge HTTP request failed")?;
    let status = resp.status();
    let body: Value = resp.json().await.context("Judge response was not JSON")?;
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
    let latency_s = started.elapsed().as_secs_f64();

    let de_fenced = strip_code_fences(&content);
    let verdict: Value = serde_json::from_str(de_fenced).with_context(|| {
        format!(
            "Judge output was not JSON. Raw (first 500 chars): {}",
            content.chars().take(500).collect::<String>()
        )
    })?;
    Ok(JudgeOutput {
        verdict,
        raw_content: content,
        latency_s,
    })
}

fn strip_code_fences(s: &str) -> &str {
    let t = s.trim();
    let t = t
        .strip_prefix("```json")
        .or_else(|| t.strip_prefix("```"))
        .unwrap_or(t);
    t.strip_suffix("```").unwrap_or(t).trim()
}
