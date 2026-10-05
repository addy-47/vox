import { useState, useEffect, memo, useCallback, useMemo, useRef } from "react";
import { useNavigate } from "react-router-dom";
import {
  Bell,
  Check,
  X,
  Loader2,
  Shrink,
  RotateCcw,
  Settings,
  Brain,
  Activity,
  FileText,
  Headphones,
  Cpu,
  Database,
  AlertCircle,
  Clock,
  Calendar,
  ChevronRight,
  RotateCw,
  type LucideIcon,
} from "lucide-react";
import { motion } from "framer-motion";
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

interface NotificationPanelProps {
  onClose: () => void;
}

interface CategoryVisual {
  icon: LucideIcon;
  accent: string;
  action: string;
  actionWorking: string;
}

/**
 * Single source of category colour: the accent-derived `--notif-*` family
 * (notifications-spec §4.7). Never `--accent` itself. Full literal class
 * strings so Tailwind's scanner sees every arbitrary value.
 */
const CATEGORY_VISUALS: Record<NotificationCategory, CategoryVisual> = {
  session_compaction: {
    icon: Shrink,
    accent: "text-[rgb(var(--notif-session-compaction))]",
    action:
      "border-[rgba(var(--notif-session-compaction),0.35)] bg-[rgba(var(--notif-session-compaction),0.10)] text-[rgb(var(--notif-session-compaction))] hover:bg-[rgba(var(--notif-session-compaction),0.20)] hover:border-[rgba(var(--notif-session-compaction),0.5)] active:scale-95",
    actionWorking:
      "border-[rgba(var(--notif-session-compaction),0.4)] bg-[rgba(var(--notif-session-compaction),0.15)] text-[rgb(var(--notif-session-compaction))] cursor-wait",
  },
  memory_consolidation: {
    icon: Brain,
    accent: "text-[rgb(var(--notif-memory-consolidation))]",
    action:
      "border-[rgba(var(--notif-memory-consolidation),0.35)] bg-[rgba(var(--notif-memory-consolidation),0.10)] text-[rgb(var(--notif-memory-consolidation))] hover:bg-[rgba(var(--notif-memory-consolidation),0.20)] hover:border-[rgba(var(--notif-memory-consolidation),0.5)] active:scale-95",
    actionWorking:
      "border-[rgba(var(--notif-memory-consolidation),0.4)] bg-[rgba(var(--notif-memory-consolidation),0.15)] text-[rgb(var(--notif-memory-consolidation))] cursor-wait",
  },
  pipeline: {
    icon: Activity,
    accent: "text-[rgb(var(--notif-pipeline))]",
    action:
      "border-[rgba(var(--notif-pipeline),0.35)] bg-[rgba(var(--notif-pipeline),0.10)] text-[rgb(var(--notif-pipeline))] hover:bg-[rgba(var(--notif-pipeline),0.20)] hover:border-[rgba(var(--notif-pipeline),0.5)] active:scale-95",
    actionWorking:
      "border-[rgba(var(--notif-pipeline),0.4)] bg-[rgba(var(--notif-pipeline),0.15)] text-[rgb(var(--notif-pipeline))] cursor-wait",
  },
  dictation: {
    icon: FileText,
    accent: "text-[rgb(var(--notif-dictation))]",
    action:
      "border-[rgba(var(--notif-dictation),0.35)] bg-[rgba(var(--notif-dictation),0.10)] text-[rgb(var(--notif-dictation))] hover:bg-[rgba(var(--notif-dictation),0.20)] hover:border-[rgba(var(--notif-dictation),0.5)] active:scale-95",
    actionWorking:
      "border-[rgba(var(--notif-dictation),0.4)] bg-[rgba(var(--notif-dictation),0.15)] text-[rgb(var(--notif-dictation))] cursor-wait",
  },
  hardware: {
    icon: Headphones,
    accent: "text-[rgb(var(--notif-hardware))]",
    action:
      "border-[rgba(var(--notif-hardware),0.35)] bg-[rgba(var(--notif-hardware),0.10)] text-[rgb(var(--notif-hardware))] hover:bg-[rgba(var(--notif-hardware),0.20)] hover:border-[rgba(var(--notif-hardware),0.5)] active:scale-95",
    actionWorking:
      "border-[rgba(var(--notif-hardware),0.4)] bg-[rgba(var(--notif-hardware),0.15)] text-[rgb(var(--notif-hardware))] cursor-wait",
  },
  models: {
    icon: Cpu,
    accent: "text-[rgb(var(--notif-models))]",
    action:
      "border-[rgba(var(--notif-models),0.35)] bg-[rgba(var(--notif-models),0.10)] text-[rgb(var(--notif-models))] hover:bg-[rgba(var(--notif-models),0.20)] hover:border-[rgba(var(--notif-models),0.5)] active:scale-95",
    actionWorking:
      "border-[rgba(var(--notif-models),0.4)] bg-[rgba(var(--notif-models),0.15)] text-[rgb(var(--notif-models))] cursor-wait",
  },
  storage: {
    icon: Database,
    accent: "text-[rgb(var(--notif-storage))]",
    action:
      "border-[rgba(var(--notif-storage),0.35)] bg-[rgba(var(--notif-storage),0.10)] text-[rgb(var(--notif-storage))] hover:bg-[rgba(var(--notif-storage),0.20)] hover:border-[rgba(var(--notif-storage),0.5)] active:scale-95",
    actionWorking:
      "border-[rgba(var(--notif-storage),0.4)] bg-[rgba(var(--notif-storage),0.15)] text-[rgb(var(--notif-storage))] cursor-wait",
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

const NotificationItem = memo(
  ({
    group,
    isWorking,
    isInitialUnread,
    onPrimary,
    onDismiss,
    onOpen,
  }: {
    group: RolledUpNotification;
    isWorking: boolean;
    isInitialUnread?: boolean;
    onPrimary: (notif: NotificationRecord) => void;
    onDismiss: (groupKey: string) => void;
    onOpen: (notif: NotificationRecord) => void;
  }) => {
    const notif = group.latest;
    const category = toCategory(notif.category);
    const visual = CATEGORY_VISUALS[category] ?? CATEGORY_VISUALS.pipeline;
    const Icon = visual.icon;
    const receipt = isReceipt(notif);
    const unread = isInitialUnread ?? group.hasUnread;
    const isCritical = notif.severity === "critical";
    const isWarning = notif.severity === "warning";
    const hasSession = notif.session_id !== null && notif.session_id !== undefined;
    const kicker = NOTIFICATION_COPY.categoryKicker[category] ?? NOTIFICATION_COPY.categoryKicker.pipeline;
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
    const message = notif.message.trim();

    const { ActionIcon, actionTooltip } = useMemo(() => {
      if (!isInteractive) {
        return { ActionIcon: Shrink, actionTooltip: NOTIFICATION_COPY.compactTooltip };
      }
      let parsedAction: { action?: string; target?: string } = {};
      try {
        parsedAction = JSON.parse(notif.action_payload || "{}");
      } catch {
        // safe fallback
      }

      if (category === "session_compaction" || parsedAction.action === "compact_session") {
        return { ActionIcon: Shrink, actionTooltip: NOTIFICATION_COPY.compactTooltip };
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
      return { ActionIcon: Shrink, actionTooltip: NOTIFICATION_COPY.compactTooltip };
    }, [isInteractive, category, notif.action_payload]);

    return (
      <div
        className={cn(
          "group relative flex flex-col gap-1.5 px-3 py-2.5 rounded-lg border transition-colors duration-200",
          unread
            ? "border-[rgba(var(--accent),0.25)] bg-[rgba(var(--card),0.75)] hover:border-[rgba(var(--accent),0.40)] hover:bg-[rgba(var(--card),0.9)]"
            : "border-[rgba(var(--border),0.1)] bg-[rgba(var(--card),0.4)] hover:border-[rgba(var(--border),0.18)] hover:bg-[rgba(var(--card),0.55)]",
          isCritical &&
            "border-[rgba(var(--error),0.65)] shadow-[0_0_12px_-4px_rgba(var(--error),0.4)]",
          isWarning &&
            "border-[rgba(var(--warning),0.55)] shadow-[0_0_12px_-4px_rgba(var(--warning),0.35)]",
          receipt && !unread && "opacity-75 hover:opacity-95"
        )}
      >
        {/* Kicker: category identity. Unboxed typography per design-spec 5.1.
            Severity lives on the card border only (notifications-spec §4.1). */}
        <div className="flex items-center gap-1.5 min-w-0">
          <Tooltip label={blurb} side="top">
            <span className="flex items-center gap-1.5 min-w-0 cursor-help">
              <Icon size={12} strokeWidth={2} className={cn("shrink-0", visual.accent)} />
              <span
                className={cn(
                  "font-mono text-[11px] font-bold uppercase tracking-[0.14em] truncate",
                  visual.accent
                )}
              >
                {kicker}
              </span>
            </span>
          </Tooltip>
          {group.count > 1 && (
            <span className="shrink-0 font-mono tabular-nums text-[11px] text-[rgb(var(--foreground-muted))]/70">
              ×{group.count}
            </span>
          )}
          <div className="flex-1" aria-hidden="true" />
          <Tooltip label={NOTIFICATION_COPY.dismiss} side="left">
            <button
              type="button"
              onClick={(e) => { e.stopPropagation(); handleDismiss(); }}
              className="w-8 h-8 -mr-1.5 -my-1 flex items-center justify-center rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-all cursor-pointer shrink-0 [@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-hover:opacity-100 [@media(hover:hover)]:group-focus-within:opacity-100 focus-visible:opacity-100"
              aria-label={NOTIFICATION_COPY.dismiss}
            >
              <X size={13} />
            </button>
          </Tooltip>
        </div>

        {/* Heading */}
        <span className="font-display text-[15px] font-bold leading-snug tracking-tight text-[rgb(var(--foreground))] break-words">
          {notif.title}
        </span>

        {message && (
          <>
            <div className="h-px w-full bg-[rgba(var(--border),0.08)]" aria-hidden="true" />
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed break-words line-clamp-3">
              {message}
            </p>
          </>
        )}

        <div className="flex items-center gap-2 pt-0.5">
          <span className="font-mono tabular-nums text-[11px] text-[rgb(var(--foreground-muted))]/70 truncate">
            {formatClockTime(notif.created_at)}
            {turnMeta && ` · ${turnMeta}`}
          </span>
          <div className="flex-1" aria-hidden="true" />

          {resolution === "resolved" ? (
            <Tooltip label={NOTIFICATION_COPY.resolvedTooltip} side="top">
              <span
                className={cn(
                  "flex items-center justify-center w-8 h-8 rounded-md border transition-colors",
                  visual.action
                )}
              >
                <Check size={13} />
              </span>
            </Tooltip>
          ) : resolution === "failed" ? (
            <Tooltip label={NOTIFICATION_COPY.failedTooltip} side="top">
              <span className="flex items-center justify-center w-8 h-8 rounded-md border border-[rgba(var(--error),0.35)] bg-[rgba(var(--error),0.1)] text-[rgb(var(--error))]">
                <AlertCircle size={13} />
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
                  "flex items-center justify-center w-8 h-8 rounded-md border transition-colors cursor-pointer",
                  isWorking ? visual.actionWorking : visual.action
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
              className="h-8 px-2.5 flex items-center gap-0.5 rounded-md text-[11px] font-semibold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer shrink-0"
            >
              {NOTIFICATION_COPY.view}
              <ChevronRight size={10} />
            </button>
          )}
        </div>
      </div>
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
  const error = useNotificationStore((s) => s.error);
  const fetchNotifications = useNotificationStore((s) => s.fetchNotifications);

  // Snapshot unread group keys upon initial mount of the panel so visual styling
  // remains completely stable without mid-slide flickering when markAllRead resolves.
  const unreadSnapshotRef = useRef<Set<string> | null>(null);
  if (!unreadSnapshotRef.current && (tasks.length > 0 || updates.length > 0)) {
    unreadSnapshotRef.current = new Set(
      [...tasks, ...updates].filter((n) => n.hasUnread).map((n) => n.key)
    );
  }

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
    <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden custom-scrollbar flex flex-col gap-0 p-3 pb-24 [scrollbar-gutter:stable]">

      {/* ── Masthead ── */}
      {badgeCount > 0 && (
        <div className="flex items-baseline justify-end gap-2 shrink-0 mb-2.5">
          <span className="font-mono tabular-nums text-[11px] text-[rgb(var(--accent))]/80 shrink-0">
            {NOTIFICATION_COPY.unreadSummary(badgeCount)}
          </span>
        </div>
      )}

      {/* ── Underline tab bar (canonical design-spec 5.2 grammar) ── */}
      <div
        className="flex items-end gap-0 mb-2 px-0.5 border-b border-[rgba(var(--accent),0.08)] select-none overflow-x-auto no-scrollbar shrink-0"
      >
        <div
          ref={tabListRef}
          role="tablist"
          aria-label={NOTIFICATION_COPY.tabsAriaLabel}
          className="flex items-end gap-0"
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
          {([
            { id: "tasks", label: NOTIFICATION_COPY.tasksTab, count: tasks.length },
            { id: "updates", label: NOTIFICATION_COPY.updatesTab, count: updates.length },
          ] as const).map((tab, index) => {
            const isActive = activeTab === tab.id;
            return (
              <span key={tab.id} className="flex items-end">
                {index > 0 && (
                  <span
                    className="text-[11px] text-[rgb(var(--foreground-muted))]/25 select-none pb-1.5 px-1 sm:px-2"
                    aria-hidden="true"
                  >
                    |
                  </span>
                )}
                <button
                  type="button"
                  id={`notification-tab-${tab.id}`}
                  role="tab"
                  aria-selected={isActive}
                  aria-controls="notification-tabpanel"
                  tabIndex={isActive ? 0 : -1}
                  data-arrow-nav
                  onClick={() => setActiveTab(tab.id)}
                  className={cn(
                    "flex items-center gap-1.5 px-2 pb-1.5 pt-1 text-[11px] sm:text-[11.5px] font-mono font-bold uppercase tracking-[0.06em] border-b-2 transition-colors duration-150 cursor-pointer shrink-0",
                    isActive
                      ? "border-[rgb(var(--accent))] text-[rgb(var(--accent))]"
                      : "border-transparent text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))]"
                  )}
                >
                  <span>{tab.label}</span>
                  {tab.count > 0 && (
                    <span className="font-mono tabular-nums text-[11px] opacity-70">{tab.count}</span>
                  )}
                </button>
              </span>
            );
          })}
        </div>

        <div className="flex-1" />
        {displayedItems.length > 0 && (
          <Tooltip label={NOTIFICATION_COPY.dismissAllTooltip} side="left">
            <button
              type="button"
              onClick={handleDismissAll}
              className="mb-1 flex items-center gap-1 h-8 px-2 text-[11px] font-medium text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer rounded-md hover:bg-[rgba(var(--foreground),0.04)]"
              aria-label={NOTIFICATION_COPY.dismissAllTooltip}
            >
              <X size={11} />
              <span>{NOTIFICATION_COPY.dismissAll}</span>
            </button>
          </Tooltip>
        )}
      </div>

      {/* Content list */}
      {error && displayedItems.length === 0 ? (
        /* A failed fetch is not an empty list. This panel is where pipeline
           failures surface, so it must not claim there is nothing to report. */
        <div
          id="notification-tabpanel"
          aria-labelledby={`notification-tab-${activeTab}`}
          role="alert"
          className="flex flex-col items-center justify-center py-16 px-4 text-center gap-3"
        >
          <div className="w-14 h-14 rounded-2xl border border-[rgba(var(--danger),0.25)] bg-[rgba(var(--danger),0.07)] flex items-center justify-center mb-1">
            <Bell size={24} className="text-[rgb(var(--danger))]/70" />
          </div>
          <h3 className="font-display text-[16px] font-black uppercase tracking-[0.12em] text-[rgb(var(--foreground))]">
            {NOTIFICATION_COPY.fetchFailedTitle}
          </h3>
          <p className="text-[12px] text-[rgb(var(--foreground-muted))]/70 max-w-[240px] leading-relaxed">
            {NOTIFICATION_COPY.fetchFailedSubtitle}
          </p>
          <button
            type="button"
            onClick={() => void fetchNotifications()}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-mono border border-[rgba(var(--border),0.3)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.4)] hover:bg-[rgba(var(--accent),0.08)] transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <RotateCw size={11} className={loading ? "animate-spin" : undefined} />
            {NOTIFICATION_COPY.fetchFailedRetry}
          </button>
        </div>
      ) : loading && displayedItems.length === 0 ? (
        <div id="notification-tabpanel" role="tabpanel" aria-labelledby={`notification-tab-${activeTab}`} className="flex flex-col gap-2.5" aria-hidden="true">
          {[0, 1, 2].map((i) => (
            <div
              key={i}
              className="flex flex-col gap-1.5 p-2.5 rounded-lg border border-[rgba(var(--border),0.1)] bg-[rgba(var(--card),0.4)] animate-pulse"
            >
              <div className="flex items-center gap-1.5">
                <div className="h-2.5 w-2.5 rounded-full bg-[rgba(var(--foreground),0.1)]" />
                <div className="h-2.5 rounded-md bg-[rgba(var(--foreground),0.08)] w-20" />
              </div>
              <div className="h-3.5 rounded-md bg-[rgba(var(--foreground),0.1)] w-3/5" />
              <div className="h-px w-full bg-[rgba(var(--border),0.08)]" />
              <div className="h-2.5 rounded-md bg-[rgba(var(--foreground),0.06)] w-full" />
              <div className="h-2.5 rounded-md bg-[rgba(var(--foreground),0.05)] w-2/5" />
            </div>
          ))}
        </div>
      ) : displayedItems.length === 0 ? (
        <div id="notification-tabpanel" role="tabpanel" aria-labelledby={`notification-tab-${activeTab}`} className="flex flex-col items-center justify-center py-16 px-4 text-center gap-3">
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
        <motion.div
          key={activeTab}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0.15, ease: "easeOut" }}
          className="flex flex-col"
        >
        <div id="notification-tabpanel" role="tabpanel" aria-labelledby={`notification-tab-${activeTab}`} className="flex flex-col gap-3">
          {sections.map(({ group, items }) => (
            <div key={group} className="flex flex-col gap-1.5">
              <TimeGroupHeader group={group} />
              {items.map((item) => (
                <NotificationItem
                  key={item.key}
                  group={item}
                  isWorking={activeActionIds.includes(item.latest.id)}
                  isInitialUnread={unreadSnapshotRef.current ? unreadSnapshotRef.current.has(item.key) : item.hasUnread}
                  onPrimary={handlePrimary}
                  onDismiss={dismissGroup}
                  onOpen={handleOpen}
                />
              ))}
            </div>
          ))}
        </div>
        </motion.div>
      )}
    </div>
  );
});
NotificationPanel.displayName = "NotificationPanel";
