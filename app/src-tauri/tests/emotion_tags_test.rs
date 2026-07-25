//! ============================================================================
//! emotion_tags_test.rs — Supertonic 3 TTS Emotion Tag Processing Integration Test
//! ============================================================================
//! Category     : Integration Test
//! Component    : TTS Engine (`vox_lib::services::tts`)
//! Prerequisites: Local Supertonic model at `~/.vox/models/tts/supertonic-3`
//! Execution    : cargo test --test emotion_tags_test
//! ============================================================================

//! Test whether Supertonic 3 TTS actually processes emotion tags (<laugh>, <breath>, <sigh>).
//!
//! Synthesizes tagged and plain variants of the same text, saves both as WAV files,
//! and compares the audio output to determine if tags produce detectable differences.
//!
//! Run: cargo run --release --bin test-emotion-tags
//! Output: outputs/emotion_tag_test/ (both WAVs + analysis report)

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;

fn model_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Could not find home directory");
    home.join(".vox")
        .join("models")
        .join("tts")
        .join("supertonic-3")
}

fn synthesize_text(
    engine: &mut vox_lib::services::tts::TtsEngine,
    text: &str,
    turn_id: u32,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();

    use vox_lib::services::tts::providers::TtsProvider as _;

    engine.synthesize_chunk(text, turn_id, cancel, tx)?;

    let mut all_samples = Vec::new();
    while let Ok(event) = rx.recv() {
        match event {
            vox_lib::core::events::VoxEvent::TtsChunk { samples, .. } => {
                all_samples.extend(samples);
            }
            vox_lib::core::events::VoxEvent::TtsFinished { .. } => break,
            _ => {}
        }
    }

    if all_samples.is_empty() {
        return Err("No audio samples produced".into());
    }

    Ok(all_samples)
}

fn save_wav(
    path: &PathBuf,
    samples: &[f32],
    sample_rate: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in samples {
        writer.write_sample((sample * 32767.0) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup paths
    let vox_root = dirs::home_dir()
        .expect("Could not find home directory")
        .join(".vox");
    vox_lib::utils::paths::init_with_root(vox_root);

    let dir = model_dir();

    // Verify model files exist
    let required_files = [
        "text_encoder.int8.onnx",
        "duration_predictor.int8.onnx",
        "vector_estimator.int8.onnx",
        "vocoder.int8.onnx",
        "tts.json",
        "unicode_indexer.bin",
        "voice.bin",
    ];
    for f in &required_files {
        let path = dir.join(f);
        if !path.exists() {
            eprintln!("ERROR: Required model file not found: {:?}", path);
            std::process::exit(1);
        }
    }

    println!("[Test Emotion Tags] Loading Supertonic TTS engine...");
    let mut engine = vox_lib::services::tts::TtsEngine::new(&dir, 0, 8, 1.05)
        .expect("Failed to load Supertonic engine");

    // Test pair: with and without <laugh>
    let plain_text = "That is very funny.";
    let tagged_text = "That is <laugh> very funny.";

    println!("\n=== Synthesizing PLAIN text ===");
    let plain_samples = synthesize_text(&mut engine, plain_text, 0)?;
    println!(
        "  {} samples ({:.2}s at 24kHz)",
        plain_samples.len(),
        plain_samples.len() as f32 / 24000.0
    );

    println!("\n=== Synthesizing TAGGED text ===");
    let tagged_samples = synthesize_text(&mut engine, tagged_text, 1)?;
    println!(
        "  {} samples ({:.2}s at 24kHz)",
        tagged_samples.len(),
        tagged_samples.len() as f32 / 24000.0
    );

    // Also test breath and sigh
    let breath_text = "Let me think about that <breath> okay.";
    println!("\n=== Synthesizing BREATH text ===");
    let breath_samples = synthesize_text(&mut engine, breath_text, 2)?;
    println!(
        "  {} samples ({:.2}s at 24kHz)",
        breath_samples.len(),
        breath_samples.len() as f32 / 24000.0
    );

    let sigh_text = "I don't know <sigh> maybe.";
    println!("\n=== Synthesizing SIGH text ===");
    let sigh_samples = synthesize_text(&mut engine, sigh_text, 3)?;
    println!(
        "  {} samples ({:.2}s at 24kHz)",
        sigh_samples.len(),
        sigh_samples.len() as f32 / 24000.0
    );

    // Save WAVs
    let output_dir = PathBuf::from("outputs/emotion_tag_test");
    fs::create_dir_all(&output_dir)?;

    save_wav(&output_dir.join("plain_funny.wav"), &plain_samples, 24000)?;
    save_wav(
        &output_dir.join("tagged_laugh_funny.wav"),
        &tagged_samples,
        24000,
    )?;
    save_wav(
        &output_dir.join("tagged_breath.wav"),
        &breath_samples,
        24000,
    )?;
    save_wav(&output_dir.join("tagged_sigh.wav"), &sigh_samples, 24000)?;

    println!("\n=== Analysis ===");

    // 1. Compare lengths — if tag adds non-verbal audio, tagged should be longer
    let len_ratio = tagged_samples.len() as f64 / plain_samples.len() as f64;
    if (tagged_samples.len() as isize - plain_samples.len() as isize).abs() < 100 {
        println!("⚠️  LENGTH: Plain and tagged have nearly identical sample counts — tags may be silently ignored.");
    } else {
        println!(
            "📏 LENGTH: Tagged is {:.2}x the plain length ({:.2}s vs {:.2}s)",
            len_ratio,
            tagged_samples.len() as f32 / 24000.0,
            plain_samples.len() as f32 / 24000.0
        );
    }

    // 2. Compare raw audio — identical samples = tags not processed
    if plain_samples == tagged_samples {
        println!("🔴 IDENTICAL: Plain and tagged audio are bit-for-bit identical. Emotion tags are NOT being processed.");
        println!("   The '<laugh>' tag was spoken literally or silently ignored.");
    } else {
        // Compute simple difference metric
        let min_len = plain_samples.len().min(tagged_samples.len());
        let diff_sum: f64 = plain_samples[..min_len]
            .iter()
            .zip(tagged_samples[..min_len].iter())
            .map(|(a, b)| (a - b).abs() as f64)
            .sum();
        let avg_diff = diff_sum / min_len as f64;
        let max_diff = plain_samples[..min_len]
            .iter()
            .zip(tagged_samples[..min_len].iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);

        println!("🟢 DIFFERENT: Audio differs (avg diff={:.6}, max diff={:.4}). Emotion tags MAY be working.",
            avg_diff, max_diff);

        if avg_diff > 0.01 {
            println!("   Large difference suggests prosodic change from tag.");
        } else if avg_diff > 0.001 {
            println!("   Moderate difference — possible subtle prosodic effect.");
        } else {
            println!("   Tiny difference — likely due to resampling drift or minor timing shift.");
        }
    }

    // 3. Check for literal "<laugh>" in the produced audio
    // We can't directly check the audio for the words "less than laugh greater than"
    // but we can listen to the WAVs. Print a note.
    println!("\n   ⚠️  Manual verification recommended:");
    println!("      Listen to: {:?}", output_dir.join("plain_funny.wav"));
    println!(
        "           vs:  {:?}",
        output_dir.join("tagged_laugh_funny.wav")
    );
    println!("      If you hear \"less than laugh greater than\" in the tagged version, tags are spoken literally.");
    println!("      If you hear a laughing prosody, tags work correctly.");

    // Summary
    println!("\n=== Summary ===");
    println!("  Plain text:     '{}'", plain_text);
    println!("  Tagged (<laugh>): '{}'", tagged_text);
    println!("  Plain samples:  {}", plain_samples.len());
    println!("  Tagged samples: {}", tagged_samples.len());
    println!("  All WAVs saved to: {:?}", output_dir);

    Ok(())
}
