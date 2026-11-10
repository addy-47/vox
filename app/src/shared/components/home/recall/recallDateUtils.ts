import type { SessionRow } from "@/services/persistenceService";

export interface DayGroup {
  dayKey: string;
  dayLabel: string;
  latestTimestamp: number;
  sessions: SessionRow[];
}

export interface MonthGroup {
  monthKey: string;
  monthLabel: string;
  days: DayGroup[];
  totalSessions: number;
}

export interface SessionWindow {
  sessions: SessionRow[];
  label: string;
  dateSpanLabel: string;
  startMs: number;
  endMs: number;
}

export interface MonthWindow {
  days: DayGroup[];
  label: string;
}

export type HistoryView = "day" | "month";

export function ringRadiusFor(width: number, height: number): number {
  return Math.min(width * 0.46, (height * 0.54) / 0.42);
}

export function orbitCapacityFor(
  width: number,
  height: number,
  cardWidth: number = 220
): number {
  const radius = ringRadiusFor(width, height);
  const slots = (2 * Math.PI * radius) / (cardWidth * 1.15);
  return Math.min(24, Math.max(12, Math.floor(slots)));
}

export function toDayKey(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(
    d.getDate()
  ).padStart(2, "0")}`;
}

export function toMonthKey(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

/** Formats card date matching the mockup: "19 Oct" */
export function formatCardDate(ms: number): string {
  if (!ms || isNaN(ms)) return "";
  const d = new Date(ms);
  const day = d.getDate();
  const month = d.toLocaleDateString("en-US", { month: "short" });
  return `${day} ${month}`;
}

export function formatDayLabel(dayKey: string): string {
  if (!dayKey) return "";
  const [y, m, d] = dayKey.split("-").map(Number);
  if (!y || !m || !d) return "";
  return new Date(y, m - 1, d).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

export function formatMonthLabel(monthKey: string): string {
  if (!monthKey) return "";
  const [y, m] = monthKey.split("-").map(Number);
  if (!y || !m) return "";
  return new Date(y, m - 1, 1).toLocaleDateString(undefined, {
    month: "long",
    year: "numeric",
  });
}

export function formatMonthShortLabel(monthKey: string): string {
  if (!monthKey) return "";
  const [y, m] = monthKey.split("-").map(Number);
  if (!y || !m) return "";
  return new Date(y, m - 1, 1).toLocaleDateString(undefined, {
    month: "short",
    year: "numeric",
  });
}

export function formatClockTime(ms: number): string {
  if (!ms || isNaN(ms)) return "";
  return new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

export function formatDayHeroLabel(dayKey: string): string {
  if (!dayKey) return "";
  const [y, m, d] = dayKey.split("-").map(Number);
  if (!y || !m || !d) return "";
  const monthShort = new Date(y, m - 1, 1).toLocaleDateString(undefined, {
    month: "short",
  });
  return `${monthShort.toUpperCase()} ${d}`;
}

export function formatDayHeroParts(dayKey: string): { month: string; day: string } {
  if (!dayKey) return { month: "", day: "" };
  const [y, m, d] = dayKey.split("-").map(Number);
  if (!y || !m || !d) return { month: "", day: "" };
  const monthShort = new Date(y, m - 1, 1).toLocaleDateString(undefined, {
    month: "short",
  });
  return {
    month: monthShort.toUpperCase(),
    day: String(d),
  };
}

export function formatWeekdayLabel(dayKey: string): string {
  if (!dayKey) return "";
  const [y, m, d] = dayKey.split("-").map(Number);
  if (!y || !m || !d) return "";
  return new Date(y, m - 1, d)
    .toLocaleDateString(undefined, { weekday: "long" })
    .toUpperCase();
}

export function formatDayYearLabel(dayKey: string): string {
  if (!dayKey) return "";
  return dayKey.split("-")[0] ?? "";
}

export function formatMonthHeroLabel(monthKey: string): string {
  if (!monthKey) return "";
  const [y, m] = monthKey.split("-").map(Number);
  if (!y || !m) return "";
  return new Date(y, m - 1, 1)
    .toLocaleDateString(undefined, { month: "short" })
    .toUpperCase();
}

export function formatMonthFullLabel(monthKey: string): string {
  if (!monthKey) return "";
  const [y, m] = monthKey.split("-").map(Number);
  if (!y || !m) return "";
  return new Date(y, m - 1, 1)
    .toLocaleDateString(undefined, { month: "long" })
    .toUpperCase();
}

export function formatMonthYearLabel(monthKey: string): string {
  if (!monthKey) return "";
  return monthKey.split("-")[0] ?? "";
}

export function groupSessionsByDay(sessions: SessionRow[]): DayGroup[] {
  const map = new Map<string, SessionRow[]>();
  for (const session of sessions) {
    const key = toDayKey(session.created_at);
    const existing = map.get(key);
    if (existing) {
      existing.push(session);
    } else {
      map.set(key, [session]);
    }
  }
  return Array.from(map.entries())
    .map(([dayKey, daySessions]) => ({
      dayKey,
      dayLabel: formatDayLabel(dayKey),
      latestTimestamp: daySessions[0].created_at,
      sessions: daySessions.sort((a, b) => b.created_at - a.created_at),
    }))
    .sort((a, b) => b.latestTimestamp - a.latestTimestamp);
}

export function groupDaysByMonth(dayGroups: DayGroup[]): MonthGroup[] {
  const map = new Map<string, MonthGroup>();
  for (const day of dayGroups) {
    const monthKey = toMonthKey(day.latestTimestamp);
    const existing = map.get(monthKey);
    if (existing) {
      existing.days.push(day);
      existing.totalSessions += day.sessions.length;
    } else {
      map.set(monthKey, {
        monthKey,
        monthLabel: formatMonthLabel(monthKey),
        days: [day],
        totalSessions: day.sessions.length,
      });
    }
  }
  return Array.from(map.values()).sort((a, b) =>
    b.monthKey.localeCompare(a.monthKey)
  );
}

export function chunkSessionsIntoWindows(
  sessions: SessionRow[],
  maxPerWindow: number
): SessionWindow[] {
  if (sessions.length === 0 || maxPerWindow <= 0) return [];
  const sorted = [...sessions].sort((a, b) => b.created_at - a.created_at);
  const windows: SessionWindow[] = [];
  for (let i = 0; i < sorted.length; i += maxPerWindow) {
    const slice = sorted.slice(i, i + maxPerWindow);
    const oldest = slice[slice.length - 1];
    const newest = slice[0];
    windows.push({
      sessions: slice,
      startMs: newest.created_at,
      endMs: oldest.created_at,
      label: `${formatClockTime(oldest.created_at)} – ${formatClockTime(newest.created_at)}`,
      dateSpanLabel: formatCardDate(newest.created_at),
    });
  }
  return windows;
}

export function chunkDaysIntoWindows(
  days: DayGroup[],
  maxPerWindow: number
): MonthWindow[] {
  if (days.length === 0 || maxPerWindow <= 0) return [];
  const windows: MonthWindow[] = [];
  for (let i = 0; i < days.length; i += maxPerWindow) {
    const slice = days.slice(i, i + maxPerWindow);
    const dayNumbers = slice.map((d) => Number(d.dayKey.split("-")[2])).sort((a, b) => a - b);
    const min = dayNumbers[0];
    const max = dayNumbers[dayNumbers.length - 1];
    windows.push({
      days: slice,
      label: min === max ? String(min) : `${min}–${max}`,
    });
  }
  return windows;
}
