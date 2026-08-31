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

            let logits_tensor = decoder_outputs
                .get("logits")
                .ok_or_else(|| "Missing 'logits' tensor in decoder outputs".to_string())?;
            let logits_view = logits_tensor
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

            let next_h_tensor = decoder_outputs
                .get("h")
                .ok_or_else(|| "Missing 'h' tensor in decoder outputs".to_string())?;
            let next_h_view = next_h_tensor
                .try_extract_array::<f32>()
                .map_err(|e| format!("Failed to extract decoder h: {}", e))?;
            let h_shape = next_h_view.shape();
            if h_shape.len() < 3 || h_shape[0] < 2 || h_shape[2] < 256 {
                return Err(format!("Unexpected decoder h shape: {:?}", h_shape));
            }

            let next_c_tensor = decoder_outputs
                .get("c")
                .ok_or_else(|| "Missing 'c' tensor in decoder outputs".to_string())?;
            let next_c_view = next_c_tensor
                .try_extract_array::<f32>()
                .map_err(|e| format!("Failed to extract decoder c: {}", e))?;
            let c_shape = next_c_view.shape();
            if c_shape.len() < 3 || c_shape[0] < 2 || c_shape[2] < 256 {
                return Err(format!("Unexpected decoder c shape: {:?}", c_shape));
            }

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

        let enc_outputs_tensor = enc_outputs
            .get("encoder_outputs")
            .ok_or_else(|| "Missing 'encoder_outputs' in encoder outputs".to_string())?;
        let enc_outputs_view = enc_outputs_tensor
            .try_extract_array::<f32>()
            .map_err(|e| format!("Failed to extract encoder_outputs: {}", e))?;

        let enc_outputs_owned = enc_outputs_view
            .to_owned()
            .into_dimensionality::<ndarray::Dim<[usize; 3]>>()
            .map_err(|e| format!("Failed to reshape encoder_outputs: {}", e))?;

        let enc_h_tensor = enc_outputs
            .get("h_states")
            .ok_or_else(|| "Missing 'h_states' in encoder outputs".to_string())?;
        let enc_h_view = enc_h_tensor
            .try_extract_array::<f32>()
            .map_err(|e| format!("Failed to extract h_states: {}", e))?;
        let enc_h_shape = enc_h_view.shape();
        if enc_h_shape.len() < 3 || enc_h_shape[0] < 4 || enc_h_shape[2] < 256 {
            return Err(format!(
                "Unexpected encoder h_states shape: {:?}",
                enc_h_shape
            ));
        }

        let enc_c_tensor = enc_outputs
            .get("c_states")
            .ok_or_else(|| "Missing 'c_states' in encoder outputs".to_string())?;
        let enc_c_view = enc_c_tensor
            .try_extract_array::<f32>()
            .map_err(|e| format!("Failed to extract c_states: {}", e))?;
        let enc_c_shape = enc_c_view.shape();
        if enc_c_shape.len() < 3 || enc_c_shape[0] < 4 || enc_c_shape[2] < 256 {
            return Err(format!(
                "Unexpected encoder c_states shape: {:?}",
                enc_c_shape
            ));
        }

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
    let lock = TRANSLITERATION_ENGINE.read();
    if let Some(engine) = lock.as_ref() {
        return match engine.transliterate_word(word) {
            Ok(res) => res,
            Err(e) => {
                log::warn!(
                    "[Translit] Transliteration failed for '{}': {}. Falling back to raw word.",
                    word,
                    e
                );
                word.to_string()
            }
        };
    }
    drop(lock);

    if let Err(e) = init_transliteration_engine() {
        log::warn!("[Translit] Lazy initialization failed: {}", e);
        return word.to_string();
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

/// Returns true if the string contains any Devanagari Unicode characters (U+0900..=U+097F).
pub fn is_devanagari(text: &str) -> bool {
    text.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c))
}

#[derive(Debug)]
enum ScriptToken {
    Devanagari(String),
    Other(String),
}

/// Partitions a text string into contiguous Devanagari and non-Devanagari character slices.
fn tokenize_devanagari_slices(text: &str) -> Vec<ScriptToken> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_devanagari = false;

    for c in text.chars() {
        let is_c_devanagari = ('\u{0900}'..='\u{097F}').contains(&c);
        if is_c_devanagari {
            if !in_devanagari && !current_token.is_empty() {
                tokens.push(ScriptToken::Other(current_token));
                current_token = String::new();
            }
            in_devanagari = true;
        } else {
            if in_devanagari && !current_token.is_empty() {
                tokens.push(ScriptToken::Devanagari(current_token));
                current_token = String::new();
            }
            in_devanagari = false;
        }
        current_token.push(c);
    }

    if !current_token.is_empty() {
        if in_devanagari {
            tokens.push(ScriptToken::Devanagari(current_token));
        } else {
            tokens.push(ScriptToken::Other(current_token));
        }
    }

    tokens
}

/// Transliterates Devanagari Hindi text to Roman script with trailing incomplete word protection.
pub fn transliterate_if_hi(text: &str, is_final: bool, transliterate_enabled: bool) -> String {
    if !transliterate_enabled || !is_devanagari(text) {
        return text.to_string();
    }

    let ends_with_boundary = if is_final {
        true
    } else if let Some(last_char) = text.chars().last() {
        last_char.is_whitespace() || last_char.is_ascii_punctuation() || last_char == '।'
    } else {
        true
    };

    let tokens = tokenize_devanagari_slices(text);
    let mut result = String::new();
    let num_tokens = tokens.len();

    for (i, token) in tokens.into_iter().enumerate() {
        match token {
            ScriptToken::Devanagari(word) => {
                let is_last = i == num_tokens - 1;
                if is_last && !ends_with_boundary {
                    result.push_str(&word);
                } else {
                    let raw_trans = transliterate(&word);
                    result.push_str(&raw_trans);
                }
            }
            ScriptToken::Other(other) => {
                result.push_str(&other);
            }
        }
    }

    result
}
