import { useState, useEffect, useCallback, useRef } from "react";
import { getObservations, type ObservationRecord } from "@/services/memoryService";

const PAGE_SIZE = 25;

export type ObservationFilter = "all" | "staged" | "pending" | "integrated";

export interface UseObservationsListReturn {
  observations: ObservationRecord[];
  statusFilter: ObservationFilter;
  setStatusFilter: (f: ObservationFilter) => void;
  isLoading: boolean;
  isLoadingMore: boolean;
  hasMore: boolean;
  loadMore: () => Promise<void>;
  refresh: () => Promise<void>;
}

export function useObservationsList(
  active: boolean,
  projectId?: string,
  observationType?: string
): UseObservationsListReturn {
  const [statusFilter, setStatusFilter] = useState<ObservationFilter>("staged");
  const [observations, setObservations] = useState<ObservationRecord[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(true);

  const offsetRef = useRef(0);
  const hasMoreRef = useRef(true);
  const isFetchingRef = useRef(false);
  // Monotonic request id: only the latest fetch may commit. A filter change
  // during an in-flight fetch previously hit the mutex below and was silently
  // dropped, leaving the previous filter's rows under the new tab label.
  const seqRef = useRef(0);
  // Mounted guard (style-guide §4.3). NOTE: no AbortController — Tauri's
  // invoke() accepts no abort signal, so guard-on-commit is the complete fix.
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const fetchBatch = useCallback(
    async (filter: ObservationFilter, offset: number, append: boolean) => {
      const seq = ++seqRef.current;
      isFetchingRef.current = true;

      if (offset === 0) {
        setIsLoading(true);
      } else {
        setIsLoadingMore(true);
      }

      try {
        const statusParam = filter === "all" ? undefined : filter;
        const typeParam = observationType === "all" ? undefined : observationType;
        const batch = await getObservations(projectId, statusParam, PAGE_SIZE, offset, typeParam);

        // Stale (superseded) or unmounted: commit nothing.
        if (!mountedRef.current || seq !== seqRef.current) return;

        if (batch.length < PAGE_SIZE) {
          hasMoreRef.current = false;
          setHasMore(false);
        } else {
          hasMoreRef.current = true;
          setHasMore(true);
        }

        offsetRef.current = offset + batch.length;

        if (append) {
          setObservations((prev) => {
            const seen = new Set(prev.map((o) => o.id));
            const newItems = batch.filter((o) => !seen.has(o.id));
            return [...prev, ...newItems];
          });
        } else {
          setObservations(batch);
        }
      } catch (err) {
        if (!mountedRef.current || seq !== seqRef.current) return;
        console.error("[useObservationsList] Failed to fetch observations:", err);
      } finally {
        // Only the latest request clears the flags.
        if (mountedRef.current && seq === seqRef.current) {
          setIsLoading(false);
          setIsLoadingMore(false);
          isFetchingRef.current = false;
        }
      }
    },
    [projectId, observationType]
  );

  // Re-fetch when active becomes true or statusFilter changes
  useEffect(() => {
    if (!active) return;
    offsetRef.current = 0;
    hasMoreRef.current = true;
    setHasMore(true);
    fetchBatch(statusFilter, 0, false);
  }, [active, statusFilter, fetchBatch]);

  const loadMore = useCallback(async () => {
    if (!active || isFetchingRef.current || !hasMoreRef.current) return;
    await fetchBatch(statusFilter, offsetRef.current, true);
  }, [active, statusFilter, fetchBatch]);

  const refresh = useCallback(async () => {
    offsetRef.current = 0;
    hasMoreRef.current = true;
    setHasMore(true);
    await fetchBatch(statusFilter, 0, false);
  }, [statusFilter, fetchBatch]);

  return {
    observations,
    statusFilter,
    setStatusFilter,
    isLoading,
    isLoadingMore,
    hasMore,
    loadMore,
    refresh,
  };
}
