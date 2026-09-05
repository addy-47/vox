import { useState, useEffect, useCallback } from "react";
import {
  getSessions,
  sortSessionsNewestFirst,
  type SessionRow,
} from "@/services/historyService";
import {
  onSessionTitleUpdated,
  onSessionsChanged,
} from "@/services/eventsService";
import { SESSION_COPY } from "@/data/sessionCopy";

function getErrorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  return SESSION_COPY.sessionsFailed;
}

/**
 * Conversation list for the Home side rail (session continuation §A).
 * Sessions are newest-first by last activity. A persisted title patch
 * updates the matching entry in place — no reselect, reload, or restart
 * required (spec §A.6).
 *
 * @param listVersion bump to force a refetch (engage / disengage /
 * restore / new-conversation change the session set even when the backend
 * emits no `sessions_changed` event).
 */
export function useConversationList(listVersion: number) {
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const data = await getSessions();
      setSessions(sortSessionsNewestFirst(data));
    } catch (e: unknown) {
      console.error("[sessions] Failed to fetch conversations:", e);
      setError(getErrorMessage(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let isMounted = true;
    setLoading(true);
    refresh().catch(() => {
      if (isMounted) setLoading(false);
    });

    const unlisteners: (() => void)[] = [];
    unlisteners.push(
      onSessionTitleUpdated((payload) => {
        if (!isMounted) return;
        const title = payload.title.trim();
        if (!title) return;
        setSessions((prev) =>
          prev.map((s) => (s.id === payload.session_id ? { ...s, title } : s))
        );
      })
    );
    unlisteners.push(
      onSessionsChanged(() => {
        if (!isMounted) return;
        refresh().catch(() => {});
      })
    );

    return () => {
      isMounted = false;
      unlisteners.forEach((fn) => fn());
    };
  }, [refresh, listVersion]);

  return { sessions, loading, error, refresh };
}
