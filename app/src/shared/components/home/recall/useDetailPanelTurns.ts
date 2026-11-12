import { useState, useEffect, useCallback, useRef } from "react";
import { getTurns, type SessionRow, type TurnRow } from "@/services/persistenceService";

export interface UseDetailPanelTurnsReturn {
  turns: TurnRow[];
  loading: boolean;
  error: string | null;
  retry: () => void;
}

/**
 * Shared hook to manage fetching and retrying turns for a selected session.
 * Guarantees monotonic sequence ID tracking across both initial fetch and manual retry cycles,
 * preventing race condition state leaks during rapid session switches.
 */
export function useDetailPanelTurns(session: SessionRow | null): UseDetailPanelTurnsReturn {
  const [turns, setTurns] = useState<TurnRow[]>([]);
  const [loading, setLoading] = useState(() => Boolean(session));
  const [error, setError] = useState<string | null>(null);
  const activeReqIdRef = useRef(0);

  const fetchTurns = useCallback((targetSession: SessionRow) => {
    const reqId = ++activeReqIdRef.current;
    void getTurns(targetSession.id)
      .then((data) => {
        if (reqId === activeReqIdRef.current) {
          setTurns(data);
          setLoading(false);
        }
      })
      .catch((err: unknown) => {
        if (reqId === activeReqIdRef.current) {
          setError(err instanceof Error ? err.message : "Failed to load session transcript");
          setLoading(false);
        }
      });
  }, []);

  useEffect(() => {
    if (!session) {
      activeReqIdRef.current += 1;
      return;
    }
    fetchTurns(session);
    return () => {
      activeReqIdRef.current += 1;
    };
  }, [session, fetchTurns]);

  const retry = useCallback(() => {
    if (session) {
      setLoading(true);
      setError(null);
      fetchTurns(session);
    }
  }, [session, fetchTurns]);

  return {
    turns: session ? turns : [],
    loading: session ? loading : false,
    error: session ? error : null,
    retry,
  };
}
