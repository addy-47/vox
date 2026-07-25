//! ============================================================================
//! embedding_bench.rs — ONNX MiniLM vs BGE-M3 Embedding Performance Benchmark
//! ============================================================================
//! Category     : Benchmark
//! Component    : Embedding Engine (`vox_lib::services::memory::embedder`)
//! Prerequisites: Local ONNX embedding models at `~/.vox/models/embedding/`
//! Execution    : cargo test --bench embedding_bench
//! ============================================================================

use anyhow::{anyhow, Result};
use ndarray::Array2;
use std::path::PathBuf;
use std::time::Instant;
use tokenizers::Tokenizer;

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
            return Err(anyhow!("Missing model ({:?}) or tokenizer ({:?})", model_path, tokenizer_path));
        }

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

        Ok(Self {
            name: name.to_string(),
            session,
            tokenizer,
            dim: expected_dim,
            has_token_type_ids,
        })
    }

    fn embed(&mut self, text: &str) -> Result<(Vec<f32>, u128)> {
        let start = Instant::now();
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
            return Ok((vec![0.0f32; self.dim], start.elapsed().as_micros()));
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
        let last_hidden_state = outputs[output_key].try_extract_array::<f32>()
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
        for dim in 0..hidden_size {
            sum_embeddings[dim] /= divisor;
        }

        // L2 Normalization
        let norm: f32 = sum_embeddings.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for dim in 0..hidden_size {
                sum_embeddings[dim] /= norm;
            }
        }

        let latency_us = start.elapsed().as_micros();
        Ok((sum_embeddings, latency_us))
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

struct CategoryPair {
    category: &'static str,
    query: &'static str,
    fact: &'static str,
}

fn main() -> Result<()> {
    let home = dirs::home_dir().expect("Could not find home directory");
    let candidates_dir = home.join(".vox").join("models").join("memory_candidates");
    let default_models_dir = home.join(".vox").join("models");

    let bge_m3_dir = if default_models_dir.join("embedding").join("bge-m3").exists() {
        default_models_dir.join("embedding").join("bge-m3")
    } else {
        default_models_dir.join("models").join("bge-m3")
    };

    let minilm_l12_dir = if default_models_dir.join("embedding").join("minilm-l12-v2").exists() {
        default_models_dir.join("embedding").join("minilm-l12-v2")
    } else {
        candidates_dir.join("xenova-paraphrase-multilingual-MiniLM-L12-v2")
    };

    println!("=========================================================================================");
    println!("     EMPIRICAL SIDE-BY-SIDE EVALUATION: BGE-M3 (1024d) vs MINILM-L12 INT8 (384d)        ");
    println!("=========================================================================================\n");

    let mut bge_m3 = ModelInstance::load_file("BGE-M3 (1024d)", &bge_m3_dir, "model_quantized.onnx", 1024)?;
    let mut minilm = ModelInstance::load_file("MiniLM-L12 (INT8 384d)", &minilm_l12_dir, "model_int8.onnx", 384)?;

    // ─── PART 1: 5 RELATIONSHIP CATEGORIES SIDE-BY-SIDE ───────────────
    let category_pairs = vec![
        CategoryPair {
            category: "1. Exact / Identity Match",
            query: "Alex's favorite color is teal",
            fact: "User preference: Alex's favorite color is teal.",
        },
        CategoryPair {
            category: "2. Similar / Paraphrased",
            query: "What backend language does Alex prefer?",
            fact: "Technical role: Alex is a senior system engineer building Vox in Rust and dislikes Python for backends.",
        },
        CategoryPair {
            category: "3. Semantically Related Domain",
            query: "Are there any microphone or audio recording issues?",
            fact: "Active task: Fix microphone permissions error on Linux for voice capture.",
        },
        CategoryPair {
            category: "4. Unrelated / Different Topic",
            query: "What programming language do I like?",
            fact: "User experience: Alex visited Japan last summer and tried sushi.",
        },
        CategoryPair {
            category: "5. Completely Different / Noise",
            query: "What is my marathon running goal?",
            fact: "User preference: Alex dislikes rainy weather and prefers coffee over tea.",
        },
    ];

    println!("--------------------------------------------------------------------------------------------------");
    println!("{:<32} | {:<12} | {:<12} | {:<12} | {:<12}", "CATEGORY", "BGE-M3 SIM", "MINILM SIM", "BGE MARGIN*", "MINILM MARGIN*");
    println!("--------------------------------------------------------------------------------------------------");

    let bge_baseline_noise = 0.60f32;
    let minilm_baseline_noise = 0.10f32;

    for cp in &category_pairs {
        let (q_bge, _) = bge_m3.embed(cp.query)?;
        let (f_bge, _) = bge_m3.embed(cp.fact)?;
        let bge_sim = cosine_similarity(&q_bge, &f_bge);
        let bge_margin = bge_sim - bge_baseline_noise;

        let (q_mini, _) = minilm.embed(cp.query)?;
        let (f_mini, _) = minilm.embed(cp.fact)?;
        let mini_sim = cosine_similarity(&q_mini, &f_mini);
        let mini_margin = mini_sim - minilm_baseline_noise;

        println!(
            "{:<32} | {:<12.4} | {:<12.4} | {:<12.4} | {:<12.4}",
            cp.category,
            bge_sim,
            mini_sim,
            bge_margin,
            mini_margin
        );
    }
    println!("--------------------------------------------------------------------------------------------------\n");

    // ─── PART 2: TOP-K RETRIEVAL SCAN ACROSS 10 REAL EXTRACTED FACTS ──
    println!("=========================================================================================");
    println!("     TOP-K RETRIEVAL MATRIX SCAN OVER 10 REAL EXTRACTED SESSION FACTS                    ");
    println!("=========================================================================================\n");

    let real_facts = vec![
        "User preference: Alex's favorite color is teal.",
        "Technical role: Alex is a senior system engineer building Vox in Rust and dislikes Python for backends.",
        "User preference: Alex lives in New Delhi and likes coffee.",
        "Active task: Fix microphone permissions error on Linux for voice capture.",
        "Active goal: Read 12 technical books before the end of the year.",
        "Active goal: User aims to run a half-marathon under 2 hours in October.",
        "User relationship: User has a sister named Sarah who lives in Boston.",
        "User preference: User bought a red bicycle yesterday for commuting.",
        "User experience: Alex visited Japan last summer and tried authentic ramen.",
        "User preference: Alex dislikes rainy weather and prefers indoor workouts.",
    ];

    println!("Indexing 10 Real Extracted Facts into Vector Memory...");
    let mut bge_fact_embeddings = Vec::new();
    for f in &real_facts {
        let (emb, _) = bge_m3.embed(f)?;
        bge_fact_embeddings.push(emb);
    }

    let mut mini_fact_embeddings = Vec::new();
    for f in &real_facts {
        let (emb, _) = mini_fact_embeddings_embed(&mut minilm, f)?;
        mini_fact_embeddings.push(emb);
    }

    let sample_queries = vec![
        ("English Query", "What favorite color did I mention?"),
        ("Hinglish Query", "Mera marathon running goal kya tha?"),
        ("Hindi Query", "नमस्ते, क्या आपको याद है मेरी पसंदीदा भाषा कौन सी है?"),
    ];

    let bge_threshold = 0.65f32;
    let minilm_threshold = 0.40f32;

    for (q_label, query_text) in sample_queries {
        println!("\n>>> SAMPLE QUERY: [{}] \"{}\"", q_label, query_text);
        
        // 1. Evaluate BGE-M3 Top-3 Retrieval
        let (q_bge, _) = bge_m3.embed(query_text)?;
        let mut bge_scores: Vec<(usize, f32)> = bge_fact_embeddings
            .iter()
            .enumerate()
            .map(|(idx, f_emb)| (idx, cosine_similarity(&q_bge, f_emb)))
            .collect();
        bge_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        println!("  [BGE-M3 (Threshold {:.2})] Top-3 Retrieved Facts:", bge_threshold);
        for rank in 0..3 {
            let (fact_idx, score) = bge_scores[rank];
            let status = if score >= bge_threshold { "RETRIEVED (PASS)" } else { "FILTERED (NOISE)" };
            println!("    Rank {}: [{:.4}] [{}] \"{}\"", rank + 1, score, status, real_facts[fact_idx]);
        }

        // 2. Evaluate MiniLM-L12 Top-3 Retrieval
        let (q_mini, _) = minilm.embed(query_text)?;
        let mut mini_scores: Vec<(usize, f32)> = mini_fact_embeddings
            .iter()
            .enumerate()
            .map(|(idx, f_emb)| (idx, cosine_similarity(&q_mini, f_emb)))
            .collect();
        mini_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        println!("  [MiniLM-L12 (Threshold {:.2})] Top-3 Retrieved Facts:", minilm_threshold);
        for rank in 0..3 {
            let (fact_idx, score) = mini_scores[rank];
            let status = if score >= minilm_threshold { "RETRIEVED (PASS)" } else { "FILTERED (NOISE)" };
            println!("    Rank {}: [{:.4}] [{}] \"{}\"", rank + 1, score, status, real_facts[fact_idx]);
        }
    }

    println!("\n=========================================================================================\n");
    Ok(())
}

fn mini_fact_embeddings_embed(model: &mut ModelInstance, text: &str) -> Result<(Vec<f32>, u128)> {
    model.embed(text)
}
