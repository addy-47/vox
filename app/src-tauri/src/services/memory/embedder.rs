use anyhow::Result;
use ndarray::Array2;
use parking_lot::Mutex;
use std::path::Path;
use tokenizers::Tokenizer;

pub const EMBEDDING_DIM: usize = 384;
pub const PRIMARY_MODEL_DIR: &str = "minilm-l12-v2";
pub const PRIMARY_MODEL_FILENAME: &str = "model_int8.onnx";
pub const FALLBACK_MODEL_DIR: &str = "bge-m3";
pub const FALLBACK_MODEL_FILENAME: &str = "model_quantized.onnx";
pub const TOKENIZER_FILENAME: &str = "tokenizer.json";

/// ONNX session container for running dense sentence text embeddings.
pub struct TextEmbedder {
    session: Mutex<ort::session::Session>,
    tokenizer: Tokenizer,
    has_token_type_ids: bool,
}

static EMBEDDER: parking_lot::RwLock<Option<TextEmbedder>> = parking_lot::RwLock::new(None);

/// Initializes the text embedding model singleton.
pub fn init_embedder(model_dir: &Path, is_primary: bool) -> Result<bool> {
    let model_filename = if is_primary {
        PRIMARY_MODEL_FILENAME
    } else {
        FALLBACK_MODEL_FILENAME
    };
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

    let mut lock = EMBEDDER.write();
    if lock.is_some() {
        return Ok(true);
    }

    let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
        anyhow::anyhow!("Failed to load tokenizer from {:?}: {}", tokenizer_path, e)
    })?;

    let session = ort::session::Session::builder()
        .map_err(|e| anyhow::anyhow!("Failed to create session builder: {:?}", e))?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow::anyhow!("Failed to set optimization level: {:?}", e))?
        .with_intra_threads(1)
        .map_err(|e| anyhow::anyhow!("Failed to set intra threads: {:?}", e))?
        .commit_from_file(&model_path)
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to commit session from file {:?}: {:?}",
                model_path,
                e
            )
        })?;

    let has_token_type_ids = session
        .inputs()
        .iter()
        .any(|i| i.name() == "token_type_ids");

    let embedder = TextEmbedder {
        session: Mutex::new(session),
        tokenizer,
        has_token_type_ids,
    };

    *lock = Some(embedder);
    log::info!(
        "[Embedder] Successfully loaded text embedding model from {:?}",
        model_dir
    );

    Ok(true)
}

/// Evicts the text embedding model singleton from process memory.
pub fn unload_embedder() {
    let mut lock = EMBEDDER.write();
    if lock.is_some() {
        *lock = None;
        log::info!("[Embedder] Text embedder ONNX model evicted from memory.");
    }
}

/// Lazily loads the text embedding model into RAM only when required.
pub fn ensure_embedder_loaded(memory_enabled: bool) -> Result<bool> {
    if !memory_enabled {
        log::debug!("[Embedder] Memory subsystem disabled. Skipping model load.");
        return Ok(false);
    }
    if EMBEDDER.read().is_some() {
        return Ok(true);
    }
    let models_dir = if let Some(p) = crate::utils::paths::try_get() {
        p.models.clone()
    } else {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".vox")
            .join("models")
    };

    let minilm_dir = models_dir.join("embedding").join(PRIMARY_MODEL_DIR);
    if minilm_dir.join(PRIMARY_MODEL_FILENAME).exists() {
        init_embedder(&minilm_dir, true)
    } else {
        let bge_dir = models_dir.join("embedding").join(FALLBACK_MODEL_DIR);
        init_embedder(&bge_dir, false)
    }
}

struct TextEncodingTensors {
    input_ids: Array2<i64>,
    attention_mask: Array2<i64>,
    type_ids: Option<Array2<i64>>,
    encoding: tokenizers::Encoding,
    seq_len: usize,
}

/// Encodes input string into 2D tensor arrays and extracts encoding tokens.
fn encode_text(embedder: &TextEmbedder, text: &str) -> Result<TextEncodingTensors> {
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

    let input_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), ids)?;
    let attention_mask_arr = Array2::<i64>::from_shape_vec((1, seq_len), mask)?;

    let type_ids_arr = if embedder.has_token_type_ids {
        let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();
        Some(Array2::<i64>::from_shape_vec((1, seq_len), type_ids)?)
    } else {
        None
    };

    Ok(TextEncodingTensors {
        input_ids: input_ids_arr,
        attention_mask: attention_mask_arr,
        type_ids: type_ids_arr,
        encoding,
        seq_len,
    })
}

/// Pools token hidden states via attention mask weighting and applies L2 normalization.
fn mean_pool_and_normalize(
    last_hidden_state: &ndarray::ArrayViewD<f32>,
    encoding_mask: &[u32],
) -> Vec<f32> {
    let shape = last_hidden_state.shape();
    let out_seq_len = shape[1];
    let hidden_size = shape[2];

    let mut sum_embeddings = vec![0.0f32; hidden_size];
    let mut sum_mask = 0.0f32;

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

    l2_normalize_in_place(&mut sum_embeddings);
    sum_embeddings
}

/// Generates a dense vector embedding for the input text.
pub fn generate_embedding(text: &str) -> Result<Option<Vec<f32>>> {
    let lock = EMBEDDER.read();
    let embedder = match lock.as_ref() {
        Some(e) => e,
        None => return Ok(None),
    };

    let tensors = encode_text(embedder, text)?;

    if tensors.seq_len == 0 {
        return Ok(Some(vec![0.0f32; EMBEDDING_DIM]));
    }

    let input_ids_tensor = ort::value::Tensor::from_array(tensors.input_ids)
        .map_err(|e| anyhow::anyhow!("Failed to create input_ids tensor: {:?}", e))?;
    let attention_mask_tensor = ort::value::Tensor::from_array(tensors.attention_mask)
        .map_err(|e| anyhow::anyhow!("Failed to create attention_mask tensor: {:?}", e))?;

    let mut session_guard = embedder.session.lock();

    let outputs = if let Some(type_ids_arr) = tensors.type_ids {
        let type_ids_tensor = ort::value::Tensor::from_array(type_ids_arr)
            .map_err(|e| anyhow::anyhow!("Failed to create type_ids tensor: {:?}", e))?;

        session_guard
            .run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "token_type_ids" => type_ids_tensor
            ])
            .map_err(|e| anyhow::anyhow!("ONNX inference error: {:?}", e))?
    } else {
        session_guard
            .run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor
            ])
            .map_err(|e| anyhow::anyhow!("ONNX inference error: {:?}", e))?
    };

    let output_key = outputs
        .keys()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No output in model"))?;
    let last_hidden_state = outputs[output_key]
        .try_extract_array::<f32>()
        .map_err(|e| anyhow::anyhow!("Failed to extract output array: {:?}", e))?;

    let pooled = mean_pool_and_normalize(&last_hidden_state, tensors.encoding.get_attention_mask());
    Ok(Some(pooled))
}

/// Returns true if the text embedder model is loaded and ready.
pub fn is_embedder_loaded() -> bool {
    EMBEDDER.read().is_some()
}

/// L2 normalizes a slice of floats in-place.
pub fn l2_normalize_in_place(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 && norm.is_finite() {
        for val in v.iter_mut() {
            *val /= norm;
        }
    }
}

/// L2 normalizes a vector, returning a new normalized vector.
pub fn l2_normalize(v: &[f32]) -> Vec<f32> {
    let mut out = v.to_vec();
    l2_normalize_in_place(&mut out);
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_normalization_zero_vector() {
        let mut zero_vec = vec![0.0f32; 384];
        l2_normalize_in_place(&mut zero_vec);
        for &val in zero_vec.iter() {
            assert_eq!(
                val, 0.0f32,
                "Zero vector normalization must remain all zeroes"
            );
        }

        let non_finite_vec = vec![f32::NAN, f32::INFINITY, 0.0];
        let normalized = l2_normalize(&non_finite_vec);
        assert_eq!(normalized.len(), 3);
    }

    #[test]
    fn test_l2_normalization_standard() {
        let mut vec = vec![3.0f32, 4.0f32];
        l2_normalize_in_place(&mut vec);

        assert!((vec[0] - 0.6).abs() < 1e-6);
        assert!((vec[1] - 0.8).abs() < 1e-6);

        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_edge_cases() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let orthogonal = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &orthogonal) - 0.0).abs() < 1e-6);

        let opposite = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &opposite) - (-1.0)).abs() < 1e-6);

        let empty: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&empty, &empty), 0.0);

        let mismatch = vec![1.0, 0.0];
        assert_eq!(cosine_similarity(&a, &mismatch), 0.0);
    }
}
