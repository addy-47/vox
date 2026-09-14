use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;
use turso::Connection;

use super::{
    router::{resolve_channel, DeliveryChannel},
    Action, NotificationParams,
};
use crate::{
    core::events::{emit_ipc, IpcEvent},
    persistence::{
        db::VoxDb,
        notifications::{
            create_notification, find_active_interactive_by_group, update_interactive_notification,
            NewNotification, NotificationRecord,
        },
    },
};

/// Universal front door for all user alerting across the Vox application.
/// Resolves the delivery channel via the deterministic 3D routing engine and dispatches accordingly.
pub async fn notify<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &VoxDb,
    params: NotificationParams<'_>,
) -> anyhow::Result<Option<String>> {
    let channel = resolve_channel(params.impact, params.severity, &params.action);

    if channel == DeliveryChannel::ToastOnly || channel == DeliveryChannel::ToastAndNotification {
        let toast_ok = dispatch_toast(
            app,
            params.title,
            params.message,
            params.severity,
            params.duration_ms,
        );
        if !toast_ok && channel == DeliveryChannel::ToastOnly {
            let conn = db.connect()?;
            return elevate_toast_to_drawer(app, &conn, &params).await;
        }
    }

    if channel == DeliveryChannel::NotificationOnly
        || channel == DeliveryChannel::ToastAndNotification
    {
        let conn = db.connect()?;
        return dispatch_drawer(app, &conn, &params).await;
    }

    Ok(None)
}

fn dispatch_toast<R: tauri::Runtime>(
    app: &AppHandle<R>,
    title: &str,
    message: &str,
    severity: crate::core::events::Severity,
    duration_ms: Option<u64>,
) -> bool {
    if let Err(e) = crate::toast::show_toast(app, title, message, severity, duration_ms) {
        log::warn!("[Notifications] show_toast overlay dispatch failed: {}", e);
        false
    } else {
        true
    }
}

async fn elevate_toast_to_drawer<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &Connection,
    params: &NotificationParams<'_>,
) -> anyhow::Result<Option<String>> {
    let id = format!(
        "notif_{}_{}",
        params.category.as_str(),
        current_timestamp_ms()
    );
    let group_key = params
        .group_key
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}:elevated", params.category.as_str()));

    let new_notif = NewNotification {
        id: id.clone(),
        group_key,
        category: params.category.as_str().to_string(),
        severity: params.severity,
        action_type: "receipt".to_string(),
        action_payload: "{}".to_string(),
        title: params.title.to_string(),
        message: params.message.to_string(),
        status: "unread".to_string(),
        session_id: params.session_id,
        metadata: params.metadata.unwrap_or("{}").to_string(),
    };

    if let Ok(record) = create_notification(db, &new_notif).await {
        let _ = emit_ipc(app, IpcEvent::NotificationCreated(record));
        return Ok(Some(id));
    }
    Ok(None)
}

async fn dispatch_drawer<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &Connection,
    params: &NotificationParams<'_>,
) -> anyhow::Result<Option<String>> {
    let group_key = params
        .group_key
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}:general", params.category.as_str()));

    if matches!(params.action, Action::Interactive(_)) {
        if let Some(updated_id) =
            try_update_interactive_in_place(app, db, params, &group_key).await?
        {
            return Ok(Some(updated_id));
        }
    }

    insert_new_drawer_notification(app, db, params, group_key).await
}

async fn try_update_interactive_in_place<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &Connection,
    params: &NotificationParams<'_>,
    group_key: &str,
) -> anyhow::Result<Option<String>> {
    if let Ok(Some(existing)) = find_active_interactive_by_group(db, group_key).await {
        let metadata_str = params.metadata.unwrap_or("{}");
        update_interactive_notification(db, &existing.id, params.message, metadata_str).await?;

        let updated_record = NotificationRecord {
            id: existing.id.clone(),
            group_key: existing.group_key,
            category: existing.category,
            severity: params.severity,
            action_type: existing.action_type,
            action_payload: existing.action_payload,
            title: params.title.to_string(),
            message: params.message.to_string(),
            status: existing.status,
            session_id: params.session_id.or(existing.session_id),
            metadata: metadata_str.to_string(),
            created_at: existing.created_at,
            updated_at: current_timestamp_ms(),
        };

        let _ = emit_ipc(app, IpcEvent::NotificationUpdated(updated_record));
        return Ok(Some(existing.id));
    }
    Ok(None)
}

async fn insert_new_drawer_notification<R: tauri::Runtime>(
    app: &AppHandle<R>,
    db: &Connection,
    params: &NotificationParams<'_>,
    group_key: String,
) -> anyhow::Result<Option<String>> {
    let id = format!(
        "notif_{}_{}",
        params.category.as_str(),
        current_timestamp_ms()
    );
    let (action_type, action_payload) = match &params.action {
        Action::Interactive(payload) => (
            "interactive".to_string(),
            serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string()),
        ),
        _ => ("receipt".to_string(), "{}".to_string()),
    };

    let new_notif = NewNotification {
        id: id.clone(),
        group_key,
        category: params.category.as_str().to_string(),
        severity: params.severity,
        action_type,
        action_payload,
        title: params.title.to_string(),
        message: params.message.to_string(),
        status: "unread".to_string(),
        session_id: params.session_id,
        metadata: params.metadata.unwrap_or("{}").to_string(),
    };

    let record = create_notification(db, &new_notif).await?;
    let _ = emit_ipc(app, IpcEvent::NotificationCreated(record.clone()));
    Ok(Some(record.id))
}

fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
