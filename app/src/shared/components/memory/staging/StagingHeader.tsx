import React, { memo } from "react";
import {
  Upload,
  Edit3,
  Check,
  X,
  Sparkles,
  ArrowLeft,
  MessageSquare,
  Layers,
  CheckCircle2,
  Tag,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import type { StagingMode } from "../stagingTypes";

export interface StagingHeaderProps {
  mode: StagingMode;
  onModeChange: (mode: StagingMode) => void;
  justCommitted?: boolean;
  isSuggestionsActive: boolean;
  decisionStats: {
    total: number;
    accepted: number;
    rejected: number;
    pending: number;
  };
  commentsCount: number;
  isApplyingSuggestions?: boolean;
  isSaving: boolean;
  draftEmpty: boolean;
  onAcceptAll: () => void;
  onRejectAll: () => void;
  onApplyAllDecisions: () => void;
  onRegenerate: () => void;
  onClearComments?: () => void;
  onCommit: () => void;
}

export const StagingHeader: React.FC<StagingHeaderProps> = memo(
  ({
    mode,
    onModeChange,
    justCommitted,
    isSuggestionsActive,
    decisionStats,
    commentsCount,
    isApplyingSuggestions,
    isSaving,
    draftEmpty,
    onAcceptAll,
    onRejectAll,
    onApplyAllDecisions,
    onRegenerate,
    onClearComments,
    onCommit,
  }) => {
    return (
      <div className="flex items-center justify-between gap-4 border-b border-[rgba(var(--border),0.12)] pb-3.5 min-h-[44px] shrink-0">
        <div className="flex items-center gap-3">
          <div
            className={cn(
              "w-8 h-8 rounded-xl border flex items-center justify-center transition-colors shadow-sm",
              isSuggestionsActive
                ? "bg-[rgba(var(--accent),0.18)] border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))]"
                : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.15)] text-[rgb(var(--foreground-muted))]"
            )}
          >
            {justCommitted ? (
              <CheckCircle2 size={16} className="text-emerald-400" />
            ) : isSuggestionsActive ? (
              <Sparkles size={16} className="text-[rgb(var(--accent))]" />
            ) : mode === "comment" ? (
              <MessageSquare size={16} className="text-[rgb(var(--accent))]" />
            ) : mode === "edit" ? (
              <Edit3 size={16} className="text-[rgb(var(--accent))]" />
            ) : mode === "import" ? (
              <Upload size={16} className="text-[rgb(var(--accent))]" />
            ) : mode === "facts" ? (
              <Tag size={16} className="text-[rgb(var(--accent))]" />
            ) : (
              <Layers size={16} />
            )}
          </div>

          <div className="flex flex-col">
            <span className="text-[13px] font-semibold tracking-wide text-[rgb(var(--foreground))]">
              {mode === "facts" ? "Observations" : MEMORY_COPY.stagingMirror}
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
              {justCommitted
                ? "All changes integrated"
                : isSuggestionsActive
                ? `${decisionStats.total} ${
                    decisionStats.total === 1
                      ? MEMORY_COPY.suggestedChangeCount
                      : MEMORY_COPY.suggestedChangesCount
                  } (${decisionStats.accepted} accepted, ${decisionStats.rejected} rejected, ${
                    decisionStats.pending
                  } pending)`
                : mode === "comment"
                ? `${commentsCount} line-anchored comments queued`
                : mode === "edit"
                ? "Direct in-place edits"
                : mode === "import"
                ? "Paste markdown to replace current profile"
                : mode === "facts"
                ? "Extracted knowledge and behavioral observations"
                : "No pending suggestions"}
            </span>
          </div>
        </div>

        {/* Action controls in header */}
        <div className="flex items-center gap-2">
          {mode === "facts" ? (
            <button
              type="button"
              onClick={() => onModeChange("idle")}
              className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[11px] font-mono border border-[rgba(var(--border),0.18)] bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] transition-all cursor-pointer"
              title="Return to Staging Mirror"
            >
              <X size={12} />
              <span>Close</span>
            </button>
          ) : isSuggestionsActive ? (
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={onAcceptAll}
                disabled={isApplyingSuggestions}
                className="px-2.5 py-1 rounded-lg text-[11px] font-mono text-emerald-400 hover:bg-emerald-500/10 transition-colors cursor-pointer"
              >
                {MEMORY_COPY.acceptAll}
              </button>
              <button
                type="button"
                onClick={onRejectAll}
                disabled={isApplyingSuggestions}
                className="px-2.5 py-1 rounded-lg text-[11px] font-mono text-rose-400 hover:bg-rose-500/10 transition-colors cursor-pointer"
              >
                {MEMORY_COPY.rejectAll}
              </button>
              <button
                type="button"
                onClick={onApplyAllDecisions}
                disabled={isApplyingSuggestions}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono font-medium bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.45)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-all cursor-pointer shadow-sm disabled:opacity-40"
              >
                <Check size={12} className={cn(isApplyingSuggestions && "animate-spin")} />
                <span>
                  {isApplyingSuggestions
                    ? MEMORY_COPY.applyingDecisions
                    : MEMORY_COPY.consolidate}
                </span>
              </button>
            </div>
          ) : mode === "comment" ? (
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={onRegenerate}
                disabled={isSaving || commentsCount === 0}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors disabled:opacity-40 cursor-pointer shadow-sm"
              >
                <Sparkles size={13} className={cn(isSaving && "animate-spin")} />
                <span>{isSaving ? MEMORY_COPY.regenerating : MEMORY_COPY.regenerate}</span>
              </button>
              <button
                type="button"
                onClick={() => {
                  onClearComments?.();
                  onModeChange("idle");
                }}
                disabled={isSaving}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                <X size={13} /> {MEMORY_COPY.cancel}
              </button>
            </div>
          ) : mode === "edit" || mode === "import" ? (
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={onCommit}
                disabled={isSaving || draftEmpty}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors disabled:opacity-40 cursor-pointer shadow-sm"
              >
                <ArrowLeft size={13} className={cn(isSaving && "animate-pulse")} />
                <span>{isSaving ? "Saving…" : "Save & Commit"}</span>
              </button>
              <button
                type="button"
                onClick={() => onModeChange("idle")}
                disabled={isSaving}
                className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--border),0.14)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                <X size={13} /> {MEMORY_COPY.cancel}
              </button>
            </div>
          ) : null}
        </div>
      </div>
    );
  }
);

StagingHeader.displayName = "StagingHeader";
