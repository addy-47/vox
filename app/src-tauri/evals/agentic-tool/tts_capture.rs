//! ============================================================================
//! evals/agentic-tool/tts_capture.rs — Real TTS With Device-Free PCM Capture
//! ============================================================================
//! Category     : Evaluation
//! Component    : evals agentic-tool harness (audio side)
//! Prerequisites: a local Kokoro model under `~/.vox/models/`
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : per-job intent, duration, sample count, RMS; WAV artifacts
//!
//! ## Why this exists
//!
//! A voice assistant's filler is part of the product, so a tool eval that asserts
//! only on text is evaluating half of it. This runs the *real* TTS worker and the
//! *real* playback engine, then keeps the PCM so a WAV lands on disk.
//!
//! ## How PCM is captured without an audio device
//!
//! `PlaybackEngine` writes upsampled 48 kHz samples into a `HeapProd<f32>` ring
//! buffer that a CPAL stream normally drains. `from_parts` accepts
//! `stream: Option<cpal::Stream>`, so this module creates the ring buffer itself:
//!
//! ```text
//! HeapRb::new(PLAYBACK_BUFFER_SAMPLES).split() -> (HeapProd, HeapCons)
//! HeapProd  -> PlaybackEngine::from_parts(.., stream: None)
//! HeapCons  -> retained here, drained after the run
//! ```
//!
//! All production behaviour still runs: 24 kHz to 48 kHz upsampling, pre-roll
//! cushioning, `PlaybackStarted`/`PlaybackFinished` gating, overflow warnings.
//! Only the sound card is absent. The buffer is 30 s at 48 kHz (1.44 M samples)
//! versus roughly 264 k for a filler plus a short response, so nothing is dropped
//! and no concurrent drain thread is needed.

use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
// `split()` hands back a cached consumer whose required method is `try_pop`.
use ringbuf::{
    traits::{Consumer, Split},
    HeapRb,
};
use serde::Serialize;
use vox_lib::{
    core::events::{AudioIntent, VoxEvent},
    services::{
        audio::{
            playback::{PlaybackEngine, PlaybackEngineHandles},
            PLAYBACK_BUFFER_SAMPLES,
        },
        tts::{
            actor::TtsWorkerHandles, config::ProviderCaps, factory::create_tts_provider,
            providers::SynthesisContext, spawn_tts_worker, TtsActiveProvider, TtsCommand,
            TtsProvider, TtsSettings,
        },
    },
};

use crate::common::stage_dump::{self, names};

/// Sample rate of captured PCM. The playback engine upsamples to this before
/// pushing, so the consumer side is already at 48 kHz.
pub const CAPTURE_SAMPLE_RATE: u32 = 48_000;

/// Live TTS capture wired into the harness.
#[allow(dead_code)]
pub struct TtsCapture {
    /// Channel the harness sends `TtsCommand`s to.
    pub tts_tx: mpsc::Sender<TtsCommand>,
    /// The device-free playback engine the real TTS worker writes into.
    pub playback: Arc<PlaybackEngine>,
    /// Pending-job counter the harness reads to know when the turn settled.
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    cancel_flag: Arc<AtomicBool>,
    consumer: Arc<Mutex<ringbuf::HeapCons<f32>>>,
    render_log: Arc<Mutex<Vec<RenderRecord>>>,
}

/// Builds the real TTS provider for the eval.
///
/// Defaults to Kokoro because it is local ONNX, needs no network, and loads fast
/// enough to sit inside a per-case budget.
#[allow(dead_code)]
pub fn build_provider(settings: &TtsSettings) -> Result<Box<dyn TtsProvider>> {
    let mut s = settings.clone();
    s.active = TtsActiveProvider::Kokoro;
    let model_dir = vox_lib::utils::paths::model_dir("");
    create_tts_provider(&s, &model_dir, None)
        .map_err(|e| anyhow!("Failed to initialize Kokoro for eval capture: {}", e))
}

/// Starts the real TTS worker against a device-free playback engine.
pub fn start(settings: &TtsSettings, event_tx: mpsc::Sender<VoxEvent>) -> Result<TtsCapture> {
    let recorder = RecordingProvider::new(build_provider(settings)?);
    let render_log = recorder.log();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let pending_synthesis_jobs = Arc::new(AtomicU32::new(0));
    let current_turn_id = Arc::new(AtomicU32::new(0));
    let playback_intent = Arc::new(AtomicU8::new(0));
    let is_playback_muted = Arc::new(AtomicBool::new(false));
    let state_atomic = Arc::new(AtomicU32::new(0));
    let telemetry_rtf = Arc::new(AtomicU32::new(0f32.to_bits()));

    // The split is the whole trick: we keep the consumer, the engine gets the producer.
    let rb = HeapRb::<f32>::new(PLAYBACK_BUFFER_SAMPLES);
    let (producer, consumer) = rb.split();

    let playback = Arc::new(PlaybackEngine::from_parts(
        producer,
        PlaybackEngineHandles {
            cancel_flag: Arc::clone(&cancel_flag),
            state_atomic,
            current_turn_id: Arc::clone(&current_turn_id),
            pending_synthesis_jobs: Arc::clone(&pending_synthesis_jobs),
            event_tx: event_tx.clone(),
            playback_intent: Arc::clone(&playback_intent),
            is_playback_muted,
            turn_metrics: None,
        },
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
        None, // no audio device
    ));

    let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
    // Same inline-loop convention as the LLM worker: `spawn_tts_worker` blocks on
    // `rx.recv()`, so it gets the same dedicated thread production gives it.
    let worker_playback = Arc::clone(&playback);
    let worker_cancel = Arc::clone(&cancel_flag);
    let worker_pending = Arc::clone(&pending_synthesis_jobs);
    std::thread::Builder::new()
        .name("eval-tts-worker".to_string())
        .spawn(move || {
            spawn_tts_worker(
                tts_rx,
                Box::new(recorder),
                TtsWorkerHandles {
                    playback: worker_playback,
                    event_tx,
                    cancel_flag: worker_cancel,
                    pending_synthesis_jobs: Some(worker_pending),
                    telemetry_rtf: Some(telemetry_rtf),
                    turn_metrics: None,
                },
            );
        })
        .map_err(|e| anyhow::anyhow!("Failed to spawn eval TTS worker thread: {e}"))?;

    Ok(TtsCapture {
        tts_tx,
        playback,
        pending_synthesis_jobs,
        cancel_flag,
        consumer: Arc::new(Mutex::new(consumer)),
        render_log,
    })
}

/// Snapshot of the recorded synthesis jobs.
pub fn render_log(cap: &TtsCapture) -> Vec<RenderRecord> {
    cap.render_log.lock().clone()
}

/// Blocks until the queue drains, or `timeout` elapses.
///
/// Returns `true` when drained. `false` means jobs were still in flight, so any
/// PCM captured afterwards would be a truncation.
pub fn drain_until_idle(cap: &TtsCapture, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while cap.pending_synthesis_jobs.load(Ordering::Relaxed) > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    cap.pending_synthesis_jobs.load(Ordering::Relaxed) == 0
}

/// Signals the worker to exit and waits for it.
pub fn shutdown(cap: &TtsCapture) {
    cap.cancel_flag.store(true, Ordering::Relaxed);
    let _ = cap.tts_tx.send(TtsCommand::Shutdown);
}

/// Pulls every sample the engine produced and writes mono 16-bit WAVs.
///
/// One WAV per contiguous burst of samples, so a filler and a response stay
/// separate files. Returns the paths written.
pub fn harvest(case_dir: &Path, cap: &TtsCapture) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(case_dir)?;

    // `parking_lot::MutexGuard` does not forward the ringbuf traits, so the guard
    // is dereferenced before the consumer methods are called.
    let mut samples: Vec<f32> = Vec::new();
    {
        let mut guard = cap.consumer.lock();
        let consumer = &mut *guard;
        while let Some(s) = consumer.try_pop() {
            samples.push(s);
        }
    }

    if samples.is_empty() {
        return Ok(Vec::new());
    }

    // Split on silent gaps so each utterance becomes its own clip. A 200 ms gap at
    // 48 kHz is well beyond natural inter-clause silence and far below the silence
    // between a filler and the response.
    const GAP_SAMPLES: usize = CAPTURE_SAMPLE_RATE as usize / 5;
    let bursts = split_on_gap(&samples, GAP_SAMPLES);

    // E2: per-clip measured durations (from the WAV samples themselves, not from
    // synthesis wall time). `render_log.duration_ms` is synthesis latency; audio
    // length is measured here. Identical summaries across cases mean identical
    // synthesized text (see E1), not a harvest bug — this summary is per case.
    let mut written = Vec::new();
    let mut clip_entries = Vec::new();
    for (i, burst) in bursts.iter().enumerate() {
        if burst.len() < 100 {
            continue; // ignore clicks
        }
        let rms = root_mean_square(burst);
        if rms < 1e-4 {
            continue; // ignore silence
        }
        let path = case_dir.join(format!("tts_{:02}.wav", i));
        write_wav_mono16(&path, burst, CAPTURE_SAMPLE_RATE)?;
        clip_entries.push(serde_json::json!({
            "file": path.file_name(),
            "samples": burst.len(),
            "seconds": burst.len() as f64 / CAPTURE_SAMPLE_RATE as f64,
            "rms": rms,
        }));
        written.push(path);
    }

    let summary = serde_json::json!({
        "total_samples": samples.len(),
        "total_seconds": samples.len() as f64 / CAPTURE_SAMPLE_RATE as f64,
        "clips": written.len(),
        "clips_detail": clip_entries,
        "rms": root_mean_square(&samples),
        "peak": samples.iter().fold(0.0f32, |a, b| a.max(b.abs())),
        "buffer_overflow_risk_samples": PLAYBACK_BUFFER_SAMPLES,
    });
    stage_dump::write_json(case_dir, names::AUDIO_SUMMARY, &summary)?;

    Ok(written)
}

/// Records the TTS job log alongside the clips.
pub fn persist_render_log(case_dir: &Path, records: &[RenderRecord]) -> Result<PathBuf> {
    stage_dump::write_json(case_dir, names::RENDER_LOG, &serde_json::to_value(records)?)
}

/// Root-mean-square amplitude, used to prove a clip is real speech rather than
/// silence or a click.
pub fn root_mean_square(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum = samples.iter().map(|s| s * s).sum::<f32>();
    (sum / samples.len() as f32).sqrt()
}

fn split_on_gap(samples: &[f32], gap: usize) -> Vec<Vec<f32>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < samples.len() {
        let quiet = samples[i].abs() < 1e-4;
        if quiet {
            let run_end = samples[i..]
                .iter()
                .position(|s| s.abs() >= 1e-4)
                .map(|o| i + o)
                .unwrap_or(samples.len());
            if run_end - i >= gap {
                out.push(samples[start..i].to_vec());
                start = run_end;
                i = run_end;
                continue;
            }
            i = run_end;
        } else {
            i += 1;
        }
    }
    if start < samples.len() {
        out.push(samples[start..].to_vec());
    }
    out
}

fn write_wav_mono16(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let n = samples.len();
    let data_bytes = n * 2;
    let mut bytes: Vec<u8> = Vec::with_capacity(44 + data_bytes);

    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_bytes) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&v.to_le_bytes());
    }

    std::fs::write(path, bytes).with_context(|| format!("Failed to write WAV {path:?}"))?;
    Ok(())
}

/// One synthesis job as the eval observed it.
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct RenderRecord {
    pub turn_id: u32,
    /// `interim_filler` or `turn_response`.
    pub intent: String,
    /// Exact text handed to synthesis.
    pub text: String,
    pub chars: usize,
    pub words: usize,
    /// Synthesis wall-clock latency in ms. This is NOT the audio length — audio
    /// duration is measured from WAV samples in `audio_summary.json`
    /// (`clips_detail[].seconds`). Comparing this field to audio length is a
    /// category error (QA §4.6).
    pub duration_ms: u64,
    pub ok: bool,
    pub error: Option<String>,
}

/// Wraps the real provider to record every job, then delegates unchanged.
///
/// The production worker only logs job boundaries; wrapping the provider is the
/// least invasive way to capture what was actually synthesized while keeping all
/// production synthesis behaviour.
pub struct RecordingProvider {
    inner: Box<dyn TtsProvider>,
    log: Arc<Mutex<Vec<RenderRecord>>>,
}

impl RecordingProvider {
    pub fn new(inner: Box<dyn TtsProvider>) -> Self {
        Self {
            inner,
            log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn log(&self) -> Arc<Mutex<Vec<RenderRecord>>> {
        Arc::clone(&self.log)
    }
}

impl TtsProvider for RecordingProvider {
    /// `TtsProvider::caps` is an associated function gated on `Self: Sized`, so it
    /// cannot be reached through a `dyn` box. The wrapper therefore reports the
    /// configured voice-source capability it was built with rather than
    /// panicking; the eval never consumes caps, and the frontend path is not in
    /// scope here.
    fn caps() -> ProviderCaps
    where
        Self: Sized,
    {
        ProviderCaps {
            voices: vox_lib::services::tts::config::TtsVoiceSource::Catalog,
            clone: false,
            speed_range: vox_lib::services::tts::ParamRange {
                min: 0.5,
                max: 2.0,
                step: 0.05,
            },
        }
    }

    fn synthesize_chunk(&self, text: &str, ctx: &SynthesisContext<'_>) -> anyhow::Result<()> {
        let start = Instant::now();
        let res = self.inner.synthesize_chunk(text, ctx);
        self.log.lock().push(RenderRecord {
            turn_id: ctx.turn_id,
            intent: intent_label(&ctx.intent).to_string(),
            text: text.to_string(),
            chars: text.chars().count(),
            words: text.split_whitespace().count(),
            duration_ms: start.elapsed().as_millis() as u64,
            ok: res.is_ok(),
            error: res.as_ref().err().map(|e| e.to_string()),
        });
        res
    }

    fn set_speed(&self, s: f32) {
        self.inner.set_speed(s)
    }

    fn set_voice(&self, v: i32) {
        self.inner.set_voice(v)
    }

    fn health_check(&self) -> bool {
        self.inner.health_check()
    }
}

/// Intent label used in artifacts.
pub fn intent_label(intent: &AudioIntent) -> &'static str {
    match intent {
        AudioIntent::InterimFiller => "interim_filler",
        AudioIntent::TurnResponse => "turn_response",
    }
}
