import { useState, useEffect, useMemo, useCallback, memo } from "react";
import {
  ArrowLeft,
  Sparkles,
} from "lucide-react";
import {
  getSessions,
  resolveSessionTitle,
  formatSessionRecency,
  sortSessionsNewestFirst,
  type SessionRow,
} from "@/services/historyService";
import { ObservationRecord } from "@/services/memoryService";
import { cn } from "@/shared/lib/utils";
import { toMemoryCategory } from "./memoryGraphTypes";

interface MemorySessionRailProps {
  facts: ObservationRecord[];
  selectedSessionId: string | null;
  selectedFactId: string | null;
  onSelectSession: (sessionId: string | null) => void;
  onSelectFact: (fact: ObservationRecord) => void;
  onClose?: () => void;
  isLightMode?: boolean;
}

export const MemorySessionRail = memo<MemorySessionRailProps>(({
  facts,
  selectedSessionId,
  selectedFactId,
  onSelectSession,
  onSelectFact,
}) => {
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeDrillSession, setActiveDrillSession] = useState<SessionRow | null>(null);

  // Load sessions
  useEffect(() => {
    let isMounted = true;
    getSessions()
      .then((data) => {
        if (isMounted) {
          setSessions(sortSessionsNewestFirst(data));
        }
      })
      .catch((err) => {
        console.error("[MemorySessionRail] Failed to load sessions:", err);
      })
      .finally(() => {
        if (isMounted) setLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, []);

  // Map session ID to facts
  const sessionFactsMap = useMemo(() => {
    const map = new Map<number, ObservationRecord[]>();
    for (const f of facts) {
      if (f.session_id !== null) {
        const list = map.get(f.session_id) || [];
        list.push(f);
        map.set(f.session_id, list);
      }
    }
    return map;
  }, [facts]);

  // Sync active drill session when selectedSessionId changes externally
  useEffect(() => {
    if (selectedSessionId && sessions.length > 0) {
      const match = sessions.find((s) => String(s.id) === selectedSessionId);
      if (match && (!activeDrillSession || activeDrillSession.id !== match.id)) {
        setActiveDrillSession(match);
      }
    }
  }, [selectedSessionId, sessions, activeDrillSession]);

  const activeDrillFacts = useMemo(() => {
    if (!activeDrillSession) return [];
    return sessionFactsMap.get(activeDrillSession.id) || [];
  }, [activeDrillSession, sessionFactsMap]);

  const handleSelectSessionCard = useCallback(
    (session: SessionRow) => {
      const sId = String(session.id);
      onSelectSession(sId);
      setActiveDrillSession(session);
    },
    [onSelectSession]
  );

  const handleBackToSessions = useCallback(() => {
    setActiveDrillSession(null);
    onSelectSession(null);
  }, [onSelectSession]);

  return (
    <div className="flex flex-col h-full w-full bg-transparent overflow-hidden select-none font-sans">
      {/* ── View 1: Drilled Session Memory Details ── */}
      {activeDrillSession ? (
        <div className="flex flex-col h-full overflow-hidden">
          {/* Minimal Header */}
          <div className="px-4 pt-3 pb-2.5 shrink-0 border-b border-[rgba(var(--border),0.08)] flex flex-col gap-1.5">
            <button
              type="button"
              onClick={handleBackToSessions}
              className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--accent))] hover:underline cursor-pointer w-fit"
            >
              <ArrowLeft size={12} />
              <span>Back to sessions</span>
            </button>

            <div className="flex flex-col">
              <h3 className="text-[13px] font-medium text-[rgb(var(--foreground))] truncate">
                {resolveSessionTitle(activeDrillSession)}
              </h3>
              <p className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] mt-0.5">
                {formatSessionRecency(activeDrillSession.updated_at)} · {activeDrillSession.turn_count} turns · {activeDrillFacts.length} memories
              </p>
            </div>
          </div>

          {/* Facts List — Zero pill containers, clean flat cards */}
          <div className="flex-1 overflow-y-auto custom-scrollbar px-3 py-2 pb-28 space-y-2 scrollbar-thin scrollbar-thumb-[rgba(var(--foreground),0.15)] scrollbar-track-transparent">
            {activeDrillFacts.length === 0 ? (
              <div className="flex flex-col items-center justify-center p-8 text-center">
                <Sparkles size={18} className="text-[rgb(var(--foreground-muted))] opacity-40 mb-2" />
                <p className="text-[11.5px] font-mono text-[rgb(var(--foreground))]">
                  No memories recorded
                </p>
                <p className="text-[10.5px] text-[rgb(var(--foreground-muted))] mt-0.5">
                  This session did not produce permanent graph observations.
                </p>
              </div>
            ) : (
              activeDrillFacts.map((fact) => {
                const isFactSelected = selectedFactId === fact.id;
                const kind = toMemoryCategory(fact.fact_type) || "objective";

                return (
                  <button
                    key={fact.id}
                    type="button"
                    onClick={() => onSelectFact(fact)}
                    className={cn(
                      "w-full text-left p-3 rounded-lg border transition-colors cursor-pointer flex flex-col gap-1.5",
                      isFactSelected
                        ? "border-[rgba(var(--accent),0.4)] bg-[rgba(var(--accent),0.08)]"
                        : "border-[rgba(var(--border),0.07)] bg-[rgba(var(--card),0.3)] hover:bg-[rgba(var(--card),0.6)] hover:border-[rgba(var(--accent),0.2)]"
                    )}
                  >
                    <span className="text-[9.5px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))] font-medium">
                      {kind.replace("_", " ")}
                    </span>
                    <p className="text-[12px] font-sans text-[rgb(var(--foreground))] line-clamp-3 leading-relaxed m-0">
                      {fact.text}
                    </p>
                  </button>
                );
              })
            )}
          </div>
        </div>
      ) : (
        /* ── View 2: Sessions List — Zero heading, zero search pill, clean rows ── */
        <div className="flex-1 overflow-y-auto custom-scrollbar px-3 pt-3 pb-28 space-y-1 scrollbar-thin scrollbar-thumb-[rgba(var(--foreground),0.15)] scrollbar-track-transparent">
          {loading ? (
            <div className="flex items-center justify-center p-8">
              <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] animate-pulse">
                Loading sessions…
              </span>
            </div>
          ) : sessions.length === 0 ? (
            <div className="flex flex-col items-center justify-center p-8 text-center">
              <p className="text-[11.5px] font-mono text-[rgb(var(--foreground-muted))]">
                No sessions available
              </p>
            </div>
          ) : (
            sessions.map((session) => {
              const sIdStr = String(session.id);
              const isSelected = selectedSessionId === sIdStr;
              const sessionFacts = sessionFactsMap.get(session.id) || [];
              const factCount = sessionFacts.length;
              const title = resolveSessionTitle(session);
              const recency = formatSessionRecency(session.updated_at);

              return (
                <button
                  key={session.id}
                  type="button"
                  onClick={() => handleSelectSessionCard(session)}
                  className={cn(
                    "w-full text-left px-3 py-2.5 rounded-lg border transition-colors cursor-pointer flex flex-col gap-0.5",
                    isSelected
                      ? "border-[rgba(var(--accent),0.4)] bg-[rgba(var(--accent),0.08)]"
                      : "border-transparent hover:bg-[rgba(var(--foreground),0.04)]"
                  )}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span
                      className={cn(
                        "text-[12.5px] font-medium truncate",
                        isSelected ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
                      )}
                    >
                      {title}
                    </span>
                    <span className="text-[9.5px] font-mono text-[rgb(var(--foreground-muted))]/70 shrink-0">
                      {recency}
                    </span>
                  </div>

                  <div className="flex items-center gap-2 text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                    <span>{session.turn_count} turns</span>
                    {factCount > 0 && (
                      <>
                        <span>·</span>
                        <span className="text-[rgb(var(--accent))]">{factCount} memories</span>
                      </>
                    )}
                  </div>
                </button>
              );
            })
          )}
        </div>
      )}
    </div>
  );
});

MemorySessionRail.displayName = "MemorySessionRail";
