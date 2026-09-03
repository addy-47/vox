use std::sync::Arc;
use anyhow::{anyhow, Result};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

use crate::core::events::{emit_ipc, IpcEvent, NotificationRecord};
use crate::core::state::{AppState, InteractionState};
use crate::persistence::compactions::{
    commit_compaction_results, fetch_latest_compaction_run, fetch_turns_for_compaction,
    record_compaction_finish, record_compaction_start,
};
use crate::persistence::db::VoxDb;
use crate::persistence::notifications::{
    create_notification, find_active_notification_by_session, update_notification_status,
    NewNotification,
};
use crate::services::harness::buffer::{ChatMessage, Role};
use crate::services::llm::LlmProvider;
use crate::services::memory::compaction::runner::run_compaction;

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
    pub async fn run_compaction_slice<R: tauri::Runtime>(
        app: &AppHandle<R>,
        state: &Arc<AppState>,
        session_id: i64,
        trigger_kind: &str,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<Option<CompactionExecutionSummary>> {

        if trigger_kind == "auto" {
            let current_state = state.pipeline.state();
            if current_state != InteractionState::Idle && current_state != InteractionState::Paused {
                log::info!(
                    "[CompactionCoordinator] Deferring auto-compaction for session {}: pipeline state is {:?}",
                    session_id,
                    current_state
                );
                return Ok(None);
            }
        }

        let db_path = crate::utils::paths::db_path();
        let conn = VoxDb::open(&db_path).await?;

        // 2. Prevent concurrent / duplicate compaction for the same session
        if let Ok(Some(latest)) = fetch_latest_compaction_run(&conn, session_id).await {
            if latest.status == "in_progress" {
                log::info!(
                    "[CompactionCoordinator] Compaction already in progress for session {}",
                    session_id
                );
                return Ok(None);
            }
        }

        let last_compacted_turn = match fetch_latest_compaction_run(&conn, session_id).await {
            Ok(Some(run)) if run.status == "completed" => run.to_turn_id,
            _ => 0,
        };

        let turns = fetch_turns_for_compaction(&conn, session_id, last_compacted_turn).await?;
        if turns.is_empty() {
            log::info!(
                "[CompactionCoordinator] No turns pending compaction for session {}",
                session_id
            );
            return Ok(None);
        }

        let from_turn_id = turns.first().map(|t| t.turn_id).unwrap_or(1);
        let to_turn_id = turns.last().map(|t| t.turn_id).unwrap_or(from_turn_id);

        let run_id = record_compaction_start(
            &conn,
            session_id,
            trigger_kind,
            from_turn_id,
            to_turn_id,
        )
        .await?;

        let mut history_messages = Vec::with_capacity(turns.len() * 2);
        for turn in &turns {
            if !turn.user_text.trim().is_empty() {
                history_messages.push(ChatMessage {
                    role: Role::User,
                    content: turn.user_text.clone(),
                    timestamp_ms: 0,
                });
            }
            if !turn.assistant_text.trim().is_empty() {
                history_messages.push(ChatMessage {
                    role: Role::Assistant,
                    content: turn.assistant_text.clone(),
                    timestamp_ms: 0,
                });
            }
        }

        let (llm_settings, pipeline_enabled) = {
            let s = state.settings.read().map(|s| s.clone()).unwrap_or_default();
            (s.llm.clone(), s.memory.pipeline_processing_enabled)
        };

        let provider_box: Option<Box<dyn LlmProvider>> = {
            let models_dir = crate::utils::paths::get().models.clone();
            let llm_path = models_dir
                .join(crate::services::llm::QWEN_MODEL_DIR)
                .join(crate::services::llm::QWEN_MODEL_FILE);
            crate::services::llm::actor::create_llm_provider_from_llm_settings(
                &llm_settings,
                &llm_path,
            )
            .ok()
        };

        let active_provider = match provider_box {
            Some(p) => p,
            None => {
                let err_msg = "Failed to initialize LLM provider for compaction";
                let _ = record_compaction_finish(&conn, run_id, "failed", 0, Some(err_msg)).await;
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
                    "[CompactionCoordinator] Compaction execution failed for session {}: {}",
                    session_id,
                    err_str
                );
                let _ = record_compaction_finish(
                    &conn,
                    run_id,
                    "failed",
                    0,
                    Some(&err_str),
                )
                .await;

                if let Ok(Some(mut notif)) = find_active_notification_by_session(
                    &conn,
                    session_id,
                    "session_compaction",
                )
                .await
                {
                    let _ = update_notification_status(&conn, &notif.id, "failed").await;
                    notif.status = "failed".to_string();
                    let _ = emit_ipc(app, IpcEvent::NotificationUpdated(notif));
                }

                return Err(e);
            }
        };

        let facts_count = commit_compaction_results(
            &conn,
            run_id,
            &session_id.to_string(),
            &compaction_res.context_summary,
            compaction_res.personal_memory,
            pipeline_enabled,
        )
        .await?;

        if let Ok(Some(mut notif)) =
            find_active_notification_by_session(&conn, session_id, "session_compaction").await
        {
            let _ = update_notification_status(&conn, &notif.id, "completed").await;
            notif.status = "completed".to_string();
            let _ = emit_ipc(app, IpcEvent::NotificationUpdated(notif));
        }

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
        session_id: i64,
        uncompacted_turns: u32,
    ) -> Result<NotificationRecord> {
        let db_path = crate::utils::paths::db_path();
        let conn = VoxDb::open(&db_path).await?;

        if let Some(existing) =
            find_active_notification_by_session(&conn, session_id, "session_compaction").await?
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

        let record = create_notification(&conn, &notif).await?;
        if let Err(e) = emit_ipc(app, IpcEvent::NotificationCreated(record.clone())) {
            log::warn!(
                "[CompactionCoordinator] Failed to emit NotificationCreated: {}",
                e
            );
        }

        Ok(record)
    }
}
