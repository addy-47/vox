import { memo, useCallback } from "react";
import { motion } from "framer-motion";
import {
  Bell,
  Check,
  X,
  Loader2,
  Layers,
  Download,
  CloudOff,
  Brain,
  Database,
  AlertTriangle,
  type LucideIcon,
} from "lucide-react";
import { useNavigate } from "react-router-dom";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { NOTIFICATION_COPY } from "@/data/notificationCopy";
import { useNotificationStore, selectBadgeCount } from "@/store/notificationStore";
import {
  toCategory,
  isReceipt,
  metadataTurnCount,
  type NotificationRecord,
  type NotificationCategory,
} from "@/services/notificationService";
import { formatSessionRecency } from "@/services/historyService";

interface NotificationsPopoverProps {
  panelRef: React.RefObject<HTMLDivElement | null>;
  onClose: () => void;
}

interface CategoryVisual {
  icon: LucideIcon;
  tile: string;
}

const CATEGORY_VISUALS: Record<NotificationCategory, CategoryVisual> = {
  session_compaction: {
    icon: Layers,
    tile:
      "bg-[rgba(var(--violet),0.12)] text-[rgb(var(--violet))] border-[rgba(var(--violet),0.30)]",
  },
  model_ready: {
    icon: Download,
    tile:
      "bg-[rgba(var(--success),0.12)] text-[rgb(var(--success))] border-[rgba(var(--success),0.30)]",
  },
  model_failed: {
    icon: CloudOff,
    tile:
      "bg-[rgba(var(--error),0.12)] text-[rgb(var(--error))] border-[rgba(var(--error),0.30)]",
  },
  memory_issue: {
    icon: Brain,
    tile:
      "bg-[rgba(var(--warning),0.14)] text-[rgb(var(--warning))] border-[rgba(var(--warning),0.30)]",
  },
  storage_health: {
    icon: Database,
    tile:
      "bg-[rgba(var(--error),0.12)] text-[rgb(var(--error))] border-[rgba(var(--error),0.30)]",
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
    notif,
    isWorking,
    onPrimary,
    onDismiss,
    onOpen,
  }: {
    notif: NotificationRecord;
    isWorking: boolean;
    onPrimary: (notif: NotificationRecord) => void;
    onDismiss: (id: string) => void;
    onOpen: (notif: NotificationRecord) => void;
  }) => {
    const category = toCategory(notif.category);
    const visual = CATEGORY_VISUALS[category];
    const Icon = visual.icon;
    const receipt = isReceipt(notif);
    const failed = notif.status === "failed";
    const unread = !notif.is_read && !receipt;
    const hasSession = notif.session_id !== null && notif.session_id !== undefined;
    const blurb =
      NOTIFICATION_COPY.categoryBlurb[category] ?? NOTIFICATION_COPY.categoryBlurb.session_compaction;

    const handlePrimary = useCallback(() => {
      onPrimary(notif);
    }, [onPrimary, notif]);

    const handleDismiss = useCallback(() => {
      onDismiss(notif.id);
    }, [onDismiss, notif.id]);

    const handleOpen = useCallback(() => {
      onOpen(notif);
    }, [onOpen, notif]);

    const turnMeta = formatTurnMeta(metadataTurnCount(notif));
    const statusLabel =
      notif.status === "in_progress"
        ? NOTIFICATION_COPY.statusLabels.in_progress
        : failed
          ? NOTIFICATION_COPY.statusLabels.failed
          : notif.status === "completed"
            ? NOTIFICATION_COPY.statusLabels.completed
            : null;

    return (
      <div
        className={cn(
          "group relative flex gap-2.5 p-3 rounded-xl border transition-all duration-200",
          unread
            ? "border-[rgba(var(--accent),0.25)] bg-[rgba(var(--card),0.75)] shadow-[0_4px_20px_rgba(var(--accent),0.06)]"
            : "border-[rgba(var(--border),0.1)] bg-[rgba(var(--card),0.4)]",
          receipt && "opacity-75"
        )}
      >
        <div
          className={cn(
            "w-8 h-8 rounded-xl border flex items-center justify-center shrink-0",
            visual.tile
          )}
        >
          <Icon size={15} strokeWidth={1.75} />
        </div>

        <div className="flex-1 min-w-0 flex flex-col gap-1">
          <div className="flex items-start justify-between gap-2">
            <div className="flex items-center gap-1.5 min-w-0">
              {unread && (
                <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shrink-0 animate-pulse" />
              )}
              <span className="text-[13px] font-semibold text-[rgb(var(--foreground))] truncate">
                {notif.title}
              </span>
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
            {statusLabel && (
              <span
                className={cn(
                  "inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider border",
                  failed
                    ? "border-[rgba(var(--error),0.35)] text-[rgb(var(--error))]"
                    : notif.status === "completed"
                      ? "border-[rgba(var(--success),0.35)] text-[rgb(var(--success))]"
                      : "border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))]"
                )}
              >
                {failed && <AlertTriangle size={10} />}
                {statusLabel}
              </span>
            )}
            {!receipt && category === "session_compaction" && (
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
                <span>{isWorking ? NOTIFICATION_COPY.tidying : NOTIFICATION_COPY.tidyNow}</span>
              </button>
            )}
            {!receipt && failed && (
              <button
                type="button"
                disabled={isWorking}
                onClick={handlePrimary}
                className={cn(
                  "flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all cursor-pointer",
                  "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:opacity-90 active:scale-95 shadow-sm",
                  isWorking && "opacity-60 cursor-wait"
                )}
              >
                {isWorking && <Loader2 size={11} className="animate-spin" />}
                <span>{isWorking ? NOTIFICATION_COPY.retrying : NOTIFICATION_COPY.retry}</span>
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

export const NotificationsPopover = memo(({ panelRef, onClose }: NotificationsPopoverProps) => {
  const navigate = useNavigate();
  const notifications = useNotificationStore((s) => s.notifications);
  const badgeCount = useNotificationStore(selectBadgeCount);
  const compactingSessionIds = useNotificationStore((s) => s.compactingSessionIds);
  const markAllRead = useNotificationStore((s) => s.markAllRead);
  const dismiss = useNotificationStore((s) => s.dismiss);
  const triggerCompaction = useNotificationStore((s) => s.triggerCompaction);

  const handlePrimary = useCallback(
    (notif: NotificationRecord) => {
      if (notif.session_id !== null && notif.session_id !== undefined) {
        triggerCompaction(notif.session_id).catch(() => {});
      }
    },
    [triggerCompaction]
  );

  const handleOpen = useCallback(
    (notif: NotificationRecord) => {
      onClose();
      const category = toCategory(notif.category);
      if (notif.session_id !== null && notif.session_id !== undefined) {
        navigate(`/history?sessionId=${notif.session_id}`);
      } else if (category === "memory_issue") {
        navigate("/memory");
      } else {
        navigate("/settings");
      }
    },
    [navigate, onClose]
  );

  return (
    <motion.div
      ref={panelRef}
      initial={{ opacity: 0, scale: 0.96, y: -6 }}
      animate={{ opacity: 1, scale: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.96, y: -6 }}
      transition={{ duration: 0.15, ease: "easeOut" }}
      className="absolute top-12 right-0 w-[360px] max-w-[calc(100vw-32px)] max-h-[460px] flex flex-col glass-card rounded-2xl shadow-[0_16px_40px_rgba(0,0,0,0.4)] z-50 overflow-hidden select-none pointer-events-auto"
    >
      <div className="flex items-center justify-between px-4 py-3 border-b border-[rgba(var(--border),0.1)] shrink-0">
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

        <div className="flex items-center gap-2">
          {badgeCount > 0 && (
            <button
              type="button"
              onClick={markAllRead}
              className="flex items-center gap-1 text-[11px] font-semibold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
            >
              <Check size={11} />
              <span>{NOTIFICATION_COPY.markAllRead}</span>
            </button>
          )}
          <Tooltip label={NOTIFICATION_COPY.dismiss} side="left">
            <button
              type="button"
              onClick={onClose}
              className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-all cursor-pointer"
              aria-label={NOTIFICATION_COPY.dismiss}
            >
              <X size={14} />
            </button>
          </Tooltip>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-3 flex flex-col gap-2 min-h-0 custom-scrollbar">
        {notifications.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-10 px-4 text-center">
            <div className="w-10 h-10 rounded-full border border-[rgba(var(--accent),0.2)] bg-[rgba(var(--accent),0.05)] flex items-center justify-center mb-2.5">
              <Bell size={18} className="text-[rgb(var(--accent))]/60" />
            </div>
            <span className="text-[13px] font-semibold text-[rgb(var(--foreground))]">
              {NOTIFICATION_COPY.emptyTitle}
            </span>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 max-w-[220px] mt-1 leading-relaxed">
              {NOTIFICATION_COPY.emptySubtitle}
            </p>
          </div>
        ) : (
          notifications.map((notif) => (
            <NotificationItem
              key={notif.id}
              notif={notif}
              isWorking={
                notif.session_id
                  ? compactingSessionIds.includes(notif.session_id)
                  : false
              }
              onPrimary={handlePrimary}
              onDismiss={dismiss}
              onOpen={handleOpen}
            />
          ))
        )}
      </div>
    </motion.div>
  );
});

NotificationsPopover.displayName = "NotificationsPopover";
