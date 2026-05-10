use anyhow::{anyhow, Result};
use std::path::Path;
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineQwen3ASRModelConfig,
};
use tauri::{AppHandle, Manager, Emitter};
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::core::state::{InteractionOwner, InteractionState};
use crate::core::events::VoxEvent;
use crate::core::constants::{
    STT_THROTTLE_MS, MODEL_FILE_ASR_FRONTEND, MODEL_FILE_ASR_ENCODER, 
    MODEL_FILE_ASR_DECODER, MODEL_FILE_ASR_TOKENIZER
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
            conv_frontend: Some(model_dir.join(MODEL_FILE_ASR_FRONTEND).to_string_lossy().into()),
            encoder: Some(model_dir.join(MODEL_FILE_ASR_ENCODER).to_string_lossy().into()),
            decoder: Some(model_dir.join(MODEL_FILE_ASR_DECODER).to_string_lossy().into()),
            tokenizer: Some(model_dir.join(MODEL_FILE_ASR_TOKENIZER).to_string_lossy().into()),
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

// ─── Worker ───────────────────────────────────────────────────────────────────

pub fn spawn_stt_worker(
    app: AppHandle,
    rx: std::sync::mpsc::Receiver<SttCommand>,
    model_path: std::path::PathBuf,
    pipeline_event_tx: Option<std::sync::mpsc::Sender<VoxEvent>>,
    _is_engaged: Arc<std::sync::atomic::AtomicBool>,
) {
    std::thread::spawn(move || {
        log::info!("[STT] >>> Dedicated worker thread started.");
        
        let engine = match SttEngine::new(&model_path) {
            Ok(e) => e,
            Err(err) => {
                log::error!("[STT] CRITICAL: Failed to initialize Sherpa engine: {}", err);
                return;
            }
        };

        let mut last_emit_time = Instant::now();
        let mut last_transcript = String::new();

        while let Ok(cmd) = rx.recv() {
            match cmd {
                SttCommand::Shutdown => {
                    log::info!("[STT] Shutdown signal received. Exiting worker thread.");
                    break;
                }
                SttCommand::Partial(sid, owner, utterance) => {
                    // UX Throttling: Only run inference if STT_THROTTLE_MS passed to save CPU
                    if last_emit_time.elapsed() >= Duration::from_millis(STT_THROTTLE_MS) {
                        match engine.transcribe(&utterance) {
                            Ok(text) => {
                                let text_str: String = text;
                                if !text_str.is_empty() && text_str != last_transcript {
                                    if let Some(ref pipeline_tx) = pipeline_event_tx {
                                        let _ = pipeline_tx.send(VoxEvent::TranscriptPartial {
                                            session_id: sid,
                                            owner,
                                            text: text_str.clone(),
                                        });
                                    }
                                    
                                    last_transcript = text_str;
                                }
                                last_emit_time = Instant::now();
                            }
                            Err(e) => log::error!("[STT] Partial transcription failed: {}", e),
                        }
                    }
                }
                SttCommand::Final(sid, owner, utterance) => {
                    match engine.transcribe(&utterance) {
                        Ok(text) => {
                            let text_str: String = text;
                            // RCA Fix: Always notify the pipeline of a final transcript,
                            // even if it's empty. This ensures the session ID bumps and 
                            // the UI resets its state/clears old text.
                            if let Some(ref pipeline_tx) = pipeline_event_tx {
                                let _ = pipeline_tx.send(VoxEvent::TranscriptFinal {
                                    session_id: sid,
                                    owner,
                                    text: text_str,
                                });
                            }
                        }
                        Err(e) => log::error!("[STT] Final transcription failed: {}", e),
                    }
                    
                    // Signal UI that processing is complete for PTT
                    let target = match owner {
                        InteractionOwner::MainWindow | InteractionOwner::Ptt => "main",
                        InteractionOwner::Tray => "tray",
                    };
                    let _ = app.emit_to(target, "ptt_status", serde_json::json!({ "state": "IDLE" }));

                    // Reset interaction state
                    let state: tauri::State<'_, crate::core::state::AppState> = app.state();
                    let idle_state = if state.pipeline.is_engaged.load(std::sync::atomic::Ordering::Relaxed) {
                        InteractionState::Listening
                    } else {
                        InteractionState::Idle
                    };
                    state.pipeline.update_interaction_state(idle_state, owner, &app);

                    last_transcript.clear();
                    last_emit_time = Instant::now();
                }
                SttCommand::ResetStream => {
                    log::info!("[STT] ResetStream received. Aggressively clearing state.");
                    last_transcript.clear();
                    // Drain pending transcripts
                    while let Ok(pending_cmd) = rx.try_recv() {
                        match pending_cmd {
                            SttCommand::Partial(..) | SttCommand::Final(..) => continue,
                            SttCommand::ResetStream => continue,
                            SttCommand::Shutdown => {
                                log::info!("[STT] Shutdown detected during ResetStream drain.");
                                return;
                            }
                        }
                    }
                }
            }
        }
        log::info!("[STT] Worker thread exiting.");
    });
}
