use anyhow::{anyhow, Result};
use std::path::Path;
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineQwen3ASRModelConfig,
};

// ─── Constants ────────────────────────────────────────────────────────────────

/// The expected input sample rate for the Qwen3-ASR model.
pub const SAMPLE_RATE: i32 = 16000;

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Commands sent from the VAD/Router thread to the STT worker thread.
pub enum SttCommand {
    /// Partial audio buffer sent during an active speech segment for real-time feedback.
    /// Format: (Session ID, Owner, Samples)
    Partial(u32, crate::core::state::InteractionOwner, Vec<f32>),
    
    /// Complete audio buffer sent when VAD detects the end of a speech segment.
    /// Format: (Session ID, Owner, Samples)
    Final(u32, crate::core::state::InteractionOwner, Vec<f32>),

    /// Resets the internal acoustic and contextual states.
    ResetStream,

    /// Gracefully shutdown the worker thread.
    Shutdown,
}

// ─── Engine ───────────────────────────────────────────────────────────────────

/// Wrapper for the Sherpa-ONNX offline recognizer, optimized for Qwen3-ASR.
pub struct SttEngine {
    recognizer: OfflineRecognizer,
}

impl SttEngine {
    /// Creates a new SttEngine instance by loading ONNX models from the specified directory.
    /// 
    /// # Arguments
    /// * `model_dir` - Path to the directory containing conv_frontend.onnx, encoder.onnx, etc.
    /// 
    /// # Errors
    /// Returns an error if any of the model files are missing or if the ONNX runtime 
    /// fails to initialize the engine.
    pub fn new(model_dir: &Path) -> Result<Self> {
        log::info!("[STT] >>> Initializing Sherpa-ONNX Qwen3-ASR Engine...");
        
        let mut config = OfflineRecognizerConfig::default();
        
        // Qwen3-ASR is an Audio-LLM model that requires a specific multi-stage pipeline:
        // 1. conv_frontend: Initial audio feature extraction.
        // 2. encoder: Transformer-based speech encoding.
        // 3. decoder: Auto-regressive text generation.
        // 4. tokenizer: BPE-based token mapping.
        config.model_config.qwen3_asr = OfflineQwen3ASRModelConfig {
            conv_frontend: Some(model_dir.join("conv_frontend.onnx").to_string_lossy().into()),
            encoder: Some(model_dir.join("encoder.int8.onnx").to_string_lossy().into()),
            decoder: Some(model_dir.join("decoder.int8.onnx").to_string_lossy().into()),
            tokenizer: Some(model_dir.join("tokenizer").to_string_lossy().into()),
            max_total_len: 2048,
            max_new_tokens: 512,
            ..Default::default()
        };
        
        // Runtime optimization settings:
        // - num_threads: Set to 2 to balance latency and background CPU impact.
        // - provider: Uses "cpu" for maximum compatibility across Linux distros.
        config.model_config.num_threads = 2;
        config.model_config.debug = false;
        config.model_config.provider = Some("cpu".into());
        
        let recognizer = OfflineRecognizer::create(&config)
            .ok_or_else(|| anyhow!("Failed to create Sherpa OfflineRecognizer. Verify paths in: {:?}", model_dir))?;
            
        log::info!("[STT] Sherpa-ONNX Engine loaded successfully.");
        Ok(Self { recognizer })
    }

    /// Processes a single audio buffer and returns the transcribed text.
    /// 
    /// This function handles the low-level Sherpa-ONNX stream lifecycle:
    /// 1. Creating a transient OfflineStream.
    /// 2. Feeding the resampled 16kHz waveform.
    /// 3. Executing the synchronous decode pass.
    /// 4. Extracting the JSON-formatted result into a clean String.
    /// 
    /// # Arguments
    /// * `audio` - Slice of f32 samples at 16,000Hz.
    pub fn transcribe(&self, audio: &[f32]) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let start = std::time::Instant::now();

        // We create a fresh stream for every request to ensure KV-cache is cleared 
        // and no state leaks between utterances.
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(SAMPLE_RATE, audio);
        
        // Perform the blocking inference pass on the calling thread.
        self.recognizer.decode(&stream);
        
        // Retrieve the transcription result.
        let result = stream.get_result()
            .ok_or_else(|| anyhow!("STT decode failed (no result returned)"))?;
            
        let elapsed = start.elapsed().as_secs_f32();
        let audio_duration = audio.len() as f32 / SAMPLE_RATE as f32;
        let rtf = if audio_duration > 0.0 { elapsed / audio_duration } else { 0.0 };

        log::info!(
            "[STT] Transcribed: {:?}. (Audio: {:.2}s, Latency: {:.2}s, RTF: {:.3})",
            result.text.trim(), audio_duration, elapsed, rtf
        );

        Ok(result.text.trim().to_string())
    }
}
