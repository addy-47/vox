import React, { memo, useRef, useCallback, useEffect } from "react";
import { Calendar, Circle, Loader2 } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import type { ObservationRecord } from "@/services/memoryService";
import type { ObservationFilter } from "@/shared/hooks/useObservationsList";

export interface LearnedFactsListProps {
  observations: ObservationRecord[];
  statusFilter?: ObservationFilter;
  onStatusFilterChange?: (filter: ObservationFilter) => void;
  isLoading?: boolean;
  isLoadingMore?: boolean;
  hasMore?: boolean;
  onLoadMore?: () => void;
  isConsolidating?: boolean;
}

const ObservationCard = memo(function ObservationCard({
  fact,
  dimmed,
  showStatus,
}: {
  fact: ObservationRecord;
  dimmed: boolean;
  showStatus: boolean;
}) {
  const isStaged = fact.status === "active";
  const isPending = fact.status === "pending";
  const isIntegrated = fact.status === "integrated";

  return (
    <article
      className={cn(
        "group relative p-3.5 rounded-xl bg-[rgba(var(--card),0.5)] hover:bg-[rgba(var(--card),0.8)]",
        "border border-[rgba(var(--border),0.12)] hover:border-[rgba(var(--accent),0.35)]",
        "flex flex-col justify-between h-[160px] transition-all duration-150 shadow-xs",
        "select-text",
        dimmed && "opacity-60"
      )}
    >
      <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-0.5">
        {/* Fact / Observation Text */}
        <p className="text-[12px] text-[rgb(var(--foreground))] leading-relaxed font-sans">
          {fact.text}
        </p>
      </div>

      {/* Footer: Status on bottom-left (only when 'all' filter is active) & Date in accent color on bottom-right */}
      <div className="flex items-center justify-between shrink-0 pt-1.5 text-[10px] font-mono">
        {showStatus ? (
          <span className="text-[10px] font-mono tracking-wider uppercase text-[rgb(var(--foreground-muted))]/80 font-medium">
            {isStaged
              ? MEMORY_COPY.observationStatusStaged
              : isPending
              ? MEMORY_COPY.observationStatusPending
              : isIntegrated
              ? MEMORY_COPY.observationStatusIntegrated
              : fact.status}
          </span>
        ) : (
          <span />
        )}

        <span className="flex items-center gap-1 font-medium text-[rgb(var(--accent))] ml-auto">
          <Calendar size={10} />
          {new Date(fact.created_at).toLocaleDateString(undefined, {
            month: "short",
            day: "numeric",
          })}
        </span>
      </div>
    </article>
  );
});

export const LearnedFactsList: React.FC<LearnedFactsListProps> = memo(
  ({
    observations,
    statusFilter,
    isLoading = false,
    isLoadingMore = false,
    hasMore = false,
    onLoadMore,
    isConsolidating = false,
  }) => {
    const listRef = useRef<HTMLDivElement>(null);

    const handleScroll = useCallback(() => {
      const el = listRef.current;
      if (!el || isLoadingMore || !hasMore || !onLoadMore) return;
      if (el.scrollHeight - el.scrollTop - el.clientHeight < 150) {
        onLoadMore();
      }
    }, [isLoadingMore, hasMore, onLoadMore]);

    useEffect(() => {
      const el = listRef.current;
      if (!el) return;
      el.addEventListener("scroll", handleScroll, { passive: true });
      return () => el.removeEventListener("scroll", handleScroll);
    }, [handleScroll]);

    return (
      <div className="flex-1 min-h-0 flex flex-col justify-between">
        {isLoading && observations.length === 0 ? (
          <div className="flex-1 flex flex-col items-center justify-center p-8 text-center">
            <Loader2
              size={24}
              className="animate-spin text-[rgb(var(--accent))] mb-3"
              aria-hidden
            />
            <span className="text-[12px] font-mono text-[rgb(var(--foreground-muted))]">
              {MEMORY_COPY.loadingObservations}
            </span>
          </div>
        ) : observations.length === 0 ? (
          <div className="flex-1 flex flex-col items-center justify-center p-8 text-center">
            <div className="w-12 h-12 rounded-2xl bg-[rgba(var(--accent),0.08)] border border-[rgba(var(--accent),0.18)] flex items-center justify-center text-[rgb(var(--accent))] mb-3">
              <Circle size={20} />
            </div>
            <h4 className="text-[13px] font-semibold text-[rgb(var(--foreground))] mb-1">
              {statusFilter === "staged"
                ? MEMORY_COPY.observationEmptyStaged
                : statusFilter === "pending"
                ? MEMORY_COPY.observationEmptyPending
                : statusFilter === "integrated"
                ? MEMORY_COPY.observationEmptyIntegrated
                : MEMORY_COPY.observationEmptyAll}
            </h4>
            <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] max-w-xs">
              {statusFilter === "staged" || statusFilter === "pending"
                ? MEMORY_COPY.observationEmptyActiveHint
                : MEMORY_COPY.observationEmptyHint}
            </p>
          </div>
        ) : (
          <div
            ref={listRef}
            className="flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-1 pt-1 grid grid-cols-[repeat(auto-fill,minmax(190px,1fr))] gap-3 content-start"
          >
            {observations.map((fact) => (
              <ObservationCard
                key={fact.id}
                fact={fact}
                dimmed={Boolean(isConsolidating)}
                showStatus={statusFilter === "all"}
              />
            ))}

            {/* Paginated loading footer */}
            {isLoadingMore && (
              <div className="col-span-full flex items-center justify-center py-3 text-center">
                <Loader2
                  size={16}
                  className="animate-spin text-[rgb(var(--accent))] mr-2"
                  aria-hidden
                />
                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                  {MEMORY_COPY.loadingMoreObservations}
                </span>
              </div>
            )}

            {!hasMore && observations.length >= 25 && (
              <div className="col-span-full text-center py-2 text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/60">
                {MEMORY_COPY.allObservationsLoaded}
              </div>
            )}
          </div>
        )}
      </div>
    );
  }
);

LearnedFactsList.displayName = "LearnedFactsList";
