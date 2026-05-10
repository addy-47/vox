use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use vox_lib::services::llm::LlmWorker;
use vox_lib::services::tts::TtsEngine;
use vox_lib::services::playback::PlaybackEngine;
use vox_lib::core::events::VoxEvent;

#[tokio::test]
async fn test_full_pipeline_responses() {
    let assets_dir = Path::new("assets");
    let model_path = assets_dir.join("gemma4").join("google_gemma-4-E2B-it-IQ2_M.gguf");
    let tts_en_dir = assets_dir.join("kokoro");
    let tts_hi_dir = assets_dir.join("piper_hi");

    println!("\n[VERIFICATION] STARTING GEMMA 4 INTEGRATION TEST");
    
    let (event_tx, mut event_rx) = mpsc::channel(100);
    let cancel = Arc::new(AtomicBool::new(false));
    let playback_active = Arc::new(AtomicBool::new(false));

    println!("[VERIFICATION] Loading Multi-Model TTS...");
    let tts = Arc::new(std::sync::Mutex::new(
        TtsEngine::new(&tts_en_dir, &tts_hi_dir).expect("Failed to load Multi-Model TTS")
    ));

    println!("[VERIFICATION] Loading Playback...");
    let playback = PlaybackEngine::new(playback_active.clone(), cancel.clone())
        .expect("Failed to load Playback");

    println!("[VERIFICATION] Loading LLM (Gemma 4)...");
    let worker = Arc::new(LlmWorker::new(&model_path, 2048, 4).expect("Failed to load LLM"));

    // Helper to run a case
    let run_case = |prompt: String, session_id: u32| {
        let worker = Arc::clone(&worker);
        let tx = event_tx.clone();
        let cancel = Arc::clone(&cancel);
        
        println!("\n[USER]: {}", prompt);
        tokio::task::spawn_blocking(move || {
            worker.generate(&prompt, session_id, &cancel, &tx).expect("LLM Gen failed");
        });
    };

    // ── CASE: HINDI SPACE FACTS (Devanagari) ──────────────────────────────
    println!("\n--- CASE: HINDI SPACE FACTS (Devanagari) ---");
    run_case("Tell me one interesting fact about space in Hindi using ONLY Devanagari script. Do not use English script. Keep it 2-3 sentences.".to_string(), 1);

    let mut full_response = String::new();
    let mut thinking = false;
    print!("[VOX]: ");
    
    while let Some(event) = event_rx.recv().await {
        match event {
            VoxEvent::LlmToken { token, .. } => {
                if token.contains("<|channel>thought") { thinking = true; continue; }
                if token.contains("<channel|>") { thinking = false; continue; }
                if thinking { continue; }

                print!("{}", token);
                full_response.push_str(&token);
            }
            VoxEvent::LlmFinished { .. } => {
                println!("\n[LLM] Finished.");
                break;
            }
            _ => {}
        }
    }

    println!("[TTS] Routing chunks to engine...");
    
    let chunks: Vec<String> = full_response
        .split(|c| c == '.' || c == '?' || c == '!' || c == '।' ) // Split on punctuation
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    for (idx, chunk) in chunks.iter().enumerate() {
        let tts_cancel = Arc::clone(&cancel);
        let tts_tx = event_tx.clone();
        let chunk_clone = chunk.clone();
        let tts_inner = Arc::clone(&tts);
        let session_id = 200 + idx as u32;

        // Detect language for logging
        let is_hi = chunk.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c));
        println!("[TTS] Routing chunk: \"{}\" -> {}", chunk, if is_hi { "PIPER (HINDI)" } else { "KOKORO (ENGLISH)" });
        
        tokio::task::spawn_blocking(move || {
            let mut tts_lock = tts_inner.lock().unwrap();
            tts_lock.synthesize_chunk(&chunk_clone, 0, session_id, tts_cancel, tts_tx)
                .expect("TTS Synthesis failed");
        }).await.expect("TTS task panicked");

        // Playback loop for this chunk
        while let Some(event) = event_rx.recv().await {
            match event {
                VoxEvent::TtsChunk { samples, .. } => {
                    playback.ingest_chunk(&samples);
                }
                VoxEvent::TtsFinished { .. } => break,
                _ => {}
            }
        }
    }
    
    println!("[Playback] Waiting for audio to finish...");
    while !playback.is_idle() {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    println!("\n[VERIFICATION] TEST COMPLETE");
}
