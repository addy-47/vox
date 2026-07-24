use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use sysinfo::{ProcessRefreshKind, System};


use vox_lib::core::constants::{
    PM_RELATION_CONFLICTS, PM_RELATION_SUPPORTS, PM_RELATION_SIMILAR, MODEL_DIR_NLI_DEFAULT,
};
use vox_lib::persistence::schema;
use vox_lib::persistence::repository;
use vox_lib::services::memory::deduplication::{jaccard_similarity, is_exact_duplicate};
use vox_lib::services::memory::embedder::{ensure_embedder_loaded, generate_embedding, cosine_similarity};
use vox_lib::services::memory::nli::{
    ensure_nli_loaded, classify_pair, relation_from_result, get_calibrated_class_mapping_strings,
    NliRelation, NLI_CONTRADICTION_THRESHOLD, NLI_ENTAILMENT_THRESHOLD,
};

const NLI_CANDIDATE_LIMIT: usize = 5;
const SIMILAR_EDGE_THRESHOLD: f32 = 0.95;
const NLI_CLASSIFICATION_MIN_THRESHOLD: f32 = 0.82;

#[derive(Parser, Debug)]
#[command(name = "nli_graph_test")]
#[command(about = "Sacred Native Rust Test Harness for Vox Cognitive Memory Phase 2 & 3 Graph NLI Audit", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Batch test Phase 2 & 3 NLI and Graph Edge creation on a compaction result JSON file
    Json {
        #[arg(short, long)]
        input: PathBuf,

        /// Optional path to write a detailed JSON audit report for QA Engineer
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Inspect graph relations and facts directly in an existing SQLite database file
    Db {
        #[arg(short, long)]
        db_path: PathBuf,

        #[arg(short, long)]
        output: Option<PathBuf>,
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

    match cli.command {
        Commands::Json { input, output } => {
            println!("=================================================================");
            println!(" NATIVE RUST NLI & GRAPH AUDIT");
            println!(" Input JSON: {:?}", input);
            println!("=================================================================\n");

            if !input.exists() {
                anyhow::bail!("Input JSON file does not exist: {:?}", input);
            }

            let content = fs::read_to_string(&input)?;
            let root_val: Value = serde_json::from_str(&content)?;

            let model_name = root_val["model"].as_str().unwrap_or("unknown_model");
            let dataset_name = root_val["dataset"].as_str().unwrap_or("unknown_dataset");

            let mut extracted_facts: Vec<(String, String)> = Vec::new();
            
            if let Some(sessions) = root_val["sessions"].as_array() {
                for session in sessions {
                    if let Some(facts) = session["facts"].as_array() {
                        for item in facts {
                            if let (Some(col), Some(content)) = (item["collection"].as_str(), item["content"].as_str()) {
                                extracted_facts.push((col.to_string(), content.trim().to_string()));
                            }
                        }
                    }
                }
            } else if let Some(comp_results) = root_val["compaction_results"].as_array() {
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
            } else if let Some(facts_arr) = root_val.as_array() {
                 for item in facts_arr {
                     if let (Some(col), Some(f)) = (item["collection"].as_str(), item["fact"].as_str()) {
                         extracted_facts.push((col.to_string(), f.trim().to_string()));
                     }
                 }
            }

            println!("Loaded Model: {}", model_name);
            println!("Dataset:      {}", dataset_name);
            println!("Total Raw Extracted Facts: {}\n", extracted_facts.len());

            let db = turso::Builder::new_local(":memory:").build().await?;
            let conn = db.connect()?;
            schema::run_migrations(&conn).await?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            for (col, fact_text) in &extracted_facts {
                conn.execute(
                    "INSERT INTO personal_memory_queue (fact, collection, source, session_id, status, created_at)
                     VALUES (?, ?, 'LLM', 'test_session', 'pending', ?)",
                    (fact_text.as_str(), col.as_str(), now),
                )
                .await?;
            }

            let mut total_pipeline_ms = 0.0;
            let mut total_embedding_ms = 0.0;
            let mut total_sim_search_ms = 0.0;
            let mut total_nli_inference_ms = 0.0;
            let mut total_db_persistence_ms = 0.0;
            
            let mut nli_pair_latencies = Vec::new();
            let mut classified_pairs = Vec::new();
            
            let mut merged_count = 0;
            let mut ingested_count = 0;

            loop {
                let mut rows = conn
                    .query(
                        "SELECT id, fact, collection, source, session_id FROM personal_memory_queue 
                         WHERE status = 'pending' ORDER BY created_at ASC LIMIT 1",
                        (),
                    )
                    .await?;

                let item = if let Some(row) = rows.next().await? {
                    Some((
                        row.get::<i64>(0)?,
                        row.get::<String>(1)?,
                        row.get::<String>(2)?,
                        row.get::<String>(3)?,
                        row.get::<String>(4)?,
                    ))
                } else {
                    None
                };

                let (job_id, fact, collection, source, session_id) = match item {
                    Some(x) => x,
                    None => break,
                };

                let item_start = Instant::now();

                conn.execute(
                    "UPDATE personal_memory_queue SET status = 'processing' WHERE id = ?",
                    (job_id,),
                )
                .await?;

                let fact_id = format!("mem_{}_{}", now, uuid::Uuid::new_v4().simple());

                // Embedding
                let emb_start = Instant::now();
                ensure_embedder_loaded(true)?;
                let embedding = generate_embedding(&fact)?.unwrap();
                total_embedding_ms += emb_start.elapsed().as_secs_f64() * 1000.0;

                // Class C Guardrail: Identity & Context are completely isolated (No Candidate Search, No NLI Edges)
                let is_class_c = collection.eq_ignore_ascii_case("Identity") || collection.eq_ignore_ascii_case("Context");
                let is_class_a = collection.eq_ignore_ascii_case("Tasks") || collection.eq_ignore_ascii_case("Goals") || collection.eq_ignore_ascii_case("Constraints");

                if is_class_c || !is_class_a {
                    // Store fact directly without graph edges
                    let db_start = Instant::now();
                    repository::insert_fact_with_vector_and_relations(
                        &conn, job_id, &fact_id, &fact, &collection, &source, &session_id, &embedding, Vec::new(), now
                    ).await?;
                    total_db_persistence_ms += db_start.elapsed().as_secs_f64() * 1000.0;
                    ingested_count += 1;
                    total_pipeline_ms += item_start.elapsed().as_secs_f64() * 1000.0;
                    continue;
                }

                // Candidate Similarity (Strictly Intra-Collection for Class A)
                let sim_start = Instant::now();
                let candidate_vectors = repository::fetch_active_candidate_vectors(&conn, &collection).await?;
                let mut scored_candidates = Vec::new();
                for (cand_id, cand_fact, emb_vector) in candidate_vectors {
                    let sim = cosine_similarity(&embedding, &emb_vector);
                    scored_candidates.push((sim, cand_id, cand_fact));
                }
                scored_candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                
                let mut exact_match = None;
                for (sim, cand_id, cand_fact) in &scored_candidates {
                    let jacc_sim = jaccard_similarity(&fact, cand_fact);
                    if is_exact_duplicate(*sim, jacc_sim) {
                        exact_match = Some(cand_id.clone());
                        break;
                    }
                }
                total_sim_search_ms += sim_start.elapsed().as_secs_f64() * 1000.0;

                if let Some(matched_cand_id) = exact_match {
                    let db_start = Instant::now();
                    repository::insert_exact_merged_fact(
                        &conn, job_id, &fact_id, &fact, &collection, &source, &session_id, &matched_cand_id, &embedding, now
                    ).await?;
                    total_db_persistence_ms += db_start.elapsed().as_secs_f64() * 1000.0;
                    merged_count += 1;
                } else {
                    let candidates: Vec<(f32, String, String)> = scored_candidates
                        .into_iter()
                        .take(NLI_CANDIDATE_LIMIT)
                        .collect();

                    let mut relations = Vec::new();
                    let mut nli_pairs_to_classify = Vec::new();

                    for (sim, cand_id, cand_fact) in candidates {
                        if sim > SIMILAR_EDGE_THRESHOLD {
                            relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_SIMILAR));
                        } else if sim >= NLI_CLASSIFICATION_MIN_THRESHOLD {
                            nli_pairs_to_classify.push((sim, cand_id, cand_fact));
                        }
                    }

                    if !nli_pairs_to_classify.is_empty() {
                        ensure_nli_loaded(MODEL_DIR_NLI_DEFAULT)?;
                        for (sim, cand_id, cand_fact) in nli_pairs_to_classify {
                            let nli_start = Instant::now();
                            let nli_res = classify_pair(&fact, &cand_fact)?;
                            let relation = relation_from_result(&nli_res);
                            let pair_ms = nli_start.elapsed().as_secs_f64() * 1000.0;
                            total_nli_inference_ms += pair_ms;
                            nli_pair_latencies.push(pair_ms);

                            let relation_str = match relation {
                                NliRelation::Conflicts => {
                                    let rel_str = if collection.eq_ignore_ascii_case("Tasks") || collection.eq_ignore_ascii_case("Goals") {
                                        "SUPERSEDES"
                                    } else {
                                        PM_RELATION_CONFLICTS
                                    };
                                    relations.push((fact_id.clone(), cand_id.clone(), rel_str));
                                    rel_str
                                }
                                NliRelation::Supports => {
                                    relations.push((fact_id.clone(), cand_id.clone(), PM_RELATION_SUPPORTS));
                                    PM_RELATION_SUPPORTS
                                }
                                NliRelation::Neutral => "NEUTRAL",
                            };

                            classified_pairs.push(serde_json::json!({
                                "premise_fact_id": fact_id.clone(),
                                "premise_fact": fact.clone(),
                                "hypothesis_fact_id": cand_id,
                                "hypothesis_fact": cand_fact,
                                "cosine_sim": sim,
                                "contradiction_prob": nli_res.contradiction,
                                "entailment_prob": nli_res.entailment,
                                "neutral_prob": nli_res.neutral,
                                "relation": relation_str,
                                "inference_ms": pair_ms,
                            }));
                        }
                    }

                    let db_start = Instant::now();
                    repository::insert_fact_with_vector_and_relations(
                        &conn, job_id, &fact_id, &fact, &collection, &source, &session_id, &embedding, relations, now
                    ).await?;
                    total_db_persistence_ms += db_start.elapsed().as_secs_f64() * 1000.0;
                    ingested_count += 1;
                }
                
                total_pipeline_ms += item_start.elapsed().as_secs_f64() * 1000.0;
            }

            sys.refresh_process_specifics(
                sysinfo::get_current_pid().unwrap(),
                ProcessRefreshKind::new().with_memory().with_cpu(),
            );
            let end_mem_mb = sysinfo_mem_mb(&sys);
            let peak_ram_mb = end_mem_mb.max(start_mem_mb);

            let calibrated_class_mapping = get_calibrated_class_mapping_strings();
            if let Some(ref mapping) = calibrated_class_mapping {
                log::info!("[Audit] Calibrated Class Mapping: [0: {}, 1: {}, 2: {}]", mapping[0], mapping[1], mapping[2]);
            }

            // Fetch final relations
            let mut relation_rows = conn
                .query(
                    "SELECT r.from_id, r.to_id, r.relation, 
                            f1.fact as from_fact_text, f2.fact as to_fact_text,
                            f1.collection as from_collection, f2.collection as to_collection
                     FROM memory_relations r
                     JOIN memory_facts f1 ON r.from_id = f1.id
                     JOIN memory_facts f2 ON r.to_id = f2.id",
                    ()
                ).await?;

            let mut final_relations = Vec::new();
            while let Some(row) = relation_rows.next().await? {
                final_relations.push(serde_json::json!({
                    "from_fact_id": row.get::<String>(0)?,
                    "to_fact_id": row.get::<String>(1)?,
                    "relation": row.get::<String>(2)?,
                    "from_fact_text": row.get::<String>(3)?,
                    "to_fact_text": row.get::<String>(4)?,
                    "from_collection": row.get::<String>(5)?,
                    "to_collection": row.get::<String>(6)?,
                }));
            }

            // Fetch final facts
            let mut fact_rows = conn
                .query("SELECT id, collection, fact FROM memory_facts WHERE status = 'active'", ())
                .await?;

            let mut final_facts = Vec::new();
            while let Some(row) = fact_rows.next().await? {
                final_facts.push(serde_json::json!({
                    "id": row.get::<String>(0)?,
                    "collection": row.get::<String>(1)?,
                    "fact": row.get::<String>(2)?,
                }));
            }

            println!("=================================================================");
            println!(" NATIVE RUST NLI METRICS & RESULTS");
            println!("=================================================================");
            println!(" Total Enqueued:           {}", extracted_facts.len());
            println!(" Merged:                   {}", merged_count);
            println!(" Ingested:                 {}", ingested_count);
            println!(" Graph Edges Created:      {}", final_relations.len());
            println!(" Classified NLI Pairs:     {}", classified_pairs.len());
            if let Some(ref mapping) = calibrated_class_mapping {
                println!(" Calibrated Class Mapping: [0: {}, 1: {}, 2: {}]", mapping[0], mapping[1], mapping[2]);
            } else {
                println!(" Calibrated Class Mapping: [NLI Engine Unloaded]");
            }
            println!(" Thresholds Enforced:      Contradiction >= {}, Entailment >= {}", NLI_CONTRADICTION_THRESHOLD, NLI_ENTAILMENT_THRESHOLD);
            println!(" Total Pipeline Latency:   {:.2} ms", total_pipeline_ms);
            println!(" Embedding Latency:        {:.2} ms", total_embedding_ms);
            println!(" Sim Search Latency:       {:.2} ms", total_sim_search_ms);
            println!(" NLI Inference Latency:    {:.2} ms", total_nli_inference_ms);
            println!(" DB Persistence Latency:   {:.2} ms", total_db_persistence_ms);
            println!(" Peak RAM Usage:           {:.2} MB", peak_ram_mb);
            println!("=================================================================\n");

            let report_json = serde_json::json!({
                "model": model_name,
                "dataset": dataset_name,
                "calibrated_class_mapping": calibrated_class_mapping,
                "nli_thresholds": {
                    "contradiction_threshold": NLI_CONTRADICTION_THRESHOLD,
                    "entailment_threshold": NLI_ENTAILMENT_THRESHOLD,
                },
                "metrics": {
                    "total_pipeline_ms": total_pipeline_ms,
                    "total_embedding_ms": total_embedding_ms,
                    "total_sim_search_ms": total_sim_search_ms,
                    "total_nli_inference_ms": total_nli_inference_ms,
                    "total_db_persistence_ms": total_db_persistence_ms,
                    "nli_pair_latencies_ms": nli_pair_latencies,
                    "peak_ram_mb": peak_ram_mb,
                },
                "counts": {
                    "total_enqueued": extracted_facts.len(),
                    "merged_count": merged_count,
                    "ingested_count": ingested_count,
                    "final_active_facts": final_facts.len(),
                    "graph_edges_created": final_relations.len(),
                    "classified_nli_pairs": classified_pairs.len(),
                },
                "classified_nli_pairs": classified_pairs,
                "relations": final_relations,
                "facts": final_facts,
            });

            let out_file = output.unwrap_or_else(|| {
                let mut p = input.clone();
                p.set_extension("rust_nli_audit.json");
                p
            });

            fs::write(&out_file, serde_json::to_string_pretty(&report_json)?)?;
            println!("Saved detailed QA audit report to: {:?}", out_file);
        }

        Commands::Db { db_path, output } => {
            println!("Connecting to SQLite database at {:?}", db_path);
            let db = turso::Builder::new_local(db_path.to_str().unwrap()).build().await?;
            let conn = db.connect()?;

            let mut relation_rows = conn
                .query(
                    "SELECT r.from_id, r.to_id, r.relation, 
                            f1.fact as from_fact_text, f2.fact as to_fact_text,
                            f1.collection as from_collection, f2.collection as to_collection
                     FROM memory_relations r
                     JOIN memory_facts f1 ON r.from_id = f1.id
                     JOIN memory_facts f2 ON r.to_id = f2.id",
                    ()
                ).await?;

            let mut final_relations = Vec::new();
            while let Some(row) = relation_rows.next().await? {
                final_relations.push(serde_json::json!({
                    "from_fact_id": row.get::<String>(0)?,
                    "to_fact_id": row.get::<String>(1)?,
                    "relation": row.get::<String>(2)?,
                    "from_fact_text": row.get::<String>(3)?,
                    "to_fact_text": row.get::<String>(4)?,
                    "from_collection": row.get::<String>(5)?,
                    "to_collection": row.get::<String>(6)?,
                }));
            }

            let mut fact_rows = conn
                .query("SELECT id, collection, fact FROM memory_facts WHERE status = 'active'", ())
                .await?;

            let mut final_facts = Vec::new();
            while let Some(row) = fact_rows.next().await? {
                final_facts.push(serde_json::json!({
                    "id": row.get::<String>(0)?,
                    "collection": row.get::<String>(1)?,
                    "fact": row.get::<String>(2)?,
                }));
            }

            println!("Active personal memory facts in DB: {}", final_facts.len());
            println!("Active memory relations in DB: {}", final_relations.len());

            let report_json = serde_json::json!({
                "db_path": db_path,
                "counts": {
                    "final_active_facts": final_facts.len(),
                    "graph_edges": final_relations.len(),
                },
                "relations": final_relations,
                "facts": final_facts,
            });

            if let Some(out_file) = output {
                fs::write(&out_file, serde_json::to_string_pretty(&report_json)?)?;
                println!("Saved DB audit report to: {:?}", out_file);
            }
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
