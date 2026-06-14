use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thread_priority::*;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteMode {
    LocalVad = 0,
    DirectRealtime = 1,
}

pub enum RouterCommand {
    SetMode(RouteMode),
    StartRealtime(UnboundedSender<Vec<i16>>),
    StopRealtime,
}

pub struct AudioRouter {
    cmd_tx: std::sync::mpsc::Sender<RouterCommand>,
    _thread_handle: std::thread::JoinHandle<()>,
}

impl AudioRouter {
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
                // Elevate priority of audio router to prevent dropouts/underruns
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

                loop {
                    // Check shutdown flag
                    if engine_shutdown.load(Ordering::Relaxed) {
                        log::info!("[AudioRouter] Shutdown flag detected. Exiting loop.");
                        break;
                    }

                    // Process commands
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        match cmd {
                            RouterCommand::SetMode(m) => {
                                log::info!("[AudioRouter] Mode switched to {:?}", m);
                                mode = m;
                            }
                            RouterCommand::StartRealtime(tx) => {
                                log::info!("[AudioRouter] Routing directly to Realtime.");
                                realtime_tx = Some(tx);
                            }
                            RouterCommand::StopRealtime => {
                                log::info!("[AudioRouter] Realtime routing stopped.");
                                realtime_tx = None;
                            }
                        }
                    }

                    // Consume CPAL mic samples
                    if consumer.occupied_len() >= 256 {
                        consumer.pop_slice(&mut chunk);

                        // If paused, drop the audio chunks immediately
                        if is_paused.load(Ordering::SeqCst) {
                            continue;
                        }

                        match mode {
                            RouteMode::LocalVad => {
                                // Feed VAD thread's ring buffer
                                let pushed = vad_producer.push_slice(&chunk);
                                if pushed < chunk.len() {
                                    static OVERFLOW_COUNT: std::sync::atomic::AtomicU32 =
                                        std::sync::atomic::AtomicU32::new(0);
                                    let prev = OVERFLOW_COUNT.fetch_add(1, Ordering::Relaxed);
                                    if prev % 100 == 0 {
                                        log::warn!(
                                            "[AudioRouter] VAD queue overflow! Dropped {} chunks.",
                                            prev + 1
                                        );
                                    }
                                }
                            }
                            RouteMode::DirectRealtime => {
                                // Convert f32 chunk to i16 and stream directly to WS sender
                                if let Some(ref tx) = realtime_tx {
                                    let i16_samples: Vec<i16> = chunk
                                        .iter()
                                        .map(|&x| {
                                            let clamped = x.clamp(-1.0, 1.0);
                                            (clamped * 32767.0) as i16
                                        })
                                        .collect();
                                    // Try sending, drop chunk if full
                                    if let Err(e) = tx.send(i16_samples) {
                                        log::debug!(
                                            "[AudioRouter] Failed to send PCM to WS: {:?}",
                                            e
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
            })?;

        Ok(Self {
            cmd_tx,
            _thread_handle: thread_handle,
        })
    }

    pub fn set_mode(&self, mode: RouteMode) {
        let _ = self.cmd_tx.send(RouterCommand::SetMode(mode));
    }

    pub fn start_realtime(&self, tx: UnboundedSender<Vec<i16>>) {
        let _ = self.cmd_tx.send(RouterCommand::StartRealtime(tx));
    }

    pub fn stop_realtime(&self) {
        let _ = self.cmd_tx.send(RouterCommand::StopRealtime);
    }
}
