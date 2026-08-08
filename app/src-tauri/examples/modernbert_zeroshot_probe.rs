//! ============================================================================
//! modernbert_zeroshot_probe.rs — Vox v7 Gate 3 ModernBERT-large-zeroshot ONNX Probe
//! ============================================================================
//! Category     : Utility Tool / Benchmark Harness (Cargo Example)
//! Component    : Cognitive Edge Classifier Engine (ModernBERT-large-zeroshot-v2.0 ONNX)
//! Architecture : Vox v7 6-Domain Cognitive Memory Spec (Section 7 / Gate 3)
//! Target Model : `~/.vox/models/classifier/modernbert-zeroshot/onnx/model_quantized.onnx`
//!
//! How ModernBERT zeroshot works:
//!   - It is an NLI-style binary model (entailment / not_entailment)
//!   - For zero-shot classification, we format: premise = "fact pair context"
//!     hypothesis = "The relationship between these facts is <LABEL>"
//!   - We run one inference per candidate label, pick the label with highest entailment score
//!   - Only allowed labels for the given domain pair are tested
//!
//! Execution:
//!   cargo run --example modernbert_zeroshot_probe -- \
//!     --input sandbox/datasets/gate3_edge_1750_pairs.json \
//!     --output sandbox/results/gate3_modernbert_zeroshot_scores.json \
//!     [--max-pairs 100]
//! ============================================================================

use anyhow::{anyhow, Result};
use clap::Parser;
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tokenizers::Tokenizer;

#[derive(Parser, Debug)]
#[command(
    name = "modernbert_zeroshot_probe",
    about = "Vox v7 Gate 3: ModernBERT-large-zeroshot ONNX zero-shot edge classifier"
)]
struct Args {
    /// Input JSON dataset (gate3 pairs with expected_label)
    #[arg(
        short,
        long,
        default_value = "sandbox/datasets/gate3_edge_1750_pairs.json"
    )]
    input: PathBuf,

    /// Output JSON result file
    #[arg(
        short,
        long,
        default_value = "sandbox/results/gate3_modernbert_zeroshot_scores.json"
    )]
    output: PathBuf,

    /// Model directory override
    #[arg(short, long)]
    model_dir: Option<PathBuf>,

    /// Limit number of pairs to evaluate
    #[arg(long)]
    max_pairs: Option<usize>,

    /// Hypothesis template variant (0=relation_is, 1=these_facts_are, 2=fact_a_x_fact_b)
    #[arg(long, default_value = "0")]
    hypothesis_variant: usize,
}

/// Allowed edge labels per domain pair per v7 spec §7.2
fn get_allowed_labels(_src_domain: &str, _tgt_domain: &str) -> &'static [&'static str] {
    &["SHAPES", "DEPENDS_ON", "CONFLICTS_WITH", "NONE"]
}

/// Build NLI hypothesis for a given label
fn build_hypothesis(label: &str, src_domain: &str, tgt_domain: &str, _variant: usize) -> String {
    match label {
        "SHAPES" => format!(
            "The {} fact modifies or shapes how the {} fact is executed or interpreted.",
            src_domain, tgt_domain
        ),
        "DEPENDS_ON" => format!(
            "The {} fact functionally requires or depends on the {} fact to exist first.",
            src_domain, tgt_domain
        ),
        "CONFLICTS_WITH" => format!(
            "The {} fact conflicts with or opposes the {} fact.",
            src_domain, tgt_domain
        ),
        "NONE" => format!(
            "The {} fact and the {} fact are independent with no relationship.",
            src_domain, tgt_domain
        ),
        _ => label.to_string(),
    }
}

/// Build NLI premise from a pair
fn build_premise(
    src_domain: &str,
    tgt_domain: &str,
    src_fact: &str,
    tgt_fact: &str,
    session_narrative: &str,
) -> String {
    format!(
        "Context: {}\nFact A ({domain_a}): {fact_a}\nFact B ({domain_b}): {fact_b}",
        session_narrative,
        domain_a = src_domain,
        fact_a = src_fact,
        domain_b = tgt_domain,
        fact_b = tgt_fact,
    )
}

#[derive(Debug, Deserialize, Clone)]
struct InputEdgePair {
    id: usize,
    source_domain: String,
    target_domain: String,
    #[serde(alias = "fact_a")]
    source_fact: String,
    #[serde(alias = "fact_b")]
    target_fact: String,
    #[serde(alias = "context")]
    session_narrative: String,
    #[serde(default)]
    expected_label: String,
    #[allow(dead_code)]
    explanation: Option<String>,
}

#[derive(Debug, Serialize)]
struct LabelScore {
    label: String,
    entailment_prob: f32,
    not_entailment_prob: f32,
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
    predicted_label: String,
    predicted_score: f32,
    label_scores: Vec<LabelScore>,
    is_match: bool,
    total_inference_ms: f64,
    per_label_ms: f64,
}

#[derive(Debug, Serialize)]
struct MetricSummary {
    total_evaluated: usize,
    total_matches: usize,
    overall_accuracy_pct: f64,
    domain_pair_accuracy: HashMap<String, f64>,
    label_precision: HashMap<String, f64>,
    label_recall: HashMap<String, f64>,
    avg_total_latency_ms: f64,
    avg_per_label_latency_ms: f64,
    model_info: String,
    hypothesis_variant: usize,
}

#[derive(Debug, Serialize)]
struct ProbeReport {
    summary: MetricSummary,
    results: Vec<OutputPairResult>,
}

struct ModernBertSession {
    session: ort::session::Session,
    tokenizer: Tokenizer,
    // class index mapping: entailment index in model output
    entailment_idx: usize,
}

impl ModernBertSession {
    fn load(model_dir: &PathBuf) -> Result<Self> {
        let onnx_dir = model_dir.join("onnx");
        let model_path = if onnx_dir.join("model_quantized.onnx").exists() {
            onnx_dir.join("model_quantized.onnx")
        } else {
            return Err(anyhow!("model_quantized.onnx not found in {:?}", onnx_dir));
        };
        let tokenizer_path = model_dir.join("tokenizer.json");

        if !tokenizer_path.exists() {
            return Err(anyhow!("tokenizer.json not found in {:?}", model_dir));
        }

        println!("  Loading tokenizer from: {:?}", tokenizer_path);
        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer: {}", e))?;

        // Set max length to prevent truncation issues
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: 512,
                strategy: tokenizers::TruncationStrategy::LongestFirst,
                stride: 0,
                direction: tokenizers::TruncationDirection::Right,
            }))
            .map_err(|e| anyhow!("Truncation config error: {}", e))?;

        println!("  Loading ONNX session from: {:?}", model_path);
        let session = ort::session::Session::builder()
            .map_err(|e| anyhow!("ORT session builder: {:?}", e))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow!("{:?}", e))?
            .with_intra_threads(4)
            .map_err(|e| anyhow!("{:?}", e))?
            .commit_from_file(&model_path)
            .map_err(|e| anyhow!("Failed to load model: {:?}", e))?;

        println!(
            "  ONNX inputs: {:?}",
            session
                .inputs()
                .iter()
                .map(|i| i.name())
                .collect::<Vec<_>>()
        );
        println!(
            "  ONNX outputs: {:?}",
            session
                .outputs()
                .iter()
                .map(|o| o.name())
                .collect::<Vec<_>>()
        );

        // Calibrate: "A cat is sleeping" entails "An animal is resting"
        // ModernBERT-large-zeroshot config says: id2label {0: "entailment", 1: "not_entailment"}
        // So entailment_idx = 0
        let entailment_idx = 0usize;

        Ok(Self {
            session,
            tokenizer,
            entailment_idx,
        })
    }

    /// Run a single NLI inference, return [entailment_score, not_entailment_score]
    fn predict_nli(&mut self, premise: &str, hypothesis: &str) -> Result<[f32; 2]> {
        let encoding = self
            .tokenizer
            .encode((premise, hypothesis), true)
            .map_err(|e| anyhow!("Tokenization error: {:?}", e))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        let seq_len = ids.len();

        if seq_len == 0 {
            return Ok([0.5, 0.5]);
        }

        let input_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), ids)?;
        let attn_mask_arr = Array2::<i64>::from_shape_vec((1, seq_len), mask)?;

        let ids_tensor =
            ort::value::Tensor::from_array(input_ids_arr).map_err(|e| anyhow!("{:?}", e))?;
        let mask_tensor =
            ort::value::Tensor::from_array(attn_mask_arr).map_err(|e| anyhow!("{:?}", e))?;

        let has_token_type_ids = self
            .session
            .inputs()
            .iter()
            .any(|i| i.name() == "token_type_ids");

        let outputs = if has_token_type_ids {
            let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();
            let type_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), type_ids)?;
            let type_ids_tensor =
                ort::value::Tensor::from_array(type_ids_arr).map_err(|e| anyhow!("{:?}", e))?;
            self.session
                .run(ort::inputs![
                    "input_ids" => ids_tensor,
                    "attention_mask" => mask_tensor,
                    "token_type_ids" => type_ids_tensor
                ])
                .map_err(|e| anyhow!("{:?}", e))?
        } else {
            self.session
                .run(ort::inputs![
                    "input_ids" => ids_tensor,
                    "attention_mask" => mask_tensor
                ])
                .map_err(|e| anyhow!("{:?}", e))?
        };

        let out_key = outputs.keys().next().ok_or_else(|| anyhow!("No outputs"))?;
        let logits = outputs[out_key]
            .try_extract_array::<f32>()
            .map_err(|e| anyhow!("{:?}", e))?;

        // Softmax over 2 logits
        let l0 = logits[[0, 0]];
        let l1 = logits[[0, 1]];
        let max_l = l0.max(l1);
        let e0 = (l0 - max_l).exp();
        let e1 = (l1 - max_l).exp();
        let sum = e0 + e1;

        Ok([e0 / sum, e1 / sum])
    }

    /// Zero-shot: run NLI for each allowed label, pick max entailment score
    fn classify_zeroshot(
        &mut self,
        premise: &str,
        allowed_labels: &[&str],
        src_domain: &str,
        tgt_domain: &str,
        hypothesis_variant: usize,
    ) -> Result<(String, f32, Vec<LabelScore>, f64, f64)> {
        let mut best_label = "NONE".to_string();
        let mut best_score = f32::NEG_INFINITY;
        let mut all_scores = Vec::new();
        let mut total_ms = 0.0f64;

        for &label in allowed_labels {
            let hypothesis = build_hypothesis(label, src_domain, tgt_domain, hypothesis_variant);
            let start = Instant::now();
            let probs = self.predict_nli(premise, &hypothesis)?;
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            total_ms += elapsed;

            let ent_score = probs[self.entailment_idx];
            let not_ent_score = probs[1 - self.entailment_idx];

            if ent_score > best_score {
                best_score = ent_score;
                best_label = label.to_string();
            }

            all_scores.push(LabelScore {
                label: label.to_string(),
                entailment_prob: ent_score,
                not_entailment_prob: not_ent_score,
            });
        }

        let per_label_ms = total_ms / allowed_labels.len() as f64;
        Ok((best_label, best_score, all_scores, total_ms, per_label_ms))
    }
}

fn resolve_model_dir(override_dir: Option<PathBuf>) -> PathBuf {
    if let Some(d) = override_dir {
        return d;
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".vox/models/classifier/modernbert-zeroshot")
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("=================================================================");
    println!(" VOX v7 GATE 3 — ModernBERT-large-zeroshot ONNX Probe");
    println!(" Input:  {:?}", args.input);
    println!(" Output: {:?}", args.output);
    println!(" Hypothesis variant: {}", args.hypothesis_variant);
    println!("=================================================================");

    if !args.input.exists() {
        return Err(anyhow!("Input dataset not found: {:?}", args.input));
    }

    let model_dir = resolve_model_dir(args.model_dir);
    println!("Loading ModernBERT from: {:?}", model_dir);

    let mut model = ModernBertSession::load(&model_dir)?;
    println!("Model loaded. Entailment index: {}", model.entailment_idx);

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DatasetWrapper {
        Array(Vec<InputEdgePair>),
        Object { pairs: Vec<InputEdgePair> },
    }

    let input_bytes = fs::read(&args.input)?;
    let wrapper: DatasetWrapper = serde_json::from_slice(&input_bytes)?;
    let mut pairs = match wrapper {
        DatasetWrapper::Array(p) => p,
        DatasetWrapper::Object { pairs } => pairs,
    };
    println!("Loaded {} pairs.", pairs.len());

    // Shuffle for representative sampling
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

    if let Some(limit) = args.max_pairs {
        pairs.truncate(limit);
        println!("Using {} pairs (limited).", pairs.len());
    }

    let mut results: Vec<OutputPairResult> = Vec::with_capacity(pairs.len());
    let mut total_matches = 0usize;
    let mut total_latency_sum = 0.0f64;

    let mut domain_total: HashMap<String, usize> = HashMap::new();
    let mut domain_matches: HashMap<String, usize> = HashMap::new();
    let mut label_expected_count: HashMap<String, usize> = HashMap::new();
    let mut label_predicted_count: HashMap<String, usize> = HashMap::new();
    let mut label_tp_count: HashMap<String, usize> = HashMap::new();

    let global_start = Instant::now();

    for (idx, pair) in pairs.iter().enumerate() {
        let allowed = get_allowed_labels(&pair.source_domain, &pair.target_domain);
        let allowed_slice: Vec<&str> = allowed.to_vec();

        let premise = build_premise(
            &pair.source_domain,
            &pair.target_domain,
            &pair.source_fact,
            &pair.target_fact,
            &pair.session_narrative,
        );

        let (predicted_label, predicted_score, label_scores, total_ms, per_label_ms) = model
            .classify_zeroshot(
                &premise,
                &allowed_slice,
                &pair.source_domain,
                &pair.target_domain,
                args.hypothesis_variant,
            )?;

        total_latency_sum += total_ms;
        let domain_pair_key = format!("{} -> {}", pair.source_domain, pair.target_domain);

        // Normalize expected label
        let exp_label = pair.expected_label.trim().to_uppercase();
        let exp_label = if exp_label == "RELATES_TO" || exp_label == "RELATES TO" {
            "RELATES_TO".to_string()
        } else {
            exp_label
        };

        let is_match = !exp_label.is_empty() && predicted_label == exp_label;

        if is_match {
            total_matches += 1;
            *domain_matches.entry(domain_pair_key.clone()).or_insert(0) += 1;
            *label_tp_count.entry(exp_label.clone()).or_insert(0) += 1;
        }

        *domain_total.entry(domain_pair_key.clone()).or_insert(0) += 1;
        if !exp_label.is_empty() {
            *label_expected_count.entry(exp_label.clone()).or_insert(0) += 1;
        }
        *label_predicted_count
            .entry(predicted_label.clone())
            .or_insert(0) += 1;

        results.push(OutputPairResult {
            id: pair.id,
            domain_pair: domain_pair_key,
            source_fact: pair.source_fact.clone(),
            target_fact: pair.target_fact.clone(),
            session_narrative: pair.session_narrative.clone(),
            allowed_labels: allowed_slice.iter().map(|s| s.to_string()).collect(),
            expected_label: exp_label,
            predicted_label,
            predicted_score,
            label_scores,
            is_match,
            total_inference_ms: total_ms,
            per_label_ms,
        });

        if (idx + 1) % 50 == 0 || (idx + 1) == pairs.len() {
            let acc_so_far = (total_matches as f64 / (idx + 1) as f64) * 100.0;
            println!(
                "  [{}/{}] Running accuracy: {:.1}%",
                idx + 1,
                pairs.len(),
                acc_so_far
            );
        }
    }

    let total_duration = global_start.elapsed().as_secs_f64();
    let n = pairs.len();
    let overall_acc = if n > 0 {
        (total_matches as f64 / n as f64) * 100.0
    } else {
        0.0
    };
    let avg_total_lat = if n > 0 {
        total_latency_sum / n as f64
    } else {
        0.0
    };
    // Per label latency: each domain pair runs N_labels inferences; average per label across all
    let total_label_inferences: usize = pairs
        .iter()
        .map(|p| get_allowed_labels(&p.source_domain, &p.target_domain).len())
        .sum();
    let avg_per_label_ms = if total_label_inferences > 0 {
        total_latency_sum / total_label_inferences as f64
    } else {
        0.0
    };

    let mut domain_pair_accuracy = HashMap::new();
    for (dp, tot) in &domain_total {
        let mat = domain_matches.get(dp).cloned().unwrap_or(0);
        domain_pair_accuracy.insert(
            dp.clone(),
            if *tot > 0 {
                (mat as f64 / *tot as f64) * 100.0
            } else {
                0.0
            },
        );
    }

    let mut label_precision = HashMap::new();
    let mut label_recall = HashMap::new();
    for label in &["REQUIRES", "RESTRICTS", "ENABLES", "RELATES_TO", "NONE"] {
        let ls = label.to_string();
        let tp = *label_tp_count.get(&ls).unwrap_or(&0) as f64;
        let pred_cnt = *label_predicted_count.get(&ls).unwrap_or(&0) as f64;
        let exp_cnt = *label_expected_count.get(&ls).unwrap_or(&0) as f64;
        label_precision.insert(
            ls.clone(),
            if pred_cnt > 0.0 {
                tp / pred_cnt * 100.0
            } else {
                0.0
            },
        );
        label_recall.insert(
            ls.clone(),
            if exp_cnt > 0.0 {
                tp / exp_cnt * 100.0
            } else {
                0.0
            },
        );
    }

    let summary = MetricSummary {
        total_evaluated: n,
        total_matches,
        overall_accuracy_pct: overall_acc,
        domain_pair_accuracy,
        label_precision,
        label_recall,
        avg_total_latency_ms: avg_total_lat,
        avg_per_label_latency_ms: avg_per_label_ms,
        model_info: "ModernBERT-large-zeroshot-v2.0 ONNX INT8".to_string(),
        hypothesis_variant: args.hypothesis_variant,
    };

    let report = ProbeReport { summary, results };

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serde_json::to_string_pretty(&report)?)?;

    println!();
    println!("=================================================================");
    println!(" MODERNBERT ZEROSHOT PROBE COMPLETE");
    println!(
        " Overall Accuracy:      {:.2}% ({}/{})",
        overall_acc, total_matches, n
    );
    println!(" Avg total latency:     {:.1} ms/pair", avg_total_lat);
    println!(
        " Avg per-label latency: {:.1} ms/label-inference",
        avg_per_label_ms
    );
    println!(" Total wall time:       {:.1} sec", total_duration);
    println!(" Report: {:?}", args.output);
    println!("=================================================================");
    println!();
    println!(" Domain accuracy breakdown:");
    let mut domain_acc_vec: Vec<_> = report.summary.domain_pair_accuracy.iter().collect();
    domain_acc_vec.sort_by(|a, b| a.0.cmp(b.0));
    for (dp, acc) in &domain_acc_vec {
        let tot = domain_total.get(*dp).cloned().unwrap_or(0);
        let mat = domain_matches.get(*dp).cloned().unwrap_or(0);
        println!("   {:<35}: {:.1}%  ({}/{})", dp, acc, mat, tot);
    }

    Ok(())
}
