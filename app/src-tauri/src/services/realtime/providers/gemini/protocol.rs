use crate::core::settings::GeminiRealtimeConfig;

/// Builds the Gemini Live setup JSON frame.
pub(super) fn build_setup_frame(
    model: &str,
    config: &GeminiRealtimeConfig,
    system_prompt: &str,
    is_ptt: bool,
    resume_handle: Option<&str>,
) -> serde_json::Value {
    let formatted_model = if model.starts_with("models/") || model.starts_with("publishers/") {
        model.to_string()
    } else {
        format!("models/{}", model)
    };

    let voice = if config.voice_name.is_empty() {
        "Aoede"
    } else {
        &config.voice_name
    };
    let lang = if config.language_code.is_empty() {
        "en-US"
    } else {
        &config.language_code
    };

    let mut frame = serde_json::json!({
        "setup": {
            "model": formatted_model,
            "generationConfig": {
                "responseModalities": ["AUDIO"],
                "speechConfig": {
                    "voiceConfig": { "prebuiltVoiceConfig": { "voiceName": voice } },
                    "languageCode": lang
                },
                "temperature": config.temperature,
                "thinkingConfig": { "thinkingBudget": 0 }
            },
            "inputAudioTranscription": {},
            "outputAudioTranscription": {}
        }
    });

    if config.enable_web_search {
        frame["setup"]["tools"] = serde_json::json!([{ "googleSearchRetrieval": {} }]);
    }

    if !system_prompt.is_empty() {
        frame["setup"]["systemInstruction"] = serde_json::json!({
            "parts": [{ "text": system_prompt }]
        });
    }

    let activity = if is_ptt {
        serde_json::json!({ "disabled": true })
    } else {
        serde_json::json!({
            "disabled": false,
            "startOfSpeechSensitivity": "START_SENSITIVITY_HIGH",
            "endOfSpeechSensitivity": "END_SENSITIVITY_HIGH",
            "prefixPaddingMs": 50,
            "silenceDurationMs": 200
        })
    };
    frame["setup"]["realtimeInputConfig"] = serde_json::json!({
        "automaticActivityDetection": activity,
        "turnCoverage": "TURN_INCLUDES_ONLY_ACTIVITY"
    });

    if let Some(h) = resume_handle {
        frame["setup"]["sessionResumption"] = serde_json::json!({ "handle": h });
    }

    frame
}

pub(super) fn encode_activity_start() -> String {
    serde_json::json!({ "realtimeInput": { "activityStart": {} } }).to_string()
}

pub(super) fn encode_activity_end() -> String {
    serde_json::json!({ "realtimeInput": { "activityEnd": {} } }).to_string()
}
