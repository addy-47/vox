use crate::core::constants::is_valid_inter_collection_pair;
use crate::services::memory::{
    EDGE_CLASSIFIER_MODEL_DIR, EDGE_CLASSIFIER_MODEL_FILENAME, EDGE_CLASSIFIER_THRESHOLD,
    EDGE_CLASSIFIER_TOKENIZER_FILENAME,
};
use anyhow::{anyhow, Result};
use ndarray::Array2;
use std::path::Path;
use tokenizers::Tokenizer;

/// Runtime state container holding the ONNX inference session and tokenizer for edge classification.
pub struct EdgeClassifierEngine {
    session: parking_lot::Mutex<ort::session::Session>,
    tokenizer: Tokenizer,
    has_token_type_ids: bool,
}

static EDGE_ENGINE: parking_lot::RwLock<Option<EdgeClassifierEngine>> =
    parking_lot::RwLock::new(None);

/// Initializes the ModernBERT INT8 ONNX Edge Classifier Engine from model directory.
pub fn init_edge_classifier(model_dir: &Path) -> Result<bool> {
    let model_path = model_dir.join(EDGE_CLASSIFIER_MODEL_FILENAME);
    let tokenizer_path = model_dir.join(EDGE_CLASSIFIER_TOKENIZER_FILENAME);

    if !model_path.exists() || !tokenizer_path.exists() {
        log::warn!(
            "[EdgeClassifier] Model or tokenizer file missing in {:?}. Expected model: {}, tokenizer: {}. Skipping init.",
            model_dir,
            EDGE_CLASSIFIER_MODEL_FILENAME,
            EDGE_CLASSIFIER_TOKENIZER_FILENAME
        );
        return Ok(false);
    }

    let mut lock = EDGE_ENGINE.write();
    if lock.is_some() {
        return Ok(true);
    }

    let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| anyhow!("Failed to load Edge Classifier tokenizer: {}", e))?;

    if tokenizer.get_truncation().is_none() {
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: 512,
                ..Default::default()
            }))
            .map_err(|e| {
                anyhow!(
                    "Failed to configure Edge Classifier tokenizer truncation: {}",
                    e
                )
            })?;
    }

    let session = ort::session::Session::builder()
        .map_err(|e| anyhow!("Failed to create Edge Classifier session builder: {:?}", e))?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow!("Failed to set Edge Classifier optimization level: {:?}", e))?
        .with_intra_threads(1)
        .map_err(|e| anyhow!("Failed to set Edge Classifier intra threads: {:?}", e))?
        .commit_from_file(&model_path)
        .map_err(|e| anyhow!("Failed to commit Edge Classifier session: {:?}", e))?;

    let has_token_type_ids = session
        .inputs()
        .iter()
        .any(|i| i.name() == "token_type_ids");

    let engine = EdgeClassifierEngine {
        session: parking_lot::Mutex::new(session),
        tokenizer,
        has_token_type_ids,
    };

    *lock = Some(engine);
    log::info!(
        "[EdgeClassifier] ModernBERT INT8 ONNX Edge Classifier Engine initialized successfully."
    );
    Ok(true)
}

/// Evicts the ModernBERT Edge Classifier Engine from process memory.
pub fn unload_edge_classifier() {
    let mut lock = EDGE_ENGINE.write();
    if lock.is_some() {
        *lock = None;
        log::info!("[EdgeClassifier] Edge classifier ONNX model evicted from memory.");
    }
}

/// Returns true if the edge classifier ONNX session is currently loaded in memory.
pub fn is_edge_classifier_loaded() -> bool {
    EDGE_ENGINE.read().is_some()
}

/// Ensures the edge classifier model is loaded, loading default weights from disk if absent.
pub fn ensure_edge_classifier_loaded() -> Result<()> {
    if is_edge_classifier_loaded() {
        return Ok(());
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/addy".to_string());
    let default_dir = std::path::PathBuf::from(home)
        .join(".vox")
        .join("models")
        .join(EDGE_CLASSIFIER_MODEL_DIR);

    init_edge_classifier(&default_dir)?;
    Ok(())
}

/// Tokenizes source and target collection facts for inter-edge classification.
fn tokenize_input(
    engine: &EdgeClassifierEngine,
    src_collection: &str,
    src_fact: &str,
    tgt_collection: &str,
    tgt_fact: &str,
) -> Result<(Array2<i64>, Array2<i64>, usize)> {
    let input_text = format!(
        "[{}] {} [SEP] [{}] {}",
        src_collection, src_fact, tgt_collection, tgt_fact
    );

    let encoding = engine
        .tokenizer
        .encode(input_text.as_str(), true)
        .map_err(|e| anyhow!("Tokenization failed for Edge Classifier: {}", e))?;

    let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
    let attention_mask: Vec<i64> = encoding
        .get_attention_mask()
        .iter()
        .map(|&m| m as i64)
        .collect();
    let seq_len = input_ids.len();

    let input_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), input_ids)?;
    let attention_mask_arr = Array2::<i64>::from_shape_vec((1, seq_len), attention_mask)?;

    Ok((input_ids_arr, attention_mask_arr, seq_len))
}

/// Executes the ONNX graph session to compute raw logits.
fn run_inference(
    engine: &EdgeClassifierEngine,
    input_ids_arr: Array2<i64>,
    attention_mask_arr: Array2<i64>,
    seq_len: usize,
) -> Result<Vec<f32>> {
    let input_ids_tensor = ort::value::Tensor::from_array(input_ids_arr)?;
    let attention_mask_tensor = ort::value::Tensor::from_array(attention_mask_arr)?;

    let mut session_guard = engine.session.lock();

    let outputs = if engine.has_token_type_ids {
        let token_type_ids_arr = Array2::<i64>::zeros((1, seq_len));
        let token_type_ids_tensor = ort::value::Tensor::from_array(token_type_ids_arr)?;
        session_guard.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
            "token_type_ids" => token_type_ids_tensor
        ])?
    } else {
        session_guard.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor
        ])?
    };

    let output_key = outputs
        .keys()
        .next()
        .ok_or_else(|| anyhow!("ONNX model output missing logits tensor key"))?;

    let logits_array = outputs[output_key].try_extract_array::<f32>()?;
    Ok(logits_array.iter().copied().collect())
}

/// Computes softmax probabilities over logits and identifies the highest-probability class index.
fn compute_softmax(logits_slice: &[f32]) -> (usize, f32) {
    if logits_slice.is_empty() {
        return (0, 0.0);
    }

    let max_logit = logits_slice
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = logits_slice
        .iter()
        .map(|&l| (l - max_logit).exp())
        .collect();
    let sum_exp: f32 = exps.iter().sum();
    let probs: Vec<f32> = if sum_exp > 0.0 {
        exps.iter().map(|e| e / sum_exp).collect()
    } else {
        vec![0.25; logits_slice.len()]
    };

    let mut max_idx = 0;
    let mut max_prob = f32::NEG_INFINITY;
    for (i, &p) in probs.iter().enumerate() {
        if p > max_prob {
            max_prob = p;
            max_idx = i;
        }
    }

    (max_idx, max_prob)
}

/// Classifies an inter-collection candidate pair using ModernBERT INT8 ONNX sequence classification.
pub fn classify_edge(
    src_collection: &str,
    src_fact: &str,
    tgt_collection: &str,
    tgt_fact: &str,
) -> Result<(Option<String>, f32)> {
    if !is_valid_inter_collection_pair(src_collection, tgt_collection) {
        return Ok((None, 0.0));
    }

    if !is_edge_classifier_loaded() {
        ensure_edge_classifier_loaded()?;
    }

    let lock = EDGE_ENGINE.read();
    let engine = match lock.as_ref() {
        Some(e) => e,
        None => return Ok((None, 0.0)),
    };

    let (input_ids, attention_mask, seq_len) =
        tokenize_input(engine, src_collection, src_fact, tgt_collection, tgt_fact)?;
    let logits = run_inference(engine, input_ids, attention_mask, seq_len)?;
    let (max_idx, max_prob) = compute_softmax(&logits);

    let predicted_label = match max_idx {
        0 => crate::core::constants::PM_RELATION_SHAPES,
        1 => crate::core::constants::PM_RELATION_DEPENDS_ON,
        2 => crate::core::constants::PM_RELATION_CONFLICTS,
        _ => "",
    };

    if !predicted_label.is_empty() && max_prob >= EDGE_CLASSIFIER_THRESHOLD {
        Ok((Some(predicted_label.to_string()), max_prob))
    } else {
        Ok((None, max_prob))
    }
}
