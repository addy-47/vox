use crate::state::VoxSettings;
use std::path::PathBuf;

#[test]
fn test_settings_serialization() {
    let settings = VoxSettings {
        stt_model_dir: PathBuf::from("models/stt"),
        vad_model_path: PathBuf::from("models/vad.onnx"),
        noise_gate_threshold: 0.005,
        waveform_sensitivity: 0.5,
        waveform_speed: 0.5,
    };
    
    let json = serde_json::to_string(&settings).unwrap();
    let decoded: VoxSettings = serde_json::from_str(&json).unwrap();
    
    assert_eq!(decoded.stt_model_dir, settings.stt_model_dir);
    assert_eq!(decoded.noise_gate_threshold, settings.noise_gate_threshold);
}
