import { describe, it, expect } from "vitest";
import {
  formatDayLabel,
  formatMonthLabel,
  formatMonthShortLabel,
  formatDayHeroLabel,
  formatDayYearLabel,
  formatMonthHeroLabel,
  formatMonthFullLabel,
  formatMonthYearLabel,
  formatClockTime,
  groupSessionsByDay,
  groupDaysByMonth,
  chunkDaysIntoWindows,
  chunkSessionsIntoWindows,
} from "@/shared/components/home/recall";

describe("Recall date formatters empty state & null-safety regressions", () => {
  describe("formatters with empty/malformed inputs", () => {
    it("handles empty strings safely without throwing or NaN", () => {
      expect(formatDayLabel("")).toBe("");
      expect(formatMonthLabel("")).toBe("");
      expect(formatMonthShortLabel("")).toBe("");
      expect(formatClockTime(0)).toBe("");
      expect(formatDayHeroLabel("")).toBe("");
      expect(formatDayYearLabel("")).toBe("");
      expect(formatMonthHeroLabel("")).toBe("");
      expect(formatMonthFullLabel("")).toBe("");
      expect(formatMonthYearLabel("")).toBe("");
    });

    it("handles malformed string keys safely", () => {
      expect(formatDayLabel("invalid")).toBe("");
      expect(formatMonthLabel("invalid")).toBe("");
      expect(formatMonthHeroLabel("invalid")).toBe("");
    });
  });

  describe("chunking and grouping with empty collections", () => {
    it("returns empty arrays instead of throwing or producing undefined groups", () => {
      const days = groupSessionsByDay([]);
      expect(days).toEqual([]);

      const months = groupDaysByMonth(days);
      expect(months).toEqual([]);

      const dayWindows = chunkDaysIntoWindows([], 12);
      expect(dayWindows).toEqual([]);

      const sessionWindows = chunkSessionsIntoWindows([], 12);
      expect(sessionWindows).toEqual([]);
    });
  });
});
