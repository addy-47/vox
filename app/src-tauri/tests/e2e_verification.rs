use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use vox_ui_lib::services::stt::SttEngine;
use vox_ui_lib::services::llm::LlmWorker;
use vox_ui_lib::services::tts::TtsEngine;
use vox_ui_lib::core::events::VoxEvent;
use sysinfo::{ProcessExt, System, SystemExt, Pid};
use std::time::Instant;
use hound;

struct Metrics {
    stt_load_time: f32,
    llm_load_time: f32,
    tts_load_time: f32,
    stt_inference_time: f32,
    llm_ttft: f32,
    llm_total_gen_time: f32,
    tts_total_synthesis_time: f32,
    peak_rss_mb: f32,
}

fn get_process_memory() -> f32 {
    let mut sys = System::new_all();
    sys.refresh_all();
    let pid = Pid::from(std::process::id() as usize);
    if let Some(process) = sys.process(pid) {
        return process.memory() as f32 / (1024.0 * 1024.0); // Returns MB from Bytes
    }
    0.0
}

#[test]
fn test_full_e2e_pipeline_with_metrics() {
    let mut metrics = Metrics {
        stt_load_time: 0.0,
        llm_load_time: 0.0,
        tts_load_time: 0.0,
        stt_inference_time: 0.0,
        llm_ttft: 0.0,
        llm_total_gen_time: 0.0,
        tts_total_synthesis_time: 0.0,
        peak_rss_mb: 0.0,
    };

    // 1. Setup Paths
    let assets_dir = Path::new("assets");
    let stt_model_dir = assets_dir.join("qwen3-asr");
    let llm_model_path = assets_dir.join("gemma4").join("google_gemma-4-E2B-it-Q4_K_M.gguf");
    let tts_en_dir = assets_dir.join("kokoro");
    let tts_hi_dir = assets_dir.join("piper_hi");
    let input_wav = assets_dir.join("qwen3-asr").join("test_wavs").join("input_10s.wav");
    let output_wav = "vox_e2e_output.wav";

    println!("\n[E2E] STARTING INSTRUMENTED PIPELINE VERIFICATION");
    let start_total = Instant::now();

    let cancel = Arc::new(AtomicBool::new(false));

    // 2. Initialize Engines with Timing
    println!("[E2E] Loading STT...");
    let now = Instant::now();
    let stt = SttEngine::new(&stt_model_dir).expect("Failed to load STT");
    metrics.stt_load_time = now.elapsed().as_secs_f32();
    metrics.peak_rss_mb = metrics.peak_rss_mb.max(get_process_memory());

    println!("[E2E] Loading LLM...");
    let now = Instant::now();
    let llm = Arc::new(LlmWorker::new(&llm_model_path, 2048, 4).expect("Failed to load LLM"));
    metrics.llm_load_time = now.elapsed().as_secs_f32();
    metrics.peak_rss_mb = metrics.peak_rss_mb.max(get_process_memory());

    println!("[E2E] Loading TTS...");
    let now = Instant::now();
    let mut tts = TtsEngine::new(&tts_en_dir, &tts_hi_dir).expect("Failed to load TTS");
    metrics.tts_load_time = now.elapsed().as_secs_f32();
    metrics.peak_rss_mb = metrics.peak_rss_mb.max(get_process_memory());

    let (event_tx, mut event_rx) = mpsc::channel(100);

    // 3. STT Inference
    println!("[E2E] Reading input WAV...");
    let mut reader = hound::WavReader::open(&input_wav).expect("Failed to open input WAV");
    let samples: Vec<f32> = reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();

    println!("[E2E] Transcribing...");
    let now = Instant::now();
    let transcript = stt.transcribe(&samples).expect("STT failed");
    metrics.stt_inference_time = now.elapsed().as_secs_f32();
    println!("[E2E] Transcript: {:?}", transcript);

    // 4. LLM Generation
    let prompt = format!("The user said: '{}'. Respond to this in Hindi using Devanagari script. Be extremely detailed and provide a very long response about Canada's education, jobs, and lifestyle.", transcript);
    let now = Instant::now();
    let llm_clone = llm.clone();
    let tx_llm = event_tx.clone();
    let cancel_llm = Arc::clone(&cancel);
    
    std::thread::spawn(move || {
        llm_clone.generate(&prompt, 1, &cancel_llm, &tx_llm).expect("LLM failed");
    });

    let mut full_llm_response = String::new();
    let mut first_token = true;
    while let Some(event) = event_rx.blocking_recv() {
        match event {
            VoxEvent::LlmToken { token, .. } => {
                if first_token {
                    metrics.llm_ttft = now.elapsed().as_secs_f32();
                    first_token = false;
                }
                full_llm_response.push_str(&token);
            }
            VoxEvent::LlmFinished { .. } => break,
            _ => {}
        }
    }
    metrics.llm_total_gen_time = now.elapsed().as_secs_f32();
    println!("[E2E] LLM Result: {:?}", full_llm_response);

    // 5. TTS Synthesis
    println!("[E2E] Synthesizing...");
    let chunks: Vec<String> = full_llm_response
        .split(|c| c == '.' || c == '!' || c == '?' || c == '।')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut all_audio_samples = Vec::new();
    let now = Instant::now();

    for (idx, chunk) in chunks.iter().enumerate() {
        let tx_tts = event_tx.clone();
        let cancel_tts = Arc::clone(&cancel);
        let chunk_clone = chunk.clone();
        let tts_session_id = 400 + idx as u32;

        let tts_arc = Arc::new(std::sync::Mutex::new(tts));
        let tts_for_thread = Arc::clone(&tts_arc);
        
        std::thread::spawn(move || {
            let mut tts_lock = tts_for_thread.lock().unwrap();
            tts_lock.synthesize_chunk(&chunk_clone, 0, tts_session_id, cancel_tts, tx_tts)
        });

        while let Some(event) = event_rx.blocking_recv() {
            match event {
                VoxEvent::TtsChunk { samples, .. } => {
                    all_audio_samples.extend(samples);
                }
                VoxEvent::TtsFinished { .. } => break,
                _ => {}
            }
        }
        tts = match Arc::try_unwrap(tts_arc) {
            Ok(mutex) => mutex.into_inner().expect("Mutex poisoned"),
            Err(_) => panic!("[E2E] Failed to recover TTS engine from Arc"),
        };
    }
    metrics.tts_total_synthesis_time = now.elapsed().as_secs_f32();
    metrics.peak_rss_mb = metrics.peak_rss_mb.max(get_process_memory());

    // 6. Loopback Verification (Can we understand our own audio?)
    println!("[E2E] Performing Loopback Verification...");
    let loopback_transcript = stt.transcribe(&all_audio_samples).unwrap_or_default();
    println!("[E2E] Loopback Transcript: {:?}", loopback_transcript);

    // 7. Save and Report
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 22050,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(output_wav, spec).unwrap();
    for sample in &all_audio_samples { writer.write_sample(*sample).unwrap(); }
    writer.finalize().unwrap();

    let total_time = start_total.elapsed().as_secs_f32();
    
    println!("\n==========================================");
    println!("           VOX PIPELINE METRICS           ");
    println!("==========================================");
    println!("STT Load:      {:.2}s", metrics.stt_load_time);
    println!("LLM Load:      {:.2}s", metrics.llm_load_time);
    println!("TTS Load:      {:.2}s", metrics.tts_load_time);
    println!("------------------------------------------");
    println!("STT Inference: {:.2}s (Audio: 10.0s)", metrics.stt_inference_time);
    println!("LLM TTFT:      {:.2}s", metrics.llm_ttft);
    println!("LLM Total:     {:.2}s", metrics.llm_total_gen_time);
    println!("TTS Total:     {:.2}s", metrics.tts_total_synthesis_time);
    println!("------------------------------------------");
    println!("PEAK RSS:      {:.2} MB / 5120.0 MB", metrics.peak_rss_mb);
    println!("TOTAL E2E:     {:.2}s", total_time);
    println!("==========================================");

    // Assertions for stability
    assert!(metrics.peak_rss_mb < 5120.0, "MEMORY LIMIT EXCEEDED");
    assert!(!all_audio_samples.is_empty(), "AUDIO GENERATION FAILED");
}
