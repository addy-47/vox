import { memo, useState, useEffect } from "react";
import { Ghost, AlertCircle, RotateCcw } from "lucide-react";
import ReactMarkdown from "react-markdown";
import { formatDateTime, type SessionRow, type TurnRow } from "@/services/historyService";
import { EmptyState, OrbitalLoader } from "@/shared/components/common";
import { HISTORY_COPY } from "@/data/historyCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
import { Drawer } from "@/shared/ui/Drawer";

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
  const isSimpleUserText = !/[*_#`\[\]]/.test(turn.user_text);
  const isSimpleAssistantText = !/[*_#`\[\]]/.test(turn.assistant_text);

  return (
    <div className="space-y-4 [contain:content]">
      {/* User bubble */}
      <div className="flex flex-col items-end w-full">
        <span className="text-[11px] font-sans font-bold text-[rgb(var(--foreground-muted))] uppercase tracking-widest mb-1 mr-2">
          {HISTORY_COPY.userLabel}
        </span>
        <div className="glass-card rounded-2xl rounded-tr-none px-4 py-2.5 max-w-[75%] text-[14px] text-[rgb(var(--foreground))] leading-relaxed break-words prose prose-invert select-text">
          {isSimpleUserText ? (
            <p className="m-0 whitespace-pre-wrap">{turn.user_text}</p>
          ) : (
            <ReactMarkdown>{turn.user_text}</ReactMarkdown>
          )}
        </div>
      </div>

      {/* Assistant bubble */}
      <div className="flex flex-col items-start w-full">
        <span className="text-[11px] font-sans font-bold text-[rgb(var(--accent))] uppercase tracking-widest mb-1 ml-2">
          {HISTORY_COPY.voxLabel}
        </span>
        <div className="glass-card rounded-2xl rounded-tl-none px-4 py-2.5 max-w-[75%] text-[14px] text-[rgb(var(--foreground))] leading-relaxed break-words prose prose-invert select-text border border-[rgba(var(--accent),0.15)]">
          {isSimpleAssistantText ? (
            <p className="m-0 whitespace-pre-wrap">{turn.assistant_text}</p>
          ) : (
            <ReactMarkdown>{turn.assistant_text}</ReactMarkdown>
          )}
          <div className="flex gap-3 mt-2 border-t border-[rgba(var(--accent),0.1)] pt-1.5 shrink-0 text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            {turn.stt_latency_ms !== null && (
              <Tooltip label={HISTORY_COPY.hearingTime}>
                <span className="cursor-help hover:text-[rgb(var(--accent))] transition-colors">
                  {HISTORY_COPY.hearingPrefix} {turn.stt_latency_ms}ms
                </span>
              </Tooltip>
            )}
            {turn.ttft_ms !== null && (
              <Tooltip label={HISTORY_COPY.thinkingTime}>
                <span className="cursor-help hover:text-[rgb(var(--accent))] transition-colors">
                  {HISTORY_COPY.thinkingPrefix} {turn.ttft_ms}ms
                </span>
              </Tooltip>
            )}
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

    return (
      <Drawer
        open={open}
        onClose={onClose}
        position="global"
        ariaLabel="Session transcript"
        resizeHint={HISTORY_COPY.resizeHint}
        bodyClassName="px-6 py-4"
        title={
          session ? (
            <span className="text-[13px] font-display font-black tracking-[0.16em] uppercase text-[rgb(var(--accent))]">
              {HISTORY_COPY.sessionPrefix}
              {session.id}
            </span>
          ) : undefined
        }
        subtitle={
          session ? (
            <div className="text-[11px] font-mono font-medium text-[rgb(var(--foreground-muted))] mt-0.5">
              {formatDateTime(session.started_at)} · {session.turn_count}{" "}
              {session.turn_count === 1 ? HISTORY_COPY.turnSingular : HISTORY_COPY.turnPlural}
            </div>
          ) : undefined
        }
      >
        {loading ? (
          <div className="flex justify-center py-12">
            <OrbitalLoader
              size="sm"
              title="Loading conversation transcript..."
              subtitle="Fetching turns and voice telemetry"
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