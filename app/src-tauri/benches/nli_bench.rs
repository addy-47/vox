//! ============================================================================
//! nli_bench.rs — ONNX DeBERTa-v3-xsmall NLI Classification Benchmark
//! ============================================================================
//! Category     : Benchmark
//! Component    : NLI Classifier Engine (`vox_lib::services::memory::nli`)
//! Prerequisites: Local ONNX NLI model at `~/.vox/models/nli/deberta-v3-xsmall-nli/`
//! Execution    : cargo test --bench nli_bench
//! ============================================================================

use anyhow::{anyhow, Result};
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokenizers::Tokenizer;

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
}

#[derive(Debug, Deserialize, Clone)]
struct TestPair {
    premise: String,
    hypothesis: String,
    label: String, // "Entailment", "Contradiction", "Neutral"
    category: String,
    language: String,
}

struct ModelConfig {
    name: String,
    dir_name: String,
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
            return Err(anyhow!("Missing files in {:?}", dir));
        }

        let model_size_bytes = fs::metadata(&model_path)?.len();
        let model_size_mb = model_size_bytes as f64 / (1024.0 * 1024.0);

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer from {:?}: {}", tokenizer_path, e))?;

        let session = ort::session::Session::builder()
            .map_err(|e| anyhow!("{:?}", e))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow!("{:?}", e))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("{:?}", e))?
            .commit_from_file(&model_path)
            .map_err(|e| anyhow!("{:?}", e))?;

        let has_token_type_ids = session.inputs().iter().any(|i| i.name() == "token_type_ids");
        let class_mapping = [NliLabel::Contradiction, NliLabel::Entailment, NliLabel::Neutral];

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

        let ent_idx = logits_ent.iter().enumerate().max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap()).unwrap().0;
        let con_idx = logits_con.iter().enumerate().max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap()).unwrap().0;

        // Ensure indices do not collide (fallback to standard mapping if calibration is unstable)
        if ent_idx == con_idx {
            println!("  [WARN] Calibration collision for {}. Falling back to default labels.", self.name);
            if self.name.contains("BART") {
                self.class_mapping = [NliLabel::Contradiction, NliLabel::Neutral, NliLabel::Entailment];
            } else if self.name.contains("mDeBERTa") {
                self.class_mapping = [NliLabel::Contradiction, NliLabel::Entailment, NliLabel::Neutral];
            } else {
                self.class_mapping = [NliLabel::Contradiction, NliLabel::Entailment, NliLabel::Neutral];
            }
        } else {
            let mut indices = vec![0, 1, 2];
            indices.retain(|&x| x != ent_idx && x != con_idx);
            let neu_idx = indices[0];

            self.class_mapping[ent_idx] = NliLabel::Entailment;
            self.class_mapping[con_idx] = NliLabel::Contradiction;
            self.class_mapping[neu_idx] = NliLabel::Neutral;
        }

        println!("  Calibrated Mapping for {}: [0: {:?}, 1: {:?}, 2: {:?}]", 
                 self.name, self.class_mapping[0], self.class_mapping[1], self.class_mapping[2]);

        Ok(())
    }

    fn raw_predict(&mut self, premise: &str, hypothesis: &str) -> Result<Vec<f32>> {
        let encoding = self.tokenizer
            .encode((premise, hypothesis), true)
            .map_err(|e| anyhow!("Tokenization failed: {:?}", e))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();
        let seq_len = ids.len();

        if seq_len == 0 {
            return Ok(vec![0.0, 0.0, 0.0]);
        }

        let input_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), ids)?;
        let attention_mask_arr = Array2::<i64>::from_shape_vec((1, seq_len), mask)?;

        let input_ids_tensor = ort::value::Tensor::from_array(input_ids_arr)
            .map_err(|e| anyhow!("{:?}", e))?;
        let attention_mask_tensor = ort::value::Tensor::from_array(attention_mask_arr)
            .map_err(|e| anyhow!("{:?}", e))?;

        let outputs = if self.has_token_type_ids {
            let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();
            let type_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), type_ids)?;
            let type_ids_tensor = ort::value::Tensor::from_array(type_ids_arr)
                .map_err(|e| anyhow!("{:?}", e))?;
            self.session.run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "token_type_ids" => type_ids_tensor
            ]).map_err(|e| anyhow!("{:?}", e))?
        } else {
            self.session.run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor
            ]).map_err(|e| anyhow!("{:?}", e))?
        };

        let output_key = outputs.keys().next().ok_or_else(|| anyhow!("No output in model"))?;
        let logits_array = outputs[output_key].try_extract_array::<f32>().map_err(|e| anyhow!("{:?}", e))?;

        Ok(vec![logits_array[[0, 0]], logits_array[[0, 1]], logits_array[[0, 2]]])
    }

    fn predict(&mut self, premise: &str, hypothesis: &str) -> Result<(NliLabel, f32, u128)> {
        let start = Instant::now();
        let logits = self.raw_predict(premise, hypothesis)?;
        let elapsed = start.elapsed().as_nanos(); // use nanoseconds for high precision

        // Softmax
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exps.iter().sum();
        let probs: Vec<f32> = exps.iter().map(|e| e / sum_exp).collect();

        let (max_idx, &max_prob) = probs.iter().enumerate().max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap()).unwrap();
        let label = self.class_mapping[max_idx];

        Ok((label, max_prob, elapsed))
    }
}

fn main() -> Result<()> {
    let home = dirs::home_dir().expect("Could not find home directory");
    let nli_dir = home.join(".vox").join("models").join("nli");
    let test_suite_path = PathBuf::from("/home/addy/projects/apps/vox/temp/nli_test_suite.json");

    if !test_suite_path.exists() {
        return Err(anyhow!("Test suite file not found at {:?}", test_suite_path));
    }

    println!("Loading test suite from {:?}...", test_suite_path);
    let suite_content = fs::read_to_string(&test_suite_path)?;
    let test_cases: Vec<TestPair> = serde_json::from_str(&suite_content)?;
    println!("Loaded {} test pairs successfully!", test_cases.len());

    let configs = vec![
        ModelConfig {
            name: "nli-MiniLM2-L6-H768".to_string(),
            dir_name: "nli-minilm2-l6-h768".to_string(),
        },
        ModelConfig {
            name: "DeBERTa-v3-xSmall-NLI".to_string(),
            dir_name: "deberta-v3-xsmall".to_string(),
        },
        ModelConfig {
            name: "DeBERTa-v3-Small-NLI".to_string(),
            dir_name: "deberta-v3-small-nli".to_string(),
        },
        ModelConfig {
            name: "DeBERTa-v3-base-mnli-ONNX".to_string(),
            dir_name: "deberta-v3-base-mnli-onnx".to_string(),
        },
        ModelConfig {
            name: "mDeBERTa-v3-base".to_string(),
            dir_name: "mdeberta-v3-base-xnli".to_string(),
        },
        ModelConfig {
            name: "BART-Large-MNLI".to_string(),
            dir_name: "bart-large-mnli".to_string(),
        },
        ModelConfig {
            name: "BART-Large-MNLI-ONNX".to_string(),
            dir_name: "bart-large-mnli-onnx".to_string(),
        },
    ];

    let mut loaded_models = Vec::new();
    for config in &configs {
        let path = nli_dir.join(&config.dir_name);
        println!("\nLoading and calibrating {} from {:?}...", config.name, path);
        match ModelInstance::load(&config.name, &path) {
            Ok(model) => {
                println!("  [SUCCESS] Loaded. Size: {:.2} MB", model.model_size_mb);
                loaded_models.push(model);
            }
            Err(e) => {
                println!("  [FAILED] Failed to load model {}: {}", config.name, e);
            }
        }
    }

    if loaded_models.is_empty() {
        return Err(anyhow!("No models could be loaded."));
    }

    println!("\n==========================================================================");
    println!("  RUNNING BULK MULTI-LINGUAL BENCHMARK ON {} PAIRS", test_cases.len());
    println!("==========================================================================");

    // Track detailed results for final report
    struct ModelMetrics {
        name: String,
        size_mb: f64,
        total_latency_ns: u128,
        total_correct: usize,
        category_correct: HashMap<String, usize>,
        category_total: HashMap<String, usize>,
        lang_correct: HashMap<String, usize>,
        lang_total: HashMap<String, usize>,
    }

    let mut metrics_report = Vec::new();

    for model in &mut loaded_models {
        println!("Evaluating {}...", model.name);
        let mut total_latency_ns = 0;
        let mut total_correct = 0;
        let mut category_correct = HashMap::new();
        let mut category_total = HashMap::new();
        let mut lang_correct = HashMap::new();
        let mut lang_total = HashMap::new();

        // Warmup pass
        for tc in test_cases.iter().take(50) {
            let _ = model.predict(&tc.premise, &tc.hypothesis)?;
        }

        // Real pass
        for tc in &test_cases {
            let (predicted_label, _, lat_ns) = model.predict(&tc.premise, &tc.hypothesis)?;
            total_latency_ns += lat_ns;

            let expected_label = match tc.label.as_str() {
                "Entailment" => NliLabel::Entailment,
                "Contradiction" => NliLabel::Contradiction,
                _ => NliLabel::Neutral,
            };

            let is_correct = predicted_label == expected_label;
            
            // Stats updates
            *lang_total.entry(tc.language.clone()).or_insert(0) += 1;
            *category_total.entry(tc.category.clone()).or_insert(0) += 1;
            
            if is_correct {
                total_correct += 1;
                *lang_correct.entry(tc.language.clone()).or_insert(0) += 1;
                *category_correct.entry(tc.category.clone()).or_insert(0) += 1;
            }
        }

        metrics_report.push(ModelMetrics {
            name: model.name.clone(),
            size_mb: model.model_size_mb,
            total_latency_ns,
            total_correct,
            category_correct,
            category_total,
            lang_correct,
            lang_total,
        });
    }

    // Output Markdown Report
    let mut markdown = String::new();
    markdown.push_str("# Local NLI Model Benchmark Validation Report\n\n");
    markdown.push_str(&format!("Generated across **{}** generated English & Hindi test cases.\n\n", test_cases.len()));
    
    markdown.push_str("## Overall Model Summary\n\n");
    markdown.push_str("| Model | Disk Size (MB) | Accuracy | Avg Latency (CPU) |\n");
    markdown.push_str("| :--- | :---: | :---: | :---: |\n");
    
    for m in &metrics_report {
        let acc = (m.total_correct as f64 / test_cases.len() as f64) * 100.0;
        let avg_lat_ms = (m.total_latency_ns as f64 / test_cases.len() as f64) / 1_000_000.0;
        markdown.push_str(&format!("| **{}** | {:.2} MB | **{:.2}%** | {:.2} ms |\n", m.name, m.size_mb, acc, avg_lat_ms));
    }
    markdown.push_str("\n---\n\n## Accuracy Breakdown by Language\n\n");
    
    // Header for lang table
    let mut lang_header = "| Model ".to_string();
    let mut lang_sub = "| :--- ".to_string();
    let mut languages: Vec<String> = test_cases.iter().map(|t| t.language.clone()).collect();
    languages.sort();
    languages.dedup();
    
    for l in &languages {
        lang_header.push_str(&format!("| {} Accuracy ", l));
        lang_sub.push_str("| :---: ");
    }
    lang_header.push_str("|\n");
    lang_sub.push_str("|\n");
    markdown.push_str(&lang_header);
    markdown.push_str(&lang_sub);

    for m in &metrics_report {
        let mut row = format!("| **{}** ", m.name);
        for l in &languages {
            let correct = m.lang_correct.get(l).cloned().unwrap_or(0);
            let total = m.lang_total.get(l).cloned().unwrap_or(0);
            let pct = if total > 0 { (correct as f64 / total as f64) * 100.0 } else { 0.0 };
            row.push_str(&format!("| {:.2}% ({}/{}) ", pct, correct, total));
        }
        row.push_str("|\n");
        markdown.push_str(&row);
    }

    markdown.push_str("\n---\n\n## Accuracy Breakdown by Semantic Category\n\n");
    let mut cat_header = "| Model ".to_string();
    let mut cat_sub = "| :--- ".to_string();
    let mut categories: Vec<String> = test_cases.iter().map(|t| t.category.clone()).collect();
    categories.sort();
    categories.dedup();
    
    for c in &categories {
        cat_header.push_str(&format!("| {} ", c));
        cat_sub.push_str("| :---: ");
    }
    cat_header.push_str("|\n");
    cat_sub.push_str("|\n");
    markdown.push_str(&cat_header);
    markdown.push_str(&cat_sub);

    for m in &metrics_report {
        let mut row = format!("| **{}** ", m.name);
        for c in &categories {
            let correct = m.category_correct.get(c).cloned().unwrap_or(0);
            let total = m.category_total.get(c).cloned().unwrap_or(0);
            let pct = if total > 0 { (correct as f64 / total as f64) * 100.0 } else { 0.0 };
            row.push_str(&format!("| {:.1}% ", pct));
        }
        row.push_str("|\n");
        markdown.push_str(&row);
    }

    fs::write("/home/addy/projects/apps/vox/temp/nli_benchmark_report.md", markdown)?;
    println!("\n[SUCCESS] Final Markdown Report written to: /home/addy/projects/apps/vox/temp/nli_benchmark_report.md");

    // Print beautiful console report
    println!("\n==========================================================================");
    println!(" FINAL LOCAL NLI BENCHMARK REPORT (Console Summary)");
    println!("==========================================================================");
    println!("{:<28} | {:<12} | {:<12} | {:<16} | {:<15}", "MODEL", "DISK SIZE", "AVG LATENCY", "ENGLISH ACCURACY", "OVERALL ACCURACY");
    println!("-------------------------------------------------------------------------------------------------------");
    for m in &metrics_report {
        let acc = (m.total_correct as f64 / test_cases.len() as f64) * 100.0;
        let avg_lat_ms = (m.total_latency_ns as f64 / test_cases.len() as f64) / 1_000_000.0;
        
        let eng_correct = m.lang_correct.get("English").cloned().unwrap_or(0);
        let eng_total = m.lang_total.get("English").cloned().unwrap_or(0);
        let eng_acc = if eng_total > 0 { (eng_correct as f64 / eng_total as f64) * 100.0 } else { 0.0 };
        
        println!("{:<28} | {:<12.2} MB | {:<12.2} ms | {:<15.2}% | {:<15.2}%", m.name, m.size_mb, avg_lat_ms, eng_acc, acc);
    }
    println!("==========================================================================");

    Ok(())
}
