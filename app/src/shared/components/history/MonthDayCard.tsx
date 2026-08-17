import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { dayNumberFromKey, type DayGroup } from "./orbitMath";
import { HISTORY_COPY } from "@/data/historyCopy";

export interface MonthDayCardProps {
  day: DayGroup;
  onOpen: (dayKey: string) => void;
}

function weekdayLabel(dayKey: string): string {
  const [y, m, d] = dayKey.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString(undefined, { weekday: "short" });
}

function monthShortLabel(dayKey: string): string {
  const [y, m] = dayKey.split("-").map(Number);
  return new Date(y, m - 1, 1).toLocaleDateString(undefined, { month: "short" });
}

export const MonthDayCard = memo(({ day, onOpen }: MonthDayCardProps) => {
  const turnTotal = day.sessions.reduce((sum, s) => sum + s.turn_count, 0);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onOpen(day.dayKey);
    }
  };

  return (
    <div
      role="button"
      tabIndex={0}
      onKeyDown={handleKeyDown}
      className={cn(
        "w-48 h-32 rounded-2xl p-4 flex flex-col text-left select-none group cursor-pointer transition-all duration-200 glass-card backdrop-blur-xl focus-visible:outline focus-visible:outline-2 focus-visible:outline-[rgb(var(--accent))]",
        "border-[rgba(var(--border),0.15)] bg-[rgb(var(--card))]/75 hover:border-[rgba(var(--accent),0.55)] hover:bg-[rgb(var(--card))]/90 hover:shadow-[0_0_20px_rgba(var(--accent),0.25)] hover:scale-[1.02]"
      )}
      onClick={(e) => {
        e.stopPropagation();
        onOpen(day.dayKey);
      }}
    >
      {/* Weekday + session count row */}
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-mono font-bold uppercase tracking-[0.16em] text-[rgb(var(--foreground-muted))]">
          {weekdayLabel(day.dayKey)}
        </span>
        <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))]">
          {day.sessions.length}{" "}
          {day.sessions.length === 1
            ? HISTORY_COPY.sessionSingular
            : HISTORY_COPY.sessionPlural}
        </span>
      </div>

      {/* Day number anchor */}
      <div className="flex-1 flex flex-col items-center justify-center leading-none">
        <span className="font-display text-[26px] font-black tracking-tight text-[rgb(var(--foreground))] drop-shadow-[0_2px_8px_rgba(0,0,0,0.4)]">
          {dayNumberFromKey(day.dayKey)}
        </span>
        <span className="text-[11px] font-mono font-bold uppercase tracking-[0.2em] text-[rgb(var(--foreground-muted))] mt-1">
          {monthShortLabel(day.dayKey)}
        </span>
      </div>

      {/* Turn total footer */}
      <div className="flex items-center justify-center gap-1.5 text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
        <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] opacity-70" />
        {turnTotal} {turnTotal === 1 ? HISTORY_COPY.turnSingular : HISTORY_COPY.turnPlural}
      </div>
    </div>
  );
});

MonthDayCard.displayName = "MonthDayCard";