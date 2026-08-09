//! ============================================================================
//! embedding_bench.rs — Vox v7 Cognitive Memory Embedding Baseline Evaluation Benchmark
//! ============================================================================
//! Category     : Benchmark
//! Component    : Embedding Subsystem (`vox_lib::services::memory::embedder`)
//! Prerequisites: Local ONNX embedding models at `~/.vox/models/embedding/`
//!                Dataset at `sandbox/datasets/vox_embedding_baseline_v1.json`
//!                Optional: NVIDIA_API_KEY in `temp/.env` for NVIDIA NIM evaluation
//! Execution    : cargo test --bench embedding_bench
//! ============================================================================

use anyhow::{anyhow, Result};
use ndarray::Array2;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tokenizers::Tokenizer;

#[derive(Debug, Deserialize)]
struct DatasetMetadata {
    total_samples: usize,
}

#[derive(Debug, Deserialize)]
struct SoftDedupSample {
    fact_a: String,
    fact_b: String,
    is_duplicate: bool,
}

#[derive(Debug, Deserialize)]
struct IntraEdgeSample {
    fact_a: String,
    fact_b: String,
    is_candidate: bool,
}

#[derive(Debug, Deserialize)]
struct InterEdgeSample {
    fact_a: String,
    fact_b: String,
    is_relational: bool,
}

#[derive(Debug, Deserialize)]
struct RagCutoffSample {
    query: String,
    language: String,
    target_fact: String,
    distractor_facts: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BaselineDataset {
    metadata: DatasetMetadata,
    soft_dedup: Vec<SoftDedupSample>,
    intra_edge: Vec<IntraEdgeSample>,
    inter_edge: Vec<InterEdgeSample>,
    rag_cutoff: Vec<RagCutoffSample>,
}

struct ModelInstance {
    name: String,
    session: ort::session::Session,
    tokenizer: Tokenizer,
    dim: usize,
    has_token_type_ids: bool,
}

impl ModelInstance {
    fn load_file(name: &str, dir: &PathBuf, filename: &str, expected_dim: usize) -> Result<Self> {
        let model_path = if dir.join(filename).exists() {
            dir.join(filename)
        } else if dir.join("onnx").join(filename).exists() {
            dir.join("onnx").join(filename)
        } else {
            return Err(anyhow!("File {} not found in {:?}", filename, dir));
        };

        let tokenizer_path = if dir.join("tokenizer.json").exists() {
            dir.join("tokenizer.json")
        } else if dir.join("onnx").join("tokenizer.json").exists() {
            dir.join("onnx").join("tokenizer.json")
        } else {
            dir.join("tokenizer.json")
        };

        if !model_path.exists() || !tokenizer_path.exists() {
            return Err(anyhow!(
                "Missing model ({:?}) or tokenizer ({:?})",
                model_path,
                tokenizer_path
            ));
        }

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

        Ok(Self {
            name: name.to_string(),
            session,
            tokenizer,
            dim: expected_dim,
            has_token_type_ids,
        })
    }

    fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow!("Tokenization failed for text '{}': {:?}", text, e))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();
        let seq_len = ids.len();

        if seq_len == 0 {
            return Ok(vec![0.0f32; self.dim]);
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
        let last_hidden_state = outputs[output_key]
            .try_extract_array::<f32>()
            .map_err(|e| anyhow!("{:?}", e))?;

        let shape = last_hidden_state.shape();
        let out_seq_len = shape[1];
        let hidden_size = shape[2];

        let mut sum_embeddings = vec![0.0f32; hidden_size];
        let mut sum_mask = 0.0f32;

        let encoding_mask = encoding.get_attention_mask();
        for token_idx in 0..out_seq_len {
            let mask_val = if token_idx < encoding_mask.len() {
                encoding_mask[token_idx] as f32
            } else {
                0.0
            };
            sum_mask += mask_val;
            for dim in 0..hidden_size {
                sum_embeddings[dim] += last_hidden_state[[0, token_idx, dim]] * mask_val;
            }
        }

        let divisor = if sum_mask > 0.0 { sum_mask } else { 1.0 };
        for val in sum_embeddings.iter_mut().take(hidden_size) {
            *val /= divisor;
        }

        // L2 Normalization
        let norm: f32 = sum_embeddings.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in sum_embeddings.iter_mut().take(hidden_size) {
                *val /= norm;
            }
        }

        Ok(sum_embeddings)
    }
}

fn cosine_similarity(u: &[f32], v: &[f32]) -> f32 {
    if u.len() != v.len() || u.is_empty() {
        return 0.0;
    }
    let dot: f32 = u.iter().zip(v.iter()).map(|(x, y)| x * y).sum();
    let norm_u: f32 = u.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_v: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_u > 0.0 && norm_v > 0.0 {
        dot / (norm_u * norm_v)
    } else {
        0.0
    }
}

fn get_nvidia_api_key() -> Option<String> {
    if let Ok(key) = std::env::var("NVIDIA_API_KEY") {
        if !key.trim().is_empty() {
            return Some(key.trim().to_string());
        }
    }
    let home = dirs::home_dir()?;
    let env_path = home
        .join("projects")
        .join("apps")
        .join("vox")
        .join("temp")
        .join(".env");
    if env_path.exists() {
        if let Ok(content) = fs::read_to_string(env_path) {
            for line in content.lines() {
                if let Some(key) = line.strip_prefix("NVIDIA_API_KEY=") {
                    let trimmed = key.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }
    None
}

fn fetch_nvidia_embeddings(api_key: &str, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let response = client
        .post("https://integrate.api.nvidia.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "nvidia/nv-embedqa-e5-v5",
            "input": texts,
            "input_type": "passage",
            "encoding_format": "float"
        }))
        .send()?
        .json::<serde_json::Value>()?;

    let mut result = Vec::new();
    if let Some(data) = response["data"].as_array() {
        for item in data {
            if let Some(arr) = item["embedding"].as_array() {
                let vec: Vec<f32> = arr
                    .iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect();
                result.push(vec);
            }
        }
    }
    if result.is_empty() {
        return Err(anyhow!(
            "Failed to retrieve NVIDIA embeddings from API response"
        ));
    }
    Ok(result)
}

fn main() -> Result<()> {
    let home = dirs::home_dir().expect("Could not find home directory");
    let default_models_dir = home.join(".vox").join("models");
    let minilm_l12_dir = default_models_dir.join("embedding").join("minilm-l12-v2");

    let dataset_path = home
        .join("projects")
        .join("apps")
        .join("vox")
        .join("sandbox")
        .join("datasets")
        .join("vox_embedding_baseline_v1.json");

    if !dataset_path.exists() {
        return Err(anyhow!(
            "Baseline dataset not found at {:?}. Run python3 sandbox/generate_baseline_dataset.py first.",
            dataset_path
        ));
    }

    let dataset_json = fs::read_to_string(&dataset_path)?;
    let dataset: BaselineDataset = serde_json::from_str(&dataset_json)?;

    let nv_api_key = get_nvidia_api_key();

    println!(
        "========================================================================================="
    );
    println!(
        "     VOX V7 COGNITIVE MEMORY EMBEDDING BASELINE EVALUATION SCORECARD                    "
    );
    println!(
        "========================================================================================="
    );
    println!(
        "Dataset Loaded: {:?} | Total Samples: {}",
        dataset_path.file_name().unwrap(),
        dataset.metadata.total_samples
    );
    if let Some(ref _key) = nv_api_key {
        println!("NVIDIA NIM API Key: Authenticated (nvidia/nv-embedqa-e5-v5 enabled)");
    } else {
        println!("NVIDIA NIM API Key: Not available (skipping live API endpoint calls)");
    }
    println!();

    let mut minilm = ModelInstance::load_file(
        "MiniLM-L12 (INT8 384d)",
        &minilm_l12_dir,
        "model_int8.onnx",
        384,
    )?;

    let start_eval = Instant::now();

    // ─── SPLIT 1: SOFT DEDUP EVALUATION (Threshold >= 0.95) ───────────────
    println!("--- Split 1: Soft Deduplication (Threshold >= 0.95) ---");
    let mut mini_tp = 0;
    let mut mini_fp = 0;
    let mut mini_fn = 0;
    let mut mini_tn = 0;

    let mut nv_tp = 0;
    let mut nv_fp = 0;
    let mut nv_fn = 0;
    let mut nv_tn = 0;

    for sample in &dataset.soft_dedup {
        let e_a = minilm.embed(&sample.fact_a)?;
        let e_b = minilm.embed(&sample.fact_b)?;
        let sim = cosine_similarity(&e_a, &e_b);

        if sim >= 0.95 {
            if sample.is_duplicate {
                mini_tp += 1;
            } else {
                mini_fp += 1;
            }
        } else {
            if sample.is_duplicate {
                mini_fn += 1;
            } else {
                mini_tn += 1;
            }
        }

        if let Some(ref key) = nv_api_key {
            if let Ok(vecs) = fetch_nvidia_embeddings(key, &[&sample.fact_a, &sample.fact_b]) {
                if vecs.len() == 2 {
                    let nv_sim = cosine_similarity(&vecs[0], &vecs[1]);
                    if nv_sim >= 0.95 {
                        if sample.is_duplicate {
                            nv_tp += 1;
                        } else {
                            nv_fp += 1;
                        }
                    } else {
                        if sample.is_duplicate {
                            nv_fn += 1;
                        } else {
                            nv_tn += 1;
                        }
                    }
                }
            }
        }
    }

    let mini_prec = if (mini_tp + mini_fp) > 0 {
        mini_tp as f32 / (mini_tp + mini_fp) as f32
    } else {
        0.0
    };
    let mini_rec = if (mini_tp + mini_fn) > 0 {
        mini_tp as f32 / (mini_tp + mini_fn) as f32
    } else {
        0.0
    };
    let mini_f1 = if (mini_prec + mini_rec) > 0.0 {
        (2.0 * mini_prec * mini_rec) / (mini_prec + mini_rec)
    } else {
        0.0
    };

    println!(
        "  {:<26} | F1: {:.4} | Precision: {:.4} | Recall: {:.4} | (TP:{}, FP:{}, TN:{}, FN:{})",
        minilm.name, mini_f1, mini_prec, mini_rec, mini_tp, mini_fp, mini_tn, mini_fn
    );

    if nv_api_key.is_some() && (nv_tp + nv_fp + nv_tn + nv_fn) > 0 {
        let nv_prec = if (nv_tp + nv_fp) > 0 {
            nv_tp as f32 / (nv_tp + nv_fp) as f32
        } else {
            0.0
        };
        let nv_rec = if (nv_tp + nv_fn) > 0 {
            nv_tp as f32 / (nv_tp + nv_fn) as f32
        } else {
            0.0
        };
        let nv_f1 = if (nv_prec + nv_rec) > 0.0 {
            (2.0 * nv_prec * nv_rec) / (nv_prec + nv_rec)
        } else {
            0.0
        };
        println!(
            "  {:<26} | F1: {:.4} | Precision: {:.4} | Recall: {:.4} | (TP:{}, FP:{}, TN:{}, FN:{})",
            "NVIDIA NIM (nv-embedqa)", nv_f1, nv_prec, nv_rec, nv_tp, nv_fp, nv_tn, nv_fn
        );
    }

    // ─── SPLIT 2: INTRA-EDGE FILTER EVALUATION (Candidate Recall @ Cutoff >= 0.40)
    println!("\n--- Split 2: Intra-Edge Filter (Candidate Recall @ Cutoff >= 0.40) ---");
    let mut intra_cand_passed = 0;
    let mut intra_cand_total = 0;

    for sample in &dataset.intra_edge {
        if sample.is_candidate {
            intra_cand_total += 1;
            let e_a = minilm.embed(&sample.fact_a)?;
            let e_b = minilm.embed(&sample.fact_b)?;
            let sim = cosine_similarity(&e_a, &e_b);
            if sim >= 0.40 {
                intra_cand_passed += 1;
            }
        }
    }

    let intra_rec = if intra_cand_total > 0 {
        (intra_cand_passed as f32 / intra_cand_total as f32) * 100.0
    } else {
        0.0
    };
    println!(
        "  {:<26} | Candidate Recall @ 0.40: {:.1}% ({}/{})",
        minilm.name, intra_rec, intra_cand_passed, intra_cand_total
    );

    // ─── SPLIT 3: INTER-EDGE FILTER EVALUATION (Cutoff >= 0.55) ─────────────
    println!("\n--- Split 3: Inter-Edge Filter (Precision & Recall @ Cutoff >= 0.55) ---");
    let mut inter_tp = 0;
    let mut inter_fp = 0;
    let mut inter_fn = 0;

    for sample in &dataset.inter_edge {
        let e_a = minilm.embed(&sample.fact_a)?;
        let e_b = minilm.embed(&sample.fact_b)?;
        let sim = cosine_similarity(&e_a, &e_b);

        if sim >= 0.55 {
            if sample.is_relational {
                inter_tp += 1;
            } else {
                inter_fp += 1;
            }
        } else if sample.is_relational {
            inter_fn += 1;
        }
    }

    let inter_prec = if (inter_tp + inter_fp) > 0 {
        inter_tp as f32 / (inter_tp + inter_fp) as f32
    } else {
        0.0
    };
    let inter_rec = if (inter_tp + inter_fn) > 0 {
        inter_tp as f32 / (inter_tp + inter_fn) as f32
    } else {
        0.0
    };
    println!(
        "  {:<26} | Precision: {:.4} | Recall: {:.4} | (TP:{}, FP:{})",
        minilm.name, inter_prec, inter_rec, inter_tp, inter_fp
    );

    // ─── SPLIT 4: RAG CUTOFF EVALUATION (Asymmetric Speech Query Search) ────
    println!("\n--- Split 4: RAG Cutoff (Asymmetric Speech Query-vs-Fact Search) ---");
    let eng_samples: Vec<&RagCutoffSample> = dataset
        .rag_cutoff
        .iter()
        .filter(|s| s.language == "English")
        .collect();
    let hinglish_samples: Vec<&RagCutoffSample> = dataset
        .rag_cutoff
        .iter()
        .filter(|s| s.language == "Hinglish")
        .collect();

    let mut evaluate_rag_subsplit = |samples: &[&RagCutoffSample], label: &str| -> Result<()> {
        let mut hits_at_3 = 0;
        let mut total_margin = 0.0f32;

        for sample in samples {
            let q_emb = minilm.embed(&sample.query)?;
            let t_emb = minilm.embed(&sample.target_fact)?;
            let target_sim = cosine_similarity(&q_emb, &t_emb);

            let mut distractor_sims = Vec::new();
            for d in &sample.distractor_facts {
                let d_emb = minilm.embed(d)?;
                distractor_sims.push(cosine_similarity(&q_emb, &d_emb));
            }

            let max_distractor_sim = distractor_sims.iter().cloned().fold(0.0f32, f32::max);
            let margin = target_sim - max_distractor_sim;
            total_margin += margin;

            let mut all_sims = vec![(0, target_sim)];
            for (idx, &d_sim) in distractor_sims.iter().enumerate() {
                all_sims.push((idx + 1, d_sim));
            }
            all_sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            let top_3_ids: Vec<usize> = all_sims.iter().take(3).map(|s| s.0).collect();
            if top_3_ids.contains(&0) {
                hits_at_3 += 1;
            }
        }

        let acc = if !samples.is_empty() {
            (hits_at_3 as f32 / samples.len() as f32) * 100.0
        } else {
            0.0
        };
        let avg_margin = if !samples.is_empty() {
            total_margin / samples.len() as f32
        } else {
            0.0
        };

        println!(
            "  {:<26} | RAG {:<10} | Top-3 Recall: {:.1}% ({}/{}) | Avg Cosine Margin: +{:.4}",
            minilm.name,
            label,
            acc,
            hits_at_3,
            samples.len(),
            avg_margin
        );
        Ok(())
    };

    evaluate_rag_subsplit(&eng_samples, "English")?;
    evaluate_rag_subsplit(&hinglish_samples, "Hinglish")?;

    let dur = start_eval.elapsed();
    println!(
        "\nTotal Benchmark Execution Time: {:.2}s across {} samples",
        dur.as_secs_f32(),
        dataset.metadata.total_samples
    );
    println!("=========================================================================================\n");

    Ok(())
}
