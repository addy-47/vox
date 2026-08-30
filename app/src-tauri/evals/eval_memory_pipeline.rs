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
use std::sync::atomic::{AtomicBool, Ordering};

use std::sync::Arc;
use turso::Builder;
use vox_lib::core::constants::{is_valid_inter_collection_pair, PM_SEMANTIC_GRAPH_COLLECTIONS};
use vox_lib::persistence::{decode_f32_blob, encode_f32_blob, mutations};
use vox_lib::services::memory::ingestion::{CandidateAuditLog, DedupAuditLog};
use vox_lib::services::memory::ingestion::{
    run_stage1_dedup, run_stage2_embed, stage3_eval, stage4_commit,
};
use vox_lib::services::memory::{
    INTER_COLLECTION_CANDIDATE_SEARCH, SAME_COLLECTION_CANDIDATE_SEARCH, SUBFLOOR_CANDIDATE_FLOOR,
};

async fn fetch_intra_subfloor_candidates(
    conn: &turso::Connection,
    collection: &str,
    query_embedding: &[f32],
    floor_threshold: f32,
    ceil_threshold: f32,
    limit: Option<i64>,
) -> Result<Vec<(String, String, f32)>> {
    let query_blob = encode_f32_blob(query_embedding);

    let (query_str, params) = match limit {
        Some(lim) if lim > 0 => (
            "SELECT id, fact, sim FROM (
                SELECT mf.id as id, mf.fact as fact, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection = ? AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection = ? AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? AND sim < ? ORDER BY sim DESC LIMIT ?".to_string(),
            vec![
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Real(floor_threshold as f64),
                turso::Value::Real(ceil_threshold as f64),
                turso::Value::Integer(lim),
            ],
        ),
        _ => (
            "SELECT id, fact, sim FROM (
                SELECT mf.id as id, mf.fact as fact, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection = ? AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection = ? AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? AND sim < ? ORDER BY sim DESC".to_string(),
            vec![
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Blob(query_blob.clone()),
                turso::Value::Text(collection.to_string()),
                turso::Value::Real(floor_threshold as f64),
                turso::Value::Real(ceil_threshold as f64),
            ],
        ),
    };

    let mut cand_rows = conn.query(&query_str, params).await?;

    let mut candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        let sim: f64 = row.get(2)?;
        candidates.push((id, f_text, sim as f32));
    }
    Ok(candidates)
}

async fn fetch_inter_subfloor_candidates(
    conn: &turso::Connection,
    target_collections: &[&str],
    query_embedding: &[f32],
    floor_threshold: f32,
    ceil_threshold: f32,
    limit: Option<i64>,
) -> Result<Vec<(String, String, String, f32)>> {
    if target_collections.is_empty() {
        return Ok(Vec::new());
    }

    let query_blob = encode_f32_blob(query_embedding);
    let placeholders = vec!["?"; target_collections.len()].join(",");

    let has_limit = matches!(limit, Some(lim) if lim > 0);

    let query_str = if has_limit {
        format!(
            "SELECT id, fact, collection, sim FROM (
                SELECT mf.id as id, mf.fact as fact, mf.collection as collection, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection IN ({}) AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, q.collection as collection, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection IN ({}) AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? AND sim < ? ORDER BY sim DESC LIMIT ?",
            placeholders, placeholders
        )
    } else {
        format!(
            "SELECT id, fact, collection, sim FROM (
                SELECT mf.id as id, mf.fact as fact, mf.collection as collection, (1.0 - vector_distance_cos(mfv.embedding, ?)) as sim
                FROM memory_facts_vectors mfv
                JOIN memory_facts mf ON mf.id = mfv.fact_id
                WHERE mf.collection IN ({}) AND mf.status = 'active'
                
                UNION ALL
                
                SELECT printf('item_%d', q.id) as id, q.fact as fact, q.collection as collection, (1.0 - vector_distance_cos(q.vector, ?)) as sim
                FROM personal_memory_queue q
                WHERE q.collection IN ({}) AND q.status IN ('embedded', 'evaluated') AND q.vector IS NOT NULL
             ) WHERE sim >= ? AND sim < ? ORDER BY sim DESC",
            placeholders, placeholders
        )
    };

    let mut params: Vec<turso::Value> = Vec::new();
    params.push(turso::Value::Blob(query_blob.clone()));
    for col in target_collections {
        params.push(turso::Value::Text(col.to_string()));
    }
    params.push(turso::Value::Blob(query_blob.clone()));
    for col in target_collections {
        params.push(turso::Value::Text(col.to_string()));
    }
    params.push(turso::Value::Real(floor_threshold as f64));
    params.push(turso::Value::Real(ceil_threshold as f64));

    if let Some(lim) = limit {
        if lim > 0 {
            params.push(turso::Value::Integer(lim));
        }
    }

    let mut cand_rows = conn.query(&query_str, params).await?;

    let mut candidates = Vec::new();
    while let Some(row) = cand_rows.next().await? {
        let id: String = row.get(0)?;
        let f_text: String = row.get(1)?;
        let col: String = row.get(2)?;
        let sim: f64 = row.get(3)?;
        candidates.push((id, f_text, col, sim as f32));
    }
    Ok(candidates)
}

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

fn format_stage3_batch_toon(
    batch_num: usize,
    chunk: &[(i64, String, String, Vec<CandidateAuditLog>)],
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<stage3_batch_evaluation batch=\"{:02}\" fact_count=\"{}\">\n",
        batch_num,
        chunk.len()
    ));

    for (idx, (_item_id, item_fact, item_coll, logs)) in chunk.iter().enumerate() {
        let item_num = idx + 1;
        out.push_str(&format!(
            "\n[Fact {:02}/{:02}] [{}] {}\n",
            item_num,
            chunk.len(),
            item_coll,
            item_fact
        ));

        if logs.is_empty() {
            out.push_str("  Candidates Evaluated: None\n");
            continue;
        }

        out.push_str(&format!("  Candidates Evaluated ({}):\n", logs.len()));

        for (cand_idx, log) in logs.iter().enumerate() {
            let cand_num = cand_idx + 1;
            let engine_tag = match log.engine.as_str() {
                "NLI" => "NLI",
                "ModernBERT" => "ModernBERT",
                "subfloor" => "Subfloor",
                other => other,
            };

            out.push_str(&format!(
                "  {}. [{}] [{}] {}\n",
                cand_num, engine_tag, log.cand_collection, log.cand_fact
            ));

            let mut metrics = vec![
                format!("cos_sim: {:.3}", log.cosine_sim),
                format!("source: {}", log.candidate_source),
            ];

            if let Some([c, e, n]) = log.nli_scores {
                metrics.push(format!("logits: [c: {:.3}, e: {:.3}, n: {:.3}]", c, e, n));
            }

            if let Some(edge_sc) = log.edge_score {
                metrics.push(format!("edge_score: {:.3}", edge_sc));
            }

            metrics.push(format!("decision: {}", log.decision));

            if let Some(ref reason) = log.rejection_reason {
                metrics.push(format!("rejection_reason: {}", reason));
            }

            out.push_str(&format!("     {}\n", metrics.join(" | ")));
        }
    }

    out.push_str("</stage3_batch_evaluation>");
    out
}

const OLLAMA_GPU_SERVER_URL: &str = "http://100.86.62.14:11434/v1/chat/completions";

async fn run_llm_judge_prompt_with_model(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<String> {
    let payload = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.5,
        "max_tokens": 3000
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
        let fallback_model = if model.contains("gemma") {
            "google/gemma-2-27b-it"
        } else {
            "meta/llama-3.1-70b-instruct"
        };
        let fallback_payload = serde_json::json!({
            "model": fallback_model,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.5,
            "max_tokens": 3000
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

    Err(anyhow::anyhow!(
        "LLM Judge API call failed on both Ollama GPU server and Nvidia API"
    ))
}

async fn run_llm_judge_prompt(
    client: &reqwest::Client,
    api_key: &str,
    prompt: &str,
) -> Result<String> {
    run_llm_judge_prompt_with_model(client, api_key, "gemma4:e4b", prompt).await
}

#[tokio::main]

async fn main() -> Result<()> {
    println!("=== Vox Memory Subsystem: Ladder Eval 2 (4-Stage Pipeline & Ingestion) ===");

    let cli_arg = std::env::args().nth(1);
    let input_db_path = if let Some(ref db_path) = cli_arg {
        resolve_path(
            db_path
                .trim_start_matches("--db=")
                .trim_start_matches("--db"),
        )
    } else {
        let primary = resolve_path("evals/results/stage_1_compaction.db");
        if primary.exists() {
            primary
        } else {
            resolve_path("evals/results/stage_1_compaction_staged.db")
        }
    };
    let output_db_path = resolve_path("evals/results/stage_2_pipeline.db");
    let reports_dir = resolve_path("evals/results/reports");

    if !input_db_path.exists() {
        return Err(anyhow::anyhow!(
            "Input DB at {:?} not found. Please run eval_compaction first.",
            input_db_path
        ));
    }

    // Flush WAL pages on input DB before copying
    if let Ok(abs_in) = std::fs::canonicalize(&input_db_path) {
        if let Some(in_str) = abs_in.to_str() {
            if let Ok(db_in) = Builder::new_local(in_str).build().await {
                if let Ok(conn_in) = db_in.connect() {
                    let _ = conn_in.execute("PRAGMA wal_checkpoint(FULL);", ()).await;
                }
            }
        }
    }

    if output_db_path.exists() {
        let _ = std::fs::remove_file(&output_db_path);
        let _ = std::fs::remove_file(reports_dir.join("../stage_2_pipeline.db-wal"));
        let _ = std::fs::remove_file(reports_dir.join("../stage_2_pipeline.db-shm"));
    }

    let _ = std::fs::copy(&input_db_path, &output_db_path)?;

    let abs_db_path = std::fs::canonicalize(&output_db_path)?;
    let db_path_str = abs_db_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid output DB path {:?}", abs_db_path))?;
    let db = Builder::new_local(db_path_str).build().await?;
    let conn = db.connect()?;
    vox_lib::persistence::schema::run_migrations(&conn).await?;

    let run_id = uuid::Uuid::new_v4().to_string();
    println!(
        "[Eval 2] Running 4-stage memory pipeline (run_id={}) on {:?}",
        run_id, output_db_path
    );

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let start_time = std::time::Instant::now();

    let mut processed_count = 0;
    let mut stage3_seq = 0;
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }
        let n1 = run_stage1_dedup(&conn).await?;
        let n2 = run_stage2_embed(&conn).await?;
        let n3 = stage3_eval::run_stage3_eval_with_metrics_seq(
            &conn, &run_id, stage3_seq,
        )
        .await?;
        if n1 == 0 && n2 == 0 && n3 == 0 {
            break;
        }
        processed_count += n1 + n2 + n3;
        stage3_seq += 1;
    }
    let total_duration = start_time.elapsed();

    println!(
        "[Eval 2] Stages 1-3 execution complete. Processed {} items across stages in {:?}",
        processed_count, total_duration
    );

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
        subfloor_scan_items.push((
            id,
            fact,
            collection,
            decode_f32_blob(&vec_blob),
            audit_json_raw,
        ));
    }

    let mut total_subfloor_candidates_found = 0;
    for (item_id, item_fact, item_collection, vector, audit_json_raw) in subfloor_scan_items {
        let mut existing_logs: Vec<CandidateAuditLog> = audit_json_raw
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_default();

        let intra_subfloor = fetch_intra_subfloor_candidates(
            &conn,
            &item_collection,
            &vector,
            SUBFLOOR_CANDIDATE_FLOOR,
            SAME_COLLECTION_CANDIDATE_SEARCH,
            Some(5),
        )
        .await
        .unwrap_or_default();

        let policy_targets: Vec<&'static str> = PM_SEMANTIC_GRAPH_COLLECTIONS
            .iter()
            .copied()
            .filter(|&tgt| is_valid_inter_collection_pair(&item_collection, tgt))
            .collect();

        let inter_subfloor = if !policy_targets.is_empty() {
            fetch_inter_subfloor_candidates(
                &conn,
                &policy_targets,
                &vector,
                SUBFLOOR_CANDIDATE_FLOOR,
                INTER_COLLECTION_CANDIDATE_SEARCH,
                Some(5),
            )
            .await
            .unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut added_count = 0;
        for (cand_id, cand_fact, sim) in intra_subfloor {
            let cand_source = if cand_id.starts_with("item_") {
                "queue_in_flight".to_string()
            } else {
                "memory_facts".to_string()
            };
            existing_logs.push(CandidateAuditLog {
                item_id,
                item_fact: item_fact.clone(),
                item_collection: item_collection.clone(),
                cand_id,
                cand_fact,
                cand_collection: item_collection.clone(),
                candidate_source: cand_source,
                cosine_sim: sim,
                engine: "subfloor-intra".to_string(),
                nli_scores: None,
                edge_score: None,
                decision: "NONE".to_string(),
                rejection_reason: Some("below_intra_nli_search_threshold_0.60".to_string()),
            });
            added_count += 1;
        }

        for (cand_id, cand_fact, cand_coll, sim) in inter_subfloor {
            let cand_source = if cand_id.starts_with("item_") {
                "queue_in_flight".to_string()
            } else {
                "memory_facts".to_string()
            };
            existing_logs.push(CandidateAuditLog {
                item_id,
                item_fact: item_fact.clone(),
                item_collection: item_collection.clone(),
                cand_id,
                cand_fact,
                cand_collection: cand_coll,
                candidate_source: cand_source,
                cosine_sim: sim,
                engine: "subfloor-inter".to_string(),
                nli_scores: None,
                edge_score: None,
                decision: "NONE".to_string(),
                rejection_reason: Some("below_inter_edge_search_threshold_0.40".to_string()),
            });
            added_count += 1;
        }

        if added_count > 0 {
            total_subfloor_candidates_found += added_count;
            mutations::write_candidate_audit(&conn, item_id, &existing_logs).await?;
        }
    }
    println!(
        "  Identified & Logged {} sub-floor candidate pairs.",
        total_subfloor_candidates_found
    );

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

    let chunks = batch_items.chunks(16).collect::<Vec<_>>();
    println!(
        "  Total Stage 3 Evaluation Batches to process: {}",
        chunks.len()
    );
    let mut generated_batch_reports = Vec::new();

    for (batch_idx, chunk) in chunks.iter().enumerate() {
        let batch_num = batch_idx + 1;
        println!(
            "  Generating Judge Report for Stage 3 Batch {:02}/{}",
            batch_num,
            chunks.len()
        );

        let batch_toon = format_stage3_batch_toon(batch_num, chunk);
        let batch_prompt = format!(
            "{}\n\n\
            <evaluation_instructions>\n\
            You are a Principal AI Knowledge Graph & Memory Systems Architect performing a rigorous, evidence-anchored audit of Stage 3 candidate evaluations for Batch {:02}.\n\n\
            <input_structure_guide>\n\
            - The input above presents exactly 16 target facts (labeled `[Fact XX/16]`). Each target fact is the NEW incoming memory state (Hypothesis).\n\
            - Listed under each target fact are its evaluated candidates. Each candidate is an established historical DB record (Premise).\n\
            - Candidate tags: `[NLI]` (DeBERTa-v3 state transition evaluation), `[ModernBERT]` (cross-collection edge classification), `[Subfloor]` (near-miss candidates in 0.25 <= cos_sim < threshold).\n\
            </input_structure_guide>\n\n\
            <audit_thought_process>\n\
            For each of the 16 facts in this batch, execute the following systematic 4-step audit in your thought process before rendering your verdict:\n\n\
            Step 1: NLI State Transition & Contradiction Audit (`[NLI]` candidates)\n\
            - Check formed `SUPERSEDES` and `CONFLICTS` edges against DeBERTa-v3 logits (`c` = contradiction, `e` = entailment, `n` = neutral).\n\
            - Audit for FALSE POSITIVES: Did the pipeline mistakenly supersede an existing fact when the incoming fact was merely an additive update, task progression, or separate context?\n\
            - Audit for FALSE NEGATIVES: Did the pipeline fail to supersede or conflict an existing fact when the new fact clearly contradicted it?\n\
            - Check formed `SUPPORTS` edges against entailment score (threshold >= 0.85).\n\n\
            Step 2: Inter-Collection Graph Edge Audit (`[ModernBERT]` candidates)\n\
            - Verify relation classification (`SHAPES`, `restricted_by`, `DEPENDS_ON`) against inter-collection policy rules.\n\
            - Verify confidence scores (`edge_score` >= 0.80) to ensure high-confidence edge creation.\n\n\
            Step 3: Subfloor Near-Miss Analysis (`[Subfloor]` candidates)\n\
            - Examine candidates in the `0.25 <= cos_sim < search_threshold` window.\n\
            - Determine if any vital semantic contradiction or relationship was missed purely due to similarity floor cutoff.\n\n\
            Step 4: Synthesis & Scorecard\n\
            - Score this batch from 0 to 10 based strictly on semantic precision and graph integrity.\n\
            </audit_thought_process>\n\n\
            <report_format_requirements>\n\
            Format your response as clean, professional Markdown starting with '# Stage 3 Batch {:02} Evaluation & Audit Report' using the following exact sections:\n\n\
            # Stage 3 Batch {:02} Evaluation & Audit Report\n\n\
            ## 1. Executive Summary & Batch Scorecard\n\
            - Overall Batch Score: X/10\n\
            - Total Facts Audited: 16\n\
            - Key Operational Observations\n\n\
            ## 2. NLI Intra-Collection State Transition Audit\n\
            - Detailed analysis of formed `SUPERSEDES`, `CONFLICTS`, and `SUPPORTS` edges.\n\
            - Highlight specific false positive or false negative state resolutions with exact fact text.\n\n\
            ## 3. ModernBERT Inter-Collection Edge Audit\n\
            - Audit of cross-collection graph relationships and confidence score calibration.\n\n\
            ## 4. Subfloor Near-Miss Analysis\n\
            - Audit of near-miss candidate pairs in the 0.25-0.40 range.\n\n\
            ## 5. Actionable Engineering Recommendations\n\
            - Concrete logic or threshold adjustments indicated by evidence in this batch.\n\
            </report_format_requirements>",
            batch_toon,
            batch_num,
            batch_num,
            batch_num
        );

        let debug_log_path =
            reports_dir.join(format!("stage3_batch_{:02}_raw_vs_toon.log", batch_num));

        let mut debug_content = String::new();
        debug_content.push_str(
            "=========================================================================\n",
        );
        debug_content.push_str(&format!(
            "STAGE 3 BATCH {:02} RAW INPUT vs TOON FORMAT AUDIT LOG\n",
            batch_num
        ));
        debug_content.push_str(
            "=========================================================================\n\n",
        );
        debug_content.push_str("--- SECTION 1: RAW CandidateAuditLog STRUCTS (JSON) ---\n");
        for (item_id, item_fact, item_coll, logs) in chunk.iter() {
            debug_content.push_str(&format!(
                "\nQueue Item #{} [{}] {}\n",
                item_id, item_coll, item_fact
            ));
            debug_content.push_str(&serde_json::to_string_pretty(logs).unwrap_or_default());
            debug_content.push('\n');
        }
        debug_content.push_str("\n\n--- SECTION 2: FORMATTED TOON STRING ---\n");
        debug_content.push_str(&batch_toon);
        debug_content.push_str("\n\n--- SECTION 3: FULL PROMPT SENT TO LLM JUDGE ---\n");
        debug_content.push_str(&batch_prompt);

        let _ = std::fs::write(&debug_log_path, &debug_content);

        let report_b_path = reports_dir.join(format!("stage3_batch_{:02}_report.md", batch_num));

        match run_llm_judge_prompt(&client, &api_key, &batch_prompt).await {
            Ok(content) => {
                std::fs::write(&report_b_path, &content)?;
                println!(
                    "    Saved Batch Report {:02} To: {:?}",
                    batch_num, report_b_path
                );
                generated_batch_reports.push(content);
            }
            Err(e) => println!("    [Batch {:02} Warning] Failed: {}", batch_num, e),
        }
    }

    // =========================================================================
    // Phase D: Master Report C — LLM Synthesis across Stages 1-3 & Batch Reports
    // =========================================================================
    println!("\n[Report C] Initiating Master Synthesis LLM Judge Call across Stages 1-3...");

    let dedup_report_text = std::fs::read_to_string(&report_a_path).unwrap_or_default();
    let mut combined_batch_reports = String::new();
    for (idx, r) in generated_batch_reports.iter().enumerate() {
        combined_batch_reports.push_str(&format!(
            "<stage3_batch_report num=\"{:02}\">\n{}\n</stage3_batch_report>\n\n",
            idx + 1,
            r
        ));
    }

    let report_c_prompt = format!(
        "<master_pipeline_synthesis_audit>\n\
         <stage1_stage2_dedup_report>\n{}\n</stage1_stage2_dedup_report>\n\n\
         <stage3_batch_reports>\n{}\n</stage3_batch_reports>\n\n\
         <task>\n\
         Act as a Principal AI Memory Systems Architect. Synthesize the Stage 1 & 2 Deduplication Report and all Stage 3 Batch Reports into a unified Master Ingestion Pipeline Audit Report (Report C).\n\
         Evaluate the pipeline across the following 6 core pillars:\n\
         1. Overall Pipeline Assessment & Scorecard (Overall Score out of 10.0, Stage 1-2 Sub-Score, Stage 3 Sub-Score).\n\
         2. Deduplication & Merging Semantic Audit (Assess Jaccard exact match priority resolution and Soft Vector dedup precision).\n\
         3. Stage 3 NLI State Resolution Precision (Synthesize false positive and false negative state transition rates across all batches).\n\
         4. ModernBERT Inter-Collection Edge Calibration (Assess cross-collection relation classification and confidence score calibration).\n\
         5. Subfloor Cutoff Analysis (Synthesize near-miss candidate findings in the 0.25-0.40 range).\n\
         6. Actionable Engineering Recommendations (Concrete logic, threshold, or model fine-tuning recommendations).\n\n\
         Format output ONLY as clean Markdown starting with '# Stage 3 Master Pipeline Evaluation & Audit Report (Report C)'.\n\
         </task>\n\
         </master_pipeline_synthesis_audit>",
        dedup_report_text, combined_batch_reports
    );

    let report_c_path = reports_dir.join("stage3_master_report_c.md");
    match run_llm_judge_prompt_with_model(
        &client,
        &api_key,
        "meta/llama-3.1-70b-instruct",
        &report_c_prompt,
    )
    .await
    {
        Ok(content) => {
            std::fs::write(&report_c_path, &content)?;
            println!(
                "  [Report C] Saved Master Synthesis Report To: {:?}",
                report_c_path
            );
        }
        Err(e) => println!("  [Report C Error] Master synthesis failed: {}", e),
    }

    // Run Stage 4 Commit & Prune post-reporting
    let n4 = stage4_commit::run_stage4_commit_with_metrics(
        &conn, &run_id,
    )
    .await?;
    println!(
        "  [Stage 4 Commit] Finalized and pruned {} facts into memory_facts DB.",
        n4
    );

    println!("\n=========================================================================");
    println!("[Eval 2 Execution Complete]");
    println!("  Total Items Processed : {}", processed_count);
    println!("  Execution Time        : {:?}", total_duration);

    println!("  Batch Reports Saved To: {:?}", reports_dir);
    println!("=========================================================================\n");

    Ok(())
}
