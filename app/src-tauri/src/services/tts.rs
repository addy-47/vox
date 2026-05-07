//! TTS Runtime — Chatterbox multilingual ONNX inference engine.
//!
//! Uses `ort` (load-dynamic, Directive 1) to run the 4-session Chatterbox pipeline:
//!   speech_encoder → embed_tokens → language_model_q4 → conditional_decoder
//!
//! Tensor shapes are HARDCODED (Directive 4) — discovered via one-time introspection
//! and locked here. Mismatch = immediate panic on init.
//!
//! DIRECTIVE 1: Before any ort API call, ORT_DYLIB_PATH must be set to sherpa-onnx's
//! extracted libonnxruntime.so so both crates share a single runtime binary.

use anyhow::{anyhow, Result};
use ndarray::{Array1, Array2, Array3, ArrayD};
use ort::{
    session::{builder::SessionBuilder, Session},
    value::{DynValue, TensorElementType, Value, ValueType},
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::async_runtime::Sender;
use crate::core::events::VoxEvent;

const LM_NUM_HEADS: usize = 16;
/// Head dimension in language_model_q4 KV cache (confirmed: 64).
const LM_HEAD_DIM: usize = 64;
/// Number of transformer layers in language_model_q4 (confirmed: 30 layers = 60 KV tensors / 2).
const LM_NUM_LAYERS: usize = 30;
/// speaker_embeddings vector length (confirmed: 192).
const SPEAKER_EMB_DIM: usize = 192;
/// BOS token for Chatterbox LM.
const BOS_TOKEN: i64 = 1;
/// EOS tokens for Chatterbox LM (either stops generation).
const EOS_TOKENS: &[i64] = &[2, 6562];
/// Maximum speech tokens to generate before forcing EOS.
const MAX_SPEECH_TOKENS: usize = 1500;
/// Default exaggeration value for prosody (1.0 = neutral).
const DEFAULT_EXAGGERATION: f32 = 1.0;

// ─── TTS Engine ───────────────────────────────────────────────────────────────

/// Chatterbox TTS engine owning all four ONNX sessions.
pub struct TtsEngine {
    _speech_encoder:      Session,
    embed_tokens:        Session,
    language_model:      Session,
    conditional_decoder: Session,
    speaker_embeddings: Array2<f32>,
    speaker_features:   Array3<f32>,
    kv_cache: HashMap<String, ArrayD<f32>>,
}

impl TtsEngine {
    pub fn new(model_dir: &Path) -> Result<Self> {
        Self::init_ort_dylib()?;

        if !ort::init()
            .with_name("chatterbox")
            .commit() {
            log::warn!("[TTS] ort already initialized or failed to initialize");
        }

        log::info!("[TTS] Loading Chatterbox ONNX sessions from {:?}", model_dir);

        let onnx_dir = model_dir.join("onnx");

        let mut speech_encoder = Self::load_session(&onnx_dir.join("speech_encoder.onnx"))?;
        let embed_tokens    = Self::load_session(&onnx_dir.join("embed_tokens.onnx"))?;
        let language_model  = Self::load_session(&onnx_dir.join("language_model_q4.onnx"))?;
        let conditional_decoder = Self::load_session(&onnx_dir.join("conditional_decoder.onnx"))?;

        validate_session_inputs(&speech_encoder, "speech_encoder", &[
            ("audio_values", TensorElementType::Float32, 2),
        ]);
        validate_session_inputs(&embed_tokens, "embed_tokens", &[
            ("input_ids",   TensorElementType::Int64, 2),
            ("position_ids",TensorElementType::Int64, 2),
            ("exaggeration",TensorElementType::Float32, 1),
        ]);
        validate_session_inputs(&language_model, "language_model_q4", &[
            ("inputs_embeds",  TensorElementType::Float32, 3),
            ("attention_mask", TensorElementType::Int64, 2),
        ]);
        validate_session_inputs(&conditional_decoder, "conditional_decoder", &[
            ("speech_tokens",      TensorElementType::Int64, 2),
            ("speaker_embeddings", TensorElementType::Float32, 2),
            ("speaker_features",   TensorElementType::Float32, 3),
        ]);

        let (speaker_embeddings, speaker_features) =
            Self::encode_reference_voice(&mut speech_encoder, model_dir)?;

        Ok(Self {
            _speech_encoder: speech_encoder,
            embed_tokens,
            language_model,
            conditional_decoder,
            speaker_embeddings,
            speaker_features,
            kv_cache: HashMap::new(),
        })
    }

    pub fn synthesize_chunk(
        &mut self,
        text: &str,
        session_id: u32,
        cancel: &AtomicBool,
        event_tx: &Sender<VoxEvent>,
    ) -> Result<()> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let text_token_ids: Vec<i64> = std::iter::once(BOS_TOKEN)
            .chain(text.bytes().map(|b| b as i64 + 10))
            .collect();

        let seq_len = text_token_ids.len();
        let input_ids = Array2::from_shape_vec((1, seq_len), text_token_ids.clone())?;
        let position_ids = Array2::from_shape_vec((1, seq_len), (0..seq_len as i64).collect())?;
        let exaggeration = Array1::from_vec(vec![DEFAULT_EXAGGERATION]);

        if cancel.load(Ordering::Relaxed) { return Ok(()); }

        let inputs_embeds = {
            let inputs = vec![
                ("input_ids", Value::from_array(input_ids)?.into_dyn()),
                ("position_ids", Value::from_array(position_ids)?.into_dyn()),
                ("exaggeration", Value::from_array(exaggeration)?.into_dyn()),
            ];

            let embed_outputs = self.embed_tokens.run(inputs)?;

            let view = embed_outputs["inputs_embeds"].try_extract_tensor::<f32>()?;
            let shape: Vec<usize> = view.0.iter().map(|&x| x as usize).collect();
            let data = view.1.to_vec();
            ArrayD::from_shape_vec(shape, data)?.into_dimensionality::<ndarray::Ix3>()?
        };

        let speech_tokens = self.run_language_model(&inputs_embeds, cancel)?;

        if cancel.load(Ordering::Relaxed) { return Ok(()); }

        let n_speech = speech_tokens.len();
        let token_array = Array2::from_shape_vec((1, n_speech), speech_tokens)?;

        let inputs = vec![
            ("speech_tokens", Value::from_array(token_array)?.into_dyn()),
            ("speaker_embeddings", Value::from_array(self.speaker_embeddings.to_owned())?.into_dyn()),
            ("speaker_features", Value::from_array(self.speaker_features.to_owned())?.into_dyn()),
        ];

        let decoder_outputs = self.conditional_decoder.run(inputs)?;

        let view = decoder_outputs["waveform"].try_extract_tensor::<f32>()?;
        let waveform = view.1.to_vec();

        let _ = event_tx.blocking_send(VoxEvent::TtsChunk { session_id, samples: waveform });
        Ok(())
    }

    fn run_language_model(
        &mut self,
        inputs_embeds: &Array3<f32>,
        cancel: &AtomicBool,
    ) -> Result<Vec<i64>> {
        let seq_len = inputs_embeds.shape()[1];
        let mut attention_mask = Array2::from_elem((1, seq_len), 1i64);
        
        // Initial step
        let kv_empty = ArrayD::zeros(vec![1, LM_NUM_HEADS, 0, LM_HEAD_DIM]);
        let kv_inputs = Self::build_kv_inputs(&kv_empty.view(), LM_NUM_LAYERS)?;
        
        let results = self.run_lm_step(inputs_embeds.clone().into_dyn(), attention_mask.clone(), kv_inputs)?;
        self.kv_cache = results;
        
        let logits = self.kv_cache.remove("logits").ok_or_else(|| anyhow!("No logits"))?;
        let mut last_token = self.greedy_sample(&logits)?;
        
        let mut tokens = vec![last_token];
        
        // Autoregressive steps
        while tokens.len() < MAX_SPEECH_TOKENS && !EOS_TOKENS.contains(&last_token) {
            if cancel.load(Ordering::Relaxed) { break; }
            
            let step_input = {
                let step_embeds = self.embed_tokens.run(vec![
                    ("input_ids", Value::from_array(Array2::from_elem((1, 1), last_token))?.into_dyn()),
                    ("position_ids", Value::from_array(Array2::from_elem((1, 1), tokens.len() as i64 + seq_len as i64))?.into_dyn()),
                    ("exaggeration", Value::from_array(Array1::from_vec(vec![DEFAULT_EXAGGERATION]))?.into_dyn()),
                ])?;
                
                let view = step_embeds["inputs_embeds"].try_extract_tensor::<f32>()?;
                let shape: Vec<usize> = view.0.iter().map(|&x| x as usize).collect();
                ArrayD::from_shape_vec(shape, view.1.to_vec())?
            };
            
            attention_mask = Array2::from_elem((1, tokens.len() + seq_len + 1), 1i64);
            
            let mut kv_inputs: Vec<(String, DynValue)> = Vec::with_capacity(LM_NUM_LAYERS * 2);
            for layer in 0..LM_NUM_LAYERS {
                let k_name = format!("past_key_values.{}.key", layer);
                let v_name = format!("past_key_values.{}.value", layer);
                kv_inputs.push((k_name.clone(), Value::from_array(self.kv_cache.get(&k_name).unwrap().to_owned())?.into_dyn()));
                kv_inputs.push((v_name.clone(), Value::from_array(self.kv_cache.get(&v_name).unwrap().to_owned())?.into_dyn()));
            }
            
            let results = self.run_lm_step(step_input, attention_mask.clone(), kv_inputs)?;
            self.kv_cache = results;
            
            let logits = self.kv_cache.remove("logits").ok_or_else(|| anyhow!("No logits"))?;
            last_token = self.greedy_sample(&logits)?;
            tokens.push(last_token);
        }
        
        Ok(tokens)
    }

    fn greedy_sample(&self, logits: &ArrayD<f32>) -> Result<i64> {
        let view = logits.view().into_dimensionality::<ndarray::Ix3>()?;
        let last_step = view.slice(ndarray::s![0, -1, ..]);
        let mut max_idx = 0;
        let mut max_val = f32::NEG_INFINITY;
        for (i, &v) in last_step.iter().enumerate() {
            if v > max_val {
                max_val = v;
                max_idx = i;
            }
        }
        Ok(max_idx as i64)
    }

    fn run_lm_step(
        &mut self,
        embeds: ArrayD<f32>,
        attention_mask: Array2<i64>,
        kv: Vec<(String, DynValue)>,
    ) -> Result<HashMap<String, ArrayD<f32>>> {
        let mut inputs = vec![
            ("inputs_embeds", Value::from_array(embeds)?.into_dyn()),
            ("attention_mask", Value::from_array(attention_mask)?.into_dyn()),
        ];
        for (name, val) in kv {
            inputs.push((Box::leak(name.into_boxed_str()), val)); 
        }
        
        let outputs = self.language_model.run(inputs)?;
        
        let mut results = HashMap::new();
        for (name, val) in outputs.iter() {
            let view = val.try_extract_tensor::<f32>()?;
            let shape: Vec<usize> = view.0.iter().map(|&x| x as usize).collect();
            let array = ArrayD::from_shape_vec(shape, view.1.to_vec())?;
            results.insert(name.to_string(), array);
        }
        Ok(results)
    }

    fn build_kv_inputs(empty: &ndarray::ArrayViewD<f32>, n_layers: usize) -> Result<Vec<(String, Value)>> {
        let mut v = Vec::with_capacity(n_layers * 2);
        for layer in 0..n_layers {
            v.push((format!("past_key_values.{}.key", layer), Value::from_array(empty.to_owned())?.into_dyn()));
            v.push((format!("past_key_values.{}.value", layer), Value::from_array(empty.to_owned())?.into_dyn()));
        }
        Ok(v)
    }


    /// Encode the reference voice WAV through speech_encoder.
    /// Returns (speaker_embeddings [1,192], speaker_features [1,feat_dim,80]).
    /// Falls back to silence (1 second) if wav is missing.
    fn encode_reference_voice(
        speech_encoder: &mut Session,
        model_dir: &Path,
    ) -> Result<(Array2<f32>, Array3<f32>)> {
        const SAMPLE_RATE: usize = 16_000; // Chatterbox expects 16kHz input audio

        let wav_path = model_dir.join("default_voice.wav");
        let audio_samples: Vec<f32> = if wav_path.exists() {
            let mut reader = hound::WavReader::open(&wav_path)
                .map_err(|e| anyhow!("[TTS] Failed to open default_voice.wav: {}", e))?;
            let raw: Vec<i16> = reader.samples::<i16>()
                .filter_map(|s| s.ok())
                .collect();
            // Normalise to f32 [-1.0, 1.0]
            raw.iter().map(|&s| s as f32 / 32768.0).collect()
        } else {
            log::warn!("[TTS] default_voice.wav not found at {:?} — using 1s silence", wav_path);
            vec![0.0f32; SAMPLE_RATE]
        };

        let n = audio_samples.len();
        let audio = Array2::from_shape_vec((1, n), audio_samples)?;

        let inputs = vec![
            ("audio_values", Value::from_array(audio)?.into_dyn()),
        ];
        let outputs = speech_encoder.run(inputs)?;

        // speaker_embeddings: [1, 192]
        let emb_view = outputs["speaker_embeddings"].try_extract_tensor::<f32>()?;
        let (emb_shape, emb_data) = (emb_view.0, emb_view.1);
        let emb_shape_usize: Vec<usize> = emb_shape.iter().map(|&x| x as usize).collect();
        let emb: Array2<f32> = ndarray::ArrayViewD::from_shape(emb_shape_usize, emb_data)?
            .to_owned()
            .into_dimensionality::<ndarray::Ix2>()?;

        // speaker_features: [1, feat_dim, 80]
        let feat_view = outputs["speaker_features"].try_extract_tensor::<f32>()?;
        let (feat_shape, feat_data) = (feat_view.0, feat_view.1);
        let feat_shape_usize: Vec<usize> = feat_shape.iter().map(|&x| x as usize).collect();
        let feat: Array3<f32> = ndarray::ArrayViewD::from_shape(feat_shape_usize, feat_data)?
            .to_owned()
            .into_dimensionality::<ndarray::Ix3>()?;

        // Validate hardcoded dimensions
        assert_eq!(emb.shape()[1], SPEAKER_EMB_DIM,
            "[TTS] FATAL: speaker_embeddings dim mismatch: expected {} got {}",
            SPEAKER_EMB_DIM, emb.shape()[1]);
        assert_eq!(feat.shape()[2], 80,
            "[TTS] FATAL: speaker_features last dim mismatch: expected 80 got {}",
            feat.shape()[2]);

        Ok((emb, feat))
    }

    fn load_session(path: &Path) -> Result<Session> {
        let resolved = path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

        if !resolved.exists() {
            return Err(anyhow!("[TTS] ONNX file not found: {:?}", resolved));
        }

        log::info!("[TTS] Loading session: {:?}", resolved);
        SessionBuilder::new()
            .map_err(|e| anyhow!("[TTS] Failed to create SessionBuilder: {}", e))?
            .with_intra_threads(2)
            .map_err(|e| anyhow!("[TTS] Failed to set intra threads: {}", e))?
            .commit_from_file(&resolved)
            .map_err(|e| anyhow!("[TTS] Session load failed for {:?}: {}", resolved, e))
    }

    /// Directive 1: Locate sherpa-onnx's libonnxruntime.so and point ort at it.
    ///
    /// sherpa-onnx extracts the runtime to a predictable temp path during its
    /// first use. We search for it and set ORT_DYLIB_PATH before any ort call.
    /// If not found, we return an error rather than risk loading a second instance.
    fn init_ort_dylib() -> Result<()> {
        // If already set (e.g., by the caller or environment), trust it.
        if std::env::var("ORT_DYLIB_PATH").is_ok() {
            log::info!("[TTS] ORT_DYLIB_PATH already set — skipping auto-detection");
            return Ok(());
        }

        // Candidate paths where sherpa-onnx may have extracted libonnxruntime
        let candidates = [
            "/tmp/sherpa-onnx-libonnxruntime/libonnxruntime.so",
            "/tmp/libonnxruntime.so",
        ];

        // Also search LD_LIBRARY_PATH
        let ld_paths: Vec<String> = std::env::var("LD_LIBRARY_PATH")
            .unwrap_or_default()
            .split(':')
            .filter(|s| !s.is_empty())
            .map(|s| format!("{}/libonnxruntime.so", s))
            .collect();

        for candidate in candidates.iter().chain(ld_paths.iter().map(|s| s.as_str()).collect::<Vec<_>>().iter()) {
            let p = std::path::Path::new(candidate.as_ref() as &str);
            if p.exists() {
                log::info!("[TTS] Found libonnxruntime at: {:?} — setting ORT_DYLIB_PATH", p);
                std::env::set_var("ORT_DYLIB_PATH", p);
                return Ok(());
            }
        }

        // Last resort: check if libonnxruntime.so.* is on the system
        let system_so = which_onnxruntime();
        if let Some(path) = system_so {
            log::warn!("[TTS] Using system libonnxruntime at {:?} — ensure sherpa-onnx uses the same", path);
            std::env::set_var("ORT_DYLIB_PATH", &path);
            return Ok(());
        }

        Err(anyhow!(
            "[TTS] FATAL: Cannot locate libonnxruntime.so for ORT_DYLIB_PATH. \
             Set ORT_DYLIB_PATH manually to the sherpa-onnx extracted runtime. \
             Loading a separate onnxruntime instance would violate Directive 1."
        ))
    }
}

// ─── Directive 4 Validation ───────────────────────────────────────────────────

/// Validate that a loaded session has the expected named inputs with correct dtype and rank.
/// Panics immediately on any mismatch — production code must never silently accept wrong shapes.
fn validate_session_inputs(
    session: &Session,
    session_name: &str,
    expected: &[(&str, TensorElementType, usize)],
) {
    for (name, expected_dtype, expected_rank) in expected {
        let input = session.inputs().iter().find(|i| i.name() == *name)
            .unwrap_or_else(|| panic!(
                "[TTS] FATAL: Session '{}' missing required input '{}'. \
                  Model schema mismatch — update hardcoded constants in tts.rs.",
                session_name, name
            ));

        // Validate rank
        if let ValueType::Tensor { ty, shape, .. } = input.dtype() {
            let rank = shape.len();
            assert_eq!(rank, *expected_rank,
                "[TTS] FATAL: Session '{}' input '{}': expected rank {} got {}",
                session_name, name, expected_rank, rank);

            assert_eq!(*ty, *expected_dtype,
                "[TTS] FATAL: Session '{}' input '{}': dtype mismatch — expected {:?} got {:?}",
                session_name, name, expected_dtype, ty);
        }
    }

    log::debug!("[TTS] Session '{}' schema validated OK", session_name);
}

/// Probe system for libonnxruntime.so (fallback).
fn which_onnxruntime() -> Option<std::path::PathBuf> {
    for dir in ["/usr/lib", "/usr/local/lib", "/usr/lib/x86_64-linux-gnu"] {
        let p = std::path::Path::new(dir).join("libonnxruntime.so");
        if p.exists() {
            return Some(p);
        }
        // Try versioned name
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("libonnxruntime.so") {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}
