use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Manager, Runtime};

use crate::{
    core::{
        events::{Severity, TaskStatus},
        state::{AppState, InteractionState},
    },
    harness::stages::inbox::{InboxMessage, QueuedMessageDelivery},
    pipeline::{transition, RoutingContext},
    services::notifications::{notify, Action, NotificationCategory, NotificationParams},
};

/// Handles incoming background task finished notifications according to the 9-state matrix.
pub fn on_task_finished<R: Runtime + 'static>(
    task_id: String,
    session_id: i64,
    status: TaskStatus,
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // 1. Asynchronously persist terminal task state to Turso DB (upsert to handle direct event dispatches)
    let db = Arc::clone(&state.db);
    let tid = task_id.clone();
    let stat = status.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(conn) = db.connect() {
            let _ = crate::persistence::tasks::record_task_finished(
                &conn,
                &tid,
                session_id,
                &stat,
            )
            .await;
        }
    });

    // 2. Route through the universal alert front door: OS toast + group-keyed drawer card.
    let notify_app = app.clone();
    let notify_db = Arc::clone(&state.db);
    let notify_task_id = task_id.clone();
    let notify_status = status.clone();
    tauri::async_runtime::spawn(async move {
        let title = format!("Task {}", notify_status.as_str());
        let message = format!(
            "Background task {} finished with status {}",
            notify_task_id,
            notify_status.as_str()
        );
        let group_key = format!("task:{}", notify_task_id);
        if let Err(e) = notify(
            &notify_app,
            &notify_db,
            NotificationParams {
                group_key: Some(&group_key),
                category: NotificationCategory::Task,
                severity: Severity::Info,
                impact: None,
                action: Action::Receipt,
                title: &title,
                message: &message,
                session_id: Some(session_id),
                metadata: None,
                duration_ms: None,
            },
        )
        .await
        {
            log::warn!(
                "[Pipeline::Tasks] Task notification delivery failed for {}: {}",
                notify_task_id,
                e
            );
        }
    });

    // 3. Evaluate 9-state handling matrix
    let current_state = state.pipeline.state();
    match current_state {
        InteractionState::Ready => {
            let last_activity = state.pipeline.last_user_activity_ms.load(Ordering::Relaxed);
            let is_active = now.saturating_sub(last_activity) < 5 * 60 * 1000;
            if !is_active {
                log::info!(
                    "[Pipeline::Tasks] Quiet window (>5m) in Ready; suppressing spoken announcement for task {}",
                    task_id
                );
                return;
            }

            let query = format!(
                "[Background Task Finished] task_id={} status={}",
                task_id,
                status.as_str()
            );

            // Single writer for Ready -> Thinking. Competing events are buffered for the
            // next inbox drain instead of spawning a second proactive turn.
            if state
                .pipeline
                .proactive_turn_in_flight
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                buffer_task_notification(&task_id, &status, now, state);
                return;
            }

            if state.pipeline.state() != InteractionState::Ready {
                state
                    .pipeline
                    .proactive_turn_in_flight
                    .store(false, Ordering::Release);
                return;
            }

            let state_arc = match app.try_state::<Arc<AppState>>() {
                Some(st) => Arc::clone(st.inner()),
                None => {
                    state
                        .pipeline
                        .proactive_turn_in_flight
                        .store(false, Ordering::Release);
                    return;
                }
            };
            let app_clone = app.clone();
            let ctx_clone = ctx.clone();
            let should_coalesce = state.tasks.active_tasks_count(session_id) > 0;

            let (new_turn_id, _) = state.pipeline.next_turn();
            state.pipeline_accumulator.lock().clear();
            state.turn_metrics.start_turn(new_turn_id);
            state.turn_metrics.record_speech_end();

            {
                let mut guard = state.harness.lock();
                if let Some(ref mut harness) = *guard {
                    harness.stage_user_query(new_turn_id, query.clone());
                }
            }
            state
                .pipeline_accumulator
                .lock()
                .set_user_transcript(query.clone());
            state.turn_metrics.record_transcript_final();

            transition(InteractionState::Thinking, None, ctx, app, state);

            tauri::async_runtime::spawn(async move {
                if should_coalesce {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                let still_valid = state_arc.pipeline.state() == InteractionState::Thinking
                    && state_arc.pipeline.peek_turn_id() == new_turn_id
                    && !state_arc.pipeline.is_turn_cancelled();
                if still_valid {
                    super::transcript::spawn_harness_turn_task(
                        new_turn_id,
                        query,
                        &app_clone,
                        &state_arc,
                        &ctx_clone,
                    );
                } else {
                    log::info!(
                        "[Pipeline::Tasks] Proactive turn {} superseded after coalescing wait",
                        new_turn_id
                    );
                }
                state_arc
                    .pipeline
                    .proactive_turn_in_flight
                    .store(false, Ordering::Release);
            });
        }
        InteractionState::Listening
        | InteractionState::Thinking
        | InteractionState::Working
        | InteractionState::Speaking => {
            let guard = state.harness.lock();
            if let Some(ref harness) = *guard {
                let msg = InboxMessage::new(
                    format!(
                        "[Background Task Finished] task_id={} status={}",
                        task_id,
                        status.as_str()
                    ),
                    QueuedMessageDelivery::Immediate,
                    now,
                );
                harness.inbox().push_task_notification(msg);
                log::info!(
                    "[Pipeline::Tasks] Buffered TaskFinished into StepInbox during {:?} (task {})",
                    current_state,
                    task_id
                );
            }
        }
        InteractionState::Idle
        | InteractionState::Paused
        | InteractionState::Sleeping
        | InteractionState::Error => {
            log::info!(
                "[Pipeline::Tasks] TaskFinished processed in {:?}; zero audio synthesized (task {})",
                current_state,
                task_id
            );
        }
    }
}

/// Drains any pending task notifications buffered in StepInbox and dispatches a proactive turn if active.
pub fn check_and_dispatch_inbox_tasks<R: Runtime + 'static>(
    app: &AppHandle<R>,
    state: &AppState,
    ctx: &RoutingContext,
) {
    let inbox_msgs = {
        let guard = state.harness.lock();
        if let Some(ref harness) = *guard {
            harness.inbox().drain_immediate()
        } else {
            Vec::new()
        }
    };

    if inbox_msgs.is_empty() {
        return;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let last_activity = state.pipeline.last_user_activity_ms.load(Ordering::Relaxed);
    if now.saturating_sub(last_activity) >= 5 * 60 * 1000 {
        return;
    }

    let combined_query = inbox_msgs
        .into_iter()
        .map(|m| m.text)
        .collect::<Vec<_>>()
        .join("\n");

    let app_clone = app.clone();
    let state_arc = match app.try_state::<Arc<AppState>>() {
        Some(st) => Arc::clone(st.inner()),
        None => return,
    };
    let ctx_clone = ctx.clone();

    tauri::async_runtime::spawn(async move {
        if state_arc.pipeline.state() != InteractionState::Ready {
            return;
        }

        let (new_turn_id, _) = state_arc.pipeline.next_turn();
        state_arc.pipeline_accumulator.lock().clear();
        state_arc.turn_metrics.start_turn(new_turn_id);
        state_arc.turn_metrics.record_speech_end();

        {
            let mut guard = state_arc.harness.lock();
            if let Some(ref mut harness) = *guard {
                harness.stage_user_query(new_turn_id, combined_query.clone());
            }
        }
        state_arc
            .pipeline_accumulator
            .lock()
            .set_user_transcript(combined_query.clone());
        state_arc.turn_metrics.record_transcript_final();

        transition(
            InteractionState::Thinking,
            None,
            &ctx_clone,
            &app_clone,
            &state_arc,
        );

        super::transcript::spawn_harness_turn_task(
            new_turn_id,
            combined_query,
            &app_clone,
            &state_arc,
            &ctx_clone,
        );
    });
}

/// Buffers a task completion for the next inbox drain without spawning a turn.
fn buffer_task_notification(task_id: &str, status: &TaskStatus, now: u64, state: &AppState) {
    let guard = state.harness.lock();
    if let Some(ref harness) = *guard {
        let msg = InboxMessage::new(
            format!(
                "[Background Task Finished] task_id={} status={}",
                task_id,
                status.as_str()
            ),
            QueuedMessageDelivery::Immediate,
            now,
        );
        harness.inbox().push_task_notification(msg);
    }
}
