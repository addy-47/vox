use std::{
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::AppHandle;

use super::{notify, Action, NotificationCategory, NotificationParams};
use crate::{
    core::{
        events::Severity,
        state::AppState,
        settings::DictationOutputMode
    },    
    persistence::VoxDb,
    services::dictation::DICTATION_PARTIAL_UPDATE_THROTTLE_MS,
    toast::{show_replaceable_toast, update_replaceable_toast},
};

/// Resident server-side notification ID backing the active dictation lifecycle card (0 = none).
static LIFECYCLE_NOTIFY_ID: AtomicU32 = AtomicU32::new(0);
/// Millisecond timestamp of the last live partial update (throttle gate).
static LIFECYCLE_LAST_UPDATE_MS: AtomicU64 = AtomicU64::new(0);
/// Expiry keeping the lifecycle card resident until replaced.
const LIFECYCLE_RESIDENT_MS: u64 = 30_000;
/// Correlation key shared by every card in one dictation lifecycle.
const LIFECYCLE_GROUP_KEY: &str = "dictation:lifecycle";

/// Content of one Transient lifecycle card delivered through the front door.
pub struct LifecycleCard<'a> {
    /// Card title (may carry a leading status glyph).
    pub title: &'a str,
    /// Card body (supports Pango `<b>` markup on Linux).
    pub message: &'a str,
    /// Visual urgency of the card.
    pub severity: Severity,
    /// Resident expiry in milliseconds.
    pub duration_ms: u64,
}

fn is_tray_mode<R: tauri::Runtime>(app: &AppHandle<R>) -> bool {
    use tauri::Manager;
    if let Some(state) = app.try_state::<Arc<AppState>>() {
        if let Ok(s) = state.settings.read() {
            return s.dictation.output_mode == DictationOutputMode::Tray;
        }
    }
    false
}

/// Starts the persistent dictation lifecycle card in Listening state.
pub async fn dictation_listening<R: tauri::Runtime>(app: &AppHandle<R>, db: &VoxDb) {
    if is_tray_mode(app) {
        return;
    }

    use tauri::Manager;
    let auto_stop_ms = app
        .try_state::<Arc<AppState>>()
        .and_then(|st| {
            st.settings
                .read()
                .ok()
                .map(|s| s.dictation.silence_auto_stop_ms)
        })
        .unwrap_or(1200);

    let auto_stop_text = if auto_stop_ms > 0 {
        format!(
            " · a {:.1}s pause auto-finishes",
            auto_stop_ms as f32 / 1000.0
        )
    } else {
        String::new()
    };

    let title = "🎙️ Dictation";
    let message = format!("<b>Listening...</b> Speak clearly{}", auto_stop_text);
    let server_id = show_replaceable_toast(title, &message, Severity::Info, LIFECYCLE_RESIDENT_MS);
    if server_id == 0 {
        notify_transient(
            app,
            db,
            LifecycleCard {
                title,
                message: &message,
                severity: Severity::Info,
                duration_ms: LIFECYCLE_RESIDENT_MS,
            },
        )
        .await;
        return;
    }
    LIFECYCLE_NOTIFY_ID.store(server_id, Ordering::Relaxed);
    LIFECYCLE_LAST_UPDATE_MS.store(now_ms(), Ordering::Relaxed);
}

/// Refreshes the lifecycle card with throttled live partial text.
/// Synchronous and database-free, so OS worker threads may call it directly.
pub fn dictation_live_update(partial_text: &str) {
    let server_id = LIFECYCLE_NOTIFY_ID.load(Ordering::Relaxed);
    if server_id == 0 || partial_text.trim().is_empty() {
        return;
    }
    let now = now_ms();
    if now.saturating_sub(LIFECYCLE_LAST_UPDATE_MS.load(Ordering::Relaxed))
        < DICTATION_PARTIAL_UPDATE_THROTTLE_MS
    {
        return;
    }
    LIFECYCLE_LAST_UPDATE_MS.store(now, Ordering::Relaxed);
    let snippet = truncate_partial(partial_text);
    let body = format!("<b>Listening...</b>\n<i>\"{}\"</i>", snippet);
    update_replaceable_toast(
        server_id,
        "🎙️ Dictation",
        &body,
        Severity::Info,
        LIFECYCLE_RESIDENT_MS,
    );
}

/// Replaces the lifecycle card with the Transcribing state.
pub async fn dictation_transcribing<R: tauri::Runtime>(app: &AppHandle<R>, db: &VoxDb) {
    if is_tray_mode(app) {
        return;
    }
    replace_or_notify(
        app,
        db,
        LifecycleCard {
            title: "⏳ Dictation",
            message: "<b>Transcribing...</b> Processing speech",
            severity: Severity::Info,
            duration_ms: 5000,
        },
    )
    .await;
}

/// Replaces the lifecycle card with a terminal state and releases it.
pub async fn dictation_terminal<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &VoxDb,
    card: LifecycleCard<'_>,
) {
    if is_tray_mode(app) {
        LIFECYCLE_NOTIFY_ID.store(0, Ordering::Relaxed);
        return;
    }
    replace_or_notify(app, db, card).await;
    LIFECYCLE_NOTIFY_ID.store(0, Ordering::Relaxed);
}

/// Replaces the resident lifecycle card, falling back to the front door when no card exists.
async fn replace_or_notify<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &VoxDb,
    card: LifecycleCard<'_>,
) {
    let active = LIFECYCLE_NOTIFY_ID.load(Ordering::Relaxed);
    if active == 0 {
        notify_transient(app, db, card).await;
        return;
    }
    update_replaceable_toast(
        active,
        card.title,
        card.message,
        card.severity,
        card.duration_ms,
    );
}

/// Routes a dictation Transient card through the universal front door.
/// Transient impact always resolves to ToastOnly with drawer elevation on dispatch failure.
async fn notify_transient<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &VoxDb,
    card: LifecycleCard<'_>,
) {
    let params = NotificationParams {
        group_key: Some(LIFECYCLE_GROUP_KEY),
        category: NotificationCategory::Dictation,
        severity: card.severity,
        impact: None,
        action: Action::Transient,
        title: card.title,
        message: card.message,
        session_id: None,
        metadata: None,
        duration_ms: Some(card.duration_ms),
    };
    if let Err(e) = notify(app, db, params).await {
        log::warn!(
            "[Notification::Lifecycle] Front-door dispatch failed: {}",
            e
        );
    }
}

/// Returns current wall-clock milliseconds for update throttling.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Truncates live partial text to a notification-friendly snippet.
fn truncate_partial(text: &str) -> String {
    const MAX_SNIPPET_CHARS: usize = 240;
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_SNIPPET_CHARS {
        text.to_string()
    } else {
        let head: String = chars[..MAX_SNIPPET_CHARS].iter().collect();
        format!("{}…", head)
    }
}
