import React, { useState, useMemo, memo } from "react";
import { ChevronLeft, ChevronRight, RotateCcw, Calendar as CalendarIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface CalendarPickerProps {
  startDate: string | null; // "YYYY-MM-DD"
  endDate: string | null;   // "YYYY-MM-DD"
  onChange: (start: string | null, end: string | null) => void;
  sessionDates?: Set<string>;
  initialDate?: Date;
  className?: string;
}

export function toDateKey(d: Date): string {
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

const WEEKDAYS = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

export const CalendarPicker: React.FC<CalendarPickerProps> = memo(
  ({
    startDate,
    endDate,
    onChange,
    sessionDates,
    initialDate,
    className,
  }) => {
    // Current viewed month and year in calendar
    const [viewDate, setViewDate] = useState(() => {
      if (startDate) {
        const [y, m] = startDate.split("-").map(Number);
        return new Date(y, m - 1, 1);
      }
      return initialDate ? new Date(initialDate.getFullYear(), initialDate.getMonth(), 1) : new Date();
    });

    const year = viewDate.getFullYear();
    const month = viewDate.getMonth();

    const monthName = viewDate.toLocaleString("en-US", { month: "short" }).toUpperCase();

    const prevMonth = () => {
      setViewDate(new Date(year, month - 1, 1));
    };

    const nextMonth = () => {
      setViewDate(new Date(year, month + 1, 1));
    };

    // Days in the grid
    const calendarCells = useMemo(() => {
      const firstDayIndex = new Date(year, month, 1).getDay();
      const daysInMonth = new Date(year, month + 1, 0).getDate();
      const prevMonthDays = new Date(year, month, 0).getDate();

      const cells: {
        dateKey: string;
        dayNum: number;
        isCurrentMonth: boolean;
      }[] = [];

      // Prev month filler
      for (let i = firstDayIndex - 1; i >= 0; i--) {
        const d = prevMonthDays - i;
        const dateObj = new Date(year, month - 1, d);
        cells.push({
          dateKey: toDateKey(dateObj),
          dayNum: d,
          isCurrentMonth: false,
        });
      }

      // Current month days
      for (let d = 1; d <= daysInMonth; d++) {
        const dateObj = new Date(year, month, d);
        cells.push({
          dateKey: toDateKey(dateObj),
          dayNum: d,
          isCurrentMonth: true,
        });
      }

      // Next month filler to complete 35 or 42 cells (multiple of 7)
      const remaining = (7 - (cells.length % 7)) % 7;
      for (let d = 1; d <= remaining; d++) {
        const dateObj = new Date(year, month + 1, d);
        cells.push({
          dateKey: toDateKey(dateObj),
          dayNum: d,
          isCurrentMonth: false,
        });
      }

      return cells;
    }, [year, month]);

    const handleDateClick = (dateKey: string) => {
      if (!startDate || (startDate && endDate)) {
        // Start fresh selection
        onChange(dateKey, null);
      } else {
        // Complete the range
        if (dateKey < startDate) {
          onChange(dateKey, startDate);
        } else if (dateKey === startDate) {
          // Double click on same date selects just that date
          onChange(dateKey, dateKey);
        } else {
          onChange(startDate, dateKey);
        }
      }
    };

    const handleReset = () => {
      onChange(null, null);
    };

    return (
      <div
        className={cn(
          "w-full p-2.5 rounded-xl bg-[rgba(var(--card),0.95)] border border-[rgba(var(--accent),0.25)] shadow-xl backdrop-blur-2xl text-[11px] font-mono select-none space-y-2",
          className
        )}
      >
        {/* Month Header Navigation */}
        <div className="flex items-center justify-between px-1">
          <div className="flex items-center gap-1.5">
            <CalendarIcon size={12} className="text-[rgb(var(--accent))]" />
            <span className="font-bold tracking-wider text-[rgb(var(--foreground))] uppercase text-[11px]">
              {monthName} {year}
            </span>
          </div>

          <div className="flex items-center gap-1">
            {startDate && (
              <button
                type="button"
                onClick={handleReset}
                title="Reset date range"
                className="text-[10px] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] mr-1 transition-colors cursor-pointer flex items-center gap-0.5"
              >
                <RotateCcw size={10} /> Clear
              </button>
            )}
            <button
              type="button"
              onClick={prevMonth}
              className="w-5 h-5 rounded flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] cursor-pointer transition-colors"
              aria-label="Previous month"
            >
              <ChevronLeft size={13} />
            </button>
            <button
              type="button"
              onClick={nextMonth}
              className="w-5 h-5 rounded flex items-center justify-center text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] cursor-pointer transition-colors"
              aria-label="Next month"
            >
              <ChevronRight size={13} />
            </button>
          </div>
        </div>

        {/* Weekday Labels */}
        <div className="grid grid-cols-7 gap-1 text-center font-bold text-[9px] uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70">
          {WEEKDAYS.map((wd) => (
            <div key={wd} className="py-0.5">
              {wd}
            </div>
          ))}
        </div>

        {/* Days Grid */}
        <div className="grid grid-cols-7 gap-1">
          {calendarCells.map((cell) => {
            const { dateKey, dayNum, isCurrentMonth } = cell;
            const isStart = startDate === dateKey;
            const isEnd = endDate === dateKey;
            const isInRange =
              startDate && endDate && dateKey > startDate && dateKey < endDate;
            const hasSession = sessionDates?.has(dateKey);

            return (
              <button
                key={dateKey}
                type="button"
                disabled={!isCurrentMonth}
                onClick={() => handleDateClick(dateKey)}
                className={cn(
                  "relative h-6 flex items-center justify-center text-[10.5px] font-mono transition-all",
                  !isCurrentMonth && "text-[rgb(var(--foreground-muted))]/25 pointer-events-none",
                  isCurrentMonth && !isStart && !isEnd && !isInRange &&
                    "text-[rgb(var(--foreground))] hover:bg-[rgba(var(--accent),0.12)] hover:text-[rgb(var(--accent))] rounded-md cursor-pointer",
                  isInRange &&
                    "font-semibold rounded-none cursor-pointer",
                  (isStart || isEnd) &&
                    "font-black rounded-md shadow-[0_0_8px_rgba(var(--accent),0.35)] cursor-pointer z-10",
                  isStart && endDate && "rounded-r-none",
                  isEnd && startDate && "rounded-l-none"
                )}
                style={
                  isStart || isEnd
                    ? {
                        backgroundColor: "rgb(var(--accent))",
                        color: "rgb(var(--accent-foreground, 255, 255, 255))",
                      }
                    : isInRange
                    ? {
                        backgroundColor: "rgba(var(--accent), 0.18)",
                        color: "rgb(var(--accent))",
                      }
                    : undefined
                }
              >
                <span>{dayNum}</span>
                {/* Session activity dot */}
                {hasSession && isCurrentMonth && (
                  <span
                    className="absolute bottom-0.5 left-1/2 -translate-x-1/2 w-1 h-1 rounded-full pointer-events-none"
                    style={{
                      backgroundColor:
                        isStart || isEnd
                          ? "rgb(var(--accent-foreground, 255, 255, 255))"
                          : "rgb(var(--accent))",
                    }}
                  />
                )}
              </button>
            );
          })}
        </div>

        {/* Selected Range Display Note */}
        {(startDate || endDate) && (
          <div className="pt-1 border-t border-[rgba(var(--border),0.1)] flex items-center justify-between text-[9.5px] text-[rgb(var(--foreground-muted))] font-mono">
            <span>
              {startDate && !endDate && `Selected: ${startDate}`}
              {startDate && endDate && startDate === endDate && `Date: ${startDate}`}
              {startDate && endDate && startDate !== endDate && `${startDate} → ${endDate}`}
            </span>
          </div>
        )}
      </div>
    );
  }
);
CalendarPicker.displayName = "CalendarPicker";
