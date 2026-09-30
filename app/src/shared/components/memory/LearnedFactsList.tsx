import React, { memo, useRef, useCallback, useEffect } from "react";
import { Sparkles, Calendar, Check, Loader2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import type { ObservationRecord } from "@/services/memoryService";
import type { ObservationFilter } from "@/shared/hooks/useObservationsList";

export interface LearnedFactsListProps {
  observations: ObservationRecord[];
  statusFilter: ObservationFilter;
  onStatusFilterChange: (filter: ObservationFilter) => void;
  isLoading?: boolean;
  isLoadingMore?: boolean;
  hasMore?: boolean;
  onLoadMore?: () => void;
  isConsolidating?: boolean;
}

export const LearnedFactsList: React.FC<LearnedFactsListProps> = memo(
  ({
    observations,
    statusFilter,
    onStatusFilterChange,
    isLoading = false,
    isLoadingMore = false,
    hasMore = false,
    onLoadMore,
    isConsolidating = false,
  }) => {
    const listRef = useRef<HTMLDivElement>(null);

    // Infinite scroll listener with 150px threshold
    const handleScroll = useCallback(() => {
      const el = listRef.current;
      if (!el || isLoadingMore || !hasMore || !onLoadMore) return;
      if (el.scrollHeight - el.scrollTop - el.clientHeight < 150) {
        onLoadMore();
      }
    }, [isLoadingMore, hasMore, onLoadMore]);

    // Attach scroll listener
    useEffect(() => {
      const el = listRef.current;
      if (!el) return;
      el.addEventListener("scroll", handleScroll, { passive: true });
      return () => el.removeEventListener("scroll", handleScroll);
    }, [handleScroll]);

    return (
      <div className="flex-1 min-h-0 flex flex-col justify-between">
        {/* Sleek Minimal Segmented Filter Tabs (No pills) */}
        <div className="flex items-center justify-between gap-3 pb-3 border-b border-[rgba(var(--border),0.1)]">
          <div className="flex items-center gap-1 bg-[rgba(var(--foreground),0.03)] p-1 rounded-xl border border-[rgba(var(--border),0.12)]">
            <button
              type="button"
              onClick={() => onStatusFilterChange("active")}
              className={cn(
                "px-3 py-1 rounded-lg text-[11px] font-mono transition-all cursor-pointer",
                statusFilter === "active"
                  ? "bg-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground))] font-semibold shadow-xs"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              )}
            >
              Pending
            </button>
            <button
              type="button"
              onClick={() => onStatusFilterChange("integrated")}
              className={cn(
                "px-3 py-1 rounded-lg text-[11px] font-mono transition-all cursor-pointer",
                statusFilter === "integrated"
                  ? "bg-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground))] font-semibold shadow-xs"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              )}
            >
              Integrated
            </button>
            <button
              type="button"
              onClick={() => onStatusFilterChange("all")}
              className={cn(
                "px-3 py-1 rounded-lg text-[11px] font-mono transition-all cursor-pointer",
                statusFilter === "all"
                  ? "bg-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground))] font-semibold shadow-xs"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              )}
            >
              All
            </button>
          </div>

          <div className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
            {observations.length} {observations.length === 1 ? "item" : "items"}
          </div>
        </div>

        {/* Observation Cards List */}
        {isLoading && observations.length === 0 ? (
          <div className="flex-1 flex flex-col items-center justify-center p-8 text-center">
            <Loader2 size={24} className="animate-spin text-[rgb(var(--accent))] mb-3" />
            <span className="text-[12px] font-mono text-[rgb(var(--foreground-muted))]">
              Loading observations…
            </span>
          </div>
        ) : observations.length === 0 ? (
          <div className="flex-1 flex flex-col items-center justify-center p-8 text-center">
            <div className="w-12 h-12 rounded-2xl bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.18)] flex items-center justify-center text-[rgb(var(--accent))] mb-3">
              <Sparkles size={20} />
            </div>
            <h4 className="text-[13px] font-semibold text-[rgb(var(--foreground))] mb-1">
              {statusFilter === "active"
                ? "No pending observations"
                : statusFilter === "integrated"
                ? "No integrated observations yet"
                : "No observations found"}
            </h4>
            <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] max-w-xs">
              {statusFilter === "active"
                ? "All candidate observations have been consolidated into your profile."
                : "Converse with Vox to generate personal observations."}
            </p>
          </div>
        ) : (
          <div
            ref={listRef}
            className="flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-1 pt-3 space-y-2.5"
          >
            {observations.map((fact) => {
              const isPending = fact.status === "active";
              const isIntegrated = fact.status === "integrated";

              return (
                <div
                  key={fact.id}
                  className={cn(
                    "p-3.5 rounded-xl bg-[rgba(var(--card),0.55)] border transition-all flex flex-col gap-2 shadow-xs",
                    isPending
                      ? "border-[rgba(var(--border),0.16)] border-l-2 border-l-amber-500/80 hover:border-[rgba(var(--accent),0.3)] bg-amber-500/[0.015]"
                      : isIntegrated
                      ? "border-[rgba(var(--border),0.12)] border-l-2 border-l-emerald-500/50 hover:border-[rgba(var(--accent),0.3)]"
                      : "border-[rgba(var(--border),0.12)] hover:border-[rgba(var(--accent),0.3)]",
                    isConsolidating && "opacity-60 pointer-events-none"
                  )}
                >
                  {/* Observation Meta Header (Zero pills) */}
                  <div className="flex items-center justify-between gap-2 flex-wrap">
                    <div className="flex items-center gap-2.5">
                      {isPending ? (
                        <span className="flex items-center gap-1.5 text-[10.5px] font-mono tracking-wider font-semibold uppercase text-amber-400">
                          <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" />
                          Pending
                        </span>
                      ) : isIntegrated ? (
                        <span className="flex items-center gap-1.5 text-[10.5px] font-mono tracking-wider uppercase text-emerald-400/80">
                          <Check size={11} strokeWidth={2.5} />
                          Integrated
                        </span>
                      ) : (
                        <span className="text-[10.5px] font-mono tracking-wider uppercase text-zinc-400/80">
                          {fact.status}
                        </span>
                      )}

                      <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60 tracking-wider uppercase font-semibold">
                        {fact.observation_type || fact.fact_type}
                      </span>
                    </div>

                    <div className="flex items-center gap-2.5 text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/50">
                      {fact.session_id && <span>Session #{fact.session_id}</span>}
                      <span className="flex items-center gap-1">
                        <Calendar size={10} />
                        {new Date(fact.created_at).toLocaleDateString(undefined, {
                          month: "short",
                          day: "numeric",
                        })}
                      </span>
                    </div>
                  </div>

                  {/* Fact Content Text */}
                  <p className="text-[12.5px] text-[rgb(var(--foreground))] leading-relaxed font-sans select-text pl-0.5">
                    {fact.text}
                  </p>
                </div>
              );
            })}

            {/* Infinite Scroll Footer */}
            {isLoadingMore && (
              <div className="flex items-center justify-center py-3 text-center">
                <Loader2 size={16} className="animate-spin text-[rgb(var(--accent))] mr-2" />
                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                  Loading more observations…
                </span>
              </div>
            )}

            {!hasMore && observations.length >= 25 && (
              <div className="text-center py-2 text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/60">
                All observations loaded
              </div>
            )}
          </div>
        )}
      </div>
    );
  }
);

LearnedFactsList.displayName = "LearnedFactsList";
export const ObservationsList = LearnedFactsList;
