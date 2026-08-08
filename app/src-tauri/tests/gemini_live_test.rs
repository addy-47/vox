//! ============================================================================
//! gemini_live_test.rs — Gemini Live S2S WebSockets Streaming Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : Realtime Voice Providers (`vox_lib::services::realtime`)
//! Prerequisites: Spawns local mock WebSocket server
//! Execution    : cargo test --test gemini_live_test
//! ============================================================================

use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::GeminiRealtimeConfig;
use vox_lib::services::realtime::{
    providers::gemini_live::GeminiLiveProvider, RealtimeVoiceProvider,
};

#[tokio::test(flavor = "multi_thread")]
async fn test_gemini_live_handshake_and_bidirectional_flow() {
    // 1. Spin up a local mock Gemini Live WebSocket server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("ws://127.0.0.1:{}", port);

    // Track events on the server side
    let server_received_setup = Arc::new(Mutex::new(false));
    let server_received_audio = Arc::new(Mutex::new(false));
    let server_received_setup_clone = server_received_setup.clone();
    let server_received_audio_clone = server_received_audio.clone();

    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.unwrap();
            let (mut ws_write, mut ws_read) = ws_stream.split();

            // First message must be setup
            if let Some(Ok(Message::Text(text))) = ws_read.next().await {
                let setup_val: serde_json::Value = serde_json::from_str(&text).unwrap();
                if setup_val.get("setup").is_some() {
                    *server_received_setup_clone.lock().unwrap() = true;

                    // Send setupComplete back
                    let setup_complete = serde_json::json!({
                        "setupComplete": {}
                    })
                    .to_string();
                    ws_write
                        .send(Message::Text(setup_complete.into()))
                        .await
                        .unwrap();
                }
            }

            // Next, handle realtimeInput audio stream and respond with mock S2S messages
            while let Some(Ok(msg)) = ws_read.next().await {
                if let Message::Text(text) = msg {
                    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
                    if let Some(realtime_input) = val.get("realtimeInput") {
                        if let Some(audio) = realtime_input.get("audio") {
                            if let Some(data) = audio.get("data").and_then(|d| d.as_str()) {
                                if !data.is_empty() {
                                    *server_received_audio_clone.lock().unwrap() = true;

                                    // Send user input transcription (ASR)
                                    let input_tx_msg = serde_json::json!({
                                        "serverContent": {
                                            "inputTranscription": {
                                                "text": "testing speech"
                                            }
                                        }
                                    })
                                    .to_string();
                                    ws_write
                                        .send(Message::Text(input_tx_msg.into()))
                                        .await
                                        .unwrap();

                                    // Send mock assistant audio response (24kHz, 16-bit PCM little-endian)
                                    // We send 480 samples of 42i16 = 960 bytes
                                    let mock_pcm = vec![42i16; 480];
                                    let mock_bytes = unsafe {
                                        std::slice::from_raw_parts(
                                            mock_pcm.as_ptr() as *const u8,
                                            mock_pcm.len() * 2,
                                        )
                                    };
                                    let mock_b64 = base64::Engine::encode(
                                        &base64::prelude::BASE64_STANDARD,
                                        mock_bytes,
                                    );

                                    let audio_msg = serde_json::json!({
                                        "serverContent": {
                                            "modelTurn": {
                                                "parts": [
                                                    {
                                                        "inlineData": {
                                                            "mimeType": "audio/pcm;rate=24000",
                                                            "data": mock_b64
                                                        }
                                                    }
                                                ]
                                            }
                                        }
                                    })
                                    .to_string();
                                    ws_write
                                        .send(Message::Text(audio_msg.into()))
                                        .await
                                        .unwrap();

                                    // Send assistant output transcription (TTS Text)
                                    let output_tx_msg = serde_json::json!({
                                        "serverContent": {
                                            "outputTranscription": {
                                                "text": "hello"
                                            }
                                        }
                                    })
                                    .to_string();
                                    ws_write
                                        .send(Message::Text(output_tx_msg.into()))
                                        .await
                                        .unwrap();

                                    // Send turnComplete
                                    let turn_complete_msg = serde_json::json!({
                                        "serverContent": {
                                            "turnComplete": true
                                        }
                                    })
                                    .to_string();
                                    ws_write
                                        .send(Message::Text(turn_complete_msg.into()))
                                        .await
                                        .unwrap();
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    // 2. Configure GeminiLiveProvider with Endpoint Override
    std::env::set_var("GEMINI_LIVE_ENDPOINT_OVERRIDE", &server_url);

    let config = GeminiRealtimeConfig {
        api_key: "test_key".to_string(),
        model: "gemini-3.1-flash-live-preview".to_string(),
        voice_name: "Aoede".to_string(),
        language_code: "en-US".to_string(),
        temperature: 0.2,
        enable_web_search: false,
        resume_handle: None,
    };

    let provider = GeminiLiveProvider::new(
        config,
        "you are helpful".to_string(),
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );

    // Channels to receive output from the provider session
    let (playback_tx, mut playback_rx) = tokio::sync::mpsc::channel::<Vec<i16>>(100);
    let (event_tx, event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    // 3. Connect (This executes the WebSocket handshake and setup configuration)
    let session = provider
        .connect(
            vox_lib::core::settings::InteractionMode::Passive,
            playback_tx,
            event_tx,
        )
        .unwrap();

    // Verify setup handshake on the mock server
    assert!(
        *server_received_setup.lock().unwrap(),
        "Server did not receive setup configuration"
    );

    // 4. Send client audio
    let client_pcm = vec![123i16; 320]; // 16kHz raw PCM chunk
    session.send_audio(&client_pcm).unwrap();

    // Give some time for background threads to exchange messages
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify audio reached the mock server
    assert!(
        *server_received_audio.lock().unwrap(),
        "Server did not receive client audio stream"
    );

    // 5. Gather received events from the provider connection
    let mut events = Vec::new();
    while let Ok(evt) = event_rx.try_recv() {
        events.push(evt);
    }

    // Verify events list
    let mut has_transcript_final = false;
    let mut has_llm_token = false;
    let mut has_llm_finished = false;

    for evt in &events {
        match evt {
            VoxEvent::TranscriptFinal { text, .. } => {
                assert_eq!(text, "testing speech");
                has_transcript_final = true;
            }
            VoxEvent::LlmToken { token, .. } => {
                if token == "hello" {
                    has_llm_token = true;
                }
            }
            VoxEvent::LlmFinished { .. } => {
                has_llm_finished = true;
            }
            _ => {}
        }
    }

    assert!(has_transcript_final, "Missing TranscriptFinal event");
    assert!(has_llm_token, "Missing LlmToken event");
    assert!(has_llm_finished, "Missing LlmFinished event");

    // Verify decoded output audio reached the playback bridge
    let mut played_chunks = Vec::new();
    while let Ok(pcm_chunk) = playback_rx.try_recv() {
        played_chunks.push(pcm_chunk);
    }

    assert_eq!(
        played_chunks.len(),
        1,
        "Expected exactly one audio response chunk"
    );
    assert_eq!(played_chunks[0].len(), 480);
    assert_eq!(played_chunks[0][0], 42i16);

    // Clean up
    session.disconnect().unwrap();
    std::env::remove_var("GEMINI_LIVE_ENDPOINT_OVERRIDE");
}
