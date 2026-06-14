use std::io::Write;
use std::net::TcpListener;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;
use vox_lib::core::events::VoxEvent;
use vox_lib::services::llm::{EmbeddedProvider, LlmProvider, OpenAiCompatProvider};

// Helper to spawn a mock SSE HTTP server returning a custom body and status.
fn spawn_mock_server(status: u16, headers: String, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    std::thread::spawn(move || {
        for _ in 0..5 {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0; 1024];
                let _ = std::io::Read::read(&mut stream, &mut buf);

                let req_str = String::from_utf8_lossy(&buf);
                let (resp_status, resp_body, extra_headers) = if req_str.contains("GET /api/tags") {
                    ("404 Not Found", "", "Content-Length: 0\r\n".to_string())
                } else {
                    (
                        if status == 200 {
                            "200 OK"
                        } else {
                            "500 Internal Server Error"
                        },
                        body,
                        format!("Content-Length: {}\r\n", body.len()),
                    )
                };

                let response = format!(
                    "HTTP/1.1 {}\r\n{}{}\r\n{}",
                    resp_status, extra_headers, headers, resp_body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            } else {
                break;
            }
        }
    });

    format!("http://127.0.0.1:{}", port)
}

#[test]
fn test_embedded_provider_health_check() {
    let temp_dir = tempfile::tempdir().unwrap();
    let model_path = temp_dir.path().join("model.gguf");

    // Path doesn't exist
    let provider = EmbeddedProvider::new(&model_path, 2048, 4);
    assert!(provider.is_err()); // LlmWorker requires file to exist on init

    // Create the GGUF file
    std::fs::File::create(&model_path).unwrap();
}

#[test]
fn test_openai_compat_health_check() {
    // Healthy (200 OK)
    let url = spawn_mock_server(
        200,
        "Content-Length: 0\r\nConnection: close\r\n".to_string(),
        "",
    );
    let provider = OpenAiCompatProvider::new(&url, "test-model", None, None);
    assert!(provider.health_check());

    // Unhealthy (500 Error)
    let url = spawn_mock_server(
        500,
        "Content-Length: 0\r\nConnection: close\r\n".to_string(),
        "",
    );
    let provider = OpenAiCompatProvider::new(&url, "test-model", None, None);
    assert!(!provider.health_check());
}

#[test]
fn test_openai_compat_list_models() {
    let body = r#"{"data":[{"id":"model-a"},{"id":"model-b"}]}"#;
    let url = spawn_mock_server(
        200,
        format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        ),
        body,
    );

    let provider = OpenAiCompatProvider::new(&url, "test-model", None, None);
    let models = provider.list_models().unwrap();
    let expected = vec![
        vox_lib::core::settings::RemoteModelInfo {
            id: "model-a".to_string(),
            name: "model a".to_string(),
            size_bytes: None,
            quantization: None,
            family: None,
            provider_kind: "open_ai_compat".to_string(),
        },
        vox_lib::core::settings::RemoteModelInfo {
            id: "model-b".to_string(),
            name: "model b".to_string(),
            size_bytes: None,
            quantization: None,
            family: None,
            provider_kind: "open_ai_compat".to_string(),
        },
    ];
    assert_eq!(models, expected);
}

#[test]
fn test_openai_compat_generate() {
    let sse_body = "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\r\n\r\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\r\n\r\n\
                    data: [DONE]\r\n\r\n";
    let url = spawn_mock_server(
        200,
        "Content-Type: text/event-stream\r\nConnection: close\r\n".to_string(),
        sse_body,
    );

    let provider = OpenAiCompatProvider::new(&url, "test-model", None, None);
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();

    provider
        .generate("test prompt", "system prompt", 1, &cancel_flag, &tx)
        .unwrap();

    let mut tokens = Vec::new();
    let mut finished = false;

    while let Ok(event) = rx.recv_timeout(Duration::from_millis(500)) {
        match event {
            VoxEvent::LlmToken { token, .. } => tokens.push(token),
            VoxEvent::LlmFinished { .. } => {
                finished = true;
                break;
            }
            _ => {}
        }
    }

    assert_eq!(tokens, vec!["Hello".to_string(), " world".to_string()]);
    assert!(finished);
}

#[test]
fn test_embedded_provider_list_models_in_dir() {
    let temp_dir = tempfile::tempdir().unwrap();
    let llm_dir = temp_dir.path().join("llm");
    std::fs::create_dir_all(&llm_dir).unwrap();

    let model_a = llm_dir.join("gemma_4_reasoning_q4_k_m.gguf");
    let model_b = llm_dir.join("llama_3_2_q6_k.gguf");
    let non_model = llm_dir.join("not_a_model.txt");

    std::fs::File::create(&model_a).unwrap();
    std::fs::File::create(&model_b).unwrap();
    std::fs::File::create(&non_model).unwrap();

    let models = EmbeddedProvider::list_models_in_dir(&llm_dir).unwrap();
    assert_eq!(models.len(), 2);

    let gemma = models
        .iter()
        .find(|m| m.id == "gemma_4_reasoning_q4_k_m.gguf")
        .unwrap();
    assert_eq!(gemma.name, "gemma 4 reasoning q4 k m");
    assert_eq!(gemma.quantization, Some("Q4_K_M".to_string()));
    assert_eq!(gemma.family, Some("Gemma".to_string()));
    assert_eq!(gemma.provider_kind, "embedded");

    let llama = models
        .iter()
        .find(|m| m.id == "llama_3_2_q6_k.gguf")
        .unwrap();
    assert_eq!(llama.name, "llama 3 2 q6 k");
    assert_eq!(llama.quantization, Some("Q6_K".to_string()));
    assert_eq!(llama.family, Some("Llama".to_string()));
    assert_eq!(llama.provider_kind, "embedded");
}
