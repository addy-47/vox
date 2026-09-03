import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface NotificationRecord {
  id: string;
  category: string;
  title: string;
  message: string;
  status: "unread" | "read" | "dismissed" | string;
  session_id: number | null;
  turn_id: number | null;
  action_payload: string | null;
  created_at_ms: number;
  read_at_ms: number | null;
  dismissed_at_ms: number | null;
}

/** Fetches all active (unread or read) notifications */
export function getNotifications(): Promise<NotificationRecord[]> {
  return invoke("get_notifications");
}

/** Marks all notifications as read */
export function markNotificationsRead(): Promise<void> {
  return invoke("mark_notifications_read");
}

/** Dismisses a specific notification */
export function dismissNotification(id: string): Promise<void> {
  return invoke("dismiss_notification", { id });
}

/** Triggers background compaction for a session */
export function triggerSessionCompaction(sessionId: number): Promise<void> {
  return invoke("trigger_session_compaction", { sessionId });
}

/** Subscribes to notification creation events */
export function listenNotificationCreated(
  cb: (notif: NotificationRecord) => void
): Promise<UnlistenFn> {
  return listen<NotificationRecord>("notification_created", (event) => {
    cb(event.payload);
  });
}

/** Subscribes to notification updated events */
export function listenNotificationUpdated(
  cb: (notif: NotificationRecord) => void
): Promise<UnlistenFn> {
  return listen<NotificationRecord>("notification_updated", (event) => {
    cb(event.payload);
  });
}

/** Subscribes to notification dismissed events */
export function listenNotificationDismissed(
  cb: (payload: { id: string }) => void
): Promise<UnlistenFn> {
  return listen<{ id: string }>("notification_dismissed", (event) => {
    cb(event.payload);
  });
}

/** Subscribes to mark all notifications read events */
export function listenNotificationsMarkedRead(
  cb: () => void
): Promise<UnlistenFn> {
  return listen("notifications_marked_read", () => {
    cb();
  });
}
