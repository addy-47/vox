//! ============================================================================
//! memory_consolidation_eval.rs — Ladder rung 3: personal consolidation quality
//! ============================================================================
//! Category     : Evaluation
//! Component    : Personal memory consolidation (consolidate_personal_memory)
//! Prerequisites: Rung-2 ladder DB; remote Ollama server; NVIDIA_API_KEY
//! Execution    : cargo run --release --bin memory_consolidation_eval -- --db-in <eval_r2.db>
//! Metrics      : Candidate count, merge latency, judge coverage/quality/
//!                groundedness verdict
//! ============================================================================

//! Takes the rung-2 DB (its active personal facts are the input — the ladder),
//! copies it so rung 2 stays immutable, and runs ONE consolidation pass with
//! gemma3:12b through the production function. Then ONE judge call grades
//! coverage and document quality against the input facts.

#[path = "common/mod.rs"]
mod common;

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use common::{db, judge, report, settings_cfg};
use vox_lib::{
    persistence::{
        facts::fetch_active_facts_by_type, personal_memory::get_personal_memory,
        queue::has_unfinished_items,
    },
    services::{
        llm::actor::create_llm_provider_from_llm_settings,
        memory::personal::consolidate_personal_memory,
    },
};

#[derive(Parser, Debug)]
#[command(
    name = "memory_consolidation_eval",
    about = "Memory ladder rung 3: consolidation eval"
)]
struct Args {
    /// Rung-2 ladder DB file (copied, never mutated in place).
    #[arg(long)]
    db_in: PathBuf,
    /// Remote Ollama base URL (gemma3:12b executor).
    #[arg(long, default_value = "http://100.67.98.126:11434/v1")]
    server_url: String,
    /// Executor model on the server.
    #[arg(long, default_value = "gemma3:12b")]
    server_model: String,
    /// Nvidia hosted judge model.
    #[arg(long, default_value = "nvidia/nemotron-3-super-120b-a12b")]
    judge_model: String,
    /// Skip the judge call (deterministic asserts still run; verdict pending).
    #[arg(long, default_value_t = false)]
    no_judge: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    tokio::time::timeout(Duration::from_secs(30 * 60), run(args))
        .await
        .context("Rung 3 top-level timeout (30 min) exceeded")?
}

async fn run(args: Args) -> Result<()> {
    let started = Instant::now();
    let run_id = report::new_run_id();
    let eval_name = "consolidation";
    let run_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evals/results")
        .join(eval_name)
        .join(&run_id);
    let db_path = run_dir.join("eval_r3.db");

    // --- Ladder handoff: copy rung-2 DB --------------------------------------
    std::fs::create_dir_all(&run_dir)?;
    db::checkpoint_source_db(&args.db_in)
        .await
        .context("Failed to checkpoint ladder DB before copy")?;
    std::fs::copy(&args.db_in, &db_path)
        .with_context(|| format!("Failed to copy ladder DB from {}", args.db_in.display()))?;
    let (_db, conn) = db::open_existing_eval_db(&db_path).await?;

    // --- Quiescence gate must hold before consolidation may run --------------
    anyhow::ensure!(
        !has_unfinished_items(&conn).await?,
        "Ingestion queue not drained — rung 2 handoff is dirty"
    );
    anyhow::ensure!(
        !vox_lib::persistence::has_in_progress_compaction(&conn).await?,
        "A compaction run is still in progress — gate blocks consolidation"
    );

    // --- Snapshot candidates BEFORE the run (post-run they flip status) -----
    let candidates = fetch_active_facts_by_type(&conn, "personal").await?;
    anyhow::ensure!(
        !candidates.is_empty(),
        "INCOMPLETE, not green: zero active personal facts in ladder DB — nothing to consolidate. The ladder lineage produced no personal facts; eval stops here."
    );
    let candidate_texts: Vec<String> = candidates.iter().map(|f| f.text.clone()).collect();
    let candidate_ids: Vec<String> = candidates.iter().map(|f| f.id.clone()).collect();
    let doc_before = get_personal_memory(&conn, None).await?;

    // --- ONE executor run through the production function --------------------
    let settings = settings_cfg::server_llm_settings(
        &args.server_url,
        &args.server_model,
        32768,
        None,
        Some("ollama".to_string()),
    );
    let provider = create_llm_provider_from_llm_settings(
        &settings_cfg::llm_settings_of(&settings),
        std::path::Path::new(""),
    )
    .map_err(|e| anyhow::anyhow!("Failed to build server provider: {e}"))?;
    let llm_started = Instant::now();
    let record = tokio::time::timeout(
        Duration::from_secs(600),
        consolidate_personal_memory(&conn, provider.as_ref(), None, None, None, None),
    )
    .await
    .context("Consolidation executor call timed out")?
    .context("Consolidation executor run failed")?;
    let llm_latency_s = llm_started.elapsed().as_secs_f64();

    // --- Deterministic baseline asserts --------------------------------------
    anyhow::ensure!(
        !record.content.trim().is_empty(),
        "Consolidated document is empty"
    );
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM memory_facts WHERE type = 'personal' AND status = 'active';",
            (),
        )
        .await?;
    let leftover: i64 = rows.next().await?.context("No count row")?.get(0)?;
    anyhow::ensure!(
        leftover == 0,
        "{leftover} personal facts still 'active' — merge did not transition all candidates"
    );

    // --- ONE judge call: coverage + quality ----------------------------------
    let judge_out = if args.no_judge {
        None
    } else {
        let api_key = std::env::var("NVIDIA_API_KEY")
            .context("NVIDIA_API_KEY not set — export it from temp/.env first")?;
        let system_prompt = judge::load_prompt("judge_consolidation.md")?;
        let facts_json = serde_json::to_string_pretty(&candidate_texts)?;
        let user_content = format!(
            "FACTS:\n{facts_json}\n\nPRE_EXISTING_DOCUMENT:\n{}\n\nDOCUMENT:\n{}",
            doc_before.content, record.content
        );
        Some(
            tokio::time::timeout(
                Duration::from_secs(600),
                judge::run_judge(
                    "",
                    &api_key,
                    &args.judge_model,
                    &system_prompt,
                    &user_content,
                    6000,
                ),
            )
            .await
            .context("Judge call timed out")?
            .context("Judge call failed")?,
        )
    };

    // --- Report ---------------------------------------------------------------
    let total_s = started.elapsed().as_secs_f64();
    let payload = serde_json::json!({
        "eval": "rung3_personal_consolidation",
        "inputs": {
            "ladder_db_in": args.db_in.to_string_lossy(),
            "server_model": args.server_model,
            "judge_model": args.judge_model,
            "candidate_personal_facts": candidate_ids.len(),
        },
        "executor": {
            "latency_s": llm_latency_s,
            "doc_chars": record.content.len(),
            "doc_version": record.version,
            "leftover_active": leftover,
        },
        "document": record.content,
        "judge": judge_out.as_ref().map(|j| serde_json::json!({
            "verdict": j.verdict.as_str(),
            "latency_s": j.latency_s,
            "report_markdown": j.report_markdown,
        })),
        "ladder_handoff_db": db_path.to_string_lossy(),
        "total_latency_s": total_s,
    });
    let written = report::write_report(eval_name, &run_id, payload)?;
    if let Some(j) = judge_out.as_ref() {
        let _ = std::fs::write(written.join("judge_report.md"), &j.report_markdown);
        println!("Judge verdict: {}", j.verdict.as_str());
    }
    println!(
        "Rung 3 complete: {} personal facts consolidated into {}-char document.",
        candidate_ids.len(),
        record.content.len()
    );
    println!("Report: {}", written.join("report.json").display());
    Ok(())
}
