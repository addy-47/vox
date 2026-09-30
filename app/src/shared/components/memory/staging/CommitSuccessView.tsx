import React, { memo } from "react";
import { Check, History } from "lucide-react";
import { MEMORY_COPY } from "@/data/memoryCopy";

export interface CommitSuccessViewProps {
  activeVersion?: number;
  onViewVersionHistory?: () => void;
}

export const CommitSuccessView: React.FC<CommitSuccessViewProps> = memo(
  ({ activeVersion, onViewVersionHistory }) => {
    return (
      <div className="flex-1 min-h-0 flex flex-col items-center justify-center p-6 text-center animate-in fade-in duration-300">
        <div className="w-14 h-14 rounded-full bg-emerald-500/15 border border-emerald-500/30 flex items-center justify-center text-emerald-400 mb-4 shadow-lg">
          <Check size={26} strokeWidth={2.5} />
        </div>
        <h3 className="text-[15px] font-semibold tracking-wide text-[rgb(var(--foreground))] mb-1.5">
          {MEMORY_COPY.allChangesIntegrated}
        </h3>
        <p className="text-[12px] text-[rgb(var(--foreground-muted))] max-w-sm mb-6 font-mono leading-relaxed">
          {MEMORY_COPY.allChangesIntegratedDesc.replace("{v}", String(activeVersion ?? 1))}
        </p>
        {onViewVersionHistory && (
          <button
            type="button"
            onClick={onViewVersionHistory}
            className="flex items-center gap-2 px-4 py-2 rounded-xl text-[11.5px] font-mono bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.18)] text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.4)] hover:bg-[rgba(var(--accent),0.08)] transition-all cursor-pointer shadow-sm"
          >
            <History size={14} className="text-[rgb(var(--accent))]" />
            <span>{MEMORY_COPY.viewVersionHistory}</span>
          </button>
        )}
      </div>
    );
  }
);

CommitSuccessView.displayName = "CommitSuccessView";
