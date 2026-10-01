import React, { memo, useState, useRef, useEffect } from "react";
import {
  Upload,
  Edit3,
  X,
  RotateCw,
  Sparkles,
  ArrowLeft,
  MessageSquare,
  Layers,
  CheckCircle2,
  Tag,
  Filter,
  ChevronDown,
  Check,
  Loader2,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
import type { StagingMode } from "../stagingTypes";
import type { ObservationFilter } from "@/shared/hooks/useObservationsList";

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
  onRegenerate: () => void;
  onClearComments?: () => void;
  onCommit: () => void;
  // Observation filter & count
  observationFilter?: ObservationFilter;
  onObservationFilterChange?: (filter: ObservationFilter) => void;
  observationCount?: number;
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
    onRegenerate,
    onClearComments,
    onCommit,
    observationFilter,
    onObservationFilterChange,
    observationCount,
  }) => {
    const [isFilterOpen, setIsFilterOpen] = useState(false);
    const filterMenuRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
      if (!isFilterOpen) return;
      const handleClickOutside = (e: MouseEvent) => {
        if (filterMenuRef.current && !filterMenuRef.current.contains(e.target as Node)) {
          setIsFilterOpen(false);
        }
      };
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }, [isFilterOpen]);
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
              <CheckCircle2 size={16} className="text-[rgb(var(--accent))]" />
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
              {mode === "facts" ? MEMORY_COPY.observationsTitleLabel : MEMORY_COPY.stagingMirror}
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
              {justCommitted
                ? MEMORY_COPY.allChangesIntegratedShort
                : isSuggestionsActive
                ? `${decisionStats.total} ${
                    decisionStats.total === 1
                      ? MEMORY_COPY.suggestedChangeCount
                      : MEMORY_COPY.suggestedChangesCount
                  } (${decisionStats.accepted} accepted, ${decisionStats.rejected} rejected, ${
                    decisionStats.pending
                  } pending)`
                : mode === "comment"
                ? MEMORY_COPY.commentsQueued(commentsCount)
                : mode === "edit"
                ? MEMORY_COPY.directEditsDesc
                : mode === "import"
                ? MEMORY_COPY.pasteMarkdownDesc
                : mode === "facts"
                ? observationCount !== undefined
                  ? MEMORY_COPY.observationsDesc(observationCount)
                  : MEMORY_COPY.observationsDescBare
                : MEMORY_COPY.noPendingSuggestionsTitle}
            </span>
          </div>
        </div>

        {/* Action controls in header */}
        <div className="flex items-center gap-2">
          {mode === "facts" ? (
            <div className="flex items-center gap-1.5">
              {/* Filter dropdown */}
              <div className="relative" ref={filterMenuRef}>
                <button
                  type="button"
                  onClick={() => setIsFilterOpen((prev) => !prev)}
                  className={cn(
                    "flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[11px] font-mono border transition-colors duration-150 cursor-pointer",
                    isFilterOpen || (observationFilter && observationFilter !== "all")
                      ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))]"
                      : "border-[rgba(var(--border),0.18)] bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)]"
                  )}
                  title="Filter observations"
                  aria-expanded={isFilterOpen}
                >
                  <Filter size={11} className={observationFilter && observationFilter !== "all" ? "text-[rgb(var(--accent))]" : undefined} />
                  <span className="capitalize">{observationFilter === "staged" ? "Staged" : observationFilter === "pending" ? "Pending" : observationFilter === "integrated" ? "Integrated" : "All"}</span>
                  <ChevronDown size={11} className={cn("transition-transform duration-150 opacity-70", isFilterOpen && "rotate-180")} />
                </button>

                {isFilterOpen && (
                  <div className="absolute right-0 top-full mt-1.5 w-36 py-1 rounded-xl bg-[rgb(var(--card))] border border-[rgba(var(--border),0.2)] shadow-xl z-50 backdrop-blur-md">
                    {(
                      [
                        { id: "staged", label: "Staged" },
                        { id: "pending", label: "Pending" },
                        { id: "integrated", label: "Integrated" },
                        { id: "all", label: "All" },
                      ] as const
                    ).map((item) => (
                      <button
                        key={item.id}
                        type="button"
                        onClick={() => {
                          onObservationFilterChange?.(item.id);
                          setIsFilterOpen(false);
                        }}
                        className={cn(
                          "w-full px-3 py-1.5 text-left text-[11px] font-mono flex items-center justify-between transition-colors cursor-pointer",
                          observationFilter === item.id
                            ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-medium"
                            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)]"
                        )}
                      >
                        <span>{item.label}</span>
                        {observationFilter === item.id && <Check size={11} className="text-[rgb(var(--accent))]" />}
                      </button>
                    ))}
                  </div>
                )}
              </div>

              <Tooltip label="Return to Staging Mirror">
                <button
                  type="button"
                  onClick={() => onModeChange("idle")}
                  className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[11px] font-mono border border-[rgba(var(--border),0.18)] bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.3)] transition-colors duration-150 cursor-pointer"
                >
                  <X size={12} />
                  <span>Close</span>
                </button>
              </Tooltip>
            </div>
          ) : isSuggestionsActive ? (
            /* Bulk selectors only. The primary apply action lives in the review
               view's sticky footer so there is exactly one, and it can state how
               many decisions it will send. */
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={onAcceptAll}
                disabled={isApplyingSuggestions}
                className="px-2.5 py-1 rounded-lg text-[11px] font-mono text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors duration-150 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]"
              >
                {MEMORY_COPY.acceptAll}
              </button>
              <button
                type="button"
                onClick={onRejectAll}
                disabled={isApplyingSuggestions}
                className="px-2.5 py-1 rounded-lg text-[11px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--danger),0.08)] hover:text-[rgb(var(--danger))] transition-colors duration-150 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--danger))]"
              >
                {MEMORY_COPY.rejectAll}
              </button>
            </div>
          ) : mode === "comment" ? (
            <div className="flex items-center gap-2">
              <Tooltip label="Regenerate profile with applied comments">
                <button
                  type="button"
                  onClick={onRegenerate}
                  disabled={isSaving || commentsCount === 0}
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors duration-150 disabled:opacity-40 cursor-pointer shadow-sm"
                >
                  <RotateCw size={13} className={cn(isSaving && "animate-spin")} />
                  <span>{isSaving ? MEMORY_COPY.regenerating : MEMORY_COPY.regenerate}</span>
                </button>
              </Tooltip>
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
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-xl text-[11px] font-mono bg-[rgba(var(--accent),0.2)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.3)] transition-colors duration-150 disabled:opacity-40 cursor-pointer shadow-sm"
            >
              {isSaving ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <ArrowLeft size={13} />
              )}
              <span>{isSaving ? MEMORY_COPY.saving : MEMORY_COPY.saveAndCommit}</span>
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
