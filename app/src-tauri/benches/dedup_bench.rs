//! ============================================================================
//! dedup_bench.rs — Native Memory Deduplication Latency & Memory Benchmark
//! ============================================================================
//! Category     : Benchmark
//! Component    : Memory Deduplication Engine (`vox_lib::services::memory::deduplication`)
//! Prerequisites: None (runs in-memory SQLite database)
//! Execution    : cargo test --bench dedup_bench
//! ============================================================================

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use sysinfo::{ProcessRefreshKind, System};

use vox_lib::core::settings::MemorySettings;
use vox_lib::persistence::schema;
use vox_lib::services::memory::deduplication::{is_exact_duplicate, jaccard_similarity};
use vox_lib::services::memory::embedder::{cosine_similarity, ensure_embedder_loaded, generate_embedding};
use vox_lib::services::memory::orchestrator::{process_one_queue_item, PipelineOutcome};

#[derive(Parser, Debug)]
#[command(name = "dedup_test")]
#[command(about = "Sacred Native Rust Test Harness for Vox Cognitive Memory Phase 1 Deduplication & Subsystem", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Compare two specific facts directly using Jaccard and Cosine similarity
    Direct {
        #[arg(short, long)]
        fact1: String,

        #[arg(short, long)]
        fact2: String,
    },

    /// Batch test Phase 1 Deduplication & Orchestrator pipeline on a compaction result JSON file
    Json {
        #[arg(short, long)]
        input: PathBuf,

        /// Optional path to write a detailed JSON audit report for QA Engineer
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Inspect facts directly in an existing SQLite database file
    Db {
        #[arg(short, long)]
        db_path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();

    let mut sys = System::new_all();
    sys.refresh_process_specifics(
        sysinfo::get_current_pid().unwrap(),
        ProcessRefreshKind::new().with_memory().with_cpu(),
    );

    let start_mem_mb = sysinfo_mem_mb(&sys);
    let start_time = Instant::now();

    match cli.command {
        Commands::Direct { fact1, fact2 } => {
            println!("=================================================================");
            println!(" NATIVE RUST DIRECT FACT DEDUPLICATION TEST");
            println!("=================================================================");
            println!("Fact 1: \"{}\"", fact1);
            println!("Fact 2: \"{}\"\n", fact2);

            let jacc_sim = jaccard_similarity(&fact1, &fact2);

            let mut cos_sim = 0.0f32;
            let mut emb_loaded = false;

            if ensure_embedder_loaded(true).is_ok() {
                emb_loaded = true;
                if let (Ok(Some(emb1)), Ok(Some(emb2))) = (generate_embedding(&fact1), generate_embedding(&fact2)) {
                    cos_sim = cosine_similarity(&emb1, &emb2);
                }
            }

            let is_exact = is_exact_duplicate(cos_sim, jacc_sim);

            let elapsed = start_time.elapsed();
            sys.refresh_process_specifics(
                sysinfo::get_current_pid().unwrap(),
                ProcessRefreshKind::new().with_memory().with_cpu(),
            );
            let end_mem_mb = sysinfo_mem_mb(&sys);

            println!("--- RESULTS ---");
            println!("Jaccard Token Overlap Similarity: {:.4}", jacc_sim);
            if emb_loaded {
                println!("Embedding Vector Cosine Similarity: {:.4}", cos_sim);
            } else {
                println!("Embedding Model: Not loaded / Fallback mode");
            }
            println!("Phase 1 Hard Merge Triggered (is_exact_duplicate): {}", is_exact);
            println!("----------------");
            println!("Execution Latency: {:.2} ms", elapsed.as_secs_f64() * 1000.0);
            println!("RAM Memory Delta: {:.2} MB (Current Peak: {:.2} MB)", end_mem_mb - start_mem_mb, end_mem_mb);
        }

        Commands::Json { input, output } => {
            println!("=================================================================");
            println!(" NATIVE RUST ORCHESTRATOR PHASE 1 DEDUPLICATION AUDIT");
            println!(" Input JSON: {:?}", input);
            println!("=================================================================\n");

            if !input.exists() {
                anyhow::bail!("Input JSON file does not exist: {:?}", input);
            }

            let content = fs::read_to_string(&input)?;
            let root_val: Value = serde_json::from_str(&content)?;

            let model_name = root_val["model"].as_str().unwrap_or("unknown_model");
            let dataset_name = root_val["dataset"].as_str().unwrap_or("unknown_dataset");

            // Extract facts sequentially across compaction checkpoints
            let mut extracted_facts: Vec<(String, String)> = Vec::new(); // (collection, fact)
            if let Some(comp_results) = root_val["compaction_results"].as_array() {
                for cp in comp_results {
                    if let Some(delta) = cp["extracted_delta"].as_object() {
                        for (col, items) in delta {
                            if let Some(arr) = items.as_array() {
                                for item in arr {
                                    if let Some(s) = item.as_str() {
                                        extracted_facts.push((col.clone(), s.trim().to_string()));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            println!("Loaded Model: {}", model_name);
            println!("Dataset:      {}", dataset_name);
            println!("Total Raw Extracted Facts: {}\n", extracted_facts.len());

            // Initialize in-memory Turso SQLite database
            let db = turso::Builder::new_local(":memory:").experimental_index_method(true).build().await?;
            let conn = db.connect()?;
            schema::run_migrations(&conn).await?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            // Enqueue all extracted facts into personal_memory_queue
            for (col, fact_text) in &extracted_facts {
                conn.execute(
                    "INSERT INTO personal_memory_queue (fact, collection, source, session_id, status, created_at)
                     VALUES (?, ?, 'LLM', 'test_session', 'pending', ?)",
                    (fact_text.as_str(), col.as_str(), now),
                )
                .await?;
            }

            let settings = MemorySettings::default();
            let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let mut merged_events = Vec::new();
            let mut ingested_events = Vec::new();

            let pipeline_start = Instant::now();

            // Run process_one_queue_item in a loop until no pending items remain
            loop {
                let outcome = process_one_queue_item(&conn, &settings, &cancel_flag).await?;
                match outcome {
                    PipelineOutcome::NoWork => break,
                    PipelineOutcome::Merged { fact_id, merged_into } => {
                        merged_events.push((fact_id, merged_into));
                    }
                    PipelineOutcome::Ingested { fact_id, relations } => {
                        ingested_events.push((fact_id, relations));
                    }
                }
            }

            let pipeline_duration = pipeline_start.elapsed();

            // Query final state from active memory_facts table
            let mut rows = conn
                .query("SELECT id, fact, collection FROM memory_facts WHERE status = 'active'", ())
                .await?;

            let mut final_db_facts = Vec::new();
            while let Some(row) = rows.next().await? {
                let fid: String = row.get(0)?;
                let fact: String = row.get(1)?;
                let col: String = row.get(2)?;
                final_db_facts.push((fid, col, fact));
            }

            sys.refresh_process_specifics(
                sysinfo::get_current_pid().unwrap(),
                ProcessRefreshKind::new().with_memory().with_cpu(),
            );
            let end_mem_mb = sysinfo_mem_mb(&sys);

            println!("=================================================================");
            println!(" NATIVE RUST PIPELINE METRICS & RESULTS");
            println!("=================================================================");
            println!(" Total Incoming Facts Enqueued:    {}", extracted_facts.len());
            println!(" Phase 1 Hard-Merged / Intercepted: {}", merged_events.len());
            println!(" Final Active Unique Memory Facts:  {}", final_db_facts.len());
            println!(" Pipeline Execution Latency:        {:.2} ms ({:.2} ms/fact)", 
                     pipeline_duration.as_secs_f64() * 1000.0,
                     (pipeline_duration.as_secs_f64() * 1000.0) / extracted_facts.len().max(1) as f64);
            println!(" Process RAM Memory Usage:          {:.2} MB", end_mem_mb);
            println!("=================================================================\n");

            // Write detailed report for QA Engineer audit
            let report_json = serde_json::json!({
                "model": model_name,
                "dataset": dataset_name,
                "total_enqueued_facts": extracted_facts.len(),
                "phase1_merged_count": merged_events.len(),
                "final_active_memory_count": final_db_facts.len(),
                "execution_latency_ms": pipeline_duration.as_secs_f64() * 1000.0,
                "ram_usage_mb": end_mem_mb,
                "merged_events": merged_events.iter().map(|(fid, m_into)| {
                    serde_json::json!({"merged_fact_id": fid, "target_fact_id": m_into})
                }).collect::<Vec<_>>(),
                "final_active_facts": final_db_facts.iter().map(|(fid, col, f)| {
                    serde_json::json!({"id": fid, "collection": col, "fact": f})
                }).collect::<Vec<_>>()
            });

            let out_file = output.unwrap_or_else(|| {
                let mut p = input.clone();
                p.set_extension("rust_dedup_audit.json");
                p
            });

            fs::write(&out_file, serde_json::to_string_pretty(&report_json)?)?;
            println!("Saved detailed QA audit report to: {:?}", out_file);
        }

        Commands::Db { db_path } => {
            println!("Connecting to SQLite database at {:?}", db_path);
            let db = turso::Builder::new_local(db_path.to_str().unwrap()).build().await?;
            let conn = db.connect()?;

            let mut rows = conn
                .query("SELECT COUNT(*) FROM memory_facts WHERE status = 'active'", ())
                .await?;

            let count: i64 = if let Some(row) = rows.next().await? {
                row.get(0)?
            } else {
                0
            };

            println!("Active personal memory count in DB: {}", count);
        }
    }

    Ok(())
}

fn sysinfo_mem_mb(sys: &System) -> f64 {
    if let Some(p) = sys.process(sysinfo::get_current_pid().unwrap()) {
        (p.memory() as f64) / (1024.0 * 1024.0)
    } else {
        0.0
    }
}
