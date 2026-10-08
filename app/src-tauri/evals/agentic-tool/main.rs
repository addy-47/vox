//! ============================================================================
//! evals/agentic-tool/main.rs — Agentic Tool Evaluation Harness
//! ============================================================================
//! Category     : Evaluation
//! Component    : services/harness (loop, steps, stages/tools) + services/tts + services/audio
//! Prerequisites: a local Kokoro model; outbound network for tool egress
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : tool latency, LLM-visible context, per-stage telemetry, audio clips
//! ============================================================================
//!
//! ## The seam
//!
//! ```text
//! INPUT   = the LLM's output to the harness = a scripted tool call
//!           flags / corpus -> CanonicalToolCall -> mock LLM serves it as SSE
//! OUTPUT  = the harness's output to the LLM = the context string the model sees
//!           captured from the FINAL request body the mock LLM received
//! ```
//!
//! The eval sits only at that boundary. Everything between is production code:
//! the real `RemoteTransport` parses the stream, the real `StreamRoutingStage`
//! detects the tool call, the real state machine runs Thinking -> Working, the real
//! TTS worker synthesizes the interim filler into a device-free
//! `PlaybackEngine`, and the real tool executes.
//!
//! ## Tool-agnostic by construction
//!
//! This file knows about mock LLM, harness wiring, TTS and artifact capture. It
//! knows nothing about any individual tool's arguments. Per-tool semantics live in
//! `tools/<name>.rs`.
//!
//! ## Quality judgement is not in here
//!
//! No in-run LLM judge. The QA pass is a separate step performed by the main agent
//! (or any CLI coding harness) using a prompt from `common/qa_prompts.yaml`. See
//! that file for how a harness is expected to pick the prompt.

#[path = "../common/mod.rs"]
mod common;

mod harness_ctx;
mod mock_llm;
mod tools;
mod tts_capture;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use common::{
    metrics::{LatencyStats, StageAggregator},
    reporting::{markdown_table, resolve_output_dir, write_summary_markdown},
    stage_dump::{self, names},
};
use mock_llm::{MockLlmServer, TurnScript};
use tools::web_search::{CorpusEntry, WebSearchScript};

const EVAL_NAME: &str = "agentic-tool";

#[derive(Parser, Debug)]
#[command(
    name = "agentic_tool_eval",
    about = "Agentic tool evaluation: scripted tool call in, LLM-visible context out"
)]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run one tool through the harness using flag-supplied arguments.
    Tool(ToolArgs),
    /// Run every corpus query through the harness.
    Batch(BatchArgs),
    /// Print the run directories available for a QA pass.
    Runs,
}

#[derive(Parser, Debug)]
struct ToolArgs {
    /// Tool name. Resolved against `tools/`.
    #[arg(long, default_value = "web_search")]
    tool: String,

    /// The user query. Becomes the tool call's `query` property.
    #[arg(long)]
    query: String,

    #[arg(long, default_value = "any")]
    time_filter: String,
    #[arg(long, default_value = "hybrid")]
    ranking_mode: String,
    #[arg(long, default_value_t = 5)]
    max_passages: u64,
    #[arg(long, default_value = "Searching the web now...")]
    spoken_filler: String,

    /// Depth family knob. Not in the shipped schema; inert until the tool adopts it.
    #[arg(long)]
    depth: Option<String>,
    /// Depth family knob. Focus sub-aspect.
    #[arg(long)]
    focus: Option<String>,
    #[arg(long, value_delimiter = ',')]
    sub_queries: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    must_contain: Vec<String>,

    /// Embedding arm used by the tool. Varies the ranking backend, not the schema.
    #[arg(long, default_value = "minilm-prod")]
    embedder: EmbedderArm,

    #[arg(long, default_value_t = 8192)]
    context_window: u32,
    /// Wall-clock ceiling for the whole turn.
    #[arg(long, default_value_t = 60)]
    turn_timeout_s: u64,
    /// Wall-clock ceiling for TTS to settle after the turn.
    #[arg(long, default_value_t = 20)]
    tts_timeout_s: u64,
    /// Text the mock returns on the follow-up turn once the tool observation landed.
    /// E1: a realistic two-sentence grounded-style answer (not a stub) so the
    /// eval exercises the answer-render path at realistic length. Still canned —
    /// the mock cannot ground against an observation it has not seen yet — but
    /// no longer a 5-word constant that hides render regressions.
    #[arg(
        long,
        default_value = "Based on what I found, here is the answer in brief. The top sources agree on the key facts, with details and caveats as cited above."
    )]
    final_text: String,

    #[arg(long)]
    output_dir: Option<PathBuf>,
}

#[derive(Parser, Debug)]
struct BatchArgs {
    #[arg(long, default_value = "web_search")]
    tool: String,
    /// Restrict to a substring of the corpus id or query.
    #[arg(long)]
    filter: Option<String>,
    /// Restrict to a maximum number of entries.
    #[arg(long)]
    limit: Option<usize>,

    #[arg(long, default_value = "any")]
    time_filter: String,
    #[arg(long, default_value = "hybrid")]
    ranking_mode: String,
    #[arg(long, default_value_t = 5)]
    max_passages: u64,
    #[arg(long, default_value = "Searching the web now...")]
    spoken_filler: String,
    #[arg(long, default_value = "minilm-prod")]
    embedder: EmbedderArm,

    #[arg(long, default_value_t = 8192)]
    context_window: u32,
    #[arg(long, default_value_t = 60)]
    turn_timeout_s: u64,
    #[arg(long, default_value_t = 20)]
    tts_timeout_s: u64,
    #[arg(long, default_value = "Here is what I found.")]
    final_text: String,
    /// Skip entries whose case directory already has artifacts.
    #[arg(long, default_value_t = false)]
    resume: bool,
    /// Pause between cases to stay under provider rate limits.
    ///
    /// Without this, 24 back-to-back queries reliably trip upstream throttling and
    /// cases fail with `network.failed/transient`, which reads as a quality
    /// regression but is not one. See the pacing note in `run_cases`.
    #[arg(long, default_value_t = 2000)]
    case_delay_ms: u64,
    /// Retry a case this many times when the tool reports a transient failure.
    #[arg(long, default_value_t = 2)]
    transient_retries: u32,
    #[arg(long)]
    output_dir: Option<PathBuf>,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbedderArm {
    MinilmProd,
    MinilmFixed,
    BgeM3,
    None,
}

impl EmbedderArm {
    fn label(self) -> &'static str {
        match self {
            Self::MinilmProd => "minilm-prod",
            Self::MinilmFixed => "minilm-fixed",
            Self::BgeM3 => "bge-m3",
            Self::None => "none",
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    match Args::parse().cmd {
        Cmd::Tool(a) => {
            let entry = CorpusEntry {
                id: format!("single_{}", slug(&a.query)),
                query: a.query.clone(),
            };
            let script = WebSearchScript {
                query: a.query.clone(),
                time_filter: a.time_filter.clone(),
                ranking_mode: a.ranking_mode.clone(),
                max_passages: a.max_passages,
                spoken_filler: a.spoken_filler.clone(),
                depth: a.depth.clone(),
                focus: a.focus.clone(),
                sub_queries: a.sub_queries.clone(),
                must_contain: a.must_contain.clone(),
                ..Default::default()
            };
            let cfg = RunConfig {
                tool: a.tool.clone(),
                embedder: a.embedder,
                context_window: a.context_window,
                turn_timeout_s: a.turn_timeout_s,
                tts_timeout_s: a.tts_timeout_s,
                final_text: a.final_text.clone(),
                output_dir: a.output_dir.clone(),
                case_delay_ms: 0,
                transient_retries: 0,
            };
            run_cases(&[entry], &[script], &cfg, false).await
        }
        Cmd::Batch(a) => {
            let corpus = load_corpus_for(&a.tool, a.filter.as_deref(), a.limit)?;
            let base = WebSearchScript {
                time_filter: a.time_filter.clone(),
                ranking_mode: a.ranking_mode.clone(),
                max_passages: a.max_passages,
                spoken_filler: a.spoken_filler.clone(),
                ..Default::default()
            };
            let scripts: Vec<WebSearchScript> = corpus
                .iter()
                .map(|e| WebSearchScript::from_corpus_entry(&e.query, &base))
                .collect();
            let cfg = RunConfig {
                tool: a.tool.clone(),
                embedder: a.embedder,
                context_window: a.context_window,
                turn_timeout_s: a.turn_timeout_s,
                tts_timeout_s: a.tts_timeout_s,
                final_text: a.final_text.clone(),
                output_dir: a.output_dir.clone(),
                case_delay_ms: a.case_delay_ms,
                transient_retries: a.transient_retries,
            };
            run_cases(&corpus, &scripts, &cfg, a.resume).await
        }
        Cmd::Runs => {
            let root = resolve_output_dir(None, EVAL_NAME);
            let mut dirs = Vec::new();
            if let Ok(rd) = std::fs::read_dir(&root) {
                for e in rd.flatten() {
                    if e.path().is_dir() {
                        dirs.push(e.path());
                    }
                }
            }
            dirs.sort();
            println!("Run directories under {}", root.display());
            for d in dirs {
                println!("  {}", d.display());
            }
            println!(
                "\nNext: read evals/common/qa_prompts.yaml, pick the prompt matching the eval you\n\
                 ran, substitute the placeholders, and launch your subagent."
            );
            Ok(())
        }
    }
}

struct RunConfig {
    tool: String,
    embedder: EmbedderArm,
    context_window: u32,
    turn_timeout_s: u64,
    tts_timeout_s: u64,
    final_text: String,
    output_dir: Option<PathBuf>,
    /// Pause between cases so provider throttling does not corrupt the measurement.
    case_delay_ms: u64,
    /// Retries for a case that failed with a transient upstream error.
    transient_retries: u32,
}

fn load_corpus_for(
    tool: &str,
    filter: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<CorpusEntry>> {
    anyhow::ensure!(
        tool == "web_search",
        "No corpus is registered for tool '{}'. Add tools/{}.rs with a loader, or pass \
         --query via the `tool` subcommand.",
        tool,
        tool
    );
    let all = tools::web_search::load_corpus()?;
    let mut out: Vec<CorpusEntry> = all
        .into_iter()
        .filter(|e| {
            filter
                .map(|f| e.id.contains(f) || e.query.contains(f))
                .unwrap_or(true)
        })
        .collect();
    if let Some(l) = limit {
        out.truncate(l);
    }
    anyhow::ensure!(!out.is_empty(), "Corpus selection produced zero entries");
    Ok(out)
}

fn slug(s: &str) -> String {
    s.chars()
        .take(40)
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}

async fn run_cases(
    corpus: &[CorpusEntry],
    scripts: &[WebSearchScript],
    cfg: &RunConfig,
    resume: bool,
) -> Result<()> {
    anyhow::ensure!(
        corpus.len() == scripts.len(),
        "corpus/script length mismatch: {} vs {}",
        corpus.len(),
        scripts.len()
    );

    let started = Instant::now();
    let results_root = resolve_output_dir(cfg.output_dir.as_deref(), EVAL_NAME);
    let run_id = common::reporting::generate_run_id();
    let run_dir = common::reporting::create_run_directory(&results_root, &run_id)?;
    let cases_dir = run_dir.join("cases");
    std::fs::create_dir_all(&cases_dir)?;

    println!("=== Agentic Tool Eval: {EVAL_NAME} / {} ===", run_id);
    println!("Tool     : {}", cfg.tool);
    println!("Embedder : {}", cfg.embedder.label());
    println!("Cases    : {}", corpus.len());
    println!("Out      : {}", run_dir.display());

    let manifest = stage_dump::build_run_manifest(
        EVAL_NAME,
        &run_id,
        &std::env::args().collect::<Vec<_>>(),
        serde_json::json!({
            "tool": cfg.tool,
            "embedder": cfg.embedder.label(),
            "context_window": cfg.context_window,
            "turn_timeout_s": cfg.turn_timeout_s,
            "cases": corpus,
            "scripts": scripts.iter().map(|s| s.summary()).collect::<Vec<_>>(),
            "seam": "input=scripted tool call via mock LLM; output=final request body",
        }),
    )?;
    stage_dump::write_json(&run_dir, names::MANIFEST, &manifest)?;

    let mut rows: Vec<CaseSummary> = Vec::new();

    for (idx, (entry, script)) in corpus.iter().zip(scripts).enumerate() {
        let case_dir = cases_dir.join(&entry.id);
        if resume && case_dir.join(names::CONTEXT_LLM_SAW).exists() {
            println!("  {:<24} skipped (resume)", entry.id);
            if let Ok(v) = stage_dump::read_json(&case_dir, "case_summary.json") {
                if let Ok(s) = serde_json::from_value(v) {
                    rows.push(s);
                    continue;
                }
            }
        }

        // Pace between cases. Providers throttle bursts, and a throttled case
        // surfaces as `network.failed/transient` - indistinguishable from a
        // retrieval-quality failure unless the caller looks at the status code.
        // Pacing keeps a batch measurement honest; without it the same code can
        // score 62% or 83% purely on how fast the batch was fired.
        if idx > 0 && cfg.case_delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(cfg.case_delay_ms)).await;
        }

        let mut summary = match run_one(entry, script, &case_dir, cfg).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  {:<24} FAILED: {}", entry.id, e);
                rows.push(CaseSummary::failed(&entry.id, e.to_string()));
                continue;
            }
        };

        // Retry only transient upstream failures, with backoff. Retrying a genuine
        // quality miss would just burn time re-confirming the miss.
        let mut attempt = 0;
        while summary.transient_failure && attempt < cfg.transient_retries {
            attempt += 1;
            let backoff_ms = 3_000 * u64::from(attempt);
            eprintln!(
                "  {:<24} transient upstream failure, retry {}/{} in {}ms",
                entry.id, attempt, cfg.transient_retries, backoff_ms
            );
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            // Drop the partial case dir so the retry writes a clean set of artifacts.
            let _ = std::fs::remove_dir_all(&case_dir);
            match run_one(entry, script, &case_dir, cfg).await {
                Ok(s) => summary = s,
                Err(e) => {
                    eprintln!("  {:<24} FAILED on retry: {}", entry.id, e);
                    break;
                }
            }
        }

        if summary.transient_failure {
            eprintln!(
                "  {:<24} STILL TRANSIENT after {} retries - treat as infrastructure noise, not a quality miss",
                entry.id, cfg.transient_retries
            );
        }

        println!(
            "  {:<24} tool={}ms ctx={}B filler={} audio={} retrieval={}{}",
            entry.id,
            summary.tool_duration_ms,
            summary.context_bytes,
            summary.filler_chars,
            summary.audio_clips,
            if summary.retrieval_ok { "ok" } else { "FAIL" },
            if summary.transient_failure { " TRANSIENT" } else { "" },
        );
        rows.push(summary);
    }

    write_summary(
        &run_dir,
        &results_root,
        &run_id,
        &rows,
        started.elapsed().as_secs_f64(),
    )?;
    println!(
        "\nDone in {:.1}s. Summary: {}",
        started.elapsed().as_secs_f64(),
        run_dir.join(names::SUMMARY_MD).display()
    );
    println!(
        "QA pass  : read evals/common/qa_prompts.yaml, use the prompt matching this eval, and\n\
         launch your subagent against {}",
        run_dir.display()
    );
    Ok(())
}

async fn run_one(
    entry: &CorpusEntry,
    script: &WebSearchScript,
    case_dir: &Path,
    cfg: &RunConfig,
) -> Result<CaseSummary> {
    std::fs::create_dir_all(case_dir)?;
    let started = Instant::now();

    // 1. Script the tool call the mock LLM will emit.
    let args = script.arguments();
    stage_dump::write_json(
        case_dir,
        "tool_call_arguments.json",
        &serde_json::json!({ "name": script.tool, "arguments": args }),
    )?;

    let turn_script = TurnScript::tool(&script.tool, args.clone()).then_text(&cfg.final_text);
    let server = MockLlmServer::start(vec![turn_script])?;

    // 2. Build the production context, pointed at the mock.
    let hcfg = harness_ctx::EvalConfig {
        eval_name: EVAL_NAME.to_string(),
        server_url: format!("{}/chat/completions", server.base_url),
        server_model: "mock-llm".to_string(),
        server_api_key: None,
        server_provider: None,
        context_window: cfg.context_window,
    };
    let results_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("evals/results");
    let ctx = harness_ctx::setup_isolated_eval_context(&hcfg, &results_root).await?;
    let case_run_dir = ctx.results_dir.clone();
    // The harness writes its own `pipeline_events.json` into this dir during the
    // turn and `persist_event_trace` writes the eval's trace here too — it must
    // exist (it was previously deleted here, which broke the trace write ENOENT).
    std::fs::create_dir_all(&case_run_dir)?;

    // 3. Real TTS into a device-free playback engine.
    let tts = tts_capture::start(&ctx.tts_settings, ctx.event_tx.clone())?;

    // 4. Run the real turn loop.
    let harness = harness_ctx::build_modular_harness(&ctx, Some(&script.tool))?;
    let harness_arc = Arc::new(parking_lot::Mutex::new(Some(harness)));

    let req = harness_ctx::build_turn_request(
        &ctx,
        1,
        entry.query.clone(),
        &tts.tts_tx,
        &tts.pending_synthesis_jobs,
    );
    let turn_start = Instant::now();
    let outcome = tokio::time::timeout(
        Duration::from_secs(cfg.turn_timeout_s),
        vox_lib::services::harness::chassis::Harness::execute_turn(&harness_arc, req),
    )
    .await;
    let turn_ms = turn_start.elapsed().as_millis() as u64;

    let tts_drained = tts_capture::drain_until_idle(&tts, Duration::from_secs(cfg.tts_timeout_s));

    // 5. The output: the final request body the mock received.
    mock_llm::require_requests(&server, 2)?;
    let requests = server.captured_requests();
    let final_request = requests.last().cloned().expect("require_requests checked");
    stage_dump::write_json(case_dir, names::CONTEXT_LLM_SAW, &final_request)?;

    let req_dir = case_dir.join(names::REQUESTS_DIR);
    std::fs::create_dir_all(&req_dir)?;
    for (i, r) in requests.iter().enumerate() {
        stage_dump::write_json(&req_dir, &format!("request_{i}.json"), r)?;
    }

    let observations = mock_llm::tool_observations(&final_request);
    let observation = observations
        .last()
        .map(|(_, c)| c.clone())
        .unwrap_or_default();
    stage_dump::write_verbatim(case_dir, names::EVIDENCE, &observation)?;

    // 6. Tool-call ledger: latency and the arguments the harness actually received.
    // The persistence worker writes asynchronously, so poll briefly for the row
    // instead of assuming it landed before the query.
    let conn = ctx.db.connect().map_err(|e| anyhow!("{e}"))?;
    let mut tool_calls = fetch_tool_calls(&conn, ctx.session_id, 1).await?;
    let ledger_deadline = Instant::now() + Duration::from_secs(5);
    while tool_calls.as_array().map(|a| a.is_empty()).unwrap_or(true)
        && Instant::now() < ledger_deadline
    {
        tokio::time::sleep(Duration::from_millis(100)).await;
        tool_calls = fetch_tool_calls(&conn, ctx.session_id, 1).await?;
    }
    stage_dump::write_json(case_dir, names::TOOL_CALLS, &tool_calls)?;
    let received = mock_llm::tool_call_from_request(&final_request);

    // 7. Audio artifacts.
    let render_log = tts_capture::render_log(&tts);
    tts_capture::persist_render_log(case_dir, &render_log)?;
    let clips = tts_capture::harvest(case_dir, &tts)?;
    tts_capture::shutdown(&tts);

    // 8. Pipeline events, proving the state machine ran and returned.
    let trace = harness_ctx::drain_events_after_turn(&ctx.event_rx, 250);
    harness_ctx::persist_event_trace(&ctx, &trace)?;
    let events_path = stage_dump::write_json(
        case_dir,
        names::PIPELINE_EVENTS,
        &serde_json::to_value(&trace)?,
    )?;

    // Copy anything the harness wrote into its own run dir into the case dir.
    copy_if_exists(&case_run_dir.join(names::PIPELINE_EVENTS), &events_path).ok();

    let tool_duration_ms = tool_calls
        .get(0)
        .and_then(|c| c.get("duration_ms"))
        .and_then(|d| d.as_u64())
        .unwrap_or(0);
    let filler = render_log
        .iter()
        .find(|r| r.intent == "interim_filler")
        .cloned();
    let turn_text = match &outcome {
        Ok(o) => harness_ctx::outcome_text(o),
        Err(_) => "[TIMEOUT]".to_string(),
    };

    let sources = count_attr(&observation, "total_sources");
    let passages = count_attr(&observation, "total_passages");
    // E3: filler credibility from measured audio, not synthesis wall time. The
    // first clip is the interim filler (dispatched before the tool ran); its
    // measured length minus tool latency is the overrun QA §4.6 caught.
    let filler_overrun_ms = stage_dump::read_json(case_dir, names::AUDIO_SUMMARY)
        .ok()
        .and_then(|v| {
            v.get("clips_detail")?
                .as_array()?
                .first()?
                .get("seconds")?
                .as_f64()
        })
        .map(|secs| (secs * 1000.0) as i64 - tool_duration_ms as i64);
    let summary = CaseSummary {
        id: entry.id.clone(),
        query: entry.query.clone(),
        ok: true,
        retrieval_ok: retrieval_gate(&entry.query, passages, sources, &observation),
        // Upstream unreachable, not a retrieval-quality miss. Detected from the
        // tool's own structured status so a throttled batch cannot masquerade as
        // a scoring regression.
        transient_failure: observation.contains("code=\"network.failed\"")
            && observation.contains("error_kind=\"transient\""),
        error: None,
        turn_ms,
        tool_duration_ms,
        timed_out: outcome.is_err(),
        context_bytes: observation.len(),
        observation_chars: observation.chars().count(),
        sources,
        passages,
        tool_args_match: received
            .as_ref()
            .map(|c| c.name == script.tool)
            .unwrap_or(false),
        filler_chars: filler.as_ref().map(|f| f.chars).unwrap_or(0),
        filler_ms: filler.as_ref().map(|f| f.duration_ms).unwrap_or(0),
        filler_overrun_ms,
        tts_jobs: render_log.len(),
        audio_clips: clips.len(),
        tts_drained,
        state_transitions: trace.len(),
        assistant_text: turn_text,
        wall_ms: started.elapsed().as_millis() as u64,
    };
    stage_dump::write_json(
        case_dir,
        "case_summary.json",
        &serde_json::to_value(&summary)?,
    )?;
    Ok(summary)
}

/// Reads an integer XML attribute out of the observation for quick reporting.
fn count_attr(observation: &str, key: &str) -> u64 {
    let pat = format!("{key}=\"");
    observation
        .find(&pat)
        .and_then(|s| {
            let rest = &observation[s + pat.len()..];
            rest.find('"').and_then(|e| rest[..e].parse().ok())
        })
        .unwrap_or(0)
}

fn copy_if_exists(from: &Path, to: &Path) -> Result<()> {
    if from.exists() {
        std::fs::copy(from, to)?;
    }
    Ok(())
}

async fn fetch_tool_calls(
    conn: &turso::Connection,
    session_id: i64,
    turn_id: u32,
) -> Result<serde_json::Value> {
    let mut rows = conn
        .query(
            "SELECT tool_name, id, arguments, result, is_error, duration_ms \
             FROM session_tool_calls WHERE session_id = ? AND turn_id = ? ORDER BY created_at ASC;",
            (session_id, turn_id),
        )
        .await?;
    let mut out = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        out.push(serde_json::json!({
            "tool_name": row.get::<String>(0).unwrap_or_default(),
            "tool_call_id": row.get::<String>(1).unwrap_or_default(),
            "arguments": row.get::<String>(2).unwrap_or_default(),
            "result": row.get::<String>(3).unwrap_or_default(),
            "is_error": row.get::<i64>(4).unwrap_or(0) != 0,
            "duration_ms": row.get::<i64>(5).unwrap_or(0),
        }));
    }
    Ok(serde_json::Value::Array(out))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CaseSummary {
    id: String,
    query: String,
    /// Execution plumbing succeeded (turn ran, artifacts written).
    ok: bool,
    /// P0-1: retrieval actually produced answer-bearing evidence. Separate from
    /// `ok` — G3 scored `ok:true` for 0-passage cases and a 42KB JS dump.
    retrieval_ok: bool,
    /// Upstream providers were unreachable (`network.failed/transient`).
    ///
    /// This is infrastructure noise, not a retrieval-quality miss. A batch fired
    /// without pacing trips provider throttling and scores ~20 points lower for no
    /// code reason at all, so it must be counted separately.
    transient_failure: bool,
    error: Option<String>,
    turn_ms: u64,
    tool_duration_ms: u64,
    timed_out: bool,
    context_bytes: usize,
    observation_chars: usize,
    sources: u64,
    passages: u64,
    tool_args_match: bool,
    filler_chars: usize,
    filler_ms: u64,
    /// E3: filler audio length minus tool latency, in ms. Positive means the
    /// filler outlives the tool call it covers (G3 `wall_02`: +126ms).
    /// `None` when no filler clip was captured.
    filler_overrun_ms: Option<i64>,
    tts_jobs: usize,
    audio_clips: usize,
    tts_drained: bool,
    state_transitions: usize,
    assistant_text: String,
    wall_ms: u64,
}

/// P0-1: retrieval-quality gate. Execution success (`ok`) is plumbing;
/// this decides whether the model received anything answer-bearing.
fn retrieval_gate(query: &str, passages: u64, sources: u64, observation: &str) -> bool {
    if passages == 0 || sources == 0 {
        return false;
    }
    if observation.chars().count() < 100 {
        return false;
    }
    // XML status indicating empty or degraded results
    if observation.contains("web_search_status")
        && (observation.contains("code=\"empty_results\"")
            || observation.contains("code=\"deadline.hit\"")
            || observation.contains("code=\"panic_recovered\""))
    {
        return false;
    }
    // `missing_factual_answer` means passages were delivered but none carried the
    // answer the query asked for. That is a real quality miss (mechanism B), not
    // an empty result, so it must not score as success.
    if observation.contains("error_kind=\"missing_factual_answer\"") {
        return false;
    }
    // Degraded plain-text messages carry no evidence, however wordy.
    const DEGRADED_MARKERS: &[&str] = &[
        "No relevant web results could be retrieved",
        "network connection could not be established",
        "exceeded the context budget and were withheld",
    ];
    if observation.starts_with("Web search completed for")
        && DEGRADED_MARKERS.iter().any(|m| observation.contains(m))
    {
        return false;
    }

    // Factual/numerical gate: queries seeking numeric or time lookup must have digits
    let q_lower = query.to_ascii_lowercase();
    let is_numeric_lookup = q_lower.contains("year")
        || q_lower.contains("how many")
        || q_lower.contains("number of")
        || q_lower.contains("when did")
        || q_lower.contains("how much")
        || q_lower.contains("distance")
        || q_lower.contains("elevation")
        || q_lower.contains("sum of")
        || q_lower.contains("range")
        || q_lower.contains("frequency");
    if is_numeric_lookup && !observation.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }

    true
}

impl CaseSummary {
    fn failed(id: &str, error: String) -> Self {
        Self {
            id: id.to_string(),
            query: String::new(),
            ok: false,
            retrieval_ok: false,
            transient_failure: false,
            error: Some(error),
            turn_ms: 0,
            tool_duration_ms: 0,
            timed_out: false,
            context_bytes: 0,
            observation_chars: 0,
            sources: 0,
            passages: 0,
            tool_args_match: false,
            filler_chars: 0,
            filler_ms: 0,
            filler_overrun_ms: None,
            tts_jobs: 0,
            audio_clips: 0,
            tts_drained: false,
            state_transitions: 0,
            assistant_text: String::new(),
            wall_ms: 0,
        }
    }
}

fn write_summary(
    run_dir: &Path,
    results_root: &Path,
    run_id: &str,
    rows: &[CaseSummary],
    wall_s: f64,
) -> Result<()> {
    let ok: Vec<&CaseSummary> = rows.iter().filter(|r| r.ok).collect();
    let mut stages = StageAggregator::new();
    for r in &ok {
        stages.record("turn_ms", r.turn_ms as f64);
        stages.record("tool_duration_ms", r.tool_duration_ms as f64);
        stages.record("wall_ms", r.wall_ms as f64);
    }

    let stat = |f: &dyn Fn(&CaseSummary) -> f64| -> LatencyStats {
        LatencyStats::from_samples(&ok.iter().map(|r| f(r)).collect::<Vec<_>>())
    };
    let turn = stat(&|r| r.turn_ms as f64);
    let tool = stat(&|r| r.tool_duration_ms as f64);

    let mean = |f: &dyn Fn(&CaseSummary) -> f64| -> f64 {
        if ok.is_empty() {
            return 0.0;
        }
        ok.iter().map(|r| f(r)).sum::<f64>() / ok.len() as f64
    };

    // P0-1: retrieval quality is reported separately from execution plumbing.
    let retrieval_ok_count = ok.iter().filter(|r| r.retrieval_ok).count();
    let health_rows = vec![
        format!("cases ok | {}/{}", ok.len(), rows.len()),
        format!(
            "cases retrieval_ok | {}/{} (P0-1: content-bearing evidence, not just plumbing)",
            retrieval_ok_count,
            ok.len()
        ),
        format!(
            "tool args echoed correctly | {}",
            ok.iter().filter(|r| r.tool_args_match).count()
        ),
        format!(
            "turn timed out | {}",
            ok.iter().filter(|r| r.timed_out).count()
        ),
        format!(
            "tts queue drained | {}",
            ok.iter().filter(|r| r.tts_drained).count()
        ),
        format!(
            "cases with an interim filler | {}",
            ok.iter().filter(|r| r.filler_chars > 0).count()
        ),
        format!(
            "cases with audio on disk | {}",
            ok.iter().filter(|r| r.audio_clips > 0).count()
        ),
        format!(
            "cases with zero delivered passages | {}",
            ok.iter().filter(|r| r.passages == 0).count()
        ),
        format!(
            "cases with a single passage | {}",
            ok.iter().filter(|r| r.passages == 1).count()
        ),
    ];

    let per_case: Vec<String> = rows
        .iter()
        .map(|r| {
            format!(
                "`{}` | {} | {} | {} | {} | {} | {} | {} | {}",
                r.id,
                if r.ok { "ok" } else { "FAIL" },
                if r.retrieval_ok { "ok" } else { "FAIL" },
                r.tool_duration_ms,
                r.passages,
                r.sources,
                r.filler_chars,
                r.audio_clips,
                r.filler_overrun_ms
                    .map(|o| format!("{o}ms"))
                    .unwrap_or_else(|| "n/a".to_string()),
            )
        })
        .collect();

    let summary = serde_json::json!({
        "run_id": run_id,
        "wall_seconds": wall_s,
        "cases": rows.len(),
        "ok_cases": ok.len(),
        "retrieval_ok_cases": retrieval_ok_count,
        "turn_latency": turn,
        "tool_latency": tool,
        "mean_observation_chars": mean(&|r| r.observation_chars as f64),
        "mean_passages": mean(&|r| r.passages as f64),
        "mean_sources": mean(&|r| r.sources as f64),
        "mean_filler_chars": mean(&|r| r.filler_chars as f64),
        "cases_with_audio": ok.iter().filter(|r| r.audio_clips > 0).count(),
        "cases_zero_passages": ok.iter().filter(|r| r.passages == 0).count(),
        "cases_single_passage": ok.iter().filter(|r| r.passages == 1).count(),
        "failures": rows.iter().filter_map(|r| r.error.clone()).collect::<Vec<_>>(),
        "per_case": rows,
        "process_rss_bytes": stage_dump::process_rss_bytes(),
        "qa_prompt": "evals/common/qa_prompts.yaml",
    });
    stage_dump::write_json(run_dir, names::SUMMARY_JSON, &summary)?;
    stage_dump::write_latest(results_root, EVAL_NAME, &summary)?;

    let preamble = format!(
        "Run `{run_id}`. {} of {} cases succeeded ({} retrieval_ok). Wall clock {wall_s:.1}s.\n\n\
         The eval input was a scripted tool call served by a mock LLM; the output is the \
         final request body captured in `cases/*/context_llm_saw.json`.\n\n\
         Quality judgement is NOT done here. Read `evals/common/qa_prompts.yaml`, take the \
         prompt matching this eval, substitute the placeholders, and launch a subagent.",
        ok.len(),
        rows.len(),
        retrieval_ok_count
    );

    write_summary_markdown(
        run_dir,
        "Agentic Tool Eval Summary",
        &preamble,
        &[
            (
                "Execution health".to_string(),
                markdown_table(&["check", "value"], &health_rows),
            ),
            (
                "Latency".to_string(),
                markdown_table(
                    &["phase", "n", "mean", "p50", "p95", "max", "min", "sd"],
                    &[
                        format!("turn | {}", turn.to_markdown_cells()),
                        format!("tool | {}", tool.to_markdown_cells()),
                    ],
                ),
            ),
            (
                "Per case".to_string(),
                markdown_table(
                    &[
                        "case",
                        "status",
                        "retrieval",
                        "tool ms",
                        "passages",
                        "sources",
                        "filler chars",
                        "audio clips",
                        "filler overrun",
                    ],
                    &per_case,
                ),
            ),
        ],
    )?;
    Ok(())
}
