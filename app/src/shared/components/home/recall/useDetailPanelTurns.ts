import { useState, useEffect, useCallback } from "react";
import { getTurns, type SessionRow, type TurnRow } from "@/services/persistenceService";

export interface UseDetailPanelTurnsReturn {
  turns: TurnRow[];
  loading: boolean;
  error: string | null;
  retry: () => void;
}

/**
 * Shared hook to manage fetching and retrying turns for a selected session.
 * Guarantees active/unmounted guards across both initial fetch and manual retry cycles.
 */
export function useDetailPanelTurns(session: SessionRow | null): UseDetailPanelTurnsReturn {
  const [turns, setTurns] = useState<TurnRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchTurns = useCallback((targetSession: SessionRow) => {
    let active = true;
    setLoading(true);
    setError(null);

    void getTurns(targetSession.id)
      .then((data) => {
        if (active) setTurns(data);
      })
      .catch((err: unknown) => {
        if (active) {
          setError(err instanceof Error ? err.message : "Failed to load session transcript");
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!session) {
      setTurns([]);
      setLoading(false);
      setError(null);
      return;
    }
    return fetchTurns(session);
  }, [session, fetchTurns]);

  const retry = useCallback(() => {
    if (session) {
      fetchTurns(session);
    }
  }, [session, fetchTurns]);

  return { turns, loading, error, retry };
}
