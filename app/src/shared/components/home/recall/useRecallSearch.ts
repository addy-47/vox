import { useState, useEffect, useCallback, useMemo } from "react";
import {
  getSessions,
  getCachedSessions,
  deleteSession,
  resolveSessionTitle,
  type SessionRow,
} from "@/services/persistenceService";
import { getProjects, type ProjectRow } from "@/services/projectsService";
import { onSessionsChanged } from "@/services/eventsService";
import type { CarouselItemData } from "./recallTypes";
import { useDetailPanelTurns } from "./useDetailPanelTurns";

export type SortOrder = "newest" | "oldest" | "turns";

export function useRecallSearch() {
  const [sessions, setSessions] = useState<SessionRow[]>(() => {
    const cached = getCachedSessions();
    return cached ? [...cached].sort((a, b) => a.created_at - b.created_at) : [];
  });
  const [projects, setProjects] = useState<ProjectRow[]>([]);
  const [loading, setLoading] = useState(() => !getCachedSessions());
  const [searchQuery, setSearchQuery] = useState("");
  const [sortOrder, setSortOrder] = useState<SortOrder>("newest");
  const [userNavigatedIndex, setUserNavigatedIndex] = useState<number | null>(null);

  // Selected session for DetailPanel drawer
  const [selectedSession, setSelectedSession] = useState<SessionRow | null>(null);
  const {
    turns,
    loading: turnsLoading,
    error: turnsError,
    retry: retryFetchTurns,
  } = useDetailPanelTurns(selectedSession);

  // Fetch initial data & subscribe to background changes
  const refreshData = useCallback(async () => {
    try {
      const [fetchedSessions, fetchedProjects] = await Promise.all([
        getSessions(),
        getProjects().catch(() => []),
      ]);
      setSessions([...fetchedSessions].sort((a, b) => a.created_at - b.created_at));
      setProjects(fetchedProjects);
    } catch (e) {
      console.warn("[useRecallSearch] Failed to fetch sessions/projects:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let isMounted = true;
    queueMicrotask(() => {
      if (isMounted) void refreshData();
    });

    const unlisten = onSessionsChanged(() => {
      if (isMounted) void refreshData();
    });
    return () => {
      isMounted = false;
      unlisten();
    };
  }, [refreshData]);

  const projectNameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const p of projects) {
      map.set(p.id, p.name);
    }
    return map;
  }, [projects]);

  // Reset user navigated index when search query or sort order changes
  useEffect(() => {
    setUserNavigatedIndex(null);
  }, [searchQuery, sortOrder]);

  // Filtered and sorted sessions
  // Timeline axis flows left-to-right (past -> future) chronologically
  const filteredSessions = useMemo(() => {
    let result = [...sessions];

    const q = searchQuery.trim().toLowerCase();
    if (q) {
      result = result.filter((s) => {
        const title = resolveSessionTitle(s).toLowerCase();
        const project = (s.project_id ? projectNameMap.get(s.project_id) ?? s.project_id : "").toLowerCase();
        return title.includes(q) || project.includes(q);
      });
    }

    if (sortOrder === "turns") {
      result.sort((a, b) => b.turn_count - a.turn_count);
    } else {
      // Both "newest" and "oldest" sort chronologically left-to-right (past -> future)
      result.sort((a, b) => a.created_at - b.created_at);
    }

    return result;
  }, [sessions, searchQuery, sortOrder, projectNameMap]);

  // Carousel items (sessions with right-hand skeleton placeholders for symmetry)
  const carouselItems = useMemo<CarouselItemData[]>(() => {
    if (filteredSessions.length === 0) return [];
    if (sortOrder !== "turns") {
      const skeletons: CarouselItemData[] = [
        { id: "skeleton-slot-1", isSkeleton: true },
        { id: "skeleton-slot-2", isSkeleton: true },
      ];
      return [...filteredSessions, ...skeletons];
    }
    return filteredSessions;
  }, [filteredSessions, sortOrder]);

  const maxRealIndex = Math.max(0, filteredSessions.length - 1);

  // Safe active index derived directly during render to prevent cascading renders
  const activeIndex = useMemo(() => {
    if (filteredSessions.length === 0) return 0;
    if (userNavigatedIndex !== null) {
      return Math.max(0, Math.min(userNavigatedIndex, maxRealIndex));
    }
    // "newest" focuses the most recent session
    if (sortOrder === "newest") {
      return maxRealIndex;
    }
    // "oldest" or "turns" focuses the first item
    return 0;
  }, [filteredSessions.length, userNavigatedIndex, sortOrder, maxRealIndex]);

  const handleDeleteSession = useCallback(async (id: number) => {
    try {
      await deleteSession(id);
      setSessions((prev) => prev.filter((s) => s.id !== id));
      if (selectedSession?.id === id) {
        setSelectedSession(null);
      }
    } catch (e) {
      console.warn("[useRecallSearch] Failed to delete session:", e);
    }
  }, [selectedSession]);

  const openSessionDetail = useCallback((session: SessionRow) => {
    setSelectedSession(session);
  }, []);

  const closeSessionDetail = useCallback(() => {
    setSelectedSession(null);
  }, []);

  const selectIndex = useCallback((index: number) => {
    setUserNavigatedIndex(Math.max(0, Math.min(index, maxRealIndex)));
  }, [maxRealIndex]);

  const nextCard = useCallback(() => {
    setUserNavigatedIndex((prev) => {
      const current = prev ?? (sortOrder === "newest" ? maxRealIndex : 0);
      return Math.min(current + 1, maxRealIndex);
    });
  }, [maxRealIndex, sortOrder]);

  const prevCard = useCallback(() => {
    setUserNavigatedIndex((prev) => {
      const current = prev ?? (sortOrder === "newest" ? maxRealIndex : 0);
      return Math.max(current - 1, 0);
    });
  }, [maxRealIndex, sortOrder]);

  return {
    sessions,
    filteredSessions,
    carouselItems,
    loading,
    searchQuery,
    setSearchQuery,
    sortOrder,
    setSortOrder,
    activeIndex,
    setActiveIndex: setUserNavigatedIndex,
    selectIndex,
    nextCard,
    prevCard,
    selectedSession,
    setSelectedSession,
    openSessionDetail,
    closeSessionDetail,
    turns,
    turnsLoading,
    turnsError,
    retryFetchTurns,
    handleDeleteSession,
    projectNameMap,
  };
}
