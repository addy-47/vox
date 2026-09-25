//! ============================================================================
//! audio.rs — Shared Eval Audio Capture & TTS Worker Lifecycle
//! ============================================================================
//! Category     : Evaluation Common Helper
//! Component    : evals/common/audio.rs
//! Prerequisites: None
//! ============================================================================

use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

use anyhow::Result;
use parking_lot::Mutex;
use ringbuf::{traits::*, HeapRb};
use vox_lib::{
    core::events::VoxEvent,
    services::{
        audio::{
            playback::{PlaybackEngine, PlaybackEngineHandles},
            PLAYBACK_BUFFER_SAMPLES,
        },
        tts::{
            actor::{spawn_tts_worker, TtsCommand, TtsWorkerHandles},
            TtsProvider,
        },
    },
};

/// Handles to inspect and control audio capture during an evaluation turn.
pub struct AudioCaptureHandles {
    pub tts_tx: mpsc::Sender<TtsCommand>,
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    pub cancel_flag: Arc<AtomicBool>,
    pub captured_samples: Arc<Mutex<Vec<f32>>>,
    pub playback: Arc<PlaybackEngine>,
}

/// Spawns a dedicated audio capture pipeline with PlaybackEngine ringbuffer,
/// background sample collector, and persistent TTS worker thread.
pub fn setup_audio_capture(
    tts_provider: Box<dyn TtsProvider>,
    event_tx: mpsc::Sender<VoxEvent>,
) -> AudioCaptureHandles {
    let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
    let pending_synthesis_jobs = Arc::new(AtomicU32::new(0));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let state_atomic = Arc::new(AtomicU32::new(0));

    let playback_rb = HeapRb::<f32>::new(PLAYBACK_BUFFER_SAMPLES);
    let (pb_prod, mut pb_cons) = playback_rb.split();
    let pb_handles = PlaybackEngineHandles {
        cancel_flag: Arc::clone(&cancel_flag),
        state_atomic,
        current_turn_id: Arc::new(AtomicU32::new(1)),
        pending_synthesis_jobs: Arc::clone(&pending_synthesis_jobs),
        event_tx: event_tx.clone(),
        playback_intent: Arc::new(AtomicU8::new(0)),
        is_playback_muted: Arc::new(AtomicBool::new(false)),
        turn_metrics: None,
    };
    let playback_engine = PlaybackEngine::from_parts(
        pb_prod,
        pb_handles,
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
        None,
    );
    let playback_arc = Arc::new(playback_engine);

    let captured_samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = Arc::clone(&captured_samples);
    let playback_cancel = Arc::clone(&cancel_flag);
    std::thread::Builder::new()
        .name("vox-eval-audio-drain".to_string())
        .spawn(move || {
            let mut buffer = [0.0f32; 1024];
            while !playback_cancel.load(Ordering::Relaxed) {
                let read = pb_cons.pop_slice(&mut buffer);
                if read > 0 {
                    captured_clone.lock().extend_from_slice(&buffer[..read]);
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            // Drain remaining on exit
            let mut drain_buf = [0.0f32; 1024];
            loop {
                let read = pb_cons.pop_slice(&mut drain_buf);
                if read == 0 {
                    break;
                }
                captured_clone.lock().extend_from_slice(&drain_buf[..read]);
            }
        })
        .expect("Failed to spawn audio drain thread");

    let tts_handles = TtsWorkerHandles {
        playback: Arc::clone(&playback_arc),
        event_tx,
        cancel_flag: Arc::clone(&cancel_flag),
        pending_synthesis_jobs: Some(Arc::clone(&pending_synthesis_jobs)),
        telemetry_rtf: None,
        turn_metrics: None,
    };
    std::thread::Builder::new()
        .name("vox-eval-tts-worker".to_string())
        .spawn(move || {
            spawn_tts_worker(tts_rx, tts_provider, tts_handles);
        })
        .expect("Failed to spawn TTS worker thread");

    AudioCaptureHandles {
        tts_tx,
        pending_synthesis_jobs,
        cancel_flag,
        captured_samples,
        playback: playback_arc,
    }
}

/// Serializes captured audio samples to a 32-bit float WAV file at the given sample rate.
pub fn save_wav_file(path: &Path, samples: &[f32], sample_rate: u32) -> Result<f64> {
    if samples.is_empty() {
        return Ok(0.0);
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for sample in samples {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(samples.len() as f64 / sample_rate as f64)
}
