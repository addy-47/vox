//! ============================================================================
//! nli_bench.rs — Vox v7 Gate 2 DeBERTa-v3 NLI Domain Precision Benchmark
//! ============================================================================
//! Category     : Benchmark / Audit Harness
//! Component    : NLI State Resolution Engine (`vox_lib::services::memory::nli`)
//! Architecture : Vox v7 6-Domain Cognitive Memory Spec (Section 5 / Gate 2)
//! Prerequisites: Local ONNX NLI model at `~/.vox/models/nli/deberta-v3-xsmall/`
//! Execution    : cargo test --bench nli_bench -- batch-nli-score --input <JSON> --output <JSON>
//! ============================================================================

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokenizers::Tokenizer;

#[derive(Parser, Debug)]
#[command(name = "nli_bench")]
#[command(about = "Vox v7 Gate 2 DeBERTa-v3 NLI Domain Precision Benchmark", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Batch test domain NLI fact pairs from a JSON dataset
    BatchNliScore {
        /// Path to input JSON dataset file (e.g. sandbox/datasets/gate2_nli_400_pairs.json)
        #[arg(short, long)]
        input: PathBuf,

        /// Path to output JSON result file (e.g. sandbox/results/gate2_nli_raw_scores.json)
        #[arg(short, long)]
        output: PathBuf,

        /// Optional model directory override (defaults to ~/.vox/models/nli/deberta-v3-xsmall)
        #[arg(short, long)]
        model_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum NliLabel {
    Contradiction,
    Entailment,
    Neutral,
}

impl NliLabel {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Contradiction => "CONTRADICTION",
            Self::Entailment => "ENTAILMENT",
            Self::Neutral => "NEUTRAL",
        }
    }

    fn from_str_lenient(s: &str) -> Option<Self> {
        let u = s.trim().to_uppercase();
        if u.contains("CONTRADICT") {
            Some(Self::Contradiction)
        } else if u.contains("ENTAIL") {
            Some(Self::Entailment)
        } else if u.contains("NEUTRAL") {
            Some(Self::Neutral)
        } else {
            None
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct InputNliPair {
    id: usize,
    domain: String,
    premise: String,
    hypothesis: String,
    expected_label: String,
}

#[derive(Debug, Serialize)]
struct LabelProbabilities {
    entailment: f32,
    contradiction: f32,
    neutral: f32,
}

#[derive(Debug, Serialize)]
struct OutputPairScore {
    id: usize,
    domain: String,
    premise: String,
    hypothesis: String,
    expected_label: String,
    predicted_label: String,
    is_match: bool,
    probabilities: LabelProbabilities,
    max_probability: f32,
    latency_us: u128,
}

#[derive(Debug, Serialize)]
struct NliBatchSummary {
    total_pairs_scored: usize,
    total_matches: usize,
    overall_accuracy_pct: f64,
    domain_accuracies: HashMap<String, f64>,
    total_duration_ms: f64,
    avg_pair_latency_ms: f64,
}

#[derive(Debug, Serialize)]
struct NliBatchOutput {
    summary: NliBatchSummary,
    raw_results: Vec<OutputPairScore>,
}

struct ModelInstance {
    name: String,
    session: ort::session::Session,
    tokenizer: Tokenizer,
    has_token_type_ids: bool,
    class_mapping: [NliLabel; 3],
    model_size_mb: f64,
}

impl ModelInstance {
    fn load(name: &str, dir: &Path) -> Result<Self> {
        let model_path = if dir.join("model_quantized.onnx").exists() {
            dir.join("model_quantized.onnx")
        } else if dir.join("model_int8.onnx").exists() {
            dir.join("model_int8.onnx")
        } else {
            dir.join("model.onnx")
        };
        let tokenizer_path = dir.join("tokenizer.json");

        if !model_path.exists() || !tokenizer_path.exists() {
            return Err(anyhow!("Missing model/tokenizer files in {:?}", dir));
        }

        let model_size_bytes = fs::metadata(&model_path)?.len();
        let model_size_mb = model_size_bytes as f64 / (1024.0 * 1024.0);

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer from {:?}: {}", tokenizer_path, e))?;

        let session = ort::session::Session::builder()
            .map_err(|e| anyhow!("{:?}", e))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow!("{:?}", e))?
            .with_intra_threads(2)
            .map_err(|e| anyhow!("{:?}", e))?
            .commit_from_file(&model_path)
            .map_err(|e| anyhow!("{:?}", e))?;

        let has_token_type_ids = session
            .inputs()
            .iter()
            .any(|i| i.name() == "token_type_ids");
        let class_mapping = [
            NliLabel::Contradiction,
            NliLabel::Entailment,
            NliLabel::Neutral,
        ];

        let mut instance = Self {
            name: name.to_string(),
            session,
            tokenizer,
            has_token_type_ids,
            class_mapping,
            model_size_mb,
        };

        instance.calibrate()?;
        Ok(instance)
    }

    fn calibrate(&mut self) -> Result<()> {
        let p_ent = "A person is playing tennis.";
        let h_ent = "A person is playing tennis.";
        let logits_ent = self.raw_predict(p_ent, h_ent)?;

        let p_con = "A person is playing tennis.";
        let h_con = "A person is sleeping.";
        let logits_con = self.raw_predict(p_con, h_con)?;

        let ent_idx = logits_ent
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap()
            .0;
        let con_idx = logits_con
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap()
            .0;

        if ent_idx == con_idx {
            println!(
                "  [WARN] Calibration collision for {}. Using default mapping.",
                self.name
            );
            self.class_mapping = [
                NliLabel::Contradiction,
                NliLabel::Entailment,
                NliLabel::Neutral,
            ];
        } else {
            let mut indices = vec![0, 1, 2];
            indices.retain(|&x| x != ent_idx && x != con_idx);
            let neu_idx = indices[0];

            self.class_mapping[ent_idx] = NliLabel::Entailment;
            self.class_mapping[con_idx] = NliLabel::Contradiction;
            self.class_mapping[neu_idx] = NliLabel::Neutral;
        }

        println!(
            "  Calibrated Mapping for {}: [0: {:?}, 1: {:?}, 2: {:?}] (Model Size: {:.1} MB)",
            self.name,
            self.class_mapping[0],
            self.class_mapping[1],
            self.class_mapping[2],
            self.model_size_mb
        );

        Ok(())
    }

    fn raw_predict(&mut self, premise: &str, hypothesis: &str) -> Result<Vec<f32>> {
        let encoding = self
            .tokenizer
            .encode((premise, hypothesis), true)
            .map_err(|e| anyhow!("Tokenization failed: {:?}", e))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        let seq_len = ids.len();

        if seq_len == 0 {
            return Ok(vec![0.0, 0.0, 0.0]);
        }

        let input_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), ids)?;
        let attention_mask_arr = Array2::<i64>::from_shape_vec((1, seq_len), mask)?;

        let input_ids_tensor =
            ort::value::Tensor::from_array(input_ids_arr).map_err(|e| anyhow!("{:?}", e))?;
        let attention_mask_tensor =
            ort::value::Tensor::from_array(attention_mask_arr).map_err(|e| anyhow!("{:?}", e))?;

        let outputs = if self.has_token_type_ids {
            let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();
            let type_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), type_ids)?;
            let type_ids_tensor =
                ort::value::Tensor::from_array(type_ids_arr).map_err(|e| anyhow!("{:?}", e))?;
            self.session
                .run(ort::inputs![
                    "input_ids" => input_ids_tensor,
                    "attention_mask" => attention_mask_tensor,
                    "token_type_ids" => type_ids_tensor
                ])
                .map_err(|e| anyhow!("{:?}", e))?
        } else {
            self.session
                .run(ort::inputs![
                    "input_ids" => input_ids_tensor,
                    "attention_mask" => attention_mask_tensor
                ])
                .map_err(|e| anyhow!("{:?}", e))?
        };

        let output_key = outputs
            .keys()
            .next()
            .ok_or_else(|| anyhow!("No output in model"))?;
        let logits_array = outputs[output_key]
            .try_extract_array::<f32>()
            .map_err(|e| anyhow!("{:?}", e))?;

        Ok(vec![
            logits_array[[0, 0]],
            logits_array[[0, 1]],
            logits_array[[0, 2]],
        ])
    }

    fn predict_full(
        &mut self,
        premise: &str,
        hypothesis: &str,
    ) -> Result<(NliLabel, LabelProbabilities, f32, u128)> {
        let start = Instant::now();
        let logits = self.raw_predict(premise, hypothesis)?;
        let elapsed_us = start.elapsed().as_micros();

        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exps.iter().sum();
        let probs: Vec<f32> = exps.iter().map(|e| e / sum_exp).collect();

        let mut prob_map = HashMap::new();
        for (i, &p) in probs.iter().enumerate() {
            prob_map.insert(self.class_mapping[i], p);
        }

        let p_ent = prob_map.get(&NliLabel::Entailment).cloned().unwrap_or(0.0);
        let p_con = prob_map
            .get(&NliLabel::Contradiction)
            .cloned()
            .unwrap_or(0.0);
        let p_neu = prob_map.get(&NliLabel::Neutral).cloned().unwrap_or(0.0);

        let (max_idx, &max_prob) = probs
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();
        let label = self.class_mapping[max_idx];

        let probabilities = LabelProbabilities {
            entailment: p_ent,
            contradiction: p_con,
            neutral: p_neu,
        };

        Ok((label, probabilities, max_prob, elapsed_us))
    }
}

fn resolve_model_dir(override_dir: Option<PathBuf>) -> PathBuf {
    if let Some(d) = override_dir {
        return d;
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let default_path = PathBuf::from(home).join(".vox/models/nli/deberta-v3-xsmall");
    if default_path.exists() {
        return default_path;
    }

    
    PathBuf::from("~/.vox/models/nli/deberta-v3-xsmall")
}

fn run_batch_nli_score(
    input: PathBuf,
    output: PathBuf,
    model_dir_opt: Option<PathBuf>,
) -> Result<()> {
    println!("=================================================================");
    println!(" VOX v7 GATE 2 DEBERTA-V3 NLI DOMAIN PRECISION BENCHMARK");
    println!(" Input Dataset: {:?}", input);
    println!(" Output Destination: {:?}", output);
    println!("=================================================================");

    if !input.exists() {
        return Err(anyhow!(
            "Input JSON dataset file does not exist: {:?}",
            input
        ));
    }

    let model_dir = resolve_model_dir(model_dir_opt);
    println!("Loading DeBERTa ONNX model from: {:?}", model_dir);
    let mut model = ModelInstance::load("DeBERTa-v3-xsmall", &model_dir)?;

    let input_bytes = fs::read(&input)?;
    let pairs: Vec<InputNliPair> = serde_json::from_slice(&input_bytes)?;
    println!("Loaded {} pairs from input dataset.", pairs.len());

    let mut output_scores = Vec::with_capacity(pairs.len());
    let mut total_matches = 0;
    let mut domain_total: HashMap<String, usize> = HashMap::new();
    let mut domain_matches: HashMap<String, usize> = HashMap::new();

    let total_start = Instant::now();

    for (idx, p) in pairs.iter().enumerate() {
        let (pred_label, probs, max_prob, elapsed_us) =
            model.predict_full(&p.premise, &p.hypothesis)?;

        // Vox v7 threshold enforcement: ENTAILMENT or CONTRADICTION requires P >= 0.85, else fallback to NEUTRAL
        let final_pred_label = match pred_label {
            NliLabel::Entailment => {
                if probs.entailment >= 0.85 {
                    NliLabel::Entailment
                } else {
                    NliLabel::Neutral
                }
            }
            NliLabel::Contradiction => {
                if probs.contradiction >= 0.85 {
                    NliLabel::Contradiction
                } else {
                    NliLabel::Neutral
                }
            }
            NliLabel::Neutral => NliLabel::Neutral,
        };

        let expected_opt = NliLabel::from_str_lenient(&p.expected_label);
        let is_match = match expected_opt {
            Some(exp) => exp == final_pred_label,
            None => false,
        };

        if is_match {
            total_matches += 1;
            *domain_matches.entry(p.domain.clone()).or_insert(0) += 1;
        }
        *domain_total.entry(p.domain.clone()).or_insert(0) += 1;

        output_scores.push(OutputPairScore {
            id: p.id,
            domain: p.domain.clone(),
            premise: p.premise.clone(),
            hypothesis: p.hypothesis.clone(),
            expected_label: p.expected_label.clone(),
            predicted_label: final_pred_label.as_str().to_string(),
            is_match,
            probabilities: probs,
            max_probability: max_prob,
            latency_us: elapsed_us,
        });

        if (idx + 1) % 50 == 0 || (idx + 1) == pairs.len() {
            println!("Processed {} / {} pairs...", idx + 1, pairs.len());
        }
    }

    let total_duration_ms = total_start.elapsed().as_secs_f64() * 1000.0;
    let avg_pair_latency_ms = if !pairs.is_empty() {
        total_duration_ms / pairs.len() as f64
    } else {
        0.0
    };
    let overall_accuracy = if !pairs.is_empty() {
        (total_matches as f64 / pairs.len() as f64) * 100.0
    } else {
        0.0
    };

    let mut domain_accuracies = HashMap::new();
    for (d, tot) in &domain_total {
        let mat = domain_matches.get(d).cloned().unwrap_or(0);
        let acc = if *tot > 0 {
            (mat as f64 / *tot as f64) * 100.0
        } else {
            0.0
        };
        domain_accuracies.insert(d.clone(), acc);
    }

    let summary = NliBatchSummary {
        total_pairs_scored: pairs.len(),
        total_matches,
        overall_accuracy_pct: overall_accuracy,
        domain_accuracies,
        total_duration_ms,
        avg_pair_latency_ms,
    };

    let batch_output = NliBatchOutput {
        summary,
        raw_results: output_scores,
    };

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let output_json = serde_json::to_string_pretty(&batch_output)?;
    fs::write(&output, output_json)?;

    println!("\n=================================================================");
    println!(" BENCHMARK COMPLETE");
    println!(
        " Overall Accuracy: {:.2}% ({}/{} matches)",
        overall_accuracy,
        total_matches,
        pairs.len()
    );
    for (dom, acc) in &batch_output.summary.domain_accuracies {
        println!("   Domain '{}': {:.2}%", dom, acc);
    }
    println!(
        " Total Time: {:.2}s ({:.2} ms/pair)",
        total_duration_ms / 1000.0,
        avg_pair_latency_ms
    );
    println!(" Output persisted to: {:?}", output);
    println!("=================================================================");

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::BatchNliScore {
            input,
            output,
            model_dir,
        } => {
            run_batch_nli_score(input, output, model_dir)?;
        }
    }

    Ok(())
}
