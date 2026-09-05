import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { NotificationRecord } from "./eventsService";

export type { NotificationRecord };

/** Backend notification categories (persistence/notifications.rs). */
export type NotificationCategory =
  | "session_compaction"
  | "model_ready"
  | "model_failed"
  | "memory_issue"
  | "storage_health";

const KNOWN_CATEGORIES: readonly string[] = [
  "session_compaction",
  "model_ready",
  "model_failed",
  "memory_issue",
  "storage_health",
];

/** Normalize unknown future categories to the compaction presentation. */
export function toCategory(category: string): NotificationCategory {
  return (KNOWN_CATEGORIES as readonly string[]).includes(category)
    ? (category as NotificationCategory)
    : "session_compaction";
}

/** Terminal receipts render dimmed with a status chip and no badge weight. */
export function isReceipt(notif: NotificationRecord): boolean {
  return notif.status === "completed" || notif.status === "failed";
}

/** Counts badge weight: unread, non-dismissed, non-receipt. */
export function countsTowardBadge(notif: NotificationRecord): boolean {
  if (notif.is_read) return false;
  if (notif.status === "dismissed") return false;
  return !isReceipt(notif);
}

/** Best-effort turn count from notification metadata JSON. */
export function metadataTurnCount(notif: NotificationRecord): number | null {
  try {
    const parsed: unknown = JSON.parse(notif.metadata || "{}");
    if (parsed && typeof parsed === "object" && "uncompacted_turns" in parsed) {
      const n = (parsed as { uncompacted_turns: unknown }).uncompacted_turns;
      if (typeof n === "number" && Number.isFinite(n)) return Math.max(0, Math.floor(n));
    }
  } catch {
    // malformed metadata is a backend data issue, never a UI crash
  }
  return null;
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
