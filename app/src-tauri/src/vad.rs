use anyhow::{anyhow, Result};
use std::path::Path;
use sherpa_onnx::{
    VoiceActivityDetector, VadModelConfig, TenVadModelConfig,
};
use serde_json::json;
use tokio::sync::mpsc;
use tauri::Manager;

/// Wrapper for the Sherpa-ONNX Voice Activity Detection (VAD) engine.
/// 
/// Uses the TenVAD model which is optimized for real-time speech/silence 
/// classification with low latency.
pub struct VadEngine {
    detector: VoiceActivityDetector,
}

impl VadEngine {
    /// Initializes the VAD engine using a TenVAD ONNX model.
    /// 
    /// # Arguments
    /// * `model_path` - Path to the ten_vad.onnx file.
    /// 
    /// # Errors
    /// Returns an error if the model file is not found or fails to load.
    pub fn new(model_path: &Path) -> Result<Self> {
        log::info!("[VAD] >>> Initializing Sherpa-ONNX TenVAD Engine...");
        
        let config = VadModelConfig {
            silero_vad: Default::default(),
            ten_vad: TenVadModelConfig {
                model: Some(model_path.to_string_lossy().into()),
                threshold: 0.6,
                min_silence_duration: 0.8,
                min_speech_duration: 0.1,
                window_size: 160, // 10ms at 16kHz
                max_speech_duration: 30.0,
            },
            sample_rate: 16000,
            num_threads: 1,
            debug: false,
            provider: Some("cpu".into()),
        };
        
        // buffer_size_in_seconds = 60.0 allows the detector to track long segments.
        let detector = VoiceActivityDetector::create(&config, 60.0)
            .ok_or_else(|| anyhow!("Failed to create Sherpa VoiceActivityDetector. Check model path: {:?}", model_path))?;
            
    log::info!("[VAD] TenVAD Engine loaded successfully.");
        Ok(Self { detector })
    }

    /// Predicts if the given 10ms chunk contains speech.
    /// 
    /// # Arguments
    /// * `chunk` - 160 samples of 16kHz audio.
    pub fn predict(&mut self, chunk: &[f32]) -> bool {
        self.detector.accept_waveform(chunk);
        self.detector.detected()
    }

    /// The main synchronous loop for VAD processing and audio routing.
    /// 
    /// This function runs on a dedicated OS thread (Tier 2) to ensure that 
    /// audio processing never blocks the UI or the ingestion stream.
    /// 
    /// It performs the following logic:
    /// 1. Consumes 10ms chunks from the lock-free ring buffer.
    /// 2. Feeds samples into the VAD detector.
    /// 3. Detects speech onset (speech_start) and offset (speech_end).
    /// 4. Manages an internal `utterance_buffer` for active segments.
    /// 5. Routes partial buffers to the STT worker every 800ms for streaming feedback.
    /// 6. Routes the final full buffer to the STT worker upon speech termination.
    /// 
    /// # Arguments
    /// * `consumer` - The lock-free ringbuf consumer providing live audio.
    /// * `event_tx` - Channel to send UI-level events (start/end).
    /// * `stt_tx` - Channel to send audio chunks to the STT worker.
    pub fn run_sync_loop<C>(
        &mut self,
        app: tauri::AppHandle,
        mut consumer: C,
        event_tx: mpsc::Sender<serde_json::Value>,
        stt_tx: mpsc::Sender<crate::stt::SttCommand>,
    ) -> Result<()> 
    where 
        C: ringbuf::traits::Consumer<Item = f32> 
    {
        log::info!("[VAD] Starting synchronous VAD loop on dedicated thread.");
        
        let mut in_speech = false;
        let mut current_session_id: u32 = 0;
        let mut utterance_buffer: Vec<f32> = Vec::new();
        let mut samples_since_partial = 0;
        
        // 10ms chunks (160 samples at 16kHz)
        let mut chunk = vec![0.0f32; 160];

        loop {
            // Check if we have at least 10ms of audio available
            if consumer.occupied_len() >= 160 {
                consumer.pop_slice(&mut chunk);

                // Mode-based routing
                let mode = {
                    let state: tauri::State<'_, crate::state::AppState> = app.state();
                    let lock = state.interaction.blocking_lock();
                    *lock
                };

                if mode == crate::state::InteractionMode::Ptt {
                    self.detector.accept_waveform(&chunk);
                    let detected = self.detector.detected();
                    crate::ptt::handle_ptt_audio_sync(&app, &chunk, detected);
                    
                    // Ensure VAD state is clean when we return to passive mode later
                    if in_speech {
                        in_speech = false;
                        self.detector.reset();
                    }
                    continue;
                }
                
                // Classify chunk as speech or silence
                self.detector.accept_waveform(&chunk);

                let detected = self.detector.detected();
                
                if detected {
                    // Transition: Silence -> Speech
                    if !in_speech {
                        in_speech = true;
                        current_session_id += 1;
                        log::info!("[VAD] >>> SPEECH START (session: {})", current_session_id);
                        
                        let _ = event_tx.try_send(json!({ 
                            "type": "speech_start", 
                            "session_id": current_session_id 
                        }));
                        utterance_buffer.clear();
                        samples_since_partial = 0;
                    }

                    utterance_buffer.extend_from_slice(&chunk);
                    samples_since_partial += chunk.len();

                    // Partial Emit: Every 800ms (12,800 samples), send the current 
                    // buffer to STT for intermediate transcription.
                    if samples_since_partial >= 12800 {
                        // For partial transcripts, only send the last 15 seconds to keep CPU low
                        // 15 seconds * 16,000 samples/sec = 240,000 samples
                        let start_idx = utterance_buffer.len().saturating_sub(240000);
                        let _ = stt_tx.try_send(crate::stt::SttCommand::Partial(
                            current_session_id, 
                            utterance_buffer[start_idx..].to_vec()
                        ));
                        samples_since_partial = 0;
                    }
                } else {
                    // Transition: Speech -> Silence
                    if in_speech {
                        in_speech = false;
                        log::info!("[VAD] <<< SPEECH END (session: {})", current_session_id);
                        
                        let _ = event_tx.try_send(json!({ 
                            "type": "speech_end",
                            "session_id": current_session_id 
                        }));

                        // Flush the internal detector state to capture any trailing samples
                        self.detector.flush();
                        
                        // Routing: Only send to STT if the segment meets a minimum 
                        // duration threshold (e.g., 0.2s) to filter out clicks/noise.
                        if utterance_buffer.len() >= 3200 { 
                            let _ = stt_tx.try_send(crate::stt::SttCommand::Final(
                                current_session_id, 
                                utterance_buffer.clone()
                            ));
                        }
                        
                        utterance_buffer.clear();
                        samples_since_partial = 0;
                        
                        // Critical: Reset VAD state for the next utterance to prevent 
                        // history leaking between sessions.
                        self.detector.reset();
                    }
                }
            } else {
                // Throttle: Prevent the loop from spinning and pinning a CPU core 
                // when the ring buffer is empty.
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }
}
