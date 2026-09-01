use ringbuf::traits::{Observer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use vox_lib::core::events::VoxEvent;
use vox_lib::core::settings::{AudioOutputMode, InteractionMode};
use vox_lib::services::stt::actor::{
    spawn_stt_worker, SttActorChannels, SttActorHandles, SttCommand,
};
use vox_lib::services::stt::SttProvider;
use vox_lib::services::vad::actor::{
    spawn_vad_actor, VadActorChannels, VadActorConfig, VadActorHandles,
};
use vox_lib::services::vad::VadCommand;
use vox_lib::services::vad::{VAD_CHUNK_SIZE, VAD_SPEECH_END_FRAMES};

use super::reporting::{get_process_memory_mb, ClipBenchmarkResult, EngineBenchmarkRun};
use super::scoring::levenshtein_similarity;

/// Input ground truth clip definition.
#[derive(Debug, Clone)]
pub struct BenchmarkClip {
    pub filename: String,
    pub lang: String,
    pub expected_text: String,
    pub audio_samples: Vec<f32>,
    pub duration_s: f32,
}

/// Creates a mock AppHandle for benchmark runs.
pub fn get_test_app_handle() -> AppHandle<tauri::test::MockRuntime> {
    tauri::test::mock_app().handle().clone()
}

/// Executes streaming benchmark on an STT provider across a list of audio clips.
pub fn benchmark_streaming_provider(
    engine_name: &str,
    model_type: &str,
    model_path: &str,
    provider: Box<dyn SttProvider>,
    clips: &[BenchmarkClip],
) -> EngineBenchmarkRun {
    println!("\n================================================================================");
    println!(">>> Streaming Benchmark: {}", engine_name);
    println!("================================================================================");

    let (stt_tx, stt_rx) = mpsc::channel::<SttCommand>();
    let (pipeline_event_tx, pipeline_event_rx) = mpsc::channel::<VoxEvent>();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let engine_shutdown = Arc::new(AtomicBool::new(false));

    let channels = SttActorChannels {
        rx: stt_rx,
        pipeline_event_tx: Some(pipeline_event_tx),
    };
    let handles = SttActorHandles {
        cancel_flag,
        engine_shutdown: engine_shutdown.clone(),
    };

    let stt_handle =
        spawn_stt_worker(channels, provider, handles).expect("Failed to spawn STT worker");

    let rb = HeapRb::<f32>::new(65536);
    let (mut producer, consumer) = rb.split();

    let vad_config = VadActorConfig {
        initial_threshold: 0.3,
        initial_noise_gate: 0.001,
        initial_mode: InteractionMode::Passive,
        initial_audio_mode: AudioOutputMode::Headset,
    };

    let (vad_cmd_tx, vad_cmd_rx) = mpsc::channel::<VadCommand>();
    let (telemetry_tx, _telemetry_rx) = crossbeam_channel::unbounded();
    let (vox_event_tx, vox_event_rx) = mpsc::channel::<VoxEvent>();

    let vad_channels = VadActorChannels {
        stt_tx: stt_tx.clone(),
        vad_rx: vad_cmd_rx,
        telemetry_tx,
        vox_event_tx: Some(vox_event_tx),
    };

    let vad_handles = VadActorHandles {
        state_atomic: Arc::new(AtomicU32::new(0)),
        turn_id_atomic: Arc::new(AtomicU32::new(0)),
        audio_suppressed: Arc::new(AtomicBool::new(false)),
        engine_shutdown: engine_shutdown.clone(),
        dropped_counter: Arc::new(AtomicU64::new(0)),
        turn_token: Arc::new(parking_lot::Mutex::new(tokio_util::sync::CancellationToken::new())),
        turn_epoch: Arc::new(std::sync::atomic::AtomicU64::new(0)),
    };

    let earshot_engine =
        vox_lib::services::vad::earshot_vad::EarshotVadEngine::new(vad_config.initial_threshold)
            .expect("Failed to create EarshotVadEngine");
    let vad_backend = vox_lib::services::vad::VadBackend::Earshot(earshot_engine);

    let vad_handle = std::thread::Builder::new()
        .name("bench-vad-actor".to_string())
        .spawn(move || {
            let _ = spawn_vad_actor(vad_backend, consumer, vad_channels, vad_handles, vad_config);
        })
        .expect("Failed to spawn VAD actor thread");

    let mem_after_init = get_process_memory_mb();
    println!(
        "Process Memory (RSS) post-initialization: ~{} MB",
        mem_after_init
    );

    let mut clip_results = Vec::new();

    for clip in clips {
        let stream_start = Instant::now();
        let mut partials_count = 0;
        let mut final_utterances: Vec<String> = Vec::new();

        // 1. Stream 256-sample chunks to VAD ring buffer
        let mut speech_ends_seen = 0;

        for chunk in clip.audio_samples.chunks(VAD_CHUNK_SIZE) {
            let mut padded = chunk.to_vec();
            if padded.len() < VAD_CHUNK_SIZE {
                padded.resize(VAD_CHUNK_SIZE, 0.0);
            }
            while producer.vacant_len() < padded.len() {
                std::thread::sleep(Duration::from_millis(1));
            }
            producer.push_slice(&padded);
            std::thread::sleep(Duration::from_millis(1));

            // Poll intermediate events
            while let Ok(event) = vox_event_rx.try_recv() {
                if let VoxEvent::SpeechEnd { .. } = event {
                    speech_ends_seen += 1;
                }
            }
            while let Ok(event) = pipeline_event_rx.try_recv() {
                match event {
                    VoxEvent::TranscriptPartial { .. } => partials_count += 1,
                    VoxEvent::TranscriptFinal { text, .. } => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            final_utterances.push(trimmed.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        // 2. Stream silence frames to trigger SpeechEnd
        let silence = vec![0.0f32; VAD_CHUNK_SIZE];
        for _ in 0..(VAD_SPEECH_END_FRAMES + 25) {
            while producer.vacant_len() < silence.len() {
                std::thread::sleep(Duration::from_millis(1));
            }
            producer.push_slice(&silence);
            std::thread::sleep(Duration::from_millis(1));

            while let Ok(event) = vox_event_rx.try_recv() {
                if let VoxEvent::SpeechEnd { .. } = event {
                    speech_ends_seen += 1;
                }
            }
            while let Ok(event) = pipeline_event_rx.try_recv() {
                match event {
                    VoxEvent::TranscriptPartial { .. } => partials_count += 1,
                    VoxEvent::TranscriptFinal { text, .. } => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            final_utterances.push(trimmed.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        // Wait for buffer to drain
        let drain_deadline = Instant::now() + Duration::from_secs(5);
        while producer.occupied_len() > 0 && Instant::now() < drain_deadline {
            std::thread::sleep(Duration::from_millis(5));
        }

        let audio_feed_end = Instant::now();

        // 3. Track speech ends and collect all final transcripts
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut last_activity = Instant::now();

        while Instant::now() < deadline {
            // Poll VAD actor events
            while let Ok(event) = vox_event_rx.recv_timeout(Duration::from_millis(50)) {
                last_activity = Instant::now();
                if let VoxEvent::SpeechEnd { .. } = event {
                    speech_ends_seen += 1;
                }
            }

            // Poll STT worker events
            while let Ok(event) = pipeline_event_rx.try_recv() {
                last_activity = Instant::now();
                match event {
                    VoxEvent::TranscriptPartial { .. } => partials_count += 1,
                    VoxEvent::TranscriptFinal { text, .. } => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            final_utterances.push(trimmed.to_string());
                        }
                    }
                    _ => {}
                }
            }

            // Exit when we received final transcripts for all speech ends and activity settled
            if speech_ends_seen > 0
                && final_utterances.len() >= speech_ends_seen
                && last_activity.elapsed() > Duration::from_millis(500)
            {
                break;
            }

            // Fallback timeout if all speech ends finished and no new events for >2s
            if !final_utterances.is_empty() && last_activity.elapsed() > Duration::from_millis(2000)
            {
                break;
            }
        }

        let final_transcript = final_utterances.join(" ");

        // Clean isolation between clips: ensure queues are fully empty before next clip
        std::thread::sleep(Duration::from_millis(100));
        while vox_event_rx.try_recv().is_ok() {}
        while pipeline_event_rx.try_recv().is_ok() {}
        let _ = stt_tx.send(SttCommand::ResetStream);
        std::thread::sleep(Duration::from_millis(50));

        let total_stream_time = stream_start.elapsed();
        let final_post_speech_latency = audio_feed_end.elapsed();
        let rtf = if clip.duration_s > 0.0 {
            total_stream_time.as_secs_f64() / (clip.duration_s as f64)
        } else {
            0.0
        };
        let throughput = if total_stream_time.as_secs_f64() > 0.0 {
            clip.audio_samples.len() as f64 / total_stream_time.as_secs_f64()
        } else {
            0.0
        };
        let similarity = if clip.expected_text.is_empty() {
            1.0
        } else {
            levenshtein_similarity(&final_transcript, &clip.expected_text)
        };

        println!(
            "[{}] {:<24} | Aud: {:>4.2}s | Stream: {:>6.2}s | FinalPost: {:>6.0}ms | RTF: {:>5.3}x | Partials: {:>2} | Sim: {:>5.1}%",
            clip.lang,
            clip.filename,
            clip.duration_s,
            total_stream_time.as_secs_f64(),
            final_post_speech_latency.as_secs_f64() * 1000.0,
            rtf,
            partials_count,
            similarity * 100.0
        );

        clip_results.push(ClipBenchmarkResult {
            filename: clip.filename.clone(),
            lang: clip.lang.clone(),
            duration_s: clip.duration_s,
            total_stream_time_ms: total_stream_time.as_secs_f64() * 1000.0,
            final_post_speech_latency_ms: final_post_speech_latency.as_secs_f64() * 1000.0,
            rtf,
            throughput_spl_s: throughput,
            partials_emitted: partials_count,
            similarity,
            hypothesis: final_transcript,
            ground_truth: clip.expected_text.clone(),
        });

        // Drain leftover events
        while vox_event_rx.recv_timeout(Duration::from_millis(50)).is_ok() {}
        while pipeline_event_rx.try_recv().is_ok() {}
    }

    // Teardown
    let _ = vad_cmd_tx.send(VadCommand::Shutdown);
    let _ = stt_tx.send(SttCommand::Shutdown);
    engine_shutdown.store(true, Ordering::Relaxed);
    let _ = vad_handle.join();
    let _ = stt_handle.join();

    let total_audio_s: f32 = clip_results.iter().map(|r| r.duration_s).sum();
    let total_stream_ms: f64 = clip_results.iter().map(|r| r.total_stream_time_ms).sum();
    let count = clip_results.len().max(1);
    let avg_final_post_latency_ms = clip_results
        .iter()
        .map(|r| r.final_post_speech_latency_ms)
        .sum::<f64>()
        / count as f64;
    let avg_rtf = clip_results.iter().map(|r| r.rtf).sum::<f64>() / count as f64;
    let avg_sim = clip_results.iter().map(|r| r.similarity).sum::<f64>() / count as f64;
    let total_samples: usize = (total_audio_s * 16000.0) as usize;
    let overall_throughput = if total_stream_ms > 0.0 {
        total_samples as f64 / (total_stream_ms / 1000.0)
    } else {
        0.0
    };

    println!("\n--- Overall Streaming Summary for {} ---", engine_name);
    println!("Total Audio Processed     : {:.2}s", total_audio_s);
    println!(
        "Total Stream Elapsed      : {:.2}s",
        total_stream_ms / 1000.0
    );
    println!(
        "Avg Post-Speech Final Latency: {:.1}ms",
        avg_final_post_latency_ms
    );
    println!("Average Streaming RTF     : {:.3}x", avg_rtf);
    println!(
        "Streaming Audio Throughput: {:.0} samples/s ({:.2}x real-time)",
        overall_throughput,
        if avg_rtf > 0.0 { 1.0 / avg_rtf } else { 0.0 }
    );
    println!("Average Character Accuracy: {:.1}%", avg_sim * 100.0);
    println!("Active Working Set Memory : ~{} MB RSS", mem_after_init);

    EngineBenchmarkRun {
        engine_name: engine_name.to_string(),
        model_type: model_type.to_string(),
        model_path: model_path.to_string(),
        memory_rss_mb: mem_after_init,
        total_audio_s,
        total_stream_time_ms: total_stream_ms,
        avg_post_speech_latency_ms: avg_final_post_latency_ms,
        avg_rtf,
        overall_throughput_spl_s: overall_throughput,
        avg_similarity: avg_sim,
        clips: clip_results,
    }
}
