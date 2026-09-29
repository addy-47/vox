use serde::{Deserialize, Serialize};

use crate::core::defaults::{
    DEFAULT_VAD_MAX_SPEECH_DURATION_S, DEFAULT_VAD_PTT_NOISE_GATE, DEFAULT_VAD_SILENCE_DURATION_MS,
    DEFAULT_VAD_SPEECH_ONSET_MS, DEFAULT_VAD_THRESHOLD,
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum VadBackendOption {
    /// Earshot — pure Rust, no ONNX dependency, embedded neural weights.
    Earshot,
    /// TenVAD — ONNX-based standard VAD engine.
    TenVad,
    /// SileroVad — MIT-licensed ONNX VAD engine with superior noise immunity.
    #[default]
    SileroVad,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct VadSettings {
    pub threshold: f32,
    pub ptt_noise_gate: f32,
    pub vad_backend: VadBackendOption,
    pub silence_duration_ms: u32,
    pub speech_onset_ms: u32,
    pub max_speech_duration_s: u32,
}

impl Default for VadSettings {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_VAD_THRESHOLD,
            ptt_noise_gate: DEFAULT_VAD_PTT_NOISE_GATE,
            vad_backend: VadBackendOption::SileroVad,
            silence_duration_ms: DEFAULT_VAD_SILENCE_DURATION_MS,
            speech_onset_ms: DEFAULT_VAD_SPEECH_ONSET_MS,
            max_speech_duration_s: DEFAULT_VAD_MAX_SPEECH_DURATION_S,
        }
    }
}
