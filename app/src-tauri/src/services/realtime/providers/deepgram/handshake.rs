use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use super::protocol::build_settings_frame;
use crate::{
    core::settings::DeepgramVoiceAgentConfig,
    services::realtime::{
        transport::{WsReader, WsWriter},
        WS_HANDSHAKE_TIMEOUT,
    },
};

pub(super) async fn perform_handshake(
    url: &str,
    api_key: &str,
    config: &DeepgramVoiceAgentConfig,
    system_prompt: &str,
) -> Result<(WsWriter, WsReader)> {
    log::info!(
        "[DeepgramVoiceAgent] Connecting to Deepgram Voice Agent WebSocket: {}",
        url
    );

    let mut request =
        tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(url)?;
    request.headers_mut().insert(
        "Authorization",
        tokio_tungstenite::tungstenite::http::HeaderValue::from_str(&format!("Token {}", api_key))?,
    );

    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| anyhow!("Deepgram WebSocket connection failed: {:?}", e))?;

    let (mut ws_write, mut ws_read) = ws_stream.split();
    log::info!(
        "[DeepgramVoiceAgent] WebSocket connection established. Sending Settings config frame."
    );

    let settings_json = build_settings_frame(config, system_prompt);

    ws_write
        .send(Message::Text(settings_json.to_string().into()))
        .await
        .map_err(|e| anyhow!("Failed to send Settings JSON frame: {:?}", e))?;

    log::info!(
        "[DeepgramVoiceAgent] Settings frame sent. Waiting for Welcome and SettingsApplied..."
    );

    let mut welcome_received = false;
    let mut settings_applied_received = false;

    let handshake_timeout = tokio::time::timeout(WS_HANDSHAKE_TIMEOUT, async {
        while let Some(res) = ws_read.next().await {
            match res {
                Ok(Message::Text(text)) => {
                    let val: serde_json::Value = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(e) => {
                            log::warn!(
                                "[DeepgramVoiceAgent] Invalid JSON during handshake: {} (raw: {})",
                                e,
                                text
                            );
                            continue;
                        }
                    };
                    if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
                        if msg_type == "Welcome" {
                            log::info!("[DeepgramVoiceAgent] Received Welcome event.");
                            welcome_received = true;
                        } else if msg_type == "SettingsApplied" {
                            log::info!("[DeepgramVoiceAgent] Received SettingsApplied event.");
                            settings_applied_received = true;
                        } else if msg_type == "Error" || msg_type == "Warning" {
                            log::error!(
                                "[DeepgramVoiceAgent] Server error/warning during handshake: {:?}",
                                val
                            );
                            return Err(anyhow!("Deepgram error during handshake: {:?}", val));
                        }

                        if welcome_received && settings_applied_received {
                            return Ok(());
                        }
                    }
                }
                Ok(msg) => {
                    log::debug!(
                        "[DeepgramVoiceAgent] Received non-text message during handshake: {:?}",
                        msg
                    );
                }
                Err(e) => {
                    return Err(anyhow!("WebSocket error during handshake: {:?}", e));
                }
            }
        }
        Err(anyhow!(
            "WebSocket stream terminated before handshake complete"
        ))
    })
    .await;

    match handshake_timeout {
        Ok(Ok(())) => {
            log::info!("[DeepgramVoiceAgent] Handshake completed successfully.");
            Ok((ws_write, ws_read))
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err(anyhow!(
            "Handshake timed out after {} seconds",
            WS_HANDSHAKE_TIMEOUT.as_secs()
        )),
    }
}
