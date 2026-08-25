use ndarray::{Array1, Array2, Array3};
use ort::session::Session;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::Path;

/// Character-level Seq2Seq ONNX engine transliterating Devanagari Hindi text to Latin Hinglish.
pub struct TransliterationEngine {
    encoder_sess: Mutex<Session>,
    decoder_sess: Mutex<Session>,
    src_vocab: HashMap<String, i64>,
    tgt_vocab: HashMap<String, i64>,
    tgt_idx2char: HashMap<i64, String>,
}

impl TransliterationEngine {
    /// Loads vocabulary tables and initializes single-threaded ONNX encoder/decoder sessions.
    pub fn new(model_dir: &Path) -> Result<Self, String> {
        let src_vocab_path = model_dir.join("input_vocab.json");
        let tgt_vocab_path = model_dir.join("target_vocab.json");
        let encoder_path = model_dir.join("encoder.onnx");
        let decoder_path = model_dir.join("decoder.onnx");

        if !src_vocab_path.exists()
            || !tgt_vocab_path.exists()
            || !encoder_path.exists()
            || !decoder_path.exists()
        {
            return Err(format!("Model files not found in {:?}", model_dir));
        }

        let src_vocab_str = std::fs::read_to_string(&src_vocab_path)
            .map_err(|e| format!("Failed to read input_vocab.json: {}", e))?;
        let src_vocab: HashMap<String, i64> = serde_json::from_str(&src_vocab_str)
            .map_err(|e| format!("Failed to parse input_vocab.json: {}", e))?;

        let tgt_vocab_str = std::fs::read_to_string(&tgt_vocab_path)
            .map_err(|e| format!("Failed to read target_vocab.json: {}", e))?;
        let tgt_vocab: HashMap<String, i64> = serde_json::from_str(&tgt_vocab_str)
            .map_err(|e| format!("Failed to parse target_vocab.json: {}", e))?;

        let mut tgt_idx2char = HashMap::new();
        for (k, v) in &tgt_vocab {
            tgt_idx2char.insert(*v, k.clone());
        }

        let encoder_sess = Session::builder()
            .map_err(|e| e.to_string())?
            .with_intra_threads(1)
            .map_err(|e| e.to_string())?
            .with_inter_threads(1)
            .map_err(|e| e.to_string())?
            .commit_from_file(encoder_path)
            .map_err(|e| e.to_string())?;

        let decoder_sess = Session::builder()
            .map_err(|e| e.to_string())?
            .with_intra_threads(1)
            .map_err(|e| e.to_string())?
            .with_inter_threads(1)
            .map_err(|e| e.to_string())?
            .commit_from_file(decoder_path)
            .map_err(|e| e.to_string())?;

        Ok(Self {
            encoder_sess: Mutex::new(encoder_sess),
            decoder_sess: Mutex::new(decoder_sess),
            src_vocab,
            tgt_vocab,
            tgt_idx2char,
        })
    }

    /// Converts character sequence into input IDs 2D tensor.
    fn encode_source_ids(&self, word: &str) -> Result<Array2<i64>, String> {
        let unk_idx = *self.src_vocab.get("<unk>").unwrap_or(&1);
        let sos_idx = *self.src_vocab.get("<s>").unwrap_or(&2);
        let eos_idx = *self.src_vocab.get("</s>").unwrap_or(&3);

        let mut src_ids = Vec::with_capacity(word.chars().count() + 2);
        src_ids.push(sos_idx);
        for c in word.chars() {
            let key = c.to_string();
            src_ids.push(*self.src_vocab.get(&key).unwrap_or(&unk_idx));
        }
        src_ids.push(eos_idx);

        let seq_len = src_ids.len();
        Array2::<i64>::from_shape_vec((1, seq_len), src_ids)
            .map_err(|e| format!("Failed to create input_ids shape: {}", e))
    }

    /// Autoregressively generates target Latin character tokens up to 32 steps or EOS.
    fn decode_autoregressive(
        &self,
        enc_outputs: Array3<f32>,
        mut dec_h: Array3<f32>,
        mut dec_c: Array3<f32>,
    ) -> Result<String, String> {
        let tgt_sos_idx = *self.tgt_vocab.get("<s>").unwrap_or(&2);
        let tgt_eos_idx = *self.tgt_vocab.get("</s>").unwrap_or(&3);
        let tgt_pad_idx = *self.tgt_vocab.get("<pad>").unwrap_or(&0);

        let mut dec_input = Array1::<i64>::from_shape_vec(1, vec![tgt_sos_idx])
            .map_err(|e| format!("Failed to create dec_input: {}", e))?;

        let mut output_chars = Vec::new();

        for _step in 0..32 {
            let dec_input_tensor = ort::value::Tensor::from_array(dec_input.clone())
                .map_err(|e| format!("Failed to convert dec_input: {}", e))?;
            let dec_h_tensor = ort::value::Tensor::from_array(dec_h.clone())
                .map_err(|e| format!("Failed to convert dec_h: {}", e))?;
            let dec_c_tensor = ort::value::Tensor::from_array(dec_c.clone())
                .map_err(|e| format!("Failed to convert dec_c: {}", e))?;
            let enc_outputs_tensor = ort::value::Tensor::from_array(enc_outputs.clone())
                .map_err(|e| format!("Failed to convert enc_outputs: {}", e))?;

            let mut dec_sess = self.decoder_sess.lock();
            let decoder_outputs = dec_sess
                .run(ort::inputs![
                    "input_char" => dec_input_tensor,
                    "prev_h" => dec_h_tensor,
                    "prev_c" => dec_c_tensor,
                    "encoder_outputs" => enc_outputs_tensor
                ])
                .map_err(|e| format!("Decoder run failed: {}", e))?;

            let logits_view = decoder_outputs["logits"]
                .try_extract_array::<f32>()
                .map_err(|e| format!("Failed to extract logits: {}", e))?;

            let logits_shape = logits_view.shape();
            let target_vocab_size = logits_shape[logits_shape.len() - 1];

            let mut max_val = f32::NEG_INFINITY;
            let mut next_char_idx = 0;

            if logits_shape.len() == 2 {
                for idx in 0..target_vocab_size {
                    let val = logits_view[[0, idx]];
                    if val > max_val {
                        max_val = val;
                        next_char_idx = idx;
                    }
                }
            } else if logits_shape.len() == 3 {
                for idx in 0..target_vocab_size {
                    let val = logits_view[[0, 0, idx]];
                    if val > max_val {
                        max_val = val;
                        next_char_idx = idx;
                    }
                }
            } else {
                return Err(format!("Unexpected logits shape: {:?}", logits_shape));
            }

            if next_char_idx as i64 == tgt_eos_idx || next_char_idx as i64 == tgt_pad_idx {
                break;
            }

            if let Some(c) = self.tgt_idx2char.get(&(next_char_idx as i64)) {
                output_chars.push(c.clone());
            }

            dec_input[[0]] = next_char_idx as i64;

            let next_h_view = decoder_outputs["h"]
                .try_extract_array::<f32>()
                .map_err(|e| format!("Failed to extract decoder h: {}", e))?;
            let next_c_view = decoder_outputs["c"]
                .try_extract_array::<f32>()
                .map_err(|e| format!("Failed to extract decoder c: {}", e))?;

            for i in 0..2 {
                for j in 0..256 {
                    dec_h[[i, 0, j]] = next_h_view[[i, 0, j]];
                    dec_c[[i, 0, j]] = next_c_view[[i, 0, j]];
                }
            }
        }

        Ok(output_chars.join(""))
    }

    /// Transliterates an individual Devanagari word into phonetic Latin Hinglish text.
    pub fn transliterate_word(&self, word: &str) -> Result<String, String> {
        let input_ids = self.encode_source_ids(word)?;

        let input_ids_tensor = ort::value::Tensor::from_array(input_ids)
            .map_err(|e| format!("Failed to create input_ids tensor: {}", e))?;
        let mut enc_sess = self.encoder_sess.lock();
        let enc_outputs = enc_sess
            .run(ort::inputs![
                "input_ids" => input_ids_tensor
            ])
            .map_err(|e| format!("Encoder run failed: {}", e))?;

        let enc_outputs_view = enc_outputs["encoder_outputs"]
            .try_extract_array::<f32>()
            .map_err(|e| format!("Failed to extract encoder_outputs: {}", e))?;

        let enc_outputs_owned = enc_outputs_view
            .to_owned()
            .into_dimensionality::<ndarray::Dim<[usize; 3]>>()
            .map_err(|e| format!("Failed to reshape encoder_outputs: {}", e))?;

        let enc_h_view = enc_outputs["h_states"]
            .try_extract_array::<f32>()
            .map_err(|e| format!("Failed to extract h_states: {}", e))?;
        let enc_c_view = enc_outputs["c_states"]
            .try_extract_array::<f32>()
            .map_err(|e| format!("Failed to extract c_states: {}", e))?;

        let mut dec_h = Array3::<f32>::zeros((2, 1, 256));
        let mut dec_c = Array3::<f32>::zeros((2, 1, 256));

        for i in 0..2 {
            for j in 0..256 {
                dec_h[[i, 0, j]] =
                    (enc_h_view[[2 * i, 0, j]] + enc_h_view[[2 * i + 1, 0, j]]) / 2.0;
                dec_c[[i, 0, j]] =
                    (enc_c_view[[2 * i, 0, j]] + enc_c_view[[2 * i + 1, 0, j]]) / 2.0;
            }
        }

        self.decode_autoregressive(enc_outputs_owned, dec_h, dec_c)
    }
}

static TRANSLITERATION_ENGINE: parking_lot::RwLock<Option<TransliterationEngine>> =
    parking_lot::RwLock::new(None);

/// Loads the global transliteration engine into memory from the standard model directory.
pub fn init_transliteration_engine() -> Result<(), String> {
    let mut lock = TRANSLITERATION_ENGINE.write();
    if lock.is_some() {
        return Ok(());
    }

    let models_dir = if let Some(p) = crate::utils::paths::try_get() {
        p.models.clone()
    } else {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".vox")
            .join("models")
    };

    let model_path = models_dir.join("translit");
    let engine = TransliterationEngine::new(&model_path)?;
    *lock = Some(engine);
    log::info!("[Translit] Transliteration ONNX engine loaded into memory.");
    Ok(())
}

/// Evicts the global transliteration engine from memory to conserve system RAM.
pub fn unload_transliteration_engine() {
    let mut lock = TRANSLITERATION_ENGINE.write();
    if lock.is_some() {
        *lock = None;
        log::info!("[Translit] Transliteration ONNX engine evicted from memory.");
    }
}

/// Returns true if the transliteration engine is currently initialized in memory.
pub fn is_transliteration_engine_loaded() -> bool {
    TRANSLITERATION_ENGINE.read().is_some()
}

/// Transliterates a Devanagari string with lazy engine initialization and raw word fallback.
pub fn transliterate(word: &str) -> String {
    {
        let lock = TRANSLITERATION_ENGINE.read();
        if lock.is_none() {
            drop(lock);
            if let Err(e) = init_transliteration_engine() {
                log::warn!("[Translit] Lazy initialization failed: {}", e);
                return word.to_string();
            }
        }
    }

    let lock = TRANSLITERATION_ENGINE.read();
    if let Some(engine) = lock.as_ref() {
        match engine.transliterate_word(word) {
            Ok(res) => res,
            Err(e) => {
                log::warn!(
                    "[Translit] Transliteration failed for '{}': {}. Falling back to raw word.",
                    word,
                    e
                );
                word.to_string()
            }
        }
    } else {
        word.to_string()
    }
}
