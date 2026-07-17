use anyhow::{anyhow, Result};
use ndarray::Array2;
use std::path::Path;
use std::sync::OnceLock;
use tokenizers::Tokenizer;
use crate::core::settings::MemorySettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NliLabel {
    Contradiction,
    Entailment,
    Neutral,
}

#[derive(Debug, Clone)]
pub struct NliResult {
    pub contradiction: f32,
    pub entailment: f32,
    pub neutral: f32,
}

pub enum NliRelation {
    Conflicts,
    Supports,
    Neutral,
}

pub struct NliEngine {
    session: std::sync::Mutex<ort::session::Session>,
    tokenizer: Tokenizer,
    has_token_type_ids: bool,
    class_mapping: [NliLabel; 3],
}

static NLI_ENGINE: OnceLock<NliEngine> = OnceLock::new();

/// Loads the NLI model from disk and runs calibration to determine output label indices.
pub fn init_nli_engine(model_dir: &Path) -> Result<()> {
    let model_path = if model_dir.join("model_quantized.onnx").exists() {
        model_dir.join("model_quantized.onnx")
    } else if model_dir.join("model_int8.onnx").exists() {
        model_dir.join("model_int8.onnx")
    } else {
        model_dir.join("model.onnx")
    };

    let tokenizer_path = model_dir.join("tokenizer.json");

    if !model_path.exists() || !tokenizer_path.exists() {
        return Err(anyhow!(
            "[NliEngine] Model or tokenizer file missing in {:?}",
            model_dir
        ));
    }

    let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| anyhow!("Failed to load NLI tokenizer: {}", e))?;

    // Clamp tokenizer maximum window size to prevent tensor overflows
    if tokenizer.get_truncation().is_none() {
        tokenizer
            .with_truncation(Some(tokenizers::TruncationParams {
                max_length: 512,
                ..Default::default()
            }))
            .map_err(|e| anyhow!("Failed to configure tokenizer truncation: {}", e))?;
    }

    let session = ort::session::Session::builder()
        .map_err(|e| anyhow!("Failed to create NLI session builder: {:?}", e))?
        .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow!("Failed to set NLI optimization level: {:?}", e))?
        .with_intra_threads(1)
        .map_err(|e| anyhow!("Failed to set NLI intra threads: {:?}", e))?
        .commit_from_file(&model_path)
        .map_err(|e| anyhow!("Failed to commit NLI session: {:?}", e))?;

    let has_token_type_ids = session.inputs().iter().any(|i| i.name() == "token_type_ids");
    let class_mapping = [NliLabel::Contradiction, NliLabel::Entailment, NliLabel::Neutral];

    let mut engine = NliEngine {
        session: std::sync::Mutex::new(session),
        tokenizer,
        has_token_type_ids,
        class_mapping,
    };

    // Run calibration
    engine.calibrate()?;

    if NLI_ENGINE.set(engine).is_err() {
        log::warn!("[NliEngine] NLI Engine singleton already set.");
    } else {
        log::info!(
            "[NliEngine] Successfully loaded and calibrated NLI model from {:?}",
            model_dir
        );
    }

    Ok(())
}

impl NliEngine {
    fn calibrate(&mut self) -> Result<()> {
        let p_ent = "A person is playing tennis.";
        let h_ent = "A person is playing tennis.";
        let logits_ent = self.raw_predict(p_ent, h_ent)?;

        let p_con = "A person is playing tennis.";
        let h_con = "A person is sleeping.";
        let logits_con = self.raw_predict(p_con, h_con)?;

        let ent_idx = logits_ent
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap_or((1, &0.0))
            .0;

        let con_idx = logits_con
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap_or((0, &0.0))
            .0;

        if ent_idx == con_idx {
            log::warn!("[NliEngine] Calibration collision. Falling back to default label order.");
            self.class_mapping = [NliLabel::Contradiction, NliLabel::Entailment, NliLabel::Neutral];
        } else {
            let mut indices = vec![0, 1, 2];
            indices.retain(|&x| x != ent_idx && x != con_idx);
            let neu_idx = indices[0];

            self.class_mapping[ent_idx] = NliLabel::Entailment;
            self.class_mapping[con_idx] = NliLabel::Contradiction;
            self.class_mapping[neu_idx] = NliLabel::Neutral;
        }

        log::debug!(
            "[NliEngine] Calibrated Class Mapping: [0: {:?}, 1: {:?}, 2: {:?}]",
            self.class_mapping[0],
            self.class_mapping[1],
            self.class_mapping[2]
        );

        Ok(())
    }

    fn raw_predict(&self, premise: &str, hypothesis: &str) -> Result<Vec<f32>> {
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

        let mut session_guard = self.session.lock().map_err(|e| anyhow!("Session lock poisoned: {}", e))?;

        let outputs = if self.has_token_type_ids {
            let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&x| x as i64).collect();
            let type_ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), type_ids)?;
            let type_ids_tensor = ort::value::Tensor::from_array(type_ids_arr)
                .map_err(|e| anyhow!("{:?}", e))?;
            session_guard.run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor,
                "token_type_ids" => type_ids_tensor
            ]).map_err(|e| anyhow!("{:?}", e))?
        } else {
            session_guard.run(ort::inputs![
                "input_ids" => input_ids_tensor,
                "attention_mask" => attention_mask_tensor
            ]).map_err(|e| anyhow!("{:?}", e))?
        };

        let output_key = outputs.keys().next().ok_or_else(|| anyhow!("No output in model"))?;
        let logits_array = outputs[output_key].try_extract_array::<f32>().map_err(|e| anyhow!("{:?}", e))?;

        let shape = logits_array.shape();
        if shape.len() < 2 || shape[0] != 1 || shape[1] < 3 {
            return Err(anyhow!(
                "Invalid output logits tensor dimensions. Expected [1, >=3], received: {:?}",
                shape
            ));
        }

        Ok(vec![
            logits_array[[0, 0]],
            logits_array[[0, 1]],
            logits_array[[0, 2]],
        ])
    }
}

/// Lazily loads the NLI engine if not already loaded.
pub fn ensure_nli_loaded(model_name: &str) -> Result<()> {
    if NLI_ENGINE.get().is_some() {
        return Ok(());
    }

    let models_dir = if let Some(p) = crate::utils::paths::try_get() {
        p.models.clone()
    } else {
        dirs::home_dir().unwrap_or_default().join(".vox").join("models")
    };

    let nli_model_dir = models_dir.join("nli").join(model_name);
    init_nli_engine(&nli_model_dir)
}

/// Performs NLI classification between premise and hypothesis.
/// Returns the softmax probabilities mapped to the output struct.
pub fn classify_pair(premise: &str, hypothesis: &str) -> Result<NliResult> {
    let engine = NLI_ENGINE.get().ok_or_else(|| anyhow!("NLI Engine is not loaded."))?;
    let logits = engine.raw_predict(premise, hypothesis)?;

    // Softmax
    let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
    let sum_exp: f32 = exps.iter().sum();
    let probs: Vec<f32> = if sum_exp > 0.0 {
        exps.iter().map(|e| e / sum_exp).collect()
    } else {
        vec![0.333, 0.333, 0.333]
    };

    let mut contradiction = 0.0;
    let mut entailment = 0.0;
    let mut neutral = 0.0;

    for (i, &prob) in probs.iter().enumerate() {
        match engine.class_mapping[i] {
            NliLabel::Contradiction => contradiction = prob,
            NliLabel::Entailment => entailment = prob,
            NliLabel::Neutral => neutral = prob,
        }
    }

    Ok(NliResult {
        contradiction,
        entailment,
        neutral,
    })
}

/// Determines the logical relationship classification based on prediction scores and settings.
pub fn relation_from_result(result: &NliResult, settings: &MemorySettings) -> NliRelation {
    if result.contradiction >= settings.nli_contradiction_threshold {
        NliRelation::Conflicts
    } else if result.entailment >= settings.nli_entailment_threshold {
        NliRelation::Supports
    } else {
        NliRelation::Neutral
    }
}
