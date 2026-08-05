//! ============================================================================
//! eval_compaction.rs — Ladder Eval 1: Real Multi-Window LLM Compaction & Semantic Quality
//! ============================================================================
//! Category     : Evaluation Script
//! Component    : services::memory / evals
//! Prerequisites: evals/datasets/curated_300_turns.json
//! Execution    : cargo run --example eval_compaction
//! Metrics      : Real LLM Fact Extraction, Accuracy (0-100), Redundancy %, Disambiguation, Recall
//! ============================================================================

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use turso::Builder;
use vox_lib::core::constants::COMPACTION_SYSTEM_PROMPT;
use vox_lib::persistence::mutations::enqueue_personal_facts;
use vox_lib::persistence::schema::run_migrations;
use vox_lib::utils::json::parse_compaction_json;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ConversationTurn {
    turn: u32,
    user: String,
    assistant: String,
}

fn resolve_path(rel: &str) -> PathBuf {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if base.ends_with("app/src-tauri") {
        base.join(rel)
    } else {
        base.join("app/src-tauri").join(rel)
    }
}

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

async fn extract_facts_via_nvidia_llm(
    client: &reqwest::Client,
    api_key: &str,
    window_turns: &[ConversationTurn],
    chunk_idx: usize,
) -> Result<HashMap<String, Vec<String>>> {
    let mut history_text = String::new();
    for turn in window_turns {
        history_text.push_str(&format!("User: {}\nAssistant: {}\n\n", turn.user, turn.assistant));
    }

    let user_content = format!(
        "<conversation_history>\n{}\n</conversation_history>\n\n\
         <task>\n\
         Analyze the <conversation_history> above and extract all stated facts into the 6 collections from the <schema>.\n\
         Follow every rule in <rules>. Output ONLY the JSON object starting with {{ and ending with }}.\n\
         </task>",
        history_text
    );

    let payload = serde_json::json!({
        "model": "meta/llama-3.1-8b-instruct",
        "messages": [
            {"role": "system", "content": COMPACTION_SYSTEM_PROMPT},
            {"role": "user", "content": user_content}
        ],
        "temperature": 0.1,
        "max_tokens": 1500
    });

    println!("[Eval 1 Chunk {}] Requesting compaction extraction via Nvidia API...", chunk_idx + 1);

    let max_retries = 3;
    let mut last_err = anyhow!("Unknown error");

    for attempt in 1..=max_retries {
        match client
            .post("https://integrate.api.nvidia.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    let json_body: serde_json::Value = resp.json().await?;
                    let content = json_body["choices"][0]["message"]["content"]
                        .as_str()
                        .ok_or_else(|| anyhow!("No content in Nvidia API response for chunk {}", chunk_idx + 1))?;

                    if let Some(parsed) = parse_compaction_json(content) {
                        let fact_count: usize = parsed.values().map(|v| v.len()).sum();
                        println!("[Eval 1 Chunk {}] Extracted {} facts across collections.", chunk_idx + 1, fact_count);
                        return Ok(parsed);
                    } else {
                        println!("[Eval 1 Chunk {}] Warning: Failed to parse JSON from response:\n{}", chunk_idx + 1, content);
                        return Ok(HashMap::new());
                    }
                } else {
                    let err_text = resp.text().await.unwrap_or_default();
                    last_err = anyhow!("Nvidia API returned error status: {}", err_text);
                }
            }
            Err(e) => {
                last_err = anyhow!("Request to Nvidia API failed: {}", e);
            }
        }

        if attempt < max_retries {
            println!("[Eval 1 Chunk {}] Attempt {} failed ({}). Retrying in 3s...", chunk_idx + 1, attempt, last_err);
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }

    Err(anyhow!("Nvidia API call failed for chunk {} after {} attempts. Last error: {}", chunk_idx + 1, max_retries, last_err))
}

async fn run_llm_compaction_judge_report(
    client: &reqwest::Client,
    api_key: &str,
    raw_dialogue: &str,
    extracted_facts_json: &str,
) -> Result<String> {
    let judge_prompt = format!(
        "<judge_compaction_evaluation>\n\
         <raw_dialogue>\n{}\n</raw_dialogue>\n\n\
         <extracted_facts>\n{}\n</extracted_facts>\n\n\
         <task>\n\
         Act as an expert AI Evaluation Judge. Analyze <extracted_facts> against <raw_dialogue>.\n\
         Write a comprehensive, highly detailed Markdown Evaluation Report auditing the compaction engine across 5 core pillars:\n\
         1. Overall Assessment & Score Breakdown (Overall Score out of 100, Fact Accuracy %, Redundancy %, Schema Disambiguation %, Recall Coverage %).\n\
         2. Information Coverage & Recall Analysis (Did extracted facts capture all critical user information across conversation turns, or was vital context silently dropped?).\n\
         3. Redundancy & Over-Extraction Audit (Identify specific duplicate or redundant fact strings extracted across sliding windows).\n\
         4. Collection Disambiguation & Category Correctness (Verify if facts were assigned to the correct collections: Identity, Directives, Profile, Entities, Constraints, Narrative. Call out any misclassified facts).\n\
         5. Hallucinations & Precision Check (Check if any extracted fact is false, hallucinated, or unstated in raw_dialogue).\n\
         6. Actionable System Recommendations (Provide concrete recommendations to optimize the compaction prompt, schema boundaries, or token windowing).\n\n\
         Format your output ONLY as a clean, complete GitHub-flavored Markdown report starting with '# Eval 1 Compaction Evaluation Report'.\n\
         DO NOT output raw JSON or code fences around the report.\n\
         </task>\n\
         </judge_compaction_evaluation>",
        raw_dialogue, extracted_facts_json
    );

    let payload = serde_json::json!({
        "model": "meta/llama-3.1-70b-instruct",
        "messages": [
            {"role": "user", "content": judge_prompt}
        ],
        "temperature": 0.2,
        "max_tokens": 2500
    });

    let max_retries = 3;
    let mut last_err = anyhow!("Unknown error");

    for attempt in 1..=max_retries {
        match client
            .post("https://integrate.api.nvidia.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    let json_body: serde_json::Value = resp.json().await?;
                    let content = json_body["choices"][0]["message"]["content"]
                        .as_str()
                        .ok_or_else(|| anyhow!("Invalid response structure from LLM Judge API"))?;

                    let cleaned = content
                        .trim()
                        .trim_start_matches("```markdown")
                        .trim_start_matches("```")
                        .trim_end_matches("```")
                        .trim();

                    return Ok(cleaned.to_string());
                } else {
                    let err_text = resp.text().await.unwrap_or_default();
                    last_err = anyhow!("LLM Judge Nvidia API returned error: {}", err_text);
                }
            }
            Err(e) => {
                last_err = anyhow!("Request to LLM Judge Nvidia API failed: {}", e);
            }
        }

        if attempt < max_retries {
            println!("[Eval 1 LLM Judge] Attempt {} failed ({}). Retrying in 5s...", attempt, last_err);
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    Err(anyhow!("LLM Judge Nvidia API call failed after {} attempts. Last error: {}", max_retries, last_err))
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Vox Memory Subsystem: Ladder Eval 1 (Real Multi-Window LLM Compaction) ===");

    let api_key = get_nvidia_api_key();
    if api_key.is_empty() {
        return Err(anyhow!("NVIDIA_API_KEY not found in environment or temp/.env. Cannot run real LLM compaction."));
    }

    let dataset_path = resolve_path("evals/datasets/curated_300_turns.json");
    let output_db_path = resolve_path("evals/results/stage_1_compaction.db");

    if let Some(parent) = output_db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if output_db_path.exists() {
        let _ = std::fs::remove_file(&output_db_path);
    }
    std::fs::File::create(&output_db_path)?;

    let abs_db_path = std::fs::canonicalize(&output_db_path)?;
    let db_path_str = abs_db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid output DB path {:?}", abs_db_path))?;
    let db = Builder::new_local(db_path_str).build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    let dataset_bytes = std::fs::read(&dataset_path)
        .map_err(|e| anyhow::anyhow!("Failed to read dataset at {:?}: {}", dataset_path, e))?;
    let turns: Vec<ConversationTurn> = serde_json::from_slice(&dataset_bytes)?;

    println!("[Eval 1] Loaded {} turns from {:?}", turns.len(), dataset_path);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    // Chunk 300 turns into 30-turn windows (10 windows total)
    let window_size = 30;
    let chunks: Vec<&[ConversationTurn]> = turns.chunks(window_size).collect();
    println!("[Eval 1] Divided 300 turns into {} sliding context windows ({} turns/window)", chunks.len(), window_size);

    let mut accumulated_facts: HashMap<String, Vec<String>> = HashMap::new();
    let mut raw_dialogue_summary = String::new();

    for (idx, chunk) in chunks.iter().enumerate() {
        for t in *chunk {
            raw_dialogue_summary.push_str(&format!("Turn {}: User: {} | Asst: {}\n", t.turn, t.user, t.assistant));
        }

        match extract_facts_via_nvidia_llm(&client, &api_key, chunk, idx).await {
            Ok(facts) => {
                for (col, fact_list) in facts {
                    accumulated_facts.entry(col).or_default().extend(fact_list);
                }
            }
            Err(e) => {
                println!("[Eval 1 Chunk {} Error] {}", idx + 1, e);
            }
        }

        // Pacing delay between chunk extraction calls to prevent API rate limiting
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let mut facts_by_collection = HashMap::new();
    let mut total_extracted = 0;
    for (col, list) in &accumulated_facts {
        facts_by_collection.insert(col.clone(), list.len());
        total_extracted += list.len();
    }

    println!("\n[Eval 1] Real LLM Fact Extraction Completed. Extracted {} total facts.", total_extracted);
    for (col, count) in &facts_by_collection {
        println!("  - Collection {:<12}: {} facts", col, count);
    }

    // Enqueue all extracted facts into personal_memory_queue in stage_1_compaction.db
    enqueue_personal_facts(&conn, accumulated_facts.clone(), "session_eval_1", true).await?;

    let mut count_row = conn.query("SELECT COUNT(*) FROM personal_memory_queue", ()).await?;
    let total_enqueued: i64 = count_row
        .next()
        .await?
        .ok_or_else(|| anyhow::anyhow!("Failed to query personal_memory_queue count"))?
        .get(0)?;

    println!("[Eval 1] Enqueued {} facts into personal_memory_queue at {:?}", total_enqueued, output_db_path);

    // Run Deep Semantic LLM-as-a-Judge evaluation over raw dialogue turns AND extracted facts
    println!("\n[Eval 1] Requesting Deep Semantic LLM-as-a-Judge Markdown Evaluation via Nvidia API...");
    let extracted_facts_json = serde_json::to_string_pretty(&accumulated_facts)?;

    let judge_markdown_report = run_llm_compaction_judge_report(
        &client,
        &api_key,
        &raw_dialogue_summary,
        &extracted_facts_json,
    )
    .await?;

    let report_path = resolve_path("evals/results/eval1_compaction_report.md");
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&report_path, &judge_markdown_report)?;

    println!("\n=========================================================================");
    println!("[Eval 1 Deep Semantic LLM-as-a-Judge Evaluation Complete]");
    println!("  Turns Processed           : {}", turns.len());
    println!("  Compaction Windows (30t)  : {}", chunks.len());
    println!("  Total Facts Extracted     : {}", total_extracted);
    println!("  Total Facts Enqueued in DB: {}", total_enqueued);
    println!("  Output Database Artifact  : {:?}", output_db_path);
    println!("  -----------------------------------------------------------------------");
    println!("  LLM Judge Report Saved To : {:?}", report_path);
    println!("=========================================================================\n");
    println!("--- LLM Judge Markdown Report Preview ---\n{}\n", judge_markdown_report);

    Ok(())
}
