import React, { memo, useCallback, useEffect, useMemo, useState, useRef } from "react";
import {
  Search,
  X,
} from "lucide-react";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  getSessions,
  sortSessionsNewestFirst,
  type SessionRow,
} from "@/services/historyService";
import {
  getIngestionStats,
  type IngestionStats,
  type ObservationRecord,
} from "@/services/memoryService";
import { onMemoryIngestionUpdated, onSessionsChanged } from "@/services/eventsService";
import { useSettingsStore } from "@/store/settingsStore";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
import { TimelineQueueSection } from "./timeline/TimelineQueueSection";
import { SessionAccordionItem } from "./timeline/SessionAccordionItem";

const memoryTimelineScrollTop = { value: 0 };

export interface MemoryTimelineProps {
  facts: ObservationRecord[];
  selectedSessionId: string | null;
  selectedFactId: string | null;
  onSelectSession: (sessionId: string | null) => void;
  onSelectFact: (fact: ObservationRecord) => void;
  onClose?: () => void;
}

export const MemoryTimeline = memo<MemoryTimelineProps>(({
  facts,
  selectedSessionId,
  selectedFactId,
  onSelectSession,
  onSelectFact,
}) => {
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [loadingSessions, setLoadingSessions] = useState(true);
  const [stats, setStats] = useState<IngestionStats | null>(null);
  const [sessionSearchQuery, setSessionSearchQuery] = useState("");
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [expandedSessionId, setExpandedSessionId] = useState<number | null>(() => {
    return selectedSessionId ? Number(selectedSessionId) : null;
  });

  const searchInputRef = useRef<HTMLInputElement>(null);

  const [isLightMode, setIsLightMode] = useState(false);
  useEffect(() => {
    const checkTheme = () => {
      setIsLightMode(document.documentElement.getAttribute("data-theme") === "light");
    };
    checkTheme();
    const obs = new MutationObserver(checkTheme);
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => obs.disconnect();
  }, []);

  const pipelineProcessingEnabled = useSettingsStore(
    (s) => s.settings?.personal_memory?.pipeline_processing_enabled ?? true
  );

  const scrollRef = useRef<HTMLDivElement>(null);
  const scrollTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Persist scroll offset across drawer close/reopen
  const handleScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    if (loadingSessions) return;
    memoryTimelineScrollTop.value = e.currentTarget.scrollTop;
  }, [loadingSessions]);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const raf = requestAnimationFrame(() => {
      el.scrollTop = memoryTimelineScrollTop.value;
    });
    return () => cancelAnimationFrame(raf);
  }, [loadingSessions]);

  useEffect(() => {
    if (isSearchOpen && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isSearchOpen]);

  const fetchStats = useCallback(async () => {
    try {
      const data = await getIngestionStats();
      setStats(data);
    } catch (err) {
      console.warn("[MemoryTimeline] Failed to load ingestion stats:", err);
    }
  }, []);

  const fetchSessionsList = useCallback(async () => {
    try {
      const data = await getSessions();
      setSessions(sortSessionsNewestFirst(data));
    } catch (err) {
      console.warn("[MemoryTimeline] Failed to load sessions:", err);
    } finally {
      setLoadingSessions(false);
    }
  }, []);

  useEffect(() => {
    fetchStats();
    fetchSessionsList();
  }, [fetchStats, fetchSessionsList]);

  useEffect(() => {
    const unlistenStats = onMemoryIngestionUpdated(() => {
      fetchStats();
    });
    const unlistenSessions = onSessionsChanged(() => {
      fetchSessionsList();
    });

    return () => {
      unlistenStats();
      unlistenSessions();
    };
  }, [fetchStats, fetchSessionsList]);

  useEffect(() => {
    if (selectedSessionId) {
      const sIdNum = Number(selectedSessionId);
      setExpandedSessionId(sIdNum);

      if (scrollTimeoutRef.current) clearTimeout(scrollTimeoutRef.current);
      scrollTimeoutRef.current = setTimeout(() => {
        const el = document.getElementById(`session-item-${selectedSessionId}`);
        if (el) {
          el.scrollIntoView({ behavior: "smooth", block: "nearest" });
        }
      }, 120);
    }
    return () => {
      if (scrollTimeoutRef.current) clearTimeout(scrollTimeoutRef.current);
    };
  }, [selectedSessionId]);

  const sessionFactsMap = useMemo(() => {
    const map = new Map<number, ObservationRecord[]>();
    for (const f of facts) {
      const isPersonal =
        f.observation_type === "personal" || f.fact_type === "personal";
      if (!isPersonal && f.session_id !== null) {
        const list = map.get(f.session_id) || [];
        list.push(f);
        map.set(f.session_id, list);
      }
    }
    return map;
  }, [facts]);

  const handleToggleSession = useCallback(
    (sessionId: number) => {
      const sIdStr = String(sessionId);
      if (expandedSessionId === sessionId) {
        setExpandedSessionId(null);
        onSelectSession(null);
      } else {
        setExpandedSessionId(sessionId);
        onSelectSession(sIdStr);
      }
    },
    [expandedSessionId, onSelectSession]
  );

  const filteredSessions = useMemo(() => {
    const q = sessionSearchQuery.trim().toLowerCase();
    if (!q) return sessions;
    return sessions.filter((s) => {
      const t = (s.title || "").toLowerCase();
      const p = (s.project_id || "").toLowerCase();
      const fm = (s.first_message || "").toLowerCase();
      return t.includes(q) || p.includes(q) || fm.includes(q);
    });
  }, [sessions, sessionSearchQuery]);

  const rowVirtualizer = useVirtualizer({
    count: filteredSessions.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 50,
    overscan: 16,
  });

  return (
    <div className="flex-1 min-h-0 flex flex-col px-1.5 pt-3 select-none font-sans text-[rgb(var(--foreground))]">
      <TimelineQueueSection
        stats={stats}
        pipelineProcessingEnabled={pipelineProcessingEnabled}
      />

      <section className="flex flex-col flex-1 min-h-0 pt-3">
        <div className="flex items-center justify-between pb-2.5 px-2 shrink-0">
          <div className="flex items-center gap-1.5 min-w-0">
            <span className="text-[11.5px] font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))]/60">
              {MEMORY_COPY.timeline.sessionsHeader}
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/50">
              ({filteredSessions.length})
            </span>
          </div>

          <div className="flex items-center gap-1">
            {isSearchOpen ? (
              <div className="flex items-center gap-1 animate-in fade-in duration-150">
                <input
                  ref={searchInputRef}
                  type="text"
                  value={sessionSearchQuery}
                  onChange={(e) => setSessionSearchQuery(e.target.value)}
                  placeholder={MEMORY_COPY.timeline.searchSessionsPlaceholder}
                  className="w-36 bg-transparent border-0 border-b border-[rgb(var(--accent))] px-1 py-0.5 text-[11px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none"
                />
                <button
                  type="button"
                  onClick={() => {
                    setSessionSearchQuery("");
                    setIsSearchOpen(false);
                  }}
                  className="p-1 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer bg-transparent border-none"
                >
                  <X size={12} />
                </button>
              </div>
            ) : (
              <Tooltip label={MEMORY_COPY.timeline.searchSessionsPlaceholder} side="bottom">
                <button
                  type="button"
                  onClick={() => setIsSearchOpen(true)}
                  aria-label={MEMORY_COPY.timeline.searchSessionsPlaceholder}
                  className="p-1 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)] cursor-pointer bg-transparent border-none transition-colors"
                >
                  <Search size={13} />
                </button>
              </Tooltip>
            )}
          </div>
        </div>

        <div
          ref={scrollRef}
          onScroll={handleScroll}
          className="flex-1 overflow-y-auto custom-scrollbar flex flex-col gap-0.5 pr-0.5 pt-1 pb-24"
        >
          {loadingSessions ? (
            <div className="flex items-center justify-center p-8">
              <span className="text-[11.5px] font-mono text-[rgb(var(--foreground-muted))] animate-pulse">
                Loading sessions…
              </span>
            </div>
          ) : filteredSessions.length === 0 ? (
            <span className="px-2 py-4 text-[11.5px] text-[rgb(var(--foreground-muted))]/40 italic text-center">
              {sessionSearchQuery ? MEMORY_COPY.timeline.noSessionsFound : "No sessions available"}
            </span>
          ) : (
            <div
              style={{
                height: `${rowVirtualizer.getTotalSize()}px`,
                width: "100%",
                position: "relative",
              }}
            >
              {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                const session = filteredSessions[virtualRow.index];
                const isExpanded = expandedSessionId === session.id;
                const isSelectedSession = selectedSessionId !== null && Number(selectedSessionId) === session.id;
                const sessionFacts = sessionFactsMap.get(session.id) || [];

                return (
                  <div
                    key={session.id}
                    data-index={virtualRow.index}
                    ref={rowVirtualizer.measureElement}
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                  >
                    <SessionAccordionItem
                      session={session}
                      facts={sessionFacts}
                      isExpanded={isExpanded}
                      isSelectedSession={isSelectedSession}
                      selectedFactId={selectedFactId}
                      isLightMode={isLightMode}
                      onToggle={() => handleToggleSession(session.id)}
                      onSelectFact={onSelectFact}
                    />
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </section>
    </div>
  );
});

MemoryTimeline.displayName = "MemoryTimeline";
