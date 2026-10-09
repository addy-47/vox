//! ============================================================================
//! evals/memory-pipeline/main.rs — Vox Memory Pipeline Evaluation Harness
//! ============================================================================
//! Category     : Evaluation Suite
//! Component    : services/memory (Compaction, Ingestion, Consolidation)
//! Execution    : cargo bench --bench memory_pipeline_eval --release -- [FLAGS]
//!
//! Three sequential passes, run in this order per case set:
//!
//!   1. --dump-slices   Plan compaction slice boundaries. No LLM calls.
//!   2. generate_baseline.py   Produce the reference ceiling for each slice.
//!   3. (default)       Run the full pipeline and the three judges.
//!
//! Pass 1 exists because the baseline must consume the same slices the runtime
//! consumes, and the slices must be inspectable before any generation happens.
//! Pass 2 refuses to pick its own boundaries, so the two sides cannot drift.
//! ============================================================================

#[path = "../common/mod.rs"]
mod common;

mod compaction;
mod consolidation;
mod ingestion;

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use clap::Parser;
use common::{
    datasets::load_eval_case,
    db::EvalDbGuard,
    llm_client::{create_pipeline_provider, JudgeClient, RecordingLlmProvider},
    reporting::{create_case_directory, create_run_directory, resolve_output_dir, write_summary_markdown},
    slices::plan_slices,
    stage_dump::{build_run_manifest, write_json},
    verdicts::ratio_str,
};
use compaction::{dump_case_slices, judge_compaction, run_slices_and_persist};
use consolidation::evaluate_consolidation_stage;
use ingestion::evaluate_ingestion_stage;

const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434/v1";
const DEFAULT_MODEL: &str = "gemma4:12b";

#[derive(Parser, Debug)]
#[command(
    name = "memory_eval",
    about = "Vox Memory Pipeline Evaluation Harness (compaction -> ingestion -> consolidation)"
)]
struct CliArgs {
    /// Number of sequential cases to evaluate from sandbox/datasets/eval-sessions/ (1..14)
    #[arg(long, default_value_t = 1)]
    cases: usize,

    /// Plan slice boundaries and write them to disk. Makes no LLM calls.
    #[arg(long, default_value_t = false)]
    dump_slices: bool,

    /// Run compaction only, skipping ingestion and consolidation.
    #[arg(long, default_value_t = false)]
    compaction_only: bool,

    /// Pipeline execution LLM endpoint (Ollama or OpenAI-compatible)
    #[arg(long, default_value = DEFAULT_OLLAMA_URL)]
    pipeline_url: String,

    /// Pipeline execution LLM model slug
    #[arg(long, default_value = DEFAULT_MODEL)]
    pipeline_model: String,

    /// Judge model slug
    #[arg(long, default_value = DEFAULT_MODEL)]
    judge_model: String,

    /// Judge endpoint (OpenAI-compatible /chat/completions). Empty means local Ollama.
    #[arg(long, default_value = "")]
    judge_url: String,

    /// Judge API key. Omit for a local judge; required only for a cloud gateway.
    #[arg(long)]
    judge_key: Option<String>,

    /// Seed applied to both the pipeline and the judge. Fixed seeds make a run
    /// exactly reproducible; varying them across runs measures the spread.
    #[arg(long)]
    seed: Option<u64>,

    /// Optional path to an existing evaluation database
    #[arg(long)]
    db: Option<PathBuf>,

    /// Directory holding `baseline/slice_NN.json` files from `generate_baseline.py`.
    ///
    /// Defaults to the run directory itself. Point it at the `--dump-slices` run
    /// directory when the passes were run separately: baselines are a function of
    /// (case, slice, model, seed), not of the run id, so they are reusable.
    #[arg(long)]
    baseline_dir: Option<PathBuf>,

    /// Destination directory for evaluation reports
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Re-parse saved judge responses with the current schemas and recount.
    ///
    /// Reads each case directory's `judge_traces.json`, re-validates every response
    /// with today's verdict contracts, rewrites the `*_verdict.json` files, and
    /// prints corrected totals. Used when a schema fix lands after a run: the
    /// responses are preserved in the traces, so no LLM call is repeated and no
    /// pipeline stage re-executes. Takes the run directory as `--output-dir`
    /// (which must contain case directories directly).
    #[arg(long, default_value_t = false)]
    reparse: bool,
}

/// Per-case roll-up written into the run summary. Every field is a count produced
/// by typed code, never a number read out of a judge's prose.
#[derive(Default)]
struct CaseTotals {
    slices: usize,
    fallback_slices: usize,
    runtime_facts: usize,
    baseline_facts: usize,
    compaction_judge_parsed: usize,
    compaction_judge_invalid: usize,
    phantom_verdicts: usize,
    attribution_flags: usize,
    personal_cited: usize,
    personal_total: usize,
    compaction_matched: usize,
    compaction_novel: usize,
    compaction_ungrounded: usize,
    baseline_missed: usize,
    merges: usize,
    stage1_merges: usize,
    stage2_merges: usize,
    merges_incorrect: usize,
    near_misses: usize,
    near_miss_should_merge: usize,
    observations_supplied: usize,
    observations_dropped: usize,
    deletes: usize,
    deletes_unjustified: usize,
    ungrounded_spans: usize,
    inherited_concerns: usize,
}

fn baseline_path(baseline_root: &Path, case_stem: &str, slice_index: usize) -> PathBuf {
    baseline_root
        .join(case_stem)
        .join("baseline")
        .join(format!("slice_{:02}.json", slice_index))
}

fn load_baseline(baseline_root: &Path, case_stem: &str, slice_index: usize) -> Option<serde_json::Value> {
    let path = baseline_path(baseline_root, case_stem, slice_index);
    let body = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&body).ok()
}

/// Reads the baseline provenance written by `generate_baseline.py` (`_meta.json`).
/// All cases in one baseline directory must agree; a disagreement is reported and
/// the first meta wins, so a mixed-source ceiling can never pass silently.
fn read_baseline_meta(baseline_root: &Path) -> serde_json::Value {
    let mut found: Vec<serde_json::Value> = Vec::new();
    let dirs: Vec<PathBuf> = std::fs::read_dir(baseline_root)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    for dir in &dirs {
        let meta_path = dir.join("baseline").join("_meta.json");
        if let Ok(body) = std::fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&body) {
                found.push(meta);
            }
        }
    }
    let first = found.first().cloned().unwrap_or_else(|| {
        serde_json::json!({"backend": "unknown", "model": "unknown"})
    });
    if found.iter().any(|m| m != &first) {
        log::warn!(
            "[Eval] Baseline _meta.json files disagree within {}; using the first. Mixed-source ceilings are not supported.",
            baseline_root.display()
        );
    }
    first
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    if args.reparse {
        return reparse_run(&args);
    }

    let run_id = common::reporting::generate_run_id();
    let base_dir = resolve_output_dir(args.output_dir.as_deref(), "memory-pipeline");
    let run_dir = create_run_directory(&base_dir, &run_id)?;

    let judge_url = if args.judge_url.is_empty() {
        "http://127.0.0.1:11434/api/chat".to_string()
    } else {
        args.judge_url.clone()
    };
    let judge_local = args.judge_url.is_empty() && args.judge_key.is_none();

    println!("================================================================================");
    println!("Vox Memory Pipeline Evaluation");
    println!("================================================================================");
    println!("  Cases          : {}", args.cases);
    println!("  Mode           : {}", if args.dump_slices { "dump-slices (no LLM calls)" } else { "full pipeline + judges" });
    println!("  Pipeline       : {} @ {}", args.pipeline_model, args.pipeline_url);
    println!("  Judge          : {} @ {}", args.judge_model, if judge_local { "local ollama" } else { &judge_url });
    println!("  Seed           : {:?}", args.seed);
    println!("  Run Directory  : {}", run_dir.display());
    println!("================================================================================");

    let baseline_root: PathBuf = args
        .baseline_dir
        .clone()
        .unwrap_or_else(|| run_dir.clone());
    let baseline_meta = read_baseline_meta(&baseline_root);
    println!(
        "  Baseline       : {} @ {}",
        baseline_meta.get("model").and_then(|v| v.as_str()).unwrap_or("unknown"),
        baseline_meta.get("backend").and_then(|v| v.as_str()).unwrap_or("unknown")
    );

    let manifest = build_run_manifest(
        "memory-pipeline",
        &run_id,
        &std::env::args().collect::<Vec<_>>(),
        serde_json::json!({
            "cases": args.cases,
            "dump_slices": args.dump_slices,
            "compaction_only": args.compaction_only,
            "pipeline_url": args.pipeline_url,
            "pipeline_model": args.pipeline_model,
            "judge_model": args.judge_model,
            "judge_url": judge_url,
            "judge_is_local": judge_local,
            "seed": args.seed,
            "baseline_dir": args.baseline_dir.as_ref().map(|p| p.display().to_string()),
            "baseline_meta": baseline_meta,
        }),
    )?;
    write_json(&run_dir, common::stage_dump::names::MANIFEST, &manifest)?;

    let (db_path, _is_shared_db) = if let Some(ref custom_db) = args.db {
        if !custom_db.exists() {
            return Err(anyhow!("Provided database file {:?} does not exist", custom_db));
        }
        (custom_db.clone(), true)
    } else {
        (run_dir.join("eval_vox.db"), false)
    };

    let eval_db = EvalDbGuard::new(&db_path).await?;
    let raw_provider = create_pipeline_provider(&args.pipeline_url, &args.pipeline_model, None);
    let recording_provider = RecordingLlmProvider::new(raw_provider, args.pipeline_model.clone());
    let judge = JudgeClient::with_endpoint(args.judge_key.clone(), args.judge_model.clone(), &judge_url)
        .with_seed(args.seed);

    // The candidate model identity reaches the request builder (catalog lookups,
    // output constraints, seed), not just the transport. Without this the builder
    // silently evaluates every candidate under the embedded default's spec.
    let mut pipeline_settings = vox_lib::services::llm::LlmSettings {
        active: vox_lib::services::llm::LlmActiveProvider::Server,
        seed: args.seed,
        ..Default::default()
    };
    pipeline_settings.server.model = args.pipeline_model.clone();
    pipeline_settings.server.base_url = args.pipeline_url.clone();

    let mut totals = CaseTotals::default();
    let mut case_rows: Vec<String> = Vec::new();
    let mut invalid_cases: Vec<String> = Vec::new();

    for case_idx in 1..=args.cases {
        let (case_name, turns) = load_eval_case(case_idx)?;
        let case_stem = case_name.strip_suffix(".json").unwrap_or(&case_name).to_string();
        let case_dir = create_case_directory(&run_dir, &case_stem)?;
        let session_id = case_idx as i64 * 1000;

        println!("\n---- Case {:02}: {} ({} turns) ----", case_idx, case_stem, turns.len());

        // ---- Pass 1: slice planning -------------------------------------------
        let slices = plan_slices(&turns);
        let planned = dump_case_slices(&turns, &case_dir)?;
        println!(
            "  slices planned: {} ({} tokens total)",
            planned,
            common::slices::case_total_tokens(&turns)
        );
        for s in &slices {
            println!(
                "    slice {:02}: turns {}..{} ({} turns, {} tokens, {:.0}% of budget)",
                s.slice_index, s.from_turn, s.to_turn, s.turn_count, s.dialogue_tokens,
                s.utilization_at_trigger * 100.0
            );
        }

        if args.dump_slices {
            case_rows.push(format!("{} | {} slices dumped | `{}`", case_stem, planned, case_stem));
            totals.slices += planned;
            continue;
        }

        // ---- Pass 3a: compaction runtime -------------------------------------
        let (documents, slice_telemetry) = run_slices_and_persist(
            &eval_db, session_id, &case_stem, &slices, &recording_provider, &case_dir,
            &pipeline_settings,
        )
        .await?;
        totals.attribution_flags += slice_telemetry.attribution_flags;
        totals.personal_cited += slice_telemetry.personal_cited;
        totals.personal_total += slice_telemetry.personal_total;

        let mut case_baseline_facts = 0;
        // Runtime fact counts come from the documents themselves, never from the
        // verdicts: a slice whose judge response fails to parse still produced
        // facts, and the summary must not report them as zero.
        let case_runtime_facts: usize = documents
            .iter()
            .map(|(d, _)| common::verdicts::flatten_compaction(d).len())
            .sum();
        let case_fallback_slices: usize = documents.iter().filter(|(_, f)| *f).count();
        for (i, (doc, _)) in documents.iter().enumerate() {
            let slice = &slices[i];
            let baseline = load_baseline(&baseline_root, &case_stem, slice.slice_index);
            if let Some(b) = &baseline {
                case_baseline_facts += common::verdicts::flatten_compaction(b).len();
            }
            let verdict = judge_compaction(
                &judge, &case_dir, &case_stem, slice, doc, baseline.as_ref(),
            )
            .await?;

            match &verdict {
                common::verdicts::JudgeStatus::Parsed(v) => {
                    let runtime_n = common::verdicts::flatten_compaction(doc).len();
                    let baseline_n = baseline
                        .as_ref()
                        .map(|b| common::verdicts::flatten_compaction(b).len())
                        .unwrap_or(0);
                    let validation =
                        common::verdicts::validate_compaction_verdict(v, runtime_n, baseline_n);
                    totals.phantom_verdicts += validation.phantom_runtime
                        + validation.phantom_baseline;
                    if validation.phantom_runtime > 0 || validation.phantom_baseline > 0 {
                        println!(
                            "    slice {:02}: {} phantom judge indices excluded from counts",
                            slice.slice_index,
                            validation.phantom_runtime + validation.phantom_baseline
                        );
                    }
                    // Count only verdicts that address real facts. A phantom index
                    // is a judge hallucination, not a pipeline outcome.
                    let mut valid = v.clone();
                    valid.runtime_facts.retain(|f| {
                        f.index >= 1
                            && f.index <= runtime_n
                            && f.baseline_index.is_none_or(|b| b >= 1 && b <= baseline_n)
                    });
                    valid
                        .baseline_missed
                        .retain(|m| m.index >= 1 && m.index <= baseline_n);
                    let c = common::verdicts::count_compaction(&valid, baseline_n);
                    totals.compaction_matched += c.matched_baseline;
                    totals.compaction_novel += c.novel_but_valid;
                    totals.compaction_ungrounded += c.ungrounded;
                    totals.baseline_missed += c.baseline_missed;
                    totals.compaction_judge_parsed += 1;
                }
                common::verdicts::JudgeStatus::Invalid { reason } => {
                    totals.compaction_judge_invalid += 1;
                    invalid_cases.push(format!("{}/compaction/{}", case_stem, slice.slice_index));
                    println!("    slice {:02}: INVALID verdict — {}", slice.slice_index, reason);
                }
            }
        }
        totals.slices += documents.len();
        totals.fallback_slices += case_fallback_slices;
        totals.runtime_facts += case_runtime_facts;
        totals.baseline_facts += case_baseline_facts;
        println!(
            "  compaction: {} runtime facts vs {} baseline facts ({} valid, {} invalid verdicts)",
            case_runtime_facts, case_baseline_facts,
            totals.compaction_judge_parsed, totals.compaction_judge_invalid
        );

        if args.compaction_only {
            recording_provider.flush_case_traces(&case_dir)?;
            case_rows.push(format!(
                "{} | compaction only: {} facts vs {} baseline | `{}`",
                case_stem, case_runtime_facts, case_baseline_facts, case_stem
            ));
            continue;
        }

        // ---- Pass 3b: ingestion ----------------------------------------------
        let ing = evaluate_ingestion_stage(&eval_db, &case_stem, &judge, &case_dir).await?;
        totals.merges += ing.counts.merges_total;
        totals.stage1_merges += ing.stage1_merges;
        totals.stage2_merges += ing.stage2_merges;
        totals.merges_incorrect += ing.counts.merges_incorrect;
        totals.near_misses += ing.counts.near_misses_total;
        totals.near_miss_should_merge += ing.counts.near_misses_should_have_merged;
        println!(
            "  ingestion: stg1 {} processed / stg2 {} processed ({} inserted); {} merges ({} incorrect), {} near-misses ({} should have merged)",
            ing.stage1_processed, ing.stage2_processed, ing.stage2_inserted,
            ing.counts.merges_total, ing.counts.merges_incorrect,
            ing.counts.near_misses_total, ing.counts.near_misses_should_have_merged
        );
        if !ing.judge_parsed {
            invalid_cases.push(format!("{}/ingestion", case_stem));
        }

        // ---- Pass 3c: consolidation ------------------------------------------
        let con = evaluate_consolidation_stage(
            &eval_db, &case_stem, &recording_provider, &judge, &case_dir,
            &pipeline_settings,
        )
        .await?;
        totals.observations_supplied += con.counts.observations_total;
        totals.observations_dropped += con.counts.observations_dropped;
        totals.deletes += con.counts.deletes_total;
        totals.deletes_unjustified += con.counts.deletes_unjustified;
        totals.ungrounded_spans += con.counts.ungrounded_spans;
        totals.inherited_concerns += con.counts.inherited_concerns;
        println!(
            "  consolidation: {} v{}->v{} ({} sections, {} blocks) — {} observations, {} dropped, {} deletes ({} unjustified), {} ungrounded spans",
            con.mode, con.prior_version, con.new_version, con.sections_count, con.blocks_count,
            con.counts.observations_total, con.counts.observations_dropped,
            con.counts.deletes_total, con.counts.deletes_unjustified, con.counts.ungrounded_spans
        );
        if !con.judge_parsed {
            invalid_cases.push(format!("{}/consolidation", case_stem));
        }

        recording_provider.flush_case_traces(&case_dir)?;

        case_rows.push(format!(
            "{} | c:{} facts / b:{} baseline | i:{} merges | n:{} obs ({} dropped) | `{}`",
            case_stem, case_runtime_facts, case_baseline_facts,
            ing.counts.merges_total, con.counts.observations_total,
            con.counts.observations_dropped, case_stem
        ));
    }

    // ---- Run summary ---------------------------------------------------------
    let mut sections: Vec<(String, String)> = Vec::new();

    if args.dump_slices {
        sections.push((
            "Slices planned".to_string(),
            format!("Total slices across {} case(s): **{}**", args.cases, totals.slices),
        ));
        sections.push((
            "Next step".to_string(),
            "Run `python3 evals/tools/generate_baseline.py --run-dir <run_dir>` before the full pass."
                .to_string(),
        ));
    } else {
        sections.push((
            "Compaction — runtime vs Gemma baseline ceiling".to_string(),
            format!(
                "- Runtime facts extracted: **{}**\n\
                 - Baseline facts (reference): **{}**\n\
                 - Runtime facts matched to baseline: {}\n\
                 - Runtime facts novel but supported: {}\n\
                 - Runtime facts **ungrounded**: {}\n\
                 - Runtime facts flagged by deterministic attribution check: {}\n\
                 - Personal facts carrying a [turn N] citation: {}\n\
                 - Baseline facts the runtime **missed**: {}\n\
                 - Accepted runtime facts (matched + novel): {}\n\
                 - Slices that fell back to raw text: {}\n\
                 - Verdict parse rate: {} valid / {} invalid\n\
                 - Phantom judge indices excluded from counts: {}\n\
                 - BASELINE CAVEAT: the baseline contains attribution violations of its own (assistant-derived residence/roommate facts); matched measures agreement with the ceiling, not grounded truth. Correctness comes only from the ungrounded/novel split, which is checked against the turns.",
                totals.runtime_facts,
                totals.baseline_facts,
                ratio_str(totals.compaction_matched, totals.runtime_facts),
                ratio_str(totals.compaction_novel, totals.runtime_facts),
                totals.ungrounded_flag(),
                totals.attribution_flags,
                crate::common::verdicts::ratio_str(totals.personal_cited, totals.personal_total),
                ratio_str(totals.baseline_missed, totals.baseline_facts),
                totals.compaction_matched + totals.compaction_novel,
                totals.fallback_slices,
                totals.compaction_judge_parsed,
                totals.compaction_judge_invalid,
                totals.phantom_verdicts,
            ),
        ));

        sections.push((
            "Ingestion — deduplication decisions".to_string(),
            format!(
                "- Merges performed: **{}** (stage 1: {}, stage 2: {})\n\
                 - Merges judged **incorrect** (information loss): {}\n\
                 - Merge precision: {}\n\
                 - Near-miss pairs reviewed: **{}**\n\
                 - Near-misses that **should have merged** (missed duplicates): {}\n\
                 - Threshold audited: Stage 1 Jaccard 1.0, Stage 2 cosine 0.95",
                totals.merges,
                totals.stage1_merges,
                totals.stage2_merges,
                totals.merges_incorrect,
                ratio_str(
                    totals.merges.saturating_sub(totals.merges_incorrect),
                    totals.merges
                ),
                totals.near_misses,
                totals.near_miss_should_merge,
            ),
        ));

        sections.push((
            "Consolidation — coverage, deletions, grounding".to_string(),
            format!(
                "- Observations supplied: **{}**\n\
                 - Observations **dropped**: {}\n\
                 - Observation coverage: {}\n\
                 - Delete operations: **{}**\n\
                 - Deletes judged **unjustified**: {}\n\
                 - Ungrounded spans introduced by this pass: **{}**\n\
                 - Inherited concerns (pre-existing blocks no observation supports, reported but not charged to this pass): **{}**\n\
                 - Note: consolidation runs `auto_apply`, so deletions were committed. \
                   Production defaults to `manual_review`, which would stage them instead.",
                totals.observations_supplied,
                totals.observations_dropped,
                ratio_str(
                    totals.observations_supplied.saturating_sub(totals.observations_dropped),
                    totals.observations_supplied
                ),
                totals.deletes,
                totals.deletes_unjustified,
                totals.ungrounded_spans,
                totals.inherited_concerns,
            ),
        ));

        sections.push((
            "Judge validity".to_string(),
            if invalid_cases.is_empty() {
                "Every judge response parsed into its schema.".to_string()
            } else {
                format!(
                    "**{}** judge call(s) produced no usable verdict. These contribute to no \
                     metric and are not passes:\n\n{}",
                    invalid_cases.len(),
                    invalid_cases.iter().map(|c| format!("- {}", c)).collect::<Vec<_>>().join("\n")
                )
            },
        ));
    }

    sections.push((
        "Cases".to_string(),
        common::reporting::markdown_table(&["case", "detail", "dir"], &case_rows),
    ));

    let preamble = format!(
        "Run `{}` · git `{}` · {} case(s) · seed `{:?}` · pipeline `{}` · judge `{}` · baseline `{}@{}`.\n\n\
         Every number below is a count produced by typed code from database state or a \
         schema-validated judge verdict. No figure is read out of judge prose.",
        run_id,
        manifest["git"]["sha"],
        args.cases,
        args.seed,
        args.pipeline_model,
        args.judge_model,
        baseline_meta.get("model").and_then(|v| v.as_str()).unwrap_or("unknown"),
        baseline_meta.get("backend").and_then(|v| v.as_str()).unwrap_or("unknown"),
    );

    write_summary_markdown(
        &run_dir,
        "Memory Pipeline Evaluation Summary",
        &preamble,
        &sections,
    )?;

    // Machine-readable scorecard. This file — not the markdown — is what ranks
    // candidate models against each other. One flat object, every value a count
    // or a ratio with its denominator alongside it. No composite score: ranking
    // is done per metric, with red-flag counts compared first.
    let scorecard = serde_json::json!({
        "run_id": run_id,
        "git_sha": manifest["git"]["sha"],
        "cases": args.cases,
        "seed": args.seed,
        "pipeline_model": args.pipeline_model,
        "judge_model": args.judge_model,
        "baseline_meta": baseline_meta,
        "compaction": {
            "runtime_facts": totals.runtime_facts,
            "baseline_facts": totals.baseline_facts,
            "matched": totals.compaction_matched,
            "matched_rate": rate(totals.compaction_matched, totals.runtime_facts),
            "novel": totals.compaction_novel,
            "ungrounded": totals.compaction_ungrounded,
            "ungrounded_rate": rate(totals.compaction_ungrounded, totals.runtime_facts),
            "attribution_flags": totals.attribution_flags,
            "personal_cited": totals.personal_cited,
            "personal_total": totals.personal_total,
            "citation_rate": rate(totals.personal_cited, totals.personal_total),
            "baseline_missed": totals.baseline_missed,
            "miss_rate": rate(totals.baseline_missed, totals.baseline_facts),
            "fallback_slices": totals.fallback_slices,
            "judge_parsed": totals.compaction_judge_parsed,
            "judge_invalid": totals.compaction_judge_invalid,
            "phantom_indices_excluded": totals.phantom_verdicts,
        },
        "ingestion": {
            "merges": totals.merges,
            "stage1_merges": totals.stage1_merges,
            "stage2_merges": totals.stage2_merges,
            "merges_incorrect": totals.merges_incorrect,
            "merge_precision": rate(
                totals.merges.saturating_sub(totals.merges_incorrect),
                totals.merges
            ),
            "near_misses": totals.near_misses,
            "should_have_merged": totals.near_miss_should_merge,
        },
        "consolidation": {
            "observations": totals.observations_supplied,
            "dropped": totals.observations_dropped,
            "coverage_rate": rate(
                totals.observations_supplied.saturating_sub(totals.observations_dropped),
                totals.observations_supplied
            ),
            "deletes": totals.deletes,
            "deletes_unjustified": totals.deletes_unjustified,
            "ungrounded_spans": totals.ungrounded_spans,
            "inherited_concerns": totals.inherited_concerns,
        },
        "invalid_cases": invalid_cases,
    });
    std::fs::write(
        run_dir.join("scorecard.json"),
        serde_json::to_string_pretty(&scorecard)?,
    )
    .map_err(|e| anyhow!("Failed to write scorecard: {}", e))?;

    println!("\n================================================================================");
    println!("Run ID        : {}", run_id);
    println!("Run Directory : {}", run_dir.display());
    println!("Preserved DB  : {}", db_path.display());
    println!("================================================================================");

    Ok(())
}

impl CaseTotals {
    fn ungrounded_flag(&self) -> String {
        if self.compaction_ungrounded == 0 {
            "0".to_string()
        } else {
            format!("**{}**", self.compaction_ungrounded)
        }
    }
}

/// Ratio in [0,1] or null when the denominator is zero. A zero denominator is
/// never 0.0 or 1.0.
fn rate(numerator: usize, denominator: usize) -> Option<f64> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f64 / denominator as f64)
    }
}

/// Re-validates saved judge responses with the current verdict contracts.
///
/// For every case directory directly inside `--output-dir`, reads
/// `judge_traces.json` and re-parses each response. Verdict files are rewritten
/// only when the re-parse succeeds where the stored verdict is missing or
/// invalid — a successful re-parse never overwrites a parsed verdict, so this
/// mode can only rescue lost verdicts, never alter counted ones. Prints the
/// corrected per-case counts for the run summary.
fn reparse_run(args: &CliArgs) -> Result<()> {
    use common::verdicts::{
        count_consolidation, count_ingestion, parse_verdict, CompactionVerdict,
        ConsolidationVerdict, IngestionVerdict, JudgeStatus,
    };

    let run_dir = args.output_dir.as_ref().ok_or_else(|| {
        anyhow!("--reparse needs --output-dir pointing at the run directory to repair")
    })?;
    println!("Re-parsing saved judge responses in {}", run_dir.display());

    let mut cases = CaseTotals::default();
    let mut repaired: Vec<String> = Vec::new();
    let mut still_invalid: Vec<String> = Vec::new();

    let mut case_dirs: Vec<PathBuf> = std::fs::read_dir(run_dir)
        .map_err(|e| anyhow!("Failed to list {}: {}", run_dir.display(), e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    case_dirs.sort();

    for case_dir in &case_dirs {
        let case = case_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let traces_path = case_dir.join("judge_traces.json");
        let body = match std::fs::read_to_string(&traces_path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let traces: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap_or_default();

        for entry in &traces {
            let stage = entry["stage"].as_str().unwrap_or("");
            let response = entry["response"].as_str().unwrap_or("");
            if response.trim().is_empty() {
                continue;
            }
            if stage == "consolidation" {
                let status: JudgeStatus<ConsolidationVerdict> = parse_verdict(response);
                match status {
                    JudgeStatus::Parsed(v) => {
                        let counts = count_consolidation(&v);
                        let path = case_dir.join("consolidation_verdict.json");
                        let mut payload: serde_json::Value =
                            std::fs::read_to_string(&path)
                                .ok()
                                .and_then(|b| serde_json::from_str(&b).ok())
                                .unwrap_or_else(|| serde_json::json!({}));
                        let was_invalid =
                            payload.get("status") != Some(&serde_json::json!("parsed"));
                        payload["status"] = serde_json::json!("parsed");
                        payload["verdict"] = serde_json::to_value(&v)?;
                        payload["counts"] = serde_json::to_value(&counts)?;
                        payload["reparsed"] = serde_json::json!(true);
                        std::fs::write(&path, serde_json::to_string_pretty(&payload)?)?;
                        cases.observations_supplied += counts.observations_total;
                        cases.observations_dropped += counts.observations_dropped;
                        cases.deletes += counts.deletes_total;
                        cases.deletes_unjustified += counts.deletes_unjustified;
                        cases.ungrounded_spans += counts.ungrounded_spans;
                        cases.inherited_concerns += counts.inherited_concerns;
                        if was_invalid {
                            repaired.push(format!("{}/consolidation", case));
                        }
                    }
                    JudgeStatus::Invalid { reason } => {
                        still_invalid.push(format!("{}/consolidation: {}", case, reason));
                    }
                }
            } else if stage == "ingestion" {
                let status: JudgeStatus<IngestionVerdict> = parse_verdict(response);
                match status {
                    JudgeStatus::Parsed(v) => {
                        let counts = count_ingestion(&v);
                        let path = case_dir.join("ingestion_verdict.json");
                        let mut payload: serde_json::Value =
                            std::fs::read_to_string(&path)
                                .ok()
                                .and_then(|b| serde_json::from_str(&b).ok())
                                .unwrap_or_else(|| serde_json::json!({}));
                        let was_invalid =
                            payload.get("status") != Some(&serde_json::json!("parsed"));
                        payload["status"] = serde_json::json!("parsed");
                        payload["verdict"] = serde_json::to_value(&v)?;
                        payload["counts"] = serde_json::to_value(&counts)?;
                        payload["reparsed"] = serde_json::json!(true);
                        std::fs::write(&path, serde_json::to_string_pretty(&payload)?)?;
                        cases.merges += counts.merges_total;
                        cases.merges_incorrect += counts.merges_incorrect;
                        cases.near_misses += counts.near_misses_total;
                        cases.near_miss_should_merge += counts.near_misses_should_have_merged;
                        if was_invalid {
                            repaired.push(format!("{}/ingestion", case));
                        }
                    }
                    JudgeStatus::Invalid { reason } => {
                        still_invalid.push(format!("{}/ingestion: {}", case, reason));
                    }
                }
            } else if stage.starts_with("compaction") {
                let status: JudgeStatus<CompactionVerdict> = parse_verdict(response);
                match status {
                    JudgeStatus::Parsed(v) => {
                        let counts =
                            common::verdicts::count_compaction(&v, 0);
                        cases.compaction_matched += counts.matched_baseline;
                        cases.compaction_novel += counts.novel_but_valid;
                        cases.compaction_ungrounded += counts.ungrounded;
                        cases.baseline_missed += counts.baseline_missed;
                        cases.compaction_judge_parsed += 1;
                    }
                    JudgeStatus::Invalid { reason } => {
                        cases.compaction_judge_invalid += 1;
                        still_invalid.push(format!("{}/{}: {}", case, stage, reason));
                    }
                }
            }
        }
    }

    println!("\n---- reparse totals (from saved responses only) ----");
    println!(
        "compaction matched/novel/ungrounded/missed: {}/{}/{}/{}",
        cases.compaction_matched,
        cases.compaction_novel,
        cases.compaction_ungrounded,
        cases.baseline_missed
    );
    println!(
        "ingestion merges/incorrect/near-miss/should-merge: {}/{}/{}/{}",
        cases.merges, cases.merges_incorrect, cases.near_misses, cases.near_miss_should_merge
    );
    println!(
        "consolidation obs/dropped/deletes/unjustified/spans/inherited: {}/{}/{}/{}/{}/{}",
        cases.observations_supplied,
        cases.observations_dropped,
        cases.deletes,
        cases.deletes_unjustified,
        cases.ungrounded_spans,
        cases.inherited_concerns
    );
    println!("repaired verdict files:");
    for r in &repaired {
        println!("  + {}", r);
    }
    if repaired.is_empty() {
        println!("  (none — everything already parsed)");
    }
    if !still_invalid.is_empty() {
        println!("still invalid:");
        for s in &still_invalid {
            println!("  - {}", s);
        }
    }
    Ok(())
}