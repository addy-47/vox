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

const OLLAMA_GPU_SERVER_URL: &str = "http://100.86.62.14:11434/v1/chat/completions";

async fn post_chat_completion(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    messages: serde_json::Value,
    max_tokens: usize,
) -> Result<String> {
    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": 0.5,
        "max_tokens": max_tokens
    });

    // Attempt 1: Try Ollama GPU Server
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

    // Fallback: Nvidia API
    if !api_key.is_empty() {
        let fallback_model = if model.contains("gemma") {
            "google/gemma-2-27b-it"
        } else {
            "meta/llama-3.1-70b-instruct"
        };
        let fallback_payload = serde_json::json!({
            "model": fallback_model,
            "messages": messages,
            "temperature": 0.5,
            "max_tokens": max_tokens
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

    Err(anyhow!("Chat completion failed on both Ollama GPU Server and Nvidia API fallback"))
}

async fn extract_facts_via_nvidia_llm(
    client: &reqwest::Client,
    api_key: &str,
    window_turns: &[ConversationTurn],
    chunk_idx: usize,
) -> Result<HashMap<String, Vec<String>>> {
    let mut history_text = String::new();
    for turn in window_turns {
        history_text.push_str(&format!(
            "User: {}\nAssistant: {}\n\n",
            turn.user, turn.assistant
        ));
    }

    let user_content = format!(
        "<conversation_history>\n{}\n</conversation_history>\n\n\
         <task>\n\
         Analyze the <conversation_history> above and extract all stated facts into the 6 collections from the <schema>.\n\
         Follow every rule in <rules>\n\
         Output ONLY the JSON object starting with {{ and ending with }}.\n\
         </task>",
        history_text
    );

    let messages = serde_json::json!([
        {"role": "system", "content": COMPACTION_SYSTEM_PROMPT},
        {"role": "user", "content": user_content}
    ]);

    println!(
        "[Eval 1 Chunk {}] Requesting compaction extraction via GPU Ollama Server (llama3.1:8b)...",
        chunk_idx + 1
    );

    match post_chat_completion(client, api_key, "llama3.1:8b", messages, 2000).await {
        Ok(content) => {
            if let Some(parsed) = parse_compaction_json(&content) {
                let fact_count: usize = parsed.values().map(|v| v.len()).sum();
                println!(
                    "[Eval 1 Chunk {}] Extracted {} facts across collections.",
                    chunk_idx + 1,
                    fact_count
                );
                Ok(parsed)
            } else {
                println!(
                    "[Eval 1 Chunk {}] Warning: Failed to parse JSON from response:\n{}",
                    chunk_idx + 1,
                    content
                );
                Ok(HashMap::new())
            }
        }
        Err(e) => Err(anyhow!("Compaction extraction failed for chunk {}: {}", chunk_idx + 1, e)),
    }
}

async fn run_llm_subbatch_judge_report(
    client: &reqwest::Client,
    api_key: &str,
    subbatch_num: usize,
    turns_start: u32,
    turns_end: u32,
    raw_dialogue: &str,
    extracted_facts_json: &str,
) -> Result<String> {
    let judge_prompt = format!(
        "<judge_subbatch_evaluation subbatch=\"{:02}\" turns=\"{}-{}\">\n\
         <raw_dialogue>\n{}\n</raw_dialogue>\n\n\
         <extracted_facts>\n{}\n</extracted_facts>\n\n\
         <task>\n\
         Act as a Principal AI Knowledge Graph & Memory Systems Auditor performing a strict, evidence-anchored audit of extracted facts against raw dialogue for Turns {} to {}.\n\n\
         Write a comprehensive Markdown Evaluation Report auditing compaction performance in this sub-batch across the following 5 mandatory criteria:\n\n\
         1. Fact Quality & Self-Containment Audit (CRITICAL):\n\
            - Audit every extracted fact string. Check if facts are complete, grammatically whole, self-contained declarative statements.\n\
            - Explicitly flag any LOW-QUALITY EXTRACTIONS, such as bare entity names, single-word labels, or incomplete fragments.\n\
            - Count exact number of bare-entity/single-word extractions vs self-contained declarative statements.\n\n\
         2. Information Coverage & Detail Density Audit:\n\
            - Compare <extracted_facts> against <raw_dialogue> for Turns {}-{}.\n\
            - Did extracted facts preserve full context, exact numbers/quantities ($5,000 budget cap, 5 miles, 7 AM), temporal markers, and specific constraints/directives?\n\
            - Detail any critical user facts that were SILENTLY DROPPED or OVER-SIMPLIFIED into generic statements.\n\n\
         3. Local Redundancy & Over-Extraction Audit:\n\
            - Identify duplicate, near-identical, or redundant fact strings extracted within this sub-batch.\n\n\
         4. Collection Disambiguation & Schema Placement:\n\
            - Audit placement across Identity, Directives, Profile, Entities, Constraints, and Narrative.\n\
            - Flag misclassified facts (e.g., general preferences wrongly placed in Identity or soft preferences placed in Constraints).\n\n\
         5. Precision & Hallucination Check:\n\
            - Identify any extracted statements that are false, unstated, or hallucinated relative to raw_dialogue.\n\n\
         Format your output ONLY as a clean Markdown report starting with '# Eval 1 Sub-Batch {:02} Compaction Audit Report (Turns {}-{})'.\n\
         </task>\n\
         </judge_subbatch_evaluation>",
        subbatch_num, turns_start, turns_end, raw_dialogue, extracted_facts_json, turns_start, turns_end, turns_start, turns_end, subbatch_num, turns_start, turns_end
    );

    let messages = serde_json::json!([
        {"role": "user", "content": judge_prompt}
    ]);

    post_chat_completion(client, api_key, "llama3.1:8b", messages, 2500).await
}

async fn run_llm_compaction_master_synthesis(
    client: &reqwest::Client,
    api_key: &str,
    subbatch_reports: &[String],
    full_extracted_facts_json: &str,
) -> Result<String> {
    let mut combined_subbatch_reports = String::new();
    for (idx, r) in subbatch_reports.iter().enumerate() {
        combined_subbatch_reports.push_str(&format!(
            "<subbatch_report num=\"{:02}\">\n{}\n</subbatch_report>\n\n",
            idx + 1,
            r
        ));
    }

    let synthesis_prompt = format!(
        "<judge_master_compaction_synthesis>\n\
         <subbatch_reports>\n{}\n</subbatch_reports>\n\n\
         <full_extracted_facts>\n{}\n</full_extracted_facts>\n\n\
         <task>\n\
         Act as a Principal AI Systems Architect. Synthesize the sub-batch evaluation reports and full extracted facts above into a Master Compaction Evaluation Report.\n\
         Evaluate the compaction engine across the following 6 unified sections:\n\
         1. Executive Summary & Score Breakdown (Overall Compaction Score out of 100, Fact Quality %, Information Coverage %, Redundancy %, Schema Disambiguation %, Precision %).\n\
         2. Fact Quality & Bare-Entity Audit (Synthesize occurrences of low-quality single-word or bare-entity extractions vs self-contained declarative statements).\n\
         3. Information Coverage & Context Retention Analysis (Synthesize recall coverage, detail preservation, numbers/amounts retention, and silent context drops).\n\
         4. Cross-Window Redundancy & Over-Extraction Audit (Identify facts extracted repeatedly across sliding context windows).\n\
         5. Collection Disambiguation & Category Correctness (Audit placement across Identity, Directives, Profile, Entities, Constraints, Narrative).\n\
         6. Actionable Engineering Recommendations (Concrete prompt, schema boundary, or token windowing recommendations).\n\n\
         Format output ONLY as clean Markdown starting with '# Eval 1 Compaction Master Evaluation Report'.\n\
         </task>\n\
         </judge_master_compaction_synthesis>",
        combined_subbatch_reports, full_extracted_facts_json
    );

    let messages = serde_json::json!([
        {"role": "user", "content": synthesis_prompt}
    ]);

    post_chat_completion(client, api_key, "gemma4:e4b", messages, 3000).await
}


#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Vox Memory Subsystem: Ladder Eval 1 (Real Multi-Window LLM Compaction) ===");

    let api_key = get_nvidia_api_key();
    if api_key.is_empty() {
        return Err(anyhow!(
            "NVIDIA_API_KEY not found in environment or temp/.env. Cannot run real LLM compaction."
        ));
    }

    let cli_arg = std::env::args().nth(1);
    let dataset_filename = cli_arg
        .map(|a| a.trim_start_matches("--dataset=").trim_start_matches("--dataset").to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            std::env::var("EVAL_DATASET_NAME").unwrap_or_else(|_| "dataset_session_1.json".to_string())
        });
    let dataset_path = resolve_path(&format!("evals/datasets/{}", dataset_filename));
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

    println!(
        "[Eval 1] Loaded {} turns from {:?}",
        turns.len(),
        dataset_path
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    // Chunk 300 turns into 30-turn windows (10 windows total)
    let window_size = 30;
    let chunks: Vec<&[ConversationTurn]> = turns.chunks(window_size).collect();
    println!(
        "[Eval 1] Divided 300 turns into {} sliding context windows ({} turns/window)",
        chunks.len(),
        window_size
    );

    let mut accumulated_facts: HashMap<String, Vec<String>> = HashMap::new();
    let mut chunk_facts_map: Vec<HashMap<String, Vec<String>>> = Vec::new();

    for (idx, chunk) in chunks.iter().enumerate() {
        match extract_facts_via_nvidia_llm(&client, &api_key, chunk, idx).await {
            Ok(facts) => {
                chunk_facts_map.push(facts.clone());
                for (col, fact_list) in facts {
                    accumulated_facts.entry(col).or_default().extend(fact_list);
                }
            }
            Err(e) => {
                println!("[Eval 1 Chunk {} Error] {}", idx + 1, e);
                chunk_facts_map.push(HashMap::new());
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

    println!(
        "\n[Eval 1] Real LLM Fact Extraction Completed. Extracted {} total facts.",
        total_extracted
    );
    for (col, count) in &facts_by_collection {
        println!("  - Collection {:<12}: {} facts", col, count);
    }

    // Enqueue all extracted facts into personal_memory_queue in stage_1_compaction.db
    enqueue_personal_facts(&conn, accumulated_facts.clone(), "session_eval_1", true).await?;

    let mut count_row = conn
        .query("SELECT COUNT(*) FROM personal_memory_queue", ())
        .await?;
    let total_enqueued: i64 = count_row
        .next()
        .await?
        .ok_or_else(|| anyhow::anyhow!("Failed to query personal_memory_queue count"))?
        .get(0)?;

    println!(
        "[Eval 1] Enqueued {} facts into personal_memory_queue at {:?}",
        total_enqueued, output_db_path
    );

    let _ = conn.execute("PRAGMA wal_checkpoint(FULL);", ()).await;
    let staged_db_path = resolve_path("evals/results/stage_1_compaction_staged.db");
    let _ = std::fs::copy(&output_db_path, &staged_db_path);

    // =========================================================================
    // Hierarchical LLM Judge Phase: 3 Sub-Batch Reports + 1 Master Synthesis Report
    // =========================================================================
    println!("\n[Eval 1] Initiating Hierarchical LLM Judge Evaluation (3 Sub-Batches + Master Synthesis)...");

    let reports_dir = resolve_path("evals/results/reports");
    std::fs::create_dir_all(&reports_dir)?;

    // Define 3 sub-batch chunk ranges: Chunks 0..3 (Turns 1-90), Chunks 3..6 (Turns 91-180), Chunks 6..10 (Turns 181-300)
    let subbatch_ranges = vec![(1, 0..3), (2, 3..6), (3, 6..chunks.len())];

    let mut subbatch_report_contents = Vec::new();

    for (sb_num, chunk_range) in subbatch_ranges {
        let mut sb_dialogue = String::new();
        let mut sb_facts: HashMap<String, Vec<String>> = HashMap::new();

        let mut start_turn = u32::MAX;
        let mut end_turn = 0;

        for chunk_i in chunk_range.clone() {
            if let Some(c) = chunks.get(chunk_i) {
                for t in *c {
                    if t.turn < start_turn {
                        start_turn = t.turn;
                    }
                    if t.turn > end_turn {
                        end_turn = t.turn;
                    }
                    sb_dialogue.push_str(&format!(
                        "Turn {}: User: {} | Asst: {}\n",
                        t.turn, t.user, t.assistant
                    ));
                }
            }
            if let Some(f_map) = chunk_facts_map.get(chunk_i) {
                for (col, f_list) in f_map {
                    sb_facts
                        .entry(col.clone())
                        .or_default()
                        .extend(f_list.clone());
                }
            }
        }

        let sb_facts_json = serde_json::to_string_pretty(&sb_facts)?;
        println!(
            "  Generating Sub-Batch {:02} Judge Report (Turns {}-{})...",
            sb_num, start_turn, end_turn
        );

        let sb_report = run_llm_subbatch_judge_report(
            &client,
            &api_key,
            sb_num,
            start_turn,
            end_turn,
            &sb_dialogue,
            &sb_facts_json,
        )
        .await?;

        let sb_report_path = reports_dir.join(format!("eval1_subbatch_{:02}_report.md", sb_num));
        std::fs::write(&sb_report_path, &sb_report)?;
        println!(
            "    Saved Sub-Batch Report {:02} To: {:?}",
            sb_num, sb_report_path
        );
        subbatch_report_contents.push(sb_report);
    }

    println!("\n[Eval 1 Master Synthesis] Generating Master Compaction Evaluation Report...");
    let full_extracted_facts_json = serde_json::to_string_pretty(&accumulated_facts)?;
    let master_report = run_llm_compaction_master_synthesis(
        &client,
        &api_key,
        &subbatch_report_contents,
        &full_extracted_facts_json,
    )
    .await?;

    let master_report_path = resolve_path("evals/results/eval1_compaction_report.md");
    std::fs::write(&master_report_path, &master_report)?;

    let secondary_master_path = reports_dir.join("eval1_compaction_master_report.md");
    std::fs::write(&secondary_master_path, &master_report)?;

    println!("\n=========================================================================");
    println!("[Eval 1 Hierarchical LLM Judge Evaluation Complete]");
    println!("  Turns Processed           : {}", turns.len());
    println!("  Sub-Batch Reports Created : 3");
    println!("  Total Facts Extracted     : {}", total_extracted);
    println!("  Total Facts Enqueued in DB: {}", total_enqueued);
    println!("  Output Database Artifact  : {:?}", output_db_path);
    println!("  -----------------------------------------------------------------------");
    println!("  Master Judge Report Saved : {:?}", master_report_path);
    println!("=========================================================================\n");

    Ok(())
}
