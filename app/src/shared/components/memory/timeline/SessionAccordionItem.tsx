import { memo, useState, useRef, useMemo } from "react";
import {
  MessageSquare,
  MessageSquarePlus,
  ArrowDownNarrowWide,
  Filter,
  Check,
  ChevronDown,
  ChevronRight,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ContextMenuShell } from "@/shared/ui/ContextMenuShell";
import {
  resolveSessionTitle,
  formatSessionRecency,
  type SessionRow,
} from "@/services/persistenceService";
import { type ObservationRecord } from "@/services/memoryService";
import {
  getActiveDynamicPalette,
  toMemoryCategory,
  getCollectionIcon,
  type MemoryCategory,
} from "../memoryGraphTypes";
import { MEMORY_COPY } from "@/data/memoryCopy";

const CATEGORY_PRIORITY: Record<MemoryCategory, number> = {
  blocker: 1,
  pitfall: 2,
  objective: 3,
  next_step: 4,
  workdone: 5,
  personal: 6,
};

const CATEGORY_ORDER: readonly MemoryCategory[] = [
  "blocker",
  "pitfall",
  "objective",
  "next_step",
  "workdone",
  "personal",
];

type SortMode = "category" | "time" | "alpha";

export interface SessionAccordionItemProps {
  session: SessionRow;
  facts: ObservationRecord[];
  isExpanded: boolean;
  isSelectedSession: boolean;
  selectedFactId: string | null;
  isLightMode: boolean;
  onToggle: (sessionId: number) => void;
  onSelectFact: (fact: ObservationRecord) => void;
}

export const SessionAccordionItem = memo<SessionAccordionItemProps>(({
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

  // Set of open category accordions within this session (closed by default)
  const [expandedCategories, setExpandedCategories] = useState<Set<MemoryCategory>>(() => {
    return new Set<MemoryCategory>();
  });

  const sortBtnRef = useRef<HTMLButtonElement>(null);
  const filterBtnRef = useRef<HTMLButtonElement>(null);

  const palette = useMemo(() => (isExpanded ? getActiveDynamicPalette(isLightMode) : {}), [isLightMode, isExpanded]);
  const title = resolveSessionTitle(session);
  const recency = formatSessionRecency(session.updated_at);
  const factCount = facts.length;

  const toggleCategory = (cat: MemoryCategory) => {
    setExpandedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(cat)) next.delete(cat);
      else next.add(cat);
      return next;
    });
  };

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

  const categoryCounts = useMemo(() => {
    if (!isExpanded) return new Map<MemoryCategory, number>();
    const counts = new Map<MemoryCategory, number>();
    for (const f of facts) {
      const cat = toMemoryCategory(f.fact_type) || "objective";
      counts.set(cat, (counts.get(cat) || 0) + 1);
    }
    return counts;
  }, [facts, isExpanded]);

  const presentCategories = useMemo(() => {
    if (!isExpanded) return [];
    return CATEGORY_ORDER.filter((c) => (categoryCounts.get(c) ?? 0) > 0);
  }, [categoryCounts, isExpanded]);

  const displayFacts = useMemo(() => {
    if (!isExpanded) return [];
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
  }, [facts, selectedCategory, sortBy, isExpanded]);

  // Group facts into category sections
  const categoryGroups = useMemo(() => {
    if (!isExpanded) return [];
    const map = new Map<MemoryCategory, ObservationRecord[]>();
    for (const cat of CATEGORY_ORDER) {
      map.set(cat, []);
    }
    for (const fact of displayFacts) {
      const cat = toMemoryCategory(fact.fact_type) || "objective";
      const list = map.get(cat) || [];
      list.push(fact);
      map.set(cat, list);
    }
    return CATEGORY_ORDER
      .filter((cat) => (map.get(cat)?.length ?? 0) > 0)
      .map((cat) => ({
        category: cat,
        label: MEMORY_COPY.categories[cat] || cat,
        color: (palette as Record<string, { main: string }>)[cat]?.main || "#00dbe9",
        facts: map.get(cat)!,
      }));
  }, [displayFacts, palette, isExpanded]);

  return (
    <div
      id={`session-item-${session.id}`}
      className={cn(
        "flex flex-col rounded-sm transition-colors border",
        isSelectedSession
          ? "bg-[rgba(var(--accent),0.06)] border-[rgba(var(--accent),0.25)] shadow-sm"
          : "border-transparent"
      )}
    >
      <div
        role="button"
        tabIndex={0}
        onClick={() => onToggle(session.id)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") onToggle(session.id);
        }}
        className={cn(
          "group relative flex items-center justify-between gap-2 py-2 px-2 rounded-sm text-left cursor-pointer transition-colors select-none",
          isSelectedSession
            ? ""
            : "hover:bg-[rgba(var(--foreground),0.04)]"
        )}
      >
        <div className="flex items-start gap-2.5 min-w-0 flex-1">
          {isExpanded ? (
            <MessageSquarePlus
              size={14}
              className="shrink-0 text-[rgb(var(--accent))] transition-colors mt-0.5"
            />
          ) : (
            <MessageSquare
              size={14}
              className="shrink-0 text-[rgb(var(--foreground-muted))]/60 group-hover:text-[rgb(var(--foreground))] transition-colors mt-0.5"
            />
          )}

          <div className="flex flex-col min-w-0 flex-1">
            <span
              className={cn(
                "text-[13.5px] font-sans font-medium tracking-normal leading-tight block",
                isExpanded ? "whitespace-normal break-words" : "truncate",
                isSelectedSession || isExpanded
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground))]"
              )}
            >
              {title}
            </span>

            <span className="text-[11px] font-mono mt-0.5 flex items-center gap-1">
              <span className="text-[rgb(var(--accent))] font-medium">{factCount}</span>
              <span className="text-[rgb(var(--foreground-muted))]/60">{MEMORY_COPY.timeline.observationsCount}</span>
            </span>
          </div>
        </div>

        {!isExpanded ? (
          <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60 whitespace-nowrap shrink-0">
            {recency}
          </span>
        ) : (
          <div
            className="flex items-center gap-1 shrink-0"
            onClick={(e) => e.stopPropagation()}
          >
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

            {sortMenuOpen && (
              <ContextMenuShell
                open={sortMenuOpen}
                onClose={() => setSortMenuOpen(false)}
                anchorRect={sortAnchor}
                triggerRef={sortBtnRef}
                width={145}
              >
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
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
                  role="menuitem"
                  tabIndex={-1}
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
                  role="menuitem"
                  tabIndex={-1}
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
              </ContextMenuShell>
            )}

            {filterMenuOpen && (
              <ContextMenuShell
                open={filterMenuOpen}
                onClose={() => setFilterMenuOpen(false)}
                anchorRect={filterAnchor}
                triggerRef={filterBtnRef}
                width={155}
              >
                <button
                  type="button"
                  role="menuitem"
                  tabIndex={-1}
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
                  const catColor = (palette as Record<string, { main: string }>)[catKey]?.main || "#00dbe9";
                  const label = MEMORY_COPY.categories[catKey] || catKey;
                  const isSelected = selectedCategory === catKey;

                  return (
                    <button
                      key={catKey}
                      type="button"
                      role="menuitem"
                      tabIndex={-1}
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
              </ContextMenuShell>
            )}
          </div>
        )}
      </div>

      {isExpanded && (
        <div className="pl-6 pr-2 pt-0.5 pb-2 flex flex-col animate-in fade-in duration-150">
          {selectedCategory !== "all" && (
            <div className="flex items-center justify-between text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/70 pb-1 px-1">
              <span>{MEMORY_COPY.timeline.filteredPrefix} {MEMORY_COPY.categories[selectedCategory as MemoryCategory] || selectedCategory}</span>
              <button
                type="button"
                onClick={() => setSelectedCategory("all")}
                className="text-[rgb(var(--accent))] hover:underline cursor-pointer bg-transparent border-none p-0"
              >
                {MEMORY_COPY.timeline.resetFilter}
              </button>
            </div>
          )}

          {categoryGroups.length === 0 ? (
            <div className="py-2 px-1 text-[11.5px] font-sans text-[rgb(var(--foreground-muted))]/40 italic">
              {MEMORY_COPY.timeline.noMemoriesInSession}
            </div>
          ) : (
            /* Clean nested observation categories matching project accordion style */
            <div className="max-h-[340px] flex flex-col overflow-y-auto overscroll-contain custom-scrollbar gap-0.5">
              {categoryGroups.map((group) => {
                const isOpen = expandedCategories.has(group.category);
                const Icon = getCollectionIcon(group.category);

                return (
                  <div key={group.category} className="flex flex-col">
                    {/* Clean Category Sub-Accordion Header */}
                    <button
                      type="button"
                      onClick={() => toggleCategory(group.category)}
                      className="w-full flex items-center justify-between px-2 py-1.5 rounded-sm text-left hover:bg-[rgba(var(--foreground),0.04)] transition-colors cursor-pointer select-none border-none bg-transparent"
                    >
                      <div className="flex items-center gap-2 min-w-0">
                        {isOpen ? (
                          <ChevronDown size={13} className="text-[rgb(var(--foreground-muted))] shrink-0" />
                        ) : (
                          <ChevronRight size={13} className="text-[rgb(var(--foreground-muted))] shrink-0" />
                        )}
                        <Icon size={13} className="shrink-0 text-[rgb(var(--foreground-muted))]/70 group-hover:text-[rgb(var(--foreground))]" />
                        <span className="text-[12.5px] font-medium text-[rgb(var(--foreground))] tracking-normal truncate">
                          {group.label}
                        </span>
                      </div>
                      <span className="text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/50 px-1 py-0.2 rounded bg-[rgba(var(--foreground),0.04)]">
                        {group.facts.length}
                      </span>
                    </button>

                    {/* Bulleted List of Observations */}
                    {isOpen && (
                      <ul className="pl-6 pr-1 py-0.5 space-y-0.5 list-none m-0">
                        {group.facts.map((fact) => {
                          const isFactSelected = selectedFactId === fact.id;
                          return (
                            <li
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
                                "group flex items-start gap-2 px-2 py-1.5 rounded-sm transition-all cursor-pointer text-left select-text",
                                isFactSelected
                                  ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-medium shadow-sm"
                                  : "hover:bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground))]"
                              )}
                            >
                              {/* Clean subtle bullet */}
                              <span className="w-1 h-1 rounded-full mt-2 shrink-0 bg-[rgb(var(--foreground-muted))]/40 group-hover:bg-[rgb(var(--foreground))]" />

                              {/* Fact text and timestamp */}
                              <div className="flex-1 min-w-0 flex items-start justify-between gap-2">
                                <span className="text-[12px] leading-relaxed break-words font-sans">
                                  {fact.text}
                                </span>
                                <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/40 shrink-0 pt-0.5 whitespace-nowrap">
                                  {new Date(fact.created_at).toLocaleDateString(undefined, {
                                    month: "short",
                                    day: "numeric",
                                  })}
                                </span>
                              </div>
                            </li>
                          );
                        })}
                      </ul>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}
    </div>
  );
});

SessionAccordionItem.displayName = "SessionAccordionItem";
