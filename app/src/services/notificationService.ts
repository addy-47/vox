import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { NotificationRecord } from "./eventsService";

export type { NotificationRecord };

/** Backend notification categories (persistence/notifications.rs & services/notifications/types.rs). */
export type NotificationCategory =
  | "session_compaction"
  | "memory_consolidation"
  | "pipeline"
  | "dictation"
  | "hardware"
  | "models"
  | "storage";

export const KNOWN_CATEGORIES: readonly NotificationCategory[] = [
  "session_compaction",
  "memory_consolidation",
  "pipeline",
  "dictation",
  "hardware",
  "models",
  "storage",
];

export interface NotificationFilter {
  ids?: string[];
  group_key?: string;
  category?: string;
}

/** Normalize unknown future categories to the pipeline presentation. */
export function toCategory(category: string): NotificationCategory {
  return (KNOWN_CATEGORIES as readonly string[]).includes(category)
    ? (category as NotificationCategory)
    : "pipeline";
}

/** Terminal receipts render dimmed with an occurrence rollup and zero badge weight. */
export function isReceipt(notif: NotificationRecord): boolean {
  return notif.action_type === "receipt";
}

/** Counts badge weight: unread, interactive tasks only. */
export function countsTowardBadge(notif: NotificationRecord): boolean {
  return notif.status === "unread" && notif.action_type === "interactive";
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

/** Fetches all active (non-dismissed) notifications */
export function getNotifications(): Promise<NotificationRecord[]> {
  return invoke("get_notifications");
}

/** Marks unread notifications matching the filter (or all unread) as read */
export function markNotificationsRead(filter?: NotificationFilter): Promise<void> {
  return invoke("mark_notifications_read", { filter });
}

/** Dismisses active notifications matching the filter (or all active) */
export function dismissNotifications(filter?: NotificationFilter): Promise<void> {
  return invoke("dismiss_notifications", { filter });
}

/** Polymorphic backend action execution */
export function executeNotificationAction(id: string, action?: string): Promise<void> {
  return invoke("execute_notification_action", { id, action });
}

import {
  onNotificationCreated,
  onNotificationUpdated,
} from "./eventsService";

/** Subscribes to notification creation events. */
export function listenNotificationCreated(
  cb: (notif: NotificationRecord) => void
): Promise<UnlistenFn> {
  const unlisten = onNotificationCreated(cb);
  return Promise.resolve(unlisten);
}

/** Subscribes to notification updated events. */
export function listenNotificationUpdated(
  cb: (notif: NotificationRecord) => void
): Promise<UnlistenFn> {
  const unlisten = onNotificationUpdated(cb);
  return Promise.resolve(unlisten);
}
