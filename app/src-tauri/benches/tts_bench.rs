//! ============================================================================
//! tts_bench.rs — TTS Synthesis Benchmark (Production-Seam, Max Quality)
//! ============================================================================
//! Category     : Benchmark
//! Component    : services::tts, services::audio::playback (TtsProvider seam)
//! Prerequisites: Local model weights in ~/.vox/models/tts/{supertonic-3,kokoro,chatterbox}
//! Execution    : cargo test --bench tts_bench --release -- --model supertonic --clip clip_01_en_briefing.wav
//!                cargo bench --bench tts_bench -- --model all
//! Metrics      : Synthesis Latency (ms), Audio Duration (s), RTF, Throughput (spl/s), Memory (MB)
//! Artifacts    : benches/results/tts_bench/<run_id>/report.json + wav/*.wav + latest.json
//! Notes        : Uses max quality steps (Supertonic 16, Chatterbox 10, speed 1.0).
//!                Kokoro uses diff voice per clip (voice = clip_idx % 10). Wavs @ 24 kHz.
//! ============================================================================

mod common;

use clap::Parser;
use common::reporting::{
    generate_run_id, save_benchmark_report, BenchmarkReport, BenchmarkSystemInfo,
};
use common::tts_harness::{benchmark_tts_provider, TtsBenchmarkPrompt};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    name = "tts_bench",
    about = "Vox TTS Synthesis Benchmark (max quality, production seam)"
)]
struct CliArgs {
    /// Model to benchmark: 'supertonic', 'kokoro', 'chatterbox', 'edge' or 'all' (default: all local)
    #[arg(long, default_value = "all")]
    model: String,

    /// Benchmark a single prompt by clip filename (e.g. clip_01_en_briefing.wav) to pull its transcript
    #[arg(long)]
    clip: Option<String>,

    /// Single custom text to synthesize (overrides clip lookup)
    #[arg(long)]
    text: Option<String>,

    /// Destination directory for benchmark JSON reports (defaults to benches/results/tts_bench/)
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Optional directory to also write wav files outside the run dir
    #[arg(long)]
    wav_dir: Option<PathBuf>,

    /// Voice index for non-kokoro models (0..9). Kokoro auto-cycles per clip.
    #[arg(long, default_value_t = 0)]
    voice: i32,

    /// Passed by cargo bench harness runner (ignored)
    #[arg(long, hide = true)]
    bench: bool,
}

struct CanonicalPromptDef {
    filename: &'static str,
    lang: &'static str,
    text: &'static str,
}

// Verbatim transcripts from test-clips (same as stt_bench CANONICAL_TEST_CLIPS)
const CANONICAL_TTS_PROMPTS: &[CanonicalPromptDef] = &[
    CanonicalPromptDef {
        filename: "clip_01_en_briefing.wav",
        lang: "EN",
        text: "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?",
    },
    CanonicalPromptDef {
        filename: "clip_02_en_weather.wav",
        lang: "EN",
        text: "Vox, what's the weather like outside right now? Is it going to rain later this afternoon?",
    },
    CanonicalPromptDef {
        filename: "clip_03_en_code.wav",
        lang: "EN",
        text: "Can you help me refactor this Rust async function to reduce mutex contention across our background threads?",
    },
    CanonicalPromptDef {
        filename: "clip_04_en_summary.wav",
        lang: "EN",
        text: "Hey Vox, summarize the key action items from my design review notes and draft a quick email to the team.",
    },
    CanonicalPromptDef {
        filename: "clip_05_en_timer.wav",
        lang: "EN",
        text: "Set a timer for twenty-five minutes for a focused Pomodoro session, and minimize background notifications.",
    },
    CanonicalPromptDef {
        filename: "clip_06_hi_greeting.wav",
        lang: "HI",
        text: "हे वॉक्स, नमस्ते! क्या आप मेरा आज का शेड्यूल देखकर बता सकते हैं कि मेरी अगली मीटिंग कब है?",
    },
    CanonicalPromptDef {
        filename: "clip_07_hi_weather.wav",
        lang: "HI",
        text: "वॉक्स, आज बाहर का मौसम कैसा है? क्या शाम को बारिश होने की कोई संभावना है?",
    },
    CanonicalPromptDef {
        filename: "clip_08_hi_reminder.wav",
        lang: "HI",
        text: "मेरे लिए एक ज़रूरी रिमाइंडर सेट कर दो, शाम को पाँच बजे टीम के साथ प्रोजेक्ट रिव्यू करना है।",
    },
    CanonicalPromptDef {
        filename: "clip_09_hi_system_cmd.wav",
        lang: "HI",
        text: "वॉक्स, टर्मिनल खोलिए और हाई परफॉरमेंस मोड ऑन करके लोकल सर्वर शुरू कर दीजिए।",
    },
    CanonicalPromptDef {
        filename: "clip_10_hi_qa.wav",
        lang: "HI",
        text: "वॉक्स, मुझे समझाइए कि मशीन लर्निंग में स्पीच-टू-टेक्स्ट मॉडल इतनी तेज़ी से आवाज़ कैसे पहचानते हैं?",
    },
];

fn load_benchmark_prompts(args: &CliArgs) -> Vec<TtsBenchmarkPrompt> {
    // Priority: --text > --clip > canonical all
    if let Some(ref custom) = args.text {
        let filename = args
            .clip
            .clone()
            .unwrap_or_else(|| "custom_prompt.txt".to_string());
        let lang = if vox_lib::services::translit::is_devanagari(custom) {
            "HI"
        } else {
            "EN"
        };
        return vec![TtsBenchmarkPrompt {
            filename,
            lang: lang.to_string(),
            text: custom.clone(),
        }];
    }

    if let Some(ref clip_name) = args.clip {
        // Resolve clip_name to canonical entry if known, else treat as raw filename with empty text lookup fallback
        let basename = Path::new(clip_name)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| clip_name.clone());
        if let Some(canon) = CANONICAL_TTS_PROMPTS
            .iter()
            .find(|c| c.filename == basename)
        {
            return vec![TtsBenchmarkPrompt {
                filename: canon.filename.to_string(),
                lang: canon.lang.to_string(),
                text: canon.text.to_string(),
            }];
        }
        // Unknown clip name but user provided explicit clip with text override already handled; fallback to verbatim search by exact string
        if let Some(canon) = CANONICAL_TTS_PROMPTS
            .iter()
            .find(|c| c.filename == clip_name.as_str())
        {
            return vec![TtsBenchmarkPrompt {
                filename: canon.filename.to_string(),
                lang: canon.lang.to_string(),
                text: canon.text.to_string(),
            }];
        }
        // If user passed e.g. "clip_01..." without path, still allow
        eprintln!(
            "[TTS Bench] Unknown clip '{}', falling back to canonical list filtered",
            clip_name
        );
        // Try prefix match
        let filtered: Vec<_> = CANONICAL_TTS_PROMPTS
            .iter()
            .filter(|c| c.filename.contains(clip_name.as_str()) || clip_name.contains(c.filename))
            .map(|c| TtsBenchmarkPrompt {
                filename: c.filename.to_string(),
                lang: c.lang.to_string(),
                text: c.text.to_string(),
            })
            .collect();
        if !filtered.is_empty() {
            return filtered;
        }
        // Last resort: treat clip arg as raw text
        return vec![TtsBenchmarkPrompt {
            filename: basename,
            lang: "EN".to_string(),
            text: clip_name.clone(),
        }];
    }

    // Default: all 10 canonical prompts verbatim
    CANONICAL_TTS_PROMPTS
        .iter()
        .map(|c| TtsBenchmarkPrompt {
            filename: c.filename.to_string(),
            lang: c.lang.to_string(),
            text: c.text.to_string(),
        })
        .collect()
}

fn main() {
    let args = CliArgs::parse();

    println!("================================================================================");
    println!("Vox TTS Synthesis Benchmark (Production Seam, Max Quality)");
    println!("================================================================================");

    let home = dirs::home_dir().expect("Unable to find home dir");
    let supertonic_dir = home.join(".vox/models/tts/supertonic-3");
    let kokoro_dir = home.join(".vox/models/tts/kokoro");
    let chatterbox_dir = home.join(".vox/models/tts/chatterbox");

    println!("Model Paths:");
    println!("  Supertonic (16 steps) : {:?}", supertonic_dir);
    println!("  Kokoro v1.1 (multi)   : {:?}", kokoro_dir);
    println!("  Chatterbox (10 steps) : {:?}", chatterbox_dir);
    println!("Configuration:");
    println!("  Target Model : {}", args.model);
    println!("  Clip Filter  : {:?}", args.clip);
    println!("  Custom Text  : {:?}", args.text);
    println!("  Voice (base) : {}", args.voice);
    println!("  Output Dir   : {:?}", args.output_dir);
    println!("  WAV Dir      : {:?}", args.wav_dir);
    println!("  Max Quality  : Supertonic=16 steps, Chatterbox=10 steps, Kokoro=speed 1.0");
    println!("  Kokoro Policy: diff voice per clip (voice = idx % 10)");

    let prompts = load_benchmark_prompts(&args);
    println!(
        "Loaded {} prompt(s) (verbatim from test-clips)",
        prompts.len()
    );
    for p in &prompts {
        println!(
            "  - {:<28} [{}] \"{}\"",
            p.filename,
            p.lang,
            p.text.chars().take(80).collect::<String>()
        );
    }

    let model_arg = args.model.to_lowercase();
    let run_supertonic = matches!(model_arg.as_str(), "supertonic" | "super" | "all");
    let run_kokoro = matches!(model_arg.as_str(), "kokoro" | "all");
    let run_chatterbox = matches!(model_arg.as_str(), "chatterbox" | "cb" | "all");
    let run_edge = matches!(model_arg.as_str(), "edge" | "edge_tts");

    // Wav persistence: per-run wav dir under report dir + optional extra dir
    let base_output_dir = args
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("benches/results/tts_bench"));
    let run_id = generate_run_id();
    let run_dir = base_output_dir.join(&run_id);
    let wav_run_dir = run_dir.join("wav");
    std::fs::create_dir_all(&wav_run_dir).expect("Failed to create wav run dir");

    let mut engine_runs = Vec::new();

    let base_voice = args.voice;

    // 1) Supertonic — max 16 steps, speed 1.0, realtime via production seam
    if run_supertonic {
        if supertonic_dir.exists() {
            use vox_lib::services::tts::providers::TtsProvider;
            let prompts_clone = prompts.clone();
            let wav_dir_clone = wav_run_dir.clone();
            let supertonic_path_str = supertonic_dir.to_string_lossy().to_string();
            let supertonic_dir_cloned = supertonic_dir.clone();
            let voice = base_voice;
            let run = benchmark_tts_provider(
                "Supertonic 3 Multilingual (max 16 steps, speed 1.0)",
                "supertonic",
                &supertonic_path_str,
                &prompts_clone,
                move |_idx, _prompt| {
                    let p: Box<dyn TtsProvider> = Box::new(
                        vox_lib::services::tts::TtsEngine::new(
                            &supertonic_dir_cloned,
                            voice,
                            16,
                            1.0,
                        )
                        .expect("Failed to init Supertonic"),
                    );
                    p
                },
                Some(&wav_dir_clone),
                voice,
            );
            engine_runs.push(run);
        } else {
            eprintln!(
                "[WARN] Supertonic model dir not found at {:?}",
                supertonic_dir
            );
        }
    }

    // 2) Kokoro — diff voice per clip, speed 1.0
    if run_kokoro {
        if kokoro_dir.exists() && kokoro_dir.join("model.onnx").exists() {
            use vox_lib::services::tts::providers::TtsProvider;
            let wav_dir_clone = wav_run_dir.clone();
            // capture kokoro_dir by clone for move closure
            let kd = kokoro_dir.clone();
            let kd_str = kd.to_string_lossy().to_string();
            let voice = base_voice;
            let run = benchmark_tts_provider(
                "Kokoro Multi-Lang v1.1 (diff voice per clip, speed 1.0)",
                "kokoro",
                &kd_str,
                &prompts,
                move |idx, _prompt| {
                    let v = (idx as i32) % 10;
                    let p: Box<dyn TtsProvider> = Box::new(
                        vox_lib::services::tts::KokoroEngine::new(&kd, v, 1.0)
                            .unwrap_or_else(|e| panic!("Failed to init Kokoro voice {}: {}", v, e)),
                    );
                    p
                },
                Some(&wav_dir_clone),
                voice,
            );
            engine_runs.push(run);

            // Print Kokoro capability hint
            println!("\n[Kokoro] Voices cycled per clip (0..9). Check wavs for Hindi prosody: HI clips should retain Devanagari phonemes if model supports it.");
        } else {
            eprintln!("[WARN] Kokoro model not found at {:?} — reinstall via sherpa-onnx tts-models/kokoro-multi-lang-v1_1.tar.bz2", kokoro_dir);
        }
    }

    // 3) Chatterbox — max 10 steps
    if run_chatterbox {
        if chatterbox_dir.join("t3-q4_0.gguf").exists() {
            use vox_lib::services::tts::providers::TtsProvider;
            let wav_dir_clone = wav_run_dir.clone();
            let cd = chatterbox_dir.clone();
            let cd_str = cd.to_string_lossy().to_string();
            let voice = base_voice;
            let run = benchmark_tts_provider(
                "Chatterbox Local (max 10 steps, speed 1.0)",
                "chatterbox",
                &cd_str,
                &prompts,
                move |_idx, _prompt| {
                    let p: Box<dyn TtsProvider> = Box::new(
                        vox_lib::services::tts::ChatterboxEngine::new(&cd, "en", 10, 1.0, None)
                            .expect("Failed to init Chatterbox"),
                    );
                    p
                },
                Some(&wav_dir_clone),
                voice,
            );
            engine_runs.push(run);
        } else {
            eprintln!("[WARN] Chatterbox model not found at {:?}", chatterbox_dir);
        }
    }

    if run_edge {
        eprintln!("[INFO] EdgeTTS is cloud and requires network; skipping bench by default (use --model edge explicitly if needed)");
    }

    if engine_runs.is_empty() {
        eprintln!("\n[ERROR] No engine runs completed — check model paths and --model flag");
        std::process::exit(1);
    }

    // 4) Persist report (also copies wav_run_dir already populated)
    let report = BenchmarkReport {
        run_id: run_id.clone(),
        timestamp_utc: chrono::Utc::now().to_rfc3339(),
        benchmark_name: "tts_bench".to_string(),
        system_info: BenchmarkSystemInfo::default(),
        runs: engine_runs.clone(),
    };

    match save_benchmark_report(&base_output_dir, &report) {
        Ok(saved_path) => {
            // Ensure wav dir is discoverable from run dir (already there)
            // Also mirror wav dir to latest symlink style: keep wavs under run_dir/wav
            // If user requested extra wav_dir, copy there too
            if let Some(extra) = args.wav_dir {
                let _ = std::fs::create_dir_all(&extra);
                if let Ok(entries) = std::fs::read_dir(&wav_run_dir) {
                    for e in entries.flatten() {
                        let dest = extra.join(e.file_name());
                        let _ = std::fs::copy(e.path(), dest);
                    }
                }
            }
            println!("\n================================================================================");
            println!("TTS Benchmark Artifact Saved!");
            println!("Run ID   : {}", run_id);
            println!("Report   : {:?}", saved_path);
            println!("WAVs     : {:?}", wav_run_dir);
            println!("Latest   : {:?}", base_output_dir.join("latest.json"));
            println!(
                "================================================================================"
            );
            println!("Manual QA: play wavs per clip and compare EN prosody vs HI (kokoro HI may be limited).");
            for run in &engine_runs {
                println!(
                    "  {} : avg RTF {:.3} | avg latency {:.0} ms | total audio {:.2}s",
                    run.model_type, run.avg_rtf, run.avg_post_speech_latency_ms, run.total_audio_s
                );
            }
        }
        Err(e) => {
            eprintln!("\n[ERROR] Failed to persist benchmark report: {}", e);
            std::process::exit(1);
        }
    }
}
