import React, { memo, useCallback, useMemo, useRef } from "react";
import { motion } from "framer-motion";
import { Plus, X, MessageSquare, Loader2, AlertCircle } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { useVoiceSession } from "@/shared/context/VoiceSessionContext";
import { useConversationList } from "@/shared/hooks/useConversationList";
import {
  resolveSessionTitle,
  sessionLastActivity,
  formatSessionRecency,
  type SessionRow,
} from "@/services/historyService";
import { SESSION_COPY } from "@/data/sessionCopy";

interface SessionRailProps {
  open: boolean;
  onClose: () => void;
}

interface SessionRowItemProps {
  session: SessionRow;
  active: boolean;
  restoring: boolean;
  onSelect: (id: number) => void;
}

const SessionRowItem: React.FC<SessionRowItemProps> = memo(
  ({ session, active, restoring, onSelect }) => {
    const handleSelect = useCallback(() => {
      onSelect(session.id);
    }, [onSelect, session.id]);

    const title = resolveSessionTitle(session);
    const recency = useMemo(
      () => formatSessionRecency(sessionLastActivity(session)),
      [session]
    );
    const turnLabel = useMemo(() => {
      const unit =
        session.turn_count === 1 ? SESSION_COPY.turnSingular : SESSION_COPY.turnPlural;
      return `${session.turn_count} ${unit}`;
    }, [session.turn_count]);

    return (
      <button
        onClick={handleSelect}
        disabled={restoring}
        aria-current={active ? "true" : undefined}
        className={cn(
          "w-full text-left px-3 py-2.5 rounded-xl border transition-all duration-300 cursor-pointer",
          "hover:bg-[rgb(var(--accent))]/10",
          active
            ? "bg-[rgb(var(--accent))]/15 border-[rgb(var(--accent))]/40"
            : "bg-transparent border-[rgba(var(--border),0.12)]",
          restoring && "opacity-60 cursor-wait"
        )}
      >
        <span className="flex items-center gap-2 min-w-0">
          {restoring ? (
            <Loader2
              size={14}
              className="shrink-0 animate-spin text-[rgb(var(--accent))]"
              aria-label={SESSION_COPY.restoringAriaLabel}
            />
          ) : (
            <MessageSquare
              size={14}
              className="shrink-0 text-[rgb(var(--foreground-muted))]"
            />
          )}
          <span className="flex-1 min-w-0 truncate text-[13px] font-medium text-[rgb(var(--foreground))]">
            {title}
          </span>
        </span>
        <span className="mt-1 block pl-6 text-[11px] tracking-wide text-[rgb(var(--foreground-muted))]">
          {recency} · {turnLabel}
        </span>
      </button>
    );
  }
);
SessionRowItem.displayName = "SessionRowItem";

export const SessionRail: React.FC<SessionRailProps> = memo(({ open, onClose }) => {
  const panelRef = useRef<HTMLDivElement>(null);
  useOverlay({ onClose, ref: panelRef, dismissOnOutside: true, active: open });

  const {
    activeSessionId,
    sessionListVersion,
    isRestoring,
    restoringSessionId,
    selectSession,
    startNewConversation,
  } = useVoiceSession();
  const { sessions, loading, error, refresh } = useConversationList(sessionListVersion);

  const handleSelect = useCallback(
    (id: number) => {
      selectSession(id).then(() => onClose()).catch(() => {});
    },
    [selectSession, onClose]
  );

  const handleNew = useCallback(() => {
    startNewConversation().then(() => onClose()).catch(() => {});
  }, [startNewConversation, onClose]);

  const handleRetry = useCallback(() => {
    refresh().catch(() => {});
  }, [refresh]);

  if (!open) return null;

  return (
    <motion.aside
      ref={panelRef}
      role="dialog"
      aria-label={SESSION_COPY.railAriaLabel}
      initial={{ opacity: 0, x: -24 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: -24 }}
      transition={{ duration: 0.25, ease: [0.16, 1, 0.3, 1] }}
      className="absolute left-4 top-[64px] bottom-[calc(72px+clamp(12px,2.5vh,28px))] z-40 w-[min(320px,85vw)] flex flex-col glass-card rounded-2xl border border-[rgba(var(--border),0.15)] bg-black/40 backdrop-blur-md shadow-2xl overflow-hidden pointer-events-auto"
    >
      <div className="flex items-center justify-between px-4 pt-3 pb-2">
        <span className="text-[11px] font-bold tracking-[0.2em] uppercase text-[rgb(var(--foreground-muted))]">
          {SESSION_COPY.railTitle}
        </span>
        <button
          onClick={onClose}
          aria-label={SESSION_COPY.closeRailAriaLabel}
          className="flex items-center justify-center w-7 h-7 rounded-full text-[rgb(var(--foreground-muted))] hover:bg-[rgb(var(--accent))]/10 hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
        >
          <X size={15} />
        </button>
      </div>

      <div className="px-3 pb-2">
        <button
          onClick={handleNew}
          aria-label={SESSION_COPY.newConversationAriaLabel}
          className="w-full flex items-center justify-center gap-2 px-3 h-10 rounded-xl border border-[rgb(var(--accent))]/40 bg-[rgb(var(--accent))]/10 hover:bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] text-[12px] font-bold tracking-wider uppercase transition-all cursor-pointer"
        >
          <Plus size={15} />
          {SESSION_COPY.newConversation}
        </button>
      </div>

      <div className="flex-1 overflow-y-auto scrollbar-none px-3 pb-3 flex flex-col gap-1.5">
        {loading ? (
          <p className="px-3 py-6 text-center text-[12px] text-[rgb(var(--foreground-muted))]">
            {SESSION_COPY.loadingSessions}
          </p>
        ) : error ? (
          <div className="flex flex-col items-center gap-2 px-3 py-6 text-center">
            <AlertCircle size={18} className="text-red-400" />
            <p className="text-[12px] text-[rgb(var(--foreground))]/80">{error}</p>
            <button
              onClick={handleRetry}
              className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] hover:underline cursor-pointer"
            >
              {SESSION_COPY.retry}
            </button>
          </div>
        ) : sessions.length === 0 ? (
          <div className="flex flex-col items-center gap-1.5 px-3 py-6 text-center">
            <p className="text-[13px] font-medium text-[rgb(var(--foreground))]">
              {SESSION_COPY.noSessionsTitle}
            </p>
            <p className="text-[12px] text-[rgb(var(--foreground-muted))]">
              {SESSION_COPY.noSessionsDesc}
            </p>
          </div>
        ) : (
          sessions.map((session) => (
            <SessionRowItem
              key={session.id}
              session={session}
              active={session.id === activeSessionId}
              restoring={isRestoring && restoringSessionId === session.id}
              onSelect={handleSelect}
            />
          ))
        )}
      </div>
    </motion.aside>
  );
});
SessionRail.displayName = "SessionRail";
