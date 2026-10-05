import { memo } from "react";
import { Loader2, Shrink } from "lucide-react";
import { HISTORY_COPY } from "@/data/historyCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useSessionStore } from "@/store/sessionStore";
import { cn } from "@/shared/lib/utils";

export interface CompactSessionButtonProps {
  sessionId: number;
  uncompactedTurns: number;
  variant?: "button" | "icon";
  className?: string;
}

/**
 * "Compact session" trigger. `uncompacted_turns` is the sole visibility
 * authority — notification state is not a second gate. During active
 * compaction it shows a spinner and disables every other trigger.
 */
export const CompactSessionButton = memo(({
  sessionId,
  uncompactedTurns,
  variant = "button",
  className,
}: CompactSessionButtonProps) => {
  const compactingSessionId = useSessionStore((s) => s.compactingSessionId);
  const isCompacting = compactingSessionId === sessionId;
  const isAnyCompacting = compactingSessionId !== null;
  const disabled = isCompacting || (isAnyCompacting && !isCompacting);

  const executeCompaction = useSessionStore((s) => s.executeCompaction);

  if (uncompactedTurns <= 0) return null;

  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    if (disabled) return;
    executeCompaction(sessionId);
  };

  const label = isCompacting
    ? HISTORY_COPY.compactingSession
    : HISTORY_COPY.compactSession;

  if (variant === "icon") {
    return (
      <Tooltip label={label} side="bottom">
        <button
          type="button"
          disabled={disabled}
          onClick={handleClick}
          className={cn(
            "w-8 h-8 rounded-xl border flex items-center justify-center transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed",
            "border-[rgba(var(--accent),0.3)] bg-[rgba(var(--card),0.5)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] hover:border-[rgba(var(--accent),0.5)] shadow-xs",
            className
          )}
          aria-label={label}
        >
          {isCompacting ? (
            <Loader2 size={13} className="animate-spin text-[rgb(var(--accent))]" />
          ) : (
            <Shrink size={13} className="text-[rgb(var(--accent))]" />
          )}
        </button>
      </Tooltip>
    );
  }

  return (
    <Tooltip label={label}>
      <button
        type="button"
        disabled={disabled}
        onClick={handleClick}
        className={cn(
          "flex items-center gap-1.5 px-2.5 py-1 rounded-lg glass-card border border-[rgba(var(--accent),0.3)] text-[11px] font-bold text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed",
          className
        )}
        aria-label={label}
      >
        {isCompacting ? (
          <Loader2 size={12} className="animate-spin text-[rgb(var(--accent))]" />
        ) : (
          <Shrink size={12} className="text-[rgb(var(--accent))]" />
        )}
        <span>{label}</span>
      </button>
    </Tooltip>
  );
});

CompactSessionButton.displayName = "CompactSessionButton";
