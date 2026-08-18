import { memo, useState, useEffect } from "react";
import { ChevronLeft, ChevronRight, MessageSquare, Brain, Clock } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { HISTORY_COPY } from "@/data/historyCopy";
import { type HistoryView } from "./ViewSelector";

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
    const [isLightMode, setIsLightMode] = useState(false);

    useEffect(() => {
      const checkTheme = () => {
        const theme = document.documentElement.getAttribute("data-theme");
        setIsLightMode(theme === "light");
      };
      checkTheme();

      const observer = new MutationObserver(checkTheme);
      observer.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["data-theme", "class"],
      });
      return () => observer.disconnect();
    }, []);

    // 48 Perimeter dial ticks around the sphere rim
    const totalTicks = 48;

    return (
      <div
        className="relative z-50 flex flex-col items-center justify-center select-none pointer-events-auto"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Central Session Hub Node — Perfectly Centered 3D Acoustic Core */}
        <div
          className="relative rounded-full flex flex-col items-center justify-center text-center transition-all duration-300 overflow-hidden isolate backdrop-blur-md"
          style={{
            width: "clamp(320px, 36vw, 440px)",
            height: "clamp(320px, 36vw, 440px)",
            minWidth: "300px",
            minHeight: "300px",
            maxWidth: "460px",
            maxHeight: "460px",
            background: isLightMode
              ? "radial-gradient(circle at 50% 30%, rgba(255, 255, 255, 0.45) 0%, rgba(255, 255, 255, 0.15) 60%, rgba(var(--accent), 0.10) 100%)"
              : "radial-gradient(circle at 50% 35%, rgba(var(--card), 0.98) 0%, rgba(10, 14, 18, 0.98) 72%, rgba(var(--accent), 0.12) 100%)",
            border: isLightMode
              ? "1.5px solid rgba(var(--accent), 0.45)"
              : "1.5px solid rgba(var(--accent), 0.55)",
            boxShadow: isLightMode
              ? "0 20px 45px -10px rgba(15, 23, 42, 0.08), 0 0 45px rgba(var(--accent), 0.15), inset 0 2px 14px rgba(255, 255, 255, 0.8), inset 0 -10px 25px rgba(var(--accent), 0.10)"
              : "0 25px 70px -10px rgba(0, 0, 0, 0.9), 0 0 60px rgba(var(--accent), 0.22), inset 0 2px 20px rgba(255, 255, 255, 0.14), inset 0 -15px 35px rgba(var(--accent), 0.25)",
          }}
        >
          {/* Perimeter Dial Ticks on Outer Rim */}
          <svg
            className="absolute inset-0 w-full h-full pointer-events-none opacity-35"
            viewBox="0 0 200 200"
            aria-hidden
          >
            {Array.from({ length: totalTicks }, (_, i) => {
              const angle = (i * 360) / totalTicks;
              const isQuarter = i % (totalTicks / 4) === 0;
              const isEighth = i % (totalTicks / 8) === 0;
              const length = isQuarter ? 8 : isEighth ? 5 : 3;
              const strokeColor = isQuarter
                ? "rgb(var(--accent))"
                : "rgba(var(--foreground-muted), 0.5)";
              const strokeWidth = isQuarter ? 2 : 1;

              return (
                <line
                  key={i}
                  x1={100}
                  y1={5}
                  x2={100}
                  y2={5 + length}
                  stroke={strokeColor}
                  strokeWidth={strokeWidth}
                  transform={`rotate(${angle} 100 100)`}
                />
              );
            })}
          </svg>

          {/* Segmented window arc on the rim */}
          {showArc && (
            <svg
              className="absolute inset-0 w-full h-full pointer-events-none z-10"
              viewBox="0 0 100 100"
              aria-hidden
            >
              {Array.from({ length: windowProgress.count }, (_, i) => {
                const active = i <= windowProgress.index;
                const angle = (i * 360) / windowProgress.count - 90;
                return (
                  <circle
                    key={i}
                    cx={50}
                    cy={50}
                    r={46}
                    fill="none"
                    stroke={active ? "rgb(var(--accent))" : "rgba(var(--foreground-muted), 0.20)"}
                    strokeWidth={active ? 2.5 : 1}
                    strokeDasharray={`${Math.PI * 2 * 46 * (1 / windowProgress.count) - 3} ${1000}`}
                    strokeLinecap="round"
                    transform={`rotate(${angle} 50 50)`}
                  />
                );
              })}
            </svg>
          )}

          {/* ── Inner Circular Safe Zone: Centered Stack with Zero Edge Clipping ── */}
          <div className="relative z-20 flex flex-col items-center justify-between w-[74%] h-[74%] py-1">
            {/* 1. Top View Switcher */}
            <div
              className={cn(
                "flex items-center p-0.5 rounded-full border shadow-inner transition-colors backdrop-blur-sm",
                isLightMode
                  ? "bg-white/45 border-[rgba(var(--accent),0.3)] shadow-slate-200/40"
                  : "bg-black/50 border-[rgba(var(--accent),0.25)]"
              )}
            >
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onViewChange("day");
                }}
                className={cn(
                  "px-3.5 py-0.5 rounded-full text-[10px] font-mono font-bold tracking-[0.18em] uppercase transition-all duration-200 cursor-pointer",
                  view === "day"
                    ? "bg-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.6)] shadow-[0_0_10px_rgba(var(--accent),0.4)]"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] opacity-60 hover:opacity-100 border border-transparent"
                )}
              >
                DAY
              </button>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onViewChange("month");
                }}
                className={cn(
                  "px-3.5 py-0.5 rounded-full text-[10px] font-mono font-bold tracking-[0.18em] uppercase transition-all duration-200 cursor-pointer",
                  view === "month"
                    ? "bg-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.6)] shadow-[0_0_10px_rgba(var(--accent),0.4)]"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] opacity-60 hover:opacity-100 border border-transparent"
                )}
              >
                MONTH
              </button>
            </div>

            {/* 2. Middle Row: Prev Button — Centered Hero Date + Metrics Stack — Next Button */}
            <div className="relative flex items-center justify-center w-full my-auto">
              {/* Prev Button */}
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onPrev();
                }}
                disabled={!canPrev}
                className={cn(
                  "absolute -left-6 sm:-left-7 top-1/2 -translate-y-1/2 w-8 h-8 sm:w-9 sm:h-9 rounded-full border flex items-center justify-center text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/20 hover:border-[rgb(var(--accent))] disabled:opacity-10 disabled:pointer-events-none transition-all cursor-pointer backdrop-blur-sm",
                  isLightMode
                    ? "bg-white/50 border-[rgba(var(--accent),0.35)] shadow-md shadow-slate-300/30"
                    : "bg-black/60 border-[rgba(var(--accent),0.35)] shadow-[0_0_12px_rgba(0,0,0,0.6)]"
                )}
                aria-label={variant === "day" ? HISTORY_COPY.prevDay : HISTORY_COPY.prevMonth}
              >
                <ChevronLeft size={18} strokeWidth={2.5} />
              </button>

              {/* Date, Weekday, & Telemetry Stack */}
              <div className="flex flex-col items-center justify-center text-center px-4">
                {/* Year with Accent Pips: e.g. "• 2 0 2 6 •" */}
                <div className="flex items-center gap-1.5 mb-0.5">
                  <span className="w-1 h-1 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_5px_rgb(var(--accent))]" />
                  <span className="text-[10px] font-mono font-bold tracking-[0.3em] text-[rgb(var(--foreground-muted))] uppercase opacity-90">
                    {secondaryLabel}
                  </span>
                  <span className="w-1 h-1 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_5px_rgb(var(--accent))]" />
                </div>

                {/* Hero Date: Dual-Tone "AUG 12" in Day view, or Full Month Name "AUGUST" in Month view */}
                {variant === "day" && dayHeroParts ? (
                  <div className="flex items-baseline gap-1.5 font-display font-black tracking-tight leading-none">
                    <span
                      className="text-[rgb(var(--foreground))]"
                      style={{ fontSize: "clamp(28px, 3.4vw, 42px)" }}
                    >
                      {dayHeroParts.month}
                    </span>
                    <span
                      className="text-[rgb(var(--accent))] drop-shadow-[0_0_18px_rgba(var(--accent),0.65)]"
                      style={{ fontSize: "clamp(28px, 3.4vw, 42px)" }}
                    >
                      {dayHeroParts.day}
                    </span>
                  </div>
                ) : (
                  <span
                    className="font-display font-black tracking-tight text-[rgb(var(--accent))] leading-none drop-shadow-[0_0_18px_rgba(var(--accent),0.65)] uppercase"
                    style={{ fontSize: "clamp(24px, 3.2vw, 36px)" }}
                  >
                    {monthFullLabel || primaryLabel}
                  </span>
                )}

                {/* Weekday Subtitle / View mode overview */}
                <span className="text-[9.5px] font-mono font-bold tracking-[0.32em] text-[rgb(var(--foreground-muted))] uppercase mt-1 opacity-80">
                  {variant === "day" ? weekdayLabel || "TODAY" : "OVERVIEW"}
                </span>

                {/* Direct Sub-Date Metrics Row: Session Count & Memory Count (No redundant center node) */}
                <div
                  className={cn(
                    "flex items-center gap-4 mt-3 px-3 py-1 rounded-xl border shadow-inner transition-colors backdrop-blur-sm",
                    isLightMode
                      ? "bg-white/45 border-[rgba(var(--accent),0.25)] shadow-slate-200/40"
                      : "bg-black/30 border-[rgba(var(--accent),0.18)]"
                  )}
                >
                  {/* Sessions */}
                  <div className="flex items-center gap-1.5">
                    <MessageSquare size={13} className="text-[rgb(var(--accent))] shrink-0" />
                    <div className="flex items-baseline gap-1">
                      <span className="text-[13px] font-display font-black text-[rgb(var(--foreground))]">
                        {sessionsCount}
                      </span>
                      <span className="text-[8.5px] font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))] uppercase">
                        SESSIONS
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
                      <span className="text-[8.5px] font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))] uppercase">
                        MEMORIES
                      </span>
                    </div>
                  </div>
                </div>
              </div>

              {/* Next Button */}
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  onNext();
                }}
                disabled={!canNext}
                className={cn(
                  "absolute -right-6 sm:-right-7 top-1/2 -translate-y-1/2 w-8 h-8 sm:w-9 sm:h-9 rounded-full border flex items-center justify-center text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/20 hover:border-[rgb(var(--accent))] disabled:opacity-10 disabled:pointer-events-none transition-all cursor-pointer",
                  isLightMode
                    ? "bg-white/90 border-[rgba(var(--accent),0.35)] shadow-md shadow-slate-300/40"
                    : "bg-black/60 border-[rgba(var(--accent),0.35)] shadow-[0_0_12px_rgba(0,0,0,0.6)]"
                )}
                aria-label={variant === "day" ? HISTORY_COPY.nextDay : HISTORY_COPY.nextMonth}
              >
                <ChevronRight size={18} strokeWidth={2.5} />
              </button>
            </div>

            {/* 3. Bottom Section: Time Span Footer */}
            <div className="w-full flex flex-col gap-1">
              <div className="w-full h-[1px] bg-gradient-to-r from-transparent via-[rgba(var(--accent),0.25)] to-transparent" />
              <div className="flex items-center justify-center gap-1.5 text-[10px] font-mono text-[rgb(var(--foreground-muted))] pt-0.5">
                <Clock size={11} className="text-[rgb(var(--accent))]" />
                <span className="text-[9px] font-bold uppercase tracking-wider opacity-70">SPAN</span>
                <span className="text-[10px] font-bold text-[rgb(var(--foreground))]">
                  {timeSpanLabel || windowLabel || "00:00 – 23:59"}
                </span>
                {showArc && (
                  <span className="text-[rgb(var(--accent))] font-semibold text-[9px]">
                    [{windowProgress.index + 1}/{windowProgress.count}]
                  </span>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  }
);

CentralClockNode.displayName = "CentralClockNode";

