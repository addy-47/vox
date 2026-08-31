//! ============================================================================
//! stt_bench.rs — Realtime Passive Streaming Benchmark for Vox STT Engines
//! ============================================================================
//! Category     : Benchmark
//! Component    : services::stt, services::vad, pipeline::modular::passive
//! Prerequisites: Local model weights in ~/.vox/models/stt/
//! Execution    : cargo bench --bench stt_bench -- [FLAGS]
//! Metrics      : Partial Latency (ms), Final Turn Latency (ms), Streaming RTF, Throughput (spl/s), Memory (MB), Similarity (%)
//! ============================================================================

mod common;

use clap::Parser;
use common::audio::{load_wav, resolve_clip_path};
use common::harness::{benchmark_streaming_provider, BenchmarkClip};
use common::reporting::{
    generate_run_id, save_benchmark_report, BenchmarkReport, BenchmarkSystemInfo,
};
use std::path::PathBuf;
use vox_lib::services::stt::providers::{EmbeddedSttProvider, SttProvider};

#[derive(Parser, Debug)]
#[command(
    name = "stt_bench",
    about = "Vox Realtime Passive Streaming STT Benchmark Harness"
)]
struct CliArgs {
    /// Model to benchmark: 'nemotron', 'qwen', or 'all'
    #[arg(long, default_value = "all")]
    model: String,

    /// Benchmark a single audio clip (path or filename in test-clips/)
    #[arg(long)]
    clip: Option<String>,

    /// Directory containing .wav audio clips to benchmark in batch
    #[arg(long)]
    input_dir: Option<PathBuf>,

    /// Destination directory for benchmark JSON reports (defaults to benches/results/stt_bench/)
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Minimum character similarity threshold for canonical test clips (0.0 to 1.0, default 0.90)
    #[arg(long, default_value_t = 0.90)]
    min_similarity: f32,

    /// Optional ground-truth text when testing a single custom clip
    #[arg(long)]
    ground_truth: Option<String>,

    /// Passed by cargo bench harness runner (ignored)
    #[arg(long, hide = true)]
    bench: bool,
}

struct CanonicalClipDef {
    filename: &'static str,
    lang: &'static str,
    expected_text: &'static str,
}

const CANONICAL_TEST_CLIPS: &[CanonicalClipDef] = &[
    CanonicalClipDef {
        filename: "clip_01_en_briefing.wav",
        lang: "EN",
        expected_text: "Hey Vox, good morning! Can you check my calendar and give me a quick briefing on today's scheduled meetings?",
    },
    CanonicalClipDef {
        filename: "clip_02_en_weather.wav",
        lang: "EN",
        expected_text: "Vox, what's the weather like outside right now? Is it going to rain later this afternoon?",
    },
    CanonicalClipDef {
        filename: "clip_03_en_code.wav",
        lang: "EN",
        expected_text: "Can you help me refactor this Rust async function to reduce mutex contention across our background threads?",
    },
    CanonicalClipDef {
        filename: "clip_04_en_summary.wav",
        lang: "EN",
        expected_text: "Hey Vox, summarize the key action items from my design review notes and draft a quick email to the team.",
    },
    CanonicalClipDef {
        filename: "clip_05_en_timer.wav",
        lang: "EN",
        expected_text: "Set a timer for twenty-five minutes for a focused Pomodoro session, and minimize background notifications.",
    },
    CanonicalClipDef {
        filename: "clip_06_hi_greeting.wav",
        lang: "HI",
        expected_text: "हे वॉक्स, नमस्ते! क्या आप मेरा आज का शेड्यूल देखकर बता सकते हैं कि मेरी अगली मीटिंग कब है?",
    },
    CanonicalClipDef {
        filename: "clip_07_hi_weather.wav",
        lang: "HI",
        expected_text: "वॉक्स, आज बाहर का मौसम कैसा है? क्या शाम को बारिश होने की कोई संभावना है?",
    },
    CanonicalClipDef {
        filename: "clip_08_hi_reminder.wav",
        lang: "HI",
        expected_text: "मेरे लिए एक ज़रूरी रिमाइंडर सेट कर दो, शाम को पाँच बजे टीम के साथ प्रोजेक्ट रिव्यू करना है।",
    },
    CanonicalClipDef {
        filename: "clip_09_hi_system_cmd.wav",
        lang: "HI",
        expected_text: "वॉक्स, टर्मिनल खोलिए और हाई परफॉरमेंस मोड ऑन करके लोकल सर्वर शुरू कर दीजिए।",
    },
    CanonicalClipDef {
        filename: "clip_10_hi_qa.wav",
        lang: "HI",
        expected_text: "वॉक्स, मुझे समझाइए कि मशीन लर्निंग में स्पीच-टू-टेक्स्ट मॉडल इतनी तेज़ी से आवाज़ कैसे पहचानते हैं?",
    },
];

fn load_benchmark_clips(args: &CliArgs) -> Vec<BenchmarkClip> {
    let mut clips = Vec::new();

    if let Some(ref single_clip) = args.clip {
        let clip_path = resolve_clip_path(single_clip, args.input_dir.as_deref())
            .unwrap_or_else(|e| panic!("{}", e));
        let (audio, duration_s) = load_wav(&clip_path).unwrap_or_else(|e| panic!("{}", e));

        let filename = clip_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| single_clip.clone());

        let (lang, expected) = if let Some(ref gt) = args.ground_truth {
            ("CUSTOM".to_string(), gt.clone())
        } else if let Some(canon) = CANONICAL_TEST_CLIPS.iter().find(|c| c.filename == filename) {
            (canon.lang.to_string(), canon.expected_text.to_string())
        } else {
            ("UNKNOWN".to_string(), String::new())
        };

        clips.push(BenchmarkClip {
            filename,
            lang,
            expected_text: expected,
            audio_samples: audio,
            duration_s,
        });
    } else if let Some(ref dir) = args.input_dir {
        println!("Discovering audio clips in directory: {:?}", dir);
        let entries = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("Failed to read input directory {:?}: {}", dir, e));

        let mut wav_paths = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wav") {
                wav_paths.push(path);
            }
        }
        wav_paths.sort();

        for path in wav_paths {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            let (audio, duration_s) = load_wav(&path).unwrap_or_else(|e| panic!("{}", e));
            let (lang, expected) =
                if let Some(canon) = CANONICAL_TEST_CLIPS.iter().find(|c| c.filename == filename) {
                    (canon.lang.to_string(), canon.expected_text.to_string())
                } else {
                    ("CUSTOM".to_string(), String::new())
                };
            clips.push(BenchmarkClip {
                filename,
                lang,
                expected_text: expected,
                audio_samples: audio,
                duration_s,
            });
        }
    } else {
        println!("Loading canonical test clips...");
        for clip in CANONICAL_TEST_CLIPS {
            let path = resolve_clip_path(clip.filename, None).unwrap_or_else(|e| panic!("{}", e));
            let (audio, duration_s) = load_wav(&path).unwrap_or_else(|e| panic!("{}", e));
            clips.push(BenchmarkClip {
                filename: clip.filename.to_string(),
                lang: clip.lang.to_string(),
                expected_text: clip.expected_text.to_string(),
                audio_samples: audio,
                duration_s,
            });
        }
    }

    clips
}

fn main() {
    let args = CliArgs::parse();

    println!("================================================================================");
    println!("Vox Realtime Passive Streaming STT Benchmark Harness");
    println!("================================================================================");

    let home = dirs::home_dir().expect("Unable to find home dir");
    let nemotron_dir = home.join(".vox/models/stt/nemotron-3.5");
    let qwen_dir = home.join(".vox/models/stt/qwen3-asr");

    println!("Model Paths:");
    println!(
        "  Nemotron-3.5 Multilingual (sherpa-onnx 1.13.6): {:?}",
        nemotron_dir
    );
    println!(
        "  Qwen3-ASR (sherpa-onnx 1.13.6)              : {:?}",
        qwen_dir
    );
    println!("Configuration:");
    println!("  Target Model   : {}", args.model);
    println!("  Min Similarity : {:.2}", args.min_similarity);
    println!("  Input Clip     : {:?}", args.clip);
    println!("  Input Dir      : {:?}", args.input_dir);
    println!("  Output Dir     : {:?}", args.output_dir);

    let loaded_clips = load_benchmark_clips(&args);
    println!("Loaded {} test clips successfully.\n", loaded_clips.len());

    let run_nemotron = matches!(
        args.model.to_lowercase().as_str(),
        "nemotron" | "nvidia_nemotron" | "all"
    );
    let run_qwen = matches!(
        args.model.to_lowercase().as_str(),
        "qwen" | "qwen3_asr" | "all"
    );

    let mut engine_runs = Vec::new();

    // 1. Benchmark Production Nemotron-3.5 Streaming (Sherpa-ONNX 1.13.6 OnlineRecognizer)
    if run_nemotron {
        if nemotron_dir.exists() {
            let provider = Box::new(
                EmbeddedSttProvider::new(&nemotron_dir, "nvidia_nemotron")
                    .expect("Failed to initialize Nemotron EmbeddedSttProvider"),
            ) as Box<dyn SttProvider>;

            let run = benchmark_streaming_provider(
                "Nemotron-3.5 Streaming (Production Sherpa-ONNX 1.13.6)",
                "nvidia_nemotron",
                &nemotron_dir.to_string_lossy(),
                provider,
                &loaded_clips,
            );
            engine_runs.push(run);
        } else {
            eprintln!(
                "[WARN] Nemotron-3.5 model directory not found at {:?}",
                nemotron_dir
            );
        }
    }

    // 2. Benchmark Production Qwen3-ASR Streaming (Sherpa-ONNX 1.13.6 OfflineRecognizer)
    if run_qwen {
        if qwen_dir.exists() {
            let provider = Box::new(
                EmbeddedSttProvider::new(&qwen_dir, "qwen3_asr")
                    .expect("Failed to initialize Qwen3-ASR EmbeddedSttProvider"),
            ) as Box<dyn SttProvider>;

            let run = benchmark_streaming_provider(
                "Qwen3-ASR Streaming (Production Sherpa-ONNX 1.13.6)",
                "qwen3_asr",
                &qwen_dir.to_string_lossy(),
                provider,
                &loaded_clips,
            );
            engine_runs.push(run);
        } else {
            eprintln!(
                "[WARN] Qwen3-ASR model directory not found at {:?}",
                qwen_dir
            );
        }
    }

    // 3. Persist Benchmark Results to JSON Artifact
    let base_output_dir = args
        .output_dir
        .unwrap_or_else(|| PathBuf::from("benches/results/stt_bench"));

    let run_id = generate_run_id();
    let report = BenchmarkReport {
        run_id: run_id.clone(),
        timestamp_utc: chrono::Utc::now().to_rfc3339(),
        benchmark_name: "stt_bench".to_string(),
        system_info: BenchmarkSystemInfo::default(),
        runs: engine_runs.clone(),
    };

    match save_benchmark_report(&base_output_dir, &report) {
        Ok(saved_path) => {
            println!("\n================================================================================");
            println!("Benchmark Result Artifact Successfully Saved!");
            println!("Run ID   : {}", run_id);
            println!("Report   : {:?}", saved_path);
            println!("Latest   : {:?}", base_output_dir.join("latest.json"));
            println!(
                "================================================================================"
            );
        }
        Err(e) => {
            eprintln!("\n[ERROR] Failed to persist benchmark report: {}", e);
        }
    }

    // 4. Evaluate Threshold Gate (when running primary Nemotron engine on canonical/clip datasets)
    if let Some(nemotron_run) = engine_runs
        .iter()
        .find(|r| r.model_type == "nvidia_nemotron")
    {
        let mut failed_clips = Vec::new();
        for clip in &nemotron_run.clips {
            if !clip.ground_truth.is_empty() && (clip.similarity as f32) < args.min_similarity {
                failed_clips.push((
                    clip.filename.clone(),
                    clip.similarity,
                    clip.ground_truth.clone(),
                    clip.hypothesis.clone(),
                ));
            }
        }

        if !failed_clips.is_empty() {
            eprintln!(
                "\n[BENCHMARK FAILED] {} clips fell below similarity threshold ({:.2}):",
                failed_clips.len(),
                args.min_similarity
            );
            for (fname, sim, gt, hyp) in failed_clips {
                eprintln!("  • {:<24} (Sim: {:>5.1}%)", fname, sim * 100.0);
                eprintln!("      Ground Truth : {}", gt);
                eprintln!("      Hypothesis   : {}", hyp);
            }
            std::process::exit(1);
        } else {
            println!(
                "\n[BENCHMARK PASSED] All clips met similarity threshold (>= {:.2}).",
                args.min_similarity
            );
        }
    }
}
