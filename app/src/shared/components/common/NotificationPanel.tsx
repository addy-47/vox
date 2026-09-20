import { useState, useEffect, memo, useCallback, useMemo, useRef } from "react";
import { useNavigate } from "react-router-dom";
import {
  Bell,
  Check,
  X,
  Loader2,
  Sparkles,
  RotateCcw,
  Settings,
  Layers,
  Brain,
  Activity,
  FileText,
  Headphones,
  Cpu,
  Database,
  AlertTriangle,
  AlertCircle,
  Clock,
  Calendar,
  ChevronRight,
  type LucideIcon,
} from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { NOTIFICATION_COPY } from "@/data/notificationCopy";
import {
  useNotificationStore,
  selectBadgeCount,
  selectTasksRolledUp,
  selectUpdatesRolledUp,
  type RolledUpNotification,
} from "@/store/notificationStore";
import {
  toCategory,
  isReceipt,
  metadataTurnCount,
  metadataResolution,
  type NotificationRecord,
  type NotificationCategory,
} from "@/services/notificationService";
import { formatSessionRecency } from "@/services/historyService";

interface NotificationPanelProps {
  onClose: () => void;
}

interface CategoryVisual {
  icon: LucideIcon;
  tile: string;
}

const CATEGORY_VISUALS: Record<NotificationCategory, CategoryVisual> = {
  session_compaction: {
    icon: Layers,
    tile: "bg-[rgba(var(--violet),0.12)] text-[rgb(var(--violet))] border-[rgba(var(--violet),0.30)]",
  },
  memory_consolidation: {
    icon: Brain,
    tile: "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.30)]",
  },
  pipeline: {
    icon: Activity,
    tile: "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] border-[rgba(var(--border),0.18)]",
  },
  dictation: {
    icon: FileText,
    tile: "bg-[rgba(var(--accent),0.10)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.25)]",
  },
  hardware: {
    icon: Headphones,
    tile: "bg-[rgba(var(--warning),0.12)] text-[rgb(var(--warning))] border-[rgba(var(--warning),0.30)]",
  },
  models: {
    icon: Cpu,
    tile: "bg-[rgba(var(--violet),0.12)] text-[rgb(var(--violet))] border-[rgba(var(--violet),0.30)]",
  },
  storage: {
    icon: Database,
    tile: "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] border-[rgba(var(--border),0.18)]",
  },
};

function formatClockTime(timestampMs: number): string {
  return new Date(timestampMs).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatTurnMeta(turns: number | null): string | null {
  if (turns === null) return null;
  const unit = turns === 1 ? NOTIFICATION_COPY.turnSingular : NOTIFICATION_COPY.turnPlural;
  return `${turns} ${unit}`;
}

function getTimeGroup(timestampMs: number): "today" | "yesterday" | "earlier" {
  const now = new Date();
  const date = new Date(timestampMs);
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today.getTime() - 86400000);
  if (date >= today) return "today";
  if (date >= yesterday) return "yesterday";
  return "earlier";
}

const TIME_GROUP_LABELS = {
  today: "Today",
  yesterday: "Yesterday",
  earlier: "Earlier",
} as const;

const TIME_GROUP_ICONS = {
  today: Clock,
  yesterday: Calendar,
  earlier: Clock,
} as const;

const itemVariants = {
  hidden: { opacity: 0, y: 6 },
  visible: { opacity: 1, y: 0, transition: { duration: 0.15, ease: "easeOut" as const } },
  exit: { opacity: 0, scale: 0.97, transition: { duration: 0.12 } },
};

const NotificationItem = memo(
  ({
    group,
    isWorking,
    onPrimary,
    onDismiss,
    onOpen,
  }: {
    group: RolledUpNotification;
    isWorking: boolean;
    onPrimary: (notif: NotificationRecord) => void;
    onDismiss: (groupKey: string) => void;
    onOpen: (notif: NotificationRecord) => void;
  }) => {
    const notif = group.latest;
    const category = toCategory(notif.category);
    const visual = CATEGORY_VISUALS[category] ?? CATEGORY_VISUALS.pipeline;
    const Icon = visual.icon;
    const receipt = isReceipt(notif);
    const unread = group.hasUnread && !receipt;
    const isCritical = notif.severity === "critical";
    const isWarning = notif.severity === "warning";
    const hasSession = notif.session_id !== null && notif.session_id !== undefined;
    const blurb =
      NOTIFICATION_COPY.categoryBlurb[category] ?? NOTIFICATION_COPY.categoryBlurb.pipeline;

    const handlePrimary = useCallback(() => {
      onPrimary(notif);
    }, [onPrimary, notif]);

    const handleDismiss = useCallback(() => {
      onDismiss(group.key);
    }, [onDismiss, group.key]);

    const handleOpen = useCallback(() => {
      onOpen(notif);
    }, [onOpen, notif]);

    const turnMeta = formatTurnMeta(metadataTurnCount(notif));
    const resolution = metadataResolution(notif);
    const isInteractive = notif.action_type === "interactive";

    const { ActionIcon, actionTooltip } = useMemo(() => {
      if (!isInteractive) {
        return { ActionIcon: Sparkles, actionTooltip: NOTIFICATION_COPY.compactTooltip };
      }
      let parsedAction: { action?: string; target?: string } = {};
      try {
        parsedAction = JSON.parse(notif.action_payload || "{}");
      } catch {
        // safe fallback
      }

      if (category === "session_compaction" || parsedAction.action === "compact_session") {
        return { ActionIcon: Sparkles, actionTooltip: NOTIFICATION_COPY.compactTooltip };
      }
      if (
        category === "memory_consolidation" ||
        parsedAction.action === "consolidate_memory"
      ) {
        return { ActionIcon: Brain, actionTooltip: NOTIFICATION_COPY.consolidateTooltip };
      }
      if (parsedAction.action === "retry") {
        return { ActionIcon: RotateCcw, actionTooltip: NOTIFICATION_COPY.retryTooltip };
      }
      if (parsedAction.action === "navigate" || parsedAction.target) {
        return { ActionIcon: Settings, actionTooltip: NOTIFICATION_COPY.settingsTooltip };
      }
      return { ActionIcon: Sparkles, actionTooltip: NOTIFICATION_COPY.compactTooltip };
    }, [isInteractive, category, notif.action_payload]);

    return (
      <motion.div
        variants={itemVariants}
        initial="hidden"
        animate="visible"
        exit="exit"
        className={cn(
          "group relative flex gap-3 px-3 py-3 rounded-xl border transition-colors duration-200",
          unread
            ? "border-[rgba(var(--accent),0.25)] bg-[rgba(var(--card),0.75)] hover:border-[rgba(var(--accent),0.40)] hover:bg-[rgba(var(--card),0.9)]"
            : "border-[rgba(var(--border),0.1)] bg-[rgba(var(--card),0.4)] hover:border-[rgba(var(--border),0.18)] hover:bg-[rgba(var(--card),0.55)]",
          receipt && "opacity-70 hover:opacity-90"
        )}
      >
        <Tooltip label={blurb} side="top">
          <span className={cn("w-9 h-9 rounded-lg border flex items-center justify-center shrink-0", visual.tile)}>
            <Icon size={16} strokeWidth={1.75} />
          </span>
        </Tooltip>

        <div className="flex-1 min-w-0 flex flex-col gap-1">
          <div className="flex items-start justify-between gap-2">
            <div className="flex items-center gap-1.5 min-w-0">
              {unread && (
                <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shrink-0" />
              )}
              <span className="text-[13px] font-semibold text-[rgb(var(--foreground))] truncate">
                {notif.title}
              </span>
              {group.count > 1 && (
                <span className="shrink-0 font-mono tabular-nums text-[11px] text-[rgb(var(--foreground-muted))]/70">
                  ×{group.count}
                </span>
              )}
            </div>
            <div className="flex items-center gap-0.5 shrink-0">
              <span className="font-mono tabular-nums text-[11px] text-[rgb(var(--foreground-muted))]/70 px-1">
                {formatClockTime(notif.created_at)}
              </span>
              <Tooltip label={NOTIFICATION_COPY.dismiss} side="left">
                <button
                  type="button"
                  onClick={(e) => { e.stopPropagation(); handleDismiss(); }}
                  className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-all cursor-pointer shrink-0 [@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-hover:opacity-100 [@media(hover:hover)]:group-focus-within:opacity-100 focus-visible:opacity-100"
                  aria-label={NOTIFICATION_COPY.dismiss}
                >
                  <X size={13} />
                </button>
              </Tooltip>
            </div>
          </div>

          {notif.message.trim() && (
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed break-words line-clamp-2">
              {notif.message}
            </p>
          )}

          <div className="flex items-center gap-2 pt-0.5">
            <span className="text-[11px] text-[rgb(var(--foreground-muted))]/70 truncate">
              {formatSessionRecency(notif.created_at)}
              {turnMeta && ` · ${turnMeta}`}
            </span>
            <div className="flex-1" aria-hidden="true" />
            {isCritical && (
              <span className="inline-flex items-center gap-1 text-[11px] font-semibold text-[rgb(var(--error))] shrink-0">
                <AlertCircle size={11} />
                {NOTIFICATION_COPY.severityCritical}
              </span>
            )}
            {isWarning && (
              <span className="inline-flex items-center gap-1 text-[11px] font-semibold text-[rgb(var(--warning))] shrink-0">
                <AlertTriangle size={11} />
                {NOTIFICATION_COPY.severityWarning}
              </span>
            )}

            {resolution === "resolved" ? (
              <Tooltip label={NOTIFICATION_COPY.resolvedTooltip} side="top">
                <span className="flex items-center justify-center w-7 h-7 rounded-lg border border-[rgba(var(--accent),0.3)] bg-[rgba(var(--accent),0.08)] text-[rgb(var(--accent))]">
                  <Check size={13} />
                </span>
              </Tooltip>
            ) : isInteractive ? (
              <Tooltip label={actionTooltip} side="top">
                <button
                  type="button"
                  disabled={isWorking}
                  onClick={(e) => { e.stopPropagation(); handlePrimary(); }}
                  aria-label={actionTooltip}
                  className={cn(
                    "flex items-center justify-center w-7 h-7 rounded-lg border transition-colors cursor-pointer",
                    isWorking
                      ? "border-[rgba(var(--accent),0.4)] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] cursor-wait"
                      : "border-[rgba(var(--accent),0.35)] bg-[rgba(var(--accent),0.10)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.20)] hover:border-[rgba(var(--accent),0.5)] active:scale-95"
                  )}
                >
                  {isWorking ? (
                    <Loader2 size={13} className="animate-spin" />
                  ) : (
                    <ActionIcon size={13} />
                  )}
                </button>
              </Tooltip>
            ) : null}

            {(hasSession || !receipt) && (
              <button
                type="button"
                onClick={(e) => { e.stopPropagation(); handleOpen(); }}
                className="px-2 py-1 rounded-lg text-[11px] font-semibold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer inline-flex items-center gap-0.5 shrink-0"
              >
                {NOTIFICATION_COPY.view}
                <ChevronRight size={10} />
              </button>
            )}
          </div>
        </div>
      </motion.div>
    );
  }
);
NotificationItem.displayName = "NotificationItem";

const TimeGroupHeader = memo(function TimeGroupHeader({ group }: { group: keyof typeof TIME_GROUP_LABELS }) {
  const Icon = TIME_GROUP_ICONS[group];
  return (
    <div className="flex items-center gap-2 px-1 py-2">
      <Icon size={13} className="text-[rgb(var(--foreground-muted))]/50" />
      <span className="text-[11px] font-bold uppercase tracking-[0.14em] text-[rgb(var(--foreground-muted))]/60">
        {TIME_GROUP_LABELS[group]}
      </span>
      <div className="flex-1 h-px bg-[rgba(var(--border),0.08)]" />
    </div>
  );
});
TimeGroupHeader.displayName = "TimeGroupHeader";

/**
 * Notification list rendered inside EdgePanel with Tasks and Updates tabs.
 * Automatically marks unread notifications as read upon opening.
 * Groups items by time period (Today / Yesterday / Earlier).
 */
export const NotificationPanel = memo(({ onClose }: NotificationPanelProps) => {
  const navigate = useNavigate();
  const [activeTab, setActiveTab] = useState<"tasks" | "updates">("tasks");
  const tabListRef = useRef<HTMLDivElement>(null);
  const tasks = useNotificationStore(selectTasksRolledUp);
  const updates = useNotificationStore(selectUpdatesRolledUp);
  const badgeCount = useNotificationStore(selectBadgeCount);
  const activeActionIds = useNotificationStore((s) => s.activeActionIds);
  const markAllRead = useNotificationStore((s) => s.markAllRead);
  const dismissGroup = useNotificationStore((s) => s.dismissGroup);
  const dismissTab = useNotificationStore((s) => s.dismissTab);
  const executeAction = useNotificationStore((s) => s.executeAction);
  const loading = useNotificationStore((s) => s.loading);

  // Deferred past the 220ms rail slide so the IPC round-trip + store
  // churn never lands inside the open animation's frame budget.
  useEffect(() => {
    const t = setTimeout(() => {
      markAllRead().catch(() => {});
    }, 280);
    return () => clearTimeout(t);
  }, [markAllRead]);

  const displayedItems = activeTab === "tasks" ? tasks : updates;

  interface TimeSection {
    group: keyof typeof TIME_GROUP_LABELS;
    items: RolledUpNotification[];
  }

  /** Group notifications into stable time-group sections (Today / Yesterday / Earlier) */
  const sections = useMemo<TimeSection[]>(() => {
    const order: Array<keyof typeof TIME_GROUP_LABELS> = ["today", "yesterday", "earlier"];
    const byGroup = new Map<keyof typeof TIME_GROUP_LABELS, RolledUpNotification[]>();

    for (const item of displayedItems) {
      const g = getTimeGroup(item.latest.created_at);
      const list = byGroup.get(g);
      if (list) {
        list.push(item);
      } else {
        byGroup.set(g, [item]);
      }
    }

    return order
      .filter((g) => byGroup.has(g))
      .map((g) => ({
        group: g,
        items: byGroup.get(g)!.sort((a, b) => b.latest.created_at - a.latest.created_at),
      }));
  }, [displayedItems]);

  const handleDismissAll = useCallback(() => {
    dismissTab(activeTab).catch(() => {});
  }, [dismissTab, activeTab]);

  const handlePrimary = useCallback(
    (notif: NotificationRecord) => {
      executeAction(notif, (target) => {
        onClose();
        const route = target.startsWith("/") ? target : `/${target}`;
        navigate(route);
      }).catch(() => {});
    },
    [executeAction, onClose, navigate]
  );

  const handleOpen = useCallback(
    (notif: NotificationRecord) => {
      onClose();
      const category = toCategory(notif.category);
      if (notif.session_id !== null && notif.session_id !== undefined) {
        navigate(`/history?sessionId=${notif.session_id}`);
      } else if (category === "memory_consolidation") {
        navigate("/memory");
      } else {
        navigate("/settings");
      }
    },
    [navigate, onClose]
  );

  return (
    <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col gap-0 p-3 pb-24">

      {/* ── Underline tab bar ── */}
      <div
        ref={tabListRef}
        role="tablist"
        aria-label="Notification tabs"
        className="flex items-end gap-0 mb-4 border-b border-[rgba(var(--border),0.1)]"
        onKeyDown={(e) => {
          if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
            e.preventDefault();
            const buttons = tabListRef.current?.querySelectorAll<HTMLButtonElement>('[data-arrow-nav]');
            if (!buttons || buttons.length === 0) return;
            const currentIndex = Array.from(buttons).findIndex((b) => b === document.activeElement);
            let nextIndex: number;
            if (e.key === "ArrowLeft") {
              nextIndex = currentIndex <= 0 ? buttons.length - 1 : currentIndex - 1;
            } else {
              nextIndex = currentIndex >= buttons.length - 1 ? 0 : currentIndex + 1;
            }
            buttons[nextIndex]?.focus();
            buttons[nextIndex]?.click();
          }
        }}
      >
        {/* Tasks tab */}
        <button
          type="button"
          id="notification-tab-tasks"
          role="tab"
          aria-selected={activeTab === "tasks"}
          aria-controls="notification-tabpanel"
          tabIndex={activeTab === "tasks" ? 0 : -1}
          data-arrow-nav
          onClick={() => setActiveTab("tasks")}
          className={cn(
            "relative flex items-center gap-1.5 px-3 pb-2.5 pt-1 text-[12px] font-semibold transition-colors duration-150 cursor-pointer shrink-0",
            activeTab === "tasks"
              ? "text-[rgb(var(--foreground))]"
              : "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground-muted))]"
          )}
        >
          <span>{NOTIFICATION_COPY.tasksTab}</span>
          {tasks.length > 0 && (
            <span
              className={cn(
                "font-mono text-[10px] font-bold tabular-nums transition-colors",
                activeTab === "tasks"
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/50"
              )}
            >
              {tasks.length}
            </span>
          )}
          {/* Active underline */}
          {activeTab === "tasks" && (
            <span
              className="absolute bottom-0 left-0 right-0 h-[2px] rounded-full bg-[rgb(var(--accent))]"
            />
          )}
        </button>

        {/* Updates tab */}
        <button
          type="button"
          id="notification-tab-updates"
          role="tab"
          aria-selected={activeTab === "updates"}
          aria-controls="notification-tabpanel"
          tabIndex={activeTab === "updates" ? 0 : -1}
          data-arrow-nav
          onClick={() => setActiveTab("updates")}
          className={cn(
            "relative flex items-center gap-1.5 px-3 pb-2.5 pt-1 text-[12px] font-semibold transition-colors duration-150 cursor-pointer shrink-0",
            activeTab === "updates"
              ? "text-[rgb(var(--foreground))]"
              : "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground-muted))]"
          )}
        >
          <span>{NOTIFICATION_COPY.updatesTab}</span>
          {updates.length > 0 && (
            <span
              className={cn(
                "font-mono text-[10px] font-bold tabular-nums transition-colors",
                activeTab === "updates"
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/50"
              )}
            >
              {updates.length}
            </span>
          )}
          {activeTab === "updates" && (
            <span
              className="absolute bottom-0 left-0 right-0 h-[2px] rounded-full bg-[rgb(var(--accent))]"
            />
          )}
        </button>

        {/* Spacer + dismiss-all */}
        <div className="flex-1" />
        {displayedItems.length > 0 && (
          <Tooltip label={NOTIFICATION_COPY.dismissAllTooltip} side="left">
            <button
              type="button"
              onClick={handleDismissAll}
              className="mb-2 flex items-center gap-1 text-[11px] font-medium text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground-muted))] transition-colors cursor-pointer px-1.5 py-1 rounded-lg hover:bg-[rgba(var(--foreground),0.04)]"
              aria-label={NOTIFICATION_COPY.dismissAllTooltip}
            >
              <X size={11} />
              <span>{NOTIFICATION_COPY.dismissAll}</span>
            </button>
          </Tooltip>
        )}

        {/* Unread badge (only when panel has unread count) */}
        {badgeCount > 0 && (
          <span className="mb-2 ml-1 min-w-[18px] h-[18px] px-1 rounded-full bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] font-mono text-[11px] font-black leading-none flex items-center justify-center">
            {badgeCount > 9 ? "9+" : badgeCount}
          </span>
        )}
      </div>

      {/* Content list */}
      {loading && displayedItems.length === 0 ? (
        <div id="notification-tabpanel" role="tabpanel" aria-labelledby="notification-tab-tasks" className="flex flex-col gap-2.5" aria-hidden="true">
          {[0, 1, 2].map((i) => (
            <div
              key={i}
              className="flex gap-3 p-3.5 rounded-xl border border-[rgba(var(--border),0.1)] bg-[rgba(var(--card),0.4)] animate-pulse"
            >
              <div className="w-9 h-9 rounded-xl bg-[rgba(var(--foreground),0.08)] shrink-0" />
              <div className="flex-1 flex flex-col gap-2 py-0.5">
                <div className="flex items-center gap-2">
                  <div className="h-3.5 rounded-md bg-[rgba(var(--foreground),0.1)] w-2/5" />
                  <div className="h-2.5 rounded-md bg-[rgba(var(--foreground),0.07)] w-12" />
                </div>
                <div className="h-2.5 rounded-md bg-[rgba(var(--foreground),0.07)] w-1/2" />
                <div className="h-2 rounded-md bg-[rgba(var(--foreground),0.05)] w-3/5" />
              </div>
            </div>
          ))}
        </div>
      ) : displayedItems.length === 0 ? (
        <div id="notification-tabpanel" role="tabpanel" aria-labelledby="notification-tab-tasks" className="flex flex-col items-center justify-center py-16 px-4 text-center gap-3">
          <div className="w-14 h-14 rounded-2xl border border-[rgba(var(--accent),0.2)] bg-[rgba(var(--accent),0.05)] flex items-center justify-center mb-1">
            <Bell size={24} className="text-[rgb(var(--accent))]/50" />
          </div>
            <h3 className="font-display text-[16px] font-black uppercase tracking-[0.12em] text-[rgb(var(--foreground))]">
            {activeTab === "tasks" ? NOTIFICATION_COPY.tasksEmptyTitle : NOTIFICATION_COPY.updatesEmptyTitle}
          </h3>
          <p className="text-[12px] text-[rgb(var(--foreground-muted))]/70 max-w-[240px] leading-relaxed">
            {activeTab === "tasks" ? NOTIFICATION_COPY.tasksEmptySubtitle : NOTIFICATION_COPY.updatesEmptySubtitle}
          </p>
        </div>
      ) : (
        <div id="notification-tabpanel" role="tabpanel" aria-labelledby="notification-tab-tasks" className="flex flex-col gap-3">
          {sections.map(({ group, items }) => (
            <div key={group} className="flex flex-col gap-1.5">
              <TimeGroupHeader group={group} />
              <AnimatePresence initial={false}>
                {items.map((item) => (
                  <NotificationItem
                    key={item.key}
                    group={item}
                    isWorking={activeActionIds.includes(item.latest.id)}
                    onPrimary={handlePrimary}
                    onDismiss={dismissGroup}
                    onOpen={handleOpen}
                  />
                ))}
              </AnimatePresence>
            </div>
          ))}
        </div>
      )}
    </div>
  );
});
NotificationPanel.displayName = "NotificationPanel";
