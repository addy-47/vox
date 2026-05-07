/// Integration test for user verification of LLM (English/Hindi) and TTS (English/Hindi).
/// 
/// This script:
/// 1. Initializes the Gemma 4 (GGUF) LLM.
/// 2. Asks an English question and a Hindi question.
/// 3. Captures the streaming tokens and prints them.
/// 4. Generates audio for both responses using Kokoro-82M.
/// 5. Saves the raw f32 samples to `output_en.pcm` and `output_hi.pcm`.
///
/// Run with:
///   cargo test --test user_verification -- --ignored --nocapture --test-threads=1

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc;
use vox_ui_lib::services::llm::{LlmWorker, LlmCommand};
use vox_ui_lib::services::tts::TtsEngine;
use vox_ui_lib::core::events::VoxEvent;

fn get_assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn gguf_path() -> PathBuf {
    get_assets_dir().join("gemma4/google_gemma-4-E2B-it-IQ2_M.gguf")
}

fn kokoro_dir() -> PathBuf {
    get_assets_dir().join("kokoro")
}

#[test]
#[ignore]
fn verify_llm_and_tts() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    println!("\n[1/4] Loading Gemma 4 Model...");
    let llm_path = gguf_path();
    let worker = LlmWorker::new(&llm_path, 1024, 4).expect("LLM load failed");
    
    let (llm_cmd_tx, llm_cmd_rx) = mpsc::channel(32);
    let (llm_event_tx, mut llm_event_rx) = mpsc::channel(64);
    
    let llm_handle = std::thread::spawn(move || {
        worker.run_loop(llm_cmd_rx, llm_event_tx);
    });

    println!("[2/4] Loading Kokoro TTS Model...");
    let mut tts = TtsEngine::new(&kokoro_dir()).expect("TTS load failed");

    // ── English Test ──────────────────────────────────────────────────────────
    println!("\n>>> TEST 1: English");
    let en_prompt = "Tell me a very short fun fact about space.";
    println!("Prompt: {}", en_prompt);
    
    let cancel = Arc::new(AtomicBool::new(false));
    llm_cmd_tx.blocking_send(LlmCommand::Generate {
        text: en_prompt.to_string(),
        session_id: 1,
        cancel_flag: cancel.clone(),
    }).unwrap();

    let mut en_response = String::new();
    rt.block_on(async {
        while let Some(event) = llm_event_rx.recv().await {
            match event {
                VoxEvent::LlmToken { token, .. } => {
                    print!("{}", token);
                    use std::io::Write;
                    std::io::stdout().flush().unwrap();
                    en_response.push_str(&token);
                }
                VoxEvent::LlmFinished { .. } => break,
                _ => {}
            }
        }
    });
    println!("\n(English Response Received)");

    println!("Synthesizing English Audio...");
    let (tts_tx, mut tts_rx) = mpsc::channel(128);
    let cancel_tts = Arc::new(AtomicBool::new(false));
    
    let en_response_clone = en_response.clone();
    let mut en_samples = Vec::new();
    
    tts.synthesize_chunk(&en_response, 1, cancel_tts, tts_tx).expect("TTS synthesis failed");
    
    rt.block_on(async {
        while let Some(event) = tts_rx.recv().await {
            match event {
                VoxEvent::TtsChunk { samples, .. } => {
                    en_samples.extend(samples);
                }
                VoxEvent::TtsFinished { .. } => break,
                _ => {}
            }
        }
    });
    
    let en_file = "output_en.pcm";
    let en_bytes: Vec<u8> = en_samples.iter().flat_map(|&f| f.to_le_bytes().to_vec()).collect();
    std::fs::write(en_file, en_bytes).unwrap();
    println!("Saved English audio to: {}", en_file);
    println!("Play with: ffplay -f f32le -ar 24000 -ac 1 {}", en_file);

    // ── Hindi Test ────────────────────────────────────────────────────────────
    println!("\n>>> TEST 2: Hindi");
    let hi_prompt = "भारत के बारे में एक छोटा सा रोचक तथ्य बताएं।";
    println!("Prompt: {}", hi_prompt);
    
    llm_cmd_tx.blocking_send(LlmCommand::Generate {
        text: hi_prompt.to_string(),
        session_id: 2,
        cancel_flag: cancel.clone(),
    }).unwrap();

    let mut hi_response = String::new();
    rt.block_on(async {
        while let Some(event) = llm_event_rx.recv().await {
            match event {
                VoxEvent::LlmToken { token, .. } => {
                    print!("{}", token);
                    use std::io::Write;
                    std::io::stdout().flush().unwrap();
                    hi_response.push_str(&token);
                }
                VoxEvent::LlmFinished { .. } => break,
                _ => {}
            }
        }
    });
    println!("\n(Hindi Response Received)");

    println!("Synthesizing Hindi Audio...");
    let (tts_hi_tx, mut tts_hi_rx) = mpsc::channel(128);
    let cancel_hi_tts = Arc::new(AtomicBool::new(false));
    
    let hi_response_clone = hi_response.clone();
    let mut hi_samples = Vec::new();
    
    tts.synthesize_chunk(&hi_response, 2, cancel_hi_tts, tts_hi_tx).expect("TTS synthesis failed");
    
    rt.block_on(async {
        while let Some(event) = tts_hi_rx.recv().await {
            match event {
                VoxEvent::TtsChunk { samples, .. } => {
                    hi_samples.extend(samples);
                }
                VoxEvent::TtsFinished { .. } => break,
                _ => {}
            }
        }
    });
    
    let hi_file = "output_hi.pcm";
    let hi_bytes: Vec<u8> = hi_samples.iter().flat_map(|&f| f.to_le_bytes().to_vec()).collect();
    std::fs::write(hi_file, hi_bytes).unwrap();
    println!("Saved Hindi audio to: {}", hi_file);
    println!("Play with: ffplay -f f32le -ar 24000 -ac 1 {}", hi_file);

    // Cleanup
    llm_cmd_tx.blocking_send(LlmCommand::Shutdown).unwrap();
    llm_handle.join().unwrap();
}
