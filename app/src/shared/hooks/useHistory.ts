import { useState, useEffect, useLayoutEffect, useCallback, useRef, useMemo } from "react";
import { useLocation } from "react-router-dom";
import {
  getSessions,
  getCachedSessions,
  getTurns,
  deleteSession,
  formatDateTime,
  sortSessionsNewestFirst,
  type SessionRow,
  type TurnRow,
} from "@/services/historyService";
import { onSessionsChanged } from "@/services/eventsService";
import { BREAKPOINT_COMPACT_HEIGHT_MAX, BREAKPOINT_COMPACT_MAX } from "@/layout/breakpoints";
import {
  chunkDaysIntoWindows,
  chunkSessionsIntoWindows,
  groupSessionsByDay,
  groupDaysByMonth,
  formatDayHeroLabel,
  formatDayYearLabel,
  formatMonthHeroLabel,
  formatMonthShortLabel,
  formatMonthYearLabel,
  formatDayHeroParts,
  formatWeekdayLabel,
  formatMonthFullLabel,
  orbitCapacityFor,
  ringRadiusFor,
  type DayGroup,
  type HistoryView,
} from "@/shared/components/history";
import { HISTORY_COPY } from "@/data/historyCopy";

const EMPTY_DAY_GROUP: DayGroup = {
  dayKey: "",
  dayLabel: formatDateTime(0),
  latestTimestamp: 0,
  sessions: [],
};

function getErrorMessage(e: unknown, fallback: string): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  if (
    e &&
    typeof e === "object" &&
    "message" in e &&
    typeof (e as { message: unknown }).message === "string"
  ) {
    return (e as { message: string }).message;
  }
  return fallback;
}

export function useHistory() {
  const location = useLocation();
  const [sessions, setSessions] = useState<SessionRow[]>(() => {
    const cached = getCachedSessions();
    return cached ? sortSessionsNewestFirst(cached) : [];
  });
  const [sessionsLoading, setSessionsLoading] = useState(() => !getCachedSessions());
  const [error, setError] = useState<string | null>(null);
  const [selectedSession, setSelectedSession] = useState<SessionRow | null>(null);
  const selectedSessionRef = useRef<SessionRow | null>(selectedSession);
  selectedSessionRef.current = selectedSession;
  const [turns, setTurns] = useState<TurnRow[]>([]);
  const [turnsLoading, setTurnsLoading] = useState(false);
  const [turnsError, setTurnsError] = useState<string | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null);
  const deleteTimerRef = useRef<NodeJS.Timeout | null>(null);

  // View state
  const [view, setView] = useState<HistoryView>("day");
  const [dateIndex, setDateIndex] = useState(0);
  const [monthIndex, setMonthIndex] = useState(0);
  const [dayWindowIndex, setDayWindowIndex] = useState(0);
  const [monthWindowIndex, setMonthWindowIndex] = useState(0);
  const dragMovedRef = useRef(false);

  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState(() => ({
    width: typeof window !== "undefined" ? window.innerWidth : BREAKPOINT_COMPACT_MAX,
    height: typeof window !== "undefined" ? window.innerHeight : 800,
  }));

  // Immediate pre-paint measurement + debounced resize observer
  useLayoutEffect(() => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      setDimensions({
        width: rect.width,
        height: rect.height,
      });
    }

    let timer: NodeJS.Timeout;
    const observer = new ResizeObserver((entries) => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        for (const entry of entries) {
          setDimensions({
            width: entry.contentRect.width,
            height: entry.contentRect.height,
          });
        }
      }, 120);
    });
    observer.observe(containerRef.current);
    return () => {
      observer.disconnect();
      clearTimeout(timer);
    };
  }, []);

  const fetchSessions = useCallback(async () => {
    setError(null);
    try {
      const data = await getSessions();
      // Same ordering as every other session list (Home, Memory rail):
      // pinned first, then by last activity — not bare created_at.
      setSessions(sortSessionsNewestFirst(data));
    } catch (e: unknown) {
      console.error("Failed to fetch sessions:", e);
      setError(getErrorMessage(e, HISTORY_COPY.failedFallback));
    }
  }, []);

  const loadSessions = useCallback(async () => {
    if (!getCachedSessions()) {
      setSessionsLoading(true);
    }
    await fetchSessions();
    setSessionsLoading(false);
  }, [fetchSessions]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  useEffect(() => {
    const unlisten = onSessionsChanged(() => {
      fetchSessions();
    });
    return () => unlisten();
  }, [fetchSessions]);

  // Groupings
  const dayGroups = useMemo(() => groupSessionsByDay(sessions), [sessions]);
  const monthGroups = useMemo(() => groupDaysByMonth(dayGroups), [dayGroups]);

  const totalDates = Math.max(1, dayGroups.length);
  const totalMonths = Math.max(1, monthGroups.length);

  // Clamped indices without cascading effect loops
  const effectiveDateIndex = Math.min(dateIndex, totalDates - 1);
  const effectiveMonthIndex = Math.min(monthIndex, totalMonths - 1);

  const currentGroup = dayGroups[effectiveDateIndex] || EMPTY_DAY_GROUP;
  const currentDateSessions = currentGroup.sessions;
  const currentMonthGroup = monthGroups[effectiveMonthIndex];

  // Orbit sizing
  const capacity = useMemo(
    () => orbitCapacityFor(dimensions.width, dimensions.height),
    [dimensions.width, dimensions.height]
  );
  const ringRadius = useMemo(
    () => ringRadiusFor(dimensions.width, dimensions.height),
    [dimensions.width, dimensions.height]
  );

  // Session-quota based full orbit windows: distributes sessions across full 360 ring
  const sessionWindows = useMemo(
    () => chunkSessionsIntoWindows(sessions, capacity),
    [sessions, capacity]
  );
  const monthWindows = useMemo(
    () => chunkDaysIntoWindows(currentMonthGroup?.days ?? [], capacity),
    [currentMonthGroup, capacity]
  );

  // Sync selected session from router query params and ensure window + day view alignment
  useEffect(() => {
    if (sessions.length === 0) return;
    const params = new URLSearchParams(location.search);
    const targetSessionId = params.get("sessionId");
    if (targetSessionId) {
      const target = sessions.find((s) => s.id === Number(targetSessionId));
      if (target) {
        setSelectedSession(target);
        const winIdx = sessionWindows.findIndex((w) =>
          w.sessions.some((s) => s.id === target.id)
        );
        if (winIdx !== -1) {
          setDayWindowIndex(winIdx);
        }
        setView("day");
        return;
      }
    }
    // Keep currently selectedSession reference fresh if it exists in the updated sessions list
    if (selectedSessionRef.current) {
      const updated = sessions.find((s) => s.id === selectedSessionRef.current?.id);
      if (
        updated &&
        (updated.uncompacted_turns !== selectedSessionRef.current.uncompacted_turns ||
          updated.title !== selectedSessionRef.current.title ||
          updated.updated_at !== selectedSessionRef.current.updated_at)
      ) {
        setSelectedSession(updated);
      }
    }
  }, [sessions, location.search, sessionWindows]);

  const effectiveSessionWindowIndex = Math.min(
    dayWindowIndex,
    Math.max(0, sessionWindows.length - 1)
  );
  const effectiveMonthWindowIndex = Math.min(
    monthWindowIndex,
    Math.max(0, monthWindows.length - 1)
  );

  const currentWindow = sessionWindows[effectiveSessionWindowIndex];
  const currentWindowSessions = currentWindow?.sessions ?? [];
  const currentMonthWindow = monthWindows[effectiveMonthWindowIndex];

  const sessionById = useMemo(
    () => new Map(currentWindowSessions.map((s) => [String(s.id), s])),
    [currentWindowSessions]
  );

  const isCompactHeight = dimensions.height < BREAKPOINT_COMPACT_HEIGHT_MAX;

  // The orbit ring needs ~940px of stage (ORBIT_RADIUS_MIN + card half-width
  // at min scale); below the compact threshold it would clip, so the list
  // view owns every viewport under it. Single source: breakpoints.ts.
  const isOrbitViewport = dimensions.width >= BREAKPOINT_COMPACT_MAX;
  const effectiveView: HistoryView = isOrbitViewport ? view : "day";

  const openMonthOf = useCallback(
    (dayIdx: number) => {
      const day = dayGroups[dayIdx];
      if (!day) return;
      const monthIdx = monthGroups.findIndex(
        (m) => m.monthKey.slice(0, 7) === day.dayKey.slice(0, 7)
      );
      setMonthIndex(monthIdx === -1 ? 0 : monthIdx);
    },
    [dayGroups, monthGroups]
  );

  const handleViewChange = useCallback(
    (next: HistoryView) => {
      if (next === view) return;
      setSelectedSession(null);
      setDayWindowIndex(0);
      setMonthWindowIndex(0);
      if (next === "month") {
        openMonthOf(effectiveDateIndex);
      } else {
        const currentMonth = monthGroups[effectiveMonthIndex];
        const firstDayIdx = currentMonth
          ? dayGroups.findIndex((d) => d.dayKey.slice(0, 7) === currentMonth.monthKey)
          : -1;
        if (firstDayIdx !== -1) setDateIndex(firstDayIdx);
      }
      setView(next);
    },
    [view, effectiveDateIndex, effectiveMonthIndex, dayGroups, monthGroups, openMonthOf]
  );

  const handleDrillIntoDay = useCallback(
    (dayKey: string) => {
      const idx = dayGroups.findIndex((d) => d.dayKey === dayKey);
      if (idx === -1) return;
      setSelectedSession(null);
      setDateIndex(idx);
      setDayWindowIndex(0);
      setView("day");
    },
    [dayGroups]
  );

  // Turn fetching. Keyed on the session id alone: an in-place refresh of the
  // same session (new turn/uncompacted counts, title, timestamp) must not
  // re-enter the loading branch and blank the transcript.
  const selectedSessionId = selectedSession?.id ?? null;

  useEffect(() => {
    if (selectedSessionId === null) {
      setTurns([]);
      setTurnsError(null);
      return;
    }
    let isCancelled = false;
    const fetchTurns = async () => {
      setTurnsLoading(true);
      setTurnsError(null);
      try {
        const data = await getTurns(selectedSessionId);
        if (!isCancelled) {
          setTurns(data);
        }
      } catch (e: unknown) {
        console.error("Failed to fetch turns:", e);
        if (!isCancelled) {
          setTurnsError(getErrorMessage(e, "Failed to load session transcript turns."));
        }
      } finally {
        if (!isCancelled) {
          setTurnsLoading(false);
        }
      }
    };
    fetchTurns();

    return () => {
      isCancelled = true;
    };
  }, [selectedSessionId]);

  const retryFetchTurns = useCallback(() => {
    if (selectedSessionId === null) return;
    const targetSessionId = selectedSessionId;
    setTurnsLoading(true);
    setTurnsError(null);
    getTurns(targetSessionId)
      .then((data) => {
        if (selectedSessionRef.current?.id === targetSessionId) {
          setTurns(data);
        }
      })
      .catch((e: unknown) => {
        if (selectedSessionRef.current?.id === targetSessionId) {
          console.error("Failed to fetch turns:", e);
          setTurnsError(getErrorMessage(e, "Failed to load session transcript turns."));
        }
      })
      .finally(() => {
        if (selectedSessionRef.current?.id === targetSessionId) {
          setTurnsLoading(false);
        }
      });
  }, [selectedSessionId]);

  const handleDelete = useCallback(
    async (e: React.MouseEvent, id: number) => {
      e.stopPropagation();
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);

      if (confirmDeleteId === id) {
        try {
          await deleteSession(id);
          setConfirmDeleteId(null);
          setDeleteError(null);
          if (selectedSession?.id === id) setSelectedSession(null);
          fetchSessions();
        } catch (err: unknown) {
          console.error("Failed to delete session:", err);
          setDeleteError(getErrorMessage(err, HISTORY_COPY.deleteFailed));
        }
      } else {
        setConfirmDeleteId(id);
        deleteTimerRef.current = setTimeout(() => {
          setConfirmDeleteId(null);
        }, 3000);
      }
    },
    [confirmDeleteId, selectedSession, fetchSessions]
  );

  useEffect(() => {
    return () => {
      if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
    };
  }, []);

  useEffect(() => {
    if (!deleteError) return;
    const timer = setTimeout(() => setDeleteError(null), 5000);
    return () => clearTimeout(timer);
  }, [deleteError]);

  const handleCancelDelete = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    if (deleteTimerRef.current) clearTimeout(deleteTimerRef.current);
    setConfirmDeleteId(null);
  }, []);

  // Chevron navigation: Left (prev) goes back in time (older), Right (next) goes forward in time (newer)
  const handlePrevDate = useCallback(() => {
    setSelectedSession(null);
    if (effectiveSessionWindowIndex < sessionWindows.length - 1) {
      setDayWindowIndex(effectiveSessionWindowIndex + 1);
    }
  }, [effectiveSessionWindowIndex, sessionWindows.length]);

  const handleNextDate = useCallback(() => {
    setSelectedSession(null);
    if (effectiveSessionWindowIndex > 0) {
      setDayWindowIndex(effectiveSessionWindowIndex - 1);
    }
  }, [effectiveSessionWindowIndex]);

  const handleGoToday = useCallback(() => {
    setSelectedSession(null);
    setDayWindowIndex(0);
    setDateIndex(0);
  }, []);

  const handlePrevMonth = useCallback(() => {
    setSelectedSession(null);
    if (effectiveMonthWindowIndex < monthWindows.length - 1) {
      setMonthWindowIndex(effectiveMonthWindowIndex + 1);
    } else {
      setMonthWindowIndex(0);
      setMonthIndex((idx) => Math.min(totalMonths - 1, idx + 1));
    }
  }, [effectiveMonthWindowIndex, monthWindows.length, totalMonths]);

  const handleNextMonth = useCallback(() => {
    setSelectedSession(null);
    if (effectiveMonthWindowIndex > 0) {
      setMonthWindowIndex(effectiveMonthWindowIndex - 1);
    } else {
      setMonthWindowIndex(0);
      setMonthIndex((idx) => Math.max(0, idx - 1));
    }
  }, [effectiveMonthWindowIndex]);

  const handleBackToMonth = useCallback(() => {
    openMonthOf(effectiveDateIndex);
    setView("month");
  }, [openMonthOf, effectiveDateIndex]);

  const handleDragState = useCallback((moved: boolean) => {
    dragMovedRef.current = moved;
  }, []);

  const handleStageClick = useCallback(() => {
    if (dragMovedRef.current) {
      dragMovedRef.current = false;
      return;
    }
    setSelectedSession(null);
  }, []);

  const isMeasuring = dimensions.width === 0;
  const showLoading = sessionsLoading || isMeasuring;

  const dayMetaLabel = useMemo(
    () =>
      `${currentWindowSessions.length} ${
        currentWindowSessions.length === 1
          ? HISTORY_COPY.sessionSingular
          : HISTORY_COPY.sessionPlural
      }`,
    [currentWindowSessions.length]
  );

  const monthMetaLabel = useMemo(() => {
    const monthDaysCount = currentMonthWindow?.days.length ?? 0;
    const totalSessions =
      currentMonthWindow?.days.reduce((sum, d) => sum + d.sessions.length, 0) ?? 0;
    return `${monthDaysCount} ${
      monthDaysCount === 1 ? HISTORY_COPY.daySingular : HISTORY_COPY.dayPlural
    } · ${totalSessions} ${
      monthDaysCount === 1 ? HISTORY_COPY.sessionSingular : HISTORY_COPY.sessionPlural
    }`;
  }, [currentMonthWindow]);

  const hintText =
    effectiveView === "month" ? HISTORY_COPY.monthHint : HISTORY_COPY.clickHint;

  const dayTurnsCount = useMemo(
    () => currentWindowSessions.reduce((sum, s) => sum + s.turn_count, 0),
    [currentWindowSessions]
  );

  const monthTurnsCount = useMemo(
    () =>
      currentMonthGroup?.days.reduce(
        (sum, d) => sum + d.sessions.reduce((sSum, s) => sSum + s.turn_count, 0),
        0
      ) ?? 0,
    [currentMonthGroup]
  );

  // Time span formatted for the window e.g. "08:15 AM - 10:42 PM"
  const dayTimeSpan = useMemo(() => {
    if (currentWindowSessions.length === 0) return null;
    const timestamps = currentWindowSessions.map((s) => s.created_at).sort((a, b) => a - b);
    const earliest = timestamps[0];
    const latest = timestamps[timestamps.length - 1];
    const fmt = (ms: number) =>
      new Date(ms).toLocaleTimeString(undefined, {
        hour: "2-digit",
        minute: "2-digit",
        hour12: true,
      });
    return earliest === latest ? fmt(earliest) : `${fmt(earliest)} – ${fmt(latest)}`;
  }, [currentWindowSessions]);

  return {
    sessions,
    showLoading,
    error,
    selectedSession,
    setSelectedSession,
    turns,
    turnsLoading,
    turnsError,
    deleteError,
    setDeleteError,
    confirmDeleteId,
    view,
    currentGroup,
    currentDateSessions,
    currentMonthGroup,
    currentWindow,
    currentWindowSessions,
    currentMonthWindow,
    sessionById,
    ringRadius,
    dayWindowIndex: effectiveSessionWindowIndex,
    monthWindowIndex: effectiveMonthWindowIndex,
    dayWindows: sessionWindows,
    monthWindows,
    dayGroups,
    monthGroups,
    totalDates,
    totalMonths,
    dateIndex: effectiveDateIndex,
    monthIndex: effectiveMonthIndex,
    isCompactHeight,
    isOrbitViewport,
    effectiveView,
    dayMetaLabel,
    monthMetaLabel,
    dayTurnsCount,
    monthTurnsCount,
    dayTimeSpan,
    hintText,
    containerRef,
    loadSessions,
    handleViewChange,
    handleDrillIntoDay,
    retryFetchTurns,
    handleDelete,
    handleCancelDelete,
    handlePrevDate,
    handleNextDate,
    handleGoToday,
    handlePrevMonth,
    handleNextMonth,
    handleBackToMonth,
    handleDragState,
    handleStageClick,
    formatDayHeroLabel,
    formatDayHeroParts,
    formatWeekdayLabel,
    formatMonthFullLabel,
    formatDayYearLabel,
    formatMonthHeroLabel,
    formatMonthShortLabel,
    formatMonthYearLabel,
  };
}
