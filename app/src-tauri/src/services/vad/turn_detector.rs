use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use ndarray::Array2;
use ort::session::Session;
use parking_lot::Mutex;

/// End-of-Utterance (EOU) classifier predicting conversational turn completion from trailing acoustic prosody.
pub struct SmartTurnDetector {
    session: Option<Arc<Mutex<Session>>>,
    model_path: PathBuf,
}

impl SmartTurnDetector {
    /// Initializes the turn detector from the model path if present on disk.
    /// Gracefully degrades to a dormant state when the model is absent (Tier 1A fallback).
    pub fn new(model_path: &Path) -> Result<Self> {
        let model_path_buf = model_path.to_path_buf();
        if !model_path_buf.exists() {
            log::info!(
                "[TurnDetector] Smart-turn model not found at {:?}; graceful fallback to timing active",
                model_path_buf
            );
            return Ok(Self {
                session: None,
                model_path: model_path_buf,
            });
        }

        let session = Session::builder()
            .map_err(|e| anyhow!("Failed to create session builder: {}", e))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("Failed to set intra threads: {}", e))?
            .commit_from_file(&model_path_buf)
            .map_err(|e| anyhow!("Failed to load smart-turn model from file: {}", e))?;

        log::info!(
            "[TurnDetector] Smart-turn model loaded successfully from {:?}",
            model_path_buf
        );

        Ok(Self {
            session: Some(Arc::new(Mutex::new(session))),
            model_path: model_path_buf,
        })
    }

    /// Evaluates trailing audio samples (16kHz mono float32) and predicts turn completion probability (0.0 to 1.0).
    /// If model is absent or audio is too short, returns None to allow standard silence timing fallback.
    pub fn predict_eou(&self, audio_16k: &[f32]) -> Result<Option<f32>> {
        let Some(ref sess_mutex) = self.session else {
            return Ok(None);
        };

        if audio_16k.is_empty() {
            return Ok(None);
        }

        // Window trailing audio up to 1.5 seconds (24,000 samples @ 16kHz)
        let slice = if audio_16k.len() > 24_000 {
            &audio_16k[audio_16k.len() - 24_000..]
        } else {
            audio_16k
        };

        let arr = Array2::from_shape_vec((1, slice.len()), slice.to_vec())
            .map_err(|e| anyhow!("Failed to reshape audio slice: {}", e))?;

        let input_tensor = ort::value::Tensor::from_array(arr)
            .map_err(|e| anyhow!("Failed to create input tensor: {}", e))?;

        let mut sess = sess_mutex.lock();
        let outputs = sess
            .run(ort::inputs![
                "waveform" => input_tensor
            ])
            .map_err(|e| anyhow!("Turn detector inference failed: {}", e))?;

        if let Some(val) = outputs.values().next() {
            if let Ok(tensor_view) = val.try_extract_tensor::<f32>() {
                if let Some(&first_val) = tensor_view.1.first() {
                    let prob = if !(0.0..=1.0).contains(&first_val) {
                        1.0 / (1.0 + (-first_val).exp())
                    } else {
                        first_val
                    };
                    return Ok(Some(prob));
                }
            }
        }

        Ok(None)
    }

    /// Returns true if the neural model is loaded.
    pub fn is_loaded(&self) -> bool {
        self.session.is_some()
    }

    /// Returns the filesystem path configured for the turn detector model.
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }
}
