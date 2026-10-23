import { useState, useMemo, useRef, useEffect, useCallback, memo } from "react";
import { Search, Ghost } from "lucide-react";
import { ObservationRecord } from "@/services/memoryService";
import { getActiveDynamicPalette, toMemoryCategory } from "./memoryGraphTypes";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { shortcutKeysSuffix } from "@/data/shortcuts";
import { SearchBarShell, HighlightMatch } from "@/shared/ui";
import { EmptyState } from "@/shared/components/common";

export interface MemorySearchBarProps {
  facts: ObservationRecord[];
  searchQuery?: string;
  onCommitSearch: (query: string) => void;
  onSelectNode: (factId: string | null) => void;
  className?: string;
  dropdownPlacement?: "bottom" | "top";
  isLightMode?: boolean;
  variant?: "inline" | "modal";
  autoFocus?: boolean;
}

export type SearchBarProps = MemorySearchBarProps;

export const MemorySearchBar = memo<MemorySearchBarProps>(({
  facts,
  searchQuery,
  onCommitSearch,
  onSelectNode,
  className,
  dropdownPlacement = "bottom",
  isLightMode = false,
  variant = "inline",
  autoFocus = false,
}) => {
  const [value, setValue] = useState(searchQuery ?? "");
  const [activeIndex, setActiveIndex] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (searchQuery !== undefined && searchQuery !== value) {
      setValue(searchQuery);
    }
  }, [searchQuery, value]);

  const palette = useMemo(() => getActiveDynamicPalette(isLightMode), [isLightMode]);

  // Lowercased haystack, recomputed only when `facts` changes.
  const searchIndex = useMemo(
    () =>
      facts.map((f) => ({
        fact: f,
        haystack: `${f.text} ${f.fact_type}`.toLowerCase(),
      })),
    [facts]
  );

  useEffect(() => {
    return () => {
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
    };
  }, []);

  const results = useMemo(() => {
    const q = value.trim().toLowerCase();
    if (!q) return [];
    const hits: ObservationRecord[] = [];
    const maxHits = variant === "modal" ? 20 : 6;
    for (const { fact, haystack } of searchIndex) {
      if (haystack.includes(q)) {
        hits.push(fact);
        if (hits.length === maxHits) break;
      }
    }
    return hits;
  }, [value, searchIndex, variant]);

  useEffect(() => {
    setActiveIndex(-1);
  }, [results]);

  const handleChange = useCallback(
    (val: string) => {
      setValue(val);
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
      debounceTimerRef.current = setTimeout(() => {
        onCommitSearch(val);
      }, 150);
    },
    [onCommitSearch]
  );

  const handleClear = useCallback(() => {
    setValue("");
    onCommitSearch("");
    if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
  }, [onCommitSearch]);

  const handleSelectFact = useCallback(
    (factId: string) => {
      onSelectNode(factId);
      onCommitSearch(value);
      setActiveIndex(-1);
    },
    [onSelectNode, onCommitSearch, value]
  );

  return (
    <SearchBarShell
      id="memory-search-input"
      inputRef={inputRef}
      value={value}
      onChange={handleChange}
      onClear={handleClear}
      placeholder={MEMORY_COPY.searchPlaceholder + shortcutKeysSuffix("memory.search")}
      ariaLabel={MEMORY_COPY.searchInputLabel}
      clearAriaLabel={MEMORY_COPY.clearSearch}
      className={className}
      dropdownPlacement={dropdownPlacement}
      variant={variant}
      autoFocus={autoFocus}
      itemCount={results.length}
      activeIndex={activeIndex}
      onActiveIndexChange={setActiveIndex}
      onSelectActive={(idx) => {
        if (results[idx]) {
          handleSelectFact(results[idx].id);
        } else {
          onCommitSearch(value);
        }
      }}
      onEscape={() => {
        if (value) {
          setValue("");
        }
        onCommitSearch("");
      }}
      dropdownHeader={
        <div className="px-2 py-1 text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))] border-b border-[rgba(var(--border),0.10)] flex items-center justify-between">
          <span>{MEMORY_COPY.matchingFacts}</span>
          <span className="opacity-70 font-bold">{results.length}</span>
        </div>
      }
      dropdownContent={
        <div className="flex flex-col gap-1 max-h-[320px] overflow-y-auto custom-scrollbar">
          {results.map((fact, index) => {
            const kind = toMemoryCategory(fact.fact_type);
            const col = (kind !== undefined ? palette[kind] : undefined) ?? palette.objective;
            return (
              <button
                key={fact.id}
                type="button"
                role="option"
                id={`search-option-${fact.id}`}
                aria-selected={index === activeIndex}
                onMouseDown={(e) => {
                  e.preventDefault();
                  handleSelectFact(fact.id);
                  inputRef.current?.blur();
                }}
                className="flex flex-col text-left p-2 rounded-xl hover:bg-[rgba(var(--foreground),0.05)] transition-colors cursor-pointer"
                style={index === activeIndex ? { background: `rgba(var(--accent),0.08)` } : undefined}
              >
                <div className="flex items-center gap-1.5 mb-1">
                  <span className="w-2 h-2 rounded-full shrink-0" style={{ background: col.main }} />
                  <span className="text-[10px] font-mono uppercase text-[rgb(var(--foreground-muted))] font-semibold">
                    {fact.fact_type}
                  </span>
                  {fact.session_id !== null && (
                    <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/60 ml-auto">
                      #{fact.session_id}
                    </span>
                  )}
                </div>
                <p className="text-[12px] text-[rgb(var(--foreground))] line-clamp-2 leading-tight m-0 font-sans">
                  <HighlightMatch text={fact.text} query={value} />
                </p>
              </button>
            );
          })}
        </div>
      }
    >
      {/* ── Modal Variant Content (Empty states & inline scrollable list) ── */}
      {variant === "modal" && (
        <div className="flex-1 flex flex-col min-h-0 overflow-hidden pt-2">
          {!value.trim() ? (
            <div className="flex-1 flex flex-col items-center justify-center p-6 text-center my-auto min-h-0">
              <EmptyState
                icon={Search}
                title={MEMORY_COPY.emptySearchTitle}
                description={MEMORY_COPY.emptySearchDesc}
                className="max-w-sm border-0 bg-transparent p-0 min-h-0"
              />
              {facts.length > 0 && (
                <div className="flex flex-wrap justify-center gap-1.5 max-w-xs mt-3">
                  {["Identity", "Objective", "Blocker", "Next Step", "Pitfall"].map((cat) => (
                    <button
                      key={cat}
                      type="button"
                      onClick={() => {
                        setValue(cat);
                        onCommitSearch(cat);
                        inputRef.current?.focus();
                      }}
                      className="text-[11px] font-mono px-2.5 py-0.5 rounded-full bg-[rgba(var(--accent),0.06)] hover:bg-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer border border-[rgba(var(--accent),0.12)]"
                    >
                      {cat}
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : results.length === 0 ? (
            <div className="flex-1 flex flex-col items-center justify-center p-6 text-center my-auto min-h-0">
              <EmptyState
                icon={Ghost}
                title={MEMORY_COPY.noMemoriesFoundTitle}
                description={MEMORY_COPY.noMemoriesFoundDesc(value.trim())}
                className="max-w-sm border-0 bg-transparent p-0 min-h-0"
              />
            </div>
          ) : (
            <div
              id="search-listbox"
              role="listbox"
              className="flex-1 overflow-y-auto min-h-0 custom-scrollbar flex flex-col gap-1 pr-1 mt-1"
            >
              <div className="px-2 py-1 text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))] border-b border-[rgba(var(--border),0.10)] flex items-center justify-between shrink-0">
                <span>{MEMORY_COPY.matchingFacts}</span>
                <span className="opacity-70 font-bold">{results.length}</span>
              </div>
              {results.map((fact, index) => {
                const kind = toMemoryCategory(fact.fact_type);
                const col = (kind !== undefined ? palette[kind] : undefined) ?? palette.objective;
                return (
                  <button
                    key={fact.id}
                    type="button"
                    role="option"
                    id={`search-option-${fact.id}`}
                    aria-selected={index === activeIndex}
                    onClick={() => {
                      handleSelectFact(fact.id);
                    }}
                    className="flex flex-col text-left p-2.5 rounded-xl hover:bg-[rgba(var(--foreground),0.05)] transition-colors cursor-pointer border border-transparent hover:border-[rgba(var(--accent),0.15)] shrink-0"
                    style={index === activeIndex ? { background: `rgba(var(--accent),0.08)`, borderColor: `rgba(var(--accent),0.3)` } : undefined}
                  >
                    <div className="flex items-center gap-1.5 mb-1">
                      <span className="w-2 h-2 rounded-full shrink-0" style={{ background: col.main }} />
                      <span className="text-[10px] font-mono uppercase text-[rgb(var(--foreground-muted))] font-semibold">
                        {fact.fact_type}
                      </span>
                      {fact.session_id !== null && (
                        <span className="text-[9px] font-mono text-[rgb(var(--foreground-muted))]/60 ml-auto">
                          #{fact.session_id}
                        </span>
                      )}
                    </div>
                    <p className="text-[12px] text-[rgb(var(--foreground))] line-clamp-2 leading-tight m-0 font-sans">
                      <HighlightMatch text={fact.text} query={value} />
                    </p>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      )}
    </SearchBarShell>
  );
});

MemorySearchBar.displayName = "MemorySearchBar";
export const SearchBar = MemorySearchBar;
