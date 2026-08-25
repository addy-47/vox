use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thread_priority::*;
use tokio::sync::mpsc::UnboundedSender;

/// Target destination mode for audio ingestion routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteMode {
    LocalVad = 0,
    DirectRealtime = 1,
}

/// Commands accepted by the audio router background thread.
pub enum RouterCommand {
    SetMode(RouteMode),
    StartRealtime(UnboundedSender<Vec<i16>>),
    StopRealtime,
}

/// Dispatches raw microphone audio samples to either local VAD or realtime websocket.
pub struct AudioRouter {
    cmd_tx: std::sync::mpsc::Sender<RouterCommand>,
    _thread_handle: std::thread::JoinHandle<()>,
}

impl AudioRouter {
    /// Spawns the audio routing worker thread.
    pub fn spawn<C, P>(
        mut consumer: C,
        mut vad_producer: P,
        is_paused: Arc<AtomicBool>,
        engine_shutdown: Arc<AtomicBool>,
    ) -> Result<Self>
    where
        C: ringbuf::traits::Consumer<Item = f32> + Send + 'static,
        P: ringbuf::traits::Producer<Item = f32> + Send + 'static,
    {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<RouterCommand>();

        let thread_handle = std::thread::Builder::new()
            .name("vox-audio-router".to_string())
            .spawn(move || {
                if let Err(e) = set_current_thread_priority(ThreadPriority::Max) {
                    log::warn!(
                        "[AudioRouter] Failed to set max priority (non-root/cap_sys_nice): {:?}",
                        e
                    );
                }

                log::info!("[AudioRouter] Thread started.");

                let mut mode = RouteMode::LocalVad;
                let mut realtime_tx: Option<UnboundedSender<Vec<i16>>> = None;
                let mut chunk = vec![0.0f32; 256];

                while !engine_shutdown.load(Ordering::Relaxed) {
                    handle_router_commands(&cmd_rx, &mut mode, &mut realtime_tx);

                    if consumer.occupied_len() >= 256 {
                        consumer.pop_slice(&mut chunk);

                        if is_paused.load(Ordering::SeqCst) {
                            continue;
                        }

                        route_audio_chunk(&chunk, mode, &mut vad_producer, &realtime_tx);
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }

                log::info!("[AudioRouter] Shutdown flag detected. Exiting loop.");
            })?;

        Ok(Self {
            cmd_tx,
            _thread_handle: thread_handle,
        })
    }

    /// Sets the active routing destination mode.
    pub fn set_mode(&self, mode: RouteMode) {
        if let Err(e) = self.cmd_tx.send(RouterCommand::SetMode(mode)) {
            log::warn!("[AudioRouter] Failed to dispatch SetMode command: {}", e);
        }
    }

    /// Enables realtime websocket audio streaming.
    pub fn start_realtime(&self, tx: UnboundedSender<Vec<i16>>) {
        if let Err(e) = self.cmd_tx.send(RouterCommand::StartRealtime(tx)) {
            log::warn!(
                "[AudioRouter] Failed to dispatch StartRealtime command: {}",
                e
            );
        }
    }

    /// Disables realtime websocket audio streaming.
    pub fn stop_realtime(&self) {
        if let Err(e) = self.cmd_tx.send(RouterCommand::StopRealtime) {
            log::warn!(
                "[AudioRouter] Failed to dispatch StopRealtime command: {}",
                e
            );
        }
    }
}

/// Drains and applies pending router control commands from the command channel.
fn handle_router_commands(
    cmd_rx: &std::sync::mpsc::Receiver<RouterCommand>,
    mode: &mut RouteMode,
    realtime_tx: &mut Option<UnboundedSender<Vec<i16>>>,
) {
    while let Ok(cmd) = cmd_rx.try_recv() {
        match cmd {
            RouterCommand::SetMode(m) => {
                log::info!("[AudioRouter] Mode switched to {:?}", m);
                *mode = m;
            }
            RouterCommand::StartRealtime(tx) => {
                log::info!("[AudioRouter] Routing directly to Realtime.");
                *realtime_tx = Some(tx);
            }
            RouterCommand::StopRealtime => {
                log::info!("[AudioRouter] Realtime routing stopped.");
                *realtime_tx = None;
            }
        }
    }
}

/// Forwards an audio chunk to either the local VAD ring buffer or the realtime websocket sender.
fn route_audio_chunk<P>(
    chunk: &[f32],
    mode: RouteMode,
    vad_producer: &mut P,
    realtime_tx: &Option<UnboundedSender<Vec<i16>>>,
) where
    P: ringbuf::traits::Producer<Item = f32>,
{
    match mode {
        RouteMode::LocalVad => {
            let pushed = vad_producer.push_slice(chunk);
            if pushed < chunk.len() {
                static OVERFLOW_COUNT: std::sync::atomic::AtomicU32 =
                    std::sync::atomic::AtomicU32::new(0);
                let prev = OVERFLOW_COUNT.fetch_add(1, Ordering::Relaxed);
                if prev.is_multiple_of(100) {
                    log::warn!(
                        "[AudioRouter] VAD queue overflow! Dropped {} chunks.",
                        prev + 1
                    );
                }
            }
        }
        RouteMode::DirectRealtime => {
            if let Some(ref tx) = realtime_tx {
                let i16_samples: Vec<i16> = chunk
                    .iter()
                    .map(|&x| {
                        let clamped = x.clamp(-1.0, 1.0);
                        (clamped * 32767.0) as i16
                    })
                    .collect();

                if let Err(e) = tx.send(i16_samples) {
                    log::debug!("[AudioRouter] Failed to send PCM to WS: {:?}", e);
                }
            }
        }
    }
}
