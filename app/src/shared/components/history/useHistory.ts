import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import {
  getSessions,
  getTurns,
  deleteSession,
  formatDateTime,
  type SessionRow,
  type TurnRow,
} from "@/services/historyService";
import {
  chunkDaysIntoWindows,
  chunkSessionsIntoWindows,
  dayNumberFromKey,
  dayToDialAngle,
  daysInMonthKey,
  dialDotRadius,
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
  timeToDialAngle,
  type DialDot,
  type HistoryView,
} from "@/shared/components/history";
import { HISTORY_COPY } from "@/data/historyCopy";

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
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [sessionsLoading, setSessionsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedSession, setSelectedSession] = useState<SessionRow | null>(null);
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
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

  // Viewport resize observer
  useEffect(() => {
    if (!containerRef.current) return;
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
      setSessions(data.sort((a, b) => b.started_at - a.started_at));
    } catch (e: unknown) {
      console.error("Failed to fetch sessions:", e);
      setError(getErrorMessage(e, HISTORY_COPY.failedFallback));
    }
  }, []);

  const loadSessions = useCallback(async () => {
    setSessionsLoading(true);
    await fetchSessions();
    setSessionsLoading(false);
  }, [fetchSessions]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // Groupings
  const dayGroups = useMemo(() => groupSessionsByDay(sessions), [sessions]);
  const monthGroups = useMemo(() => groupDaysByMonth(dayGroups), [dayGroups]);

  const totalDates = Math.max(1, dayGroups.length);
  const totalMonths = Math.max(1, monthGroups.length);

  // Clamped indices without cascading effect loops
  const effectiveDateIndex = Math.min(dateIndex, totalDates - 1);
  const effectiveMonthIndex = Math.min(monthIndex, totalMonths - 1);

  const currentGroup = dayGroups[effectiveDateIndex] || {
    dayKey: "",
    dayLabel: formatDateTime(Date.now()),
    latestTimestamp: Date.now(),
    sessions: [],
  };
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

  // Windows
  const dayWindows = useMemo(
    () => chunkSessionsIntoWindows(currentDateSessions, capacity),
    [currentDateSessions, capacity]
  );
  const monthWindows = useMemo(
    () => chunkDaysIntoWindows(currentMonthGroup?.days ?? [], capacity),
    [currentMonthGroup, capacity]
  );

  const effectiveDayWindowIndex = Math.min(
    dayWindowIndex,
    Math.max(0, dayWindows.length - 1)
  );
  const effectiveMonthWindowIndex = Math.min(
    monthWindowIndex,
    Math.max(0, monthWindows.length - 1)
  );

  const currentWindow = dayWindows[effectiveDayWindowIndex];
  const currentWindowSessions = currentWindow?.sessions ?? [];
  const currentMonthWindow = monthWindows[effectiveMonthWindowIndex];

  const windowSessionIds = useMemo(
    () => new Set(currentWindowSessions.map((s) => String(s.id))),
    [currentWindowSessions]
  );
  const windowDayKeys = useMemo(
    () => new Set((currentMonthWindow?.days ?? []).map((d) => d.dayKey)),
    [currentMonthWindow]
  );

  const sessionById = useMemo(
    () => new Map(currentWindowSessions.map((s) => [String(s.id), s])),
    [currentWindowSessions]
  );

  // Dial dots
  const dayDialDots = useMemo<DialDot[]>(
    () =>
      currentDateSessions.map((s) => ({
        key: String(s.id),
        angle: timeToDialAngle(s.started_at),
        size: dialDotRadius(s.turn_count),
        highlighted: windowSessionIds.has(String(s.id)),
      })),
    [currentDateSessions, windowSessionIds]
  );

  const monthDialDots = useMemo<DialDot[]>(() => {
    if (!currentMonthGroup) return [];
    const total = daysInMonthKey(currentMonthGroup.monthKey);
    return currentMonthGroup.days.map((d) => ({
      key: d.dayKey,
      angle: dayToDialAngle(dayNumberFromKey(d.dayKey), total),
      size: dialDotRadius(d.sessions.reduce((sum, s) => sum + s.turn_count, 0)),
      highlighted: windowDayKeys.has(d.dayKey),
    }));
  }, [currentMonthGroup, windowDayKeys]);

  const isCompactHeight = dimensions.height < 640;
  const dialRadius = useMemo(() => {
    const discRadius = isCompactHeight
      ? 96
      : dimensions.width >= 640
        ? 144
        : 128;
    return discRadius + 32;
  }, [isCompactHeight, dimensions.width]);

  const isOrbitViewport = dimensions.width >= 680;
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

  // Turn fetching
  useEffect(() => {
    if (!selectedSession) {
      setTurns([]);
      setTurnsError(null);
      return;
    }
    let isCancelled = false;
    const fetchTurns = async () => {
      setTurnsLoading(true);
      setTurnsError(null);
      try {
        const data = await getTurns(selectedSession.id);
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
  }, [selectedSession]);

  const retryFetchTurns = useCallback(() => {
    if (!selectedSession) return;
    setTurnsLoading(true);
    setTurnsError(null);
    getTurns(selectedSession.id)
      .then((data) => {
        setTurns(data);
      })
      .catch((e: unknown) => {
        console.error("Failed to fetch turns:", e);
        setTurnsError(getErrorMessage(e, "Failed to load session transcript turns."));
      })
      .finally(() => {
        setTurnsLoading(false);
      });
  }, [selectedSession]);

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

  const handlePrevDate = useCallback(() => {
    setSelectedSession(null);
    if (effectiveDayWindowIndex > 0) {
      setDayWindowIndex(effectiveDayWindowIndex - 1);
    } else {
      setDayWindowIndex(0);
      setDateIndex((idx) => Math.max(0, idx - 1));
    }
  }, [effectiveDayWindowIndex]);

  const handleNextDate = useCallback(() => {
    setSelectedSession(null);
    if (effectiveDayWindowIndex < dayWindows.length - 1) {
      setDayWindowIndex(effectiveDayWindowIndex + 1);
    } else {
      setDayWindowIndex(0);
      setDateIndex((idx) => Math.min(totalDates - 1, idx + 1));
    }
  }, [effectiveDayWindowIndex, dayWindows.length, totalDates]);

  const handleGoToday = useCallback(() => {
    setSelectedSession(null);
    setDayWindowIndex(0);
    setDateIndex(0);
  }, []);

  const handlePrevMonth = useCallback(() => {
    setSelectedSession(null);
    if (effectiveMonthWindowIndex > 0) {
      setMonthWindowIndex(effectiveMonthWindowIndex - 1);
    } else {
      setMonthWindowIndex(0);
      setMonthIndex((idx) => Math.max(0, idx - 1));
    }
  }, [effectiveMonthWindowIndex]);

  const handleNextMonth = useCallback(() => {
    setSelectedSession(null);
    if (effectiveMonthWindowIndex < monthWindows.length - 1) {
      setMonthWindowIndex(effectiveMonthWindowIndex + 1);
    } else {
      setMonthWindowIndex(0);
      setMonthIndex((idx) => Math.min(totalMonths - 1, idx + 1));
    }
  }, [effectiveMonthWindowIndex, monthWindows.length, totalMonths]);

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

  const dayMetaLabel = `${currentWindowSessions.length} ${
    currentWindowSessions.length === 1
      ? HISTORY_COPY.sessionSingular
      : HISTORY_COPY.sessionPlural
  }`;

  const monthDaysCount = currentMonthWindow?.days.length ?? 0;
  const monthMetaLabel = `${monthDaysCount} ${
    monthDaysCount === 1 ? HISTORY_COPY.daySingular : HISTORY_COPY.dayPlural
  } · ${currentMonthWindow?.days.reduce((sum, d) => sum + d.sessions.length, 0) ?? 0} ${
    monthDaysCount === 1 ? HISTORY_COPY.sessionSingular : HISTORY_COPY.sessionPlural
  }`;

  const hintText =
    effectiveView === "month" ? HISTORY_COPY.monthHint : HISTORY_COPY.clickHint;

  const dayTurnsCount = useMemo(
    () => currentDateSessions.reduce((sum, s) => sum + s.turn_count, 0),
    [currentDateSessions]
  );

  const monthTurnsCount = useMemo(
    () =>
      currentMonthGroup?.days.reduce(
        (sum, d) => sum + d.sessions.reduce((sSum, s) => sSum + s.turn_count, 0),
        0
      ) ?? 0,
    [currentMonthGroup]
  );

  // Time span formatted for the day e.g. "08:15 AM - 10:42 PM"
  const dayTimeSpan = useMemo(() => {
    if (currentDateSessions.length === 0) return null;
    const timestamps = currentDateSessions.map((s) => s.started_at).sort((a, b) => a - b);
    const earliest = timestamps[0];
    const latest = timestamps[timestamps.length - 1];
    const fmt = (ms: number) =>
      new Date(ms).toLocaleTimeString(undefined, {
        hour: "2-digit",
        minute: "2-digit",
        hour12: true,
      });
    return earliest === latest ? fmt(earliest) : `${fmt(earliest)} – ${fmt(latest)}`;
  }, [currentDateSessions]);

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
    dialRadius,
    dayDialDots,
    monthDialDots,
    dayWindowIndex: effectiveDayWindowIndex,
    monthWindowIndex: effectiveMonthWindowIndex,
    dayWindows,
    monthWindows,
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
