//! ============================================================================
//! query_embeddings.rs — Interactive Embedding Vector Query Utility CLI
//! ============================================================================
//! Category     : Utility Tool (Cargo Example)
//! Component    : Embedding Engine & Turso Vector Database
//! Prerequisites: Local ONNX embedding model at `~/.vox/models/embedding/`
//! Execution    : cargo run --example query_embeddings -- --help
//! ============================================================================

use clap::Parser;
use ndarray::Array2;
use ort::session::Session;
use std::time::Instant;
use tokenizers::Tokenizer;

#[derive(Parser, Debug)]
#[command(
    name = "query-embeddings",
    about = "Query the local Turso vector database using MiniLM embeddings"
)]
struct Args {
    /// The search query
    #[arg(index = 1)]
    query: String,

    /// Number of nearest neighbors to retrieve
    #[arg(short, long, default_value = "5")]
    k: usize,
}

fn f32_to_bytes(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for &val in vec {
        bytes.extend_from_slice(&val.to_ne_bytes());
    }
    bytes
}

fn get_embeddings(
    session: &mut Session,
    tokenizer: &Tokenizer,
    text: &str,
    has_token_type_ids: bool,
) -> anyhow::Result<Vec<f32>> {
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
    for val in sum_embeddings.iter_mut().take(hidden_size) {
        *val /= divisor;
    }

    Ok(sum_embeddings)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let query_text = args.query.trim();

    if query_text.is_empty() {
        anyhow::bail!("Query text cannot be empty.");
    }

    let home = dirs::home_dir().expect("Could not find home directory");
    let model_dir = home.join(".vox/models/memory/minilm");
    let model_path = model_dir.join("model_int8.onnx");
    let tokenizer_path = model_dir.join("tokenizer.json");

    if !model_path.exists() || !tokenizer_path.exists() {
        anyhow::bail!(
            "MiniLM model not found. Run 'embedding-bench' first to download model files."
        );
    }

    // Initialize MiniLM model
    let start_load = Instant::now();
    let mut session = Session::builder()
        .map_err(|e| anyhow::anyhow!("Failed to create builder: {:?}", e))?
        .with_intra_threads(2)
        .map_err(|e| anyhow::anyhow!("Failed to set intra threads: {:?}", e))?
        .with_inter_threads(1)
        .map_err(|e| anyhow::anyhow!("Failed to set inter threads: {:?}", e))?
        .commit_from_file(&model_path)
        .map_err(|e| anyhow::anyhow!("Failed to load model: {:?}", e))?;

    let mut has_token_type_ids = false;
    for input in session.inputs() {
        if input.name() == "token_type_ids" {
            has_token_type_ids = true;
        }
    }

    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {:?}", e))?;
    let load_dur = start_load.elapsed();

    // Embed the query
    let start_embed = Instant::now();
    let embedding = get_embeddings(&mut session, &tokenizer, query_text, has_token_type_ids)?;
    let query_bytes = f32_to_bytes(&embedding);
    let embed_dur = start_embed.elapsed();

    // Open database using library's persistence wrapper
    let db_path = home.join(".vox/vox.db");
    if !db_path.exists() {
        anyhow::bail!(
            "Database file does not exist at {:?}. Run 'seed-embeddings' first.",
            db_path
        );
    }

    let start_query = Instant::now();
    let conn = vox_lib::persistence::db::VoxDb::open_readonly(&db_path).await?;

    let mut rows = conn
        .query(
            "SELECT content, vector_distance_cos(embedding, ?) as distance
             FROM memory_entries
             ORDER BY distance ASC
             LIMIT ?",
            (query_bytes, args.k as i64),
        )
        .await?;

    println!(
        "\n\x1b[34m[Query-Embeddings]\x1b[0m Query: \"{}\"",
        query_text
    );
    println!("  Model Load time:   {}ms", load_dur.as_millis());
    println!("  Embedding time:    {}ms", embed_dur.as_millis());

    let mut count = 0;
    while let Some(row) = rows.next().await? {
        let content: String = row.get(0)?;
        let distance: f64 = row.get(1)?;
        // Cosine distance = 1 - Cosine similarity
        let similarity = 1.0 - distance;
        count += 1;
        println!(
            "  [{}] Similarity: \x1b[32m{:.4}\x1b[0m (Distance: {:.4}) | Content: \"{}\"",
            count, similarity, distance, content
        );
    }

    println!(
        "  Database Query:    {}ms",
        start_query.elapsed().as_millis()
    );
    println!("  Total retrieved:   {} results\n", count);

    Ok(())
}
