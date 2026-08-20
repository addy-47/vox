import { describe, it, expect } from "vitest";
import {
  ORBIT_CAPACITY_MAX,
  ORBIT_CAPACITY_MIN,
  ORBIT_CARD_WIDTH,
  ORBIT_RADIUS_MAX,
  ORBIT_RADIUS_MIN,
  ORBIT_TILT_COMPRESSION,
  ORBIT_Z_FRONT_MIN,
  ORBIT_Z_SELECTED,
  chunkDaysIntoWindows,
  chunkSessionsIntoWindows,
  dayNumberFromKey,
  dayToDialAngle,
  daysInMonthKey,
  depthFromAngle,
  dialDegrees,
  dialDotRadius,
  distributeAngles,
  ellipsePoint,
  formatDayHeroLabel,
  formatDayLabel,
  formatDayShortLabel,
  formatDayYearLabel,
  formatDuration,
  formatMonthHeroLabel,
  formatMonthLabel,
  formatMonthShortLabel,
  formatMonthYearLabel,
  formatTimeRange,
  groupDaysByMonth,
  groupSessionsByDay,
  orbitCapacityFor,
  ringRadiusFor,
  timeToDialAngle,
  toDayKey,
  toMonthKey,
  zIndexForAngle,
} from "@/shared/components/history/orbitMath";
import type { SessionRow } from "@/services/historyService";

function session(id: number, startedAt: number): SessionRow {
  return {
    id,
    started_at: startedAt,
    ended_at: null,
    turn_count: 2,
    first_message: `msg ${id}`,
  };
}

// Local-time helpers (test runner shares the machine TZ)
function ts(y: number, m: number, d: number, h = 12, min = 0): number {
  return new Date(y, m - 1, d, h, min).getTime();
}

describe("ellipse geometry — single tilted ring", () => {
  it("places angle 0 on the right and π/2 at the bottom (front)", () => {
    const right = ellipsePoint(0, 100);
    const bottom = ellipsePoint(Math.PI / 2, 100);
    expect(right.x).toBeCloseTo(100);
    expect(right.y).toBeCloseTo(0);
    expect(bottom.x).toBeCloseTo(0);
    expect(bottom.y).toBeCloseTo(100 * ORBIT_TILT_COMPRESSION);
  });

  it("mirrors the ring across the horizontal axis", () => {
    const top = ellipsePoint(-Math.PI / 2, 100);
    const bottom = ellipsePoint(Math.PI / 2, 100);
    expect(top.y).toBeCloseTo(-bottom.y);
    expect(top.x).toBeCloseTo(bottom.x);
  });
});

describe("depthFromAngle", () => {
  it("maps front (bottom) to 1, back (top) to 0, sides to 0.5", () => {
    expect(depthFromAngle(Math.PI / 2)).toBe(1);
    expect(depthFromAngle(-Math.PI / 2)).toBe(0);
    expect(depthFromAngle(0)).toBeCloseTo(0.5);
    expect(depthFromAngle(Math.PI)).toBeCloseTo(0.5);
  });

  it("clamps out-of-range angles", () => {
    expect(depthFromAngle(0)).toBeGreaterThanOrEqual(0);
    expect(depthFromAngle(0)).toBeLessThanOrEqual(1);
  });
});

describe("zIndexForAngle — occlusion bands around the clock", () => {
  it("keeps back-half cards behind the clock and front-half above it", () => {
    expect(zIndexForAngle(-Math.PI / 2, false)).toBeLessThan(50);
    expect(zIndexForAngle(Math.PI / 2, false)).toBeGreaterThan(50);
  });

  it("binds bands to their ceilings", () => {
    expect(zIndexForAngle(-Math.PI / 2, false)).toBe(12);
    expect(zIndexForAngle(Math.PI / 2, false)).toBe(78);
    expect(zIndexForAngle(0, false)).toBeGreaterThanOrEqual(12);
  });

  it("always promotes the selected card above everything", () => {
    expect(zIndexForAngle(-Math.PI / 2, true)).toBe(ORBIT_Z_SELECTED);
    expect(zIndexForAngle(Math.PI / 2, true)).toBe(ORBIT_Z_SELECTED);
  });

  it("never collides with the clock band (50)", () => {
    const back = zIndexForAngle(-Math.PI / 2, false);
    const front = zIndexForAngle(Math.PI / 2, false);
    expect(back).toBeLessThan(ORBIT_Z_FRONT_MIN);
    expect(front).toBeGreaterThan(50);
  });
});

describe("ring sizing & capacity — dynamic by viewport", () => {
  it("scales the radius with the viewport dimensions", () => {
    expect(ringRadiusFor(1200, 800)).toBeGreaterThanOrEqual(ORBIT_RADIUS_MIN);
    expect(ringRadiusFor(1200, 800)).toBeLessThanOrEqual(ORBIT_RADIUS_MAX);
    expect(ringRadiusFor(680, 900)).toBeGreaterThanOrEqual(ORBIT_RADIUS_MIN);
  });

  it("clamps the radius to the configured bounds", () => {
    expect(ringRadiusFor(400, 400)).toBe(ORBIT_RADIUS_MIN);
    expect(ringRadiusFor(3000, 3000)).toBe(ORBIT_RADIUS_MAX);
  });

  it("computes capacity from the ring circumference", () => {
    const cap = orbitCapacityFor(1200, 800);
    expect(cap).toBeGreaterThanOrEqual(ORBIT_CAPACITY_MIN);
    expect(cap).toBeLessThanOrEqual(ORBIT_CAPACITY_MAX);
  });

  it("stays within the 6-12 card bounds across viewports", () => {
    for (const [w, h] of [
      [680, 500],
      [700, 800],
      [900, 900],
      [1600, 1000],
      [1920, 1080],
    ]) {
      const cap = orbitCapacityFor(w, h);
      expect(cap).toBeGreaterThanOrEqual(ORBIT_CAPACITY_MIN);
      expect(cap).toBeLessThanOrEqual(ORBIT_CAPACITY_MAX);
    }
  });

  it("uses the passed card width when computing capacity", () => {
    const narrow = orbitCapacityFor(1200, 800, 160);
    const wide = orbitCapacityFor(1200, 800, ORBIT_CARD_WIDTH);
    expect(narrow).toBeGreaterThanOrEqual(wide);
  });
});

describe("distributeAngles — newest at the front", () => {
  it("places the newest card at the front (bottom) of the ring", () => {
    const angles = distributeAngles(4);
    expect(angles[0]).toBeCloseTo(Math.PI / 2);
  });

  it("spreads cards evenly around the ring", () => {
    const angles = distributeAngles(8);
    for (let i = 1; i < angles.length; i++) {
      expect(angles[i - 1] - angles[i]).toBeCloseTo((2 * Math.PI) / 8);
    }
  });
});

describe("window model — sessions", () => {
  it("keeps a small day in a single window", () => {
    const sessions = [
      session(1, ts(2025, 8, 16, 9)),
      session(2, ts(2025, 8, 16, 14)),
    ];
    const windows = chunkSessionsIntoWindows(sessions, 10);
    expect(windows).toHaveLength(1);
    expect(windows[0].sessions.map((s) => s.id)).toEqual([2, 1]);
  });

  it("chunks a heavy day into bounded windows in time order", () => {
    const sessions = Array.from({ length: 25 }, (_, i) => session(i, ts(2025, 8, 16, 8 + i)));
    const windows = chunkSessionsIntoWindows(sessions, 10);
    expect(windows).toHaveLength(3);
    expect(windows.map((w) => w.sessions.length)).toEqual([10, 10, 5]);
    expect(windows[0].sessions[0].id).toBe(24);
    expect(windows[0].sessions[9].id).toBe(15);
    expect(windows[2].sessions[4].id).toBe(0);
  });

  it("labels windows by the actual hour range they cover", () => {
    const sessions = [
      session(1, ts(2025, 8, 16, 9, 12)),
      session(2, ts(2025, 8, 16, 11, 48)),
    ];
    const windows = chunkSessionsIntoWindows(sessions, 10);
    expect(windows[0].label).toBe("09:12 – 11:48");
  });

  it("uses a single time label for a one-session window", () => {
    const windows = chunkSessionsIntoWindows([session(1, ts(2025, 8, 16, 7, 30))], 10);
    expect(windows[0].label).toBe("07:30");
  });

  it("returns an empty array for no sessions or a zero cap", () => {
    expect(chunkSessionsIntoWindows([], 10)).toEqual([]);
    expect(chunkSessionsIntoWindows([session(1, 0)], 0)).toEqual([]);
  });
});

describe("window model — month days", () => {
  const makeDay = (key: string): import("@/shared/components/history/orbitMath").DayGroup => ({
    dayKey: key,
    dayLabel: key,
    latestTimestamp: ts(2026, 8, Number(key.split("-")[2])),
    sessions: [session(1, ts(2026, 8, Number(key.split("-")[2])))],
  });

  it("labels windows by day-number range", () => {
    const days = [
      makeDay("2026-08-01"),
      makeDay("2026-08-02"),
      makeDay("2026-08-13"),
    ];
    const windows = chunkDaysIntoWindows(days, 2);
    expect(windows).toHaveLength(2);
    expect(windows[0].label).toBe("1–2");
    expect(windows[1].label).toBe("13");
  });

  it("chunks a full month into bounded windows", () => {
    const days = Array.from({ length: 31 }, (_, i) => makeDay(`2026-08-${String(i + 1).padStart(2, "0")}`));
    const windows = chunkDaysIntoWindows(days, 12);
    expect(windows.map((w) => w.days.length)).toEqual([12, 12, 7]);
    expect(windows[0].label).toBe("1–12");
    expect(windows[1].label).toBe("13–24");
    expect(windows[2].label).toBe("25–31");
  });

  it("derives day numbers and month lengths deterministically", () => {
    expect(dayNumberFromKey("2026-08-16")).toBe(16);
    expect(daysInMonthKey("2026-02")).toBe(28);
    expect(daysInMonthKey("2026-08")).toBe(31);
    expect(daysInMonthKey("2026-12")).toBe(31);
  });
});

describe("formatTimeRange & formatDuration", () => {
  it("formats a range and a single time", () => {
    expect(formatTimeRange(ts(2025, 8, 16, 7, 12), ts(2025, 8, 16, 11, 48))).toBe("07:12 – 11:48");
    expect(formatTimeRange(ts(2025, 8, 16, 7, 12), ts(2025, 8, 16, 7, 12))).toBe("07:12");
  });

  it("formats durations human-readably", () => {
    expect(formatDuration(45_000)).toBe("45s");
    expect(formatDuration(12 * 60_000)).toBe("12m");
    expect(formatDuration(65 * 60_000)).toBe("1h 05m");
    expect(formatDuration(0)).toBe("1s");
    expect(formatDuration(-5)).toBe("");
  });
});

describe("voice print dial", () => {
  it("maps midnight to the top of the dial", () => {
    expect(timeToDialAngle(ts(2025, 8, 16, 0, 0))).toBeCloseTo(0);
  });

  it("maps noon to the bottom of the dial", () => {
    expect(timeToDialAngle(ts(2025, 8, 16, 12, 0))).toBeCloseTo(Math.PI);
  });

  it("maps the first day to the top and the last day just before it", () => {
    expect(dayToDialAngle(1, 31)).toBeCloseTo(0);
    expect(dayToDialAngle(31, 31)).toBeCloseTo((2 * Math.PI * 30) / 31);
  });

  it("grows dot size with turn count, bounded", () => {
    expect(dialDotRadius(0)).toBe(2);
    expect(dialDotRadius(100)).toBe(5.5);
    expect(dialDotRadius(5)).toBeGreaterThan(dialDotRadius(1));
  });

  it("converts radians to SVG degrees", () => {
    expect(dialDegrees(Math.PI)).toBe(180);
    expect(dialDegrees(0)).toBe(0);
  });
});

describe("date grouping — locale-independent keys", () => {
  const sessions = [
    session(1, ts(2025, 8, 16, 9)),
    session(2, ts(2025, 8, 16, 14)),
    session(3, ts(2025, 7, 31, 8)),
  ];

  it("groups by YYYY-MM-DD keys with newest day first", () => {
    const groups = groupSessionsByDay(sessions);
    expect(groups).toHaveLength(2);
    expect(groups[0].dayKey).toBe("2025-08-16");
    expect(groups[1].dayKey).toBe("2025-07-31");
    expect(groups[0].sessions.map((s) => s.id)).toEqual([2, 1]);
  });

  it("groups days into months with totals", () => {
    const months = groupDaysByMonth(groupSessionsByDay(sessions));
    expect(months).toHaveLength(2);
    expect(months[0].monthKey).toBe("2025-08");
    expect(months[0].totalSessions).toBe(2);
    expect(months[1].monthKey).toBe("2025-07");
  });

  it("formats labels deterministically from keys", () => {
    expect(toDayKey(ts(2025, 8, 16))).toBe("2025-08-16");
    expect(toMonthKey(ts(2025, 8, 16))).toBe("2025-08");
    expect(formatDayLabel("2025-08-16")).toMatch(/Aug/);
    expect(formatDayLabel("2025-08-16")).toContain("2025");
    expect(formatDayLabel("2025-08-16")).toContain("16");
    expect(formatDayShortLabel("2025-08-16")).toMatch(/Aug/);
    expect(formatDayShortLabel("2025-08-16")).toContain("16");
    expect(formatMonthLabel("2025-08")).toMatch(/August/);
    expect(formatMonthLabel("2025-08")).toContain("2025");
    expect(formatMonthShortLabel("2025-08")).toMatch(/Aug/);
  });

  it("formats clock hero labels", () => {
    expect(formatDayHeroLabel("2025-08-16")).toBe("AUG 16");
    expect(formatDayYearLabel("2025-08-16")).toBe("2025");
    expect(formatMonthHeroLabel("2025-08")).toBe("AUG");
    expect(formatMonthYearLabel("2025-08")).toBe("2025");
  });

  it("round-trips padded day/month keys", () => {
    expect(toDayKey(ts(2025, 1, 3))).toBe("2025-01-03");
    expect(toMonthKey(ts(2025, 12, 3))).toBe("2025-12");
  });
});
