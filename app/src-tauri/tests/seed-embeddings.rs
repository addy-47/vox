use ndarray::Array2;
use ort::session::Session;
use serde::Deserialize;
use std::fs;
use std::time::Instant;
use tokenizers::Tokenizer;

#[derive(Deserialize)]
struct SquadDataset {
    data: Vec<SquadData>,
}

#[derive(Deserialize)]
struct SquadData {
    paragraphs: Vec<SquadParagraph>,
}

#[derive(Deserialize)]
struct SquadParagraph {
    context: String,
    qas: Vec<SquadQA>,
}

#[derive(Deserialize)]
struct SquadQA {
    question: String,
    id: String,
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
    for dim in 0..hidden_size {
        sum_embeddings[dim] /= divisor;
    }

    Ok(sum_embeddings)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("\x1b[32m[Seed-Embeddings]\x1b[0m Downloading SQuAD v2.0 dataset...");
    let url = "https://raw.githubusercontent.com/rajpurkar/SQuAD-explorer/master/dataset/train-v2.0.json";
    let client = reqwest::Client::new();
    let start_dl = Instant::now();
    let res = client.get(url).send().await?.error_for_status()?;
    let bytes = res.bytes().await?;
    println!(
        "\x1b[32m[Seed-Embeddings]\x1b[0m Downloaded {:.2} MB in {}ms",
        bytes.len() as f64 / 1024.0 / 1024.0,
        start_dl.elapsed().as_millis()
    );

    println!("\x1b[32m[Seed-Embeddings]\x1b[0m Parsing JSON...");
    let squad: SquadDataset = serde_json::from_slice(&bytes)?;
    println!("\x1b[32m[Seed-Embeddings]\x1b[0m Dataset parsed successfully.");

    // Extract questions up to 1,500
    let mut qa_pairs = Vec::new();
    for data_item in &squad.data {
        for paragraph in &data_item.paragraphs {
            for qa in &paragraph.qas {
                if !qa.question.trim().is_empty() {
                    qa_pairs.push((qa.id.clone(), qa.question.clone()));
                    if qa_pairs.len() >= 1500 {
                        break;
                    }
                }
            }
            if qa_pairs.len() >= 1500 {
                break;
            }
        }
        if qa_pairs.len() >= 1500 {
            break;
        }
    }
    println!(
        "\x1b[32m[Seed-Embeddings]\x1b[0m Collected {} questions.",
        qa_pairs.len()
    );

    // Initialize MiniLM model and tokenizer
    let home = dirs::home_dir().expect("Could not find home directory");
    let model_dir = home.join(".vox/models/memory/minilm");
    let model_path = model_dir.join("model_int8.onnx");
    let tokenizer_path = model_dir.join("tokenizer.json");

    if !model_path.exists() || !tokenizer_path.exists() {
        anyhow::bail!(
            "MiniLM model not found. Run 'embedding-bench' first to download model files."
        );
    }

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

    // Open database using library's persistence wrapper
    let db_path = home.join(".vox/vox.db");
    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Delete existing DB to prevent conflicts
    if db_path.exists() {
        println!(
            "\x1b[33m[Seed-Embeddings]\x1b[0m Wiping existing DB at {:?}",
            db_path
        );
        fs::remove_file(&db_path)?;
    }

    let conn = vox_lib::persistence::db::VoxDb::open(&db_path).await?;
    vox_lib::persistence::schema::run_migrations(&conn).await?;

    println!("\x1b[32m[Seed-Embeddings]\x1b[0m Embedding and inserting questions into DB...");
    let start_embed = Instant::now();
    let mut count = 0;

    for (id, question) in &qa_pairs {
        let embedding = get_embeddings(&mut session, &tokenizer, question, has_token_type_ids)?;
        let blob_bytes = f32_to_bytes(&embedding);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO memory_entries (id, content, embedding, created_at) VALUES (?, ?, ?, ?)",
            (id.clone(), question.clone(), blob_bytes, now),
        )
        .await?;

        count += 1;
        if count % 100 == 0 {
            println!(
                "\x1b[32m[Seed-Embeddings]\x1b[0m Embedded and inserted {}/{} questions...",
                count,
                qa_pairs.len()
            );
        }
    }

    println!(
        "\x1b[32m[Seed-Embeddings]\x1b[0m Successfully seeded {} question embeddings in {}ms",
        count,
        start_embed.elapsed().as_millis()
    );

    Ok(())
}
