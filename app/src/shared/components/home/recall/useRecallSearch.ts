import { useState, useEffect, useCallback, useMemo } from "react";
import {
  getSessions,
  getCachedSessions,
  deleteSession,
  searchHistory,
  resolveSessionTitle,
  type SessionRow,
} from "@/services/persistenceService";
import { getProjects, type ProjectRow } from "@/services/projectsService";
import { onSessionsChanged } from "@/services/eventsService";
import type { CarouselItemData } from "./recallTypes";
import { useDetailPanelTurns } from "./useDetailPanelTurns";
import { useRecallSearchStore, type SortOrder } from "@/store/recallSearchStore";

export type { SortOrder };

export function useRecallSearch() {
  const [sessions, setSessions] = useState<SessionRow[]>(() => {
    const cached = getCachedSessions();
    return cached ? [...cached].sort((a, b) => a.created_at - b.created_at) : [];
  });
  const [projects, setProjects] = useState<ProjectRow[]>([]);
  const [loading, setLoading] = useState(() => !getCachedSessions());

  const searchQuery = useRecallSearchStore((s) => s.searchQuery);
  const setSearchQuery = useRecallSearchStore((s) => s.setSearchQuery);
  const sortOrder = useRecallSearchStore((s) => s.sortOrder);
  const setSortOrder = useRecallSearchStore((s) => s.setSortOrder);
  const userNavigatedIndex = useRecallSearchStore((s) => s.userNavigatedIndex);
  const setUserNavigatedIndex = useRecallSearchStore((s) => s.setUserNavigatedIndex);

  // Selected session for DetailPanel drawer
  const [selectedSession, setSelectedSession] = useState<SessionRow | null>(null);
  const [searchResults, setSearchResults] = useState<SessionRow[] | null>(null);
  const {
    turns,
    loading: turnsLoading,
    error: turnsError,
    retry: retryFetchTurns,
  } = useDetailPanelTurns(selectedSession);

  // Debounced backend search across session titles and turn transcripts
  useEffect(() => {
    const q = searchQuery.trim();
    if (!q) {
      setSearchResults(null);
      return;
    }

    const timer = window.setTimeout(() => {
      searchHistory(q, null, 100)
        .then((hits) => {
          setSearchResults(hits.map((h) => h.session));
        })
        .catch((e: unknown) => {
          console.warn("[useRecallSearch] searchHistory error:", e);
          setSearchResults(null);
        });
    }, 150);

    return () => window.clearTimeout(timer);
  }, [searchQuery]);

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

  // Filtered and sorted sessions
  const filteredSessions = useMemo(() => {
    const baseList = searchResults ?? sessions;
    let result = [...baseList];

    // Client-side title and project filter fallback while search is pending
    const q = searchQuery.trim().toLowerCase();
    if (q && searchResults === null) {
      result = result.filter((s) => {
        const title = resolveSessionTitle(s).toLowerCase();
        const project = (s.project_id ? projectNameMap.get(s.project_id) ?? s.project_id : "").toLowerCase();
        return title.includes(q) || project.includes(q);
      });
    }

    if (sortOrder === "turns") {
      result.sort((a, b) => b.turn_count - a.turn_count);
    } else if (sortOrder === "oldest") {
      result.sort((a, b) => a.created_at - b.created_at);
    } else {
      // Default: "newest" sorts most recent first
      result.sort((a, b) => b.created_at - a.created_at);
    }

    return result;
  }, [sessions, searchResults, searchQuery, sortOrder, projectNameMap]);

  // Carousel items: real sessions without fake skeleton cards
  const carouselItems = useMemo<CarouselItemData[]>(() => {
    return filteredSessions;
  }, [filteredSessions]);

  const maxRealIndex = Math.max(0, filteredSessions.length - 1);

  // Safe active index derived directly during render to prevent cascading renders
  const activeIndex = useMemo(() => {
    if (filteredSessions.length === 0) return 0;
    if (userNavigatedIndex !== null) {
      return Math.max(0, Math.min(userNavigatedIndex, maxRealIndex));
    }
    return 0;
  }, [filteredSessions.length, userNavigatedIndex, maxRealIndex]);

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
