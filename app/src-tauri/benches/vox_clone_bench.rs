//! ============================================================================
//! vox_clone_bench.rs — Chatterbox Voice Cloning Quality & Latency Benchmark
//! ============================================================================
//! Category     : Benchmark
//! Component    : Voice Cloning Engine (`chatterbox-rs`)
//! Prerequisites: Local pre-baked speaker embeddings in `temp/voice_corpus/`
//! Execution    : cargo test --bench vox_clone_bench
//! ============================================================================

/// For each voice in `temp/voice_corpus/cloned_voices/`, re-synthesizes the
/// same text and language that the source speaker is speaking, then saves the
/// output alongside the source for easy A/B comparison.
///
/// Usage:
///   cargo run --bin vox-clone-bench -- [--voices-dir PATH] [--quality-steps N]
///
/// The corpus is keyed on the voice name. Each voice needs:
///   <voices-dir>/<name>/source.wav   — the reference audio (24kHz f32 mono)
///   <voices-dir>/<name>/baked/       — pre-baked speaker tensors (from save_voice)
///
/// Output goes to <voices-dir>/<name>/samples/<lang>_<text_slug>.wav

use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use vox_lib::core::events::VoxEvent;
use vox_lib::services::tts::TtsProvider;

#[derive(Parser, Debug)]
#[command(name = "vox-clone-bench", about = "Voice cloning validation bench for Chatterbox")]
struct Args {
    /// Root directory containing cloned voice subdirectories.
    /// Defaults to <workspace>/temp/voice_corpus/cloned_voices
    #[arg(long)]
    voices_dir: Option<PathBuf>,

    /// Number of quality/diffusion steps for Chatterbox (default: 8)
    #[arg(long, default_value = "8")]
    quality_steps: usize,

    /// Only bench a specific voice name (runs all if omitted)
    #[arg(long)]
    voice: Option<String>,

    /// Override the synthesis text for all voices
    #[arg(long)]
    text: Option<String>,

    /// Override the language code for all voices (e.g. en, hi, ja)
    #[arg(long)]
    lang: Option<String>,
}

/// Corpus entry: maps a voice name to its reference text and language.
/// The text should match what the source speaker is actually saying.
struct CorpusEntry {
    name: &'static str,
    lang: &'static str,
    text: &'static str,
}

/// Hand-curated corpus of voices and their source utterances.
/// Fill in the actual spoken content from each source WAV.
fn corpus() -> Vec<CorpusEntry> {
    vec![
        CorpusEntry {
            name: "pain",
            lang: "en",
            // Pain (Nagato) from Naruto — speaks English dub lines
            text: "If you don't share someone's pain, you can never understand them.",
        },
        CorpusEntry {
            name: "madara",
            lang: "en",
            text: "Wake up to reality. Nothing ever goes as planned in this accursed world.",
        },
        CorpusEntry {
            name: "shreya",
            lang: "hi",
            // Shreya Ghoshal — Hindi singer; we transliterate via translit.rs
            // Text below is romanized so Chatterbox can handle it directly.
            // Actual spoken source is in Hindi.
            text: "तुझ में रब दिखता है यारा मैं क्या करूँ",
        },
        CorpusEntry {
            name: "hayami",
            lang: "ja",
            // Hayami Saori — Japanese voice actress
            // Japanese kana/romaji text works reasonably with [ja] prefix
            text: "watashi wa anata no koto ga suki desu",
        },
        CorpusEntry {
            name: "ellen",
            lang: "en",
            text: "Hello, I am Ellen. I speak in a serious, direct and confident manner.",
        },
        CorpusEntry {
            name: "juniper",
            lang: "en",
            text: "Hello, I am Juniper. I speak in a grounded and professional manner.",
        },
        CorpusEntry {
            name: "mark",
            lang: "en",
            text: "Hello, I am Mark. I enjoy natural conversations with people.",
        },
        CorpusEntry {
            name: "spuds",
            lang: "en",
            text: "Hello, I am Spuds Oxley. I am wise and approachable.",
        },
    ]
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();

    // Locate the chatterbox model directory
    let home = dirs::home_dir().expect("cannot find home dir");
    let model_dir = home.join(".vox").join("models").join("tts").join("chatterbox");
    if !model_dir.exists() {
        anyhow::bail!(
            "Chatterbox model not found at {:?}. Install via Vox settings first.",
            model_dir
        );
    }

    // Resolve model GGUFs
    let t3_path = if model_dir.join("t3-q4_0.gguf").exists() {
        model_dir.join("t3-q4_0.gguf")
    } else {
        model_dir.join("chatterbox-t3-mtl-q4_0.gguf")
    };
    let s3_path = if model_dir.join("s3gen-f16.gguf").exists() {
        model_dir.join("s3gen-f16.gguf")
    } else {
        model_dir.join("chatterbox-s3gen-mtl-f16.gguf")
    };

    println!(
        "\n\x1b[32m[CloneBench]\x1b[0m Using T3: {:?}",
        t3_path.file_name().unwrap_or_default()
    );
    println!(
        "\x1b[32m[CloneBench]\x1b[0m Using S3Gen: {:?}",
        s3_path.file_name().unwrap_or_default()
    );

    // Resolve voices directory
    let voices_dir = args.voices_dir.unwrap_or_else(|| {
        // Walk up from the binary CWD to find the workspace root
        // Fallback: assume we're running from app/src-tauri
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap() // app/
            .parent().unwrap() // vox/
            .join("temp").join("voice_corpus").join("cloned_voices")
    });

    if !voices_dir.exists() {
        anyhow::bail!(
            "Voices dir not found at {:?}. Pass --voices-dir <path>.",
            voices_dir
        );
    }
    println!(
        "\x1b[32m[CloneBench]\x1b[0m Voices dir: {:?}\n",
        voices_dir
    );

    let corpus = corpus();
    let mut results: Vec<(String, bool, String)> = Vec::new();

    for entry in &corpus {
        // Filter by --voice flag if specified
        if let Some(ref only) = args.voice {
            if only != entry.name {
                continue;
            }
        }

        let voice_dir = voices_dir.join(entry.name);
        let source_wav = voice_dir.join("source.wav");
        let baked_dir = voice_dir.join("baked");

        if !voice_dir.exists() {
            println!(
                "\x1b[33m[CloneBench]\x1b[0m Skipping '{}': directory not found",
                entry.name
            );
            continue;
        }

        // Override text/lang from CLI if provided
        let text = args.text.as_deref().unwrap_or(entry.text);
        let lang = args.lang.as_deref().unwrap_or(entry.lang);

        println!(
            "\x1b[36m[CloneBench]\x1b[0m ─── Voice: '{}' | lang={} ───",
            entry.name, lang
        );
        println!("  Text: {:?}", text);

        // Determine which reference to pass to Chatterbox:
        //   - Prefer baked/ dir (pre-baked tensors) — faster
        //   - Fall back to source.wav (raw reference) — slower but always works
        let (reference, ref_kind) = if baked_dir.exists() && baked_dir.join("speaker_emb.npy").exists() {
            (baked_dir.to_string_lossy().into_owned(), "baked tensors")
        } else if source_wav.exists() {
            (source_wav.to_string_lossy().into_owned(), "source.wav (no baked tensors)")
        } else {
            println!("  \x1b[31m✗ No source.wav or baked/ dir found, skipping.\x1b[0m");
            results.push((entry.name.to_string(), false, "missing source".into()));
            continue;
        };
        println!("  Reference: {} ({})", reference, ref_kind);

        // Build ChatterboxEngine  
        let t0 = std::time::Instant::now();
        let engine = match vox_lib::services::tts::ChatterboxEngine::new(
            &model_dir,
            lang,
            args.quality_steps as u32,
            1.0,
            Some(&reference),
        ) {
            Ok(e) => e,
            Err(e) => {
                let msg = format!("Engine init failed: {}", e);
                println!("  \x1b[31m✗ {}\x1b[0m", msg);
                results.push((entry.name.to_string(), false, msg));
                continue;
            }
        };
        let init_ms = t0.elapsed().as_millis();
        println!("  Engine init: {}ms", init_ms);

        // Synthesize
        let t1 = std::time::Instant::now();
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel::<VoxEvent>();

        if let Err(e) = engine.synthesize_chunk(text, 0, cancel, tx) {
            let msg = format!("Synthesis failed: {}", e);
            println!("  \x1b[31m✗ {}\x1b[0m", msg);
            results.push((entry.name.to_string(), false, msg));
            continue;
        }

        // Drain PCM from channel
        let mut pcm: Vec<f32> = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let VoxEvent::TtsChunk { samples, .. } = event {
                pcm.extend_from_slice(&samples);
            }
        }
        let synth_ms = t1.elapsed().as_millis();

        if pcm.is_empty() {
            let msg = "Synthesis produced no audio".to_string();
            println!("  \x1b[31m✗ {}\x1b[0m", msg);
            results.push((entry.name.to_string(), false, msg));
            continue;
        }

        let duration_s = pcm.len() as f32 / 24000.0;
        println!("  Synthesis: {}ms  →  {:.2}s of audio", synth_ms, duration_s);

        // Write output WAV
        let samples_dir = voice_dir.join("samples");
        std::fs::create_dir_all(&samples_dir)?;

        // Build filename: <lang>_<text_slug>.wav
        let slug: String = text
            .chars()
            .take(40)
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .trim_end_matches('_')
            .to_string();
        let out_name = format!("{lang}_{slug}.wav");
        let out_path = samples_dir.join(&out_name);

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 24000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&out_path, spec)?;
        for s in &pcm {
            writer.write_sample(*s)?;
        }
        writer.finalize()?;

        println!("  \x1b[32m✓ Saved:\x1b[0m {:?}", out_path);
        results.push((entry.name.to_string(), true, out_path.to_string_lossy().into_owned()));
    }

    // Summary table
    println!("\n\x1b[32m[CloneBench]\x1b[0m ═══ Results ═══");
    println!("  {:<12} {:<8} {}", "Voice", "Status", "Output / Error");
    println!("  {}", "─".repeat(70));
    for (name, ok, detail) in &results {
        let status = if *ok { "\x1b[32m✓ OK\x1b[0m   " } else { "\x1b[31m✗ FAIL\x1b[0m " };
        println!("  {:<12} {} {}", name, status, detail);
    }
    println!();

    Ok(())
}
