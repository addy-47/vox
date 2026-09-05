import { create } from "zustand";
import {
  type NotificationRecord,
  countsTowardBadge,
  getNotifications,
  markNotificationsRead,
  dismissNotification,
  triggerSessionCompaction,
  listenNotificationCreated,
  listenNotificationUpdated,
  listenNotificationDismissed,
  listenNotificationsMarkedRead,
} from "@/services/notificationService";

interface NotificationStoreState {
  notifications: NotificationRecord[];
  compactingSessionIds: number[];
  loading: boolean;
  isOpen: boolean;

  setIsOpen: (open: boolean) => void;
  fetchNotifications: () => Promise<void>;
  markAllRead: () => Promise<void>;
  dismiss: (id: string) => Promise<void>;
  triggerCompaction: (sessionId: number) => Promise<void>;
  initListeners: () => Promise<() => void>;
}

export const useNotificationStore = create<NotificationStoreState>((set, get) => ({
  notifications: [],
  compactingSessionIds: [],
  loading: false,
  isOpen: false,

  setIsOpen: (isOpen) => set({ isOpen }),

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

  markAllRead: async () => {
    try {
      await markNotificationsRead();
      set((state) => ({
        notifications: state.notifications.map((n) => ({
          ...n,
          is_read: true,
          status: "read",
        })),
      }));
    } catch (e) {
      logError("Failed to mark notifications read", e);
    }
  },

  dismiss: async (id: string) => {
    try {
      await dismissNotification(id);
      set((state) => ({
        notifications: state.notifications.filter((n) => n.id !== id),
      }));
    } catch (e) {
      logError(`Failed to dismiss notification ${id}`, e);
    }
  },

  triggerCompaction: async (sessionId: number) => {
    if (get().compactingSessionIds.includes(sessionId)) return;

    set((state) => ({
      compactingSessionIds: [...state.compactingSessionIds, sessionId],
    }));

    try {
      await triggerSessionCompaction(sessionId);
    } catch (e) {
      logError(`Failed to trigger compaction for session ${sessionId}`, e);
      set((state) => ({
        compactingSessionIds: state.compactingSessionIds.filter((id) => id !== sessionId),
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
          // If the notification was completed or dismissed, remove it from compactingSessionIds
          const isFinished = notif.status === "dismissed" || notif.category === "session_compaction_completed";
          const nextCompacting = isFinished && notif.session_id
            ? state.compactingSessionIds.filter((id) => id !== notif.session_id)
            : state.compactingSessionIds;

          if (notif.status === "dismissed") {
            return {
              notifications: state.notifications.filter((n) => n.id !== notif.id),
              compactingSessionIds: nextCompacting,
            };
          }

          return {
            notifications: state.notifications.map((n) =>
              n.id === notif.id ? notif : n
            ),
            compactingSessionIds: nextCompacting,
          };
        });
      })
    );

    unlisteners.push(
      await listenNotificationDismissed(({ id }) => {
        set((state) => ({
          notifications: state.notifications.filter((n) => n.id !== id),
        }));
      })
    );

    unlisteners.push(
      await listenNotificationsMarkedRead(() => {
        set((state) => ({
          notifications: state.notifications.map((n) => ({
            ...n,
            is_read: true,
            status: "read",
          })),
        }));
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

/** Badge weight: unread, actionable notifications only (receipts excluded). */
export function selectBadgeCount(state: NotificationStoreState): number {
  return state.notifications.filter(countsTowardBadge).length;
}
