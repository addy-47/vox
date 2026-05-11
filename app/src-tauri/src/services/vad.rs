use anyhow::{anyhow, Result};
use std::path::Path;
use sherpa_onnx::{
    VoiceActivityDetector, VadModelConfig, TenVadModelConfig,
};
use tauri::Manager;
use tokio::sync::mpsc;
use serde_json::json;
use crate::core::state::{VadCommand, InteractionOwner};
use crate::core::settings::InteractionMode;

/// Wrapper for the Sherpa-ONNX Voice Activity Detection (VAD) engine.
/// 
/// Uses the TenVAD model which is optimized for real-time speech/silence 
/// classification with low latency.
pub struct VadEngine {
    detector: VoiceActivityDetector,
    model_path: std::path::PathBuf,
}
impl VadEngine {
    pub fn new(model_path: &Path, threshold: f32) -> Result<Self> {
        let model_path_buf = model_path.to_path_buf();
        let detector = Self::create_detector(&model_path_buf, threshold)?;
        
        log::info!("[VAD] TenVAD Engine loaded successfully.");
        Ok(Self { 
            detector,
            model_path: model_path_buf,
        })
    }

    fn create_detector(model_path: &Path, threshold: f32) -> Result<VoiceActivityDetector> {
        log::info!("[VAD] >>> Initializing Sherpa-ONNX TenVAD Engine (threshold={})...", threshold);
        
        let config = VadModelConfig {
            silero_vad: Default::default(),
            ten_vad: TenVadModelConfig {
                model: Some(model_path.to_string_lossy().into()),
                threshold,
                min_silence_duration: 0.5,
                min_speech_duration: 0.25,
                window_size: 256,
                max_speech_duration: 30.0,
            },
            sample_rate: 16000,
            num_threads: 1,
            debug: false,
            provider: Some("cpu".into()),
        };
        
        VoiceActivityDetector::create(&config, 60.0)
            .ok_or_else(|| anyhow!("Failed to create Sherpa VoiceActivityDetector. Check model path: {:?}", model_path))
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
        stt_tx: std::sync::mpsc::Sender<crate::services::stt::SttCommand>,
        vad_rx: std::sync::mpsc::Receiver<VadCommand>,
        telemetry_tx: crossbeam_channel::Sender<crate::telemetry::aggregator::TelemetryEvent>,
        vox_event_tx: Option<std::sync::mpsc::Sender<crate::core::events::VoxEvent>>,
    ) -> Result<()> 
    where 
        C: ringbuf::traits::Consumer<Item = f32> 
    {
        log::info!("[VAD] Starting synchronous VAD loop on dedicated thread.");
        
        let mut in_speech = false;
        let mut current_turn_id: u32 = 0;
        let mut utterance_buffer: Vec<f32> = Vec::new();
        let mut samples_since_partial = 0;
        let mut pre_roll_buffer: Vec<f32> = Vec::with_capacity(8000); // 500ms pre-roll

        // Local state initialized once, updated via vad_rx to avoid hot-path locks
        let (threshold_init, noise_gate_init, mode_init, owner_init) = {
            let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
            let settings = state.settings.read().unwrap();
            let owner = *state.owner.blocking_lock();
            let mode = match owner {
                InteractionOwner::Tray => settings.interaction.tray_mode.clone(),
                InteractionOwner::MainWindow => settings.interaction.main_app_mode.clone(),
                InteractionOwner::Ptt => InteractionMode::PTT,
            };
            (settings.vad.threshold, settings.vad.ptt_noise_gate, mode, owner)
        };
        
        let mut threshold = threshold_init;
        let mut noise_gate = noise_gate_init;
        let mut mode = mode_init;
        let mut owner = owner_init;

        log::info!("[VAD] Entering sync loop: threshold={}, noise_gate={}, mode={:?}", threshold, noise_gate, mode);

        
        // 16ms chunks (256 samples at 16kHz) — matches TenVAD window_size default
        let mut chunk = vec![0.0f32; 256];

        loop {
            // ── 0. Process hot-updates (Lock-Free) ───────────────────────────
            while let Ok(cmd) = vad_rx.try_recv() {
                match cmd {
                    VadCommand::UpdateThreshold(v) => {
                        log::info!("[VAD] Updating threshold to {} (Hot-Reloading)...", v);
                        threshold = v;
                        match Self::create_detector(&self.model_path, threshold) {
                            Ok(new_detector) => {
                                self.detector = new_detector;
                                log::info!("[VAD] Detector hot-reloaded successfully.");
                            }
                            Err(e) => {
                                log::error!("[VAD] Failed to hot-reload detector: {}", e);
                            }
                        }
                    }
                    VadCommand::UpdateNoiseGate(v) => {
                        log::info!("[VAD] Updating noise gate to {}", v);
                        noise_gate = v;
                    }
                    VadCommand::UpdateMode(m) => {
                        log::info!("[VAD] Updating interaction mode to {:?}", m);
                        mode = m;
                    }
                    VadCommand::UpdateOwner(o) => {
                        log::info!("[VAD] Updating interaction owner to {:?}", o);
                        owner = o;
                    }
                    VadCommand::Shutdown => {
                        log::info!("[VAD] Shutdown signal received. Exiting loop.");
                        return Ok(());
                    }
                }
            }

            // Check if we have at least 16ms of audio available (256 samples at 16kHz)
            if consumer.occupied_len() >= 256 {
                consumer.pop_slice(&mut chunk);

                // ── Phase 5: High-Frequency Telemetry ────────────────────────
                // (No locks used here anymore — all local state)

                // Calculate RMS energy for the 16ms chunk
                let raw_energy = (chunk.iter().map(|&x| x * x).sum::<f32>() / chunk.len() as f32).sqrt();
                
                // Apply noise gate: if below threshold, send 0 to keep waveform flat
                let gated_raw = if raw_energy > noise_gate { raw_energy } else { 0.0 };
                
                // Balanced multiplier: 8.0x provides good visibility without clipping on normal speech
                let energy = (gated_raw * 8.0).clamp(0.0, 1.0);
                
                // Send to aggregator (non-blocking)
                let _ = telemetry_tx.send(crate::telemetry::aggregator::TelemetryEvent::AudioEnergy {
                    energy,
                    vad_prob: 0.0, // VAD prob will be available after self.detector.accept_waveform
                });

                if mode == crate::core::settings::InteractionMode::PTT {
                    // PTT mode: user explicitly controls recording — skip VAD classification.
                    // Passing all audio regardless of VAD state ensures no onset frames are lost.
                    // The VAD model is NOT called to avoid corrupting its RNN hidden state;
                    // we preserve it so passive mode resumes cleanly after PTT ends.
                    crate::services::ptt::handle_ptt_audio_sync(&app, &chunk);
                    
                    // If we were mid-utterance in passive mode, cleanly exit that state
                    if in_speech {
                        in_speech = false;
                        utterance_buffer.clear();
                        samples_since_partial = 0;
                    }
                    continue;
                }
                
                // ── Phase 4: Speaker-mode mic ducking ────────────────────────────
                // Drop mic frames while playback is active in Speaker mode.
                // Prevents TTS audio from looping back through the mic and re-triggering VAD.
                // In Headset mode, mic stays live for barge-in (pipeline cancellation handles it).
                {
                    let state: tauri::State<'_, std::sync::Arc<crate::core::state::AppState>> = app.state();
                    let is_playing = state.pipeline.playback_active.load(std::sync::atomic::Ordering::Relaxed);
                    if is_playing {
                        let audio_mode = {
                            let settings = state.settings.read().unwrap();
                            settings.audio.output_mode.clone()
                        };
                        if audio_mode == crate::core::settings::AudioOutputMode::Speaker {
                            // Drop this frame — do NOT advance utterance buffer or VAD state
                            continue;
                        }
                    }
                }

                // Classify chunk as speech or silence
                self.detector.accept_waveform(&chunk);

                let detected = self.detector.detected();
                
                if detected {
                    // Transition: Silence -> Speech
                    if !in_speech {
                        in_speech = true;
                        current_turn_id += 1;
                        log::info!("[VAD] >>> SPEECH START (session: {}, owner: {:?})", current_turn_id, owner);
                        
                        // Phase 5: Reset STT decoder state for the new session
                        let _ = stt_tx.send(crate::services::stt::SttCommand::ResetStream);

                        if let Some(ref tx) = vox_event_tx {
                            let _ = tx.send(crate::core::events::VoxEvent::SpeechStart { turn_id: current_turn_id, owner });
                        }

                        let _ = event_tx.try_send(json!({ 
                            "type": "speech_start", 
                            "session_id": current_turn_id 
                        }));
                        utterance_buffer.clear();
                        
                        // Inject the pre-roll audio so we don't drop the start of the speech
                        utterance_buffer.extend_from_slice(&pre_roll_buffer);
                        samples_since_partial = utterance_buffer.len();
                        pre_roll_buffer.clear();
                    }

                    utterance_buffer.extend_from_slice(&chunk);
                    samples_since_partial += chunk.len();

                    // ── Phase 5: Interaction Ownership ───────────────────────────
                    // (owner is now tracked locally via VadCommand)

                    // Partial Emit: Every 800ms (12,800 samples), send the current 
                    // buffer to STT for intermediate transcription.
                    if samples_since_partial >= 12800 {
                        // For partial transcripts, only send the last 15 seconds to keep CPU low
                        // 15 seconds * 16,000 samples/sec = 240,000 samples
                        let start_idx = utterance_buffer.len().saturating_sub(240000);
                        let _ = stt_tx.send(crate::services::stt::SttCommand::Partial(
                            current_turn_id, 
                            owner,
                            utterance_buffer[start_idx..].to_vec()
                        ));
                        samples_since_partial = 0;
                    }
                } else {
                    // Transition: Speech -> Silence
                    if in_speech {
                        in_speech = false;
                        log::info!("[VAD] <<< SPEECH END (session: {}, owner: {:?})", current_turn_id, owner);
                        
                        if let Some(ref tx) = vox_event_tx {
                            let _ = tx.send(crate::core::events::VoxEvent::SpeechEnd { turn_id: current_turn_id, owner });
                        }
                        
                        let _ = event_tx.try_send(json!({ 
                            "type": "speech_end",
                            "session_id": current_turn_id 
                        }));

                        // Flush the internal detector state to capture any trailing samples
                        self.detector.flush();
                        
                        // Routing: Only send to STT if the segment meets a minimum 
                        // duration threshold (e.g., 0.2s) to filter out clicks/noise.
                        if utterance_buffer.len() >= 3200 { 
                            let _ = stt_tx.send(crate::services::stt::SttCommand::Final(
                                current_turn_id, 
                                owner,
                                utterance_buffer.clone()
                            ));
                        }
                        
                        utterance_buffer.clear();
                        samples_since_partial = 0;
                        
                        // RCA Fix: Do NOT reset() RNN state between natural speech segments.
                        // reset() wipes the hidden state, causing slow re-trigger on phrase continuations.
                        // Only reset on explicit ResetStream (new session, PTT stop, etc.).
                        // self.detector.reset(); — REMOVED
                    }
                    
                    // Maintain a sliding window of recent audio during silence
                    if !in_speech {
                        pre_roll_buffer.extend_from_slice(&chunk);
                        if pre_roll_buffer.len() > 8000 {
                            let excess = pre_roll_buffer.len() - 8000;
                            pre_roll_buffer.drain(0..excess);
                        }
                    }
                }
            } else {
                // Throttle: Prevent the loop from spinning and pinning a CPU core 
                // when the ring buffer is empty.
                // Throttle: 3ms sleep — enough to yield without adding perceptible latency
                std::thread::sleep(std::time::Duration::from_millis(3));
            }
        }
    }
}
