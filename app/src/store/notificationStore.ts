import { create } from "zustand";
import {
  type NotificationRecord,
  type NotificationFilter,
  countsTowardBadge,
  metadataResolution,
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
  dismissTab: (tab: "tasks" | "updates") => Promise<void>;
  executeAction: (notif: NotificationRecord, onNavigate?: (target: string) => void) => Promise<void>;
  executeCompactionForSession: (sessionId: number, onNavigate?: (target: string) => void) => Promise<void>;
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

  dismissTab: async (tab: "tasks" | "updates") => {
    try {
      await dismissNotifications({
        action_type: tab === "tasks" ? "interactive" : "receipt",
      });
      set((state) => ({
        notifications: state.notifications.filter((n) => {
          if (tab === "tasks") {
            return !(n.action_type === "interactive" && metadataResolution(n) !== "resolved");
          }
          return !(n.action_type === "receipt" || metadataResolution(n) === "resolved");
        }),
      }));
    } catch (e) {
      logError(`Failed to dismiss tab ${tab}`, e);
    }
  },

  executeCompactionForSession: async (sessionId: number, onNavigate?: (target: string) => void) => {
    const notif = get().notifications.find(
      (n) =>
        n.category === "session_compaction" &&
        n.session_id === sessionId &&
        n.status !== "dismissed" &&
        metadataResolution(n) !== "resolved"
    );
    if (notif) {
      await get().executeAction(notif, onNavigate);
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

let cachedBadgeCountNotificationsRef: NotificationRecord[] | null = null;
let cachedBadgeCount = 0;

export function selectBadgeCount(state: NotificationStoreState): number {
  if (state.notifications === cachedBadgeCountNotificationsRef) {
    return cachedBadgeCount;
  }
  cachedBadgeCountNotificationsRef = state.notifications;
  cachedBadgeCount = state.notifications.filter(countsTowardBadge).length;
  return cachedBadgeCount;
}


let cachedNotificationsRef: NotificationRecord[] | null = null;
let cachedRolledUpResult: RolledUpNotification[] = [];

/** Group notifications by group_key preserving newest-first ordering and aggregating counts.
 * Cached by state.notifications reference to guarantee useSyncExternalStore referential equality.
 */
export function selectRolledUpNotifications(
  state: NotificationStoreState
): RolledUpNotification[] {
  if (state.notifications === cachedNotificationsRef) {
    return cachedRolledUpResult;
  }
  cachedNotificationsRef = state.notifications;

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

  cachedRolledUpResult = Array.from(groups.values()).sort(
    (a, b) => b.latest.created_at - a.latest.created_at
  );
  return cachedRolledUpResult;
}

let cachedTasksRef: NotificationRecord[] | null = null;
let cachedTasksResult: RolledUpNotification[] = [];

export function selectTasksRolledUp(
  state: NotificationStoreState
): RolledUpNotification[] {
  const all = selectRolledUpNotifications(state);
  if (state.notifications === cachedTasksRef) {
    return cachedTasksResult;
  }
  cachedTasksRef = state.notifications;
  cachedTasksResult = all.filter(
    (g) => g.latest.action_type === "interactive" && metadataResolution(g.latest) !== "resolved"
  );
  return cachedTasksResult;
}

let cachedUpdatesRef: NotificationRecord[] | null = null;
let cachedUpdatesResult: RolledUpNotification[] = [];

export function selectUpdatesRolledUp(
  state: NotificationStoreState
): RolledUpNotification[] {
  const all = selectRolledUpNotifications(state);
  if (state.notifications === cachedUpdatesRef) {
    return cachedUpdatesResult;
  }
  cachedUpdatesRef = state.notifications;
  cachedUpdatesResult = all.filter(
    (g) => g.latest.action_type === "receipt" || metadataResolution(g.latest) === "resolved"
  );
  return cachedUpdatesResult;
}

export function selectUncompactedSessionIds(
  state: NotificationStoreState
): Set<number> {
  const ids = new Set<number>();
  for (const n of state.notifications) {
    if (
      n.category === "session_compaction" &&
      n.session_id !== null &&
      n.session_id !== undefined &&
      n.status !== "dismissed" &&
      metadataResolution(n) !== "resolved"
    ) {
      ids.add(n.session_id);
    }
  }
  return ids;
}

