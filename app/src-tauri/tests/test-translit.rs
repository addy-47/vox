use ndarray::{Array1, Array2, Array3};
use ort::session::Session;
use std::collections::HashMap;
use std::time::Instant;

fn run_debug_translit(
    encoder_sess: &mut Session,
    decoder_sess: &mut Session,
    src_vocab: &HashMap<String, i64>,
    tgt_vocab: &HashMap<String, i64>,
    tgt_idx2char: &HashMap<i64, String>,
    word: &str,
    prepend_sos: bool,
    prepend_pad: bool,
) -> anyhow::Result<String> {
    let unk_idx = *src_vocab.get("<unk>").unwrap_or(&1);
    let sos_idx = *src_vocab.get("<s>").unwrap_or(&2);
    let eos_idx = *src_vocab.get("</s>").unwrap_or(&3);
    let pad_idx = *src_vocab.get("<pad>").unwrap_or(&0);

    let mut src_ids = Vec::new();
    if prepend_sos {
        src_ids.push(sos_idx);
    }
    if prepend_pad {
        src_ids.push(pad_idx);
    }
    for c in word.chars() {
        let key = c.to_string();
        src_ids.push(*src_vocab.get(&key).unwrap_or(&unk_idx));
    }
    src_ids.push(eos_idx);

    let seq_len = src_ids.len();
    let input_ids = Array2::<i64>::from_shape_vec((1, seq_len), src_ids)
        .map_err(|e| anyhow::anyhow!("Failed to create input_ids shape: {}", e))?;

    let input_ids_tensor = ort::value::Tensor::from_array(input_ids)
        .map_err(|e| anyhow::anyhow!("Failed to create input_ids tensor: {}", e))?;

    let enc_outputs = encoder_sess
        .run(ort::inputs![
            "input_ids" => input_ids_tensor
        ])
        .map_err(|e| anyhow::anyhow!("Encoder run failed: {}", e))?;

    let enc_outputs_view = enc_outputs["encoder_outputs"]
        .try_extract_array::<f32>()
        .map_err(|e| anyhow::anyhow!("Failed to extract encoder_outputs: {}", e))?;

    let enc_outputs_owned = enc_outputs_view
        .to_owned()
        .into_dimensionality::<ndarray::Dim<[usize; 3]>>()
        .map_err(|e| anyhow::anyhow!("Failed to reshape encoder_outputs: {}", e))?;

    let enc_h_view = enc_outputs["h_states"]
        .try_extract_array::<f32>()
        .map_err(|e| anyhow::anyhow!("Failed to extract h_states: {}", e))?;
    let enc_c_view = enc_outputs["c_states"]
        .try_extract_array::<f32>()
        .map_err(|e| anyhow::anyhow!("Failed to extract c_states: {}", e))?;

    let mut dec_h = Array3::<f32>::zeros((2, 1, 256));
    let mut dec_c = Array3::<f32>::zeros((2, 1, 256));

    for i in 0..2 {
        for j in 0..256 {
            dec_h[[i, 0, j]] = (enc_h_view[[2 * i, 0, j]] + enc_h_view[[2 * i + 1, 0, j]]) / 2.0;
            dec_c[[i, 0, j]] = (enc_c_view[[2 * i, 0, j]] + enc_c_view[[2 * i + 1, 0, j]]) / 2.0;
        }
    }

    let tgt_sos_idx = *tgt_vocab.get("<s>").unwrap_or(&2);
    let tgt_eos_idx = *tgt_vocab.get("</s>").unwrap_or(&3);
    let tgt_pad_idx = *tgt_vocab.get("<pad>").unwrap_or(&0);

    let mut dec_input = Array1::<i64>::from_shape_vec(1, vec![tgt_sos_idx])
        .map_err(|e| anyhow::anyhow!("Failed to create dec_input: {}", e))?;

    let mut output_chars = Vec::new();

    for _step in 0..15 {
        let dec_input_tensor = ort::value::Tensor::from_array(dec_input.clone())
            .map_err(|e| anyhow::anyhow!("Failed to convert dec_input: {}", e))?;
        let dec_h_tensor = ort::value::Tensor::from_array(dec_h.clone())
            .map_err(|e| anyhow::anyhow!("Failed to convert dec_h: {}", e))?;
        let dec_c_tensor = ort::value::Tensor::from_array(dec_c.clone())
            .map_err(|e| anyhow::anyhow!("Failed to convert dec_c: {}", e))?;
        let enc_outputs_tensor = ort::value::Tensor::from_array(enc_outputs_owned.clone())
            .map_err(|e| anyhow::anyhow!("Failed to convert enc_outputs: {}", e))?;

        let decoder_outputs = decoder_sess
            .run(ort::inputs![
                "input_char" => dec_input_tensor,
                "prev_h" => dec_h_tensor,
                "prev_c" => dec_c_tensor,
                "encoder_outputs" => enc_outputs_tensor
            ])
            .map_err(|e| anyhow::anyhow!("Decoder run failed: {}", e))?;

        let logits_view = decoder_outputs["logits"]
            .try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("Failed to extract logits: {}", e))?;

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
        }

        let is_eos_or_pad =
            next_char_idx as i64 == tgt_eos_idx || next_char_idx as i64 == tgt_pad_idx;
        let next_char_str = if is_eos_or_pad {
            if next_char_idx as i64 == tgt_eos_idx {
                "</s>".to_string()
            } else {
                "<pad>".to_string()
            }
        } else {
            tgt_idx2char
                .get(&(next_char_idx as i64))
                .cloned()
                .unwrap_or_else(|| format!("<unk:{}>", next_char_idx))
        };

        if is_eos_or_pad {
            break;
        }

        output_chars.push(next_char_str);
        dec_input[[0]] = next_char_idx as i64;

        let next_h_view = decoder_outputs["h"]
            .try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("Failed to extract decoder h: {}", e))?;
        let next_c_view = decoder_outputs["c"]
            .try_extract_array::<f32>()
            .map_err(|e| anyhow::anyhow!("Failed to extract decoder c: {}", e))?;

        for i in 0..2 {
            for j in 0..256 {
                dec_h[[i, 0, j]] = next_h_view[[i, 0, j]];
                dec_c[[i, 0, j]] = next_c_view[[i, 0, j]];
            }
        }
    }

    Ok(output_chars.join(""))
}

fn main() -> anyhow::Result<()> {
    let home = dirs::home_dir().expect("Could not find home directory");
    let model_dir = home.join(".vox/models/translit");

    println!("Loading vocabularies...");
    let src_vocab_str = std::fs::read_to_string(model_dir.join("input_vocab.json"))?;
    let src_vocab: HashMap<String, i64> = serde_json::from_str(&src_vocab_str)?;

    let tgt_vocab_str = std::fs::read_to_string(model_dir.join("target_vocab.json"))?;
    let tgt_vocab: HashMap<String, i64> = serde_json::from_str(&tgt_vocab_str)?;

    let mut tgt_idx2char = HashMap::new();
    for (k, v) in &tgt_vocab {
        tgt_idx2char.insert(*v, k.clone());
    }

    println!("Loading sessions...");
    let mut encoder_sess = Session::builder()
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .with_intra_threads(1)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .with_inter_threads(1)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .commit_from_file(model_dir.join("encoder.onnx"))
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let mut decoder_sess = Session::builder()
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .with_intra_threads(1)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .with_inter_threads(1)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .commit_from_file(model_dir.join("decoder.onnx"))
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let test_words = vec![
        // === SHORT PHONETIC / COMMON PARTICLES (test basic matras, single/double char phonetics) ===
        "है",
        "क्या",
        "हाल",
        "में",
        "मेरे",
        "नमस्ते",
        "नहीं",
        "अरे",
        "यार",
        "तो",
        "भी",
        "ना",
        "जी",
        "बस",
        "ठीक",
        "अच्छा",
        "हाँ",
        "बिल्कुल",
        "थोड़ा",
        // === PRONOUNS (common in casual speech) ===
        "मैं",
        "तुम",
        "आप",
        "वो",
        "ये",
        "हम",
        "अपना",
        // === EVERYDAY NOUNS (conversational context) ===
        "बात",
        "काम",
        "घर",
        "पानी",
        "खाना",
        "समय",
        "दिन",
        "रात",
        "फोन",
        // === STRESS-TEST: COMPLEX CLUSTERS / NASALS / RARE MATRAS ===
        "श्री",
        "ज्ञान",
        "संस्कृत",
        "विश्व",
        "प्रेम",
        "ऋषि",
        "छात्र",
        "अंत",
        "संगीत",
        "विशेष",
        "राष्ट्र",
        // === LONGER / MULTI-SYLLABLE COMMON WORDS ===
        "धन्यवाद",
        "माफ़",
        "कृपया",
        "समझ",
        "चलो",
        "बताओ",
        "सुनो",
        "नमस्कार",
        "स्वागत",
        "ज्यादा",
        "पहले",
        "यहाँ",
        "वहाँ",
        "हमेशा",
    ];

    println!(
        "Testing {} words (production path: prepend <s> only)\n",
        test_words.len()
    );

    for word in test_words {
        let start = Instant::now();
        let result = run_debug_translit(
            &mut encoder_sess,
            &mut decoder_sess,
            &src_vocab,
            &tgt_vocab,
            &tgt_idx2char,
            word,
            true,
            false,
        )?;
        let elapsed = start.elapsed();
        println!(
            "{} → {}   ({:.2} ms)",
            word,
            result,
            elapsed.as_secs_f64() * 1000.0
        );
    }

    Ok(())
}
