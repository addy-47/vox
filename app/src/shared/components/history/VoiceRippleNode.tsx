import React, { useMemo, memo } from "react";
import { Trash2, Check, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { type SessionRow } from "@/services/historyService";
import { HISTORY_COPY } from "@/data/historyCopy";
import { formatClockTime, formatDuration, ORBIT_CARD_WIDTH } from "./orbitMath";

export interface VoiceRippleNodeProps {
  session: SessionRow;
  isSelected: boolean;
  isConfirmingDelete: boolean;
  onSelect: (session: SessionRow) => void;
  onDelete: (e: React.MouseEvent, id: number) => void;
  onCancelDelete: (e: React.MouseEvent) => void;
}

/** Deterministic pseudo-random bar height in [0, 1] from the turn count. */
function barHeight(turnCount: number, index: number): number {
  const seed = Math.sin(turnCount * (index + 1) * 12.9898) * 43758.5453;
  return 0.3 + (seed - Math.floor(seed)) * 0.7;
}

export const VoiceRippleNode = memo(
  ({
    session,
    isSelected,
    isConfirmingDelete,
    onSelect,
    onDelete,
    onCancelDelete,
  }: VoiceRippleNodeProps) => {
    const previewText = useMemo(() => {
      const msg = session.first_message || HISTORY_COPY.noTranscript;
      const words = msg.trim().split(/\s+/);
      if (words.length <= 7) return msg;
      return words.slice(0, 7).join(" ") + "...";
    }, [session.first_message]);

    const durationLabel = useMemo(
      () =>
        session.ended_at !== null && session.ended_at >= session.started_at
          ? formatDuration(session.ended_at - session.started_at)
          : null,
      [session.ended_at, session.started_at]
    );

    const bars = useMemo(
      () => Array.from({ length: 4 }, (_, i) => barHeight(session.turn_count, i)),
      [session.turn_count]
    );

    const handleKeyDown = (e: React.KeyboardEvent) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onSelect(session);
      }
    };

    return (
      <div
        role="button"
        tabIndex={0}
        onKeyDown={handleKeyDown}
        style={{ width: ORBIT_CARD_WIDTH }}
        className={cn(
          "rounded-2xl p-4 flex flex-col text-left select-none group cursor-pointer transition-all duration-200 glass-card backdrop-blur-xl focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
          isSelected
            ? "border-[rgba(var(--accent),0.9)] bg-[rgb(var(--card))]/95 shadow-[0_0_35px_rgba(var(--accent),0.5)] scale-[1.03]"
            : "border-[rgba(var(--border),0.15)] bg-[rgb(var(--card))]/75 hover:border-[rgba(var(--accent),0.55)] hover:bg-[rgb(var(--card))]/90 hover:shadow-[0_0_20px_rgba(var(--accent),0.25)]"
        )}
        onClick={(e) => {
          e.stopPropagation();
          onSelect(session);
        }}
      >
        {/* Glowing orbit anchor dot & time header */}
        <div className="flex items-center justify-between mb-2 pr-8">
          <div className="flex items-center gap-2">
            <div className="relative flex items-center justify-center">
              <div
                className="w-2.5 h-2.5 rounded-full"
                style={{
                  backgroundColor: "rgb(var(--accent))",
                  boxShadow: "0 0 10px rgb(var(--accent))",
                }}
              />
              {isSelected && (
                <div className="absolute inset-[-3px] rounded-full border border-[rgb(var(--accent))] animate-ping" />
              )}
            </div>
            <span className="text-[12px] font-mono text-[rgb(var(--foreground))] font-bold">
              {formatClockTime(session.started_at)}
            </span>
          </div>

          {durationLabel && (
            <span className="text-[11px] font-mono font-medium text-[rgb(var(--foreground-muted))]">
              {durationLabel}
            </span>
          )}
        </div>

        {/* Snippet preview — the prominent voice quote matching mockup */}
        <p className="text-[13px] font-medium leading-snug text-[rgb(var(--foreground))] line-clamp-2 pr-1 font-sans">
          "{previewText}"
        </p>

        {/* Derived voice signature: turn-count bars + turn total */}
        <div className="flex items-center justify-between mt-3 pt-1 border-t border-[rgba(var(--accent),0.08)]">
          <div className="flex items-end gap-[3px] h-3">
            {bars.map((h, i) => (
              <span
                key={i}
                className="w-[3px] rounded-full"
                style={{
                  height: `${Math.round(h * 12)}px`,
                  backgroundColor: "rgb(var(--accent))",
                  opacity: 0.4 + h * 0.6,
                }}
              />
            ))}
          </div>
          <span className="text-[11px] font-mono font-medium text-[rgb(var(--foreground-muted))]">
            {session.turn_count}{" "}
            {session.turn_count === 1
              ? HISTORY_COPY.turnSingular
              : HISTORY_COPY.turnPlural}
          </span>
        </div>

        {/* Delete session action */}
        <div
          className={cn(
            "absolute top-2.5 right-2.5 transition-opacity duration-200 z-20",
            isConfirmingDelete ? "opacity-100" : "opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
          )}
        >
          {isConfirmingDelete ? (
            <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
              <Tooltip label={HISTORY_COPY.deleteConfirm}>
                <button
                  onClick={(e) => onDelete(e, session.id)}
                  className="w-8 h-8 rounded-full glass-card flex items-center justify-center text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/20 cursor-pointer shadow-md focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
                  aria-label={HISTORY_COPY.deleteConfirm}
                >
                  <Check size={14} strokeWidth={2.5} />
                </button>
              </Tooltip>
              <Tooltip label={HISTORY_COPY.cancelDelete}>
                <button
                  onClick={onCancelDelete}
                  className="w-8 h-8 rounded-full glass-card flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] cursor-pointer shadow-md focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
                  aria-label={HISTORY_COPY.cancelDelete}
                >
                  <X size={14} strokeWidth={2.5} />
                </button>
              </Tooltip>
            </div>
          ) : (
            <Tooltip label={HISTORY_COPY.deleteSession}>
              <button
                onClick={(e) => onDelete(e, session.id)}
                className="w-8 h-8 rounded-full glass-card flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]"
                aria-label={HISTORY_COPY.deleteSession}
              >
                <Trash2 size={14} />
              </button>
            </Tooltip>
          )}
        </div>
      </div>
    );
  }
);

VoiceRippleNode.displayName = "VoiceRippleNode";
