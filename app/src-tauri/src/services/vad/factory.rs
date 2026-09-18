use std::path::Path;

use crate::{
    core::settings::{VadBackendOption, VadSettings},
    services::vad::{
        providers::{EarshotVadEngine, SileroVadEngine, TenVadEngine, VadBackend},
        MODEL_DIR_VAD, MODEL_FILE_VAD, MODEL_FILE_VAD_SILERO,
    },
};

/// Resolves model paths and instantiates the configured VAD backend with automatic fallback.
pub fn create_vad_instance_from_settings(
    settings: &VadSettings,
    models_dir: &Path,
) -> Result<VadBackend, String> {
    let min_silence_duration = settings.silence_duration_ms as f32 / 1000.0;
    let min_speech_duration = settings.speech_onset_ms as f32 / 1000.0;
    let max_speech_duration = settings.max_speech_duration_s as f32;
    let threshold = settings.threshold;

    match settings.vad_backend {
        VadBackendOption::Earshot => {
            log::info!("[VAD Factory] Initializing pure-Rust Earshot VAD");
            EarshotVadEngine::new(threshold)
                .map(VadBackend::Earshot)
                .map_err(|e| format!("Earshot VAD init failed: {}", e))
        }
        VadBackendOption::SileroVad => {
            let vad_path = models_dir.join(MODEL_DIR_VAD).join(MODEL_FILE_VAD_SILERO);
            if !vad_path.exists() {
                log::warn!(
                    "[VAD Factory] Silero VAD model missing at {:?}. Falling back to Earshot.",
                    vad_path
                );
                return EarshotVadEngine::new(threshold)
                    .map(VadBackend::Earshot)
                    .map_err(|e| format!("Earshot VAD fallback failed: {}", e));
            }
            log::info!(
                "[VAD Factory] Initializing Silero ONNX VAD from {:?} (threshold={}, min_silence={}s, min_speech={}s, max_speech={}s)",
                vad_path,
                threshold,
                min_silence_duration,
                min_speech_duration,
                max_speech_duration
            );
            SileroVadEngine::new(
                &vad_path,
                threshold,
                min_silence_duration,
                min_speech_duration,
                max_speech_duration,
            )
            .map(VadBackend::Silero)
            .map_err(|e| format!("Silero VAD init failed: {}", e))
        }
        VadBackendOption::TenVad => {
            let vad_path = models_dir.join(MODEL_DIR_VAD).join(MODEL_FILE_VAD);
            if !vad_path.exists() {
                log::warn!(
                    "[VAD Factory] Ten VAD model missing at {:?}. Falling back to Earshot.",
                    vad_path
                );
                return EarshotVadEngine::new(threshold)
                    .map(VadBackend::Earshot)
                    .map_err(|e| format!("Earshot VAD fallback failed: {}", e));
            }
            log::info!(
                "[VAD Factory] Initializing Ten ONNX VAD from {:?} (threshold={}, min_silence={}s, min_speech={}s, max_speech={}s)",
                vad_path,
                threshold,
                min_silence_duration,
                min_speech_duration,
                max_speech_duration
            );
            TenVadEngine::new(
                &vad_path,
                threshold,
                min_silence_duration,
                min_speech_duration,
                max_speech_duration,
            )
            .map(VadBackend::Ten)
            .map_err(|e| format!("Ten VAD init failed: {}", e))
        }
    }
}
