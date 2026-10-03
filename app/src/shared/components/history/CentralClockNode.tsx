import { memo } from "react";
import { ChevronLeft, ChevronRight, MessageSquare, Brain, Clock } from "lucide-react";
import { HISTORY_COPY } from "@/data/historyCopy";
import { ViewSelector, type HistoryView } from "./ViewSelector";

export interface WindowProgress {
  /** 0-based index of the visible window. */
  index: number;
  /** Total windows for the day/month. */
  count: number;
}

export interface CentralClockNodeProps {
  variant: "day" | "month";
  view: HistoryView;
  onViewChange: (view: HistoryView) => void;
  primaryLabel: string;
  secondaryLabel: string;
  metaLabel: string;
  dayHeroParts?: { month: string; day: string };
  dateSpanLabel?: string | null;
  weekdayLabel?: string;
  monthFullLabel?: string;
  sessionsCount: number;
  memoriesCount: number;
  timeSpanLabel?: string | null;
  /** Current window range, e.g. "07:12 – 11:48" (day) or "1–12" (month). */
  windowLabel?: string;
  /** Segmented rim arc showing position within the day/month's windows. */
  windowProgress?: WindowProgress;
  canPrev: boolean;
  canNext: boolean;
  onPrev: () => void;
  onNext: () => void;
  /** Day view: jump back to the newest day. */
  onGoToday?: () => void;
  /** Day view: breadcrumb back to the parent month. */
  onBackToMonth?: () => void;
  /** Day view: breadcrumb label, e.g. "AUG 2026". */
  breadcrumbLabel?: string;
  /** Optional compact flag for backwards compatibility. */
  compact?: boolean;
}

export const CentralClockNode = memo(
  ({
    variant,
    view,
    onViewChange,
    primaryLabel,
    secondaryLabel,
    dayHeroParts,
    dateSpanLabel,
    weekdayLabel,
    monthFullLabel,
    sessionsCount,
    memoriesCount,
    timeSpanLabel,
    windowLabel,
    windowProgress,
    canPrev,
    canNext,
    onPrev,
    onNext,
  }: CentralClockNodeProps) => {
    const showArc = windowProgress && windowProgress.count > 1;

    // 48 perimeter dial ticks around the sphere rim. Precomputed once at module
    // scope: these 48 <line> elements were previously rebuilt on every render.
    const DIAL_TICKS = 48;
    const DIAL_TICK_ELEMENTS = Array.from({ length: DIAL_TICKS }, (_, i) => {
      const angle = (i * 360) / DIAL_TICKS;
      const isQuarter = i % (DIAL_TICKS / 4) === 0;
      const isEighth = i % (DIAL_TICKS / 8) === 0;
      const length = isQuarter ? 8 : isEighth ? 5 : 3;
      return (
        <line
          key={i}
          x1={100}
          y1={2}
          x2={100}
          y2={2 + length}
          stroke={isQuarter ? "rgb(var(--accent))" : "rgba(var(--foreground-muted), 0.6)"}
          strokeWidth={isQuarter ? 2 : 1}
          transform={`rotate(${angle} 100 100)`}
        />
      );
    });

    return (
      <div
        className="relative z-50 flex flex-col items-center justify-center select-none pointer-events-auto"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Floating Chevrons (outside clock sphere, icon only, no outer border or background) */}
        <button
          onClick={(e) => {
            e.stopPropagation();
            onPrev();
          }}
          disabled={!canPrev}
          className="absolute -left-10 sm:-left-12 top-1/2 -translate-y-1/2 p-2 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] disabled:opacity-10 disabled:pointer-events-none transition-colors cursor-pointer z-30"
          aria-label={variant === "day" ? HISTORY_COPY.prevDay : HISTORY_COPY.prevMonth}
        >
          <ChevronLeft size={24} strokeWidth={2} />
        </button>

        <button
          onClick={(e) => {
            e.stopPropagation();
            onNext();
          }}
          disabled={!canNext}
          className="absolute -right-10 sm:-right-12 top-1/2 -translate-y-1/2 p-2 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] disabled:opacity-10 disabled:pointer-events-none transition-colors cursor-pointer z-30"
          aria-label={variant === "day" ? HISTORY_COPY.nextDay : HISTORY_COPY.nextMonth}
        >
          <ChevronRight size={24} strokeWidth={2} />
        </button>

        {/* Central Session Hub Node — Perfectly Centered 3D Acoustic Core */}
        <div
          className="theme-flip-surface clock-hub glass-keep-blur relative rounded-full flex flex-col items-center justify-center text-center transition-all overflow-hidden isolate glass-card"
          style={{
            width: "clamp(250px, 26vw, 320px)",
            height: "clamp(250px, 26vw, 320px)",
            minWidth: "240px",
            minHeight: "240px",
            maxWidth: "330px",
            maxHeight: "330px",
          }}
        >
          {/* Perimeter Dial Ticks on Outer Sphere Rim */}
          <svg
            className="absolute inset-0 w-full h-full pointer-events-none opacity-40 z-10"
            viewBox="0 0 200 200"
            aria-hidden
          >
            {DIAL_TICK_ELEMENTS}
          </svg>

          {/* Active window arc directly on outer sphere boundary */}
          {showArc && (
            <svg
              className="absolute inset-0 w-full h-full pointer-events-none z-20"
              viewBox="0 0 200 200"
              aria-hidden
            >
              {/* Only the active window renders. This previously mapped the whole window
                set, computed 4 trig values per entry, then returned null for
                every non-current one — 208 wasted paths' worth of arithmetic
                plus 208 null children on a 208-window month view. */}
              {(() => {
                const i = windowProgress.index;
                if (i < 0 || i >= windowProgress.count) return null;
                const anglePerSegment = 360 / windowProgress.count;
                const startAngle = i * anglePerSegment - 90;
                const endAngle = (i + 1) * anglePerSegment - 90 - 4;
                const r = 97.5;
                const startRad = (startAngle * Math.PI) / 180;
                const endRad = (endAngle * Math.PI) / 180;
                const x1 = 100 + r * Math.cos(startRad);
                const y1 = 100 + r * Math.sin(startRad);
                const x2 = 100 + r * Math.cos(endRad);
                const y2 = 100 + r * Math.sin(endRad);

                return (
                  <path
                    d={`M ${x1} ${y1} A ${r} ${r} 0 0 1 ${x2} ${y2}`}
                    fill="none"
                    stroke="rgb(var(--accent))"
                    strokeWidth={3}
                    strokeLinecap="round"
                    className="drop-shadow-[0_0_8px_rgba(var(--accent),0.6)]"
                  />
                );
              })()}
            </svg>
          )}


          {/* ── Inner Circular Safe Zone: Centered Content with Perfect Breathing Room ── */}
          <div className="relative z-20 flex flex-col items-center justify-between w-[82%] h-[82%] pt-1.5 pb-2.5 select-none">
            {/* 1. Top Section: Mode Toggle — the shared ViewSelector (correct
                 role="tablist"/"tab", aria-selected, arrow-key roving focus).
                 This node previously hand-rolled two unlabelled rounded-full
                 buttons and hardcoded "DAY" while pulling "MONTH" from copy. */}
            <div
              className="clock-pill theme-flip-surface flex items-center gap-4 rounded-full px-4 py-1 shadow-inner transition-colors"
              onClick={(e) => e.stopPropagation()}
            >
              <ViewSelector view={view} onChange={onViewChange} />
            </div>

            {/* 2. Middle Row: Centered Hero Date + Metrics Stack */}
            <div className="flex flex-col items-center justify-center text-center my-auto">
              {/* Year with Accent Pips: e.g. "• 2 0 2 6 •" */}
              <div className="flex items-center gap-1.5 mb-0.5">
                <span className="w-1 h-1 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_5px_rgb(var(--accent))]" />
                <span className="text-[11px] font-mono font-bold tracking-[0.3em] text-[rgb(var(--foreground-muted))] uppercase opacity-90">
                  {secondaryLabel}
                </span>
                <span className="w-1 h-1 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_5px_rgb(var(--accent))]" />
              </div>

              {/* Hero Date: Custom Date Span (e.g. "SEP 11 – 15"), Dual-Tone "AUG 12", or Month Name */}
              {variant === "day" && dateSpanLabel ? (
                <span
                  className="font-display font-black tracking-tight text-[rgb(var(--foreground))] leading-none"
                  style={{ fontSize: "clamp(18px, 2.2vw, 24px)" }}
                >
                  {dateSpanLabel}
                </span>
              ) : variant === "day" && dayHeroParts ? (
                <div className="flex items-baseline gap-1.5 font-display font-black tracking-tight leading-none">
                  <span
                    className="text-[rgb(var(--foreground))]"
                    style={{ fontSize: "clamp(22px, 2.6vw, 28px)" }}
                  >
                    {dayHeroParts.month}
                  </span>
                  <span
                    className="text-[rgb(var(--accent))] drop-shadow-[0_0_18px_rgba(var(--accent),0.65)]"
                    style={{ fontSize: "clamp(22px, 2.6vw, 28px)" }}
                  >
                    {dayHeroParts.day}
                  </span>
                </div>
              ) : (
                <span
                  className="font-display font-black tracking-tight text-[rgb(var(--accent))] leading-none drop-shadow-[0_0_18px_rgba(var(--accent),0.65)] uppercase"
                  style={{ fontSize: "clamp(20px, 2.5vw, 28px)" }}
                >
                  {monthFullLabel || primaryLabel}
                </span>
              )}

              {/* Weekday Subtitle / View mode overview */}
              <span className="text-[11px] font-mono font-bold tracking-[0.32em] text-[rgb(var(--foreground-muted))] uppercase mt-1">
                {variant === "day" ? weekdayLabel || "TODAY" : "OVERVIEW"}
              </span>

              {/* Direct Sub-Date Metrics Row: Session Count & Memory Count (Direct text without pill container) */}
              <div className="flex items-center gap-3 mt-2 text-[rgb(var(--foreground))]">
                {/* Sessions */}
                <div className="flex items-center gap-1.5">
                  <MessageSquare size={13} className="text-[rgb(var(--accent))] shrink-0" />
                  <div className="flex items-baseline gap-1">
                    <span className="text-[13px] font-display font-black text-[rgb(var(--foreground))] gap-2">
                      {sessionsCount}
                    </span>
                    <span className="text-[11px] font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))] uppercase">
                      {HISTORY_COPY.clockSessions}
                    </span>
                  </div>
                </div>

                {/* Subtle Separator Dot */}
                <span className="w-1 h-1 rounded-full bg-[rgb(var(--accent))]/40" />

                {/* Memories */}
                <div className="flex items-center gap-1.5">
                  <Brain size={13} className="text-[rgb(var(--accent))] shrink-0" />
                  <div className="flex items-baseline gap-1">
                    <span className="text-[13px] font-display font-black text-[rgb(var(--foreground))]">
                      {memoriesCount}
                    </span>
                    <span className="text-[11px] font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))] uppercase">
                      {HISTORY_COPY.clockMemories}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            {/* 3. Bottom Section: Time Span Footer */}
            <div className="w-full flex flex-col items-center gap-0.5 shrink-0 pt-0.5 pb-1">
              <div className="w-2/3 h-[1px] bg-gradient-to-r from-transparent via-[rgba(var(--accent),0.25)] to-transparent mb-0.5" />
              <div className="flex items-center justify-center gap-1 text-[9.5px] font-mono text-[rgb(var(--foreground-muted))]">
                <Clock size={10} className="text-[rgb(var(--accent))] shrink-0" />
                <span className="font-bold uppercase tracking-wider">{HISTORY_COPY.clockSpan}</span>
                {showArc && (
                  <span className="text-[rgb(var(--accent))] font-bold">
                    [{windowProgress.index + 1}/{windowProgress.count}]
                  </span>
                )}
              </div>
              {/* No fabricated fallback: without a measured span, show nothing
                  rather than a "00:00 – 23:59" the clock never computed. */}
              {(timeSpanLabel || windowLabel) && (
                <span className="text-[10px] sm:text-[10.5px] font-mono font-bold text-[rgb(var(--foreground))] tracking-tight whitespace-nowrap">
                  {timeSpanLabel || windowLabel}
                </span>
              )}
            </div>
          </div>
        </div>
      </div>
    );
  }
);

CentralClockNode.displayName = "CentralClockNode";

