use std::path::Path;

use anyhow::{anyhow, Result};
use sherpa_onnx::{OfflineQwen3ASRModelConfig, OfflineRecognizer, OfflineRecognizerConfig};

use super::{
    SttEngine as SttEngineTrait, MODEL_FILE_ASR_DECODER, MODEL_FILE_ASR_ENCODER,
    MODEL_FILE_ASR_FRONTEND, MODEL_FILE_ASR_TOKENIZER, QWEN_MAX_NEW_TOKENS, QWEN_MAX_TOTAL_LEN,
    SAMPLE_RATE,
};

/// Speech-to-text inference engine wrapping Sherpa-ONNX Qwen3-ASR offline recognizer.
pub struct SttEngine {
    recognizer: OfflineRecognizer,
}

impl SttEngine {
    /// Loads Qwen3-ASR ONNX model components and initializes the Sherpa recognizer.
    pub fn new(model_dir: &Path, num_threads: u32) -> Result<Self> {
        log::info!("[STT] >>> Initializing Sherpa-ONNX Qwen3-ASR Engine...");

        let mut config = OfflineRecognizerConfig {
            decoding_method: Some("greedy_search".to_string()),
            ..Default::default()
        };

        config.model_config.qwen3_asr = OfflineQwen3ASRModelConfig {
            conv_frontend: Some(
                model_dir
                    .join(MODEL_FILE_ASR_FRONTEND)
                    .to_string_lossy()
                    .into(),
            ),
            encoder: Some(
                model_dir
                    .join(MODEL_FILE_ASR_ENCODER)
                    .to_string_lossy()
                    .into(),
            ),
            decoder: Some(
                model_dir
                    .join(MODEL_FILE_ASR_DECODER)
                    .to_string_lossy()
                    .into(),
            ),
            tokenizer: Some(
                model_dir
                    .join(MODEL_FILE_ASR_TOKENIZER)
                    .to_string_lossy()
                    .into(),
            ),
            max_total_len: QWEN_MAX_TOTAL_LEN,
            max_new_tokens: QWEN_MAX_NEW_TOKENS,
            ..Default::default()
        };

        config.model_config.num_threads = num_threads as i32;
        config.model_config.debug = false;
        config.model_config.provider = Some("cpu".into());

        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            anyhow!(
                "Failed to create Sherpa OfflineRecognizer. Verify paths in: {:?}",
                model_dir
            )
        })?;

        log::info!("[STT] Sherpa-ONNX Engine loaded successfully.");
        Ok(Self { recognizer })
    }
}

/// Removes CJK Unicode ideographs and normalization artifact spaces from the transcribed text.
fn strip_cjk(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            !(('\u{4E00}'..='\u{9FFF}').contains(&c)
                || ('\u{3400}'..='\u{4DBF}').contains(&c)
                || ('\u{F900}'..='\u{FAFF}').contains(&c)
                || ('\u{3000}'..='\u{303F}').contains(&c))
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl SttEngineTrait for SttEngine {
    /// Transcribes the audio waveform using Qwen3-ASR and returns cleaned transcript text.
    fn transcribe(&self, audio: &[f32]) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let start = std::time::Instant::now();

        let stream = self.recognizer.create_stream();
        stream.accept_waveform(SAMPLE_RATE as i32, audio);

        self.recognizer.decode(&stream);

        let result = stream
            .get_result()
            .ok_or_else(|| anyhow!("STT decode failed (no result returned)"))?;

        let elapsed = start.elapsed().as_secs_f32();
        let audio_duration = audio.len() as f32 / SAMPLE_RATE as f32;
        let rtf = if audio_duration > 0.0 {
            elapsed / audio_duration
        } else {
            0.0
        };

        let cleaned_text = strip_cjk(result.text.trim());

        log::info!(
            "[STT] Transcribed (Final): {:?}. (Audio: {:.2}s, Latency: {:.2}s, RTF: {:.3})",
            cleaned_text,
            audio_duration,
            elapsed,
            rtf
        );

        Ok(cleaned_text)
    }
}
