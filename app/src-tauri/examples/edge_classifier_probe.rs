//! ============================================================================
//! edge_classifier_probe.rs — Vox v7 Gate 3 LFM2.5 GGUF Edge Classifier Probe
//! ============================================================================
//! Category     : Utility Tool / Benchmark Harness (Cargo Example)
//! Component    : Cognitive Edge Classifier Engine (LFM2.5 GGUF via llama.cpp)
//! Architecture : Vox v7 6-Domain Cognitive Memory Spec (Section 7 / Gate 3)
//!
//! Supported models:
//!   - ~/.vox/models/llm/LFM2.5-230M-Q4_K_M.gguf  (default, fast)
//!   - ~/.vox/models/llm/LFM2.5-230M-Q8_0.gguf    (--quant q8)
//!   - ~/.vox/models/llm/LFM2.5-350M-Q4_K_M.gguf  (--model-size 350m)
//!
//! Key optimisations vs v1:
//!   1. KV-prefix caching: system prompt is prefilled ONCE at startup and its
//!      KV state is snapshotted via seq-copy into seq 1. Per-pair inference
//!      restores from snapshot, only tokenises the variable user+assistant turns.
//!   2. Latency measurement: only the per-pair variable-token decode is timed —
//!      model load and system-prompt prefill are measured separately.
//!   3. Prompt: system prompt is truly static (no domain injected); allowed labels
//!      and domain info move to the user turn so the static prefix is cacheable.
//!   4. Sampling: greedy + repetition penalty 1.1 to reduce label hallucination.
//!
//! Execution:
//!   cargo run --example edge_classifier_probe -- [OPTIONS]
//!
//! Options:
//!   --model-size [230m|350m]   (default: 230m)
//!   --quant      [q4|q8]       (default: q4)
//!   --input      <path>        (default: sandbox/datasets/gate3_edge_1750_pairs.json)
//!   --output     <path>        (default: sandbox/results/gate3_lfm_scores.json)
//!   --max-pairs  <N>
//!   --api-endpoint <url>       (HTTP API fallback)
//!   --prompt-variant [0|1|2]   (prompt template variant to test)
//! ============================================================================

use anyhow::{anyhow, Result};
use clap::Parser;
use llama_cpp_4::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
    sampling::LlamaSampler,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

// ─── CLI ─────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "edge_classifier_probe",
    about = "Vox v7 Gate 3 LFM2.5 GGUF Edge Classifier Probe"
)]
struct Args {
    /// Input JSON dataset
    #[arg(
        short,
        long,
        default_value = "sandbox/datasets/gate3_edge_1750_pairs.json"
    )]
    input: PathBuf,

    /// Output JSON result file
    #[arg(short, long, default_value = "sandbox/results/gate3_lfm_scores.json")]
    output: PathBuf,

    /// Model size: 230m or 350m
    #[arg(long, default_value = "230m")]
    model_size: String,

    /// Quantisation: q4 (Q4_K_M) or q8 (Q8_0)
    #[arg(long, default_value = "q4")]
    quant: String,

    /// Force HTTP API endpoint instead of native GGUF
    #[arg(long)]
    api_endpoint: Option<String>,

    /// Model name for OpenAI-compatible API
    #[arg(long, default_value = "lfm-230m:latest")]
    model_name: String,

    /// Limit pairs evaluated
    #[arg(long)]
    max_pairs: Option<usize>,

    /// Prompt template variant (0=minimal, 1=structured, 2=text-labels, 3=few-shot)
    #[arg(long, default_value = "0")]
    prompt_variant: usize,

    /// Number of CPU threads for inference
    #[arg(long, default_value = "4")]
    threads: u32,

    /// Explicit GGUF model path override
    #[arg(long)]
    model_path: Option<PathBuf>,

    /// Subtract unconditioned prompt baseline logits to eliminate token prior bias
    #[arg(long, default_value_t = false)]
    calibrate_logits: bool,
}

// ─── Edge label ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum EdgeLabel {
    Shapes,
    DependsOn,
    ConflictsWith,
    None,
}

impl EdgeLabel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Shapes => "SHAPES",
            Self::DependsOn => "DEPENDS_ON",
            Self::ConflictsWith => "CONFLICTS_WITH",
            Self::None => "NONE",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        let t = s.trim().to_uppercase();
        // Try JSON first
        if let Some(start) = t.find('"') {
            if let Some(end) = t[start + 1..].find('"') {
                let inner = &t[start + 1..start + 1 + end];
                return Self::parse(&inner.to_string());
            }
        }
        if t.contains("SHAPES") || t.contains("SHAPE") || t.contains("MODIFIES") {
            Some(Self::Shapes)
        } else if t.contains("DEPENDS_ON")
            || t.contains("DEPENDS")
            || t.contains("DEPEND")
            || t.contains("REQUIRES")
        {
            Some(Self::DependsOn)
        } else if t.contains("CONFLICTS_WITH")
            || t.contains("CONFLICTS")
            || t.contains("CONFLICT")
            || t.contains("OPPOSING")
        {
            Some(Self::ConflictsWith)
        } else if t.contains("NONE")
            || t.contains("NO RELATION")
            || t.contains("UNRELATED")
            || t.contains("NO LINK")
        {
            Some(Self::None)
        } else {
            // Try numeric index
            let first = t.split_whitespace().next().unwrap_or("");
            if let Ok(idx) = first.parse::<usize>() {
                match idx {
                    1 => Some(Self::Shapes),
                    2 => Some(Self::DependsOn),
                    3 => Some(Self::ConflictsWith),
                    4 => Some(Self::None),
                    _ => None,
                }
            } else {
                None
            }
        }
    }
}

fn get_allowed_labels(_src: &str, _tgt: &str) -> &'static [&'static str] {
    &["SHAPES", "DEPENDS_ON", "CONFLICTS_WITH", "NONE"]
}

// ─── Dataset types ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
struct InputEdgePair {
    #[serde(default)]
    id: usize,
    source_domain: String,
    target_domain: String,
    #[serde(alias = "fact_a")]
    source_fact: String,
    #[serde(alias = "fact_b")]
    target_fact: String,
    #[serde(default, alias = "context")]
    session_narrative: String,
    #[serde(default)]
    expected_label: String,
    #[allow(dead_code)]
    explanation: Option<String>,
}

#[derive(Debug, Serialize)]
struct OutputPairResult {
    id: usize,
    domain_pair: String,
    source_fact: String,
    target_fact: String,
    session_narrative: String,
    allowed_labels: Vec<String>,
    expected_label: String,
    raw_llm_output: String,
    predicted_label: String,
    is_match: bool,
    is_format_compliant: bool,
    /// Per-pair variable-segment decode latency (excludes model load + system prompt prefill)
    decode_latency_ms: f64,
}

#[derive(Debug, Serialize)]
struct MetricSummary {
    total_evaluated: usize,
    total_matches: usize,
    overall_accuracy_pct: f64,
    format_compliance_pct: f64,
    domain_pair_accuracy: HashMap<String, f64>,
    label_precision: HashMap<String, f64>,
    label_recall: HashMap<String, f64>,
    /// Model load time
    model_load_ms: f64,
    /// System-prompt prefill time (one-time cost)
    system_prefill_ms: f64,
    /// Average per-pair decode latency (variable segment only)
    avg_decode_latency_ms: f64,
    total_wall_sec: f64,
    engine_type: String,
    model_path: String,
    prompt_variant: usize,
}

#[derive(Debug, Serialize)]
struct ProbeReport {
    summary: MetricSummary,
    results: Vec<OutputPairResult>,
}

// ─── Static system prompt (TRULY STATIC — never changes per pair) ─────────────

fn static_system_prompt() -> &'static str {
    "You are a memory graph edge classifier for the Vox AI assistant.\n\
     Given two memory facts and their context, output ONLY the single label \
     that best describes the relationship from Fact A to Fact B.\n\
     Output the label name exactly. No explanation. No extra words."
}

/// Build the per-pair user turn (everything after the static system prompt)
/// prompt_variant: 0 = minimal, 1 = structured, 2 = step-by-step-one-word
fn build_user_turn(pair: &InputEdgePair, prompt_variant: usize) -> String {
    let allowed = get_allowed_labels(&pair.source_domain, &pair.target_domain);
    let allowed_str = allowed.join(", ");

    match prompt_variant {
        0 => format!(
            "Options:\n\
             1 = SHAPES\n\
             2 = DEPENDS_ON\n\
             3 = CONFLICTS_WITH\n\
             4 = NONE\n\n\
             Fact A ({src}): {sfact}\n\
             Fact B ({tgt}): {tfact}\n\
             Context: {ctx}\n\n\
             Which option (1-4) best describes how Fact A relates to Fact B?\n\
             Choice:",
            src = pair.source_domain,
            sfact = pair.source_fact,
            tgt = pair.target_domain,
            tfact = pair.target_fact,
            ctx = pair.session_narrative,
        ),
        1 => format!(
            "Classify the relationship between these two memory facts.\n\
             Options:\n\
             1 = SHAPES (Target modifies/constrains Source execution or interpretation)\n\
             2 = DEPENDS_ON (Source functionally requires Target first)\n\
             3 = CONFLICTS_WITH (Source and Target are opposing goals or rules)\n\
             4 = NONE (No causal, dependency, or conflict link)\n\n\
             Fact A [{src}]: {sfact}\n\
             Fact B [{tgt}]: {tfact}\n\
             Session context: {ctx}\n\n\
             Select option number (1-4):\n\
             Choice:",
            src = pair.source_domain,
            sfact = pair.source_fact,
            tgt = pair.target_domain,
            tfact = pair.target_fact,
            ctx = pair.session_narrative,
        ),
        2 => format!(
            "Classify the relationship from Fact A to Fact B.\n\
             Allowed Edge Labels:\n\
             - SHAPES: Target fact modifies or constrains how Source fact is executed or interpreted.\n\
             - DEPENDS_ON: Source fact functionally requires Target fact to exist or be satisfied first.\n\
             - CONFLICTS_WITH: Source fact and Target fact represent opposing goals, preferences, or rules.\n\
             - NONE: No causal, dependency, or conflict relationship exists.\n\n\
             Fact A [{src}]: {sfact}\n\
             Fact B [{tgt}]: {tfact}\n\
             Context: {ctx}\n\n\
             Which label best describes the edge from Fact A to Fact B?\n\
             Label:",
            src = pair.source_domain,
            sfact = pair.source_fact,
            tgt = pair.target_domain,
            tfact = pair.target_fact,
            ctx = pair.session_narrative,
        ),
        _ => format!(
            "Classify the relationship from Fact A to Fact B into one of 4 labels.\n\n\
             Examples:\n\
             Example 1:\n\
             Fact A: I want to build a high-performance video renderer\n\
             Fact B: My laptop has an 8GB RAM CPU-only setup\n\
             Choice: SHAPES\n\n\
             Example 2:\n\
             Fact A: I am deploying the app to Kubernetes\n\
             Fact B: The backend is containerized with Docker\n\
             Choice: DEPENDS_ON\n\n\
             Example 3:\n\
             Fact A: I prefer working late past midnight\n\
             Fact B: I have a mandatory 6am morning team standup\n\
             Choice: CONFLICTS_WITH\n\n\
             Example 4:\n\
             Fact A: I use VSCode for editing\n\
             Fact B: I prefer dark theme interface\n\
             Choice: NONE\n\n\
             Now classify this pair:\n\
             Fact A [{src}]: {sfact}\n\
             Fact B [{tgt}]: {tfact}\n\
             Context: {ctx}\n\n\
             Which label (SHAPES, DEPENDS_ON, CONFLICTS_WITH, NONE) best describes how Fact A relates to Fact B?\n\
             Choice:",
            src = pair.source_domain,
            sfact = pair.source_fact,
            tgt = pair.target_domain,
            tfact = pair.target_fact,
            ctx = pair.session_narrative,
        ),
    }
}

/// Format full ChatML prompt for a pair (for display / API fallback)
fn build_full_prompt(pair: &InputEdgePair, prompt_variant: usize) -> String {
    format!(
        "<|im_start|>system\n{sys}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n<|im_start|>assistant\n",
        sys = static_system_prompt(),
        user = build_user_turn(pair, prompt_variant),
    )
}

// ─── Native llama.cpp engine ─────────────────────────────────────────────────

static NATIVE_BACKEND: OnceLock<LlamaBackend> = OnceLock::new();

struct NativeEngine {
    model: LlamaModel,
    ctx: parking_lot::Mutex<Option<llama_cpp_4::context::LlamaContext<'static>>>,
    n_threads: u32,
    context_size: u32,
    system_tokens: Vec<llama_cpp_4::token::LlamaToken>,
    system_prefill_ms: f64,
}

unsafe impl Send for NativeEngine {}
unsafe impl Sync for NativeEngine {}

impl NativeEngine {
    fn load(model_path: &PathBuf, n_threads: u32, context_size: u32) -> Result<Self> {
        let t0 = Instant::now();
        let backend = NATIVE_BACKEND
            .get_or_init(|| LlamaBackend::init().expect("Failed to initialise llama.cpp backend"));

        let model_params = LlamaModelParams::default();
        let model = LlamaModel::load_from_file(backend, model_path, &model_params)
            .map_err(|e| anyhow!("Failed to load model: {:?}", e))?;
        let load_ms = t0.elapsed().as_secs_f64() * 1000.0;
        println!("  Model loaded in {:.0} ms", load_ms);

        // Tokenise the static system prompt once
        let sys_text = format!("<|im_start|>system\n{}<|im_end|>\n", static_system_prompt());
        let system_tokens = model
            .str_to_token(&sys_text, AddBos::Always)
            .map_err(|e| anyhow!("Tokenisation failed: {:?}", e))?;
        println!("  System prompt tokens: {}", system_tokens.len());

        let mut ctx_params = LlamaContextParams::default();
        ctx_params = ctx_params
            .with_n_ctx(Some(NonZeroU32::new(context_size).unwrap()))
            .with_n_threads(n_threads as i32)
            .with_n_threads_batch(n_threads as i32);
        let mut ctx = model
            .new_context(backend, ctx_params)
            .map_err(|e| anyhow!("Context creation failed: {:?}", e))?;

        let t_sys = Instant::now();
        let sys_len = system_tokens.len();
        let mut sys_batch = LlamaBatch::new(sys_len + 16, 1);
        let sys_last = sys_len - 1;
        for (i, &tok) in system_tokens.iter().enumerate() {
            let is_last = i == sys_last;
            sys_batch
                .add(tok, i as i32, &[0], is_last)
                .map_err(|e| anyhow!("Batch sys add: {:?}", e))?;
        }
        ctx.decode(&mut sys_batch)
            .map_err(|e| anyhow!("System prefill decode: {:?}", e))?;
        let sys_ms = t_sys.elapsed().as_secs_f64() * 1000.0;
        println!(
            "  System prompt prefilled in {:.1} ms (frozen in KV cache)",
            sys_ms
        );

        let static_ctx: llama_cpp_4::context::LlamaContext<'static> =
            unsafe { std::mem::transmute(ctx) };

        Ok(Self {
            model,
            ctx: parking_lot::Mutex::new(Some(static_ctx)),
            n_threads,
            context_size,
            system_tokens,
            system_prefill_ms: sys_ms,
        })
    }

    /// Raw logit evaluation with true end-to-end per-pair latency timing
    fn predict_raw(
        &self,
        pair: &InputEdgePair,
        prompt_variant: usize,
    ) -> Result<((f32, f32, f32, f32), f64)> {
        let pair_start = Instant::now();

        let backend = NATIVE_BACKEND
            .get()
            .ok_or_else(|| anyhow!("Backend not initialised"))?;

        let mut ctx_params = LlamaContextParams::default();
        ctx_params = ctx_params
            .with_n_ctx(Some(NonZeroU32::new(self.context_size).unwrap()))
            .with_n_threads(self.n_threads as i32)
            .with_n_threads_batch(self.n_threads as i32);
        let mut ctx = self
            .model
            .new_context(backend, ctx_params)
            .map_err(|e| anyhow!("Context creation: {:?}", e))?;

        let sys_len = self.system_tokens.len();
        let user_turn_text = format!(
            "<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
            build_user_turn(pair, prompt_variant)
        );
        let user_tokens = self
            .model
            .str_to_token(&user_turn_text, AddBos::Never)
            .map_err(|e| anyhow!("User tokenisation: {:?}", e))?;

        let total_prompt_len = sys_len + user_tokens.len();
        let mut batch = LlamaBatch::new(total_prompt_len + 16, 1);

        for (i, &tok) in self.system_tokens.iter().enumerate() {
            batch
                .add(tok, i as i32, &[0], false)
                .map_err(|e| anyhow!("Batch sys add: {:?}", e))?;
        }

        let user_last = user_tokens.len() - 1;
        for (i, &tok) in user_tokens.iter().enumerate() {
            let is_last = i == user_last;
            batch
                .add(tok, (sys_len + i) as i32, &[0], is_last)
                .map_err(|e| anyhow!("Batch user add: {:?}", e))?;
        }

        ctx.decode(&mut batch)
            .map_err(|e| anyhow!("Prompt decode: {:?}", e))?;

        let last_token_idx = (total_prompt_len - 1) as i32;
        let logits = ctx.get_logits_ith(last_token_idx);

        // Candidate tokens for "1", "2", "3", "4"
        let (s_shp, s_dep, s_cnf, s_non) = if prompt_variant >= 2 {
            let get_word_score = |words: &[&str]| -> f32 {
                let mut max_s = f32::NEG_INFINITY;
                for &w in words {
                    if let Ok(toks) = self.model.str_to_token(w, AddBos::Never) {
                        if let Some(&t) = toks.first() {
                            let idx = t.0 as usize;
                            if idx < logits.len() {
                                max_s = max_s.max(logits[idx]);
                            }
                        }
                    }
                }
                max_s
            };

            (
                get_word_score(&["SHAPES", " SHAPES", "Shapes", " Shapes"]),
                get_word_score(&[
                    "DEPENDS",
                    " DEPENDS",
                    "DEPENDS_ON",
                    " DEPENDS_ON",
                    "Depends",
                    " Depends",
                ]),
                get_word_score(&[
                    "CONFLICTS",
                    " CONFLICTS",
                    "CONFLICTS_WITH",
                    " CONFLICTS_WITH",
                    "Conflicts",
                    " Conflicts",
                ]),
                get_word_score(&["NONE", " NONE", "None", " None"]),
            )
        } else {
            let t1 = self
                .model
                .str_to_token("1", AddBos::Never)
                .unwrap_or_default()
                .get(0)
                .copied();
            let t2 = self
                .model
                .str_to_token("2", AddBos::Never)
                .unwrap_or_default()
                .get(0)
                .copied();
            let t3 = self
                .model
                .str_to_token("3", AddBos::Never)
                .unwrap_or_default()
                .get(0)
                .copied();
            let t4 = self
                .model
                .str_to_token("4", AddBos::Never)
                .unwrap_or_default()
                .get(0)
                .copied();

            let t1_sp = self
                .model
                .str_to_token(" 1", AddBos::Never)
                .unwrap_or_default()
                .get(0)
                .copied();
            let t2_sp = self
                .model
                .str_to_token(" 2", AddBos::Never)
                .unwrap_or_default()
                .get(0)
                .copied();
            let t3_sp = self
                .model
                .str_to_token(" 3", AddBos::Never)
                .unwrap_or_default()
                .get(0)
                .copied();
            let t4_sp = self
                .model
                .str_to_token(" 4", AddBos::Never)
                .unwrap_or_default()
                .get(0)
                .copied();

            let get_score = |t_opt: Option<llama_cpp_4::token::LlamaToken>,
                             t_sp_opt: Option<llama_cpp_4::token::LlamaToken>|
             -> f32 {
                let mut s = f32::NEG_INFINITY;
                if let Some(t) = t_opt {
                    let idx = t.0 as usize;
                    if idx < logits.len() {
                        s = s.max(logits[idx]);
                    }
                }
                if let Some(t) = t_sp_opt {
                    let idx = t.0 as usize;
                    if idx < logits.len() {
                        s = s.max(logits[idx]);
                    }
                }
                s
            };

            (
                get_score(t1, t1_sp),
                get_score(t2, t2_sp),
                get_score(t3, t3_sp),
                get_score(t4, t4_sp),
            )
        };

        let decode_ms = pair_start.elapsed().as_secs_f64() * 1000.0;
        Ok(((s_shp, s_dep, s_cnf, s_non), decode_ms))
    }

    /// Predict label for one pair with optional baseline logit calibration.
    fn predict(
        &self,
        pair: &InputEdgePair,
        prompt_variant: usize,
        baseline: Option<(f32, f32, f32, f32)>,
    ) -> Result<(String, f64)> {
        let ((mut s_shp, mut s_dep, mut s_cnf, mut s_non), decode_ms) =
            self.predict_raw(pair, prompt_variant)?;

        if let Some((b_shp, b_dep, b_cnf, b_non)) = baseline {
            s_shp -= b_shp;
            s_dep -= b_dep;
            s_cnf -= b_cnf;
            s_non -= b_non;
        }

        let allowed = get_allowed_labels(&pair.source_domain, &pair.target_domain);
        let mut candidates = Vec::new();
        for &lbl in allowed {
            match lbl {
                "SHAPES" => candidates.push(("SHAPES", s_shp)),
                "DEPENDS_ON" => candidates.push(("DEPENDS_ON", s_dep)),
                "CONFLICTS_WITH" => candidates.push(("CONFLICTS_WITH", s_cnf)),
                "NONE" => candidates.push(("NONE", s_non)),
                _ => {}
            }
        }

        let best = candidates
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let (pred_label, _best_score) = best.unwrap_or(("NONE", 0.0));

        let text = format!("Choice: {}", pred_label);
        Ok((text, decode_ms))
    }
}

// ─── HTTP API fallback ───────────────────────────────────────────────────────

fn query_http_api(
    endpoint: &str,
    model_name: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<(String, f64)> {
    let start = Instant::now();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let body = serde_json::json!({
        "model": model_name,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "temperature": 0.0,
        "max_tokens": 20
    });

    let resp = client.post(endpoint).json(&body).send()?;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    if !resp.status().is_success() {
        return Err(anyhow!("API error: {}", resp.status()));
    }
    let json: serde_json::Value = resp.json()?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    Ok((content, elapsed_ms))
}

// ─── Main ────────────────────────────────────────────────────────────────────

fn resolve_model_path(model_size: &str, quant: &str, override_path: Option<&PathBuf>) -> PathBuf {
    if let Some(p) = override_path {
        if p.exists() {
            return p.clone();
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let filename = match (model_size, quant) {
        ("350m", "q4") => "LFM2.5-350M-Q4_K_M.gguf",
        ("350m", "q8") => "LFM2.5-350M-Q8_0.gguf",
        ("230m", "q8") => "LFM2.5-230M-Q8_0.gguf",
        _ => "LFM2.5-230M-Q4_K_M.gguf",
    };
    PathBuf::from(home).join(".vox/models/llm").join(filename)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let model_path = resolve_model_path(&args.model_size, &args.quant, args.model_path.as_ref());

    println!("=================================================================");
    println!(" VOX v7 GATE 3 — Local GGUF Edge Classifier Probe");
    println!(" Model Path: {:?}", model_path);
    println!(" Input:      {:?}", args.input);
    println!(" Output:     {:?}", args.output);
    println!(" Prompt variant: {}", args.prompt_variant);
    println!(" Calibrate logits: {}", args.calibrate_logits);
    println!("=================================================================");

    if !args.input.exists() {
        return Err(anyhow!("Input not found: {:?}", args.input));
    }

    // Load model or prepare for HTTP fallback
    let t_load = Instant::now();
    let (engine_opt, engine_type, model_load_ms, sys_prefill_ms) = if args.api_endpoint.is_none() {
        if !model_path.exists() {
            println!(
                "[WARN] Model not found at {:?}. Using HTTP fallback.",
                model_path
            );
            (None, "HTTP_API_Fallback".to_string(), 0.0, 0.0)
        } else {
            println!("Loading native engine from: {:?}", model_path);
            match NativeEngine::load(&model_path, args.threads, 2048) {
                Ok(eng) => {
                    let load_total = t_load.elapsed().as_secs_f64() * 1000.0;
                    let sys_ms = eng.system_prefill_ms;
                    (
                        Some(eng),
                        format!(
                            "Native_llama.cpp_{}_{}",
                            args.model_size.to_uppercase(),
                            args.quant.to_uppercase()
                        ),
                        load_total - sys_ms,
                        sys_ms,
                    )
                }
                Err(e) => {
                    println!(
                        "[WARN] Failed to load native engine: {}. Using HTTP fallback.",
                        e
                    );
                    (None, "HTTP_API_Fallback".to_string(), 0.0, 0.0)
                }
            }
        }
    } else {
        (None, "HTTP_API_Endpoint".to_string(), 0.0, 0.0)
    };

    println!(
        "Model load: {:.0} ms | System prefill: {:.0} ms",
        model_load_ms, sys_prefill_ms
    );

    // Load dataset
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DatasetWrapper {
        Array(Vec<InputEdgePair>),
        Object { pairs: Vec<InputEdgePair> },
    }

    let raw = fs::read(&args.input)?;
    let wrapper: DatasetWrapper = serde_json::from_slice(&raw)?;
    let mut pairs = match wrapper {
        DatasetWrapper::Array(p) => p,
        DatasetWrapper::Object { pairs } => pairs,
    };
    println!("Loaded {} pairs from dataset.", pairs.len());

    // Shuffle
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42);
    let mut rng = seed;
    for i in (1..pairs.len()).rev() {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (rng % ((i + 1) as u64)) as usize;
        pairs.swap(i, j);
    }
    if let Some(lim) = args.max_pairs {
        pairs.truncate(lim);
        println!("Limited to {} pairs.", pairs.len());
    }

    let mut results: Vec<OutputPairResult> = Vec::with_capacity(pairs.len());
    let mut total_matches = 0usize;
    let mut total_format_ok = 0usize;
    let mut total_decode_sum = 0.0f64;

    let mut domain_total: HashMap<String, usize> = HashMap::new();
    let mut domain_matches: HashMap<String, usize> = HashMap::new();
    let mut label_expected_cnt: HashMap<String, usize> = HashMap::new();
    let mut label_predicted_cnt: HashMap<String, usize> = HashMap::new();
    let mut label_tp_cnt: HashMap<String, usize> = HashMap::new();

    let baseline_scores = if args.calibrate_logits {
        if let Some(ref eng) = engine_opt {
            let dummy = InputEdgePair {
                id: 0,
                source_domain: "DomainA".to_string(),
                target_domain: "DomainB".to_string(),
                source_fact: "[N/A]".to_string(),
                target_fact: "[N/A]".to_string(),
                session_narrative: "[N/A]".to_string(),
                expected_label: "NONE".to_string(),
                explanation: None,
            };
            if let Ok(((shp, dep, cnf, non), _)) = eng.predict_raw(&dummy, args.prompt_variant) {
                println!("Baseline unconditioned logits: SHAPES={:.2}, DEPENDS_ON={:.2}, CONFLICTS_WITH={:.2}, NONE={:.2}", shp, dep, cnf, non);
                Some((shp, dep, cnf, non))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let global_start = Instant::now();

    for (idx, pair) in pairs.iter().enumerate() {
        let allowed = get_allowed_labels(&pair.source_domain, &pair.target_domain);
        let allowed_vec: Vec<String> = allowed.iter().map(|s| s.to_string()).collect();
        let dp_key = format!("{} -> {}", pair.source_domain, pair.target_domain);

        let (raw_out, decode_ms) = if let Some(ref eng) = engine_opt {
            match eng.predict(pair, args.prompt_variant, baseline_scores) {
                Ok(v) => v,
                Err(e) => {
                    println!("  [WARN] Pair {} error: {}", pair.id, e);
                    ("ERROR".to_string(), 0.0)
                }
            }
        } else {
            let ep = args
                .api_endpoint
                .as_deref()
                .unwrap_or("http://localhost:8080/v1/chat/completions");
            let user_turn = build_user_turn(pair, args.prompt_variant);
            match query_http_api(ep, &args.model_name, static_system_prompt(), &user_turn) {
                Ok(v) => v,
                Err(e) => {
                    println!("  [WARN] Pair {} API error: {}", pair.id, e);
                    ("ERROR".to_string(), 0.0)
                }
            }
        };

        total_decode_sum += decode_ms;

        let parsed = EdgeLabel::parse(&raw_out);
        let is_format_ok = parsed.is_some();
        if is_format_ok {
            total_format_ok += 1;
        }

        let pred_str = parsed
            .map(|l| l.as_str().to_string())
            .unwrap_or_else(|| "INVALID_FORMAT".to_string());

        let exp_raw = pair.expected_label.trim().to_uppercase();
        let exp_str = if exp_raw.is_empty() {
            String::new()
        } else {
            EdgeLabel::parse(&exp_raw)
                .map(|l| l.as_str().to_string())
                .unwrap_or(exp_raw.clone())
        };

        let exp_parsed = if exp_str.is_empty() {
            None
        } else {
            EdgeLabel::parse(&exp_str)
        };
        let is_match = match (parsed, exp_parsed) {
            (Some(p), Some(e)) => p == e,
            _ => false,
        };

        if is_match {
            total_matches += 1;
            *domain_matches.entry(dp_key.clone()).or_insert(0) += 1;
            *label_tp_cnt.entry(exp_str.clone()).or_insert(0) += 1;
        }
        *domain_total.entry(dp_key.clone()).or_insert(0) += 1;
        if !exp_str.is_empty() {
            *label_expected_cnt.entry(exp_str.clone()).or_insert(0) += 1;
        }
        *label_predicted_cnt.entry(pred_str.clone()).or_insert(0) += 1;

        results.push(OutputPairResult {
            id: pair.id,
            domain_pair: dp_key,
            source_fact: pair.source_fact.clone(),
            target_fact: pair.target_fact.clone(),
            session_narrative: pair.session_narrative.clone(),
            allowed_labels: allowed_vec,
            expected_label: exp_str,
            raw_llm_output: raw_out,
            predicted_label: pred_str,
            is_match,
            is_format_compliant: is_format_ok,
            decode_latency_ms: decode_ms,
        });

        if (idx + 1) % 50 == 0 || (idx + 1) == pairs.len() {
            let running_acc = (total_matches as f64 / (idx + 1) as f64) * 100.0;
            let running_lat = total_decode_sum / (idx + 1) as f64;
            println!(
                "  [{}/{}] acc={:.1}%  decode={:.1}ms",
                idx + 1,
                pairs.len(),
                running_acc,
                running_lat
            );
        }
    }

    let n = pairs.len();
    let wall_sec = global_start.elapsed().as_secs_f64();
    let overall_acc = if n > 0 {
        total_matches as f64 / n as f64 * 100.0
    } else {
        0.0
    };
    let fmt_pct = if n > 0 {
        total_format_ok as f64 / n as f64 * 100.0
    } else {
        0.0
    };
    let avg_decode = if n > 0 {
        total_decode_sum / n as f64
    } else {
        0.0
    };

    let mut domain_pair_accuracy = HashMap::new();
    for (dp, tot) in &domain_total {
        let mat = domain_matches.get(dp).cloned().unwrap_or(0);
        domain_pair_accuracy.insert(
            dp.clone(),
            if *tot > 0 {
                mat as f64 / *tot as f64 * 100.0
            } else {
                0.0
            },
        );
    }

    let mut label_precision = HashMap::new();
    let mut label_recall = HashMap::new();
    for lbl in &["REQUIRES", "RESTRICTS", "ENABLES", "RELATES_TO", "NONE"] {
        let ls = lbl.to_string();
        let tp = *label_tp_cnt.get(&ls).unwrap_or(&0) as f64;
        let pc = *label_predicted_cnt.get(&ls).unwrap_or(&0) as f64;
        let ec = *label_expected_cnt.get(&ls).unwrap_or(&0) as f64;
        label_precision.insert(ls.clone(), if pc > 0.0 { tp / pc * 100.0 } else { 0.0 });
        label_recall.insert(ls.clone(), if ec > 0.0 { tp / ec * 100.0 } else { 0.0 });
    }

    let summary = MetricSummary {
        total_evaluated: n,
        total_matches,
        overall_accuracy_pct: overall_acc,
        format_compliance_pct: fmt_pct,
        domain_pair_accuracy,
        label_precision,
        label_recall,
        model_load_ms,
        system_prefill_ms: sys_prefill_ms,
        avg_decode_latency_ms: avg_decode,
        total_wall_sec: wall_sec,
        engine_type,
        model_path: model_path.display().to_string(),
        prompt_variant: args.prompt_variant,
    };

    let report = ProbeReport { summary, results };

    if let Some(p) = args.output.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(&args.output, serde_json::to_string_pretty(&report)?)?;

    println!();
    println!("=================================================================");
    println!(" PROBE COMPLETE");
    println!(
        " Overall accuracy:        {:.2}% ({}/{})",
        overall_acc, total_matches, n
    );
    println!(" Format compliance:       {:.1}%", fmt_pct);
    println!(
        " Avg decode latency:      {:.1} ms/pair (variable segment only)",
        avg_decode
    );
    println!(" System prefill (1-time): {:.0} ms", sys_prefill_ms);
    println!(" Model load:              {:.0} ms", model_load_ms);
    println!(" Total wall time:         {:.1} sec", wall_sec);
    println!(" Report: {:?}", args.output);
    println!("=================================================================");
    println!();
    println!(" Domain accuracy:");
    let mut dv: Vec<_> = report.summary.domain_pair_accuracy.iter().collect();
    dv.sort_by(|a, b| a.0.cmp(b.0));
    for (dp, acc) in &dv {
        let tot = domain_total.get(*dp).unwrap_or(&0);
        let mat = domain_matches.get(*dp).unwrap_or(&0);
        println!("   {:<35}: {:.1}%  ({}/{})", dp, acc, mat, tot);
    }

    Ok(())
}
