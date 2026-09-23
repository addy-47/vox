import { memo, useState, useEffect, useCallback } from "react";
import { Ghost, AlertCircle, RotateCcw, Sparkles, Loader2 } from "lucide-react";
import { formatDateTime, resolveSessionTitle, type SessionRow, type TurnRow } from "@/services/historyService";
import { EmptyState, OrbitalLoader } from "@/shared/components/common";
import { HISTORY_COPY } from "@/data/historyCopy";
import { Drawer } from "@/shared/ui/Drawer";
import { Tooltip } from "@/shared/ui/Tooltip";
import { Markdown } from "@/shared/ui/Markdown";
import { useNotificationStore } from "@/store/notificationStore";
import { metadataResolution } from "@/services/notificationService";

function formatTime(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export interface DetailPanelProps {
  open: boolean;
  session: SessionRow | null;
  turns: TurnRow[];
  loading: boolean;
  error?: string | null;
  onClose: () => void;
  onRetry?: () => void;
}

const INITIAL_VISIBLE_TURNS = 20;

const TurnBubble = memo(({ turn }: { turn: TurnRow }) => {
  return (
    <div className="space-y-4 [contain:content]">
      {/* User bubble */}
      <div className="flex flex-col items-end w-full">
        <span className="text-[11px] font-sans font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-1 mr-2">
          {HISTORY_COPY.userLabel}
        </span>
        <div className="glass-card rounded-2xl rounded-tr-none px-4 py-2.5 max-w-[75%] text-[14px] text-[rgb(var(--foreground))] leading-relaxed break-words select-text">
          <Markdown content={turn.user_text} variant="bubble" />
        </div>
      </div>

      {/* Assistant bubble */}
      <div className="flex flex-col items-start w-full">
        <span className="text-[11px] font-sans font-bold text-[rgb(var(--accent))] uppercase tracking-widest mb-1 ml-2">
          {HISTORY_COPY.voxLabel}
        </span>
        <div className="glass-card rounded-2xl rounded-tl-none px-4 py-2.5 max-w-[75%] text-[14px] text-[rgb(var(--foreground))] leading-relaxed break-words select-text border border-[rgba(var(--accent),0.15)]">
          <Markdown content={turn.assistant_text} variant="bubble" />
          <div className="flex gap-3 mt-2 border-t border-[rgba(var(--accent),0.1)] pt-1.5 shrink-0 text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            <span className="ml-auto text-[rgb(var(--foreground-muted))] font-medium">
              {formatTime(turn.created_at)}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
});
TurnBubble.displayName = "TurnBubble";

export const DetailPanel = memo(
  ({ open, session, turns, loading, error, onClose, onRetry }: DetailPanelProps) => {
    const [visibleCount, setVisibleCount] = useState(INITIAL_VISIBLE_TURNS);

    useEffect(() => {
      setVisibleCount(INITIAL_VISIBLE_TURNS);
    }, [session?.id]);

    const visibleTurns = turns.slice(0, visibleCount);
    const hasMoreTurns = turns.length > visibleCount;

    const isUncompacted = useNotificationStore(
      useCallback(
        (s) =>
          session?.id
            ? s.notifications.some(
                (n) =>
                  n.category === "session_compaction" &&
                  n.session_id === session.id &&
                  n.status !== "dismissed" &&
                  metadataResolution(n) !== "resolved"
              )
            : false,
        [session?.id]
      )
    );

    const isCompacting = useNotificationStore(
      useCallback(
        (s) => {
          if (!session?.id) return false;
          const notif = s.notifications.find(
            (n) =>
              n.category === "session_compaction" &&
              n.session_id === session.id &&
              n.status !== "dismissed" &&
              metadataResolution(n) !== "resolved"
          );
          return notif ? s.activeActionIds.includes(notif.id) : false;
        },
        [session?.id]
      )
    );

    const executeCompaction = useNotificationStore(
      (s) => s.executeCompactionForSession
    );

    return (
      <Drawer
        open={open}
        onClose={onClose}
        position="global"
        ariaLabel={HISTORY_COPY.sessionTranscript}
        resizeHint={HISTORY_COPY.resizeHint}
        bodyClassName="px-6 py-4"
        title={
          session ? (
            <div className="flex items-center gap-1.5 min-w-0 pr-2 [text-shadow:none]">
              <span className="text-[14px] font-display font-bold tracking-tight text-[rgb(var(--accent))] shrink-0">
                {session.project_id || "default"}
              </span>
              <span className="text-[14px] font-display font-bold text-[rgb(var(--foreground-muted))]">
                :
              </span>
              <span
                className="text-[14px] font-display font-bold tracking-tight text-[rgb(var(--foreground))] truncate max-w-[240px] sm:max-w-[380px]"
                title={resolveSessionTitle(session)}
              >
                {resolveSessionTitle(session)}
              </span>
              {isUncompacted && (
                <span
                  className="w-2 h-2 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.7)] animate-pulse shrink-0 ml-1"
                  title={HISTORY_COPY.uncompactedTurnsTooltip}
                />
              )}
            </div>
          ) : undefined
        }
        headerActions={
          isUncompacted && session ? (
            <Tooltip label={isCompacting ? HISTORY_COPY.compactingSession : HISTORY_COPY.compactSession}>
              <button
                type="button"
                disabled={isCompacting}
                onClick={() => executeCompaction(session.id)}
                className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg glass-card border border-[rgba(var(--accent),0.3)] text-[11px] font-bold text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors cursor-pointer disabled:opacity-50"
                aria-label={isCompacting ? HISTORY_COPY.compactingSession : HISTORY_COPY.compactSession}
              >
                {isCompacting ? (
                  <Loader2 size={12} className="animate-spin text-[rgb(var(--accent))]" />
                ) : (
                  <Sparkles size={12} className="text-[rgb(var(--accent))]" />
                )}
                <span>{isCompacting ? HISTORY_COPY.compactingSession : HISTORY_COPY.compactSession}</span>
              </button>
            </Tooltip>
          ) : undefined
        }
        subtitle={
          session ? (
            <div className="flex items-center gap-1.5 text-[11px] font-mono font-medium text-[rgb(var(--foreground-muted))] mt-0.5 [text-shadow:none]">
              <span>#{session.id}</span>
              <span>·</span>
              <span>{formatDateTime(session.created_at)}</span>
              <span>·</span>
              <span>
                {session.turn_count}{" "}
                {session.turn_count === 1 ? HISTORY_COPY.turnSingular : HISTORY_COPY.turnPlural}
              </span>
            </div>
          ) : undefined
        }
      >
        {loading ? (
          <div className="flex justify-center py-12">
            <OrbitalLoader
              size="sm"
              title={HISTORY_COPY.loadingTranscript}
              subtitle={HISTORY_COPY.fetchingTurns}
            />
          </div>
        ) : error ? (
          <div className="flex flex-col items-center justify-center py-12 px-4 text-center gap-3">
            <AlertCircle className="text-red-400 shrink-0" size={24} />
            <p className="text-[12px] text-red-400 font-medium max-w-xs leading-relaxed">{error}</p>
            {onRetry && (
              <button
                onClick={onRetry}
                className="px-3 py-1.5 rounded-xl glass-card border border-[rgba(var(--accent),0.3)] text-[11px] font-bold text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors flex items-center gap-1.5 cursor-pointer mt-1"
              >
                <RotateCcw size={12} />
                {HISTORY_COPY.retry}
              </button>
            )}
          </div>
        ) : turns.length === 0 ? (
          <EmptyState
            icon={Ghost}
            title={HISTORY_COPY.noConversationData}
            className="py-12 border-0 bg-transparent"
          />
        ) : (
          <div className="space-y-6 pb-4">
            {visibleTurns.map((turn) => (
              <TurnBubble key={turn.id} turn={turn} />
            ))}

            {hasMoreTurns && (
              <div className="flex justify-center pt-2">
                <button
                  onClick={() => setVisibleCount((prev) => prev + 20)}
                  className="px-4 py-2 rounded-xl glass-card text-[11px] font-bold text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer"
                >
                  {HISTORY_COPY.loadOlderTurns} ({turns.length - visibleCount} {HISTORY_COPY.remaining})
                </button>
              </div>
            )}
          </div>
        )}
      </Drawer>
    );
  }
);


DetailPanel.displayName = "DetailPanel";