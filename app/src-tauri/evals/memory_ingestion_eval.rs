//! ============================================================================
//! memory_ingestion_eval.rs — Ladder rung 2: ingestion dedup end-to-end
//! ============================================================================
//! Category     : Evaluation
//! Component    : Memory ingestion 2-stage dedup (stage1 exact + stage2 cosine)
//! Prerequisites: Rung-1 ladder DB; local MiniLM-L12 ONNX model; NVIDIA_API_KEY
//! Execution    : cargo run --release --bin memory_ingestion_eval -- --db-in <eval_r1.db>
//! Metrics      : Stage summaries, merge counts, judge false-merge/missed-dupe
//!                verdict
//! ============================================================================

//! Takes the rung-1 DB (its pending queue is the input — the ladder), copies
//! it so rung 1 stays immutable, and runs ONE end-to-end dedup pass: the same
//! two stage functions `run_ingestion_cycle` calls internally, in the same
//! order. Inactive facts are snapshotted between stages so each merge is
//! honestly attributed to exact-match or semantic-match stage. Then ONE judge
//! call grades semantic dedup correctness.

#[path = "common/mod.rs"]
mod common;

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use common::{db, judge, report};
use vox_lib::{
    persistence::queue::has_unfinished_items,
    services::memory::{
        ingestion::{run_stage1_exact_dedup, run_stage2_cosine_dedup},
        ml::embedder::unload_embedder,
    },
};

#[derive(Parser, Debug)]
#[command(
    name = "memory_ingestion_eval",
    about = "Memory ladder rung 2: ingestion dedup eval"
)]
struct Args {
    /// Rung-1 ladder DB file (copied, never mutated in place).
    #[arg(long)]
    db_in: PathBuf,
    /// Nvidia hosted judge model.
    #[arg(long, default_value = "nvidia/nemotron-3-super-120b-a12b")]
    judge_model: String,
    /// Skip the judge call (deterministic asserts still run; verdict pending).
    #[arg(long, default_value_t = false)]
    no_judge: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
struct FactView {
    id: String,
    fact_type: String,
    text: String,
    status: String,
}

async fn snapshot_facts(conn: &turso::Connection) -> Result<Vec<FactView>> {
    // Dev-tool read: the persistence API exposes active-only fetchers, but the
    // eval must also see deactivated facts to grade merges.
    let mut rows = conn
        .query(
            "SELECT id, type, text, status FROM memory_facts ORDER BY id;",
            (),
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(FactView {
            id: row.get(0)?,
            fact_type: row.get(1)?,
            text: row.get(2)?,
            status: row.get(3)?,
        });
    }
    Ok(out)
}

async fn pending_count(conn: &turso::Connection) -> Result<i64> {
    let mut rows = conn
        .query(
            "SELECT COUNT(*) FROM memory_ingestion_queue WHERE status IN ('pending','stage1_processing','stage1_done','stage2_processing');",
            (),
        )
        .await?;
    let row = rows.next().await?.context("No count row")?;
    Ok(row.get(0)?)
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    tokio::time::timeout(Duration::from_secs(30 * 60), run(args))
        .await
        .context("Rung 2 top-level timeout (30 min) exceeded")?
}

async fn run(args: Args) -> Result<()> {
    let started = Instant::now();
    let run_id = report::new_run_id();
    let eval_name = "ingestion";
    let run_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evals/results")
        .join(eval_name)
        .join(&run_id);
    let db_path = run_dir.join("eval_r2.db");

    // --- Ladder handoff: copy rung-1 DB, never mutate it in place -----------
    std::fs::create_dir_all(&run_dir)?;
    std::fs::copy(&args.db_in, &db_path)
        .with_context(|| format!("Failed to copy ladder DB from {}", args.db_in.display()))?;
    let (_db, conn) = db::open_existing_eval_db(&db_path).await?;

    let pending_before = pending_count(&conn).await?;
    anyhow::ensure!(
        pending_before > 0,
        "Ladder DB has zero pending queue items — rung 1 produced nothing to ingest"
    );

    // --- ONE end-to-end dedup pass (same stages, same order as production) --
    let s1_started = Instant::now();
    let s1 = run_stage1_exact_dedup(&conn)
        .await
        .context("Stage 1 exact dedup failed")?;
    let s1_s = s1_started.elapsed().as_secs_f64();
    let inactive_after_s1: std::collections::HashSet<String> = snapshot_facts(&conn)
        .await?
        .into_iter()
        .filter(|f| f.status == "inactive")
        .map(|f| f.id)
        .collect();

    let s2_started = Instant::now();
    let s2 = run_stage2_cosine_dedup(&conn)
        .await
        .context("Stage 2 cosine dedup failed")?;
    let s2_s = s2_started.elapsed().as_secs_f64();
    unload_embedder();

    // --- Deterministic baseline asserts -------------------------------------
    anyhow::ensure!(
        !has_unfinished_items(&conn).await?,
        "Queue still has unfinished items after a full cycle"
    );
    let all_facts = snapshot_facts(&conn).await?;
    anyhow::ensure!(!all_facts.is_empty(), "Zero facts in DB after ingestion");
    let active: Vec<&FactView> = all_facts.iter().filter(|f| f.status == "active").collect();
    let inactive: Vec<&FactView> = all_facts
        .iter()
        .filter(|f| f.status == "inactive")
        .collect();
    anyhow::ensure!(!active.is_empty(), "Zero active facts after ingestion");

    // Attribute each merge to its deciding stage by snapshot diff.
    let mut exact_merges = Vec::new();
    let mut semantic_merges = Vec::new();
    for f in &inactive {
        if inactive_after_s1.contains(&f.id) {
            exact_merges.push(f);
        } else {
            semantic_merges.push(f);
        }
    }

    // --- ONE judge call: were merges real dupes, any missed? ----------------
    let judge_out = if args.no_judge {
        None
    } else {
        let api_key = std::env::var("NVIDIA_API_KEY")
            .context("NVIDIA_API_KEY not set — export it from temp/.env first")?;
        let system_prompt = judge::load_prompt("judge_ingestion.md")?;
        let merged_json = serde_json::to_string_pretty(
            &exact_merges
                .iter()
                .map(|f| serde_json::json!({"stage": "exact", "type": f.fact_type, "deactivated": f.text}))
                .chain(semantic_merges.iter().map(|f| {
                    serde_json::json!({"stage": "semantic", "type": f.fact_type, "deactivated": f.text})
                }))
                .collect::<Vec<_>>(),
        )?;
        let survivors_json = serde_json::to_string_pretty(&active)?;
        let user_content = format!("MERGED_PAIRS:\n{merged_json}\n\nSURVIVORS:\n{survivors_json}");
        Some(
            tokio::time::timeout(
                Duration::from_secs(600),
                judge::run_judge(
                    &api_key,
                    &args.judge_model,
                    &system_prompt,
                    &user_content,
                    4000,
                ),
            )
            .await
            .context("Judge call timed out")?
            .context("Judge call failed")?,
        )
    };

    // --- Report --------------------------------------------------------------
    let total_s = started.elapsed().as_secs_f64();
    let payload = serde_json::json!({
        "eval": "rung2_ingestion_dedup",
        "inputs": {
            "ladder_db_in": args.db_in.to_string_lossy(),
            "judge_model": args.judge_model,
            "pending_before": pending_before,
        },
        "stages": {
            "stage1": {"processed": s1.processed, "duplicates_deactivated": s1.duplicates_deactivated, "errors": s1.errors, "latency_s": s1_s},
            "stage2": {"processed": s2.processed, "inserted": s2.inserted, "duplicates_deactivated": s2.duplicates_deactivated, "errors": s2.errors, "latency_s": s2_s},
        },
        "outcome": {
            "queue_drained": true,
            "active_facts": active.len(),
            "inactive_facts": inactive.len(),
            "exact_merges": exact_merges.len(),
            "semantic_merges": semantic_merges.len(),
        },
        "judge": judge_out.as_ref().map(|j| serde_json::json!({
            "verdict": j.verdict,
            "latency_s": j.latency_s,
            "raw_response": j.raw_content,
        })),
        "ladder_handoff_db": db_path.to_string_lossy(),
        "total_latency_s": total_s,
    });
    let written = report::write_report(eval_name, &run_id, payload)?;
    println!("Rung 2 complete: {pending_before} queued -> {} active / {} inactive ({} exact + {} semantic merges).", active.len(), inactive.len(), exact_merges.len(), semantic_merges.len());
    println!("Report: {}", written.join("report.json").display());
    println!("Ladder DB for rung 3: {}", db_path.display());
    Ok(())
}
