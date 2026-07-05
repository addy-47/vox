use vox_lib::core::settings::LlmProviderConfig;
use vox_lib::services::llm::CapabilityProbeEngine;

#[tokio::test]
async fn test_remote_ollama_capability_probe() {
    let base_url = "http://100.86.62.14:11434".to_string();
    let config = LlmProviderConfig::OpenAiCompat {
        base_url: base_url.clone(),
        model: "gemma4:e4b".to_string(),
        api_key: None,
        provider_name: Some("ollama".to_string()),
    };

    println!("[Test] Probing gemma4:e4b on remote Ollama server (100.86.62.14)...");
    let result = CapabilityProbeEngine::probe_capabilities(&config, Some("gemma4:e4b")).await;
    assert!(result.is_ok(), "Failed to probe gemma4:e4b: {:?}", result.err());

    let caps = result.unwrap();
    println!("[Test] gemma4:e4b Probed Capabilities: {:#?}", caps);

    assert_eq!(caps.model_id, "gemma4:e4b");
    assert!(caps.supports_tools, "gemma4:e4b should support tools");
    assert!(caps.supports_latin, "gemma4:e4b should support latin script");
    assert!(caps.context_window.unwrap_or(0) > 0, "Context window should be detected");
    assert!(caps.server_has_gpu, "Server at 100.86.62.14 should be detected as having GPU hardware");
    assert!(caps.is_gpu_accelerated, "gemma4:e4b should be offloaded to VRAM post-inference");
}

#[tokio::test]
async fn test_remote_ollama_llama3_capability_probe() {
    let base_url = "http://100.86.62.14:11434".to_string();
    let config = LlmProviderConfig::OpenAiCompat {
        base_url: base_url.clone(),
        model: "llama3.1:8b-instruct-q4_K_M".to_string(),
        api_key: None,
        provider_name: Some("ollama".to_string()),
    };

    println!("[Test] Probing llama3.1:8b on remote Ollama server (100.86.62.14)...");
    let result = CapabilityProbeEngine::probe_capabilities(&config, Some("llama3.1:8b-instruct-q4_K_M")).await;
    assert!(result.is_ok(), "Failed to probe llama3.1: {:?}", result.err());

    let caps = result.unwrap();
    println!("[Test] llama3.1 Probed Capabilities: {:#?}", caps);

    assert_eq!(caps.model_id, "llama3.1:8b-instruct-q4_K_M");
    assert!(caps.supports_tools, "llama3.1 should support tools");
    assert!(caps.supports_latin, "llama3.1 should support latin script");
    assert!(caps.server_has_gpu, "Server hardware should be detected");
}

#[tokio::test]
async fn test_cloud_gemini_capability_probe() {
    let mut gemini_key = std::env::var("GEMINI_API_KEY")
        .or_else(|_| std::env::var("GEMINI_TEST"))
        .unwrap_or_default();

    if gemini_key.is_empty() {
        let env_path = std::path::Path::new("/home/addy/projects/apps/vox/temp/.env");
        if env_path.exists() {
            if let Ok(content) = std::fs::read_to_string(env_path) {
                for line in content.lines() {
                    if line.starts_with("GEMINI_TEST=") {
                        let key = line.trim_start_matches("GEMINI_TEST=").trim_matches('"').trim();
                        if !key.is_empty() {
                            gemini_key = key.to_string();
                        }
                    } else if line.starts_with("GEMINI_API_KEY=") {
                        let key = line.trim_start_matches("GEMINI_API_KEY=").trim_matches('"').trim();
                        if !key.is_empty() {
                            gemini_key = key.to_string();
                        }
                    }
                }
            }
        }
    }

    if gemini_key.is_empty() {
        println!("[Test] Skipped test_cloud_gemini_capability_probe: No GEMINI_API_KEY or GEMINI_TEST key found.");
        return;
    }

    let config = LlmProviderConfig::OpenAiCompat {
        base_url: "https://generativelanguage.googleapis.com".to_string(),
        model: "gemini-1.5-flash".to_string(),
        api_key: Some(gemini_key),
        provider_name: Some("gemini".to_string()),
    };

    println!("[Test] Probing Cloud Gemini provider...");
    let result = CapabilityProbeEngine::probe_capabilities(&config, Some("gemini-1.5-flash")).await;
    assert!(result.is_ok(), "Failed to probe Cloud Gemini: {:?}", result.err());

    let caps = result.unwrap();
    println!("[Test] Cloud Gemini Probed Capabilities: {:#?}", caps);

    assert_eq!(caps.model_id, "gemini-1.5-flash");
    assert!(caps.supports_tools, "Gemini Cloud should support tools");
    assert!(caps.supports_devanagari, "Gemini Cloud should support Devanagari");
    assert_eq!(caps.context_window, Some(1_048_576), "Gemini 1.5 Flash context window should be 1M");
    assert!(caps.server_has_gpu, "Cloud APIs should have GPU hardware");
    assert!(caps.is_gpu_accelerated, "Cloud APIs should be GPU/TPU accelerated");
    assert_eq!(caps.gpu_status, "Cloud GPU/TPU Cluster");
}
