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
    fn load(name: &str, dir: &PathBuf, expected_dim: usize) -> Result<Self> {
        let model_path = if dir.join("model_quantized.onnx").exists() {
            dir.join("model_quantized.onnx")
        } else if dir.join("model_int8.onnx").exists() {
            dir.join("model_int8.onnx")
        } else {
            dir.join("model.onnx")
        };
        let tokenizer_path = dir.join("tokenizer.json");

        if !model_path.exists() || !tokenizer_path.exists() {
            return Err(anyhow!("Missing files at {:?}", dir));
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
            .map_err(|e| anyhow!("Tokenization failed: {:?}", e))?;

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

struct TestCase {
    category: &'static str,
    query: &'static str,
    doc: &'static str,
    description: &'static str,
}

fn main() -> Result<()> {
    let home = dirs::home_dir().expect("Could not find home directory");
    let memory_models_dir = home.join(".vox").join("models").join("memory");

    let minilm_dir = memory_models_dir.join("minilm");
    let bge_m3_dir = memory_models_dir.join("bge-m3");

    println!("============================================================");
    println!("  EMBEDDING MODEL BENCHMARK: MiniLM-L6-v2 vs BGE-M3        ");
    println!("============================================================");
    println!(" MiniLM Path : {:?}", minilm_dir);
    println!(" BGE-M3 Path : {:?}", bge_m3_dir);

    println!("\nLoading models into ONNX Runtime session...");
    let mut minilm = ModelInstance::load("MiniLM-L6-v2 (384-dim)", &minilm_dir, 384)?;
    println!("  [SUCCESS] MiniLM-L6-v2 loaded (384-dim).");

    let mut bge_m3 = ModelInstance::load("BGE-M3 (1024-dim)", &bge_m3_dir, 1024)?;
    println!("  [SUCCESS] BGE-M3 loaded (1024-dim, Multilingual).\n");

    let monolithic_summary = "Alex is a senior system engineer building Vox in Rust. He dislikes Python for high-performance backends. His favorite color is teal. Vox targets sub-500ms end-to-end latency across VAD, STT, LLM, and TTS. STT recommended is Vosk or Whisper.cpp. They also discussed Christopher Nolan directing Interstellar with Hans Zimmer pipe organ score, New Delhi capital of India, calculus derivatives, and quantum entanglement.";

    let bullet_chunk_color = "User preference: Alex's favorite color is teal.";
    let bullet_chunk_lang = "Technical role: Alex is a senior system engineer building Vox in Rust and dislikes Python for backends.";

    let test_cases = vec![
        TestCase {
            category: "Failure Scenario 1 (Monolithic)",
            query: "What favorite color did I mention?",
            doc: monolithic_summary,
            description: "Short query vs 200-word monolithic summary",
        },
        TestCase {
            category: "Failure Scenario 2 (Monolithic)",
            query: "What programming language did I say was my favorite for backends?",
            doc: monolithic_summary,
            description: "Specific fact query vs monolithic summary",
        },
        TestCase {
            category: "Bullet-Chunk Ingestion (Color)",
            query: "What favorite color did I mention?",
            doc: bullet_chunk_color,
            description: "Short query vs focused bullet chunk",
        },
        TestCase {
            category: "Bullet-Chunk Ingestion (Language)",
            query: "What programming language did I say was my favorite for backends?",
            doc: bullet_chunk_lang,
            description: "Specific query vs focused bullet chunk",
        },
        TestCase {
            category: "Multilingual Hindi Query (Devanagari)",
            query: "नमस्ते, क्या आपको याद है मेरी पसंदीदा भाषा कौन सी है?",
            doc: monolithic_summary,
            description: "Hindi Devanagari query vs English summary",
        },
        TestCase {
            category: "Multilingual Hinglish Query",
            query: "Mera favorite color aur application name batao please",
            doc: monolithic_summary,
            description: "Hinglish query vs English summary",
        },
        TestCase {
            category: "Multilingual Pure Hindi Pair",
            query: "भारत की राजधानी क्या है?",
            doc: "भारत की राजधानी नई दिल्ली है। लाल किला शाहजहाँ द्वारा 17वीं शताब्दी में बनवाया गया था।",
            description: "Hindi query vs Hindi summary text",
        },
    ];

    println!("───────────────────────────────────────────────────────────────────────────────────────────");
    println!("{:<32} | {:<12} | {:<12} | {:<10} | {:<10}", "TEST CASE", "MINILM SIM", "BGE-M3 SIM", "MINILM HIT", "BGE-M3 HIT");
    println!("───────────────────────────────────────────────────────────────────────────────────────────");

    let threshold = 0.55f32;
    let mut minilm_hits = 0;
    let mut bge_hits = 0;
    let mut minilm_latencies = Vec::new();
    let mut bge_latencies = Vec::new();

    for tc in &test_cases {
        let (q_minilm, q_m_lat) = minilm.embed(tc.query)?;
        let (d_minilm, d_m_lat) = minilm.embed(tc.doc)?;
        let minilm_sim = cosine_similarity(&q_minilm, &d_minilm);
        let minilm_pass = minilm_sim >= threshold;

        let (q_bge, q_b_lat) = bge_m3.embed(tc.query)?;
        let (d_bge, d_b_lat) = bge_m3.embed(tc.doc)?;
        let bge_sim = cosine_similarity(&q_bge, &d_bge);
        let bge_pass = bge_sim >= threshold;

        minilm_latencies.push(q_m_lat + d_m_lat);
        bge_latencies.push(q_b_lat + d_b_lat);

        if minilm_pass { minilm_hits += 1; }
        if bge_pass { bge_hits += 1; }

        println!(
            "{:<32} | {:<12.4} | {:<12.4} | {:<10} | {:<10}",
            tc.category,
            minilm_sim,
            bge_sim,
            if minilm_pass { "PASS (>=0.55)" } else { "FAIL (<0.55)" },
            if bge_pass { "PASS (>=0.55)" } else { "FAIL (<0.55)" }
        );
    }

    let avg_minilm_lat = minilm_latencies.iter().sum::<u128>() as f64 / (minilm_latencies.len() as f64 * 1000.0);
    let avg_bge_lat = bge_latencies.iter().sum::<u128>() as f64 / (bge_latencies.len() as f64 * 1000.0);

    println!("───────────────────────────────────────────────────────────────────────────────────────────\n");
    println!("============================================================");
    println!("            BENCHMARK RESULTS & COMPARISON SUMMARY           ");
    println!("============================================================");
    println!(" Evaluated Threshold (0.55)     : Strict Requirement");
    println!(" Total Benchmark Scenarios      : {}", test_cases.len());
    println!(" MiniLM-L6-v2 Pass Rate (0.55)  : {} / {} ({:.1}%)", minilm_hits, test_cases.len(), (minilm_hits as f32 / test_cases.len() as f32) * 100.0);
    println!(" BGE-M3 Pass Rate (0.55)        : {} / {} ({:.1}%)", bge_hits, test_cases.len(), (bge_hits as f32 / test_cases.len() as f32) * 100.0);
    println!(" MiniLM Avg Latency per Pair    : {:.2} ms", avg_minilm_lat);
    println!(" BGE-M3 Avg Latency per Pair    : {:.2} ms", avg_bge_lat);
    println!(" MiniLM Output Dimensions       : 384 dimensions");
    println!(" BGE-M3 Output Dimensions       : 1024 dimensions");
    println!(" Multilingual Support           : MiniLM=English Only | BGE-M3=100+ Languages");
    println!("============================================================\n");

    Ok(())
}
