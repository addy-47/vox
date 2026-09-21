use std::sync::Arc;

use anyhow::{anyhow, Result};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;
use turso::Connection;

use crate::{
    core::{
        events::{emit_ipc, IpcEvent, Severity},
        settings::LlmSettings,
        state::{AppState, InteractionState},
    },
    persistence::{
        compactions::{
            commit_compaction_output, fetch_latest_compaction_run, fetch_turns_for_compaction,
            record_compaction_finish, record_compaction_start,
        },
        notifications::{find_notification_by_group, resolve_notification_in_place},
        TurnRow,
    },
    services::{
        harness::{ChatMessage, Role},
        llm::{
            actor::create_llm_provider_from_llm_settings, LlmProvider, QWEN_MODEL_DIR,
            QWEN_MODEL_FILE,
        },
        memory::compaction::runner::run_compaction,
        notifications::{notify, Action, ActionPayload, NotificationCategory, NotificationParams},
    },
    utils::paths::get,
};

/// Summary of a successfully executed compaction slice.
#[derive(Debug, Clone)]
pub struct CompactionExecutionSummary {
    pub session_id: i64,
    pub facts_enqueued: u32,
    pub session_context: String,
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

        let conn = state.db.connect()?;

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

        let turns =
            fetch_turns_for_compaction(&conn, session_id, last_compacted_turn + 1, u32::MAX)
                .await?;
        if turns.is_empty() {
            log::info!(
                "[CompactionCoordinator] No turns pending compaction for session {}",
                session_id
            );
            return Ok(None);
        }

        let from_turn_id = turns.first().map(|t| t.turn_id).unwrap_or(1);
        let to_turn_id = turns.last().map(|t| t.turn_id).unwrap_or(from_turn_id);

        let run_id = match record_compaction_start(
            &conn,
            session_id,
            trigger_kind,
            from_turn_id,
            to_turn_id,
        )
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
                    record_compaction_finish(&conn, run_id, "", "failed", Some(err_msg)).await
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
                    record_compaction_finish(&conn, run_id, "", "failed", Some(&err_str)).await
                {
                    log::warn!(
                        "[CompactionCoordinator] Failed to record compaction failure: {}",
                        record_err
                    );
                }
                emit_session_compaction_failure_receipt(app, &state.db, session_id, &err_str).await;
                return Err(e);
            }
        };

        let facts_count = compaction_res.facts.len() as u32;
        commit_compaction_output(
            &conn,
            run_id,
            &compaction_res.raw_json,
            &compaction_res.facts,
            session_id,
        )
        .await?;

        emit_session_compaction_success_receipt(app, &state.db, &conn, session_id, facts_count)
            .await;

        log::info!(
            "[CompactionCoordinator] Successfully compacted session {} (enqueued {} facts)",
            session_id,
            facts_count
        );

        Ok(Some(CompactionExecutionSummary {
            session_id,
            facts_enqueued: facts_count,
            session_context: compaction_res.session_context,
            from_turn_id,
            to_turn_id,
        }))
    }

    /// Emits a new notification alerting the user that a session has uncompacted turns.
    pub async fn notify_uncompacted_session<R: tauri::Runtime>(
        app: &AppHandle<R>,
        db: &crate::persistence::db::VoxDb,
        session_id: i64,
        uncompacted_turns: u32,
    ) -> Result<Option<String>> {
        let group_key = format!("session_compaction:{}", session_id);
        let title = format!("Session #{} Ready to Compact", session_id);
        let message = format!(
            "Session ended with {} uncompacted turn(s). Compact to extract personal memory facts.",
            uncompacted_turns
        );
        let metadata = format!(
            "{{\"uncompacted_turns\": {}, \"resolution\": \"pending\"}}",
            uncompacted_turns
        );

        let params = NotificationParams {
            group_key: Some(&group_key),
            category: NotificationCategory::SessionCompaction,
            severity: Severity::Info,
            impact: None,
            action: Action::Interactive(ActionPayload::CompactSession { session_id }),
            title: &title,
            message: &message,
            session_id: Some(session_id),
            metadata: Some(&metadata),
            duration_ms: None,
        };

        notify(app, db, params).await
    }
}

/// Helper building ChatMessage list from turns.
fn build_history_messages(turns: &[TurnRow]) -> Vec<ChatMessage> {
    let mut messages = Vec::with_capacity(turns.len() * 2);
    for turn in turns {
        if !turn.user_text.trim().is_empty() {
            messages.push(ChatMessage::new(Role::User, turn.user_text.clone()));
        }
        if !turn.assistant_text.trim().is_empty() {
            messages.push(ChatMessage::new(Role::Assistant, turn.assistant_text.clone()));
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

/// Resolves interactive card in-place on compaction success, or emits passive receipt if none exists.
async fn emit_session_compaction_success_receipt<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::persistence::db::VoxDb,
    conn: &Connection,
    session_id: i64,
    facts_count: u32,
) {
    let group_key = format!("session_compaction:{}", session_id);
    let message = format!(
        "Successfully compacted session #{} and extracted {} memory facts.",
        session_id, facts_count
    );

    // If an interactive card exists, update it in-place to resolved
    if let Ok(Some(existing)) = find_notification_by_group(conn, &group_key).await {
        if let Ok(Some(updated)) =
            resolve_notification_in_place(conn, &existing.id, "resolved", Some(&message)).await
        {
            let _ = emit_ipc(app, IpcEvent::NotificationUpdated(updated));
            return;
        }
    }

    // Otherwise (background run with no pending card), append passive receipt:
    let title = format!("Session #{} Compacted", session_id);
    let receipt_group = format!("compaction_receipt:{}", session_id);

    let params = NotificationParams {
        group_key: Some(&receipt_group),
        category: NotificationCategory::SessionCompaction,
        severity: Severity::Info,
        impact: None,
        action: Action::Receipt,
        title: &title,
        message: &message,
        session_id: Some(session_id),
        metadata: None,
        duration_ms: None,
    };

    if let Err(e) = notify(app, db, params).await {
        log::warn!(
            "[CompactionCoordinator] Failed to emit success receipt: {}",
            e
        );
    }
}

/// Resolves interactive card in-place on compaction failure, or emits passive warning receipt if none exists.
async fn emit_session_compaction_failure_receipt<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &crate::persistence::db::VoxDb,
    session_id: i64,
    err_str: &str,
) {
    let group_key = format!("session_compaction:{}", session_id);
    let message = format!("Failed to compact session #{}: {}", session_id, err_str);

    if let Ok(conn) = db.connect() {
        if let Ok(Some(existing)) = find_notification_by_group(&conn, &group_key).await {
            if let Ok(Some(updated)) =
                resolve_notification_in_place(&conn, &existing.id, "failed", Some(&message)).await
            {
                let _ = emit_ipc(app, IpcEvent::NotificationUpdated(updated));
                return;
            }
        }
    }

    let title = format!("Session #{} Compaction Failed", session_id);
    let receipt_group = format!("compaction_receipt:{}", session_id);

    let params = NotificationParams {
        group_key: Some(&receipt_group),
        category: NotificationCategory::SessionCompaction,
        severity: Severity::Warning,
        impact: None,
        action: Action::Receipt,
        title: &title,
        message: &message,
        session_id: Some(session_id),
        metadata: None,
        duration_ms: None,
    };

    if let Err(e) = notify(app, db, params).await {
        log::warn!(
            "[CompactionCoordinator] Failed to emit failure receipt: {}",
            e
        );
    }
}
