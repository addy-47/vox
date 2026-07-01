use clap::Parser;
use ndarray::Array2;
use ort::session::Session;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tokenizers::Tokenizer;
use vox_lib::utils::bench_reporter::BenchReporter;

#[derive(Parser, Debug)]
#[command(
    name = "embedding-bench",
    about = "Benchmark MiniLM multilingual embedding model for memory"
)]
struct Args {
    /// Path to model directory (contains model_int8.onnx and tokenizer.json)
    #[arg(short, long)]
    model_dir: Option<String>,

    /// Prefix for output run directory (e.g. 'minilm' -> outputs/minilm_run_...)
    #[arg(short, long)]
    output: Option<String>,
}

fn cosine_similarity(u: &[f32], v: &[f32]) -> f32 {
    if u.len() != v.len() {
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

fn get_embeddings(
    session: &mut Session,
    tokenizer: &Tokenizer,
    text: &str,
    has_token_type_ids: bool,
) -> anyhow::Result<(Vec<f32>, usize)> {
    let encoding = tokenizer
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("Tokenization failed: {:?}", e))?;

    let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let mask: Vec<i64> = encoding
        .get_attention_mask()
        .iter()
        .map(|&x| x as i64)
        .collect();
    let seq_len = ids.len();

    let input_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), ids)?;
    let attention_mask_arr = Array2::<i64>::from_shape_vec((1, seq_len), mask)?;

    let input_ids_tensor = ort::value::Tensor::from_array(input_ids_arr)?;
    let attention_mask_tensor = ort::value::Tensor::from_array(attention_mask_arr)?;

    let outputs = if has_token_type_ids {
        let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();
        let type_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), type_ids)?;
        let type_ids_tensor = ort::value::Tensor::from_array(type_ids_arr)?;

        session.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
            "token_type_ids" => type_ids_tensor
        ])?
    } else {
        session.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor
        ])?
    };

    let output_key = outputs
        .keys()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No output in model"))?;
    let last_hidden_state = outputs[output_key].try_extract_array::<f32>()?;

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

    Ok((sum_embeddings, seq_len))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let reporter = BenchReporter::new_with_prefix(args.output.as_deref());

    let home = dirs::home_dir().expect("Could not find home directory");
    let model_dir = match args.model_dir {
        Some(d) => PathBuf::from(d),
        None => home.join(".vox/models/memory/minilm"),
    };

    let model_path = model_dir.join("model_int8.onnx");
    let tokenizer_path = model_dir.join("tokenizer.json");

    if !model_path.exists() {
        anyhow::bail!("Model file not found at {:?}", model_path);
    }
    if !tokenizer_path.exists() {
        anyhow::bail!("Tokenizer file not found at {:?}", tokenizer_path);
    }

    println!("\x1b[32m[Embedding-Bench]\x1b[0m Model path: {:?}", model_path);
    println!("\x1b[32m[Embedding-Bench]\x1b[0m Tokenizer path: {:?}", tokenizer_path);

    // Measure load time and memory usage delta
    let snap_before = BenchReporter::get_memory_snapshot();
    let load_start = Instant::now();

    let mut session = Session::builder()
        .map_err(|e| anyhow::anyhow!("Failed to create session builder: {:?}", e))?
        .with_intra_threads(2)
        .map_err(|e| anyhow::anyhow!("Failed to set intra threads: {:?}", e))?
        .with_inter_threads(1)
        .map_err(|e| anyhow::anyhow!("Failed to set inter threads: {:?}", e))?
        .commit_from_file(&model_path)
        .map_err(|e| anyhow::anyhow!("Failed to load model session: {:?}", e))?;

    let load_duration = load_start.elapsed();
    let snap_after = BenchReporter::get_memory_snapshot();
    let rss_delta = snap_after.rss_mb.saturating_sub(snap_before.rss_mb);

    println!(
        "\x1b[32m[Embedding-Bench]\x1b[0m Loaded model in {}ms",
        load_duration.as_millis()
    );
    println!(
        "\x1b[32m[Embedding-Bench]\x1b[0m Memory RSS before: {}MB, after: {}MB, delta: {}MB",
        snap_before.rss_mb, snap_after.rss_mb, rss_delta
    );

    let mut has_token_type_ids = false;
    for input in session.inputs() {
        if input.name() == "token_type_ids" {
            has_token_type_ids = true;
        }
    }
    println!(
        "\x1b[32m[Embedding-Bench]\x1b[0m Model expects token_type_ids: {}",
        has_token_type_ids
    );

    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {:?}", e))?;

    // Hardcoded similarity pairs
    let similar_pairs = vec![
        (
            "I love programming in Rust",
            "Rust is my favorite language",
        ),
        ("aaj kaafi kaam kiya", "bahut saara kaam hua aaj"), // Hinglish
    ];

    let dissimilar_pairs = vec![
        ("I love programming", "The weather is sunny today"),
        ("kaam khatam ho gaya", "biryani bahut tasty thi"), // Hinglish
    ];

    let mut similar_scores = Vec::new();
    let mut dissimilar_scores = Vec::new();

    println!("\x1b[32m[Embedding-Bench]\x1b[0m Running quality validation...");

    for (t1, t2) in &similar_pairs {
        let (emb1, _) = get_embeddings(&mut session, &tokenizer, t1, has_token_type_ids)?;
        let (emb2, _) = get_embeddings(&mut session, &tokenizer, t2, has_token_type_ids)?;
        let sim = cosine_similarity(&emb1, &emb2);
        similar_scores.push(sim);
        println!("  Similar: '{}' <=> '{}' -> Cosine: {:.4}", t1, t2, sim);
    }

    for (t1, t2) in &dissimilar_pairs {
        let (emb1, _) = get_embeddings(&mut session, &tokenizer, t1, has_token_type_ids)?;
        let (emb2, _) = get_embeddings(&mut session, &tokenizer, t2, has_token_type_ids)?;
        let sim = cosine_similarity(&emb1, &emb2);
        dissimilar_scores.push(sim);
        println!("  Dissimilar: '{}' <=> '{}' -> Cosine: {:.4}", t1, t2, sim);
    }

    let similar_min = similar_scores
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let dissimilar_max = dissimilar_scores
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);

    let quality_pass = similar_min > 0.7 && dissimilar_max < 0.55;
    println!(
        "\x1b[32m[Embedding-Bench]\x1b[0m Quality Pass: {} (Min Similar: {:.4}, Max Dissimilar: {:.4})",
        if quality_pass {
            "\x1b[32mPASS\x1b[0m"
        } else {
            "\x1b[31mFAIL\x1b[0m"
        },
        similar_min,
        dissimilar_max
    );

    // Validate Hinglish tokenization
    let hinglish_test = "aaj kaafi kaam kiya humne";
    let encoding = tokenizer
        .encode(hinglish_test, true)
        .map_err(|e| anyhow::anyhow!("Tokenization failed: {:?}", e))?;
    let tokens = encoding.get_tokens();
    let hinglish_token_count = tokens.len();
    println!(
        "\x1b[32m[Embedding-Bench]\x1b[0m Hinglish tokens for '{}': {:?}",
        hinglish_test, tokens
    );
    let hinglish_tokenization_ok = hinglish_token_count < 12;

    // Latency benchmarking (100 iterations on a 128-token input)
    let dummy_paragraph = "This is a dummy paragraph used to measure the inference speed of our local MiniLM multilingual embedding model. We want to simulate a typical user input sequence length to ensure that embedding generation fits comfortably within our real-time budget. A 128-token input is representative of a long session turn or a couple of sentences of context pre-fetched from memory. We will run 100 sequential inferences and record the latencies.";
    
    // Warmup
    let _ = get_embeddings(&mut session, &tokenizer, dummy_paragraph, has_token_type_ids)?;

    let mut latencies = Vec::with_capacity(100);
    for _ in 0..100 {
        let start = Instant::now();
        let _ = get_embeddings(&mut session, &tokenizer, dummy_paragraph, has_token_type_ids)?;
        latencies.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = latencies[50];
    let p95 = latencies[95];
    let p99 = latencies[99];

    println!("\x1b[32m[Embedding-Bench]\x1b[0m Latency: p50={:.2}ms, p95={:.2}ms, p99={:.2}ms", p50, p95, p99);

    // Prepare JSON report
    let report = json!({
        "minilm": {
            "load_time_ms": load_duration.as_millis(),
            "latency_p50_ms": p50,
            "latency_p95_ms": p95,
            "latency_p99_ms": p99,
            "rss_before_mb": snap_before.rss_mb,
            "rss_after_mb": snap_after.rss_mb,
            "rss_delta_mb": rss_delta,
            "similar_cosine_scores": similar_scores,
            "dissimilar_cosine_scores": dissimilar_scores,
            "similar_min": similar_min,
            "dissimilar_max": dissimilar_max,
            "quality_pass": quality_pass,
            "hinglish_token_count": hinglish_token_count,
            "hinglish_tokenization_ok": hinglish_tokenization_ok
        }
    });

    // Save report
    reporter.save_report(report.clone());
    
    let bench_dir = home.join(".vox/benchmarks/memory");
    fs::create_dir_all(&bench_dir)?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let report_path = bench_dir.join(format!("minilm_bench_{}.json", timestamp));
    fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
    println!("\x1b[32m[Embedding-Bench]\x1b[0m Saved report to {:?}", report_path);
    println!("\x1b[32m[Embedding-Bench]\x1b[0m Saved run metrics to {:?}", reporter.run_dir.join("metrics.json"));

    Ok(())
}
