//! ============================================================================
//! benches/common/tts_harness.rs — TTS Production-Seam Benchmark Harness
//! ============================================================================
//! Uses the real worker pipeline:
//!   TtsCommand::Generate -> spawn_tts_worker -> TtsProvider::synthesize_chunk
//!       -> PlaybackEngine::ingest_chunk (HeapRb, no CPAL hardware)
//! Audio is captured from the ring buffer and persisted as 24 kHz WAV.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Split};

use vox_lib::core::events::VoxEvent;
use vox_lib::services::audio::playback::PlaybackEngineHandles;
use vox_lib::services::audio::{PlaybackEngine, PLAYBACK_BUFFER_SAMPLES};
use vox_lib::services::tts::actor::{spawn_tts_worker, TtsCommand, TtsWorkerHandles};
use vox_lib::services::tts::providers::TtsProvider;
use vox_lib::services::tts::{TTS_SAMPLE_RATE};

use super::reporting::{get_process_memory_mb, ClipBenchmarkResult, EngineBenchmarkRun};

/// Text prompt derived from canonical test-clips transcripts (verbatim).
#[derive(Debug, Clone)]
pub struct TtsBenchmarkPrompt {
    pub filename: String,
    pub lang: String,
    pub text: String,
}

/// Per-clip TTS measurement enriched with WAV persistence.
#[derive(Debug, Clone)]
pub struct TtsClipResult {
    pub prompt: TtsBenchmarkPrompt,
    pub voice: i32,
    pub wav_path: Option<PathBuf>,
    pub audio_duration_s: f32,
    pub synthesis_latency_ms: f64,
    pub rtf: f64,
    pub throughput_spl_s: f64,
    pub samples: usize,
    pub clip_result: ClipBenchmarkResult,
}

/// Persist mono 24 kHz f32 samples as WAV.
fn write_wav_f32(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create WAV dir {:?}: {}", parent, e))?;
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create WAV {:?}: {}", path, e))?;
    for s in samples {
        writer
            .write_sample(*s)
            .map_err(|e| format!("WAV write failed: {}", e))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("WAV finalize failed: {}", e))?;
    Ok(())
}

/// Drain all available samples from a HeapCons without blocking.
fn drain_consumer(consumer: &mut ringbuf::HeapCons<f32>) -> Vec<f32> {
    let mut out = Vec::new();
    while let Some(s) = consumer.try_pop() {
        out.push(s);
    }
    out
}

/// Benchmark a single TTS provider across prompts, using the production seam.
///
/// * `engine_name` – display name
/// * `model_type` – machine id (supertonic/kokoro/chatterbox)
/// * `model_path` – path for reporting
/// * `prompts` – canonical verbatim transcripts
/// * `make_provider` – factory that may vary voice per prompt (for Kokoro diff voices)
/// * `wav_output_dir` – if Some, each clip's 24 kHz WAV is persisted
/// * `base_voice` – used for non-factory path; for kokoro diff voices pass `None` and use factory
#[allow(clippy::too_many_arguments)]
pub fn benchmark_tts_provider<F>(
    engine_name: &str,
    model_type: &str,
    model_path: &str,
    prompts: &[TtsBenchmarkPrompt],
    mut make_provider: F,
    wav_output_dir: Option<&Path>,
    default_voice: i32,
) -> EngineBenchmarkRun
where
    F: FnMut(usize, &TtsBenchmarkPrompt) -> Box<dyn TtsProvider>,
{
    println!("\n================================================================================");
    println!(">>> TTS Benchmark: {} (max quality, 24 kHz)", engine_name);
    println!("================================================================================");
    println!("Prompts: {} | Default voice: {} | WAV out: {:?}", prompts.len(), default_voice, wav_output_dir);

    let mem_before = get_process_memory_mb();
    let total_start = Instant::now();

    // We reuse one PlaybackEngine + ring buffer across clips but drain between clips.
    // PlaybackEngine::from_parts allows headless operation (stream=None).
    let rb = HeapRb::<f32>::new(PLAYBACK_BUFFER_SAMPLES);
    let (prod, mut cons) = rb.split();

    // Handles required by PlaybackEngine
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let discard_request = Arc::new(AtomicBool::new(false));
    let turn_armed = Arc::new(AtomicBool::new(false));
    let state_atomic = Arc::new(AtomicU32::new(0));
    let current_turn_id = Arc::new(AtomicU32::new(1));
    let pending_jobs = Arc::new(AtomicU32::new(0));
    let (event_tx, _event_rx) = std::sync::mpsc::channel::<VoxEvent>();

    let playback_handles = PlaybackEngineHandles {
        cancel_flag: Arc::clone(&cancel_flag),
        state_atomic,
        current_turn_id: Arc::clone(&current_turn_id),
        pending_synthesis_jobs: Arc::clone(&pending_jobs),
        event_tx: event_tx.clone(),
    };

    let playback = Arc::new(PlaybackEngine::from_parts(
        prod,
        playback_handles,
        Arc::clone(&discard_request),
        Arc::clone(&turn_armed),
        None,
    ));

    let mem_after_init = get_process_memory_mb();
    println!(
        "Process Memory RSS post-init: {} MB (delta {} MB)",
        mem_after_init,
        mem_after_init.saturating_sub(mem_before)
    );

    // Per-clip results
    let mut clip_results: Vec<ClipBenchmarkResult> = Vec::new();
    let mut tts_clip_results: Vec<TtsClipResult> = Vec::new();

    for (idx, prompt) in prompts.iter().enumerate() {
        let voice_for_clip = if model_type == "kokoro" {
            // Diff voices per clip: cycle 0..N-1
            (idx as i32) % 10
        } else {
            default_voice
        };

        // For kokoro diff voices we need a fresh provider per clip (engine holds voice at init)
        // For supertonic/chatterbox a single provider suffices but creating per-clip is also fine
        // and keeps the factory path uniform. Reuse logic: factory decides.
        let provider = make_provider(idx, prompt);

        // Isolated worker per clip guarantees cold isolation without cross-clip state bleed.
        // Alternative of reusing one worker across all clips would intermix pending_synthesis_jobs.
        // Cost is thread spawn/join (~1ms) negligible vs synthesis (seconds).
        let (tx, rx) = std::sync::mpsc::channel::<TtsCommand>();
        let rtf_atomic = Arc::new(AtomicU32::new(0));
        let worker_handles = TtsWorkerHandles {
            playback: Arc::clone(&playback),
            event_tx: event_tx.clone(),
            cancel_flag: Arc::clone(&cancel_flag),
            pending_synthesis_jobs: Some(Arc::clone(&pending_jobs)),
            telemetry_rtf: Some(Arc::clone(&rtf_atomic)),
        };

        // Ensure ring buffer empty before clip
        let _ = drain_consumer(&mut cons);
        turn_armed.store(false, Ordering::Relaxed);
        discard_request.store(false, Ordering::Relaxed);
        cancel_flag.store(false, Ordering::Relaxed);
        pending_jobs.store(1, Ordering::Relaxed);
        current_turn_id.store((idx as u32) + 1, Ordering::Relaxed);

        let handle = std::thread::Builder::new()
            .name(format!("bench-tts-worker-{}", idx))
            .spawn(move || {
                spawn_tts_worker(rx, provider, worker_handles);
            })
            .expect("Failed to spawn TTS worker thread");

        let synthesis_start = Instant::now();
        let turn_id = (idx as u32) + 1;

        // Production entry seam
        if let Err(e) = tx.send(TtsCommand::Generate {
            turn_id,
            text: prompt.text.clone(),
        }) {
            eprintln!("[TTS Bench] Failed to send Generate for {}: {}", prompt.filename, e);
            let _ = tx.send(TtsCommand::Shutdown);
            let _ = handle.join();
            continue;
        }

        // Poll for completion: wait until pending_jobs drops to 0 and rtf_atomic set, with hard deadline.
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut rtf_reported = 0.0;
        let mut completed = false;

        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
            if pending_jobs.load(Ordering::Relaxed) == 0 {
                // Give PlaybackEngine a moment to push last chunk
                std::thread::sleep(Duration::from_millis(120));
                completed = true;
                break;
            }
            // Early RTF availability check is fine but pending_jobs is authoritative
            let bits = rtf_atomic.load(Ordering::Relaxed);
            if bits != 0 {
                rtf_reported = f32::from_bits(bits) as f64;
            }
            if synthesis_start.elapsed().as_millis() > 60000 {
                break;
            }
        }

        let elapsed_ms = synthesis_start.elapsed().as_secs_f64() * 1000.0;

        // Shutdown worker thread after this clip
        let _ = tx.send(TtsCommand::Shutdown);
        let _ = handle.join();

        // Final RTF from atomic
        let bits = rtf_atomic.load(Ordering::Relaxed);
        if bits != 0 {
            rtf_reported = f32::from_bits(bits) as f64;
        }

        // Drain captured audio
        let samples = drain_consumer(&mut cons);
        let audio_duration_s = if samples.is_empty() {
            0.0
        } else {
            samples.len() as f32 / TTS_SAMPLE_RATE as f32
        };

        let rtf = if audio_duration_s > 0.0 {
            (elapsed_ms / 1000.0) / audio_duration_s as f64
        } else {
            rtf_reported
        };

        let throughput = if elapsed_ms > 0.0 {
            samples.len() as f64 / (elapsed_ms / 1000.0)
        } else {
            0.0
        };

        // Persist WAV
        let wav_path = if let Some(out_dir) = wav_output_dir {
            let safe_name = prompt
                .filename
                .replace(".wav", "")
                .replace(".txt", "");
            let file_name = format!("{}_{}_voice{}_turn{}.wav", model_type, safe_name, voice_for_clip, turn_id);
            let path = out_dir.join(file_name);
            match write_wav_f32(&path, &samples, TTS_SAMPLE_RATE) {
                Ok(()) => Some(path),
                Err(e) => {
                    eprintln!("[TTS Bench] WAV write failed for {}: {}", prompt.filename, e);
                    None
                }
            }
        } else {
            None
        };

        let status = if !completed {
            "TIMEOUT"
        } else if samples.is_empty() {
            "EMPTY"
        } else {
            "OK"
        };

        println!(
            "[{:>2}/{}] {:<28} lang={:<4} voice={:<2} | synth {:>7.0} ms | audio {:>5.2} s | RTF {:>6.3} | {:>7.0} spl/s | {:<7} | wav: {}",
            idx + 1,
            prompts.len(),
            prompt.filename,
            prompt.lang,
            voice_for_clip,
            elapsed_ms,
            audio_duration_s,
            rtf,
            throughput,
            status,
            wav_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".to_string())
        );

        let cr = ClipBenchmarkResult {
            filename: prompt.filename.clone(),
            lang: prompt.lang.clone(),
            duration_s: audio_duration_s,
            total_stream_time_ms: elapsed_ms,
            final_post_speech_latency_ms: elapsed_ms,
            rtf,
            throughput_spl_s: throughput,
            partials_emitted: voice_for_clip as usize,
            similarity: if samples.is_empty() { 0.0 } else { 1.0 },
            hypothesis: format!("voice={} samples={} wav={:?}", voice_for_clip, samples.len(), wav_path),
            ground_truth: prompt.text.clone(),
        };
        clip_results.push(cr.clone());
        tts_clip_results.push(TtsClipResult {
            prompt: prompt.clone(),
            voice: voice_for_clip,
            wav_path,
            audio_duration_s,
            synthesis_latency_ms: elapsed_ms,
            rtf,
            throughput_spl_s: throughput,
            samples: samples.len(),
            clip_result: cr,
        });

        if !completed {
            eprintln!(
                "[WARN] Clip {} timed out after {:.1}s — likely synthesis hang",
                prompt.filename,
                elapsed_ms / 1000.0
            );
        }
    }

    let total_elapsed_ms = total_start.elapsed().as_secs_f64() * 1000.0;
    let total_audio_s: f32 = tts_clip_results.iter().map(|r| r.audio_duration_s).sum();
    let avg_rtf = if clip_results.is_empty() {
        0.0
    } else {
        clip_results.iter().map(|r| r.rtf).sum::<f64>() / clip_results.len() as f64
    };
    let avg_latency = if clip_results.is_empty() {
        0.0
    } else {
        clip_results.iter().map(|r| r.total_stream_time_ms).sum::<f64>() / clip_results.len() as f64
    };
    let total_samples: usize = tts_clip_results.iter().map(|r| r.samples).sum();
    let overall_throughput = if total_elapsed_ms > 0.0 {
        total_samples as f64 / (total_elapsed_ms / 1000.0)
    } else {
        0.0
    };

    println!("\n--- Overall TTS Summary for {} ---", engine_name);
    println!("Total Prompts             : {}", prompts.len());
    println!("Total Audio Generated     : {:.2}s", total_audio_s);
    println!("Total Wall Time           : {:.2}s", total_elapsed_ms / 1000.0);
    println!("Avg Synthesis Latency     : {:.0} ms", avg_latency);
    println!("Avg RTF                   : {:.3}x (lower is faster; <1.0 is realtime)", avg_rtf);
    println!(
        "Throughput                : {:.0} samples/s ({:.2}x realtime)",
        overall_throughput,
        if avg_rtf > 0.0 { 1.0 / avg_rtf } else { 0.0 }
    );
    println!("Memory RSS (post-init)    : ~{} MB", mem_after_init);

    EngineBenchmarkRun {
        engine_name: engine_name.to_string(),
        model_type: model_type.to_string(),
        model_path: model_path.to_string(),
        memory_rss_mb: mem_after_init,
        total_audio_s,
        total_stream_time_ms: total_elapsed_ms,
        avg_post_speech_latency_ms: avg_latency,
        avg_rtf,
        overall_throughput_spl_s: overall_throughput,
        avg_similarity: 1.0,
        clips: clip_results,
    }
}
