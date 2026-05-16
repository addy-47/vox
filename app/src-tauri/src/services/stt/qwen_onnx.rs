use anyhow::{anyhow, Result};
use std::path::Path;
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineQwen3ASRModelConfig,
};
use crate::services::traits;
use crate::core::constants::{
    MODEL_FILE_ASR_FRONTEND, MODEL_FILE_ASR_ENCODER, 
    MODEL_FILE_ASR_DECODER, MODEL_FILE_ASR_TOKENIZER
};

pub const SAMPLE_RATE: i32 = 16000;

pub struct SttEngine {
    recognizer: OfflineRecognizer,
}

impl SttEngine {
    pub fn new(model_dir: &Path) -> Result<Self> {
        log::info!("[STT] >>> Initializing Sherpa-ONNX Qwen3-ASR Engine...");
        
        let mut config = OfflineRecognizerConfig::default();
        
        config.model_config.qwen3_asr = OfflineQwen3ASRModelConfig {
            conv_frontend: Some(model_dir.join(MODEL_FILE_ASR_FRONTEND).to_string_lossy().into()),
            encoder: Some(model_dir.join(MODEL_FILE_ASR_ENCODER).to_string_lossy().into()),
            decoder: Some(model_dir.join(MODEL_FILE_ASR_DECODER).to_string_lossy().into()),
            tokenizer: Some(model_dir.join(MODEL_FILE_ASR_TOKENIZER).to_string_lossy().into()),
            max_total_len: 2048,
            max_new_tokens: 512,
            ..Default::default()
        };
        
        config.model_config.num_threads = 2;
        config.model_config.debug = false;
        config.model_config.provider = Some("cpu".into());
        
        let recognizer = OfflineRecognizer::create(&config)
            .ok_or_else(|| anyhow!("Failed to create Sherpa OfflineRecognizer. Verify paths in: {:?}", model_dir))?;
            
        log::info!("[STT] Sherpa-ONNX Engine loaded successfully.");
        Ok(Self { recognizer })
    }
}

impl traits::SttEngine for SttEngine {
    fn transcribe(&self, audio: &[f32]) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let start = std::time::Instant::now();

        let stream = self.recognizer.create_stream();
        stream.accept_waveform(SAMPLE_RATE, audio);
        
        self.recognizer.decode(&stream);
        
        let result = stream.get_result()
            .ok_or_else(|| anyhow!("STT decode failed (no result returned)"))?;
            
        let elapsed = start.elapsed().as_secs_f32();
        let audio_duration = audio.len() as f32 / SAMPLE_RATE as f32;
        let rtf = if audio_duration > 0.0 { elapsed / audio_duration } else { 0.0 };

        log::info!(
            "[STT] Transcribed (Final): {:?}. (Audio: {:.2}s, Latency: {:.2}s, RTF: {:.3})",
            result.text.trim(), audio_duration, elapsed, rtf
        );

        Ok(result.text.trim().to_string())
    }
}
