use std::sync::Arc;

use anyhow::{anyhow, Result};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;
use turso::Connection;

use crate::{
    core::{
        events::{emit_ipc, IpcEvent, NotificationRecord},
        settings::LlmSettings,
        state::{AppState, InteractionState},
    },
    persistence::{
        compactions::{
            commit_compaction_output, fetch_latest_compaction_run, fetch_turns_for_compaction,
            record_compaction_finish, record_compaction_start,
        },
        notifications::{
            create_notification, find_active_notification_by_session, update_notification_status,
            NewNotification,
        },
        TurnRow,
    },
    services::{
        harness::{ChatMessage, Role},
        llm::{
            actor::create_llm_provider_from_llm_settings, LlmProvider, QWEN_MODEL_DIR,
            QWEN_MODEL_FILE,
        },
        memory::compaction::runner::run_compaction,
    },
    utils::paths::get,
};

/// Summary of a successfully executed compaction slice.
#[derive(Debug, Clone)]
pub struct CompactionExecutionSummary {
    pub session_id: i64,
    pub facts_enqueued: u32,
    pub context_summary: String,
    pub from_turn_id: u32,
    pub to_turn_id: u32,
}

pub struct CompactionCoordinator;

impl CompactionCoordinator {
    /// Executes a compaction slice for a session, checking lock mutual exclusion and updating persistent state.
    pub async fn run_compaction_slice<R: tauri::Runtime>(
        app: &AppHandle<R>,
        state: &Arc<AppState>,
        session_id: i64,
        trigger_kind: &str,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<Option<CompactionExecutionSummary>> {
        if trigger_kind == "soft" {
            let current_state = state.pipeline.state();
            if current_state != InteractionState::Ready && current_state != InteractionState::Paused
            {
                log::info!(
                    "[CompactionCoordinator] Deferring compaction for session {}: pipeline state is {:?}",
                    session_id,
                    current_state
                );
                return Ok(None);
            }
        }

        let conn = &state.db;

        if let Ok(Some(latest)) = fetch_latest_compaction_run(conn, session_id).await {
            if latest.status == "in_progress" {
                log::info!(
                    "[CompactionCoordinator] Compaction already in progress for session {}",
                    session_id
                );
                return Ok(None);
            }
        }

        let last_compacted_turn = match fetch_latest_compaction_run(conn, session_id).await {
            Ok(Some(run)) if run.status == "completed" => run.to_turn_id,
            _ => 0,
        };

        let turns =
            fetch_turns_for_compaction(conn, session_id, last_compacted_turn + 1, u32::MAX).await?;
        if turns.is_empty() {
            log::info!(
                "[CompactionCoordinator] No turns pending compaction for session {}",
                session_id
            );
            return Ok(None);
        }

        let from_turn_id = turns.first().map(|t| t.turn_id).unwrap_or(1);
        let to_turn_id = turns.last().map(|t| t.turn_id).unwrap_or(from_turn_id);

        let run_id =
            match record_compaction_start(conn, session_id, trigger_kind, from_turn_id, to_turn_id)
                .await
            {
                Ok(id) => id,
                Err(e) if e.to_string().contains("UNIQUE constraint failed") => {
                    log::info!(
                        "[CompactionCoordinator] Duplicate compaction run rejected for session {}",
                        session_id
                    );
                    return Ok(None);
                }
                Err(e) => return Err(e),
            };

        let history_messages = build_history_messages(&turns);
        let llm_settings = state
            .settings
            .read()
            .map(|s| s.llm.clone())
            .unwrap_or_default();

        let active_provider = match resolve_llm_provider(&llm_settings) {
            Some(p) => p,
            None => {
                let err_msg = "Failed to initialize LLM provider for compaction";
                if let Err(e) =
                    record_compaction_finish(conn, run_id, "", "failed", Some(err_msg)).await
                {
                    log::warn!(
                        "[CompactionCoordinator] Failed to record compaction finish: {}",
                        e
                    );
                }
                return Err(anyhow!(err_msg));
            }
        };

        log::info!(
            "[CompactionCoordinator] Running compaction for session {} (turns {}-{}) via {:?}",
            session_id,
            from_turn_id,
            to_turn_id,
            trigger_kind
        );

        let compaction_res = match run_compaction(
            active_provider.as_ref(),
            &history_messages,
            Some(&llm_settings),
            cancel_token,
        )
        .await
        {
            Ok(res) => res,
            Err(e) => {
                let err_str = e.to_string();
                log::error!(
                    "[CompactionCoordinator] Compaction failed for session {}: {}",
                    session_id,
                    err_str
                );
                if let Err(record_err) =
                    record_compaction_finish(conn, run_id, "", "failed", Some(&err_str)).await
                {
                    log::warn!(
                        "[CompactionCoordinator] Failed to record compaction failure: {}",
                        record_err
                    );
                }
                update_session_notification_on_failure(app, conn, session_id).await;
                return Err(e);
            }
        };

        let facts_count = compaction_res.facts.len() as u32;
        commit_compaction_output(
            conn,
            run_id,
            &compaction_res.raw_json,
            &compaction_res.facts,
            session_id,
        )
        .await?;

        update_session_notification_on_success(app, conn, session_id).await;

        log::info!(
            "[CompactionCoordinator] Successfully compacted session {} (enqueued {} facts)",
            session_id,
            facts_count
        );

        Ok(Some(CompactionExecutionSummary {
            session_id,
            facts_enqueued: facts_count,
            context_summary: compaction_res.context_summary,
            from_turn_id,
            to_turn_id,
        }))
    }

    /// Emits a new notification alerting the user that a session has uncompacted turns.
    pub async fn notify_uncompacted_session<R: tauri::Runtime>(
        app: &AppHandle<R>,
        conn: &Connection,
        session_id: i64,
        uncompacted_turns: u32,
    ) -> Result<NotificationRecord> {
        if let Some(existing) =
            find_active_notification_by_session(conn, session_id, "session_compaction").await?
        {
            return Ok(existing);
        }

        let notif = NewNotification {
            id: format!("notif_compaction_{}", session_id),
            category: "session_compaction".to_string(),
            title: format!("Session #{} Uncompacted", session_id),
            message: format!(
                "Session ended with {} uncompacted turn(s). Compact to extract personal memory facts.",
                uncompacted_turns
            ),
            status: "pending".to_string(),
            session_id: Some(session_id),
            metadata: format!("{{\"uncompacted_turns\": {}}}", uncompacted_turns),
        };

        let record = create_notification(conn, &notif).await?;
        if let Err(e) = emit_ipc(app, IpcEvent::NotificationCreated(record.clone())) {
            log::warn!(
                "[CompactionCoordinator] Failed to emit NotificationCreated: {}",
                e
            );
        }

        Ok(record)
    }
}

/// Helper building ChatMessage list from turns.
fn build_history_messages(turns: &[TurnRow]) -> Vec<ChatMessage> {
    let mut messages = Vec::with_capacity(turns.len() * 2);
    for turn in turns {
        if !turn.user_text.trim().is_empty() {
            messages.push(ChatMessage {
                role: Role::User,
                content: turn.user_text.clone(),
                timestamp_ms: 0,
            });
        }
        if !turn.assistant_text.trim().is_empty() {
            messages.push(ChatMessage {
                role: Role::Assistant,
                content: turn.assistant_text.clone(),
                timestamp_ms: 0,
            });
        }
    }
    messages
}

/// Resolves the LLM provider for compaction from settings.
fn resolve_llm_provider(settings: &LlmSettings) -> Option<Box<dyn LlmProvider>> {
    let models_dir = get().models.clone();
    let llm_path = models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE);
    create_llm_provider_from_llm_settings(settings, &llm_path).ok()
}

/// Updates active notification on compaction failure.
async fn update_session_notification_on_failure<R: tauri::Runtime>(
    app: &AppHandle<R>,
    conn: &Connection,
    session_id: i64,
) {
    if let Ok(Some(mut notif)) =
        find_active_notification_by_session(conn, session_id, "session_compaction").await
    {
        if let Err(e) = update_notification_status(conn, &notif.id, "failed").await {
            log::warn!(
                "[CompactionCoordinator] Failed to update notification status to failed: {}",
                e
            );
        }
        notif.status = "failed".to_string();
        if let Err(e) = emit_ipc(app, IpcEvent::NotificationUpdated(notif)) {
            log::warn!(
                "[CompactionCoordinator] Failed to emit NotificationUpdated: {}",
                e
            );
        }
    }
}

/// Updates active notification on compaction success.
async fn update_session_notification_on_success<R: tauri::Runtime>(
    app: &AppHandle<R>,
    conn: &Connection,
    session_id: i64,
) {
    if let Ok(Some(mut notif)) =
        find_active_notification_by_session(conn, session_id, "session_compaction").await
    {
        if let Err(e) = update_notification_status(conn, &notif.id, "completed").await {
            log::warn!(
                "[CompactionCoordinator] Failed to update notification status to completed: {}",
                e
            );
        }
        notif.status = "completed".to_string();
        if let Err(e) = emit_ipc(app, IpcEvent::NotificationUpdated(notif)) {
            log::warn!(
                "[CompactionCoordinator] Failed to emit NotificationUpdated: {}",
                e
            );
        }
    }
}
