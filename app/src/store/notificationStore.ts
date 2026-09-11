import { create } from "zustand";
import {
  type NotificationRecord,
  type NotificationFilter,
  countsTowardBadge,
  getNotifications,
  markNotificationsRead,
  dismissNotifications,
  executeNotificationAction,
  listenNotificationCreated,
  listenNotificationUpdated,
} from "@/services/notificationService";

export interface RolledUpNotification {
  key: string;
  latest: NotificationRecord;
  count: number;
  hasUnread: boolean;
}

interface NotificationStoreState {
  notifications: NotificationRecord[];
  activeActionIds: string[];
  loading: boolean;
  fetchNotifications: () => Promise<void>;
  markAllRead: (filter?: NotificationFilter) => Promise<void>;
  dismissGroup: (groupKey: string) => Promise<void>;
  executeAction: (notif: NotificationRecord, onNavigate?: (target: string) => void) => Promise<void>;
  initListeners: () => Promise<() => void>;
}

export const useNotificationStore = create<NotificationStoreState>((set, get) => ({
  notifications: [],
  activeActionIds: [],
  loading: false,

  fetchNotifications: async () => {
    try {
      set({ loading: true });
      const records = await getNotifications();
      set({ notifications: records, loading: false });
    } catch (e) {
      logError("Failed to fetch notifications", e);
      set({ loading: false });
    }
  },

  markAllRead: async (filter?: NotificationFilter) => {
    try {
      await markNotificationsRead(filter);
      set((state) => {
        if (!filter) {
          return {
            notifications: state.notifications.map((n) =>
              n.status === "unread" ? { ...n, status: "read", updated_at: Date.now() } : n
            ),
          };
        }
        return {
          notifications: state.notifications.map((n) => {
            const matchId = filter.ids && filter.ids.includes(n.id);
            const matchGroup = filter.group_key && n.group_key === filter.group_key;
            const matchCat = filter.category && n.category === filter.category;
            if (matchId || matchGroup || matchCat) {
              return { ...n, status: "read", updated_at: Date.now() };
            }
            return n;
          }),
        };
      });
    } catch (e) {
      logError("Failed to mark notifications read", e);
    }
  },

  dismissGroup: async (groupKey: string) => {
    try {
      await dismissNotifications({ group_key: groupKey });
      set((state) => ({
        notifications: state.notifications.filter(
          (n) => (n.group_key || n.id) !== groupKey
        ),
      }));
    } catch (e) {
      logError(`Failed to dismiss notifications for group ${groupKey}`, e);
    }
  },

  executeAction: async (notif: NotificationRecord, onNavigate?: (target: string) => void) => {
    let parsed: { action?: string; target?: string } = {};
    try {
      parsed = JSON.parse(notif.action_payload || "{}");
    } catch {
      // safe fallback
    }

    // Client-side Navigate execution (spec §4.3 & §9.1)
    if (parsed.action === "navigate" && parsed.target) {
      onNavigate?.(parsed.target);
      return;
    }

    // Polymorphic backend action execution
    if (get().activeActionIds.includes(notif.id)) return;
    set((state) => ({
      activeActionIds: [...state.activeActionIds, notif.id],
    }));

    try {
      await executeNotificationAction(notif.id);
    } catch (e) {
      logError(`Failed to execute action for notification ${notif.id}`, e);
    } finally {
      set((state) => ({
        activeActionIds: state.activeActionIds.filter((id) => id !== notif.id),
      }));
    }
  },

  initListeners: async () => {
    const unlisteners: (() => void)[] = [];

    unlisteners.push(
      await listenNotificationCreated((notif) => {
        set((state) => {
          const exists = state.notifications.some((n) => n.id === notif.id);
          if (exists) {
            return {
              notifications: state.notifications.map((n) =>
                n.id === notif.id ? notif : n
              ),
            };
          }
          return { notifications: [notif, ...state.notifications] };
        });
      })
    );

    unlisteners.push(
      await listenNotificationUpdated((notif) => {
        set((state) => {
          if (notif.status === "dismissed") {
            return {
              notifications: state.notifications.filter((n) => n.id !== notif.id),
            };
          }
          return {
            notifications: state.notifications.map((n) =>
              n.id === notif.id ? notif : n
            ),
          };
        });
      })
    );

    return () => {
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  },
}));

function logError(msg: string, e: unknown) {
  console.error(`[NotificationStore] ${msg}:`, e);
}

export function selectBadgeCount(state: NotificationStoreState): number {
  return state.notifications.filter(countsTowardBadge).length;
}

/** Group notifications by group_key preserving newest-first ordering and aggregating counts. */
export function selectRolledUpNotifications(
  state: NotificationStoreState
): RolledUpNotification[] {
  const groups = new Map<string, RolledUpNotification>();

  for (const notif of state.notifications) {
    if (notif.status === "dismissed") continue;
    const groupKey = notif.group_key || notif.id;
    const existing = groups.get(groupKey);

    if (!existing) {
      groups.set(groupKey, {
        key: groupKey,
        latest: notif,
        count: 1,
        hasUnread: notif.status === "unread",
      });
    } else {
      existing.count += 1;
      if (notif.status === "unread") {
        existing.hasUnread = true;
      }
      if (notif.created_at > existing.latest.created_at) {
        existing.latest = notif;
      }
    }
  }

  return Array.from(groups.values()).sort(
    (a, b) => b.latest.created_at - a.latest.created_at
  );
}
