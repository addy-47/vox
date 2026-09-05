use super::{
    SttEngine as SttEngineTrait, MODEL_FILE_ASR_DECODER, MODEL_FILE_ASR_ENCODER,
    MODEL_FILE_ASR_JOINER, MODEL_FILE_ASR_TOKENS, SAMPLE_RATE,
};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream};
use std::path::Path;
use std::time::Instant;

struct NemotronInner {
    recognizer: OnlineRecognizer,
    stream: Option<OnlineStream>,
    fed_samples: usize,
    stream_start: Option<Instant>,
}

/// Speech-to-text inference engine wrapping Sherpa-ONNX Nemotron-3.5 online streaming transducer.
pub struct SttEngine {
    inner: Mutex<NemotronInner>,
}

impl SttEngine {
    /// Loads Nemotron-3.5 streaming transducer ONNX model components and initializes the Sherpa recognizer.
    pub fn new(model_dir: &Path, num_threads: u32) -> Result<Self> {
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
        config.model_config.num_threads = num_threads as i32;
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
            inner: Mutex::new(NemotronInner {
                recognizer,
                stream: None,
                fed_samples: 0,
                stream_start: None,
            }),
        })
    }
}

impl SttEngineTrait for SttEngine {
    /// Ingests an incremental streaming audio chunk into the active OnlineStream and decodes ready frames.
    fn accept_audio_chunk(&self, audio: &[f32]) -> Result<()> {
        if audio.is_empty() {
            return Ok(());
        }

        let mut inner = self.inner.lock();
        if inner.stream.is_none() {
            inner.stream = Some(inner.recognizer.create_stream());
            inner.stream_start = Some(Instant::now());
            inner.fed_samples = 0;
        }

        inner.fed_samples += audio.len();
        if let Some(ref stream) = inner.stream {
            stream.accept_waveform(SAMPLE_RATE as i32, audio);
            while inner.recognizer.is_ready(stream) {
                inner.recognizer.decode(stream);
            }
        }

        Ok(())
    }

    /// Fetches the intermediate recognition hypothesis decoded thus far in the active stream.
    fn get_partial_result(&self) -> Result<String> {
        let inner = self.inner.lock();
        if let Some(ref stream) = inner.stream {
            let result = inner.recognizer.get_result(stream);
            Ok(result.map(|r| r.text).unwrap_or_default())
        } else {
            Ok(String::new())
        }
    }

    /// Finalizes the active stream, drains trailing context frames, and returns the full finalized transcript.
    fn finalize_stream(&self) -> Result<String> {
        let start = Instant::now();
        let mut inner = self.inner.lock();
        let stream = inner.stream.take();
        let full_text = if let Some(stream) = stream {
            stream.input_finished();
            while inner.recognizer.is_ready(&stream) {
                inner.recognizer.decode(&stream);
            }
            let result = inner.recognizer.get_result(&stream);
            result.map(|r| r.text).unwrap_or_default()
        } else {
            String::new()
        };

        let duration_s = inner.fed_samples as f32 / SAMPLE_RATE as f32;
        inner.fed_samples = 0;
        inner.stream_start = None;

        let elapsed = start.elapsed().as_secs_f32();
        let rtf = if duration_s > 0.0 {
            elapsed / duration_s
        } else {
            0.0
        };

        log::info!(
            "[STT-Nemotron] Stream Finalized: {:?}. (Audio: {:.2}s, Finalize Latency: {:.3}s, RTF: {:.3})",
            full_text,
            duration_s,
            elapsed,
            rtf
        );

        Ok(full_text)
    }

    /// Resets and discards any active online stream and resets accumulated sample counters.
    fn reset_stream(&self) -> Result<()> {
        let mut inner = self.inner.lock();
        inner.stream = None;
        inner.fed_samples = 0;
        inner.stream_start = None;
        Ok(())
    }

    /// Transcribes complete audio buffer, reusing pre-streamed audio if present, or performing one-shot decoding.
    fn transcribe(&self, audio: &[f32]) -> Result<String> {
        if audio.is_empty() {
            return Ok(String::new());
        }

        let start = Instant::now();
        let mut inner = self.inner.lock();

        // If audio was already fully fed incrementally via accept_audio_chunk, finalize the active stream directly.
        let stream = if inner.stream.is_some() && inner.fed_samples == audio.len() {
            inner.stream.take().unwrap()
        } else {
            let stream = inner.recognizer.create_stream();
            stream.accept_waveform(SAMPLE_RATE as i32, audio);
            stream
        };

        stream.input_finished();

        while inner.recognizer.is_ready(&stream) {
            inner.recognizer.decode(&stream);
        }

        let result = inner.recognizer.get_result(&stream);
        let full_text = result.map(|r| r.text).unwrap_or_default();

        let audio_duration = audio.len() as f32 / SAMPLE_RATE as f32;
        inner.fed_samples = 0;
        inner.stream_start = None;
        inner.stream = None;

        let elapsed = start.elapsed().as_secs_f32();
        let rtf = if audio_duration > 0.0 {
            elapsed / audio_duration
        } else {
            0.0
        };

        log::info!(
            "[STT-Nemotron] Transcribed: {:?}. (Audio: {:.2}s, Latency: {:.3}s, RTF: {:.3})",
            full_text,
            audio_duration,
            elapsed,
            rtf
        );

        Ok(full_text)
    }
}
