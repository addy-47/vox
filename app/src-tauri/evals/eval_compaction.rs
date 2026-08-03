//! ============================================================================
//! eval_compaction.rs — Ladder Eval 1: Real Multi-Window LLM Compaction & Semantic Quality
//! ============================================================================
//! Category     : Evaluation Script
//! Component    : services::memory / evals
//! Prerequisites: evals/datasets/curated_300_turns.json
//! Execution    : cargo run --example eval_compaction
//! Metrics      : Real LLM Fact Extraction, Accuracy (0-100), Redundancy %, Disambiguation, Recall
//! ============================================================================

mod llm_judge;

use anyhow::{anyhow, Result};
use llm_judge::{evaluate_compaction_quality, get_nvidia_api_key, CompactionJudgeMetrics, JudgeProvider};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize)]
struct CompactionEvalReport {
    turns_processed: usize,
    chunks_processed: usize,
    facts_enqueued: usize,
    facts_by_collection: HashMap<String, usize>,
    output_db_path: String,
    judge_metrics: CompactionJudgeMetrics,
}

fn resolve_path(rel: &str) -> PathBuf {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if base.ends_with("app/src-tauri") {
        base.join(rel)
    } else {
        base.join("app/src-tauri").join(rel)
    }
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
        "model": "meta/llama-3.3-70b-instruct",
        "messages": [
            {"role": "system", "content": COMPACTION_SYSTEM_PROMPT},
            {"role": "user", "content": user_content}
        ],
        "temperature": 0.1,
        "max_tokens": 1500
    });

    println!("[Eval 1 Chunk {}] Requesting compaction extraction via Nvidia API...", chunk_idx + 1);

    let resp = client
        .post("https://integrate.api.nvidia.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("Nvidia API call failed for chunk {}: {}", chunk_idx + 1, err_text));
    }

    let json_body: serde_json::Value = resp.json().await?;
    let content = json_body["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow!("No content in Nvidia API response for chunk {}", chunk_idx + 1))?;

    if let Some(parsed) = parse_compaction_json(content) {
        let fact_count: usize = parsed.values().map(|v| v.len()).sum();
        println!("[Eval 1 Chunk {}] Extracted {} facts across collections.", chunk_idx + 1, fact_count);
        Ok(parsed)
    } else {
        println!("[Eval 1 Chunk {}] Warning: Failed to parse JSON from response:\n{}", chunk_idx + 1, content);
        Ok(HashMap::new())
    }
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
        .timeout(std::time::Duration::from_secs(45))
        .build()?;

    // Chunk 300 turns into 30-turn windows (10 windows total)
    let window_size = 30;
    let chunks: Vec<&[ConversationTurn]> = turns.chunks(window_size).collect();
    println!("[Eval 1] Divided 300 turns into {} sliding context windows ({} turns/window)", chunks.len(), window_size);

    let mut accumulated_facts: HashMap<String, Vec<String>> = HashMap::new();
    let mut raw_dialogue_summary = String::new();

    for (idx, chunk) in chunks.iter().enumerate() {
        for t in *chunk {
            raw_dialogue_summary.push_str(&format!("Turn {}: User: {} | Assistant: {}\n", t.turn, t.user, t.assistant));
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
    println!("\n[Eval 1] Requesting Deep Semantic LLM-as-a-Judge Evaluation via Nvidia API...");
    let extracted_facts_json = serde_json::to_string_pretty(&accumulated_facts)?;

    let judge_metrics = evaluate_compaction_quality(
        JudgeProvider::NvidiaApi,
        &raw_dialogue_summary,
        &extracted_facts_json,
    )
    .await?;

    let report = CompactionEvalReport {
        turns_processed: turns.len(),
        chunks_processed: chunks.len(),
        facts_enqueued: total_enqueued as usize,
        facts_by_collection,
        output_db_path: output_db_path.to_string_lossy().to_string(),
        judge_metrics: judge_metrics.clone(),
    };

    let report_json = serde_json::to_string_pretty(&report)?;
    let report_path = resolve_path("evals/results/eval_compaction_results.json");
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&report_path, report_json)?;

    println!("\n=========================================================================");
    println!("[Eval 1 Deep Semantic Metrics & Results]");
    println!("  Turns Processed           : {}", turns.len());
    println!("  Compaction Windows (30t)  : {}", chunks.len());
    println!("  Total Facts Extracted     : {}", total_extracted);
    println!("  Total Facts Enqueued in DB: {}", total_enqueued);
    println!("  Output Database Artifact  : {:?}", output_db_path);
    println!("  -----------------------------------------------------------------------");
    println!("  LLM Judge Provider        : Nvidia API (meta/llama-3.3-70b-instruct)");
    println!("  Overall Score             : {} / 100", judge_metrics.overall_score);
    println!("  Fact Accuracy Score       : {} / 100", judge_metrics.fact_accuracy_score);
    println!("  Redundancy Percentage     : {:.1}%", judge_metrics.redundancy_pct);
    println!("  Schema Disambiguation Score: {} / 100", judge_metrics.collection_disambiguation_score);
    println!("  Recall & Coverage Score   : {} / 100", judge_metrics.recall_coverage_score);
    println!("  -----------------------------------------------------------------------");
    println!("  Hallucinations Found      : {:?}", judge_metrics.hallucinations_found);
    println!("  Redundant Facts Found     : {:?}", judge_metrics.redundant_facts_found);
    println!("  Misclassified Facts Found : {:?}", judge_metrics.misclassified_facts_found);
    println!("  -----------------------------------------------------------------------");
    println!("  LLM Judge Reasoning Critique:\n{}", judge_metrics.detailed_reasoning);
    println!("  Full JSON Report Saved To : {:?}", report_path);
    println!("=========================================================================\n");

    Ok(())
}
