import { memo, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import {
  Bell,
  Check,
  X,
  Loader2,
  Layers,
  Brain,
  Activity,
  FileText,
  Headphones,
  Cpu,
  Database,
  AlertTriangle,
  AlertCircle,
  type LucideIcon,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { NOTIFICATION_COPY } from "@/data/notificationCopy";
import {
  useNotificationStore,
  selectBadgeCount,
  selectRolledUpNotifications,
  type RolledUpNotification,
} from "@/store/notificationStore";
import {
  toCategory,
  isReceipt,
  metadataTurnCount,
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

    // Dynamic Action labels
    let actionLabel: string = NOTIFICATION_COPY.view;
    let workingLabel: string = NOTIFICATION_COPY.view;
    let isInteractive = notif.action_type === "interactive";

    if (isInteractive) {
      let parsedAction: { action?: string; target?: string } = {};
      try {
        parsedAction = JSON.parse(notif.action_payload || "{}");
      } catch {
        // safe fallback
      }

      if (category === "session_compaction" || parsedAction.action === "compact_session") {
        actionLabel = NOTIFICATION_COPY.tidyNow;
        workingLabel = NOTIFICATION_COPY.tidying;
      } else if (
        category === "memory_consolidation" ||
        parsedAction.action === "consolidate_memory"
      ) {
        actionLabel = NOTIFICATION_COPY.consolidate;
        workingLabel = NOTIFICATION_COPY.consolidating;
      } else if (parsedAction.action === "retry") {
        actionLabel = NOTIFICATION_COPY.retry;
        workingLabel = NOTIFICATION_COPY.retrying;
      } else if (parsedAction.action === "navigate" || parsedAction.target) {
        actionLabel = NOTIFICATION_COPY.openSettings;
        workingLabel = NOTIFICATION_COPY.openSettings;
      } else {
        actionLabel = NOTIFICATION_COPY.tidyNow;
        workingLabel = NOTIFICATION_COPY.tidying;
      }
    }

    return (
      <div
        className={cn(
          "group relative flex gap-2.5 p-3 rounded-xl border transition-all duration-200",
          isCritical
            ? "border-[rgba(var(--error),0.35)] bg-[rgba(var(--error),0.05)] shadow-[0_4px_20px_rgba(var(--error),0.08)]"
            : isWarning
              ? "border-[rgba(var(--warning),0.30)] bg-[rgba(var(--warning),0.04)] shadow-[0_4px_20px_rgba(var(--warning),0.06)]"
              : unread
                ? "border-[rgba(var(--accent),0.25)] bg-[rgba(var(--card),0.75)] shadow-[0_4px_20px_rgba(var(--accent),0.06)]"
                : "border-[rgba(var(--border),0.1)] bg-[rgba(var(--card),0.4)]",
          receipt && "opacity-80"
        )}
      >
        <div className={cn("w-8 h-8 rounded-xl border flex items-center justify-center shrink-0", visual.tile)}>
          <Icon size={15} strokeWidth={1.75} />
        </div>

        <div className="flex-1 min-w-0 flex flex-col gap-1">
          <div className="flex items-start justify-between gap-2">
            <div className="flex items-center gap-1.5 min-w-0">
              {unread && <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shrink-0 animate-pulse" />}
              <span className="text-[13px] font-semibold text-[rgb(var(--foreground))] truncate">
                {notif.title}
              </span>
              {group.count > 1 && (
                <span className="shrink-0 px-1.5 py-0.5 rounded-full bg-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground-muted))] font-mono text-[10px] font-bold">
                  (×{group.count})
                </span>
              )}
            </div>
            <Tooltip label={NOTIFICATION_COPY.dismiss} side="left">
              <button
                type="button"
                onClick={handleDismiss}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-all cursor-pointer shrink-0"
                aria-label={NOTIFICATION_COPY.dismiss}
              >
                <X size={13} />
              </button>
            </Tooltip>
          </div>

          <div className="flex items-center gap-1.5 text-[11px] text-[rgb(var(--foreground-muted))]">
            <span>{formatSessionRecency(notif.created_at)}</span>
            <span aria-hidden="true">·</span>
            <span className="font-mono tabular-nums">{formatClockTime(notif.created_at)}</span>
            {turnMeta && (
              <>
                <span aria-hidden="true">·</span>
                <span>{turnMeta}</span>
              </>
            )}
            <span aria-hidden="true">·</span>
            <span className="truncate">{blurb}</span>
          </div>

          {notif.message.trim() && (
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed break-words">
              {notif.message}
            </p>
          )}

          <div className="flex items-center gap-2 pt-1">
            {isCritical && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider border border-[rgba(var(--error),0.35)] text-[rgb(var(--error))]">
                <AlertCircle size={10} />
                Critical
              </span>
            )}
            {isWarning && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider border border-[rgba(var(--warning),0.35)] text-[rgb(var(--warning))]">
                <AlertTriangle size={10} />
                Warning
              </span>
            )}

            {isInteractive && (
              <button
                type="button"
                disabled={isWorking}
                onClick={handlePrimary}
                className={cn(
                  "flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all cursor-pointer",
                  isWorking
                    ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] cursor-wait"
                    : "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:opacity-90 active:scale-95 shadow-sm"
                )}
              >
                {isWorking && <Loader2 size={11} className="animate-spin" />}
                <span>{isWorking ? workingLabel : actionLabel}</span>
              </button>
            )}

            {(hasSession || !receipt) && (
              <button
                type="button"
                onClick={handleOpen}
                className="px-2.5 py-1 rounded-lg text-[11px] font-semibold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer"
              >
                {NOTIFICATION_COPY.view}
              </button>
            )}
          </div>
        </div>
      </div>
    );
  }
);
NotificationItem.displayName = "NotificationItem";

/**
 * Notification list rendered inside EdgePanel. Pure list — fetch + listener
 * lifecycle runs once at app level (App.tsx). Dismissal and Escape are
 * handled by the EdgePanel overlay registration.
 */
export const NotificationPanel = memo(({ onClose }: NotificationPanelProps) => {
  const navigate = useNavigate();
  const rolledUpNotifications = useNotificationStore(selectRolledUpNotifications);
  const badgeCount = useNotificationStore(selectBadgeCount);
  const activeActionIds = useNotificationStore((s) => s.activeActionIds);
  const markAllRead = useNotificationStore((s) => s.markAllRead);
  const dismissGroup = useNotificationStore((s) => s.dismissGroup);
  const executeAction = useNotificationStore((s) => s.executeAction);
  const loading = useNotificationStore((s) => s.loading);

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
    <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col gap-2 p-3 pb-16">
      <div className="flex items-center justify-between px-1 pb-1">
        <div className="flex items-center gap-2">
          <Bell size={14} className="text-[rgb(var(--accent))]" />
          <span className="text-[13px] font-display font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
            {NOTIFICATION_COPY.title}
          </span>
          {badgeCount > 0 && (
            <span className="px-1.5 py-0.5 rounded-full bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] font-mono text-[10px] font-black leading-none">
              {badgeCount}
            </span>
          )}
        </div>
        {badgeCount > 0 && (
          <button
            type="button"
            onClick={() => markAllRead().catch(() => {})}
            className="flex items-center gap-1 text-[11px] font-semibold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
          >
            <Check size={11} />
            <span>{NOTIFICATION_COPY.markAllRead}</span>
          </button>
        )}
      </div>

      {loading && rolledUpNotifications.length === 0 ? (
        <div className="flex flex-col gap-2" aria-hidden="true">
          {[0, 1, 2].map((i) => (
            <div
              key={i}
              className="flex gap-2.5 p-3 rounded-xl border border-[rgba(var(--border),0.1)] bg-[rgba(var(--card),0.4)] animate-pulse"
            >
              <div className="w-8 h-8 rounded-xl bg-[rgba(var(--foreground),0.08)] shrink-0" />
              <div className="flex-1 flex flex-col gap-1.5 py-0.5">
                <div className="h-3 rounded-md bg-[rgba(var(--foreground),0.1)] w-2/3" />
                <div className="h-2.5 rounded-md bg-[rgba(var(--foreground),0.07)] w-1/2" />
              </div>
            </div>
          ))}
        </div>
      ) : rolledUpNotifications.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-10 px-4 text-center">
          <div className="w-10 h-10 rounded-full border border-[rgba(var(--accent),0.2)] bg-[rgba(var(--accent),0.05)] flex items-center justify-center mb-2.5">
            <Bell size={18} className="text-[rgb(var(--accent))]/60" />
          </div>
          <span className="text-[13px] font-semibold text-[rgb(var(--foreground))]">{NOTIFICATION_COPY.emptyTitle}</span>
          <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 max-w-[220px] mt-1 leading-relaxed">
            {NOTIFICATION_COPY.emptySubtitle}
          </p>
        </div>
      ) : (
        rolledUpNotifications.map((group) => (
          <NotificationItem
            key={group.key}
            group={group}
            isWorking={activeActionIds.includes(group.latest.id)}
            onPrimary={handlePrimary}
            onDismiss={dismissGroup}
            onOpen={handleOpen}
          />
        ))
      )}
      {/* Bottom spacing cushion */}
      <div className="h-8 shrink-0" aria-hidden="true" />
    </div>
  );
});
NotificationPanel.displayName = "NotificationPanel";
