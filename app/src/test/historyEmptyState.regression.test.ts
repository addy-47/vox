import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import React from "react";
import { MemoryRouter } from "react-router-dom";
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
} from "@/shared/components/history";
import { useHistory } from "@/shared/hooks/useHistory";
import * as persistenceService from "@/services/persistenceService";
import * as eventsService from "@/services/eventsService";

describe("History empty state & null-safety regressions", () => {
  beforeEach(() => {
    vi.spyOn(persistenceService, "getSessions").mockResolvedValue([]);
    vi.spyOn(persistenceService, "getCachedSessions").mockReturnValue([]);
    vi.spyOn(eventsService, "onSessionsChanged").mockReturnValue(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

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

  describe("useHistory hook initial and empty render", () => {
    it("provides stable empty objects for currentMonthGroup and windows when no sessions exist", () => {
      const wrapper: React.FC<{ children: React.ReactNode }> = ({ children }) =>
        React.createElement(MemoryRouter, null, children);

      const { result } = renderHook(() => useHistory(), { wrapper });

      expect(result.current.currentMonthGroup).toBeDefined();
      expect(result.current.currentMonthGroup.days).toBeDefined();
      expect(Array.isArray(result.current.currentMonthGroup.days)).toBe(true);
      expect(result.current.currentMonthGroup.days.length).toBe(0);

      expect(result.current.currentMonthWindow).toBeDefined();
      expect(result.current.currentMonthWindow.days).toBeDefined();
      expect(Array.isArray(result.current.currentMonthWindow.days)).toBe(true);

      expect(result.current.currentWindowSessions).toBeDefined();
      expect(Array.isArray(result.current.currentWindowSessions)).toBe(true);
      expect(result.current.currentWindowSessions.length).toBe(0);
    });
  });
});
