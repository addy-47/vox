import type { SessionRow } from "@/services/historyService";

// ─── Single-Ring Carousel Layout Constants ───────────────────────────────────
// One tilted ring, rendered as a CSS ellipse; cards travel along it like an
// infinite circular carousel. Angle 0 = right side, +π/2 = bottom (front).
// The ring never shrinks below the disc clearance; it scales with the viewport
// instead of clipping at page edges.

/** Vertical compression of the ring circle (wide perspective tilt). */
export const ORBIT_TILT_COMPRESSION = 0.42;

/** Width of orbit cards in px — mirrored by VoiceRippleNode / MonthDayCard. */
export const ORBIT_CARD_WIDTH = 220;

/** Minimum ring radius in px — keeps cards clear of the enlarged hub. */
export const ORBIT_RADIUS_MIN = 360;

/** Maximum ring radius in px — fills wide desktop screens gracefully. */
export const ORBIT_RADIUS_MAX = 720;

/** Page margin subtracted from the half-min-dimension when sizing the ring. */
export const ORBIT_RADIUS_MARGIN = 20;

/** Card angular clearance — circumference slot = card width × this factor. */
export const ORBIT_SLOT_FACTOR = 1.15;

/** Session/day cards per window (orbit capacity bounds). */
export const ORBIT_CAPACITY_MIN = 6;
export const ORBIT_CAPACITY_MAX = 12;

/** Depth attenuation range for projected cards (back → front). */
export const ORBIT_CARD_SCALE_MIN = 0.62;
export const ORBIT_CARD_SCALE_MAX = 1.16;
export const ORBIT_CARD_OPACITY_MIN = 0.35;

/** Scale multiplier applied to the selected card. */
export const ORBIT_CARD_SELECTED_BOOST = 1.10;

/** Z-banding — back-half cards slide behind the central clock (z-50). */
export const ORBIT_Z_BACK_MAX = 39;
export const ORBIT_Z_CLOCK = 50;
export const ORBIT_Z_FRONT_MIN = 51;
export const ORBIT_Z_SELECTED = 85;

/** Faint concentric guide ring offset outside the solid ring. */
export const ORBIT_GUIDE_GAP = 36;
export const ORBIT_GUIDE_OPACITY = 0.12;

// ─── Ring Geometry ───────────────────────────────────────────────────────────

export interface RingPoint {
  x: number;
  y: number;
}

/** Point on the tilted ring, centered at (0, 0), +y = bottom (front). */
export function ellipsePoint(angle: number, radius: number): RingPoint {
  return {
    x: Math.cos(angle) * radius,
    y: Math.sin(angle) * radius * ORBIT_TILT_COMPRESSION,
  };
}

/** 0 (top-back) → 1 (bottom-front) from a card's angle on the ring. */
export function depthFromAngle(angle: number): number {
  return clamp01((Math.sin(angle) + 1) / 2);
}

/**
 * Stacking band for a card at a given angle. Quantized to discrete bands
 * to prevent constant compositor layer re-sorting during rotation.
 * Back-half cards render behind the central clock (z-50); front-half cards
 * render above it; the selected card always wins (85).
 */
export function zIndexForAngle(angle: number, isSelected: boolean): number {
  if (isSelected) return ORBIT_Z_SELECTED;
  const depth = depthFromAngle(angle);
  if (depth < 0.2) return 12;
  if (depth < 0.35) return 24;
  if (depth < 0.5) return 38;
  if (depth < 0.65) return 55;
  if (depth < 0.85) return 68;
  return 78;
}

/** Ring radius for a viewport — leverages width for a wide panoramic orbit, clamped. */
export function ringRadiusFor(width: number, height: number): number {
  const horizontalBase = width * 0.46;
  const verticalBase = (height * 0.54) / ORBIT_TILT_COMPRESSION;
  const base = Math.min(horizontalBase, verticalBase) - ORBIT_RADIUS_MARGIN;
  return clamp(
    base,
    ORBIT_RADIUS_MIN,
    ORBIT_RADIUS_MAX
  );
}

/** Max cards that fit around the ring without colliding, clamped to bounds. */
export function orbitCapacityFor(
  width: number,
  height: number,
  cardWidth: number = ORBIT_CARD_WIDTH
): number {
  const radius = ringRadiusFor(width, height);
  const slots = (2 * Math.PI * radius) / (cardWidth * ORBIT_SLOT_FACTOR);
  return clamp(Math.floor(slots), ORBIT_CAPACITY_MIN, ORBIT_CAPACITY_MAX);
}

/**
 * Deterministic angle for the i-th card (newest first): the newest card sits
 * at the front (π/2, bottom) and older cards follow counter-clockwise.
 */
export function distributeAngles(count: number): number[] {
  return Array.from(
    { length: count },
    (_, i) => Math.PI / 2 - (i * Math.PI * 2) / count
  );
}

// ─── Window Model ────────────────────────────────────────────────────────────
// Days/months with more sessions than the orbit can hold are chunked into
// windows of at most `capacity` items. Windows are labeled by the actual
// hour-range (sessions) or day-range (month) they cover.

export interface SessionWindow {
  sessions: SessionRow[];
  /** Mono label, e.g. "07:12 – 11:48". */
  label: string;
  /** Newest session timestamp in the window. */
  startMs: number;
  /** Oldest session timestamp in the window. */
  endMs: number;
}

export interface MonthWindow {
  days: DayGroup[];
  /** Mono label, e.g. "1–12". */
  label: string;
}

/** Chunks newest-first sessions into bounded windows, labeled by time range. */
export function chunkSessionsIntoWindows(
  sessions: SessionRow[],
  maxPerWindow: number
): SessionWindow[] {
  if (sessions.length === 0 || maxPerWindow <= 0) return [];
  const sorted = [...sessions].sort((a, b) => b.started_at - a.started_at);
  const windows: SessionWindow[] = [];
  for (let i = 0; i < sorted.length; i += maxPerWindow) {
    const slice = sorted.slice(i, i + maxPerWindow);
    const oldest = slice[slice.length - 1];
    const newest = slice[0];
    windows.push({
      sessions: slice,
      startMs: newest.started_at,
      endMs: oldest.started_at,
      label: formatTimeRange(oldest.started_at, newest.started_at),
    });
  }
  return windows;
}

/** "07:12 – 11:48", or a single time when both timestamps are the same hour. */
export function formatTimeRange(startMs: number, endMs: number): string {
  const start = formatClockTime(startMs);
  const end = formatClockTime(endMs);
  return start === end ? start : `${start} – ${end}`;
}

/** Chunks month days into bounded windows, labeled by day-number range. */
export function chunkDaysIntoWindows(
  days: DayGroup[],
  maxPerWindow: number
): MonthWindow[] {
  if (days.length === 0 || maxPerWindow <= 0) return [];
  const windows: MonthWindow[] = [];
  for (let i = 0; i < days.length; i += maxPerWindow) {
    const slice = days.slice(i, i + maxPerWindow);
    windows.push({
      days: slice,
      label: formatDayRangeLabel(slice),
    });
  }
  return windows;
}

/** "1–12" (single day → "12"). Sorted chronologically so range is always min–max. */
export function formatDayRangeLabel(days: DayGroup[]): string {
  if (days.length === 0) return "";
  const dayNumbers = days.map((d) => dayNumberFromKey(d.dayKey)).sort((a, b) => a - b);
  const min = dayNumbers[0];
  const max = dayNumbers[dayNumbers.length - 1];
  return min === max ? String(min) : `${min}–${max}`;
}

export function dayNumberFromKey(dayKey: string): number {
  return Number(dayKey.split("-")[2]);
}

/** Number of days in a month key ("2026-02" → 28). */
export function daysInMonthKey(monthKey: string): number {
  const [y, m] = monthKey.split("-").map(Number);
  return new Date(y, m, 0).getDate();
}

// ─── Voice Print Dial ────────────────────────────────────────────────────────
// A static ring of dots between the clock and the orbit: one dot per session
// (day view) at its clock position, or per active day (month view). Dot size
// encodes turn count.

/** Angle on a 24h dial (0 = midnight, top of the dial). */
export function timeToDialAngle(ms: number): number {
  const d = new Date(ms);
  return ((d.getHours() * 60 + d.getMinutes()) / (24 * 60)) * Math.PI * 2;
}

/** Angle on a month dial (0 = first day, top of the dial). */
export function dayToDialAngle(dayNumber: number, totalDays: number): number {
  return ((dayNumber - 1) / totalDays) * Math.PI * 2;
}

/** SVG rotation degrees for a dial angle (clockwise from 12 o'clock). */
export function dialDegrees(angle: number): number {
  return (angle * 180) / Math.PI;
}

/** Dot radius from turn count, bounded so heavy days never swamp the dial. */
export function dialDotRadius(turnCount: number): number {
  return clamp(2 + turnCount * 0.3, 2, 5.5);
}

// ─── Formatting ──────────────────────────────────────────────────────────────

/** "45s" / "12m" / "1h 05m" from a duration in ms. */
export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "";
  const totalSeconds = Math.max(1, Math.round(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}h ${String(minutes).padStart(2, "0")}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${seconds}s`;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function clamp01(value: number): number {
  return clamp(value, 0, 1);
}

// ─── Date Grouping (locale-independent keys) ─────────────────────────────────

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

export function formatDayLabel(dayKey: string): string {
  const [y, m, d] = dayKey.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

export function formatDayShortLabel(dayKey: string): string {
  const [y, m, d] = dayKey.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function formatMonthLabel(monthKey: string): string {
  const [y, m] = monthKey.split("-").map(Number);
  return new Date(y, m - 1, 1).toLocaleDateString(undefined, {
    month: "long",
    year: "numeric",
  });
}

export function formatMonthShortLabel(monthKey: string): string {
  const [y, m] = monthKey.split("-").map(Number);
  return new Date(y, m - 1, 1).toLocaleDateString(undefined, {
    month: "short",
    year: "numeric",
  });
}

export function formatClockTime(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

/** Day-view clock hero, e.g. "AUG 16". */
export function formatDayHeroLabel(dayKey: string): string {
  const [y, m, d] = dayKey.split("-").map(Number);
  const monthShort = new Date(y, m - 1, 1).toLocaleDateString(undefined, {
    month: "short",
  });
  return `${monthShort.toUpperCase()} ${d}`;
}

/** Returns separated { month: "AUG", day: "12" } for dual-tone styling. */
export function formatDayHeroParts(dayKey: string): { month: string; day: string } {
  if (!dayKey) return { month: "", day: "" };
  const [y, m, d] = dayKey.split("-").map(Number);
  const monthShort = new Date(y, m - 1, 1).toLocaleDateString(undefined, {
    month: "short",
  });
  return {
    month: monthShort.toUpperCase(),
    day: String(d),
  };
}

/** Returns uppercase full weekday e.g. "TUESDAY". */
export function formatWeekdayLabel(dayKey: string): string {
  if (!dayKey) return "";
  const [y, m, d] = dayKey.split("-").map(Number);
  return new Date(y, m - 1, d)
    .toLocaleDateString(undefined, { weekday: "long" })
    .toUpperCase();
}

/** Day-view clock secondary line, e.g. "2025". */
export function formatDayYearLabel(dayKey: string): string {
  return dayKey.split("-")[0];
}

/** Month-view clock hero, e.g. "AUG". */
export function formatMonthHeroLabel(monthKey: string): string {
  const [y, m] = monthKey.split("-").map(Number);
  return new Date(y, m - 1, 1)
    .toLocaleDateString(undefined, { month: "short" })
    .toUpperCase();
}

/** Month-view full month name, e.g. "AUGUST". */
export function formatMonthFullLabel(monthKey: string): string {
  if (!monthKey) return "";
  const [y, m] = monthKey.split("-").map(Number);
  return new Date(y, m - 1, 1)
    .toLocaleDateString(undefined, { month: "long" })
    .toUpperCase();
}

/** Month-view clock secondary line, e.g. "2026". */
export function formatMonthYearLabel(monthKey: string): string {
  return monthKey.split("-")[0];
}

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

/** Groups sessions by calendar day, newest day first, sessions newest-first within each day. */
export function groupSessionsByDay(sessions: SessionRow[]): DayGroup[] {
  const map = new Map<string, SessionRow[]>();
  for (const session of sessions) {
    const key = toDayKey(session.started_at);
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
      latestTimestamp: daySessions[0].started_at,
      sessions: daySessions.sort((a, b) => b.started_at - a.started_at),
    }))
    .sort((a, b) => b.latestTimestamp - a.latestTimestamp);
}

/** Groups day groups into months, newest month first. */
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
