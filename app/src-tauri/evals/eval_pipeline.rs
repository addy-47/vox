//! ============================================================================
//! eval_pipeline.rs — Ladder Eval 2: 4-Stage Ingestion Pipeline Evaluation
//! ============================================================================
//! Category     : Evaluation Script
//! Component    : services::memory / evals
//! Prerequisites: evals/results/stage_1_compaction.db (or JSON input)
//! Execution    : cargo run --release --example eval_pipeline
//! Metrics      : Stage Latencies (ms), Dedup Audit, Stage 3 Per-Batch Audit, Master Synthesis
//! ============================================================================

use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use turso::Builder;
use vox_lib::core::constants::{
    inter_collection_edge, PM_SEMANTIC_GRAPH_COLLECTIONS,
};
use vox_lib::persistence::{decode_f32_blob, mutations, queries};
use vox_lib::services::memory::pipeline::batch_result::{CandidateAuditLog, DedupAuditLog};
use vox_lib::services::memory::pipeline::stage3_eval::{
    INTER_COLLECTION_CANDIDATE_SEARCH, SAME_COLLECTION_CANDIDATE_SEARCH, SUBFLOOR_CANDIDATE_FLOOR,
};
use vox_lib::services::memory::pipeline::drain_pipeline_queue_with_run_id;

pub const EVAL_JUDGE_MODEL: &str = "meta/llama-3.1-70b-instruct";
pub const EVAL_JUDGE_TIMEOUT_SECS: u64 = 300;
pub const EVAL_JUDGE_RETRY_COUNT: usize = 3;

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

async fn run_llm_judge_prompt(
    client: &reqwest::Client,
    api_key: &str,
    prompt: &str,
) -> Result<String> {
    let payload = serde_json::json!({
        "model": EVAL_JUDGE_MODEL,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.2,
        "max_tokens": 3000
    });

    let mut last_err = anyhow::anyhow!("Unknown error");

    for attempt in 1..=EVAL_JUDGE_RETRY_COUNT {
        match client
            .post("https://integrate.api.nvidia.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let json_body: serde_json::Value = resp.json().await?;
                    let content = json_body["choices"][0]["message"]["content"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Invalid response structure from Nvidia API"))?;

                    let cleaned = content
                        .trim()
                        .trim_start_matches("```markdown")
                        .trim_start_matches("```")
                        .trim_end_matches("```")
                        .trim();

                    return Ok(cleaned.to_string());
                } else {
                    let err_text = resp.text().await.unwrap_or_default();
                    last_err = anyhow::anyhow!("Nvidia API returned status {}: {}", status, err_text);
                }
            }
            Err(e) => {
                last_err = anyhow::anyhow!("Request to Nvidia API failed: {}", e);
            }
        }

        if attempt < EVAL_JUDGE_RETRY_COUNT {
            println!("  [LLM Judge] Attempt {} failed ({}). Retrying in 5s...", attempt, last_err);
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    Err(anyhow::anyhow!("LLM Judge API failed after {} attempts. Last error: {}", EVAL_JUDGE_RETRY_COUNT, last_err))
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Vox Memory Subsystem: Ladder Eval 2 (4-Stage Pipeline & Ingestion) ===");

    let input_db_path = resolve_path("evals/results/stage_1_compaction.db");
    let output_db_path = resolve_path("evals/results/stage_2_pipeline.db");
    let reports_dir = resolve_path("evals/results/reports");

    if !input_db_path.exists() {
        return Err(anyhow::anyhow!(
            "Input DB at {:?} not found. Please run eval_compaction first.",
            input_db_path
        ));
    }

    if let Some(parent) = output_db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir_all(&reports_dir)?;

    let _ = std::fs::copy(&input_db_path, &output_db_path)?;

    let abs_db_path = std::fs::canonicalize(&output_db_path)?;
    let db_path_str = abs_db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid output DB path {:?}", abs_db_path))?;
    let db = Builder::new_local(db_path_str).build().await?;
    let conn = db.connect()?;

    let run_id = uuid::Uuid::new_v4().to_string();
    println!("[Eval 2] Running 4-stage memory pipeline (run_id={}) on {:?}", run_id, output_db_path);

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let start_time = std::time::Instant::now();

    let processed_count = drain_pipeline_queue_with_run_id(&conn, &cancel_flag, &run_id).await?;
    let total_duration = start_time.elapsed();

    println!("[Eval 2] Pipeline execution complete. Processed {} queue items in {:?}", processed_count, total_duration);

    // =========================================================================
    // Phase A: Post-Pipeline Sub-Floor Candidate Audit Pass (0.25 <= sim < threshold)
    // =========================================================================
    println!("\n[Eval 2 Audit Pass] Scanning sub-floor candidates (0.25 <= sim < threshold)...");
    let mut eval_rows = conn
        .query(
            "SELECT id, fact, collection, vector, audit_json FROM personal_memory_queue
             WHERE status = 'evaluated' AND vector IS NOT NULL",
            (),
        )
        .await?;

    let mut subfloor_scan_items = Vec::new();
    while let Some(row) = eval_rows.next().await? {
        let id: i64 = row.get(0)?;
        let fact: String = row.get(1)?;
        let collection: String = row.get(2)?;
        let vec_blob: Vec<u8> = row.get(3)?;
        let audit_json_raw: Option<String> = row.get(4)?;
        subfloor_scan_items.push((id, fact, collection, decode_f32_blob(&vec_blob), audit_json_raw));
    }

    let mut total_subfloor_candidates_found = 0;
    for (item_id, item_fact, item_collection, vector, audit_json_raw) in subfloor_scan_items {
        let mut existing_logs: Vec<CandidateAuditLog> = audit_json_raw
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_default();

        let intra_subfloor = queries::fetch_intra_subfloor_candidates(
            &conn,
            &item_collection,
            &vector,
            SUBFLOOR_CANDIDATE_FLOOR,
            SAME_COLLECTION_CANDIDATE_SEARCH,
            None,
        )
        .await
        .unwrap_or_default();

        let policy_targets: Vec<&'static str> = PM_SEMANTIC_GRAPH_COLLECTIONS
            .iter()
            .copied()
            .filter(|&tgt| inter_collection_edge(&item_collection, tgt).is_some())
            .collect();

        let inter_subfloor = if !policy_targets.is_empty() {
            queries::fetch_inter_subfloor_candidates(
                &conn,
                &policy_targets,
                &vector,
                SUBFLOOR_CANDIDATE_FLOOR,
                INTER_COLLECTION_CANDIDATE_SEARCH,
                None,
            )
            .await
            .unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut added_count = 0;
        for (cand_id, cand_fact, sim) in intra_subfloor {
            let cand_source = if cand_id.starts_with("item_") { "queue_in_flight".to_string() } else { "memory_facts".to_string() };
            existing_logs.push(CandidateAuditLog {
                item_id,
                item_fact: item_fact.clone(),
                item_collection: item_collection.clone(),
                cand_id,
                cand_fact,
                cand_collection: item_collection.clone(),
                candidate_source: cand_source,
                cosine_sim: sim,
                engine: "subfloor".to_string(),
                nli_scores: None,
                edge_score: None,
                decision: "NONE".to_string(),
                rejection_reason: Some("below_search_threshold".to_string()),
            });
            added_count += 1;
        }

        for (cand_id, cand_fact, cand_coll, sim) in inter_subfloor {
            let cand_source = if cand_id.starts_with("item_") { "queue_in_flight".to_string() } else { "memory_facts".to_string() };
            existing_logs.push(CandidateAuditLog {
                item_id,
                item_fact: item_fact.clone(),
                item_collection: item_collection.clone(),
                cand_id,
                cand_fact,
                cand_collection: cand_coll,
                candidate_source: cand_source,
                cosine_sim: sim,
                engine: "subfloor".to_string(),
                nli_scores: None,
                edge_score: None,
                decision: "NONE".to_string(),
                rejection_reason: Some("below_search_threshold".to_string()),
            });
            added_count += 1;
        }

        if added_count > 0 {
            total_subfloor_candidates_found += added_count;
            mutations::write_candidate_audit(&conn, item_id, &existing_logs).await?;
        }
    }
    println!("  Identified & Logged {} sub-floor candidate pairs.", total_subfloor_candidates_found);

    // =========================================================================
    // Stage Observability & Metrics Summary
    // =========================================================================
    let mut metrics_rows = conn
        .query(
            "SELECT stage_name, batch_seq, items_claimed, error_count, duration_ms 
             FROM memory_pipeline_metrics WHERE run_id = ? ORDER BY id ASC",
            (run_id.clone(),),
        )
        .await?;

    let mut stage_metrics_list = Vec::new();
    println!("\n--- Operational Pipeline Stage Metrics ---");
    while let Some(row) = metrics_rows.next().await? {
        let stage: String = row.get(0)?;
        let batch_seq: i64 = row.get(1)?;
        let claimed: i64 = row.get(2)?;
        let errs: i64 = row.get(3)?;
        let duration: i64 = row.get(4)?;
        println!(
            "Stage: {:<15} | BatchSeq: {:<2} | Claimed: {:<3} | Errors: {:<3} | Duration: {} ms",
            stage, batch_seq, claimed, errs, duration
        );
        stage_metrics_list.push(serde_json::json!({
            "stage_name": stage,
            "batch_seq": batch_seq,
            "items_claimed": claimed,
            "error_count": errs,
            "duration_ms": duration,
        }));
    }

    let api_key = get_nvidia_api_key();
    if api_key.is_empty() {
        println!("\n[Warning] NVIDIA_API_KEY missing. Skipping LLM judge report generation.");
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(EVAL_JUDGE_TIMEOUT_SECS))
        .build()?;

    // =========================================================================
    // Phase B: Report A — Stage 1 & Stage 2 Deduplication Judge Report
    // =========================================================================
    println!("\n[Report A] Generating Stage 1 & Stage 2 Deduplication Audit Report...");
    let mut dedup_rows = conn
        .query(
            "SELECT id, fact, collection, dedup_match_json FROM personal_memory_queue WHERE dedup_match_json IS NOT NULL ORDER BY id ASC",
            (),
        )
        .await?;

    let mut s1_logs = Vec::new();
    let mut s2_logs = Vec::new();

    while let Some(row) = dedup_rows.next().await? {
        let raw_json: String = row.get(3)?;
        if let Ok(log) = serde_json::from_str::<DedupAuditLog>(&raw_json) {
            if log.stage == "stage1_jaccard" {
                s1_logs.push(log);
            } else if log.stage == "stage2_soft_vector" {
                s2_logs.push(log);
            }
        }
    }

    let report_a_prompt = format!(
        "<dedup_audit_evaluation>\n\
         <stage1_jaccard_entries>\n{}\n</stage1_jaccard_entries>\n\n\
         <stage2_soft_vector_entries>\n{}\n</stage2_soft_vector_entries>\n\n\
         <task>\n\
         Act as a Senior AI Knowledge Graph Judge auditing deduplication correctness.\n\
         Evaluate the captured Stage 1 (Jaccard Exact/Sub-word) and Stage 2 (Soft Cosine Vector) deduplication events:\n\
         1. Stage 1 Exact Match Audit: Were the dropped facts genuinely identical/redundant to matched facts?\n\
         2. Stage 2 Soft Vector Audit: Audit the soft-vector merges (cosine >= 0.95). Did priority resolution correctly handle incoming vs existing facts, or were distinct facts accidentally destroyed?\n\
         3. Summary Scorecard & Deduplication Precision Score (0-10).\n\n\
         Format as clean Markdown starting with '# Stage 1 & Stage 2 Deduplication Audit Report'.\n\
         </task>\n\
         </dedup_audit_evaluation>",
        serde_json::to_string_pretty(&s1_logs)?,
        serde_json::to_string_pretty(&s2_logs)?
    );

    let report_a_path = reports_dir.join("stage1_stage2_dedup_report.md");
    match run_llm_judge_prompt(&client, &api_key, &report_a_prompt).await {
        Ok(content) => {
            std::fs::write(&report_a_path, &content)?;
            println!("  Saved Report A To: {:?}", report_a_path);
        }
        Err(e) => println!("  [Report A Warning] Failed: {}", e),
    }

    // =========================================================================
    // Phase C: Report B — Per-Batch Stage 3 Evaluation Judge Reports
    // =========================================================================
    println!("\n[Report B] Generating Per-Batch Stage 3 Judge Reports...");
    let mut eval_queue_rows = conn
        .query(
            "SELECT id, fact, collection, audit_json FROM personal_memory_queue WHERE audit_json IS NOT NULL ORDER BY id ASC",
            (),
        )
        .await?;

    let mut batch_items: Vec<(i64, String, String, Vec<CandidateAuditLog>)> = Vec::new();
    while let Some(row) = eval_queue_rows.next().await? {
        let id: i64 = row.get(0)?;
        let fact: String = row.get(1)?;
        let collection: String = row.get(2)?;
        let raw_json: String = row.get(3)?;
        if let Ok(logs) = serde_json::from_str::<Vec<CandidateAuditLog>>(&raw_json) {
            batch_items.push((id, fact, collection, logs));
        }
    }

    let chunks: Vec<&[(i64, String, String, Vec<CandidateAuditLog>)]> = batch_items.chunks(16).collect();
    println!("  Total Stage 3 Evaluation Batches to process: {}", chunks.len());

    for (batch_idx, chunk) in chunks.iter().enumerate() {
        let batch_num = batch_idx + 1;
        println!("  Generating Judge Report for Stage 3 Batch {:02}/{}", batch_num, chunks.len());

        let mut nli_pairs = Vec::new();
        let mut edge_pairs = Vec::new();
        let mut subfloor_pairs = Vec::new();

        for (item_id, item_fact, item_coll, logs) in chunk.iter() {
            for log in logs {
                let entry = serde_json::json!({
                    "item_id": item_id,
                    "item_fact": format!("[{}] {}", item_coll, item_fact),
                    "cand_id": log.cand_id,
                    "cand_fact": format!("[{}] {}", log.cand_collection, log.cand_fact),
                    "candidate_source": log.candidate_source,
                    "cosine_sim": log.cosine_sim,
                    "decision": log.decision,
                    "rejection_reason": log.rejection_reason,
                    "nli_scores": log.nli_scores,
                    "edge_score": log.edge_score,
                });

                match log.engine.as_str() {
                    "NLI" => nli_pairs.push(entry),
                    "ModernBERT" => edge_pairs.push(entry),
                    "subfloor" => subfloor_pairs.push(entry),
                    _ => {}
                }
            }
        }

        let batch_prompt = format!(
            "<stage3_batch_evaluation batch=\"{}\">\n\
             <nli_intra_collection_pairs>\n{}\n</nli_intra_collection_pairs>\n\n\
             <modernbert_inter_collection_pairs>\n{}\n</modernbert_inter_collection_pairs>\n\n\
             <subfloor_near_miss_pairs>\n{}\n</subfloor_near_miss_pairs>\n\n\
             <task>\n\
             Act as a Senior AI Systems & Knowledge Graph Judge auditing Stage 3 candidate evaluations for Batch {:02}.\n\
             Evaluate the following three distinct candidate categories independently:\n\
             1. NLI Intra-Collection Analysis: Evaluate formed SUPERSEDES/CONFLICTS/SUPPORTS edges vs DeBERTa-v3 probabilities. Identify any false positive relations or invalid topic overlap rejections.\n\
             2. ModernBERT Inter-Collection Analysis: Evaluate cross-collection graph relation predictions vs ModernBERT scores. Verify edge policy alignment.\n\
             3. Sub-Floor Near-Miss Audit: Check candidate pairs in the 0.25 <= sim < threshold window. Were any valid semantic relations missed due to vector similarity floor cutoff?\n\
             4. Batch Scorecard (0-10) and Actionable Findings.\n\n\
             Format as clean Markdown starting with '# Stage 3 Batch {:02} Evaluation & Audit Report'.\n\
             </task>\n\
             </stage3_batch_evaluation>",
            batch_num,
            serde_json::to_string_pretty(&nli_pairs)?,
            serde_json::to_string_pretty(&edge_pairs)?,
            serde_json::to_string_pretty(&subfloor_pairs)?,
            batch_num,
            batch_num
        );

        let report_b_path = reports_dir.join(format!("stage3_batch_{:02}_report.md", batch_num));
        match run_llm_judge_prompt(&client, &api_key, &batch_prompt).await {
            Ok(content) => {
                std::fs::write(&report_b_path, &content)?;
                println!("    Saved Batch Report {:02} To: {:?}", batch_num, report_b_path);
            }
            Err(e) => println!("    [Batch {:02} Warning] Failed: {}", batch_num, e),
        }
    }

    // =========================================================================
    // Phase D: Report C — Reserved for QA Subagent Synthesis
    // =========================================================================
    println!("\n[Report C] Individual batch reports generated in {:?}", reports_dir);
    println!("  Master Synthesis Report (Report C) is reserved for QA Subagent review.");

    println!("\n=========================================================================");
    println!("[Eval 2 Execution Complete]");
    println!("  Total Items Processed : {}", processed_count);
    println!("  Execution Time        : {:?}", total_duration);
    println!("  Batch Reports Saved To: {:?}", reports_dir);
    println!("=========================================================================\n");

    Ok(())
}
