use std::env::var;

use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use crate::{
    core::settings::GeminiRealtimeConfig,
    services::{
        llm::CanonicalToolDefinition,
        realtime::{
            transport::{WsReader, WsWriter},
            GEMINI_DEFAULT_WS_URL_BASE, WS_HANDSHAKE_TIMEOUT,
        },
    },
};

pub(super) fn build_url(api_key: &str) -> String {
    if let Ok(override_url) = var("GEMINI_LIVE_ENDPOINT_OVERRIDE") {
        format!("{}/?key={}", override_url.trim_end_matches('/'), api_key)
    } else {
        format!("{}?key={}", GEMINI_DEFAULT_WS_URL_BASE, api_key)
    }
}

pub(super) async fn perform_handshake(
    url: &str,
    model: &str,
    config: &GeminiRealtimeConfig,
    system_prompt: &str,
    tools: &[CanonicalToolDefinition],
    is_ptt: bool,
    resume_handle: Option<&str>,
) -> Result<(WsWriter, WsReader)> {
    let redacted = url
        .find("key=")
        .map(|p| {
            format!(
                "{}key={}...",
                &url[..p + 4],
                &url[p + 4..].get(..8).unwrap_or("")
            )
        })
        .unwrap_or_else(|| url.to_string());
    log::info!("[GeminiLive] Connecting: {}", redacted);

    let (ws_stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .map_err(|e| anyhow!("WebSocket connection failed: {:?}", e))?;

    let (mut ws_write, mut ws_read) = ws_stream.split();

    let setup = build_setup_frame(model, config, system_prompt, tools, is_ptt, resume_handle);
    ws_write
        .send(Message::Text(setup.to_string().into()))
        .await
        .map_err(|e| anyhow!("Failed to send setup frame: {:?}", e))?;

    log::info!("[GeminiLive] Setup frame sent. Waiting for setupComplete...");

    let result = tokio::time::timeout(WS_HANDSHAKE_TIMEOUT, async {
        while let Some(res) = ws_read.next().await {
            match res {
                Ok(Message::Text(text)) => {
                    let val: serde_json::Value = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(e) => {
                            log::warn!(
                                "[GeminiLive] Invalid JSON during handshake: {} (raw: {})",
                                e,
                                text
                            );
                            continue;
                        }
                    };
                    if val.get("setupComplete").is_some() {
                        return Ok(());
                    }
                    if let Some(err) = val.get("error") {
                        return Err(anyhow!("Gemini setup error: {:?}", err));
                    }
                }
                Ok(Message::Binary(bytes)) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let val: serde_json::Value = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(e) => {
                            log::warn!(
                                "[GeminiLive] Invalid JSON in binary frame during handshake: {}",
                                e
                            );
                            continue;
                        }
                    };
                    if val.get("setupComplete").is_some() {
                        return Ok(());
                    }
                    if let Some(err) = val.get("error") {
                        return Err(anyhow!("Gemini setup error (binary): {:?}", err));
                    }
                }
                Ok(msg) => return Err(anyhow!("Unexpected message during setup: {:?}", msg)),
                Err(e) => return Err(anyhow!("WebSocket error during handshake: {:?}", e)),
            }
        }
        Err(anyhow!("Stream terminated before setupComplete"))
    })
    .await;

    match result {
        Ok(Ok(())) => {
            log::info!("[GeminiLive] Handshake complete.");
            Ok((ws_write, ws_read))
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(anyhow!(
            "Handshake timed out after {} s",
            WS_HANDSHAKE_TIMEOUT.as_secs()
        )),
    }
}

/// Builds the Gemini Live setup JSON frame.
pub(super) fn build_setup_frame(
    model: &str,
    config: &GeminiRealtimeConfig,
    system_prompt: &str,
    tools: &[CanonicalToolDefinition],
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

    let mut tool_list = Vec::new();
    if config.enable_web_search {
        tool_list.push(serde_json::json!({ "googleSearchRetrieval": {} }));
    }
    if !tools.is_empty() {
        let function_declarations: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                })
            })
            .collect();
        tool_list.push(serde_json::json!({
            "functionDeclarations": function_declarations
        }));
    }
    if !tool_list.is_empty() {
        frame["setup"]["tools"] = serde_json::Value::Array(tool_list);
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

pub(super) fn encode_tool_response(
    id: &str,
    name: &str,
    result: &serde_json::Value,
) -> String {
    serde_json::json!({
        "toolResponse": {
            "functionResponses": [
                {
                    "id": id,
                    "name": name,
                    "response": {
                        "output": result
                    }
                }
            ]
        }
    })
    .to_string()
}

pub(super) fn encode_text_turn(text: &str) -> String {
    serde_json::json!({
        "clientContent": {
            "turns": [
                {
                    "role": "user",
                    "parts": [{ "text": text }]
                }
            ],
            "turnComplete": true
        }
    })
    .to_string()
}
