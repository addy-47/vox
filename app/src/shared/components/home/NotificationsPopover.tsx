import { memo, useCallback } from "react";
import { motion } from "framer-motion";
import { Bell, Check, Loader2, Sparkles, X, ExternalLink } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { cn } from "@/shared/lib/utils";
import { NOTIFICATION_COPY } from "@/data/notificationCopy";
import { useNotificationStore } from "@/store/notificationStore";
import type { NotificationRecord } from "@/services/notificationService";

interface NotificationsPopoverProps {
  panelRef: React.RefObject<HTMLDivElement | null>;
  onClose: () => void;
}

const NotificationItem = memo(({
  notif,
  isCompacting,
  onCompact,
  onDismiss,
  onNavigateSession,
}: {
  notif: NotificationRecord;
  isCompacting: boolean;
  onCompact: (sessionId: number) => void;
  onDismiss: (id: string) => void;
  onNavigateSession: (sessionId: number) => void;
}) => {
  const isUnread = !notif.is_read || notif.status === "unread";
  const hasSession = notif.session_id !== null && notif.session_id !== undefined;

  return (
    <div
      className={cn(
        "group relative flex flex-col gap-2 p-3 rounded-xl border transition-all duration-200",
        isUnread
          ? "border-[rgba(var(--accent),0.25)] bg-[rgba(var(--card),0.75)] shadow-[0_4px_20px_rgba(var(--accent),0.06)]"
          : "border-[rgba(var(--border),0.1)] bg-[rgba(var(--card),0.4)] opacity-85 hover:opacity-100"
      )}
    >
      {/* Top row: Title + Session link & Dismiss */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          {isUnread && (
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shrink-0 animate-pulse" />
          )}
          {hasSession ? (
            <button
              type="button"
              onClick={() => onNavigateSession(notif.session_id!)}
              className="group/link flex items-center gap-1.5 text-[13px] font-semibold text-[rgb(var(--foreground))] hover:text-[rgb(var(--accent))] transition-colors text-left truncate cursor-pointer"
              title={NOTIFICATION_COPY.openDrawerTooltip}
            >
              <span className="truncate">{notif.title}</span>
              <ExternalLink
                size={11}
                className="opacity-40 group-hover/link:opacity-100 transition-opacity shrink-0"
              />
            </button>
          ) : (
            <span className="text-[13px] font-semibold text-[rgb(var(--foreground))] truncate">
              {notif.title}
            </span>
          )}
        </div>

        <button
          type="button"
          onClick={() => onDismiss(notif.id)}
          className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-all cursor-pointer shrink-0"
          aria-label={NOTIFICATION_COPY.dismiss}
        >
          <X size={13} />
        </button>
      </div>

      {/* Message */}
      <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed break-words">
        {notif.message}
      </p>

      {/* Actions */}
      {hasSession && (
        <div className="flex items-center gap-2 pt-1 border-t border-[rgba(var(--border),0.08)]">
          <button
            type="button"
            disabled={isCompacting}
            onClick={() => onCompact(notif.session_id!)}
            className={cn(
              "flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all cursor-pointer",
              isCompacting
                ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] cursor-wait"
                : "bg-[rgb(var(--accent))] text-black hover:opacity-90 active:scale-95 shadow-sm"
            )}
          >
            {isCompacting ? (
              <>
                <Loader2 size={11} className="animate-spin" />
                <span>{NOTIFICATION_COPY.compacting}</span>
              </>
            ) : (
              <>
                <Sparkles size={11} />
                <span>{NOTIFICATION_COPY.compactNow}</span>
              </>
            )}
          </button>

          <button
            type="button"
            onClick={() => onNavigateSession(notif.session_id!)}
            className="px-2.5 py-1 rounded-lg text-[11px] font-semibold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer"
          >
            {NOTIFICATION_COPY.viewSession}
          </button>
        </div>
      )}
    </div>
  );
});
NotificationItem.displayName = "NotificationItem";

export const NotificationsPopover = memo(({
  panelRef,
  onClose,
}: NotificationsPopoverProps) => {
  const navigate = useNavigate();
  const notifications = useNotificationStore((s) => s.notifications);
  const compactingSessionIds = useNotificationStore((s) => s.compactingSessionIds);
  const markAllRead = useNotificationStore((s) => s.markAllRead);
  const dismiss = useNotificationStore((s) => s.dismiss);
  const triggerCompaction = useNotificationStore((s) => s.triggerCompaction);

  const unreadCount = notifications.filter((n) => !n.is_read || n.status === "unread").length;

  const handleNavigateSession = useCallback((sessionId: number) => {
    onClose();
    navigate(`/history?sessionId=${sessionId}`);
  }, [navigate, onClose]);

  return (
    <motion.div
      ref={panelRef}
      initial={{ opacity: 0, scale: 0.96, y: -6 }}
      animate={{ opacity: 1, scale: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.96, y: -6 }}
      transition={{ duration: 0.15, ease: "easeOut" }}
      className="absolute top-12 right-0 w-[360px] max-w-[calc(100vw-32px)] max-h-[460px] flex flex-col glass-card border border-[rgba(var(--border),0.18)] rounded-2xl shadow-[0_16px_40px_rgba(0,0,0,0.4)] backdrop-blur-2xl z-50 overflow-hidden select-none pointer-events-auto"
      style={{
        background: "rgba(var(--card), 0.88)",
      }}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-[rgba(var(--border),0.1)] shrink-0">
        <div className="flex items-center gap-2">
          <Bell size={14} className="text-[rgb(var(--accent))]" />
          <span className="text-[13px] font-display font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
            {NOTIFICATION_COPY.title}
          </span>
          {unreadCount > 0 && (
            <span className="px-1.5 py-0.5 rounded-full bg-[rgb(var(--accent))] text-black font-mono text-[10px] font-black leading-none">
              {unreadCount}
            </span>
          )}
        </div>

        <div className="flex items-center gap-2">
          {unreadCount > 0 && (
            <button
              type="button"
              onClick={markAllRead}
              className="flex items-center gap-1 text-[11px] font-semibold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
            >
              <Check size={11} />
              <span>{NOTIFICATION_COPY.markAllRead}</span>
            </button>
          )}
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-lg text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-all cursor-pointer"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Notifications List */}
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
              isCompacting={notif.session_id ? compactingSessionIds.includes(notif.session_id) : false}
              onCompact={triggerCompaction}
              onDismiss={dismiss}
              onNavigateSession={handleNavigateSession}
            />
          ))
        )}
      </div>
    </motion.div>
  );
});

NotificationsPopover.displayName = "NotificationsPopover";
