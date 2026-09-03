use anyhow::Result;
use turso::Connection;

pub use crate::core::events::NotificationRecord;

/// Input parameters for inserting a new notification.
#[derive(Debug, Clone)]
pub struct NewNotification {
    pub id: String,
    pub category: String,
    pub title: String,
    pub message: String,
    pub status: String,
    pub session_id: Option<i64>,
    pub metadata: String,
}

/// Creates a new notification in the database and returns the persisted record.
pub async fn create_notification(
    conn: &Connection,
    notif: &NewNotification,
) -> Result<NotificationRecord> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    conn.execute(
        "INSERT INTO notifications (id, category, title, message, status, session_id, metadata, is_read, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)",
        (
            notif.id.clone(),
            notif.category.clone(),
            notif.title.clone(),
            notif.message.clone(),
            notif.status.clone(),
            notif.session_id,
            notif.metadata.clone(),
            now,
        ),
    )
    .await?;

    Ok(NotificationRecord {
        id: notif.id.clone(),
        category: notif.category.clone(),
        title: notif.title.clone(),
        message: notif.message.clone(),
        status: notif.status.clone(),
        session_id: notif.session_id,
        metadata: notif.metadata.clone(),
        is_read: false,
        created_at: now,
    })
}

/// Fetches all active notifications (excluding dismissed), ordered newest first.
pub async fn fetch_active_notifications(conn: &Connection) -> Result<Vec<NotificationRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, category, title, message, status, session_id, metadata, is_read, created_at
             FROM notifications
             WHERE status != 'dismissed'
             ORDER BY created_at DESC",
            (),
        )
        .await?;

    let mut list = Vec::new();
    while let Some(row) = rows.next().await? {
        let is_read_int: i64 = row.get(7).unwrap_or(0);
        list.push(NotificationRecord {
            id: row.get(0)?,
            category: row.get(1)?,
            title: row.get(2)?,
            message: row.get(3)?,
            status: row.get(4)?,
            session_id: row.get(5).ok(),
            metadata: row.get(6).unwrap_or_default(),
            is_read: is_read_int != 0,
            created_at: row.get(8).unwrap_or(0),
        });
    }

    Ok(list)
}

/// Updates the status of an existing notification.
pub async fn update_notification_status(conn: &Connection, id: &str, status: &str) -> Result<()> {
    conn.execute(
        "UPDATE notifications SET status = ? WHERE id = ?",
        (status.to_string(), id.to_string()),
    )
    .await?;
    Ok(())
}

/// Marks all unread notifications as read.
pub async fn mark_all_notifications_read(conn: &Connection) -> Result<()> {
    conn.execute("UPDATE notifications SET is_read = 1 WHERE is_read = 0", ())
        .await?;
    Ok(())
}

/// Dismisses a notification by marking its status as 'dismissed' and is_read as 1.
pub async fn dismiss_notification(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE notifications SET status = 'dismissed', is_read = 1 WHERE id = ?",
        (id.to_string(),),
    )
    .await?;
    Ok(())
}

/// Finds an active (non-dismissed) notification for a specific session and category.
pub async fn find_active_notification_by_session(
    conn: &Connection,
    session_id: i64,
    category: &str,
) -> Result<Option<NotificationRecord>> {
    let mut rows = conn
        .query(
            "SELECT id, category, title, message, status, session_id, metadata, is_read, created_at
             FROM notifications
             WHERE session_id = ? AND category = ? AND status != 'dismissed'
             ORDER BY created_at DESC
             LIMIT 1",
            (session_id, category.to_string()),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        let is_read_int: i64 = row.get(7).unwrap_or(0);
        Ok(Some(NotificationRecord {
            id: row.get(0)?,
            category: row.get(1)?,
            title: row.get(2)?,
            message: row.get(3)?,
            status: row.get(4)?,
            session_id: row.get(5).ok(),
            metadata: row.get(6).unwrap_or_default(),
            is_read: is_read_int != 0,
            created_at: row.get(8).unwrap_or(0),
        }))
    } else {
        Ok(None)
    }
}
