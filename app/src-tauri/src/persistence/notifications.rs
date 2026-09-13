use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use turso::Connection;

pub use crate::core::events::{NotificationRecord, Severity};

/// Input parameters for inserting a new notification.
#[derive(Debug, Clone)]
pub struct NewNotification {
    pub id: String,
    pub group_key: String,
    pub category: String,
    pub severity: Severity,
    pub action_type: String,
    pub action_payload: String,
    pub title: String,
    pub message: String,
    pub status: String,
    pub session_id: Option<i64>,
    pub metadata: String,
}

/// Optional query filter for bulk notification mutations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotificationFilter {
    pub ids: Option<Vec<String>>,
    pub group_key: Option<String>,
    pub category: Option<String>,
    pub action_type: Option<String>,
}

/// Creates a new notification in the database and returns the persisted record.
pub async fn create_notification(
    conn: &Connection,
    notif: &NewNotification,
) -> Result<NotificationRecord> {
    let now = current_timestamp_ms();

    conn.execute(
        "INSERT INTO notifications (
            id, group_key, category, severity, action_type, action_payload,
            title, message, status, session_id, metadata, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            notif.id.clone(),
            notif.group_key.clone(),
            notif.category.clone(),
            notif.severity.as_str().to_string(),
            notif.action_type.clone(),
            notif.action_payload.clone(),
            notif.title.clone(),
            notif.message.clone(),
            notif.status.clone(),
            notif.session_id,
            notif.metadata.clone(),
            now,
            now,
        ),
    )
    .await?;

    Ok(NotificationRecord {
        id: notif.id.clone(),
        group_key: notif.group_key.clone(),
        category: notif.category.clone(),
        severity: notif.severity,
        action_type: notif.action_type.clone(),
        action_payload: notif.action_payload.clone(),
        title: notif.title.clone(),
        message: notif.message.clone(),
        status: notif.status.clone(),
        session_id: notif.session_id,
        metadata: notif.metadata.clone(),
        created_at: now,
        updated_at: now,
    })
}

/// Finds an active (non-dismissed) interactive notification for a specific group key.
pub async fn find_active_interactive_by_group(
    conn: &Connection,
    group_key: &str,
) -> Result<Option<NotificationRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, group_key, category, severity, action_type, action_payload,
                    title, message, status, session_id, metadata, created_at, updated_at
             FROM notifications
             WHERE group_key = ? AND status != 'dismissed' AND action_type = 'interactive'
             ORDER BY created_at DESC
             LIMIT 1",
            (group_key.to_string(),),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(map_notification_row(&row)?))
    } else {
        Ok(None)
    }
}

/// Updates the message, metadata, and updated_at timestamp of an existing notification in place.
pub async fn update_interactive_notification(
    conn: &Connection,
    id: &str,
    message: &str,
    metadata: &str,
) -> Result<()> {
    let now = current_timestamp_ms();
    conn.execute(
        "UPDATE notifications SET message = ?, metadata = ?, updated_at = ? WHERE id = ?",
        (
            message.to_string(),
            metadata.to_string(),
            now,
            id.to_string(),
        ),
    )
    .await?;
    Ok(())
}

/// Fetches all active notifications (excluding dismissed), ordered newest first.
pub async fn fetch_active_notifications(conn: &Connection) -> Result<Vec<NotificationRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, group_key, category, severity, action_type, action_payload,
                    title, message, status, session_id, metadata, created_at, updated_at
             FROM notifications
             WHERE status != 'dismissed'
             ORDER BY created_at DESC",
            (),
        )
        .await?;

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        list.push(map_notification_row(&row)?);
    }

    Ok(list)
}

/// Updates the status of an existing notification.
pub async fn update_notification_status(conn: &Connection, id: &str, status: &str) -> Result<()> {
    let now = current_timestamp_ms();
    conn.execute(
        "UPDATE notifications SET status = ?, updated_at = ? WHERE id = ?",
        (status.to_string(), now, id.to_string()),
    )
    .await?;
    Ok(())
}

/// Marks unread notifications matching the filter (or all unread) as read.
pub async fn mark_notifications_read(
    conn: &Connection,
    filter: Option<&NotificationFilter>,
) -> Result<()> {
    let now = current_timestamp_ms();
    if let Some(f) = filter {
        if let Some(ref ids) = f.ids {
            if ids.is_empty() {
                return Ok(());
            }
            let quoted_ids: Vec<String> = ids
                .iter()
                .map(|id| format!("'{}'", id.replace('\'', "''")))
                .collect();
            let sql = format!(
                "UPDATE notifications SET status = 'read', updated_at = ? WHERE id IN ({}) AND status = 'unread'",
                quoted_ids.join(",")
            );
            conn.execute(&sql, (now,)).await?;
            return Ok(());
        }
        if let Some(ref group_key) = f.group_key {
            conn.execute(
                "UPDATE notifications SET status = 'read', updated_at = ? WHERE group_key = ? AND status = 'unread'",
                (now, group_key.clone()),
            )
            .await?;
            return Ok(());
        }
        if let Some(ref category) = f.category {
            conn.execute(
                "UPDATE notifications SET status = 'read', updated_at = ? WHERE category = ? AND status = 'unread'",
                (now, category.clone()),
            )
            .await?;
            return Ok(());
        }
        if let Some(ref action_type) = f.action_type {
            conn.execute(
                "UPDATE notifications SET status = 'read', updated_at = ? WHERE action_type = ? AND status = 'unread'",
                (now, action_type.clone()),
            )
            .await?;
            return Ok(());
        }
    }

    conn.execute(
        "UPDATE notifications SET status = 'read', updated_at = ? WHERE status = 'unread'",
        (now,),
    )
    .await?;
    Ok(())
}

/// Dismisses active notifications matching the filter (or all active).
pub async fn dismiss_notifications(
    conn: &Connection,
    filter: Option<&NotificationFilter>,
) -> Result<()> {
    let now = current_timestamp_ms();
    if let Some(f) = filter {
        if let Some(ref ids) = f.ids {
            if ids.is_empty() {
                return Ok(());
            }
            let quoted_ids: Vec<String> = ids
                .iter()
                .map(|id| format!("'{}'", id.replace('\'', "''")))
                .collect();
            let sql = format!(
                "UPDATE notifications SET status = 'dismissed', updated_at = ? WHERE id IN ({}) AND status != 'dismissed'",
                quoted_ids.join(",")
            );
            conn.execute(&sql, (now,)).await?;
            return Ok(());
        }
        if let Some(ref group_key) = f.group_key {
            conn.execute(
                "UPDATE notifications SET status = 'dismissed', updated_at = ? WHERE group_key = ? AND status != 'dismissed'",
                (now, group_key.clone()),
            )
            .await?;
            return Ok(());
        }
        if let Some(ref category) = f.category {
            conn.execute(
                "UPDATE notifications SET status = 'dismissed', updated_at = ? WHERE category = ? AND status != 'dismissed'",
                (now, category.clone()),
            )
            .await?;
            return Ok(());
        }
        if let Some(ref action_type) = f.action_type {
            conn.execute(
                "UPDATE notifications SET status = 'dismissed', updated_at = ? WHERE action_type = ? AND status != 'dismissed'",
                (now, action_type.clone()),
            )
            .await?;
            return Ok(());
        }
    }

    conn.execute(
        "UPDATE notifications SET status = 'dismissed', updated_at = ? WHERE status != 'dismissed'",
        (now,),
    )
    .await?;
    Ok(())
}

/// Marks active interactive task cards for an entity-scoped group key as dismissed.
pub async fn dismiss_interactive_by_entity(conn: &Connection, group_key: &str) -> Result<()> {
    let now = current_timestamp_ms();
    conn.execute(
        "UPDATE notifications SET status = 'dismissed', updated_at = ? WHERE group_key = ? AND action_type = 'interactive' AND status != 'dismissed'",
        (now, group_key.to_string()),
    )
    .await?;
    Ok(())
}

/// Convenience function to dismiss a single notification by ID.
pub async fn dismiss_notification(conn: &Connection, id: &str) -> Result<()> {
    dismiss_notifications(
        conn,
        Some(&NotificationFilter {
            ids: Some(vec![id.to_string()]),
            group_key: None,
            category: None,
            action_type: None,
        }),
    )
    .await
}

/// Returns any notification record matching group_key (regardless of status).
pub async fn find_notification_by_group(
    conn: &Connection,
    group_key: &str,
) -> Result<Option<NotificationRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, group_key, category, severity, action_type, action_payload, \
                    title, message, status, session_id, metadata, created_at, updated_at \
             FROM notifications WHERE group_key = ? LIMIT 1",
            (group_key.to_string(),),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(map_notification_row(&row)?))
    } else {
        Ok(None)
    }
}

/// Updates an existing notification's resolution state and message in-place.
pub async fn resolve_notification_in_place(
    conn: &Connection,
    id: &str,
    resolution: &str,
    message: Option<&str>,
) -> Result<Option<NotificationRecord>> {
    let now = current_timestamp_ms();
    let existing = fetch_notification_by_id(conn, id).await?;
    let Some(mut record) = existing else {
        return Ok(None);
    };

    let mut meta_json: serde_json::Value =
        serde_json::from_str(&record.metadata).unwrap_or_else(|_| serde_json::json!({}));
    meta_json["resolution"] = serde_json::Value::String(resolution.to_string());
    let new_metadata = meta_json.to_string();

    let new_message = message.unwrap_or(&record.message).to_string();

    conn.execute(
        "UPDATE notifications SET metadata = ?, message = ?, updated_at = ? WHERE id = ?",
        (
            new_metadata.clone(),
            new_message.clone(),
            now,
            id.to_string(),
        ),
    )
    .await?;

    record.metadata = new_metadata;
    record.message = new_message;
    record.updated_at = now;

    Ok(Some(record))
}

/// Convenience function to mark all unread notifications as read.
pub async fn mark_all_notifications_read(conn: &Connection) -> Result<()> {
    mark_notifications_read(conn, None).await
}

/// Returns true when a notification with the given ID exists (any status).
pub async fn notification_exists(conn: &Connection, id: &str) -> Result<bool> {
    let mut rows = conn
        .query(
            "SELECT id FROM notifications WHERE id = ?",
            (id.to_string(),),
        )
        .await?;
    Ok(rows.next().await?.is_some())
}

/// Fetches a single notification by ID.
pub async fn fetch_notification_by_id(
    conn: &Connection,
    id: &str,
) -> Result<Option<NotificationRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, group_key, category, severity, action_type, action_payload,
                    title, message, status, session_id, metadata, created_at, updated_at
             FROM notifications
             WHERE id = ?",
            (id.to_string(),),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(map_notification_row(&row)?))
    } else {
        Ok(None)
    }
}

/// Finds an active (non-dismissed) notification for a specific session and category.
pub async fn find_active_notification_by_session(
    conn: &Connection,
    session_id: i64,
    category: &str,
) -> Result<Option<NotificationRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, group_key, category, severity, action_type, action_payload,
                    title, message, status, session_id, metadata, created_at, updated_at
             FROM notifications
             WHERE session_id = ? AND category = ? AND status != 'dismissed'
             ORDER BY created_at DESC
             LIMIT 1",
            (session_id, category.to_string()),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(map_notification_row(&row)?))
    } else {
        Ok(None)
    }
}

/// Maps a database row to a strongly-typed `NotificationRecord`.
fn map_notification_row(row: &turso::Row) -> Result<NotificationRecord> {
    let severity_str: String = row.get(3)?;
    Ok(NotificationRecord {
        id: row.get(0)?,
        group_key: row.get(1)?,
        category: row.get(2)?,
        severity: Severity::from(severity_str.as_str()),
        action_type: row.get(4)?,
        action_payload: row.get(5).unwrap_or_else(|_| "{}".to_string()),
        title: row.get(6)?,
        message: row.get(7)?,
        status: row.get(8)?,
        session_id: row.get(9).ok(),
        metadata: row.get(10).unwrap_or_default(),
        created_at: row.get(11).unwrap_or(0),
        updated_at: row.get(12).unwrap_or(0),
    })
}

/// Helper returning current millisecond UNIX epoch.
fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
