import { memo, useMemo, useRef, useCallback } from "react";
import { type SessionRow } from "@/services/persistenceService";
import { isBoundaryCard, isSkeletonCard, type CarouselItemData } from "./recallTypes";
import { SESSION_COPY } from "@/data/sessionCopy";

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

  // Determine if carousel is ordered newest-to-oldest (default) or oldest-to-newest
  const isNewestFirst = useMemo(() => {
    if (realSessions.length < 2) return true;
    return realSessions[0].item.created_at >= realSessions[realSessions.length - 1].item.created_at;
  }, [realSessions]);

  // Construct calendar months array spanning from start month to end month
  const calendarMonths = useMemo(() => {
    if (realCount === 0) return [];

    const timestamps = realSessions.map((s) => s.item.created_at);
    const minT = Math.min(...timestamps);
    const maxT = Math.max(...timestamps);

    const dEarliest = new Date(minT);
    const dLatest = new Date(maxT);

    const earliestMonthDate = new Date(dEarliest.getFullYear(), dEarliest.getMonth(), 1);
    const latestMonthDate = new Date(dLatest.getFullYear(), dLatest.getMonth(), 1);

    const months: Array<{ year: number; month: number; key: string; label: string }> = [];

    if (isNewestFirst) {
      // Newest month at left (index 0) down to oldest month at right
      let curr = new Date(latestMonthDate);
      while (curr >= earliestMonthDate) {
        months.push({
          year: curr.getFullYear(),
          month: curr.getMonth(),
          key: `${curr.getFullYear()}-${curr.getMonth()}`,
          label: curr.toLocaleDateString("en-US", { month: "short" }),
        });
        curr = new Date(curr.getFullYear(), curr.getMonth() - 1, 1);
      }
    } else {
      // Oldest month at left (index 0) up to newest month at right
      let curr = new Date(earliestMonthDate);
      while (curr <= latestMonthDate) {
        months.push({
          year: curr.getFullYear(),
          month: curr.getMonth(),
          key: `${curr.getFullYear()}-${curr.getMonth()}`,
          label: curr.toLocaleDateString("en-US", { month: "short" }),
        });
        curr = new Date(curr.getFullYear(), curr.getMonth() + 1, 1);
      }
    }

    return months;
  }, [realSessions, realCount, isNewestFirst]);

  // Generate temporal histogram bars and month milestones
  // If a month has no sessions or few sessions, empty space (no bars) signifies the gap
  const { monthMilestones, histogramBars } = useMemo(() => {
    if (realCount === 0 || calendarMonths.length === 0) {
      return { monthMilestones: [], histogramBars: [] };
    }

    const totalMonths = calendarMonths.length;
    const monthSlotWidth = 1 / totalMonths;

    const bars: Array<{
      sessionIndex: number;
      ratio: number;
      height: number;
      turns: number;
    }> = [];

    const milestones = calendarMonths.map((m, mIdx) => {
      const monthStartRatio = mIdx * monthSlotWidth;
      const monthEndRatio = (mIdx + 1) * monthSlotWidth;
      const monthCenterRatio = (monthStartRatio + monthEndRatio) / 2;

      const mStartT = new Date(m.year, m.month, 1).getTime();
      const mEndT = new Date(m.year, m.month + 1, 0, 23, 59, 59, 999).getTime();

      // Find sessions belonging to this calendar month
      const inMonth = realSessions.filter(
        (s) => s.item.created_at >= mStartT && s.item.created_at <= mEndT
      );

      // If month has sessions, place bars temporally inside this month's slot
      // If month has 0 sessions, NO bars are generated — leaving an empty space on the track!
      if (inMonth.length > 0) {
        const maxBarsInMonth = Math.min(inMonth.length, 12);
        const step = Math.max(1, (inMonth.length - 1) / (maxBarsInMonth - 1));

        for (let b = 0; b < maxBarsInMonth; b++) {
          const sIdx = Math.min(inMonth.length - 1, Math.round(b * step));
          const entry = inMonth[sIdx];

          const turns = entry.item.turn_count || 1;
          const density = Math.min(1, Math.max(0.15, turns / 12));
          const height = Math.round(7 + density * 19);

          // Calculate time fraction within the month
          const rawTimeFrac = (entry.item.created_at - mStartT) / (mEndT - mStartT);
          const timeFrac = isNewestFirst ? 1 - rawTimeFrac : rawTimeFrac;

          // Inset slightly within month boundaries so bars stay cleanly framed
          const inset = monthSlotWidth * 0.08;
          const barRatio = monthStartRatio + inset + timeFrac * (monthSlotWidth - inset * 2);

          bars.push({
            sessionIndex: entry.originalIndex,
            ratio: Math.max(0.01, Math.min(0.99, barRatio)),
            height,
            turns,
          });
        }
      }

      return {
        key: m.key,
        label: m.label,
        ratio: monthCenterRatio,
        hasSessions: inMonth.length > 0,
      };
    });

    return { monthMilestones: milestones, histogramBars: bars };
  }, [realSessions, realCount, calendarMonths, isNewestFirst]);

  // Active session and month based on activeIndex
  const activeSession = useMemo(() => {
    return realSessions.find((s) => s.originalIndex === activeIndex) ?? realSessions[0];
  }, [realSessions, activeIndex]);

  const activeMonthKey = useMemo(() => {
    const d = new Date(activeSession.item.created_at);
    return `${d.getFullYear()}-${d.getMonth()}`;
  }, [activeSession]);

  // Proximity-based bar matching ensures exactly one bar is highlighted for the active session
  const activeBarIndex = useMemo(() => {
    if (histogramBars.length === 0) return -1;
    const directIdx = histogramBars.findIndex((b) => b.sessionIndex === activeSession.originalIndex);
    if (directIdx >= 0) return directIdx;

    let closestIdx = 0;
    let minDiff = Infinity;
    histogramBars.forEach((bar, idx) => {
      const diff = Math.abs(bar.sessionIndex - activeSession.originalIndex);
      if (diff < minDiff) {
        minDiff = diff;
        closestIdx = idx;
      }
    });
    return closestIdx;
  }, [histogramBars, activeSession]);

  const isDraggingTrackRef = useRef(false);

  const seekFromClientX = useCallback(
    (clientX: number) => {
      const track = trackRef.current;
      if (!track || realCount === 0 || histogramBars.length === 0) return;
      const rect = track.getBoundingClientRect();
      const clickX = clientX - rect.left;
      const ratio = Math.max(0, Math.min(1, clickX / rect.width));

      // Find the closest conversation bar to the clicked timeline position
      let closestBar = histogramBars[0];
      let minDiff = Infinity;
      for (const bar of histogramBars) {
        const diff = Math.abs(bar.ratio - ratio);
        if (diff < minDiff) {
          minDiff = diff;
          closestBar = bar;
        }
      }
      onSelectIndex(closestBar.sessionIndex);
    },
    [realCount, histogramBars, onSelectIndex]
  );

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    isDraggingTrackRef.current = true;
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      /* ignore */
    }
    seekFromClientX(e.clientX);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDraggingTrackRef.current) return;
    seekFromClientX(e.clientX);
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDraggingTrackRef.current) return;
    isDraggingTrackRef.current = false;
    try {
      if (e.currentTarget.hasPointerCapture(e.pointerId)) {
        e.currentTarget.releasePointerCapture(e.pointerId);
      }
    } catch {
      /* ignore */
    }
  };

  if (realCount === 0) return null;

  return (
    <div className="w-full max-w-xl px-4 flex flex-col items-center gap-1.5 select-none pointer-events-auto">
      {/* Histogram Track (No Chevrons, Tightly Framed) */}
      <div
        ref={trackRef}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerUp}
        className="relative w-full h-8 flex items-end justify-between cursor-pointer py-1 px-1 group touch-none"
        aria-label={SESSION_COPY.timelineScrubberAriaLabel}
        role="slider"
        aria-valuenow={activeIndex}
      >
        {/* Subtle baseline track indicator */}
        <div className="absolute inset-x-0 bottom-1 h-[1.5px] bg-[rgba(var(--foreground),0.12)] rounded-full pointer-events-none" />

        {/* Vertical density bars */}
        <div className="relative w-full h-full flex items-end pointer-events-none">
          {histogramBars.map((bar, i) => {
            const isCurrent = i === activeBarIndex;
            return (
              <div
                key={i}
                className="absolute bottom-1 -translate-x-1/2 flex flex-col items-center transition-[left] duration-150"
                style={{ left: `${(bar.ratio * 100).toFixed(1)}%` }}
              >
                <div
                  style={{ height: `${isCurrent ? Math.max(bar.height + 4, 24) : bar.height}px` }}
                  className={`rounded-full transition-[height,background-color,box-shadow] duration-150 ${
                    isCurrent
                      ? "w-[3px] bg-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.9)] z-10"
                      : "w-[2px] bg-[rgba(var(--foreground-muted),0.35)] group-hover:bg-[rgba(var(--foreground-muted),0.55)] hover:bg-[rgb(var(--foreground))]"
                  }`}
                />
              </div>
            );
          })}
        </div>
      </div>

      {/* Month Markers directly beneath histogram with active month accent illumination */}
      <div className="relative w-full h-3.5 px-1">
        {monthMilestones.map((m) => {
          const isCurrentMonth = m.key === activeMonthKey;

          return (
            <span
              key={m.key}
              className={`absolute top-0 -translate-x-1/2 text-[11px] font-mono tracking-wider select-none pointer-events-none transition-colors duration-200 ${
                isCurrentMonth
                  ? "text-[rgb(var(--accent))] font-bold drop-shadow-[0_0_8px_rgba(var(--accent),0.7)]"
                  : "text-[rgb(var(--foreground-muted))]/60 font-medium"
              }`}
              style={{ left: `${(m.ratio * 100).toFixed(1)}%` }}
            >
              {m.label}
            </span>
          );
        })}
      </div>
    </div>
  );
});

RecallTimelineScrubber.displayName = "RecallTimelineScrubber";
