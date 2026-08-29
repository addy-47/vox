use super::{
    SttEngine as SttEngineTrait, MODEL_FILE_ASR_DECODER, MODEL_FILE_ASR_ENCODER,
    MODEL_FILE_ASR_JOINER, MODEL_FILE_ASR_TOKENS, NEMOTRON_NUM_THREADS, SAMPLE_RATE,
};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig};
use std::path::Path;

/// Speech-to-text inference engine wrapping Sherpa-ONNX Nemotron-3.5 online streaming transducer.
pub struct SttEngine {
    recognizer: Mutex<OnlineRecognizer>,
}

impl SttEngine {
    /// Loads Nemotron-3.5 streaming transducer ONNX model components and initializes the Sherpa recognizer.
    pub fn new(model_dir: &Path) -> Result<Self> {
        log::info!("[STT] >>> Initializing Sherpa-ONNX Nemotron-3.5 Transducer Engine...");

        let encoder_path = model_dir.join(MODEL_FILE_ASR_ENCODER);
        let decoder_path = model_dir.join(MODEL_FILE_ASR_DECODER);
        let joiner_path = model_dir.join(MODEL_FILE_ASR_JOINER);
        let tokens_path = model_dir.join(MODEL_FILE_ASR_TOKENS);

        let mut config = OnlineRecognizerConfig::default();
        config.model_config.transducer.encoder = Some(encoder_path.to_string_lossy().to_string());
        config.model_config.transducer.decoder = Some(decoder_path.to_string_lossy().to_string());
        config.model_config.transducer.joiner = Some(joiner_path.to_string_lossy().to_string());
        config.model_config.tokens = Some(tokens_path.to_string_lossy().to_string());
        config.model_config.num_threads = NEMOTRON_NUM_THREADS;
        config.model_config.provider = Some("cpu".to_string());
        config.decoding_method = Some("greedy_search".to_string());

        let recognizer = OnlineRecognizer::create(&config).ok_or_else(|| {
            anyhow!(
                "Failed to initialize Sherpa-ONNX OnlineRecognizer for Nemotron at {:?}",
                model_dir
            )
        })?;

        log::info!("[STT] Nemotron-3.5 Transducer Engine loaded successfully.");
        Ok(Self {
            recognizer: Mutex::new(recognizer),
        })
    }
}

impl SttEngineTrait for SttEngine {
    /// Transcribes audio buffer using Sherpa OnlineRecognizer streaming graph and logs latency metrics.
    fn transcribe(&self, audio: &[f32]) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let start = std::time::Instant::now();
        let recognizer = self.recognizer.lock();
        let stream = recognizer.create_stream();

        stream.accept_waveform(SAMPLE_RATE as i32, audio);
        stream.input_finished();

        while recognizer.is_ready(&stream) {
            recognizer.decode(&stream);
        }

        let result = recognizer.get_result(&stream);
        let full_text = result.map(|r| r.text).unwrap_or_default();

        let elapsed = start.elapsed().as_secs_f32();
        let audio_duration = audio.len() as f32 / SAMPLE_RATE as f32;
        let rtf = if audio_duration > 0.0 {
            elapsed / audio_duration
        } else {
            0.0
        };

        log::info!(
            "[STT-Nemotron] Transcribed: {:?}. (Audio: {:.2}s, Latency: {:.2}s, RTF: {:.3})",
            full_text,
            audio_duration,
            elapsed,
            rtf
        );

        Ok(full_text)
    }
}
