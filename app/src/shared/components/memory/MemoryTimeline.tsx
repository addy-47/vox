import { memo, useCallback, useEffect, useMemo, useState, useRef } from "react";
import { createPortal } from "react-dom";
import { Tooltip } from "@/shared/ui/Tooltip";
import {
  MessageSquare,
  MessageSquarePlus,
  Activity,
  Check,
  Clock,
  RotateCcw,
  Search,
  X,
  ArrowDownNarrowWide,
  Filter,
} from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import {
  getSessions,
  resolveSessionTitle,
  formatSessionRecency,
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
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import { cn } from "@/shared/lib/utils";
import {
  getActiveDynamicPalette,
  toMemoryCategory,
  type MemoryCategory,
} from "./memoryGraphTypes";
import { MEMORY_COPY } from "@/data/memoryCopy";

export interface MemoryTimelineProps {
  facts: ObservationRecord[];
  selectedSessionId: string | null;
  selectedFactId: string | null;
  onSelectSession: (sessionId: string | null) => void;
  onSelectFact: (fact: ObservationRecord) => void;
  onClose?: () => void;
}

const CATEGORY_PRIORITY: Record<MemoryCategory, number> = {
  blocker: 1,
  pitfall: 2,
  objective: 3,
  next_step: 4,
  workdone: 5,
  personal: 6,
};

type SortMode = "category" | "time" | "alpha";

interface SessionAccordionItemProps {
  session: SessionRow;
  facts: ObservationRecord[];
  isExpanded: boolean;
  isSelectedSession: boolean;
  selectedFactId: string | null;
  isLightMode: boolean;
  onToggle: () => void;
  onSelectFact: (fact: ObservationRecord) => void;
}

const SessionAccordionItem = memo<SessionAccordionItemProps>(({
  session,
  facts,
  isExpanded,
  isSelectedSession,
  selectedFactId,
  isLightMode,
  onToggle,
  onSelectFact,
}) => {
  const [selectedCategory, setSelectedCategory] = useState<string>("all");
  const [sortBy, setSortBy] = useState<SortMode>("category");
  const [sortMenuOpen, setSortMenuOpen] = useState(false);
  const [filterMenuOpen, setFilterMenuOpen] = useState(false);
  const [sortAnchor, setSortAnchor] = useState<DOMRect | null>(null);
  const [filterAnchor, setFilterAnchor] = useState<DOMRect | null>(null);

  const sortBtnRef = useRef<HTMLButtonElement>(null);
  const filterBtnRef = useRef<HTMLButtonElement>(null);
  const sortMenuRef = useRef<HTMLDivElement>(null);
  const filterMenuRef = useRef<HTMLDivElement>(null);

  const palette = useMemo(() => getActiveDynamicPalette(isLightMode), [isLightMode]);
  const title = resolveSessionTitle(session);
  const recency = formatSessionRecency(session.updated_at);
  const factCount = facts.length;

  const handleToggleSortMenu = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (sortBtnRef.current) {
      setSortAnchor(sortBtnRef.current.getBoundingClientRect());
    }
    setSortMenuOpen((v) => !v);
    setFilterMenuOpen(false);
  };

  const handleToggleFilterMenu = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (filterBtnRef.current) {
      setFilterAnchor(filterBtnRef.current.getBoundingClientRect());
    }
    setFilterMenuOpen((v) => !v);
    setSortMenuOpen(false);
  };

  // Dismiss menus on click outside
  useEffect(() => {
    if (!sortMenuOpen && !filterMenuOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        sortMenuRef.current &&
        !sortMenuRef.current.contains(target) &&
        !sortBtnRef.current?.contains(target)
      ) {
        setSortMenuOpen(false);
      }
      if (
        filterMenuRef.current &&
        !filterMenuRef.current.contains(target) &&
        !filterBtnRef.current?.contains(target)
      ) {
        setFilterMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [sortMenuOpen, filterMenuOpen]);

  // Category counts within this session
  const categoryCounts = useMemo(() => {
    const counts = new Map<MemoryCategory, number>();
    for (const f of facts) {
      const cat = toMemoryCategory(f.fact_type) || "objective";
      counts.set(cat, (counts.get(cat) || 0) + 1);
    }
    return counts;
  }, [facts]);

  const presentCategories = useMemo(() => {
    const cats: MemoryCategory[] = ["objective", "workdone", "blocker", "next_step", "pitfall", "personal"];
    return cats.filter((c) => (categoryCounts.get(c) ?? 0) > 0);
  }, [categoryCounts]);

  // Filter and sort facts
  const displayFacts = useMemo(() => {
    let list = [...facts];
    if (selectedCategory !== "all") {
      list = list.filter((f) => (toMemoryCategory(f.fact_type) || "objective") === selectedCategory);
    }

    if (sortBy === "category") {
      list.sort((a, b) => {
        const catA = toMemoryCategory(a.fact_type) || "objective";
        const catB = toMemoryCategory(b.fact_type) || "objective";
        const pA = CATEGORY_PRIORITY[catA] ?? 10;
        const pB = CATEGORY_PRIORITY[catB] ?? 10;
        if (pA !== pB) return pA - pB;
        return b.created_at - a.created_at;
      });
    } else if (sortBy === "time") {
      list.sort((a, b) => b.created_at - a.created_at);
    } else if (sortBy === "alpha") {
      list.sort((a, b) => a.text.localeCompare(b.text));
    }

    return list;
  }, [facts, selectedCategory, sortBy]);

  return (
    <div
      id={`session-item-${session.id}`}
      className="flex flex-col"
    >
      {/* Session Row — matches SessionPanel UX and styling exactly */}
      <div
        role="button"
        tabIndex={0}
        onClick={onToggle}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") onToggle();
        }}
        className={cn(
          "group relative flex items-center justify-between gap-2 py-2 px-3 rounded-sm text-left cursor-pointer transition-all select-none border border-transparent",
          isSelectedSession
            ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.25)] shadow-sm"
            : "hover:bg-[rgba(var(--foreground),0.04)]"
        )}
      >
        {/* Left: Message Icon (MessageSquare when closed, MessageSquarePlus when open) + Title & Subtitle */}
        <div className="flex items-center gap-2.5 min-w-0 flex-1">
          {isExpanded ? (
            <MessageSquarePlus
              size={14}
              className="shrink-0 text-[rgb(var(--accent))] transition-colors"
            />
          ) : (
            <MessageSquare
              size={14}
              className="shrink-0 text-[rgb(var(--foreground-muted))]/60 group-hover:text-[rgb(var(--foreground))] transition-colors"
            />
          )}

          <div className="flex flex-col min-w-0 flex-1">
            <span
              title={title}
              className={cn(
                "truncate text-[13.5px] font-sans font-medium tracking-normal leading-tight",
                isSelectedSession || isExpanded
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground))]"
              )}
            >
              {title}
            </span>

            {/* Subtitle: ONLY memory count (no turns, no date) with accent color */}
            <span className="text-[11px] font-mono mt-0.5 flex items-center gap-1">
              <span className="text-[rgb(var(--accent))] font-medium">{factCount}</span>
              <span className="text-[rgb(var(--foreground-muted))]/60">memories</span>
            </span>
          </div>
        </div>

        {/* Right side: Recency date when closed, Action Icons (Sort & Filter) when open */}
        {!isExpanded ? (
          <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60 whitespace-nowrap shrink-0">
            {recency}
          </span>
        ) : (
          <div
            className="flex items-center gap-1 shrink-0"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Sort Menu Trigger */}
            <Tooltip label={MEMORY_COPY.timeline.sortLabel} side="bottom">
              <button
                ref={sortBtnRef}
                type="button"
                onClick={handleToggleSortMenu}
                aria-label={MEMORY_COPY.timeline.sortLabel}
                className={cn(
                  "p-1 rounded transition-colors cursor-pointer flex items-center justify-center text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] bg-transparent border-none",
                  (sortMenuOpen || sortBy !== "category") && "text-[rgb(var(--accent))]"
                )}
              >
                <ArrowDownNarrowWide size={13} />
              </button>
            </Tooltip>

            {/* Filter Menu Trigger */}
            <Tooltip label={MEMORY_COPY.timeline.filterLabel} side="bottom">
              <button
                ref={filterBtnRef}
                type="button"
                onClick={handleToggleFilterMenu}
                aria-label={MEMORY_COPY.timeline.filterLabel}
                className={cn(
                  "p-1 rounded transition-colors cursor-pointer flex items-center justify-center text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] bg-transparent border-none",
                  (filterMenuOpen || selectedCategory !== "all") && "text-[rgb(var(--accent))]"
                )}
              >
                <Filter size={12} />
              </button>
            </Tooltip>

            {/* Sort Dropdown Menu (Portaled to document.body opening toward bottom-right) */}
            {sortMenuOpen && sortAnchor && createPortal(
              <div
                ref={sortMenuRef}
                data-context-menu
                style={{
                  position: "fixed",
                  left: `${Math.round(sortAnchor.left)}px`,
                  top: `${Math.round(sortAnchor.bottom + 4)}px`,
                  zIndex: 100,
                }}
                className="min-w-[145px] p-1.5 bg-[rgb(var(--card))] border border-[rgba(var(--border),0.12)] rounded-xl shadow-2xl backdrop-blur-md text-[12px] font-sans flex flex-col gap-0.5 select-none"
              >
                <button
                  type="button"
                  onClick={() => {
                    setSortBy("category");
                    setSortMenuOpen(false);
                  }}
                  className={cn(
                    "w-full px-2.5 py-1.5 rounded-md text-left flex items-center justify-between transition-colors hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer bg-transparent border-none text-[12px]",
                    sortBy === "category" ? "text-[rgb(var(--accent))] font-medium" : "text-[rgb(var(--foreground))]"
                  )}
                >
                  <span>{MEMORY_COPY.timeline.sortCategory}</span>
                  {sortBy === "category" && <Check size={12} />}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setSortBy("time");
                    setSortMenuOpen(false);
                  }}
                  className={cn(
                    "w-full px-2.5 py-1.5 rounded-md text-left flex items-center justify-between transition-colors hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer bg-transparent border-none text-[12px]",
                    sortBy === "time" ? "text-[rgb(var(--accent))] font-medium" : "text-[rgb(var(--foreground))]"
                  )}
                >
                  <span>{MEMORY_COPY.timeline.sortTime}</span>
                  {sortBy === "time" && <Check size={12} />}
                </button>
                <button
                  type="button"
                  onClick={() => {
                    setSortBy("alpha");
                    setSortMenuOpen(false);
                  }}
                  className={cn(
                    "w-full px-2.5 py-1.5 rounded-md text-left flex items-center justify-between transition-colors hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer bg-transparent border-none text-[12px]",
                    sortBy === "alpha" ? "text-[rgb(var(--accent))] font-medium" : "text-[rgb(var(--foreground))]"
                  )}
                >
                  <span>{MEMORY_COPY.timeline.sortAlphabetical}</span>
                  {sortBy === "alpha" && <Check size={12} />}
                </button>
              </div>,
              document.body
            )}

            {/* Filter Dropdown Menu (Portaled to document.body opening toward bottom-right) */}
            {filterMenuOpen && filterAnchor && createPortal(
              <div
                ref={filterMenuRef}
                data-context-menu
                style={{
                  position: "fixed",
                  left: `${Math.round(filterAnchor.left)}px`,
                  top: `${Math.round(filterAnchor.bottom + 4)}px`,
                  zIndex: 100,
                }}
                className="min-w-[155px] p-1.5 bg-[rgb(var(--card))] border border-[rgba(var(--border),0.12)] rounded-xl shadow-2xl backdrop-blur-md text-[12px] font-sans flex flex-col gap-0.5 select-none"
              >
                <button
                  type="button"
                  onClick={() => {
                    setSelectedCategory("all");
                    setFilterMenuOpen(false);
                  }}
                  className={cn(
                    "w-full px-2.5 py-1.5 rounded-md text-left flex items-center justify-between transition-colors hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer bg-transparent border-none text-[12px]",
                    selectedCategory === "all" ? "text-[rgb(var(--accent))] font-medium" : "text-[rgb(var(--foreground))]"
                  )}
                >
                  <span>{MEMORY_COPY.timeline.allCategories} ({facts.length})</span>
                  {selectedCategory === "all" && <Check size={12} />}
                </button>
                {presentCategories.map((catKey) => {
                  const count = categoryCounts.get(catKey) ?? 0;
                  const catColor = palette[catKey]?.main || "#00dbe9";
                  const label = MEMORY_COPY.categories[catKey] || catKey;
                  const isSelected = selectedCategory === catKey;

                  return (
                    <button
                      key={catKey}
                      type="button"
                      onClick={() => {
                        setSelectedCategory(catKey);
                        setFilterMenuOpen(false);
                      }}
                      className={cn(
                        "w-full px-2.5 py-1.5 rounded-md text-left flex items-center justify-between transition-colors hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer bg-transparent border-none text-[12px]",
                        isSelected ? "text-[rgb(var(--foreground))] font-medium" : "text-[rgb(var(--foreground-muted))]"
                      )}
                    >
                      <div className="flex items-center gap-2 min-w-0">
                        <span
                          className="w-1.5 h-1.5 rounded-full shrink-0"
                          style={{ backgroundColor: catColor }}
                        />
                        <span className="truncate">{label} ({count})</span>
                      </div>
                      {isSelected && <Check size={12} className="text-[rgb(var(--accent))] shrink-0" />}
                    </button>
                  );
                })}
              </div>,
              document.body
            )}
          </div>
        )}
      </div>

      {/* Expanded Accordion Body — Clean observations list without left timeline spine or inner tabs */}
      <AnimatePresence initial={false}>
        {isExpanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.16, ease: "easeInOut" }}
            className="overflow-hidden"
          >
            <div className="pl-6 pr-1 pt-1 pb-2 flex flex-col gap-1.5">
              {/* Active filter summary if filtered */}
              {selectedCategory !== "all" && (
                <div className="flex items-center justify-between text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/70 pb-0.5">
                  <span>Filtered: {MEMORY_COPY.categories[selectedCategory as MemoryCategory] || selectedCategory}</span>
                  <button
                    type="button"
                    onClick={() => setSelectedCategory("all")}
                    className="text-[rgb(var(--accent))] hover:underline cursor-pointer bg-transparent border-none p-0"
                  >
                    Reset
                  </button>
                </div>
              )}

              {/* Memory Cards — clean, NO left accent border */}
              {displayFacts.length === 0 ? (
                <div className="py-2 text-[11.5px] font-sans text-[rgb(var(--foreground-muted))]/40 italic">
                  {MEMORY_COPY.timeline.noMemoriesInSession}
                </div>
              ) : (
                displayFacts.map((fact) => {
                  const isFactSelected = selectedFactId === fact.id;
                  const catKey = toMemoryCategory(fact.fact_type) || "objective";
                  const catColor = palette[catKey]?.main || "#00dbe9";
                  const label = MEMORY_COPY.categories[catKey] || catKey;

                  return (
                    <div
                      key={fact.id}
                      role="button"
                      tabIndex={0}
                      onClick={(e) => {
                        e.stopPropagation();
                        onSelectFact(fact);
                      }}
                      onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                          e.stopPropagation();
                          onSelectFact(fact);
                        }
                      }}
                      className={cn(
                        "p-2.5 rounded-lg border transition-all cursor-pointer flex flex-col gap-1 select-text",
                        isFactSelected
                          ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] shadow-sm"
                          : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--border),0.06)] hover:bg-[rgba(var(--foreground),0.04)]"
                      )}
                    >
                      {/* Card Header: Category Token & Turn Reference */}
                      <div className="flex items-center justify-between text-[10.5px] font-mono">
                        <span
                          className="font-bold uppercase tracking-wider"
                          style={{ color: catColor }}
                        >
                          {label}
                        </span>
                        <span className="text-[rgb(var(--foreground-muted))]/60">
                          {new Date(fact.created_at).toLocaleDateString()}
                        </span>
                      </div>

                      {/* Fact Content Text */}
                      <p className="text-[12.5px] font-sans text-[rgb(var(--foreground))] leading-relaxed line-clamp-4 m-0">
                        {fact.text}
                      </p>
                    </div>
                  );
                })
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
});

SessionAccordionItem.displayName = "SessionAccordionItem";

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

  const { containerRef } = useLenisScrollContainer<HTMLDivElement>();
  const scrollTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Focus search input when opened
  useEffect(() => {
    if (isSearchOpen && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isSearchOpen]);

  // Fetch aggregate ingestion statistics
  const fetchStats = useCallback(async () => {
    try {
      const data = await getIngestionStats();
      setStats(data);
    } catch (err) {
      console.warn("[MemoryTimeline] Failed to load ingestion stats:", err);
    }
  }, []);

  // Fetch session list
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

  // Subscribe to live IPC events for ingestion and sessions
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

  // Deep-link: Sync expanded state and scroll focused session into view
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

  // Map session ID to non-personal observations only (!personal)
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

  const pendingCount = stats?.pending ?? 0;
  const processingCount = stats?.processing ?? 0;
  const completedCount = stats?.completed ?? 0;
  const failedCount = stats?.failed ?? 0;

  // Filter sessions by search query
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

  return (
    <div className="flex-1 min-h-0 flex flex-col pt-3 pb-16 select-none font-sans text-[rgb(var(--foreground))]">
      {/* ── TOP SECTION: Ingestion Pipeline / Memory Queue (Generous Breathing Room) ── */}
      <section className="shrink-0 flex flex-col gap-3.5 px-4.5 pt-1 pb-4.5 border-b border-[rgba(var(--border),0.06)]">
        {/* Header: Title + Pipeline State */}
        <div className="flex items-center justify-between py-1">
          <span className="text-[12px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/60">
            {MEMORY_COPY.timeline.ingestionTitle}
          </span>
          {!pipelineProcessingEnabled ? (
            <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
              <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--foreground-muted))]/40" />
              <span>Disabled</span>
            </div>
          ) : processingCount > 0 ? (
            <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgb(var(--accent))]">
              <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
              <span>Processing</span>
            </div>
          ) : pendingCount > 0 ? (
            <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgba(var(--accent),0.85)]">
              <Clock size={11} />
              <span>Queued ({pendingCount})</span>
            </div>
          ) : (
            <div className="flex items-center gap-1.5 text-[11px] font-mono text-[rgba(var(--accent),0.65)]">
              <Check size={11} strokeWidth={2.5} className="text-[rgba(var(--accent),0.75)]" />
              <span>Up to date</span>
            </div>
          )}
        </div>

        {/* Continuous Pipeline Timeline Spine (Perfect column alignment, zero drift) */}
        <div
          className={cn(
            "flex flex-col px-0.5 pt-1 pb-0.5 transition-all duration-300",
            !pipelineProcessingEnabled && "opacity-35 grayscale pointer-events-none"
          )}
        >
          {/* Stage 1: Ingestion Queue (Pending) */}
          <div className="flex gap-2.5 items-stretch min-w-0">
            <div className="flex flex-col items-center shrink-0 w-4.5">
              <div className="w-4.5 h-4.5 rounded-full border border-[rgba(var(--accent),0.35)] bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10 text-[rgb(var(--foreground-muted))]/70">
                <Clock size={9} strokeWidth={2} />
              </div>
              <div className="w-[1.5px] flex-1 my-0.5 bg-[rgba(var(--accent),0.2)]" />
            </div>
            <div className="flex-1 min-w-0 pb-3 flex items-start justify-between">
              <div className="flex flex-col min-w-0 pr-2">
                <span className="text-[13px] font-sans font-medium leading-snug text-[rgb(var(--foreground))]">
                  {MEMORY_COPY.timeline.pipelineStagePending}
                </span>
                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/55 mt-0.5 leading-tight">
                  {MEMORY_COPY.timeline.pipelineStagePendingDesc}
                </span>
              </div>
              <span className="font-mono text-[11.5px] text-[rgb(var(--foreground-muted))]/70 shrink-0 pt-0.5">
                {pendingCount}
              </span>
            </div>
          </div>

          {/* Stage 2: Semantic Extraction (Processing) */}
          <div className="flex gap-2.5 items-stretch min-w-0">
            <div className="flex flex-col items-center shrink-0 w-4.5">
              <div
                className={cn(
                  "w-4.5 h-4.5 rounded-full border bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10 transition-all",
                  processingCount > 0
                    ? "border-[rgb(var(--accent))] text-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.45)]"
                    : "border-[rgba(var(--accent),0.35)] text-[rgb(var(--foreground-muted))]/60"
                )}
              >
                <Activity size={9} className={cn(processingCount > 0 && "animate-pulse")} strokeWidth={2.5} />
              </div>
              <div className={cn("w-[1.5px] flex-1 my-0.5", processingCount > 0 ? "bg-[rgb(var(--accent))]/40" : "bg-[rgba(var(--accent),0.2)]")} />
            </div>
            <div className="flex-1 min-w-0 pb-3 flex items-start justify-between">
              <div className="flex flex-col min-w-0 pr-2">
                <span
                  className={cn(
                    "text-[13px] font-sans font-medium leading-snug",
                    processingCount > 0 ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]"
                  )}
                >
                  {MEMORY_COPY.timeline.pipelineStageProcessing}
                </span>
                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/55 mt-0.5 leading-tight">
                  {MEMORY_COPY.timeline.pipelineStageProcessingDesc}
                </span>
              </div>
              <span
                className={cn(
                  "font-mono text-[11.5px] shrink-0 pt-0.5",
                  processingCount > 0 ? "text-[rgb(var(--accent))] font-bold" : "text-[rgb(var(--foreground-muted))]/70"
                )}
              >
                {processingCount}
              </span>
            </div>
          </div>

          {/* Stage 3: Graph Indexed (Completed) */}
          <div className="flex gap-2.5 items-stretch min-w-0">
            <div className="flex flex-col items-center shrink-0 w-4.5">
              <div className="w-4.5 h-4.5 rounded-full border border-[rgb(var(--accent))] bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10 text-[rgb(var(--accent))]">
                <Check size={9} strokeWidth={2.5} />
              </div>
              {failedCount > 0 && (
                <div className="w-[1.5px] flex-1 my-0.5 bg-[rgba(var(--accent),0.2)]" />
              )}
            </div>
            <div className={cn("flex-1 min-w-0 flex items-start justify-between", failedCount > 0 && "pb-3")}>
              <div className="flex flex-col min-w-0 pr-2">
                <span className="text-[13px] font-sans font-medium leading-snug text-[rgb(var(--foreground))]">
                  {MEMORY_COPY.timeline.pipelineStageCompleted}
                </span>
                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/55 mt-0.5 leading-tight">
                  {MEMORY_COPY.timeline.pipelineStageCompletedDesc}
                </span>
              </div>
              <span className="font-mono text-[11.5px] font-medium text-[rgb(var(--accent))] shrink-0 pt-0.5">
                {completedCount}
              </span>
            </div>
          </div>

          {/* Stage 4: Failed (if present) */}
          {failedCount > 0 && (
            <div className="flex gap-2.5 items-stretch min-w-0">
              <div className="flex flex-col items-center shrink-0 w-4.5">
                <div className="w-4.5 h-4.5 rounded-full border border-rose-400 bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10 text-rose-400">
                  <RotateCcw size={9} strokeWidth={2} />
                </div>
              </div>
              <div className="flex-1 min-w-0 flex items-start justify-between">
                <div className="flex flex-col min-w-0 pr-2">
                  <span className="text-[13px] font-sans font-medium leading-snug text-rose-400">
                    {MEMORY_COPY.timeline.pipelineStageFailed}
                  </span>
                  <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/55 mt-0.5 leading-tight">
                    {MEMORY_COPY.timeline.pipelineStageFailedDesc}
                  </span>
                </div>
                <span className="font-mono text-[11.5px] font-medium text-rose-400 shrink-0 pt-0.5">
                  {failedCount}
                </span>
              </div>
            </div>
          )}
        </div>
      </section>

      {/* ── BOTTOM SECTION: Sessions (Clean list, NO timeline on left) ── */}
      <section className="flex flex-col flex-1 min-h-0 px-3 pt-3">
        {/* Sessions Header: Title with count immediate right + Expandable Search Icon on right */}
        <div className="flex items-center justify-between pb-2.5 px-1 shrink-0">
          <div className="flex items-center gap-1.5 min-w-0">
            <span className="text-[11.5px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/60">
              {MEMORY_COPY.timeline.sessionsHeader}
            </span>
            <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/50">
              ({filteredSessions.length})
            </span>
          </div>

          {/* Expandable Underline Search Trigger */}
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
              <button
                type="button"
                onClick={() => setIsSearchOpen(true)}
                title="Search sessions"
                className="p-1 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.04)] cursor-pointer bg-transparent border-none transition-colors"
              >
                <Search size={13} />
              </button>
            )}
          </div>
        </div>

        {/* Scrollable Sessions List (NO timeline border on left) */}
        <div
          ref={containerRef}
          className="flex-1 overflow-y-auto custom-scrollbar flex flex-col gap-0.5 pr-0.5 pt-1"
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
            filteredSessions.map((session) => {
              const isExpanded = expandedSessionId === session.id;
              const isSelectedSession = selectedSessionId !== null && Number(selectedSessionId) === session.id;
              const sessionFacts = sessionFactsMap.get(session.id) || [];

              return (
                <SessionAccordionItem
                  key={session.id}
                  session={session}
                  facts={sessionFacts}
                  isExpanded={isExpanded}
                  isSelectedSession={isSelectedSession}
                  selectedFactId={selectedFactId}
                  isLightMode={isLightMode}
                  onToggle={() => handleToggleSession(session.id)}
                  onSelectFact={onSelectFact}
                />
              );
            })
          )}
        </div>
      </section>
    </div>
  );
});

MemoryTimeline.displayName = "MemoryTimeline";
