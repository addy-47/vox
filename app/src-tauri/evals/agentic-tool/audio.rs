//! ============================================================================
//! evals/agentic-tool/audio.rs — Headless TTS command capture and lifecycle events
//! ============================================================================
//! Category     : Evaluation
//! Component    : evals harness
//! Prerequisites: see evals/README.md
//! Execution    : cargo bench --bench agentic_tool_eval --release -- --help
//! Metrics      : see summary.md in the run directory
//! ============================================================================

use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::Serialize;
use vox_lib::{
    core::events::{AudioIntent, VoxEvent},
    services::tts::TtsCommand,
};

#[allow(dead_code)]
pub const EVAL_SAMPLE_RATE: u32 = 48_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioMode {
    Silent,
    Real,
}

impl std::str::FromStr for AudioMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "silent" | "none" => Ok(AudioMode::Silent),
            "real" => Ok(AudioMode::Real),
            o => Err(format!(
                "Unknown audio mode '{}'. Expected 'silent' or 'real'.",
                o
            )),
        }
    }
}

pub struct AudioHandles {
    pub tts_tx: mpsc::Sender<TtsCommand>,
    pub pending_synthesis_jobs: Arc<AtomicU32>,
    pub cancel_flag: Arc<AtomicBool>,
    pub render_log: Arc<Mutex<Vec<RenderRecord>>>,
    /// Retained so the worker can be joined (§6.2) rather than detached.
    pub worker: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderRecord {
    pub turn_id: u32,
    pub intent: String,
    pub text: String,
    pub chars: usize,
    pub words: usize,
    pub duration_ms: u64,
    pub events_emitted: bool,
}

impl RenderRecord {
    pub fn estimated_speech_s(&self) -> f64 {
        if self.words == 0 {
            return 0.0;
        }
        self.words as f64 / 2.8 + 0.15
    }
}

/// The production worker owns synthesis and needs a PlaybackEngine, so the eval
/// records commands and emits the lifecycle events the pipeline needs to leave
/// `Working`. Hardware-independent and therefore reproducible.
pub fn setup_audio_capture(mode: AudioMode, pipeline_tx: mpsc::Sender<VoxEvent>) -> AudioHandles {
    let (tts_tx, tts_rx) = mpsc::channel::<TtsCommand>();
    let pending_synthesis_jobs = Arc::new(AtomicU32::new(0));
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let render_log = Arc::new(Mutex::new(Vec::new()));

    let worker_log = Arc::clone(&render_log);
    let worker_pending = Arc::clone(&pending_synthesis_jobs);
    let worker_cancel = Arc::clone(&cancel_flag);

    let worker = std::thread::spawn(move || {
        // Bounded recv (§6.1): an unbounded recv would park this thread forever when
        // no further commands arrive, and the join in `shutdown` would deadlock.
        loop {
            if worker_cancel.load(Ordering::Relaxed) {
                break;
            }
            let cmd = match tts_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(c) => c,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            match cmd {
                TtsCommand::Generate {
                    turn_id,
                    text,
                    intent,
                } => {
                    worker_pending.fetch_add(1, Ordering::SeqCst);
                    let start = Instant::now();

                    let _ = pipeline_tx.send(VoxEvent::PlaybackStarted { turn_id, intent });

                    let record = RenderRecord {
                        turn_id,
                        intent: intent_label(&intent).to_string(),
                        chars: text.chars().count(),
                        words: text.split_whitespace().count(),
                        text,
                        duration_ms: start.elapsed().as_millis() as u64,
                        events_emitted: true,
                    };
                    if mode == AudioMode::Real {
                        log::info!(
                            "[AudioCapture] recorded {} chars for turn {}",
                            record.chars,
                            turn_id
                        );
                    }

                    let _ = pipeline_tx.send(VoxEvent::PlaybackFinished { turn_id, intent });
                    worker_log.lock().push(record);
                    worker_pending.fetch_sub(1, Ordering::SeqCst);
                }
                TtsCommand::Shutdown => break,
                _ => {}
            }
        }
    });

    AudioHandles {
        tts_tx,
        pending_synthesis_jobs,
        cancel_flag,
        render_log,
        worker: Arc::new(Mutex::new(Some(worker))),
    }
}

/// `false` means jobs may still be in flight, so the render log is incomplete.
pub fn drain_until_idle(handles: &AudioHandles, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while handles.pending_synthesis_jobs.load(Ordering::Relaxed) > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
    handles.pending_synthesis_jobs.load(Ordering::Relaxed) == 0
}

/// Signals the worker, then joins it so a panic surfaces instead of passing silently (§6.2).
pub fn shutdown(handles: &AudioHandles) -> Result<()> {
    handles.cancel_flag.store(true, Ordering::Relaxed);
    let _ = handles.tts_tx.send(TtsCommand::Shutdown);
    let handle = handles.worker.lock().take();
    if let Some(h) = handle {
        h.join().map_err(|_| anyhow!("Audio capture worker panicked"))?;
    }
    Ok(())
}

#[allow(dead_code)]
pub fn save_wav_file(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(f64, usize)> {
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

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("Failed to create {parent:?}"))?;
    }
    std::fs::write(path, bytes).with_context(|| format!("Failed to write {path:?}"))?;
    Ok((n as f64 / sample_rate as f64, n))
}

pub fn intent_label(intent: &AudioIntent) -> &'static str {
    match intent {
        AudioIntent::InterimFiller => "interim_filler",
        AudioIntent::TurnResponse => "turn_response",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioSummary {
    pub job_count: usize,
    pub filler_count: usize,
    pub response_count: usize,
    pub total_chars: usize,
    pub filler_speech_s: f64,
    pub max_filler_chars: usize,
    pub overlong_fillers: usize,
}

#[allow(dead_code)]
pub fn summarize(records: &[RenderRecord]) -> AudioSummary {
    let mut s = AudioSummary {
        job_count: records.len(),
        filler_count: 0,
        response_count: 0,
        total_chars: 0,
        filler_speech_s: 0.0,
        max_filler_chars: 0,
        overlong_fillers: 0,
    };
    for r in records {
        s.total_chars += r.chars;
        match r.intent.as_str() {
            "interim_filler" => {
                s.filler_count += 1;
                s.filler_speech_s += r.estimated_speech_s();
                s.max_filler_chars = s.max_filler_chars.max(r.chars);
                if r.chars > 60 {
                    s.overlong_fillers += 1;
                }
            }
            "turn_response" => s.response_count += 1,
            _ => {}
        }
    }
    s
}
