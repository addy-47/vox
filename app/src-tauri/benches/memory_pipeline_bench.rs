//! ============================================================================
//! memory_pipeline_bench.rs — Compaction & 4-Stage Ingestion Pipeline Benchmark
//! Model: meta/llama-3.1-8b-instruct via Nvidia API
//! ============================================================================
//! Category     : Benchmark
//! Component    : services::memory / pipeline / benches
//! Prerequisites: NVIDIA_API_KEY (or temp/.env)
//! Execution    : cargo run --release --bench memory_pipeline_bench -- [options]
//! Metrics      : Compaction Latency, Facts Breakdown per Collection, 4-Stage Pipeline Latency,
//!                Throughput (items/sec), Total Facts by Collection, Total Edges by Type,
//!                Cross-Collection Directed Edge Pair Matrix (from_col -> to_col)
//! ============================================================================

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use turso::Builder;

use vox_lib::core::constants::COMPACTION_SYSTEM_PROMPT;
use vox_lib::persistence::mutations::enqueue_personal_facts;
use vox_lib::persistence::schema::run_migrations;
use vox_lib::services::memory::ensure_embedder_loaded;
use vox_lib::services::memory::pipeline::stage1_dedup::run_stage1_dedup_with_metrics;
use vox_lib::services::memory::pipeline::stage2_embed::run_stage2_embed_with_metrics;
use vox_lib::services::memory::pipeline::stage3_eval::run_stage3_eval_with_metrics_seq;
use vox_lib::services::memory::pipeline::stage4_commit::run_stage4_commit_with_metrics;
use vox_lib::utils::json::parse_compaction_json;

pub const NVIDIA_JUDGE_MODEL: &str = "meta/llama-3.1-8b-instruct";
pub const NVIDIA_API_URL: &str = "https://integrate.api.nvidia.com/v1/chat/completions";

#[derive(Debug, Deserialize)]
struct ConversationTurn {
    turn: u32,
    user: String,
    assistant: String,
}

#[derive(Debug, Serialize)]
struct WindowCompactionMetric {
    window_idx: usize,
    turn_start: u32,
    turn_end: u32,
    duration_ms: u128,
    facts_extracted: usize,
    facts_per_collection: HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct CompactionSummary {
    total_windows: usize,
    total_duration_ms: u128,
    avg_window_duration_ms: f64,
    total_facts_extracted: usize,
    facts_by_collection: HashMap<String, usize>,
    window_metrics: Vec<WindowCompactionMetric>,
}

#[derive(Debug, Serialize)]
struct StageMetric {
    duration_ms: u128,
    items_processed: usize,
    items_modified: usize,
    throughput_items_per_sec: f64,
}

#[derive(Debug, Serialize)]
struct PipelineSummary {
    stage1_dedup: StageMetric,
    stage2_embed: StageMetric,
    stage3_eval: StageMetric,
    stage4_commit: StageMetric,
    total_pipeline_duration_ms: u128,
}

#[derive(Debug, Serialize)]
struct EdgeMatrixEntry {
    from_collection: String,
    to_collection: String,
    relation: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct KnowledgeBaseSummary {
    total_active_facts: usize,
    facts_by_collection: HashMap<String, usize>,
    total_relations: usize,
    edges_by_type: HashMap<String, usize>,
    cross_collection_edge_matrix: Vec<EdgeMatrixEntry>,
}

#[derive(Debug, Serialize)]
struct PipelineBenchReport {
    benchmark_name: String,
    timestamp: String,
    dataset_path: String,
    total_turns: usize,
    compaction_summary: CompactionSummary,
    pipeline_summary: PipelineSummary,
    knowledge_base_summary: KnowledgeBaseSummary,
    sqlite_db_path: String,
    json_report_path: String,
}

fn resolve_existing_path(path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.exists() {
        return p;
    }
    let cur = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidates = [
        cur.join(path_str),
        cur.join("../..").join(path_str),
        cur.join("..").join(path_str),
        cur.join("app/src-tauri").join(path_str),
    ];
    for cand in candidates {
        if cand.exists() {
            return cand;
        }
    }
    p
}

fn resolve_project_path(rel: &str) -> PathBuf {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if base.ends_with("app/src-tauri") {
        base.join(rel)
    } else {
        base.join("app/src-tauri").join(rel)
    }
}

fn get_nvidia_api_key() -> Result<String> {
    if let Ok(k) = std::env::var("NVIDIA_API_KEY") {
        if !k.trim().is_empty() {
            return Ok(k.trim().to_string());
        }
    }
    let paths = [
        "temp/.env",
        "../../temp/.env",
        "../temp/.env",
        "app/src-tauri/temp/.env",
    ];
    for p in paths {
        if let Ok(content) = fs::read_to_string(p) {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("NVIDIA_API_KEY=") {
                    let val = rest.trim();
                    if !val.is_empty() {
                        return Ok(val.to_string());
                    }
                }
            }
        }
    }
    Err(anyhow!(
        "NVIDIA_API_KEY environment variable is missing or empty. Please set NVIDIA_API_KEY in temp/.env"
    ))
}

async fn extract_facts_via_nvidia_api(
    client: &reqwest::Client,
    api_key: &str,
    window_turns: &[ConversationTurn],
) -> Result<(HashMap<String, Vec<String>>, u128)> {
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

    let payload = serde_json::json!({
        "model": NVIDIA_JUDGE_MODEL,
        "messages": [
            {"role": "system", "content": COMPACTION_SYSTEM_PROMPT},
            {"role": "user", "content": user_content}
        ],
        "temperature": 0.5,
        "max_tokens": 2000
    });

    let mut last_err = anyhow!("Unknown compaction error");
    let t_start = Instant::now();

    for attempt in 1..=3 {
        let resp_res = client
            .post(NVIDIA_API_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await;

        match resp_res {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    if let Ok(json_body) = resp.json::<serde_json::Value>().await {
                        if let Some(content) =
                            json_body["choices"][0]["message"]["content"].as_str()
                        {
                            let cleaned = content
                                .trim()
                                .trim_start_matches("```markdown")
                                .trim_start_matches("```json")
                                .trim_start_matches("```")
                                .trim_end_matches("```")
                                .trim();
                            let parsed = parse_compaction_json(cleaned).unwrap_or_default();
                            let duration_ms = t_start.elapsed().as_millis();
                            return Ok((parsed, duration_ms));
                        }
                    }
                } else {
                    let err_text = resp.text().await.unwrap_or_default();
                    last_err = anyhow!("Nvidia API HTTP {}: {}", status, err_text);
                }
            }
            Err(e) => {
                last_err = anyhow!("Nvidia API request attempt {} failed: {}", attempt, e);
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000 * attempt as u64)).await;
    }

    Err(last_err)
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("============================================================================");
    println!(" Memory Pipeline Benchmark & Latency/Bias Analysis");
    println!(
        " Execution Mode: NVIDIA API ONLY (Model: {})",
        NVIDIA_JUDGE_MODEL
    );
    println!("============================================================================");

    let api_key = get_nvidia_api_key()?;
    let reqwest_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    // Parse arguments
    let args: Vec<String> = std::env::args().collect();
    let mut dataset_arg = String::new();
    let mut out_dir_arg = String::new();
    let mut chunk_size: usize = 50;
    let mut max_turns: usize = 0;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dataset" => {
                if i + 1 < args.len() {
                    dataset_arg = args[i + 1].clone();
                    i += 1;
                }
            }
            "--out-dir" => {
                if i + 1 < args.len() {
                    out_dir_arg = args[i + 1].clone();
                    i += 1;
                }
            }
            "--chunk-size" => {
                if i + 1 < args.len() {
                    chunk_size = args[i + 1].parse().unwrap_or(50);
                    i += 1;
                }
            }
            "--max-turns" => {
                if i + 1 < args.len() {
                    max_turns = args[i + 1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // Default dataset resolution
    let dataset_path = if !dataset_arg.is_empty() {
        resolve_existing_path(&dataset_arg)
    } else {
        let candidates = [
            resolve_existing_path("sandbox/datasets/dataset_session2.json"),
            resolve_existing_path("evals/datasets/dataset_session2.json"),
            resolve_existing_path("sandbox/datasets/dataset_session1.json"),
        ];
        candidates
            .iter()
            .find(|p| p.exists())
            .cloned()
            .unwrap_or_else(|| resolve_existing_path("sandbox/datasets/dataset_session2.json"))
    };

    if !dataset_path.exists() {
        return Err(anyhow!("Dataset file not found at {:?}", dataset_path));
    }

    println!("[1/5] Loading session dataset from: {:?}", dataset_path);
    let dataset_text = fs::read_to_string(&dataset_path)?;
    let mut turns: Vec<ConversationTurn> = serde_json::from_str(&dataset_text)?;
    if max_turns > 0 && turns.len() > max_turns {
        turns.truncate(max_turns);
    }
    println!("[1/5] Loaded {} conversation turns.", turns.len());

    // Prepare output directory & SQLite database
    let results_dir = if !out_dir_arg.is_empty() {
        PathBuf::from(&out_dir_arg)
    } else {
        resolve_project_path("benches/results")
    };
    fs::create_dir_all(&results_dir)?;

    let timestamp_str = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let db_filename = format!("pipeline_bench_{}.db", timestamp_str);
    let json_filename = format!("pipeline_bench_{}.json", timestamp_str);

    let db_path = results_dir.join(&db_filename);
    let json_report_path = results_dir.join(&json_filename);

    if db_path.exists() {
        let _ = fs::remove_file(&db_path);
    }

    println!(
        "[2/5] Initializing benchmark SQLite database at: {:?}",
        db_path
    );
    let db_str = db_path.to_string_lossy();
    let db = Builder::new_local(&db_str).build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    // Phase 1: Compaction Extraction Loop
    println!("[3/5] Starting Compaction Extraction Sweep via Nvidia API...");
    let chunks: Vec<&[ConversationTurn]> = turns.chunks(chunk_size).collect();
    let mut window_metrics = Vec::new();
    let mut total_compaction_duration_ms: u128 = 0;
    let mut total_extracted_facts: usize = 0;
    let mut global_extracted_facts_by_collection: HashMap<String, usize> = HashMap::new();

    let session_id = format!("bench_session_{}", timestamp_str);

    for (window_idx, chunk) in chunks.iter().enumerate() {
        let turn_start = chunk.first().map(|t| t.turn).unwrap_or(0);
        let turn_end = chunk.last().map(|t| t.turn).unwrap_or(0);

        println!(
            "  -> Window {:02}/{:02} (Turns {}-{})...",
            window_idx + 1,
            chunks.len(),
            turn_start,
            turn_end
        );

        let (extracted_facts, duration_ms) =
            extract_facts_via_nvidia_api(&reqwest_client, &api_key, chunk).await?;

        let mut per_coll_count = HashMap::new();
        let mut window_fact_count = 0;
        for (coll, fact_list) in &extracted_facts {
            let count = fact_list.len();
            per_coll_count.insert(coll.clone(), count);
            *global_extracted_facts_by_collection
                .entry(coll.clone())
                .or_insert(0) += count;
            window_fact_count += count;
        }

        total_compaction_duration_ms += duration_ms;
        total_extracted_facts += window_fact_count;

        println!(
            "     Done in {} ms. Extracted {} facts.",
            duration_ms, window_fact_count
        );

        // Enqueue into personal_memory_queue
        if !extracted_facts.is_empty() {
            enqueue_personal_facts(&conn, extracted_facts, &session_id, true).await?;
        }

        window_metrics.push(WindowCompactionMetric {
            window_idx: window_idx + 1,
            turn_start,
            turn_end,
            duration_ms,
            facts_extracted: window_fact_count,
            facts_per_collection: per_coll_count,
        });
    }

    let avg_window_duration_ms = if !chunks.is_empty() {
        total_compaction_duration_ms as f64 / chunks.len() as f64
    } else {
        0.0
    };

    let compaction_summary = CompactionSummary {
        total_windows: chunks.len(),
        total_duration_ms: total_compaction_duration_ms,
        avg_window_duration_ms,
        total_facts_extracted: total_extracted_facts,
        facts_by_collection: global_extracted_facts_by_collection,
        window_metrics,
    };

    // Phase 2: 4-Stage Ingestion Pipeline Sweep
    println!("[4/5] Starting 4-Stage Ingestion Pipeline Sweep...");
    let run_id = uuid::Uuid::new_v4().to_string();

    // Ensure MiniLM embedder loaded before Stage 2
    ensure_embedder_loaded(true)?;

    let calc_tp = |n: usize, dur: u128| -> f64 {
        if dur > 0 {
            let tp = (n as f64 / dur as f64) * 1000.0;
            if tp.is_finite() {
                tp
            } else {
                0.0
            }
        } else {
            0.0
        }
    };

    // Stage 1: Dedup
    let t1_start = Instant::now();
    let n1 = run_stage1_dedup_with_metrics(&conn, &run_id).await?;
    let dur1_ms = t1_start.elapsed().as_millis();
    let tp1 = calc_tp(n1, dur1_ms);
    println!(
        "  -> Stage 1 (Dedup): Processed {} items in {} ms ({:.2} items/sec)",
        n1, dur1_ms, tp1
    );

    // Stage 2: Embed
    let t2_start = Instant::now();
    let n2 = run_stage2_embed_with_metrics(&conn, &run_id).await?;
    let dur2_ms = t2_start.elapsed().as_millis();
    let tp2 = calc_tp(n2, dur2_ms);
    println!(
        "  -> Stage 2 (Embed): Processed {} items in {} ms ({:.2} items/sec)",
        n2, dur2_ms, tp2
    );

    // Stage 3: Eval
    let t3_start = Instant::now();
    let n3 = run_stage3_eval_with_metrics_seq(&conn, &run_id, 0).await?;
    let dur3_ms = t3_start.elapsed().as_millis();
    let tp3 = calc_tp(n3, dur3_ms);
    println!(
        "  -> Stage 3 (Eval): Processed {} items in {} ms ({:.2} items/sec)",
        n3, dur3_ms, tp3
    );

    // Stage 4: Commit & Prune
    let t4_start = Instant::now();
    let n4 = run_stage4_commit_with_metrics(&conn, &run_id).await?;
    let dur4_ms = t4_start.elapsed().as_millis();
    let tp4 = calc_tp(n4, dur4_ms);
    println!(
        "  -> Stage 4 (Commit & Prune): Committed {} facts in {} ms ({:.2} items/sec)",
        n4, dur4_ms, tp4
    );

    let total_pipeline_duration_ms = dur1_ms + dur2_ms + dur3_ms + dur4_ms;

    let pipeline_summary = PipelineSummary {
        stage1_dedup: StageMetric {
            duration_ms: dur1_ms,
            items_processed: n1,
            items_modified: n1,
            throughput_items_per_sec: tp1,
        },
        stage2_embed: StageMetric {
            duration_ms: dur2_ms,
            items_processed: n2,
            items_modified: n2,
            throughput_items_per_sec: tp2,
        },
        stage3_eval: StageMetric {
            duration_ms: dur3_ms,
            items_processed: n3,
            items_modified: n3,
            throughput_items_per_sec: tp3,
        },
        stage4_commit: StageMetric {
            duration_ms: dur4_ms,
            items_processed: n4,
            items_modified: n4,
            throughput_items_per_sec: tp4,
        },
        total_pipeline_duration_ms,
    };

    // Phase 3: Analytics & Cross-Collection Edge Matrix
    println!("[5/5] Extracting Knowledge Base State & Directed Edge Matrix...");

    // Query active facts count per collection
    let mut facts_by_collection = HashMap::new();
    let mut rows = conn
        .query(
            "SELECT collection, COUNT(*) FROM memory_facts WHERE status = 'active' GROUP BY collection",
            (),
        )
        .await?;
    let mut total_active_facts = 0;
    while let Some(row) = rows.next().await? {
        let coll: String = row.get(0).unwrap_or_else(|_| "Unknown".to_string());
        let count: i64 = row.get(1).unwrap_or(0);
        facts_by_collection.insert(coll, count as usize);
        total_active_facts += count as usize;
    }

    // Query edges count per relation type
    let mut edges_by_type = HashMap::new();
    let mut rows = conn
        .query(
            "SELECT relation, COUNT(*) FROM memory_relations GROUP BY relation",
            (),
        )
        .await?;
    let mut total_relations = 0;
    while let Some(row) = rows.next().await? {
        let rel: String = row.get(0).unwrap_or_else(|_| "UNKNOWN".to_string());
        let count: i64 = row.get(1).unwrap_or(0);
        edges_by_type.insert(rel, count as usize);
        total_relations += count as usize;
    }

    // Directed Cross-Collection Edge Pair Matrix
    let mut cross_collection_edge_matrix = Vec::new();
    let mut rows = conn
        .query(
            "SELECT f1.collection as from_col, f2.collection as to_col, r.relation, COUNT(*) as count
             FROM memory_relations r
             JOIN memory_facts f1 ON f1.id = r.from_id
             JOIN memory_facts f2 ON f2.id = r.to_id
             GROUP BY f1.collection, f2.collection, r.relation
             ORDER BY f1.collection, f2.collection, r.relation",
            (),
        )
        .await?;
    while let Some(row) = rows.next().await? {
        let from_col: String = row.get(0).unwrap_or_else(|_| "Unknown".to_string());
        let to_col: String = row.get(1).unwrap_or_else(|_| "Unknown".to_string());
        let relation: String = row.get(2).unwrap_or_else(|_| "UNKNOWN".to_string());
        let count: i64 = row.get(3).unwrap_or(0);

        cross_collection_edge_matrix.push(EdgeMatrixEntry {
            from_collection: from_col,
            to_collection: to_col,
            relation,
            count: count as usize,
        });
    }

    let knowledge_base_summary = KnowledgeBaseSummary {
        total_active_facts,
        facts_by_collection,
        total_relations,
        edges_by_type,
        cross_collection_edge_matrix,
    };

    // Construct final benchmark report
    let report = PipelineBenchReport {
        benchmark_name: "memory_pipeline_bench".to_string(),
        timestamp: Utc::now().to_rfc3339(),
        dataset_path: dataset_path.to_string_lossy().to_string(),
        total_turns: turns.len(),
        compaction_summary,
        pipeline_summary,
        knowledge_base_summary,
        sqlite_db_path: db_path.to_string_lossy().to_string(),
        json_report_path: json_report_path.to_string_lossy().to_string(),
    };

    // Write JSON report
    let json_content = serde_json::to_string_pretty(&report)
        .map_err(|e| anyhow!("JSON serialization failed: {}", e))?;
    fs::write(&json_report_path, &json_content).map_err(|e| {
        anyhow!(
            "Writing JSON report to {:?} failed: {}",
            json_report_path,
            e
        )
    })?;

    println!("\n============================================================================");
    println!(" BENCHMARK COMPLETE & RESULTS SAVED");
    println!("============================================================================");
    println!(" Database Output : {:?}", db_path);
    println!(" JSON Report     : {:?}", json_report_path);
    println!(" Total Turns     : {}", turns.len());
    println!(
        " Compaction Time : {} ms ({:.2} s)",
        total_compaction_duration_ms,
        total_compaction_duration_ms as f64 / 1000.0
    );
    println!(
        " Pipeline Time   : {} ms ({:.2} s)",
        total_pipeline_duration_ms,
        total_pipeline_duration_ms as f64 / 1000.0
    );
    println!(" Facts Extracted : {}", total_extracted_facts);
    println!(" Active Facts    : {}", total_active_facts);
    println!(" Total Relations : {}", total_relations);
    println!("----------------------------------------------------------------------------");
    println!(" Stage Latencies & Throughput:");
    println!(
        "   Stage 1 (Dedup) : {:6} ms | Throughput: {:8.2} items/sec",
        dur1_ms, tp1
    );
    println!(
        "   Stage 2 (Embed) : {:6} ms | Throughput: {:8.2} items/sec",
        dur2_ms, tp2
    );
    println!(
        "   Stage 3 (Eval)  : {:6} ms | Throughput: {:8.2} items/sec",
        dur3_ms, tp3
    );
    println!(
        "   Stage 4 (Commit): {:6} ms | Throughput: {:8.2} items/sec",
        dur4_ms, tp4
    );
    println!("----------------------------------------------------------------------------");
    println!(" Directed Cross-Collection Edge Pair Matrix:");
    if report
        .knowledge_base_summary
        .cross_collection_edge_matrix
        .is_empty()
    {
        println!("   (No cross-collection edges generated)");
    } else {
        for entry in &report.knowledge_base_summary.cross_collection_edge_matrix {
            println!(
                "   [{:12} -> {:12}] {:12} : {} edges",
                entry.from_collection, entry.to_collection, entry.relation, entry.count
            );
        }
    }
    println!("============================================================================");

    std::process::exit(0);
}
