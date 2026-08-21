use std::fs;
use std::path::Path;
use vox_lib::core::settings::LlmProviderConfig;
use vox_lib::services::llm::CapabilityProbeEngine;

fn load_nvidia_key() -> Option<String> {
    if let Ok(key) = std::env::var("NVIDIA_API_KEY") {
        if !key.is_empty() {
            return Some(key);
        }
    }

    let env_paths = ["../../temp/.env", "../temp/.env", "temp/.env"];
    for p in env_paths {
        if Path::new(p).exists() {
            if let Ok(content) = fs::read_to_string(p) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("NVIDIA_API_KEY=") {
                        let val = trimmed.trim_start_matches("NVIDIA_API_KEY=").trim().trim_matches('"');
                        if !val.is_empty() {
                            return Some(val.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = env_logger::builder().filter_level(log::LevelFilter::Info).try_init();

    let api_key = match load_nvidia_key() {
        Some(k) => k,
        None => {
            eprintln!("[ERROR] Could not find NVIDIA_API_KEY in temp/.env or environment.");
            return Ok(());
        }
    };

    let model_id = "meta/llama-3.1-8b-instruct";
    let base_url = "https://integrate.api.nvidia.com/v1";

    println!("============================================================");
    println!("  NVIDIA NIM Live Capability & Token Validation Probe");
    println!("============================================================");
    println!("Endpoint: {}", base_url);
    println!("Model:    {}", model_id);
    println!("------------------------------------------------------------");

    let config = LlmProviderConfig::OpenAiCompat {
        base_url: base_url.to_string(),
        model: model_id.to_string(),
        api_key: Some(api_key),
        provider_name: Some("nvidia".to_string()),
    };

    println!("\n[1/3] Running CapabilityProbeEngine::probe_capabilities...");
    let start = std::time::Instant::now();
    match CapabilityProbeEngine::probe_capabilities(&config, Some(model_id)).await {
        Ok(caps) => {
            let elapsed = start.elapsed();
            println!(" Probe Succeeded in {:.2?}!", elapsed);
            println!("  - Model ID:           {}", caps.model_id);
            println!("  - Provider Kind:      {}", caps.provider_kind);
            println!("  - GPU Acceleration:   {} ({})", caps.is_gpu_accelerated, caps.gpu_status);
            println!("  - TTFT (Time to 1st): {:?} ms", caps.ttft_ms);
            println!("  - Throughput (TPS):   {:?} tokens/sec", caps.tps);
            println!("  - Structured Tools:   {}", caps.supports_tools);
            println!("  - Devanagari Script:  {}", caps.supports_devanagari);
            println!("  - Latin Script:       {}", caps.supports_latin);
            println!("  - Context Window:     {:?} (None = Endpoint Managed)", caps.context_window);
        }
        Err(e) => {
            println!("❌ Probe Failed: {}", e);
        }
    }

    println!("\n[2/3] Smoke Testing Normal Token Cap (2048 tokens)...");
    match CapabilityProbeEngine::validate_token_cap(&config, Some(model_id), 2048).await {
        Ok(None) => {
            println!(" Valid Cap: Server accepted 2048 tokens with no error.");
        }
        Ok(Some(ceiling)) => {
            println!("⚠️ Server suggested ceiling: {} tokens", ceiling);
        }
        Err(e) => {
            println!("❌ Error during valid token cap test: {}", e);
        }
    }

    println!("\n[3/3] Smoke Testing Massive Token Cap (500,000 tokens)...");
    match CapabilityProbeEngine::validate_token_cap(&config, Some(model_id), 500_000).await {
        Ok(None) => {
            println!(" Server accepted 500,000 tokens without 400 error.");
        }
        Ok(Some(ceiling)) => {
            println!(" Auto-Clamp Triggered! Server returned ceiling: {} tokens", ceiling);
        }
        Err(e) => {
            println!(" Server returned unparseable error: {}", e);
        }
    }

    println!("\n============================================================");
    println!("  Live Integration Test Complete!");
    println!("============================================================");
    Ok(())
}
