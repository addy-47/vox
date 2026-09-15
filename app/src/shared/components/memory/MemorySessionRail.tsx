import { useState, useEffect, useMemo, useCallback, memo } from "react";
import {
  Search,
  X,
  ArrowLeft,
  ChevronDown,
  GitBranch,
  Layers,
  Sparkles,
  MessageSquare,
} from "lucide-react";
import {
  getSessions,
  resolveSessionTitle,
  formatSessionRecency,
  sortSessionsNewestFirst,
  type SessionRow,
} from "@/services/historyService";
import { FactRecord } from "@/services/memoryService";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { cn } from "@/shared/lib/utils";
import {
  getActiveDynamicPalette,
  getCollectionIcon,
  type MemoryCategory,
} from "./memoryGraphTypes";

interface MemorySessionRailProps {
  facts: FactRecord[];
  selectedSessionId: string | null;
  selectedFactId: string | null;
  onSelectSession: (sessionId: string | null) => void;
  onSelectFact: (fact: FactRecord) => void;
  onClose?: () => void;
  isLightMode?: boolean;
}

export const MemorySessionRail = memo<MemorySessionRailProps>(({
  facts,
  selectedSessionId,
  selectedFactId,
  onSelectSession,
  onSelectFact,
  isLightMode = false,
}) => {
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [filterQuery, setFilterQuery] = useState("");
  const [activeDrillSession, setActiveDrillSession] = useState<SessionRow | null>(null);
  const [expandedCompactions, setExpandedCompactions] = useState<Set<number>>(new Set());

  const palette = useMemo(() => getActiveDynamicPalette(isLightMode), [isLightMode]);

  // Load sessions once
  useEffect(() => {
    let isMounted = true;
    getSessions()
      .then((data) => {
        if (isMounted) {
          const sorted = sortSessionsNewestFirst(data);
          setSessions(sorted);
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

  // Map session ID to facts count
  const sessionFactCounts = useMemo(() => {
    const map = new Map<number, number>();
    for (const f of facts) {
      if (f.session_id !== null) {
        map.set(f.session_id, (map.get(f.session_id) || 0) + 1);
      }
    }
    return map;
  }, [facts]);

  // Sync active drill session when selectedSessionId changes from external source
  useEffect(() => {
    if (selectedSessionId && sessions.length > 0) {
      const match = sessions.find((s) => String(s.id) === selectedSessionId);
      if (match && (!activeDrillSession || activeDrillSession.id !== match.id)) {
        setActiveDrillSession(match);
      }
    }
  }, [selectedSessionId, sessions, activeDrillSession]);

  // Filtered session list
  const filteredSessions = useMemo(() => {
    const q = filterQuery.trim().toLowerCase();
    if (!q) return sessions;
    return sessions.filter((s) => {
      const title = resolveSessionTitle(s).toLowerCase();
      const idMatch = String(s.id).includes(q);
      return title.includes(q) || idMatch;
    });
  }, [sessions, filterQuery]);

  // Compaction groups for active drilled session
  const compactionGroups = useMemo(() => {
    if (!activeDrillSession) return [];
    const sessionFacts = facts.filter((f) => f.session_id === activeDrillSession.id);
    const groupsMap = new Map<number, FactRecord[]>();
    for (const f of sessionFacts) {
      const list = groupsMap.get(f.compaction_id) || [];
      list.push(f);
      groupsMap.set(f.compaction_id, list);
    }
    return Array.from(groupsMap.entries())
      .map(([cId, fList]) => ({ compactionId: cId, facts: fList }))
      .sort((a, b) => b.compactionId - a.compactionId);
  }, [activeDrillSession, facts]);

  // Expand latest compaction by default when entering drill view
  useEffect(() => {
    if (activeDrillSession && compactionGroups.length > 0) {
      setExpandedCompactions(new Set([compactionGroups[0].compactionId]));
    }
  }, [activeDrillSession, compactionGroups]);

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

  const toggleCompaction = useCallback((cId: number) => {
    setExpandedCompactions((prev) => {
      const next = new Set(prev);
      if (next.has(cId)) {
        next.delete(cId);
      } else {
        next.add(cId);
      }
      return next;
    });
  }, []);

  return (
    <div className="flex flex-col h-full w-full bg-transparent overflow-hidden select-none">
      {/* ── View 1: Level 2 Compactions Drill-Down ── */}
      {activeDrillSession ? (
        <div className="flex flex-col h-full overflow-hidden">
          {/* Top Drill Bar */}
          <div className="px-4 py-3 shrink-0 border-b border-[rgba(var(--border),0.12)] bg-[rgba(var(--background),0.35)] flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <button
                type="button"
                onClick={handleBackToSessions}
                className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--accent))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                <ArrowLeft size={13} />
                <span>{MEMORY_COPY.backToSessions}</span>
              </button>

              <span className="text-[10px] font-mono px-2 py-0.5 rounded-full bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))]">
                #{activeDrillSession.id}
              </span>
            </div>

            <div className="flex flex-col">
              <h3 className="text-[13px] font-semibold text-[rgb(var(--foreground))] truncate">
                {resolveSessionTitle(activeDrillSession)}
              </h3>
              <div className="flex items-center gap-2 mt-1 text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                <span>{formatSessionRecency(activeDrillSession.updated_at)}</span>
                <span>•</span>
                <span>{activeDrillSession.turn_count} {MEMORY_COPY.turnsLabel.toLowerCase()}</span>
                <span>•</span>
                <span>{sessionFactCounts.get(activeDrillSession.id) ?? 0} {MEMORY_COPY.factsLabel.toLowerCase()}</span>
              </div>
            </div>
          </div>

          {/* Compaction Accordions List */}
          <div className="flex-1 overflow-y-auto p-3 space-y-2.5 scrollbar-thin scrollbar-thumb-[rgba(var(--foreground),0.15)] scrollbar-track-transparent">
            {compactionGroups.length === 0 ? (
              <div className="flex flex-col items-center justify-center p-8 text-center">
                <Sparkles size={24} className="text-[rgb(var(--foreground-muted))] opacity-40 mb-2" />
                <p className="text-[12px] font-mono text-[rgb(var(--foreground-muted))]">
                  {MEMORY_COPY.noCompactions}
                </p>
              </div>
            ) : (
              compactionGroups.map((group) => {
                const isExpanded = expandedCompactions.has(group.compactionId);
                return (
                  <div
                    key={group.compactionId}
                    className="rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.55)] backdrop-blur-md overflow-hidden transition-all"
                  >
                    {/* Accordion Header */}
                    <button
                      type="button"
                      onClick={() => toggleCompaction(group.compactionId)}
                      className="w-full flex items-center justify-between px-3.5 py-2.5 hover:bg-[rgba(var(--foreground),0.04)] transition-colors cursor-pointer text-left"
                    >
                      <div className="flex items-center gap-2">
                        <div className="p-1 rounded-md bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))]">
                          <Layers size={13} />
                        </div>
                        <span className="text-[11px] font-mono font-bold text-[rgb(var(--foreground))]">
                          {MEMORY_COPY.compactionLabel} #{group.compactionId}
                        </span>
                      </div>

                      <div className="flex items-center gap-2">
                        <span className="text-[10px] font-mono px-2 py-0.5 rounded-full bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]">
                          {group.facts.length} {MEMORY_COPY.factsLabel.toLowerCase()}
                        </span>
                        <ChevronDown
                          size={13}
                          className={cn(
                            "text-[rgb(var(--foreground-muted))] transition-transform duration-200",
                            isExpanded && "rotate-180"
                          )}
                        />
                      </div>
                    </button>

                    {/* Accordion Facts List */}
                    {isExpanded && (
                      <div className="p-2 border-t border-[rgba(var(--border),0.08)] bg-[rgba(var(--background),0.25)] space-y-1.5">
                        {group.facts.map((fact) => {
                          const isFactSelected = selectedFactId === fact.id;
                          const col = palette[fact.fact_type as MemoryCategory] ?? palette.objective;
                          const Icon = getCollectionIcon(fact.fact_type);
                          const catLabel = MEMORY_COPY.categories[fact.fact_type as MemoryCategory] || fact.fact_type;

                          return (
                            <button
                              key={fact.id}
                              type="button"
                              onClick={() => onSelectFact(fact)}
                              className={cn(
                                "group w-full text-left p-2.5 rounded-xl border transition-all cursor-pointer flex flex-col gap-1.5",
                                isFactSelected
                                  ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] shadow-sm"
                                  : "border-[rgba(var(--border),0.08)] bg-[rgba(var(--card),0.4)] hover:border-[rgba(var(--accent),0.3)] hover:bg-[rgba(var(--card),0.8)]"
                              )}
                            >
                              <div className="flex items-center justify-between">
                                <div className="flex items-center gap-1.5">
                                  <span
                                    className="w-2 h-2 rounded-full shrink-0"
                                    style={{ background: col.main }}
                                  />
                                  <span
                                    className="text-[10px] font-mono uppercase font-semibold"
                                    style={{ color: col.main }}
                                  >
                                    {catLabel}
                                  </span>
                                </div>
                                <Icon size={12} className="text-[rgb(var(--foreground-muted))] opacity-70 group-hover:text-[rgb(var(--accent))] transition-colors" />
                              </div>

                              <p className="text-[11px] font-sans text-[rgb(var(--foreground))] line-clamp-3 leading-relaxed m-0">
                                {fact.text}
                              </p>
                            </button>
                          );
                        })}
                      </div>
                    )}
                  </div>
                );
              })
            )}
          </div>
        </div>
      ) : (
        /* ── View 2: Level 1 Sessions List ── */
        <div className="flex flex-col h-full overflow-hidden">
          {/* Search Filter Header */}
          <div className="px-3.5 py-2.5 shrink-0 border-b border-[rgba(var(--border),0.12)] bg-[rgba(var(--background),0.3)]">
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.10)] focus-within:border-[rgba(var(--accent),0.4)] transition-colors">
              <Search size={13} className="text-[rgb(var(--foreground-muted))] shrink-0" />
              <input
                type="text"
                value={filterQuery}
                onChange={(e) => setFilterQuery(e.target.value)}
                placeholder={MEMORY_COPY.filterSessionsPlaceholder}
                className="w-full bg-transparent text-[11.5px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))] focus:outline-none"
              />
              {filterQuery && (
                <button
                  type="button"
                  onClick={() => setFilterQuery("")}
                  className="p-0.5 rounded text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] cursor-pointer"
                >
                  <X size={12} />
                </button>
              )}
            </div>
          </div>

          {/* Sessions List */}
          <div className="flex-1 overflow-y-auto p-3 space-y-2 scrollbar-thin scrollbar-thumb-[rgba(var(--foreground),0.15)] scrollbar-track-transparent">
            {loading ? (
              <div className="flex items-center justify-center p-8">
                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] animate-pulse">
                  Loading sessions…
                </span>
              </div>
            ) : filteredSessions.length === 0 ? (
              <div className="flex flex-col items-center justify-center p-8 text-center">
                <MessageSquare size={24} className="text-[rgb(var(--foreground-muted))] opacity-40 mb-2" />
                <p className="text-[12px] font-mono text-[rgb(var(--foreground))] font-semibold">
                  {MEMORY_COPY.noSessions}
                </p>
                <p className="text-[11px] text-[rgb(var(--foreground-muted))] mt-1">
                  {MEMORY_COPY.noSessionsDesc}
                </p>
              </div>
            ) : (
              filteredSessions.map((session) => {
                const sIdStr = String(session.id);
                const isSelected = selectedSessionId === sIdStr;
                const factCount = sessionFactCounts.get(session.id) ?? 0;
                const title = resolveSessionTitle(session);
                const recency = formatSessionRecency(session.updated_at);

                return (
                  <button
                    key={session.id}
                    type="button"
                    onClick={() => handleSelectSessionCard(session)}
                    className={cn(
                      "group w-full text-left p-3 rounded-2xl border transition-all cursor-pointer flex flex-col gap-2",
                      isSelected
                        ? "border-[rgba(var(--accent),0.55)] bg-[rgba(var(--accent),0.12)] shadow-md ring-1 ring-[rgba(var(--accent),0.3)]"
                        : "border-[rgba(var(--border),0.10)] bg-[rgba(var(--card),0.5)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.85)]"
                    )}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="flex items-center gap-2 min-w-0">
                        <div
                          className={cn(
                            "p-1.5 rounded-lg transition-colors shrink-0",
                            isSelected
                              ? "bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))]"
                              : "bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--accent))]"
                          )}
                        >
                          <GitBranch size={13} />
                        </div>
                        <span
                          className={cn(
                            "text-[12.5px] font-semibold truncate transition-colors",
                            isSelected ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--accent))]"
                          )}
                        >
                          {title}
                        </span>
                      </div>

                      <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 shrink-0 mt-0.5">
                        {recency}
                      </span>
                    </div>

                    {/* Meta Row: Turns + Facts Badges */}
                    <div className="flex items-center justify-between pt-1 border-t border-[rgba(var(--border),0.06)] text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                      <div className="flex items-center gap-1.5">
                        <span className="px-1.5 py-0.5 rounded bg-[rgba(var(--foreground),0.05)]">
                          {session.turn_count} {MEMORY_COPY.turnsLabel.toLowerCase()}
                        </span>
                        <span className="px-1.5 py-0.5 rounded bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)] font-semibold">
                          {factCount} {MEMORY_COPY.factsLabel.toLowerCase()}
                        </span>
                      </div>

                      <span className="text-[9px] opacity-50">
                        #{session.id}
                      </span>
                    </div>
                  </button>
                );
              })
            )}
          </div>
        </div>
      )}
    </div>
  );
});

MemorySessionRail.displayName = "MemorySessionRail";
