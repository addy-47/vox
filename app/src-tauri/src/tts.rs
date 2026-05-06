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
use ndarray::{Array1, Array2, Array3, CowArray, IxDyn};
use ort::{inputs, Session, SessionBuilder, Value};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

use crate::events::VoxEvent;

// ─── Hardcoded Model Constants (Directive 4) ──────────────────────────────────

/// Vocab size output from language_model_q4 (confirmed: 8194 tokens).
const LM_VOCAB_SIZE: usize = 8194;
/// Hidden dimension across all sessions (confirmed: 1024).
const HIDDEN_DIM: usize = 1024;
/// Number of attention heads in language_model_q4 KV cache (confirmed: 16).
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
///
/// Must be created on the dedicated TTS thread. All sessions share the single
/// onnxruntime instance established by ORT_DYLIB_PATH (Directive 1).
pub struct TtsEngine {
    speech_encoder:      Session,
    embed_tokens:        Session,
    language_model:      Session,
    conditional_decoder: Session,
    /// Pre-encoded speaker conditioning (caches the reference voice).
    speaker_embeddings: Array2<f32>,  // [1, 192]
    speaker_features:   Array3<f32>,  // [1, feat_dim, 80]
}

impl TtsEngine {
    /// Load all four Chatterbox ONNX sessions.
    ///
    /// `model_dir` is the directory containing:
    ///   onnx/speech_encoder.onnx, onnx/embed_tokens.onnx,
    ///   onnx/language_model_q4.onnx, onnx/conditional_decoder.onnx
    ///   and a reference voice wav at default_voice.wav (or silence if missing).
    ///
    /// # Panics
    /// If any loaded model doesn't match the expected input schema (Directive 4).
    pub fn new(model_dir: &Path) -> Result<Self> {
        // ── Directive 1: Set ORT_DYLIB_PATH to sherpa-onnx's libonnxruntime ──
        Self::init_ort_dylib()?;

        // ── Initialise ort runtime ────────────────────────────────────────────
        ort::init()
            .with_name("chatterbox")
            .commit()
            .map_err(|e| anyhow!("[TTS] ort init failed: {}", e))?;

        log::info!("[TTS] Loading Chatterbox ONNX sessions from {:?}", model_dir);

        let onnx_dir = model_dir.join("onnx");

        let speech_encoder = Self::load_session(&onnx_dir.join("speech_encoder.onnx"))?;
        let embed_tokens    = Self::load_session(&onnx_dir.join("embed_tokens.onnx"))?;
        let language_model  = Self::load_session(&onnx_dir.join("language_model_q4.onnx"))?;
        let conditional_decoder = Self::load_session(&onnx_dir.join("conditional_decoder.onnx"))?;

        // ── Directive 4: Validate tensor schemas ──────────────────────────────
        validate_session_inputs(&speech_encoder, "speech_encoder", &[
            ("audio_values", ort::tensor::TensorElementDataType::Float, 2),
        ]);
        validate_session_inputs(&embed_tokens, "embed_tokens", &[
            ("input_ids",   ort::tensor::TensorElementDataType::Int64, 2),
            ("position_ids",ort::tensor::TensorElementDataType::Int64, 2),
            ("exaggeration",ort::tensor::TensorElementDataType::Float, 1),
        ]);
        validate_session_inputs(&language_model, "language_model_q4", &[
            ("inputs_embeds",  ort::tensor::TensorElementDataType::Float, 3),
            ("attention_mask", ort::tensor::TensorElementDataType::Int64, 2),
        ]);
        validate_session_inputs(&conditional_decoder, "conditional_decoder", &[
            ("speech_tokens",      ort::tensor::TensorElementDataType::Int64, 2),
            ("speaker_embeddings", ort::tensor::TensorElementDataType::Float, 2),
            ("speaker_features",   ort::tensor::TensorElementDataType::Float, 3),
        ]);

        log::info!("[TTS] All sessions loaded and validated. Encoding reference voice...");

        // ── Encode reference voice (silence if wav missing) ───────────────────
        let (speaker_embeddings, speaker_features) =
            Self::encode_reference_voice(&speech_encoder, model_dir)?;

        log::info!("[TTS] Ready. speaker_embeddings=[1,{}] speaker_features=[1,{},80]",
            SPEAKER_EMB_DIM, speaker_features.shape()[1]);

        Ok(Self {
            speech_encoder,
            embed_tokens,
            language_model,
            conditional_decoder,
            speaker_embeddings,
            speaker_features,
        })
    }

    /// Synthesise `text` into PCM audio at 24kHz and stream chunks via `tx`.
    ///
    /// Checks `cancel_flag` before each major pipeline stage.
    pub fn synthesize_chunk(
        &self,
        text: &str,
        session_id: u32,
        cancel_flag: &Arc<AtomicBool>,
        tx: &Sender<VoxEvent>,
    ) -> Result<()> {
        if cancel_flag.load(Ordering::Relaxed) {
            return Ok(());
        }

        log::debug!("[TTS] Synthesising: {:?}", text);

        // ── 1. Tokenise text (simple ASCII byte tokenisation as placeholder) ──
        // TODO: replace with the actual Chatterbox tokenizer from tokenizer.json
        // For now: use character-level token IDs offset by BOS (functional stub)
        let text_token_ids: Vec<i64> = std::iter::once(BOS_TOKEN)
            .chain(text.bytes().map(|b| b as i64 + 10))
            .collect();

        let seq_len = text_token_ids.len();
        let input_ids = Array2::from_shape_vec(
            (1, seq_len),
            text_token_ids.clone(),
        )?;
        let position_ids = Array2::from_shape_vec(
            (1, seq_len),
            (0..seq_len as i64).collect(),
        )?;
        let exaggeration = Array1::from_vec(vec![DEFAULT_EXAGGERATION]);

        // ── 2. Embed tokens ───────────────────────────────────────────────────
        if cancel_flag.load(Ordering::Relaxed) { return Ok(()); }

        let embed_outputs = self.embed_tokens.run(inputs![
            "input_ids"    => input_ids.view(),
            "position_ids" => position_ids.view(),
            "exaggeration" => exaggeration.view(),
        ]?)?;

        let inputs_embeds: Array3<f32> = embed_outputs["inputs_embeds"]
            .try_extract_tensor::<f32>()?
            .view()
            .to_owned()
            .into_dimensionality::<ndarray::Ix3>()?;

        // ── 3. Autoregressive LM decoding ────────────────────────────────────
        let speech_tokens = self.run_language_model(&inputs_embeds, cancel_flag)?;

        if cancel_flag.load(Ordering::Relaxed) { return Ok(()); }

        // ── 4. Decode speech tokens → waveform ───────────────────────────────
        let n_speech = speech_tokens.len();
        let token_array = Array2::from_shape_vec((1, n_speech), speech_tokens)?;

        let decoder_outputs = self.conditional_decoder.run(inputs![
            "speech_tokens"      => token_array.view(),
            "speaker_embeddings" => self.speaker_embeddings.view(),
            "speaker_features"   => self.speaker_features.view(),
        ]?)?;

        let waveform: Vec<f32> = decoder_outputs["waveform"]
            .try_extract_tensor::<f32>()?
            .view()
            .iter()
            .copied()
            .collect();

        log::debug!("[TTS] Synthesised {} samples (24kHz)", waveform.len());

        // Send audio chunk — pipeline.rs will upsample 2x (Directive 3)
        let _ = tx.blocking_send(VoxEvent::TtsChunk { session_id, samples: waveform });
        Ok(())
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Run the autoregressive language_model_q4 loop to produce speech tokens.
    fn run_language_model(
        &self,
        inputs_embeds: &Array3<f32>,
        cancel_flag: &Arc<AtomicBool>,
    ) -> Result<Vec<i64>> {
        let seq_len = inputs_embeds.shape()[1];

        // Initialise empty KV cache (past_sequence_length = 0)
        let empty_kv: Array<f32, _> = ndarray::Array::zeros((1, LM_NUM_HEADS, 0, LM_HEAD_DIM));

        // Build initial attention mask (all ones for seq_len)
        let mut attention_mask: Vec<i64> = vec![1i64; seq_len];
        let mut past_seq_len = 0usize;

        // Prefill: run once with the full embedded sequence
        let kv_inputs = Self::build_kv_inputs(&empty_kv, LM_NUM_LAYERS);
        let prefill_outputs = self.run_lm_step(
            inputs_embeds.view().into_dyn(),
            &attention_mask,
            &kv_inputs,
        )?;

        // Extract logits and past KV from prefill
        let logits = Self::extract_logits(&prefill_outputs)?;
        let mut speech_tokens: Vec<i64> = Vec::with_capacity(256);

        // Sample first speech token from prefill logits
        let first_token = Self::greedy_sample(&logits, seq_len - 1);
        if EOS_TOKENS.contains(&first_token) {
            return Ok(speech_tokens);
        }
        speech_tokens.push(first_token);

        // Update KV cache from prefill present states
        let mut kv_cache = Self::extract_present_kv(&prefill_outputs, LM_NUM_LAYERS)?;
        past_seq_len += seq_len;

        // Decode loop — one token at a time
        let mut current_token = first_token;
        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }
            if speech_tokens.len() >= MAX_SPEECH_TOKENS {
                log::warn!("[TTS] Hit MAX_SPEECH_TOKENS limit");
                break;
            }

            // Single-token input embedding for decode step
            let token_array = Array2::from_shape_vec((1, 1), vec![current_token])?;
            let pos_array   = Array2::from_shape_vec((1, 1), vec![past_seq_len as i64])?;
            let exag        = Array1::from_vec(vec![DEFAULT_EXAGGERATION]);

            let step_embed = self.embed_tokens.run(inputs![
                "input_ids"    => token_array.view(),
                "position_ids" => pos_array.view(),
                "exaggeration" => exag.view(),
            ]?)?;

            let step_embeds: Array3<f32> = step_embed["inputs_embeds"]
                .try_extract_tensor::<f32>()?
                .view()
                .to_owned()
                .into_dimensionality::<ndarray::Ix3>()?;

            // Extend attention mask
            attention_mask.push(1);
            let kv_inputs = Self::build_kv_inputs_from_cache(&kv_cache);
            let step_outputs = self.run_lm_step(
                step_embeds.view().into_dyn(),
                &attention_mask,
                &kv_inputs,
            )?;

            let step_logits = Self::extract_logits(&step_outputs)?;
            let next_token = Self::greedy_sample(&step_logits, 0);

            if EOS_TOKENS.contains(&next_token) {
                break;
            }

            speech_tokens.push(next_token);
            kv_cache = Self::extract_present_kv(&step_outputs, LM_NUM_LAYERS)?;
            past_seq_len += 1;
            current_token = next_token;
        }

        Ok(speech_tokens)
    }

    /// Single LM forward pass — handles KV cache routing.
    fn run_lm_step(
        &self,
        inputs_embeds: ndarray::ArrayViewD<f32>,
        attention_mask: &[i64],
        kv_inputs: &[(String, Value)],
    ) -> Result<ort::SessionOutputs> {
        let total_seq = attention_mask.len();
        let mask_arr = Array2::from_shape_vec((1, total_seq), attention_mask.to_vec())?;

        let mut input_map: Vec<(&str, Value)> = vec![
            ("inputs_embeds",  Value::from_array(inputs_embeds)?),
            ("attention_mask", Value::from_array(mask_arr.view())?),
        ];

        for (name, val) in kv_inputs {
            // Safety: name outlives this scope — use as_str ref
            input_map.push((name.as_str(), val.clone()));
        }

        Ok(self.language_model.run(input_map)?)
    }

    fn build_kv_inputs(empty: &ndarray::Array<f32, ndarray::IxDyn>, n_layers: usize)
        -> Vec<(String, Value)>
    {
        let mut v = Vec::with_capacity(n_layers * 2);
        for layer in 0..n_layers {
            v.push((format!("past_key_values.{}.key",   layer), Value::from_array(empty.view()).unwrap()));
            v.push((format!("past_key_values.{}.value", layer), Value::from_array(empty.view()).unwrap()));
        }
        v
    }

    fn build_kv_inputs_from_cache(cache: &[(Array3<f32>, Array3<f32>)])
        -> Vec<(String, Value)>
    {
        let mut v = Vec::with_capacity(cache.len() * 2);
        for (i, (k, val)) in cache.iter().enumerate() {
            v.push((format!("past_key_values.{}.key",   i), Value::from_array(k.view()).unwrap()));
            v.push((format!("past_key_values.{}.value", i), Value::from_array(val.view()).unwrap()));
        }
        v
    }

    fn extract_logits(outputs: &ort::SessionOutputs) -> Result<Vec<f32>> {
        let logits: Vec<f32> = outputs["logits"]
            .try_extract_tensor::<f32>()?
            .view()
            .iter()
            .copied()
            .collect();
        Ok(logits)
    }

    /// Greedy argmax over vocab at the given sequence position.
    fn greedy_sample(logits: &[f32], pos: usize) -> i64 {
        let start = pos * LM_VOCAB_SIZE;
        let end   = start + LM_VOCAB_SIZE;
        let slice = &logits[start..end];
        slice.iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i as i64)
            .unwrap_or(EOS_TOKENS[0])
    }

    fn extract_present_kv(
        outputs: &ort::SessionOutputs,
        n_layers: usize,
    ) -> Result<Vec<(Array3<f32>, Array3<f32>)>> {
        let mut cache = Vec::with_capacity(n_layers);
        for layer in 0..n_layers {
            let k: Array3<f32> = outputs[&format!("present.{}.key",   layer)]
                .try_extract_tensor::<f32>()?
                .view()
                .to_owned()
                .into_dimensionality::<ndarray::Ix3>()?;
            let v: Array3<f32> = outputs[&format!("present.{}.value", layer)]
                .try_extract_tensor::<f32>()?
                .view()
                .to_owned()
                .into_dimensionality::<ndarray::Ix3>()?;
            cache.push((k, v));
        }
        Ok(cache)
    }

    /// Encode the reference voice WAV through speech_encoder.
    /// Returns (speaker_embeddings [1,192], speaker_features [1,feat_dim,80]).
    /// Falls back to silence (1 second) if wav is missing.
    fn encode_reference_voice(
        speech_encoder: &Session,
        model_dir: &Path,
    ) -> Result<(Array2<f32>, Array3<f32>)> {
        const SAMPLE_RATE: usize = 16_000; // Chatterbox expects 16kHz input audio

        let wav_path = model_dir.join("default_voice.wav");
        let audio_samples: Vec<f32> = if wav_path.exists() {
            let mut reader = hound::WavReader::open(&wav_path)
                .map_err(|e| anyhow!("[TTS] Failed to open default_voice.wav: {}", e))?;
            let spec = reader.spec();
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

        let outputs = speech_encoder.run(inputs![
            "audio_values" => audio.view(),
        ]?)?;

        // speaker_embeddings: [1, 192]
        let emb: Array2<f32> = outputs["speaker_embeddings"]
            .try_extract_tensor::<f32>()?
            .view()
            .to_owned()
            .into_dimensionality::<ndarray::Ix2>()?;

        // speaker_features: [1, feat_dim, 80]
        let feat: Array3<f32> = outputs["speaker_features"]
            .try_extract_tensor::<f32>()?
            .view()
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
        SessionBuilder::new()?
            .with_intra_threads(2)?
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
    expected: &[(&str, ort::tensor::TensorElementDataType, usize)],
) {
    for (name, expected_dtype, expected_rank) in expected {
        let input = session.inputs.iter().find(|i| i.name == *name)
            .unwrap_or_else(|| panic!(
                "[TTS] FATAL: Session '{}' missing required input '{}'. \
                 Model schema mismatch — update hardcoded constants in tts.rs.",
                session_name, name
            ));

        // Validate rank
        if let ort::inputs::InputType::Tensor(ref tensor_info) = input.input_type {
            let rank = tensor_info.dimensions().len();
            assert_eq!(rank, *expected_rank,
                "[TTS] FATAL: Session '{}' input '{}': expected rank {} got {}",
                session_name, name, expected_rank, rank);

            assert_eq!(tensor_info.element_type(), *expected_dtype,
                "[TTS] FATAL: Session '{}' input '{}': dtype mismatch — expected {:?} got {:?}",
                session_name, name, expected_dtype, tensor_info.element_type());
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
