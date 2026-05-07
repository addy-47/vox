use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use vox_ui_lib::services::llm::LlmWorker;
use vox_ui_lib::services::tts::TtsEngine;
use vox_ui_lib::services::playback::PlaybackEngine;
use vox_ui_lib::core::events::VoxEvent;

#[tokio::test]
async fn test_full_pipeline_responses() {
    let assets_dir = Path::new("assets");
    let model_path = assets_dir.join("gemma4").join("google_gemma-4-E2B-it-IQ2_M.gguf");
    let tts_dir = assets_dir.join("kokoro");

    println!("\n[VERIFICATION] STARTING GEMMA 4 INTEGRATION TEST");
    
    let (event_tx, mut event_rx) = mpsc::channel(100);
    let cancel = Arc::new(AtomicBool::new(false));
    let playback_active = Arc::new(AtomicBool::new(false));

    println!("[VERIFICATION] Loading TTS...");
    let tts = Arc::new(std::sync::Mutex::new(TtsEngine::new(&tts_dir).expect("Failed to load TTS")));

    println!("[VERIFICATION] Loading Playback...");
    let playback = PlaybackEngine::new(playback_active.clone(), cancel.clone())
        .expect("Failed to load Playback");

    println!("[VERIFICATION] Loading LLM (Gemma 4)...");
    let worker = Arc::new(LlmWorker::new(&model_path, 2048, 4).expect("Failed to load LLM"));

    // Helper to run a case
    let run_case = |prompt: String, session_id: u32, _voice_sid: i32| {
        let worker = Arc::clone(&worker);
        let tx = event_tx.clone();
        let cancel = Arc::clone(&cancel);
        
        println!("\n[USER]: {}", prompt);
        tokio::task::spawn_blocking(move || {
            worker.generate(&prompt, session_id, &cancel, &tx).expect("LLM Gen failed");
        });
    };

    // ── CASE 1: ENGLISH ──────────────────────────────────────────────────────
    println!("\n--- CASE 1: ENGLISH (Long Response) ---");
    run_case("Tell me a detailed story about a robot who discovered music, in about 5-6 sentences.".to_string(), 1, 0);

    let mut full_response = String::new();
    let mut thinking = false;
    print!("[VOX]: ");
    while let Some(event) = event_rx.recv().await {
        match event {
            VoxEvent::LlmToken { token, .. } => {
                // Mimic thinking detection from pipeline.rs
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

    println!("[TTS] Synthesizing English response (sid=0)...");
    let tts_cancel = Arc::clone(&cancel);
    let tts_tx = event_tx.clone();
    let full_response_clone = full_response.clone();
    let tts_inner = Arc::clone(&tts);
    
    // We must run TTS synthesis in a blocking task because it uses blocking_send
    // which is not allowed on a tokio worker thread (runtime thread).
    tokio::task::spawn_blocking(move || {
        let mut tts_lock = tts_inner.lock().unwrap();
        tts_lock.synthesize_chunk(&full_response_clone, 0, 1, tts_cancel, tts_tx)
            .expect("English TTS failed");
    }).await.expect("TTS task panicked");

    // Playback loop for this case
    while let Some(event) = event_rx.recv().await {
        match event {
            VoxEvent::TtsChunk { samples, .. } => {
                playback.ingest_chunk(&samples);
            }
            VoxEvent::TtsFinished { .. } => break,
            _ => {}
        }
    }
    
    println!("[Playback] Waiting for English audio to finish...");
    while !playback.is_idle() {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // ── CASE 2: HINDI ────────────────────────────────────────────────────────
    println!("\n--- CASE 2: HINDI (Long Response) ---");
    run_case("संगीत खोजने वाले एक रोबोट के बारे में 5-6 वाक्यों में एक विस्तृत कहानी सुनाएं।".to_string(), 2, 10);

    let mut full_response_hi = String::new();
    let mut thinking_hi = false;
    print!("[VOX]: ");
    while let Some(event) = event_rx.recv().await {
        match event {
            VoxEvent::LlmToken { token, .. } => {
                if token.contains("<|channel>thought") { thinking_hi = true; continue; }
                if token.contains("<channel|>") { thinking_hi = false; continue; }
                if thinking_hi { continue; }

                print!("{}", token);
                full_response_hi.push_str(&token);
            }
            VoxEvent::LlmFinished { .. } => {
                println!("\n[LLM] Finished.");
                break;
            }
            _ => {}
        }
    }

    println!("[TTS] Synthesizing Hindi response (sid=10)...");
    let tts_cancel_hi = Arc::clone(&cancel);
    let tts_tx_hi = event_tx.clone();
    let full_response_hi_clone = full_response_hi.clone();
    let tts_inner_hi = Arc::clone(&tts);

    tokio::task::spawn_blocking(move || {
        let mut tts_lock = tts_inner_hi.lock().unwrap();
        tts_lock.synthesize_chunk(&full_response_hi_clone, 10, 2, tts_cancel_hi, tts_tx_hi)
            .expect("Hindi TTS failed");
    }).await.expect("TTS task panicked");

    // Playback loop
    while let Some(event) = event_rx.recv().await {
        match event {
            VoxEvent::TtsChunk { samples, .. } => {
                playback.ingest_chunk(&samples);
            }
            VoxEvent::TtsFinished { .. } => break,
            _ => {}
        }
    }

    println!("[Playback] Waiting for Hindi audio to finish...");
    while !playback.is_idle() {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
        
    println!("\n[VERIFICATION] TEST COMPLETE");
}
