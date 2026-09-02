use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use crate::core::settings::GeminiRealtimeConfig;
use crate::services::realtime::transport::{WsReader, WsWriter};
use crate::services::realtime::{GEMINI_DEFAULT_WS_URL_BASE, WS_HANDSHAKE_TIMEOUT};

use super::protocol::build_setup_frame;

pub(super) fn build_url(api_key: &str) -> String {
    if let Ok(override_url) = std::env::var("GEMINI_LIVE_ENDPOINT_OVERRIDE") {
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

    let setup = build_setup_frame(model, config, system_prompt, is_ptt, resume_handle);
    ws_write
        .send(Message::Text(setup.to_string().into()))
        .await
        .map_err(|e| anyhow!("Failed to send setup frame: {:?}", e))?;

    log::info!("[GeminiLive] Setup frame sent. Waiting for setupComplete...");

    let result = tokio::time::timeout(WS_HANDSHAKE_TIMEOUT, async {
        while let Some(res) = ws_read.next().await {
            match res {
                Ok(Message::Text(text)) => {
                    let val: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                    if val.get("setupComplete").is_some() {
                        return Ok(());
                    }
                    if let Some(err) = val.get("error") {
                        return Err(anyhow!("Gemini setup error: {:?}", err));
                    }
                }
                Ok(Message::Binary(bytes)) => {
                    let val: serde_json::Value =
                        serde_json::from_str(&String::from_utf8_lossy(&bytes)).unwrap_or_default();
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
