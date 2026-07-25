use anyhow::Result;
use ndarray::Array2;
use parking_lot::Mutex;
use std::path::Path;
use std::sync::OnceLock;
use tokenizers::Tokenizer;

pub const EMBEDDING_DIM: usize = 384;
pub const PRIMARY_MODEL_DIR: &str = "minilm-l12-v2";
pub const PRIMARY_MODEL_FILENAME: &str = "model_int8.onnx";
pub const FALLBACK_MODEL_DIR: &str = "bge-m3";
pub const FALLBACK_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const TOKENIZER_FILENAME: &str = "tokenizer.json";

pub struct TextEmbedder {
    session: Mutex<ort::session::Session>,
    tokenizer: Tokenizer,
    has_token_type_ids: bool,
}

static EMBEDDER: OnceLock<TextEmbedder> = OnceLock::new();

/// Initializes the text embedding model singleton (`services/memory/embedder.rs`).
///
/// Primary Model Path: `~/.vox/models/embedding/minilm-l12-v2/model_int8.onnx` (384-dim)
/// Fallback Model Path: `~/.vox/models/embedding/bge-m3/model_quantized.onnx` (1024-dim)
/// Returns `Ok(true)` if model loaded successfully, `Ok(false)` if model assets missing.
pub fn init_embedder(model_dir: &Path, is_primary: bool) -> Result<bool> {
    let model_filename = if is_primary { PRIMARY_MODEL_FILENAME } else { FALLBACK_MODEL_FILENAME };
    let model_path = model_dir.join(model_filename);
    let tokenizer_path = model_dir.join(TOKENIZER_FILENAME);

    if !model_path.exists() || !tokenizer_path.exists() {
        log::warn!(
            "[Embedder] Model assets missing at {:?}. Required model: {}, tokenizer: {}. Skipping init.",
            model_dir,
            model_filename,
            TOKENIZER_FILENAME
        );
        return Ok(false);
    }

    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| anyhow::anyhow!("Failed to load tokenizer from {:?}: {}", tokenizer_path, e))?;

    let session = ort::session::Session::builder()
        .map_err(|e| anyhow::anyhow!("Failed to create session builder: {:?}", e))?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow::anyhow!("Failed to set optimization level: {:?}", e))?
        .with_intra_threads(1)
        .map_err(|e| anyhow::anyhow!("Failed to set intra threads: {:?}", e))?
        .commit_from_file(&model_path)
        .map_err(|e| anyhow::anyhow!("Failed to commit session from file {:?}: {:?}", model_path, e))?;

    let has_token_type_ids = session.inputs().iter().any(|i| i.name() == "token_type_ids");

    let embedder = TextEmbedder {
        session: Mutex::new(session),
        tokenizer,
        has_token_type_ids,
    };

    if EMBEDDER.set(embedder).is_err() {
        log::warn!("[Embedder] Embedder singleton already set.");
    } else {
        log::info!(
            "[Embedder] Successfully loaded text embedding model from {:?}",
            model_dir
        );
    }

    Ok(true)
}

/// Lazily loads the text embedding model into RAM only when required.
///
/// Returns `Ok(true)` if embedder is ready, `Ok(false)` if assets are missing.
pub fn ensure_embedder_loaded(memory_enabled: bool) -> Result<bool> {
    if !memory_enabled {
        log::debug!("[Embedder] Memory subsystem disabled. Skipping model load.");
        return Ok(false);
    }
    if EMBEDDER.get().is_some() {
        return Ok(true);
    }
    let models_dir = if let Some(p) = crate::utils::paths::try_get() {
        p.models.clone()
    } else {
        dirs::home_dir().unwrap_or_default().join(".vox").join("models")
    };

    let minilm_dir = models_dir.join("embedding").join(PRIMARY_MODEL_DIR);
    if minilm_dir.join(PRIMARY_MODEL_FILENAME).exists() {
        init_embedder(&minilm_dir, true)
    } else {
        let bge_dir = models_dir.join("embedding").join(FALLBACK_MODEL_DIR);
        init_embedder(&bge_dir, false)
    }
}

/// Generates a dense vector embedding for the input text.
/// Returns `Ok(None)` if the embedder model is not loaded.
pub fn generate_embedding(text: &str) -> Result<Option<Vec<f32>>> {
    let embedder = match EMBEDDER.get() {
        Some(e) => e,
        None => return Ok(None),
    };

    let encoding = embedder
        .tokenizer
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("Tokenization failed: {:?}", e))?;

    let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let mask: Vec<i64> = encoding
        .get_attention_mask()
        .iter()
        .map(|&x| x as i64)
        .collect();
    let seq_len = ids.len();

    if seq_len == 0 {
        return Ok(Some(vec![0.0f32; EMBEDDING_DIM]));
    }

    let input_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), ids)?;
    let attention_mask_arr = Array2::<i64>::from_shape_vec((1, seq_len), mask)?;

    let input_ids_tensor = ort::value::Tensor::from_array(input_ids_arr)
        .map_err(|e| anyhow::anyhow!("Failed to create input_ids tensor: {:?}", e))?;
    let attention_mask_tensor = ort::value::Tensor::from_array(attention_mask_arr)
        .map_err(|e| anyhow::anyhow!("Failed to create attention_mask tensor: {:?}", e))?;

    let mut session_guard = embedder.session.lock();

    let outputs = if embedder.has_token_type_ids {
        let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();
        let type_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), type_ids)?;
        let type_ids_tensor = ort::value::Tensor::from_array(type_ids_arr)
            .map_err(|e| anyhow::anyhow!("Failed to create type_ids tensor: {:?}", e))?;

        session_guard.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
            "token_type_ids" => type_ids_tensor
        ]).map_err(|e| anyhow::anyhow!("ONNX inference error: {:?}", e))?
    } else {
        session_guard.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor
        ]).map_err(|e| anyhow::anyhow!("ONNX inference error: {:?}", e))?
    };

    let output_key = outputs
        .keys()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No output in model"))?;
    let last_hidden_state = outputs[output_key]
        .try_extract_array::<f32>()
        .map_err(|e| anyhow::anyhow!("Failed to extract output array: {:?}", e))?;

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

    Ok(Some(sum_embeddings))
}

/// Returns true if the text embedder model is loaded and ready.
pub fn is_embedder_loaded() -> bool {
    EMBEDDER.get().is_some()
}

/// Calculates the cosine similarity between two float vectors.
pub fn cosine_similarity(u: &[f32], v: &[f32]) -> f32 {
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
