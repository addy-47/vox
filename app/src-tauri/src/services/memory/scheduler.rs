use std::sync::Arc;

use anyhow::{anyhow, Result};
use tauri::AppHandle;

use crate::{
    core::{
        events::{emit_ipc, IpcEvent, Severity},
        state::{AppState, InteractionState},
    },
    persistence::{
        notifications::dismiss_interactive_by_entity, personal_memory::get_personal_memory,
    },
    services::{
        llm::{
            actor::create_llm_provider_from_llm_settings, LlmProvider, QWEN_MODEL_DIR,
            QWEN_MODEL_FILE,
        },
        memory::personal::consolidate_personal_memory,
        notifications::{notify, Action, ActionPayload, NotificationCategory, NotificationParams},
    },
    utils::paths,
};

const DAY_SECS: u64 = 86_400;

/// Parses an "HH:MM" 24-hour time string into `(hour, minute)`.
pub fn parse_consolidation_time(value: &str) -> Option<(u32, u32)> {
    let (hour_str, min_str) = value.split_once(':')?;
    let hour: u32 = hour_str.parse().ok()?;
    let minute: u32 = min_str.parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some((hour, minute))
}

/// Resolves an LLM provider for consolidation: cached instance first, settings-built fallback.
fn resolve_provider(state: &Arc<AppState>) -> Option<Arc<dyn LlmProvider>> {
    if let Some(cached) = state.llm_provider.read().clone() {
        return Some(cached);
    }
    let llm_settings = state.settings.read().ok().map(|s| s.llm.clone())?;
    let models_dir = paths::get().models.clone();
    let llm_path = models_dir.join(QWEN_MODEL_DIR).join(QWEN_MODEL_FILE);
    create_llm_provider_from_llm_settings(&llm_settings, &llm_path)
        .ok()
        .map(Arc::from)
}

/// Runs one consolidation pass and mirrors the IPC post-steps (prompt refresh + event).
pub async fn run_consolidation_once<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
) -> Result<()> {
    let current = state.pipeline.state();
    if current != InteractionState::Ready
        && current != InteractionState::Paused
        && current != InteractionState::Idle
    {
        return Err(anyhow!("Consolidation deferred: pipeline is active"));
    }

    let provider = resolve_provider(state)
        .ok_or_else(|| anyhow!("Failed to initialize LLM provider for consolidation"))?;

    let llm_settings = state.settings.read().ok().map(|s| s.llm.clone());
    let conn = state.db.connect()?;
    let record =
        consolidate_personal_memory(&conn, provider.as_ref(), None, None, llm_settings.as_ref())
            .await?;

    if let Err(e) = emit_ipc(app, IpcEvent::PersonalMemoryUpdated(record)) {
        log::warn!(
            "[Memory::Scheduler] Failed to emit PersonalMemoryUpdated: {}",
            e
        );
    }

    // Dismiss active interactive missed card for consolidation
    if let Err(e) = dismiss_interactive_by_entity(&conn, "memory_consolidation:missed").await {
        log::warn!(
            "[Memory::Scheduler] Failed to dismiss interactive card on consolidation success: {}",
            e
        );
    }

    // Emit silent audit receipt
    if let Err(e) = notify(
        app,
        &state.db,
        NotificationParams {
            group_key: Some("memory_consolidation:daily"),
            category: NotificationCategory::MemoryConsolidation,
            severity: Severity::Info,
            impact: None,
            action: Action::Receipt,
            title: "Memory Consolidated",
            message: "Daily profile updated.",
            session_id: None,
            metadata: None,
            duration_ms: None,
        },
    )
    .await
    {
        log::warn!(
            "[Memory::Scheduler] Failed to emit consolidation receipt: {}",
            e
        );
    }

    Ok(())
}

/// Quiescence/busy deferrals are transient: retried silently, never carded as failures.
fn is_deferral(err: &anyhow::Error) -> bool {
    err.to_string().contains("deferred")
}

/// Boot check for daily runs missed while the app was down. Never runs silently.
pub async fn check_missed_consolidation_on_boot<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &Arc<AppState>,
) {
    let (cadence, time_str) = state
        .settings
        .read()
        .map(|s| {
            (
                s.personal_memory.consolidation_cadence.clone(),
                s.personal_memory.consolidation_time.clone(),
            )
        })
        .unwrap_or_else(|_| ("manual".to_string(), "02:00".to_string()));
    if cadence != "daily" {
        return;
    }
    let (hour, minute) = match parse_consolidation_time(&time_str) {
        Some(t) => t,
        None => return,
    };

    let now = chrono::Local::now();
    let today_sched = now
        .date_naive()
        .and_hms_opt(hour, minute, 0)
        .and_then(|t| t.and_local_timezone(chrono::Local).single())
        .map(|t| t.timestamp_millis())
        .unwrap_or(0);
    if now.timestamp_millis() < today_sched {
        return;
    }

    let conn = match state.db.connect() {
        Ok(c) => c,
        Err(_) => return,
    };
    let last = get_personal_memory(&conn, None)
        .await
        .map(|r| r.last_consolidated_at)
        .unwrap_or(0);
    if last >= today_sched {
        return;
    }

    let params = NotificationParams {
        group_key: Some("memory_consolidation:missed"),
        category: NotificationCategory::MemoryConsolidation,
        severity: Severity::Warning,
        impact: None,
        action: Action::Interactive(ActionPayload::ConsolidateMemory),
        title: "Memory Consolidation Missed",
        message: "The scheduled daily consolidation did not run. Tap Consolidate to run it now.",
        session_id: None,
        metadata: None,
        duration_ms: None,
    };

    if let Err(e) = notify(app, &state.db, params).await {
        log::warn!(
            "[Memory::Scheduler] Failed to emit missed consolidation notification: {}",
            e
        );
    }
}

/// Duration from now until the next local `hour:minute`. Falls back to 24h on DST gaps.
fn duration_until_next(hour: u32, minute: u32) -> std::time::Duration {
    let fallback = std::time::Duration::from_secs(DAY_SECS);
    let now = chrono::Local::now();
    let today = now
        .date_naive()
        .and_hms_opt(hour, minute, 0)
        .and_then(|t| t.and_local_timezone(chrono::Local).single())
        .filter(|t| *t > now);
    let next = today.or_else(|| {
        (now.date_naive() + chrono::Days::new(1))
            .and_hms_opt(hour, minute, 0)
            .and_then(|t| t.and_local_timezone(chrono::Local).single())
    });
    next.map(|t| (t - now).to_std().unwrap_or(fallback))
        .unwrap_or(fallback)
}

/// Spawns the daily consolidation timer. Sleeps until the next scheduled time and wakes
/// once per run; deferred runs retry at the next scheduled time, never sooner.
pub fn spawn_consolidation_scheduler<R: tauri::Runtime + 'static>(
    app: AppHandle<R>,
    state: Arc<AppState>,
) {
    tauri::async_runtime::spawn(async move {
        log::info!("[Memory::Scheduler] Consolidation scheduler spawned.");
        loop {
            let (cadence, time_str) = match state.settings.read() {
                Ok(s) => (
                    s.personal_memory.consolidation_cadence.clone(),
                    s.personal_memory.consolidation_time.clone(),
                ),
                Err(_) => {
                    log::warn!("[Memory::Scheduler] Settings lock poisoned; scheduler stopping.");
                    break;
                }
            };
            if cadence != "daily" {
                break;
            }
            let (hour, minute) = match parse_consolidation_time(&time_str) {
                Some(t) => t,
                None => {
                    log::warn!(
                        "[Memory::Scheduler] Invalid consolidation_time; scheduler stopping."
                    );
                    break;
                }
            };

            tokio::time::sleep(duration_until_next(hour, minute)).await;

            let still_daily = state
                .settings
                .read()
                .map(|s| s.personal_memory.consolidation_cadence.clone())
                .unwrap_or_default()
                == "daily";
            if !still_daily {
                break;
            }

            match run_consolidation_once(&app, &state).await {
                Ok(()) => {
                    log::info!("[Memory::Scheduler] Daily consolidation completed.");
                }
                Err(e) if is_deferral(&e) => {
                    log::info!(
                        "[Memory::Scheduler] Daily consolidation deferred (busy); next scheduled run will retry."
                    );
                }
                Err(e) => {
                    log::warn!("[Memory::Scheduler] Daily consolidation failed: {}", e);
                    let err_msg = format!(
                        "Scheduled daily consolidation failed: {}. Tap Consolidate to retry.",
                        e
                    );
                    let params = NotificationParams {
                        group_key: Some("memory_consolidation:missed"),
                        category: NotificationCategory::MemoryConsolidation,
                        severity: Severity::Warning,
                        impact: None,
                        action: Action::Interactive(ActionPayload::ConsolidateMemory),
                        title: "Memory Consolidation Failed",
                        message: &err_msg,
                        session_id: None,
                        metadata: None,
                        duration_ms: None,
                    };
                    if let Err(notif_err) = notify(&app, &state.db, params).await {
                        log::warn!(
                            "[Memory::Scheduler] Failed to emit consolidation failure notification: {}",
                            notif_err
                        );
                    }
                }
            }
        }
        log::info!("[Memory::Scheduler] Consolidation scheduler stopped.");
    });
}
