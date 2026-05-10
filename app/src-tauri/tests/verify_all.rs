use std::fs;
use std::path::PathBuf;
use vox_lib::services::stt::SttEngine;
use std::io::Write;

fn model_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/qwen3-asr")
}

fn load_wav(path: &PathBuf) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("Failed to open wav");
    let spec = reader.spec();
    
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect(),
    };

    let mono: Vec<f32> = if spec.channels == 2 {
        samples.chunks(2).map(|c| (c[0] + c[1]) / 2.0).collect()
    } else {
        samples
    };

    if spec.sample_rate != 16000 {
        let ratio = 16000.0 / spec.sample_rate as f32;
        let new_len = (mono.len() as f32 * ratio) as usize;
        (0..new_len).map(|i| {
            let src = i as f32 / ratio;
            let idx = src as usize;
            let frac = src - idx as f32;
            let a = mono.get(idx).copied().unwrap_or(0.0);
            let b = mono.get(idx + 1).copied().unwrap_or(0.0);
            a * (1.0 - frac) + b * frac
        }).collect()
    } else {
        mono
    }
}

#[test]
fn verify_all_wavs() {
    let _ = env_logger::builder().is_test(true).filter_level(log::LevelFilter::Info).try_init();
    let dir = model_dir();
    let wav_dir = dir.join("test_wavs");
    let output_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transcription_results.txt");
    
    let mut engine = SttEngine::new(&dir).expect("Failed to init engine");
    let mut output_file = fs::File::create(&output_path).expect("Failed to create output file");

    writeln!(output_file, "=== Transcription Verification Results ===").unwrap();
    writeln!(output_file, "Generated at: {:?}", std::time::SystemTime::now()).unwrap();
    writeln!(output_file, "------------------------------------------\n").unwrap();

    let mut entries: Vec<_> = fs::read_dir(wav_dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "wav"))
        .collect();
    
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let filename = entry.file_name().to_string_lossy().to_string();
        
        println!("Processing {}...", filename);
        let audio = load_wav(&path);
        
        // Transcribe (limit to 30s for safety/speed if needed, but here we process full if under 30s)
        // Qwen-ASR usually takes 30s max.
        let result = engine.transcribe(&audio, |_| {});
        
        match result {
            Ok(text) => {
                writeln!(output_file, "File: {}", filename).unwrap();
                writeln!(output_file, "Result: {}", text).unwrap();
                writeln!(output_file, "------------------------------------------").unwrap();
            }
            Err(e) => {
                writeln!(output_file, "File: {}", filename).unwrap();
                writeln!(output_file, "Error: {:?}", e).unwrap();
                writeln!(output_file, "------------------------------------------").unwrap();
            }
        }
    }

    println!("Verification complete. Results written to {:?}", output_path);
}
