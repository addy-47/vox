use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::Sender;

use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsKokoroModelConfig,
};

use crate::core::events::VoxEvent;

// ─── TTS Engine (Kokoro-82M via sherpa-onnx) ──────────────────────────────────

pub struct TtsEngine {
    tts: OfflineTts,
}

impl TtsEngine {
    /// Initialize the TTS engine using Kokoro-82M assets.
    ///
    /// The model_dir must contain:
    ///   - model.onnx
    ///   - voices.bin
    ///   - tokens.txt
    ///   - espeak-ng-data/
    pub fn new(model_dir: &Path) -> Result<Self> {
        log::info!("[TTS] Initializing Kokoro-82M engine from: {:?}", model_dir);

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kokoro: OfflineTtsKokoroModelConfig {
                    model: Some(model_dir.join("model.onnx").to_string_lossy().into()),
                    voices: Some(model_dir.join("voices.bin").to_string_lossy().into()),
                    tokens: Some(model_dir.join("tokens.txt").to_string_lossy().into()),
                    data_dir: Some(model_dir.join("espeak-ng-data").to_string_lossy().into()),
                    length_scale: 1.0,
                    ..Default::default()
                },
                num_threads: 2,
                debug: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let tts = OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("[TTS] Failed to create sherpa-onnx OfflineTts instance. Check asset paths and espeak-ng-data."))?;

        log::info!("[TTS] Engine initialized successfully. Sample rate: {}Hz", tts.sample_rate());
        Ok(Self { tts })
    }

    /// Synthesize text and stream audio chunks to the pipeline.
    ///
    /// Uses the sherpa-onnx progress callback to emit samples as they are generated,
    /// enabling low-latency "first audio" even for long sentences.
    pub fn synthesize_chunk(
        &mut self,
        text: &str,
        session_id: u32,
        cancel: Arc<AtomicBool>,
        event_tx: Sender<VoxEvent>,
    ) -> Result<()> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let gen_config = GenerationConfig {
            sid: 0, // Default voice index
            speed: 1.0,
            ..Default::default()
        };

        log::debug!("[TTS] Synthesizing session {}: {:?}", session_id, text);

        let start = std::time::Instant::now();
        let mut total_samples = 0;

        // Clone references for the closure (which must be 'static)
        let cancel_closure = Arc::clone(&cancel);
        let event_tx_closure = event_tx.clone();

        // The callback returns false to stop generation (cancellation)
        let audio = self.tts.generate_with_config(
            text,
            &gen_config,
            Some(move |samples: &[f32], progress: f32| -> bool {
                if cancel_closure.load(Ordering::Relaxed) {
                    log::info!("[TTS] Cancellation requested mid-synthesis (session {})", session_id);
                    return false;
                }

                if !samples.is_empty() {
                    // Send chunk to playback engine via the coordinator
                    // We use blocking_send because we're on a dedicated OS thread, 
                    // and we want to preserve sequence without complex async orchestration here.
                    if let Err(e) = event_tx_closure.blocking_send(VoxEvent::TtsChunk {
                        session_id,
                        samples: samples.to_vec(),
                    }) {
                        log::error!("[TTS] Failed to send TtsChunk: {}", e);
                        return false; // Stop if channel closed
                    }
                }

                log::trace!("[TTS] Progress: {:.1}%", progress * 100.0);
                true
            }),
        );

        if audio.is_none() {
            if cancel.load(Ordering::Relaxed) {
                log::info!("[TTS] Synthesis stopped by cancellation (session {})", session_id);
            } else {
                return Err(anyhow!("[TTS] Generation failed for unknown reason"));
            }
        } else {
            // Success
            if let Some(audio_data) = audio {
                total_samples = audio_data.samples().len();
            }
            
            let elapsed = start.elapsed().as_secs_f32();
            let audio_duration = total_samples as f32 / self.tts.sample_rate() as f32;
            let rtf = if audio_duration > 0.0 { elapsed / audio_duration } else { 0.0 };

            log::info!(
                "[TTS] Synthesis complete (session {}). Duration: {:.2}s, RTF: {:.3}",
                session_id, audio_duration, rtf
            );

            let _ = event_tx.blocking_send(VoxEvent::TtsFinished { session_id });
        }

        Ok(())
    }
}
