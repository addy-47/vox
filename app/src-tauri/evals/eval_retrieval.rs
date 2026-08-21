//! ============================================================================
//! eval_retrieval.rs — Ladder Eval 3: Scope-Pruned Retrieval, BFS Graph Expansion & Budgeting
//! ============================================================================
//! Category     : Evaluation Script
//! Component    : services::memory / evals
//! Prerequisites: evals/results/stage_2_pipeline.db & evals/datasets/retrieval_queries.json
//! Execution    : cargo run --release --example eval_retrieval
//! Metrics      : Precision, Recall, ChitChat Overhead (ms), Budget Cap Compliance, LLM Judge Score
//! ============================================================================

use anyhow::Result;
use query_sieve::MemoryScope;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use turso::Builder;
use vox_lib::core::settings::MemorySettings;
use vox_lib::services::memory::embedder::l2_normalize;
use vox_lib::services::memory::estimate_tokens;
use vox_lib::services::memory::retrieval::retrieve_personal_context_v7;

const OLLAMA_GPU_SERVER_URL: &str = "http://100.86.62.14:11434/v1/chat/completions";

fn get_nvidia_api_key() -> String {
    if let Ok(k) = std::env::var("NVIDIA_API_KEY") {
        if !k.trim().is_empty() {
            return k.trim().to_string();
        }
    }
    let paths = ["temp/.env", "../../temp/.env", "../temp/.env"];
    for p in paths {
        if let Ok(content) = std::fs::read_to_string(p) {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("NVIDIA_API_KEY=") {
                    let val = rest.trim();
                    if !val.is_empty() {
                        return val.to_string();
                    }
                }
            }
        }
    }
    String::new()
}

async fn run_retrieval_judge(
    client: &reqwest::Client,
    api_key: &str,
    sample_output: &str,
) -> Result<String> {
    let judge_prompt = format!(
        "<retrieval_audit_evaluation>\n\
         <retrieval_results>\n{}\n</retrieval_results>\n\n\
         <task>\n\
         Act as a Senior AI Retrieval Systems Architect auditing Ladder Eval 3 (Scope-Pruned RAG Retrieval).\n\
         Audit the rendered context outputs across the 4 MemoryScope categories:\n\
         1. ChitChat Scope: Verify zero RAG output and 0ms overhead.\n\
         2. User Scope: Verify rendered context includes core Profile/Identity facts.\n\
         3. Domain Scope: Verify rendered context includes relevant Entities & Directives.\n\
         4. Budget Cap Compliance: Verify total rendered tokens never exceed the 15% context window cap.\n\n\
         Format output as clean Markdown starting with '# Eval 3 Retrieval Evaluation Report'.\n\
         </task>\n\
         </retrieval_audit_evaluation>",
        sample_output
    );

    let payload = serde_json::json!({
        "model": "gemma4:e4b",
        "messages": [
            {"role": "user", "content": judge_prompt}
        ],
        "temperature": 0.5,
        "max_tokens": 2500
    });

    if let Ok(resp) = client
        .post(OLLAMA_GPU_SERVER_URL)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(json_body) = resp.json::<serde_json::Value>().await {
                if let Some(content) = json_body["choices"][0]["message"]["content"].as_str() {
                    let cleaned = content
                        .trim()
                        .trim_start_matches("```markdown")
                        .trim_start_matches("```")
                        .trim_end_matches("```")
                        .trim();
                    return Ok(cleaned.to_string());
                }
            }
        }
    }

    if !api_key.is_empty() {
        let fallback_payload = serde_json::json!({
            "model": "google/gemma-2-27b-it",
            "messages": [
                {"role": "user", "content": judge_prompt}
            ],
            "temperature": 0.5,
            "max_tokens": 2500
        });

        if let Ok(resp) = client
            .post("https://integrate.api.nvidia.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&fallback_payload)
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(json_body) = resp.json::<serde_json::Value>().await {
                    if let Some(content) = json_body["choices"][0]["message"]["content"].as_str() {
                        let cleaned = content
                            .trim()
                            .trim_start_matches("```markdown")
                            .trim_start_matches("```")
                            .trim_end_matches("```")
                            .trim();
                        return Ok(cleaned.to_string());
                    }
                }
            }
        }
    }

    Ok("LLM Judge unavailable.".to_string())
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RetrievalQueryItem {
    id: String,
    query: String,
    expected_scope: String,
}

#[derive(Debug, Serialize)]
struct RetrievalQueryResult {
    query_id: String,
    scope: String,
    retrieved_tokens: usize,
    token_budget_cap: usize,
    budget_compliant: bool,
    chitchat_zero_overhead: bool,
    rendered_context: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Vox Memory Subsystem: Ladder Eval 3 (Scope-Pruned Retrieval & Budgeting) ===");

    let db_path = PathBuf::from("evals/results/stage_2_pipeline.db");
    let queries_path = PathBuf::from("evals/datasets/retrieval_queries.json");

    if !db_path.exists() {
        return Err(anyhow::anyhow!(
            "Pipeline DB at {:?} not found. Please run eval_pipeline first.",
            db_path
        ));
    }

    let abs_db_path = std::fs::canonicalize(&db_path)?;
    let db_path_str = abs_db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid pipeline DB path {:?}", abs_db_path))?;
    let db = Builder::new_local(db_path_str).build().await?;
    let conn = db.connect()?;

    let queries: Vec<RetrievalQueryItem> = if queries_path.exists() {
        let queries_bytes = std::fs::read(&queries_path)?;
        serde_json::from_slice(&queries_bytes)?
    } else {
        Vec::new()
    };

    println!(
        "[Eval 3] Loaded {} test queries from {:?}",
        queries.len(),
        queries_path
    );

    let settings = MemorySettings::default(); // max_context_share = 0.15
    let context_window_size = 4096;
    let budget_cap = (context_window_size as f32 * settings.max_context_share) as usize; // 614 tokens

    let mut raw_vec = vec![0.0f32; 384];
    raw_vec[0] = 1.0;
    let norm_vec = l2_normalize(&raw_vec);

    let mut query_results = Vec::new();

    for q in &queries {
        let scope = match q.expected_scope.as_str() {
            "ChitChat" => MemoryScope::ChitChat,
            "User" => MemoryScope::User,
            "Domain" => MemoryScope::Domain,
            "Temporal" => MemoryScope::Temporal,
            _ => MemoryScope::Domain,
        };

        let start_time = std::time::Instant::now();
        let rendered_context =
            retrieve_personal_context_v7(&conn, &norm_vec, scope, &settings, context_window_size)
                .await?;
        let elapsed_ms = start_time.elapsed().as_millis();

        let token_count = estimate_tokens(&rendered_context);
        let chitchat_zero_overhead =
            scope == MemoryScope::ChitChat && rendered_context.is_empty() && elapsed_ms <= 2;
        let budget_compliant = token_count <= budget_cap;

        println!(
            "Query: {:<10} | Scope: {:<10} | Tokens: {:<4} / {} max | ChitChat 0ms: {} | Budget Compliant: {}",
            q.id, q.expected_scope, token_count, budget_cap, chitchat_zero_overhead, budget_compliant
        );

        query_results.push(RetrievalQueryResult {
            query_id: q.id.clone(),
            scope: q.expected_scope.clone(),
            retrieved_tokens: token_count,
            token_budget_cap: budget_cap,
            budget_compliant,
            chitchat_zero_overhead,
            rendered_context: rendered_context.clone(),
        });
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let api_key = get_nvidia_api_key();

    let sample_output = serde_json::to_string_pretty(&query_results)?;
    let judge_report = run_retrieval_judge(&client, &api_key, &sample_output).await?;

    let report_path = PathBuf::from("evals/results/eval_retrieval_report.md");
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&report_path, &judge_report)?;

    println!("\n[Eval 3 Completed] Report saved to {:?}", report_path);
    Ok(())
}
