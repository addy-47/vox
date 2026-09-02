use crate::core::settings::DeepgramVoiceAgentConfig;
use crate::services::realtime::{DEFAULT_INPUT_SAMPLE_RATE, DEFAULT_OUTPUT_SAMPLE_RATE};

pub(super) fn build_settings_frame(
    config: &DeepgramVoiceAgentConfig,
    system_prompt: &str,
) -> serde_json::Value {
    let model = if config.model.is_empty() {
        "gpt-4o-mini"
    } else {
        &config.model
    };

    let (provider_type, model_name) =
        if model.starts_with("gpt-") || model.starts_with("o1") || model.starts_with("o3") {
            ("open_ai", model)
        } else if model.starts_with("claude-") {
            ("anthropic", model)
        } else if model.starts_with("gemini-") {
            ("google", model)
        } else {
            ("open_ai", model)
        };

    let voice_model = match config.voice.as_str() {
        "Aoede" => "aura-asteria-en",
        "Charon" => "aura-orpheus-en",
        "Fenrir" => "aura-zeus-en",
        "Kore" => "aura-stella-en",
        "Puck" => "aura-athena-en",
        other => other,
    };

    serde_json::json!({
        "type": "Settings",
        "audio": {
            "input": {
                "encoding": "linear16",
                "sample_rate": DEFAULT_INPUT_SAMPLE_RATE
            },
            "output": {
                "encoding": "linear16",
                "sample_rate": DEFAULT_OUTPUT_SAMPLE_RATE,
                "container": "none"
            }
        },
        "agent": {
            "listen": {
                "provider": {
                    "type": "deepgram",
                    "model": "flux-general-multi",
                    "version": "v2",
                    "eot_threshold": 0.5,
                    "eager_eot_threshold": 0.4
                }
            },
            "think": {
                "provider": {
                    "type": provider_type,
                    "model": model_name,
                    "temperature": config.temperature
                },
                "prompt": system_prompt
            },
            "speak": {
                "provider": {
                    "type": "deepgram",
                    "model": voice_model
                }
            }
        }
    })
}
