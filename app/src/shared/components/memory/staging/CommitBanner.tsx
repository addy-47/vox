import React, { memo } from "react";
import { Check, History, X } from "lucide-react";
import { MEMORY_COPY } from "@/data/memoryCopy";

export interface CommitBannerProps {
  activeVersion?: number;
  onViewVersionHistory?: () => void;
  /** Explicit dismiss. Auto-dismiss is owned by the parent so the banner never
   *  keeps its own timer alive past the card's unmount. */
  onDismiss: () => void;
}

/**
 * Post-commit confirmation: a single slim banner over the staging card.
 *
 * Previously the commit replaced the entire staging body with a large centred
 * success panel, so the user lost their place and the review footer vanished
 * mid-animation. A banner confirms without destroying context.
 */
export const CommitBanner: React.FC<CommitBannerProps> = memo(
  ({ activeVersion, onViewVersionHistory, onDismiss }) => {
    return (
      <div
        role="status"
        className="flex items-center gap-2.5 px-3 py-2 rounded-xl border border-[rgba(var(--accent),0.35)] bg-[rgba(var(--accent),0.12)]"
      >
        <span className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))] flex items-center justify-center shrink-0">
          <Check size={11} strokeWidth={3} />
        </span>
        <span className="text-[11.5px] font-mono text-[rgb(var(--foreground))] truncate">
          {MEMORY_COPY.allChangesIntegrated.replace("{v}", String(activeVersion ?? 1))}
        </span>
        {onViewVersionHistory && (
          <button
            type="button"
            onClick={onViewVersionHistory}
            className="flex items-center gap-1.5 px-2 py-1 rounded-lg text-[11px] font-mono text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.14)] transition-colors duration-150 cursor-pointer shrink-0"
          >
            <History size={12} />
            {MEMORY_COPY.viewVersionHistory}
          </button>
        )}
        <button
          type="button"
          onClick={onDismiss}
          aria-label={MEMORY_COPY.dismiss}
          title={MEMORY_COPY.dismiss}
          className="ml-auto text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors duration-150 cursor-pointer shrink-0"
        >
          <X size={12} />
        </button>
      </div>
    );
  }
);

CommitBanner.displayName = "CommitBanner";