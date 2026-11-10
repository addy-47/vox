import { memo, useMemo, useRef, useCallback } from "react";
import { type SessionRow } from "@/services/persistenceService";
import { isBoundaryCard, isSkeletonCard, type CarouselItemData } from "./recallTypes";

export interface RecallTimelineScrubberProps {
  items: CarouselItemData[];
  activeIndex: number;
  onSelectIndex: (index: number) => void;
  onPrev?: () => void;
  onNext?: () => void;
}

export const RecallTimelineScrubber = memo(({
  items,
  activeIndex,
  onSelectIndex,
}: RecallTimelineScrubberProps) => {
  const trackRef = useRef<HTMLDivElement>(null);

  // Filter out skeletons and boundary cards to get strictly valid sessions
  const realSessions = useMemo(() => {
    return items
      .map((it, idx) => ({ item: it, originalIndex: idx }))
      .filter((entry): entry is { item: SessionRow; originalIndex: number } => {
        return !isSkeletonCard(entry.item) && !isBoundaryCard(entry.item);
      });
  }, [items]);

  const realCount = realSessions.length;

  // Derive unique months represented across the real sessions
  const monthMilestones = useMemo(() => {
    if (realCount === 0) return [];
    const months = new Map<string, number>();

    realSessions.forEach(({ item }, idx) => {
      const d = new Date(item.created_at);
      const monthName = d.toLocaleDateString("en-US", { month: "short" });
      const year = d.getFullYear();
      const key = `${monthName} ${year}`;
      if (!months.has(key)) {
        const ratio = realCount > 1 ? idx / (realCount - 1) : 0.5;
        months.set(key, ratio);
      }
    });

    return Array.from(months.entries()).map(([label, ratio]) => ({
      label: label.split(" ")[0],
      ratio,
    }));
  }, [realSessions, realCount]);

  // Decimate bars to a maximum of 40 histogram columns for clean rendering
  const histogramBars = useMemo(() => {
    if (realCount === 0) return [];
    const maxBars = 40;
    const step = Math.max(1, (realCount - 1) / (maxBars - 1));
    const bars: Array<{
      sessionIndex: number;
      ratio: number;
      height: number;
      isCurrent: boolean;
      turns: number;
    }> = [];

    const barCount = Math.min(realCount, maxBars);
    for (let b = 0; b < barCount; b++) {
      const sIdx = Math.min(realCount - 1, Math.round(b * step));
      const entry = realSessions[sIdx];
      if (!entry) continue;

      const turns = entry.item.turn_count || 1;
      // Height scaled between 8px and 26px based on conversation turn density
      const density = Math.min(1, Math.max(0.15, turns / 12));
      const height = Math.round(7 + density * 19);

      const ratio = realCount > 1 ? sIdx / (realCount - 1) : 0.5;
      const isCurrent = entry.originalIndex === activeIndex;

      bars.push({
        sessionIndex: entry.originalIndex,
        ratio,
        height,
        isCurrent,
        turns,
      });
    }

    return bars;
  }, [realSessions, realCount, activeIndex]);

  const handleTrackClick = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const track = trackRef.current;
    if (!track || realCount === 0) return;
    const rect = track.getBoundingClientRect();
    const clickX = e.clientX - rect.left;
    const ratio = Math.max(0, Math.min(1, clickX / rect.width));
    const targetSessionIdx = Math.round(ratio * (realCount - 1));
    const target = realSessions[targetSessionIdx];
    if (target) {
      onSelectIndex(target.originalIndex);
    }
  }, [realCount, realSessions, onSelectIndex]);

  if (realCount === 0) return null;

  return (
    <div className="w-full max-w-xl px-4 flex flex-col items-center gap-1.5 select-none pointer-events-auto">
      {/* Histogram Track (No Chevrons, Tightly Framed) */}
      <div
        ref={trackRef}
        onClick={handleTrackClick}
        className="relative w-full h-8 flex items-end justify-between cursor-pointer py-1 px-1 group"
        aria-label="Timeline Scrubber"
        role="slider"
        aria-valuenow={activeIndex}
      >
        {/* Subtle baseline track indicator */}
        <div className="absolute inset-x-0 bottom-1 h-[1.5px] bg-[rgba(var(--foreground),0.12)] rounded-full pointer-events-none" />

        {/* Vertical density bars */}
        <div className="relative w-full h-full flex items-end">
          {histogramBars.map((bar, i) => (
            <div
              key={i}
              className="absolute bottom-1 -translate-x-1/2 flex flex-col items-center transition-all duration-150"
              style={{ left: `${(bar.ratio * 100).toFixed(1)}%` }}
            >
              <div
                style={{ height: `${bar.isCurrent ? Math.max(bar.height + 4, 24) : bar.height}px` }}
                className={`rounded-full transition-all duration-150 ${
                  bar.isCurrent
                    ? "w-[3px] bg-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.9)] z-10"
                    : "w-[2px] bg-[rgba(var(--foreground-muted),0.35)] group-hover:bg-[rgba(var(--foreground-muted),0.55)] hover:bg-[rgb(var(--foreground))]"
                }`}
              />
            </div>
          ))}
        </div>
      </div>

      {/* Month Markers directly beneath histogram */}
      <div className="relative w-full h-3.5 px-1">
        {monthMilestones.map((m) => (
          <span
            key={m.label}
            className="absolute top-0 -translate-x-1/2 text-[10px] font-mono text-[rgb(var(--foreground-muted))]/60 tracking-wider select-none"
            style={{ left: `${(m.ratio * 100).toFixed(1)}%` }}
          >
            {m.label}
          </span>
        ))}
      </div>
    </div>
  );
});

RecallTimelineScrubber.displayName = "RecallTimelineScrubber";
