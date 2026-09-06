//! ============================================================================
//! voice_clone.rs — Standalone Voice Cloning & Speaker Embedding Pre-baker CLI
//! ============================================================================
//! Category     : Utility Tool (Cargo Example)
//! Component    : Chatterbox TTS Voice Cloner (`chatterbox-rs`)
//! Prerequisites: Local audio sample files
//! Execution    : cargo run --example voice_clone -- --help
//! ============================================================================

//! Takes one or more reference audio files (WAV, MP3, FLAC, etc.),
//! pre-bakes speaker embeddings, and generates English TTS samples
//! using each cloned voice.
//!
//! # Usage
//! ```sh
//! voice_clone -v "madara=path/to/madara.wav" -v "pain=path/to/pain.mp3" \
//!             --output ./cloned_voices
//! ```
//!
//! # Output structure
//! ```
//! output_dir/
//! ├── madara/
//! │   ├── source.wav            # 24 kHz mono f32 reference (truncated to 30s)
//! │   ├── baked/                # Pre-baked .npy speaker embeddings
//! │   │   ├── ...
//! │   └── samples/
//! │       └── output_en.wav     # 24 kHz mono 16-bit English TTS
//! └── pain/ ...
//! ```

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use clap::Parser;
use vox_lib::services::audio::decode as audio_decode;

const DEFAULT_EN_PROMPT: &str = "Hello, this is a sample of my cloned voice. I hope you like it. \
     The quick brown fox jumps over the lazy dog.";

#[derive(Parser, Debug)]
#[command(
    name = "voice_clone",
    about = "Standalone Chatterbox voice cloning & TTS",
    long_about = "\
Clones voices from reference audio files using Chatterbox TTS.\n\
Each voice is pre-baked into speaker embeddings, then used to\n\
synthesize English TTS samples.\n\n\
Example:\n  voice_clone -v \"madara=./madara.wav\" -v \"pain=./pain.mp3\"\n\
  voice_clone -v \"name=file.wav\" --output ./voices --en-prompt \"Custom text\"\n\
  voice_clone -v \"name=file.mp3\" --model-dir /custom/path --keep-temp"
)]
struct Args {
    /// Voice name and file path (format: "name=path/to/file"). Can be specified multiple times.
    #[arg(short = 'v', long = "voice", required = true, value_name = "NAME=PATH")]
    voices: Vec<String>,

    /// Output directory for cloned voices.
    #[arg(short = 'o', long = "output", default_value = "./cloned_voices")]
    output: PathBuf,

    /// Custom English prompt for TTS sample.
    #[arg(long = "en-prompt", default_value = DEFAULT_EN_PROMPT)]
    en_prompt: String,

    /// Path to Chatterbox model directory (contains t3-q4_0.gguf and s3gen-f16.gguf).
    /// Auto-detects from Vox model paths if not specified.
    #[arg(long = "model-dir")]
    model_dir: Option<PathBuf>,

    /// Keep intermediate files (default: temp source.wav etc are kept anyway in structured output).
    #[arg(long = "keep-temp", hide = true)]
    _keep_temp: bool,

    /// Enable verbose logging.
    #[arg(short = 'l', long = "verbose")]
    verbose: bool,

    /// Number of CFM diffusion steps (2-10, default 10).
    #[arg(long = "quality", default_value = "10", value_parser = clap::value_parser!(u32).range(2..=10))]
    quality_steps: u32,

    /// Speed factor (0.7-2.0, default 1.0). Applied as time-stretch post-synthesis.
    #[arg(long = "speed", default_value = "1.0")]
    _speed: f32,

    /// Maximum reference audio duration in seconds (default 30, clamped 10-60).
    #[arg(long = "max-duration", default_value = "30")]
    max_duration: f32,

    /// Language code for synthesis (default: "en").
    #[arg(long = "lang", default_value = "en")]
    language: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialise logging
    if args.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    // Initialise Vox paths (needed by vox_lib::utils::paths::model_dir)
    let home = dirs::home_dir().expect("Could not find home directory");
    vox_lib::utils::paths::init_with_root(home.join(".vox"));

    // Resolve model directory
    let model_dir = resolve_model_dir(args.model_dir.as_ref())?;
    let t3_path = find_t3_model(&model_dir);
    let s3_path = find_s3_model(&model_dir);

    log::info!("Using T3 model:   {}", t3_path.display());
    log::info!("Using S3Gen model: {}", s3_path.display());

    // Validate model files exist
    if !t3_path.exists() {
        anyhow::bail!("T3 model not found at: {}", t3_path.display());
    }
    if !s3_path.exists() {
        anyhow::bail!("S3Gen model not found at: {}", s3_path.display());
    }

    // Parse voice entries: "name=path"
    let voice_entries = parse_voice_args(&args.voices)?;

    // Clamp max_duration to valid range
    let max_duration = args.max_duration.clamp(10.0, 60.0);
    if (max_duration - args.max_duration).abs() > 0.01 {
        log::warn!(
            "Clamped max-duration from {:.0}s to {:.0}s",
            args.max_duration,
            max_duration
        );
    }

    // Ensure output directory exists
    std::fs::create_dir_all(&args.output)
        .map_err(|e| anyhow::anyhow!("Failed to create output dir: {}", e))?;

    // Process each voice sequentially (ENGINE_INIT_MUTEX prevents concurrency)
    let total = voice_entries.len();
    let mut successes = 0;
    let mut failures = 0;

    for (idx, (name, source_path)) in voice_entries.iter().enumerate() {
        println!("\n[{}/{}] Processing voice: {}", idx + 1, total, name);
        println!("  Source: {}", source_path.display());

        match process_one_voice(
            name,
            source_path,
            &args.output,
            &t3_path,
            &s3_path,
            &args.language,
            args.quality_steps,
            args._speed,
            max_duration,
            &args.en_prompt,
        ) {
            Ok(()) => {
                println!("  ✅ Voice '{}' cloned successfully!", name);
                successes += 1;
            }
            Err(e) => {
                eprintln!("  ❌ Failed to clone '{}': {}", name, e);
                failures += 1;
            }
        }
    }

    // Summary
    println!("\n═══════════════════════════════════════");
    println!("  Total: {total} | Success: {successes} | Failed: {failures}");
    println!("  Output: {}", args.output.display());
    println!("═══════════════════════════════════════");

    if failures > 0 {
        std::process::exit(1);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn process_one_voice(
    name: &str,
    source_path: &Path,
    output_dir: &Path,
    t3_path: &Path,
    s3_path: &Path,
    language: &str,
    quality_steps: u32,
    _speed: f32,
    max_duration: f32,
    en_prompt: &str,
) -> anyhow::Result<()> {
    // Create output directories
    let voice_dir = output_dir.join(sanitise_name(name));
    let samples_dir = voice_dir.join("samples");
    let baked_dir = voice_dir.join("baked");

    std::fs::create_dir_all(&samples_dir)?;
    std::fs::create_dir_all(&baked_dir)?;

    // ── Step 1: Decode & validate ───────────────────────────────
    println!("  ① Decoding audio...");
    let decoded = audio_decode::decode_to_24khz_mono(source_path)
        .map_err(|e| anyhow::anyhow!("Failed to decode audio: {}", e))?;

    println!(
        "     Duration: {:.1}s, Sample rate: {} Hz",
        decoded.duration_secs, decoded.sample_rate
    );

    // Auto-stitch short clips by repeating them until they reach at least 30s.
    // Voice cloning quality improves with longer reference audio; repeating a
    // short clip is a common workaround and works well with chatterbox's voice encoder.
    let min_duration = 30.0;
    let final_audio = if decoded.duration_secs < min_duration {
        let repeats = (min_duration / decoded.duration_secs).ceil() as usize;
        let stitched_samples: Vec<f32> = decoded
            .samples
            .iter()
            .copied()
            .cycle()
            .take(decoded.samples.len() * repeats)
            .collect();
        let stitched_duration = stitched_samples.len() as f32 / 24000.0;
        println!(
            "     ⚠️  Clip is only {:.1}s — auto-stitching {repeats}x to {:.1}s for better cloning.",
            decoded.duration_secs, stitched_duration
        );
        audio_decode::DecodedAudio {
            samples: stitched_samples,
            sample_rate: 24000,
            duration_secs: stitched_duration,
        }
    } else {
        decoded
    };

    // Truncate to max_duration (only if longer than max)
    let final_audio = if final_audio.duration_secs > max_duration {
        println!(
            "     Truncating from {:.1}s to {:.1}s",
            final_audio.duration_secs, max_duration
        );
        audio_decode::truncate_to(final_audio, max_duration)
    } else {
        final_audio
    };

    // Save reference WAV (24 kHz mono f32)
    let source_wav_path = voice_dir.join("source.wav");
    audio_decode::write_wav_f32_raw(&source_wav_path, &final_audio.samples, 24000)
        .map_err(|e| anyhow::anyhow!("Failed to write source WAV: {}", e))?;
    println!("     Source WAV: {}", source_wav_path.display());

    // ── Step 2: Pre-bake speaker embeddings ─────────────────────
    println!("  ② Pre-baking speaker embeddings (this may take a while)...");
    let bake_start = Instant::now();

    {
        use chatterbox_rs::{Engine, EngineOptions};

        let engine = Engine::new(EngineOptions {
            t3_gguf_path: t3_path.to_string_lossy().into_owned(),
            s3gen_gguf_path: s3_path.to_string_lossy().into_owned(),
            reference_audio: source_wav_path.to_string_lossy().into_owned(),
            cfm_steps: quality_steps as i32,
            verbose: false,
            seed: 42,
            temperature: 0.8,
            ..Default::default()
        })
        .map_err(|e| anyhow::anyhow!("Failed to initialise bake engine: {}", e))?;

        engine
            .save_voice(&baked_dir)
            .map_err(|e| anyhow::anyhow!("Failed to save voice tensors: {}", e))?;

        // Engine drops here → ENGINE_INIT_MUTEX released
    }

    println!(
        "     Baked tensors: {} ({:.1}s)",
        baked_dir.display(),
        bake_start.elapsed().as_secs_f32()
    );

    // ── Step 3: Synthesize English sample ───────────────────────
    println!("  ③ Synthesizing English TTS sample...");
    let synth_start = Instant::now();

    let en_pcm = {
        use chatterbox_rs::{Engine, EngineOptions};

        let engine = Engine::new(EngineOptions {
            t3_gguf_path: t3_path.to_string_lossy().into_owned(),
            s3gen_gguf_path: s3_path.to_string_lossy().into_owned(),
            voice_dir: baked_dir.to_string_lossy().into_owned(),
            language: language.to_string(),
            cfm_steps: quality_steps as i32,
            verbose: false,
            seed: 42,
            temperature: 0.8,
            ..Default::default()
        })
        .map_err(|e| anyhow::anyhow!("Failed to initialise synthesis engine: {}", e))?;

        let result = engine
            .synthesize(en_prompt)
            .map_err(|e| anyhow::anyhow!("Synthesis failed: {}", e))?;

        result.pcm
        // Engine drops here
    };

    println!(
        "     Synthesized {:.1}s of audio in {:.1}s",
        en_pcm.len() as f32 / 24000.0,
        synth_start.elapsed().as_secs_f32()
    );

    // Write English sample as 16-bit WAV
    let en_out_path = samples_dir.join("output_en.wav");
    audio_decode::write_wav_f32(&en_out_path, &en_pcm, 24000)
        .map_err(|e| anyhow::anyhow!("Failed to write English WAV: {}", e))?;

    println!("     English sample: {}", en_out_path.display());

    // ── Summary ─────────────────────────────────────────────────
    let voice_size = dir_size(&voice_dir);
    println!(
        "     Voice size: {:.1} MB",
        voice_size as f64 / (1024.0 * 1024.0)
    );

    Ok(())
}

/// Parse "--voice name=path" arguments.
fn parse_voice_args(args: &[String]) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let mut entries = Vec::new();

    for arg in args {
        let eq_pos = arg.find('=').ok_or_else(|| {
            anyhow::anyhow!("Invalid voice format: '{}'. Expected 'name=path'.", arg)
        })?;

        let name = arg[..eq_pos].trim().to_string();
        let path = arg[eq_pos + 1..].trim().to_string();

        if name.is_empty() {
            anyhow::bail!("Voice name cannot be empty in '{}'", arg);
        }
        if path.is_empty() {
            anyhow::bail!("Voice path cannot be empty in '{}'", arg);
        }

        let path_buf = PathBuf::from(&path);
        if !path_buf.exists() {
            anyhow::bail!("Voice file not found: {}", path_buf.display());
        }

        entries.push((name, path_buf));
    }

    Ok(entries)
}

/// Resolve the Chatterbox model directory.
fn resolve_model_dir(custom: Option<&PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(dir) = custom {
        return Ok(dir.to_path_buf());
    }

    // Try Vox standard model path
    let vox_path = vox_lib::utils::paths::model_dir("tts").join("chatterbox");
    if vox_path.exists() {
        return Ok(vox_path);
    }

    // Try fallback path in /opt/vox-models
    let opt_path = PathBuf::from("/opt/vox-models/tts/chatterbox");
    if opt_path.exists() {
        return Ok(opt_path);
    }

    anyhow::bail!(
        "Chatterbox model directory not found. \
         Tried: {:?} and {:?}. Use --model-dir to specify a path.",
        vox_path,
        opt_path
    );
}

/// Find T3 model file (supports both naming conventions).
fn find_t3_model(dir: &Path) -> PathBuf {
    let candidates = [
        dir.join("t3-q4_0.gguf"),
        dir.join("chatterbox-t3-mtl-q4_0.gguf"),
        dir.join("t3-q4_0.gguf"), // check again in base dir
    ];
    for c in &candidates {
        if c.exists() {
            return c.clone();
        }
    }
    // Return the first candidate regardless (caller will check exists())
    dir.join("t3-q4_0.gguf")
}

/// Find S3Gen model file (supports both naming conventions).
fn find_s3_model(dir: &Path) -> PathBuf {
    let candidates = [
        dir.join("s3gen-f16.gguf"),
        dir.join("chatterbox-s3gen-mtl-f16.gguf"),
    ];
    for c in &candidates {
        if c.exists() {
            return c.clone();
        }
    }
    dir.join("s3gen-f16.gguf")
}

/// Sanitise a voice name for use as a directory name.
fn sanitise_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

/// Calculate the total size of a directory in bytes.
fn dir_size(path: &PathBuf) -> u64 {
    use std::fs;

    use walkdir::WalkDir;

    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| fs::metadata(e.path()).ok())
        .map(|m| m.len())
        .sum()
}
