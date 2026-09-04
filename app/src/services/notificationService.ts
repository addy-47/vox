import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { NotificationRecord } from "./eventsService";

export type { NotificationRecord };

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
  return invoke("trigger_session_compaction", { session_id: sessionId });
}

import {
  onNotificationCreated,
  onNotificationUpdated,
  onNotificationDismissed,
  onNotificationsMarkedRead,
} from "./eventsService";

/** Subscribes to notification creation events */
export function listenNotificationCreated(
  cb: (notif: NotificationRecord) => void
): Promise<UnlistenFn> {
  const unlisten = onNotificationCreated(cb);
  return Promise.resolve(unlisten);
}

/** Subscribes to notification updated events */
export function listenNotificationUpdated(
  cb: (notif: NotificationRecord) => void
): Promise<UnlistenFn> {
  const unlisten = onNotificationUpdated(cb);
  return Promise.resolve(unlisten);
}

/** Subscribes to notification dismissed events */
export function listenNotificationDismissed(
  cb: (payload: { id: string }) => void
): Promise<UnlistenFn> {
  const unlisten = onNotificationDismissed(cb);
  return Promise.resolve(unlisten);
}

/** Subscribes to mark all notifications read events */
export function listenNotificationsMarkedRead(
  cb: () => void
): Promise<UnlistenFn> {
  const unlisten = onNotificationsMarkedRead(cb);
  return Promise.resolve(unlisten);
}
