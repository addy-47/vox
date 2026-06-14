use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use vox_lib::services::realtime::{
    audio_bridge::AudioBridge, RealtimeAudioConfig, RealtimeSession,
};

// A mock WebSocket-based session for testing
struct TestWssSession {
    tx: tokio::sync::mpsc::UnboundedSender<Message>,
}

impl RealtimeSession for TestWssSession {
    fn send_audio(&self, pcm: &[i16]) -> anyhow::Result<()> {
        let base64_audio = base64::Engine::encode(&base64::prelude::BASE64_STANDARD, unsafe {
            std::slice::from_raw_parts(pcm.as_ptr() as *const u8, pcm.len() * 2)
        });
        let msg = serde_json::json!({
            "audio": base64_audio
        })
        .to_string();

        self.tx
            .send(Message::Text(msg.into()))
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn cancel(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn disconnect(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn activity_start(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn activity_end(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_audio_bridge_and_websocket_roundtrip() {
    // 1. Spin up a local WebSocket server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_url = format!("ws://127.0.0.1:{}", port);

    let received_messages = Arc::new(Mutex::new(Vec::new()));
    let server_recv = received_messages.clone();

    // Spawn the server task
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            let ws_stream = accept_async(stream).await.unwrap();
            let (_, mut read) = ws_stream.split();
            while let Some(Ok(msg)) = read.next().await {
                if let Message::Text(txt) = msg {
                    let val: serde_json::Value = serde_json::from_str(&txt).unwrap();
                    if let Some(audio_b64) = val.get("audio").and_then(|v| v.as_str()) {
                        let decoded =
                            base64::Engine::decode(&base64::prelude::BASE64_STANDARD, audio_b64)
                                .unwrap();
                        let pcm: Vec<i16> = decoded
                            .chunks_exact(2)
                            .map(|c| i16::from_ne_bytes([c[0], c[1]]))
                            .collect();
                        server_recv.lock().unwrap().push(pcm);
                    }
                }
            }
        }
    });

    // 2. Connect client to the server
    let (ws_stream, _) = tokio_tungstenite::connect_async(server_url).await.unwrap();
    let (mut write, _) = ws_stream.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(_) = write.send(msg).await {
                break;
            }
        }
    });

    let session = Arc::new(TestWssSession { tx });

    // 3. Initialize AudioBridge
    let mut bridge = AudioBridge::new();
    let config = RealtimeAudioConfig {
        input_sample_rate: 16000,
        output_sample_rate: 16000,
        requires_input_resampling: false,
        requires_output_resampling: false,
    };

    let handle = tokio::runtime::Handle::current();
    bridge.start(session, config, &handle);

    // 4. Send 10 PCM chunks
    let chunk = vec![123i16; 320]; // 20ms of audio
    for _ in 0..10 {
        bridge.send_pcm(&chunk);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    // Give some time for background tasks to process and flush
    tokio::time::sleep(Duration::from_millis(200)).await;
    bridge.stop();

    // 5. Assert all 10 are received by the server
    let rec = received_messages.lock().unwrap();
    assert_eq!(rec.len(), 10);
    for pcm_chunk in rec.iter() {
        assert_eq!(pcm_chunk.len(), 320);
        assert_eq!(pcm_chunk[0], 123i16);
    }
}
