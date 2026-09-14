//! ============================================================================
//! memory_compaction_eval.rs — Ladder rung 1: critical compaction under true trip
//! ============================================================================
//! Category     : Evaluation
//! Component    : Memory compaction inline path (prepare_turn -> run_and_persist)
//! Prerequisites: Remote Ollama server with gemma3:12b; NVIDIA_API_KEY for judge
//! Execution    : cargo run --release --bin memory_compaction_eval -- --help
//! Metrics      : Trip turn count, utilization evidence, staged facts, judge
//!                coverage/bucket/leakage/groundedness verdict
//! ============================================================================

//! Feeds the frozen 100-turn session through the production turn loop
//! (prepare_turn -> commit_turn) until the REAL budget plugin reports
//! Critical (>=85%) and returns NeedsInlineCompaction. That slice is compacted
//! with ONE gemma3:12b run via the production inline function
//! (CompactionPlugin::run_and_persist, trigger_kind "inline"), persisted to a
//! fresh eval DB, then judged once for semantic extraction quality.
//!
//! The rung-1 DB file is the ladder handoff to rung 2. Nothing is forced: if
//! no genuine trip occurs, the eval FAILS instead of compacting anyway.

#[path = "common/mod.rs"]
mod common;

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use clap::Parser;
use common::{db, judge, report, settings_cfg, turns};
use tokio_util::sync::CancellationToken;
use vox_lib::services::{
    harness::{CompactionParams, CompactionPlugin, HarnessSession, TurnPreparation},
    llm::{actor::create_llm_provider_from_llm_settings, LlmCommand},
    memory::ml::tokenizer::estimate_tokens,
};

/// Fixed base prompt for the eval session (same role as production's persona
/// prompt: a system message counted by the budget plugin, then replaced by
/// nothing — documented here so the trip math is reproducible).
const EVAL_BASE_PROMPT: &str =
    "You are a concise voice assistant. Reply in exactly two short sentences.";

#[derive(Parser, Debug)]
#[command(
    name = "memory_compaction_eval",
    about = "Memory ladder rung 1: critical compaction eval"
)]
struct Args {
    /// Context window; 8192 is the production floor (MIN_LLM_CONTEXT_WINDOW).
    /// The 300-turn trip feed is sized so the 85% line is genuinely crossed.
    #[arg(long, default_value_t = 8192)]
    context_window: u32,
    /// Executor endpoint (Ollama /v1 or any OpenAI-compatible base URL).
    #[arg(long, default_value = "http://100.67.98.126:11434/v1")]
    server_url: String,
    /// Executor model on the server.
    #[arg(long, default_value = "gemma3:12b")]
    server_model: String,
    /// Executor API key (empty for keyless endpoints like local Ollama).
    #[arg(long, default_value = "")]
    server_api_key: String,
    /// Executor transport preset: "ollama" for Ollama native, "" for generic
    /// OpenAI chat-completions (e.g. Nvidia).
    #[arg(long, default_value = "ollama")]
    server_provider: String,
    /// Judge chat-completions URL (empty = Nvidia default).
    #[arg(long, default_value = "")]
    judge_url: String,
    /// Judge model.
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
        .context("Rung 1 top-level timeout (30 min) exceeded")?
}

async fn run(args: Args) -> Result<()> {
    let started = Instant::now();
    let run_id = report::new_run_id();
    let eval_name = "compaction";
    let run_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("evals/results")
        .join(eval_name)
        .join(&run_id);
    let db_path = run_dir.join("eval_r1.db");

    // --- Load fixture + fresh DB -------------------------------------------
    let fixture = turns::load_trip_turns()?;
    anyhow::ensure!(!fixture.is_empty(), "Turn fixture is empty");
    let (_db, conn) = db::open_fresh_eval_db(&db_path).await?;
    let (session_id, _turn_rows) = db::seed_session_with_turns(&conn, &fixture).await?;

    // --- Build production session objects ----------------------------------
    let settings = settings_cfg::server_llm_settings(
        &args.server_url,
        &args.server_model,
        args.context_window,
        if args.server_api_key.trim().is_empty() {
            None
        } else {
            Some(args.server_api_key.clone())
        },
        if args.server_provider.trim().is_empty() {
            None
        } else {
            Some(args.server_provider.clone())
        },
    );
    let provider = create_llm_provider_from_llm_settings(
        &settings_cfg::llm_settings_of(&settings),
        std::path::Path::new(""),
    )
    .map_err(|e| anyhow::anyhow!("Failed to build server provider: {e}"))?;
    let (llm_tx, _llm_rx) = std::sync::mpsc::channel::<LlmCommand>();
    let mut harness = HarnessSession::new_modular(
        Some(session_id),
        EVAL_BASE_PROMPT.to_string(),
        None,
        &settings,
        llm_tx,
    );
    harness.seed_continuation(None, Vec::new());

    // --- Feed the production turn loop until a GENUINE critical trip -------
    // prepare_turn -> commit_turn is exactly what the voice pipeline calls per
    // turn; fixture assistant text stands in for TTS-confirmed replies.
    let mut fed_turns: u32 = 0;
    let mut tripped_turn: Option<u32> = None;
    let mut compact_slice = Vec::new();
    for t in &fixture {
        match harness.prepare_turn(&t.user, t.turn) {
            TurnPreparation::NeedsInlineCompaction {
                uncompacted_slice, ..
            } => {
                tripped_turn = Some(t.turn);
                fed_turns = t.turn;
                compact_slice = uncompacted_slice;
                break;
            }
            TurnPreparation::Ready(_) => {
                harness.commit_turn(t.assistant.clone());
                fed_turns = t.turn;
            }
            TurnPreparation::DuplicateTurnIgnored => {
                anyhow::bail!(
                    "Fixture turn {} was dropped as duplicate — fixture error",
                    t.turn
                );
            }
        }
    }
    let tripped_turn = tripped_turn.with_context(|| {
        format!(
            "NO GENUINE TRIP: fed all {} turns without hitting Critical (>=85% of {} ctx window). Eval fails by design — no forced compaction.",
            fed_turns, args.context_window
        )
    })?;
    let from_turn = harness.from_turn_id();

    // Utilization evidence at trip time (production estimator, same function
    // the budget plugin uses internally).
    let slice_tokens: usize = compact_slice
        .iter()
        .map(|m| estimate_tokens(&m.content))
        .sum();

    // --- ONE executor run through the production inline function ------------
    let cancel = CancellationToken::new();
    let llm_started = Instant::now();
    let params = CompactionParams {
        session_id,
        trigger_kind: "inline",
        from_turn_id: from_turn,
        to_turn_id: tripped_turn,
        history_messages: &compact_slice,
        llm_settings: Some(&settings.llm),
        cancel: Some(&cancel),
    };
    let result = tokio::time::timeout(
        Duration::from_secs(600),
        CompactionPlugin::run_and_persist(provider.as_ref(), &conn, params),
    )
    .await
    .context("Compaction executor call timed out")?
    .context("Compaction executor run failed")?;
    let llm_latency_s = llm_started.elapsed().as_secs_f64();
    eprintln!(
        "[rung1] trip at turn {tripped_turn} ({fed_turns} fed, {} slice msgs, ~{slice_tokens} tokens); executor {llm_latency_s:.1}s, raw {} chars, {} facts",
        compact_slice.len(),
        result.raw_json.len(),
        result.facts.len(),
    );

    // --- Deterministic baseline asserts (pass/fail, not the eval) -----------
    let latest = vox_lib::persistence::compactions::fetch_latest_compaction_run(&conn, session_id)
        .await
        .context("Failed to read compaction ledger")?
        .context("No compaction run recorded in ledger")?;
    anyhow::ensure!(
        latest.status == "completed",
        "Ledger run status is '{}', expected 'completed'",
        latest.status
    );
    let mut staged_rows = conn
        .query(
            "SELECT type, text, status FROM memory_ingestion_queue WHERE compaction_id = ?;",
            (latest.id,),
        )
        .await?;
    let mut staged: Vec<(String, String, String)> = Vec::new();
    while let Some(row) = staged_rows.next().await? {
        staged.push((row.get(0)?, row.get(1)?, row.get(2)?));
    }
    anyhow::ensure!(!staged.is_empty(), {
        // Classify the zero-facts outcome instead of guessing: valid-but-empty
        // JSON vs lenient parse fallback (raw text kept, zero staged facts).
        // Persist the full raw output + a failure report so the finding is
        // auditable without re-running the executor.
        let parsed = vox_lib::utils::json::parse_unified_compaction_json(&result.raw_json);
        let classification = if parsed.is_some() {
            "valid JSON with empty categories"
        } else {
            "LENIENT PARSE FALLBACK (non-empty text, JSON parse failed both attempts)"
        };
        let _ = std::fs::create_dir_all(&run_dir);
        let _ = std::fs::write(run_dir.join("raw_compaction.json"), &result.raw_json);
        let failure = serde_json::json!({
            "eval": "rung1_critical_compaction",
            "status": "FAILED",
            "classification": classification,
            "trip": {"fed_turns": fed_turns, "tripped_at_turn": tripped_turn, "slice_messages": compact_slice.len(), "slice_tokens_estimate": slice_tokens},
            "executor": {"latency_s": llm_latency_s, "raw_chars": result.raw_json.len(), "facts_extracted": result.facts.len(), "ledger_run_id": latest.id, "ledger_status": latest.status},
            "ladder_handoff_db": db_path.to_string_lossy(),
        });
        let _ = std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_string_pretty(&failure).unwrap_or_default(),
        );
        let _ = std::fs::write(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("evals/results")
                .join(eval_name)
                .join("latest.json"),
            serde_json::to_string_pretty(&failure).unwrap_or_default(),
        );
        let preview: String = result.raw_json.chars().take(1500).collect();
        format!(
            "Zero facts staged — nothing to judge. Classification: {classification}. Full raw saved to {}/raw_compaction.json. Raw preview:\n{preview}",
            run_dir.display()
        )
    });
    anyhow::ensure!(
        staged.iter().all(|(_, _, s)| s == "pending"),
        "Staged queue items are not all 'pending'"
    );

    // --- ONE judge call: semantic extraction quality ------------------------
    let mut facts_by_cat: std::collections::BTreeMap<&str, Vec<&str>> =
        std::collections::BTreeMap::new();
    for (cat, text) in &result.facts {
        facts_by_cat
            .entry(cat.as_str())
            .or_default()
            .push(text.as_str());
    }
    let judge_out = if args.no_judge {
        None
    } else {
        let api_key = std::env::var("NVIDIA_API_KEY").unwrap_or_default();
        let system_prompt = judge::load_prompt("judge_compaction.md")?;
        let turns_json = serde_json::to_string_pretty(
            &fixture
                .iter()
                .filter(|t| t.turn <= tripped_turn)
                .collect::<Vec<_>>(),
        )?;
        let facts_json = serde_json::to_string_pretty(&facts_by_cat)?;
        let user_content = format!("TURNS:\n{turns_json}\n\nFACTS_BY_CATEGORY:\n{facts_json}");
        Some(
            tokio::time::timeout(
                Duration::from_secs(600),
                judge::run_judge(
                    &args.judge_url,
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

    // --- Report -------------------------------------------------------------
    let total_s = started.elapsed().as_secs_f64();
    let payload = serde_json::json!({
        "eval": "rung1_critical_compaction",
        "inputs": {
            "fixture_turns": fixture.len(),
            "context_window": args.context_window,
            "server_model": args.server_model,
            "judge_model": args.judge_model,
            "base_prompt": EVAL_BASE_PROMPT,
        },
        "trip": {
            "genuine_prepare_turn_trip": true,
            "fed_turns": fed_turns,
            "tripped_at_turn": tripped_turn,
            "from_turn_id": from_turn,
            "to_turn_id": tripped_turn,
            "slice_messages": compact_slice.len(),
            "slice_tokens_estimate": slice_tokens,
        },
        "executor": {
            "latency_s": llm_latency_s,
            "facts_extracted": result.facts.len(),
            "facts": result.facts,
            "ledger_run_id": latest.id,
            "ledger_status": latest.status,
            "staged_queue_items": staged.len(),
        },
        "judge": judge_out.as_ref().map(|j| serde_json::json!({
            "verdict": j.verdict.as_str(),
            "latency_s": j.latency_s,
            "report_markdown": j.report_markdown,
        })),
        "ladder_handoff_db": db_path.to_string_lossy(),
        "total_latency_s": total_s,
    });
    db::checkpoint_source_db(&db_path)
        .await
        .context("Failed to checkpoint rung-1 DB for ladder handoff")?;
    let written = report::write_report(eval_name, &run_id, payload)?;
    if let Some(j) = judge_out.as_ref() {
        let _ = std::fs::write(written.join("judge_report.md"), &j.report_markdown);
        println!("Judge verdict: {}", j.verdict.as_str());
    }
    println!(
        "Rung 1 complete: {fed_turns} turns fed, trip at turn {tripped_turn}, {} facts staged.",
        result.facts.len()
    );
    println!("Report: {}", written.join("report.json").display());
    println!("Ladder DB for rung 2: {}", db_path.display());
    Ok(())
}
