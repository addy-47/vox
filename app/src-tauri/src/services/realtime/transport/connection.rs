use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use crate::core::state::InteractionState;
use crate::services::realtime::{OutboundCommand, RealtimeProviderEvent, BRIDGE_CHANNEL_CAPACITY};

use super::{
    FrameAction, HarnessConfig, HarnessHandles, HarnessInit, ProviderDriver, WsReader, WsWriter,
};

/// Context forwarded to each `(write_task, receiver_task)` pair for a single connection lifetime.
struct ConnectionContext {
    event_tx: mpsc::Sender<RealtimeProviderEvent>,
    state_rx: tokio::sync::watch::Receiver<InteractionState>,
    reconnect_notifier: tokio::sync::oneshot::Sender<()>,
}

/// Spawns the shared duplex WebSocket reconnect harness for a realtime provider session.
pub(crate) fn spawn_harness<D: ProviderDriver>(
    driver: Arc<D>,
    config: HarnessConfig,
    init: HarnessInit,
) -> HarnessHandles {
    let (outbound_tx, outbound_rx) = mpsc::channel::<OutboundCommand>(BRIDGE_CHANNEL_CAPACITY);
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let terminated = Arc::new(AtomicBool::new(false));
    let terminated_clone = terminated.clone();

    let ws_sender: Arc<Mutex<Option<mpsc::Sender<Message>>>> = Arc::new(Mutex::new(None));

    let outbound_task = init.tokio_handle.spawn(run_outbound_encoder(
        outbound_rx,
        ws_sender.clone(),
        driver.clone(),
    ));

    let keepalive_task: Option<JoinHandle<()>> = driver.keepalive_interval().map(|interval| {
        let ka_tx = outbound_tx.clone();
        init.tokio_handle.spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                if ka_tx.send(OutboundCommand::KeepAlive).await.is_err() {
                    break;
                }
            }
        })
    });

    let HarnessInit {
        ws_write,
        ws_read,
        reconnect_fn,
        provider_event_tx,
        state_rx,
        turn_id_ref,
        tokio_handle,
    } = init;

    tokio_handle.spawn(async move {
        let (first_reconnect_tx, mut reconnect_rx) = tokio::sync::oneshot::channel::<()>();
        let ctx = ConnectionContext {
            event_tx: provider_event_tx.clone(),
            state_rx: state_rx.clone(),
            reconnect_notifier: first_reconnect_tx,
        };
        let (mut write_handle, mut recv_handle) =
            spawn_connection_tasks(ws_write, ws_read, ws_sender.clone(), &driver, ctx);

        'reconnect: loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    log::info!("[RealtimeHarness] Shutdown requested. Aborting connection tasks.");
                    *ws_sender.lock() = None;
                    write_handle.abort();
                    recv_handle.abort();
                    break 'reconnect;
                }
                _ = &mut reconnect_rx => {
                    log::warn!("[RealtimeHarness] Connection dropped. Entering reconnect cycle.");
                    *ws_sender.lock() = None;
                    write_handle.abort();
                    recv_handle.abort();
                }
            }

            let mut reconnected = false;
            for attempt in 0..config.max_reconnect_attempts {
                let delay = Duration::from_secs(
                    config.reconnect_factor_secs * attempt as u64
                        + config.reconnect_base_delay_secs,
                );
                tokio::time::sleep(delay).await;
                log::info!(
                    "[RealtimeHarness] Reconnect attempt {}/{}...",
                    attempt + 1,
                    config.max_reconnect_attempts
                );
                match reconnect_fn().await {
                    Ok((new_write, new_read)) => {
                        let (new_reconnect_tx, new_reconnect_rx) =
                            tokio::sync::oneshot::channel::<()>();
                        reconnect_rx = new_reconnect_rx;
                        let ctx = ConnectionContext {
                            event_tx: provider_event_tx.clone(),
                            state_rx: state_rx.clone(),
                            reconnect_notifier: new_reconnect_tx,
                        };
                        let (wh, rh) = spawn_connection_tasks(
                            new_write,
                            new_read,
                            ws_sender.clone(),
                            &driver,
                            ctx,
                        );
                        write_handle = wh;
                        recv_handle = rh;
                        reconnected = true;
                        log::info!("[RealtimeHarness] Reconnected successfully.");
                        break;
                    }
                    Err(e) => {
                        log::error!(
                            "[RealtimeHarness] Reconnect attempt {} failed: {:?}",
                            attempt + 1,
                            e
                        );
                    }
                }
            }

            if !reconnected {
                log::error!(
                    "[RealtimeHarness] Max reconnect attempts ({}) reached. Terminating session.",
                    config.max_reconnect_attempts
                );
                let tid = turn_id_ref.load(Ordering::Relaxed);
                if let Err(e) = provider_event_tx.try_send(RealtimeProviderEvent::Error {
                    turn_id: tid,
                    message: "Realtime connection permanently lost after max reconnect attempts."
                        .to_string(),
                }) {
                    log::warn!(
                        "[RealtimeHarness] Failed to emit terminal error event: {:?}",
                        e
                    );
                }
                terminated_clone.store(true, Ordering::SeqCst);
                *ws_sender.lock() = None;
                break 'reconnect;
            }
        }

        outbound_task.abort();
        if let Some(t) = keepalive_task {
            t.abort();
        }
    });

    HarnessHandles {
        outbound_tx,
        shutdown_tx: Mutex::new(Some(shutdown_tx)),
        terminated,
    }
}

/// Spawns a matched `(write_task, receiver_task)` pair for a single WebSocket connection lifetime.
fn spawn_connection_tasks<D: ProviderDriver>(
    ws_write: WsWriter,
    ws_read: WsReader,
    ws_sender: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
    driver: &Arc<D>,
    ctx: ConnectionContext,
) -> (JoinHandle<()>, JoinHandle<()>) {
    let (write_tx, write_rx) = mpsc::channel::<Message>(BRIDGE_CHANNEL_CAPACITY);
    *ws_sender.lock() = Some(write_tx);

    let write_handle = tokio::spawn(async move {
        let mut write_rx = write_rx;
        let mut ws_write = ws_write;
        while let Some(msg) = write_rx.recv().await {
            if let Err(e) = ws_write.send(msg).await {
                log::error!("[RealtimeHarness] WebSocket write error: {:?}", e);
                break;
            }
        }
    });

    let recv_driver = driver.clone();
    let ConnectionContext {
        event_tx,
        state_rx,
        reconnect_notifier,
    } = ctx;
    let recv_handle = tokio::spawn(async move {
        let mut ws_read = ws_read;
        let mut reconnect_tx_opt = Some(reconnect_notifier);
        while let Some(res) = ws_read.next().await {
            match res {
                Ok(Message::Close(cf)) => {
                    log::info!("[RealtimeHarness] WebSocket closed by server: {:?}", cf);
                    break;
                }
                Err(e) => {
                    log::error!("[RealtimeHarness] WebSocket read error: {:?}", e);
                    break;
                }
                Ok(msg) => match recv_driver.handle_frame(msg, &event_tx) {
                    FrameAction::Continue => {}
                    FrameAction::GoAway => {
                        log::warn!("[RealtimeHarness] GoAway received. Initiating reconnect.");
                        if *state_rx.borrow() == InteractionState::Paused {
                            break;
                        }
                        if let Some(tx) = reconnect_tx_opt.take() {
                            if tx.send(()).is_err() {
                                log::warn!(
                                    "[RealtimeHarness] Failed to signal reconnect (GoAway)."
                                );
                            }
                        }
                        break;
                    }
                },
            }
        }
        if *state_rx.borrow() == InteractionState::Paused {
            log::info!("[RealtimeHarness] WebSocket disconnected silently during Paused state.");
        } else if let Some(tx) = reconnect_tx_opt.take() {
            if tx.send(()).is_err() {
                log::warn!("[RealtimeHarness] Failed to signal reconnect on connection drop.");
            }
        }
    });

    (write_handle, recv_handle)
}

/// Reads `OutboundCommand` items from the session, encodes them via the driver, and forwards
/// to the active write-task channel. Survives reconnects via the shared `ws_sender` slot.
async fn run_outbound_encoder<D: ProviderDriver>(
    mut outbound_rx: mpsc::Receiver<OutboundCommand>,
    ws_sender: Arc<Mutex<Option<mpsc::Sender<Message>>>>,
    driver: Arc<D>,
) {
    while let Some(cmd) = outbound_rx.recv().await {
        let Some(msg) = driver.encode(cmd) else {
            continue;
        };
        let sender = ws_sender.lock().clone();
        let Some(tx) = sender else { continue };
        if let Err(e) = tx.try_send(msg) {
            log::warn!(
                "[RealtimeHarness] Outbound frame dropped (channel full or closed during reconnect): {:?}",
                e
            );
        }
    }
}
