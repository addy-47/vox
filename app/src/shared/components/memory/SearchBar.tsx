import React, { useState, useMemo, useRef, useEffect, useCallback, memo } from "react";
import { Search, X } from "lucide-react";
import { FactRecord } from "@/services/memoryService";
import { getActiveDynamicPalette, MemoryCategory } from "./memoryGraphTypes";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";

interface SearchBarProps {
  facts: FactRecord[];
  onCommitSearch: (query: string) => void;
  onSelectNode: (factId: string | null) => void;
  className?: string;
  dropdownPlacement?: "bottom" | "top";
  isLightMode?: boolean;
}

export const SearchBar = memo<SearchBarProps>(({
  facts,
  onCommitSearch,
  onSelectNode,
  className,
  dropdownPlacement = "bottom",
  isLightMode = false,
}) => {
  const [value, setValue] = useState("");
  const [focused, setFocused] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const palette = useMemo(() => getActiveDynamicPalette(isLightMode), [isLightMode]);

  useEffect(() => {
    return () => {
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
    };
  }, []);

  const results = useMemo(() => {
    const q = value.trim().toLowerCase();
    if (!q) return [];
    return facts
      .filter((f) => f.text.toLowerCase().includes(q) || f.fact_type.toLowerCase().includes(q))
      .slice(0, 6);
  }, [value, facts]);

  useEffect(() => {
    setActiveIndex(-1);
  }, [results]);

  useEffect(() => {
    if (activeIndex < 0 || !dropdownRef.current) return;
    const activeEl = dropdownRef.current.querySelector<HTMLElement>(`#search-option-${results[activeIndex]?.id}`);
    if (activeEl) {
      activeEl.scrollIntoView({ block: "nearest" });
    }
  }, [activeIndex, results]);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const val = e.target.value;
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

  const isTopDropdown = dropdownPlacement === "top";

  return (
    <div className={cn("relative pointer-events-auto flex flex-col items-center w-full", className)} data-arrow-nav role="combobox">
      {/* ── Underline Search Input: Clean, responsive, zero pill or bulky borders ── */}
      <div className="flex items-center gap-2 py-1 border-b border-[rgba(var(--foreground),0.18)] focus-within:border-[rgba(var(--accent),0.8)] transition-colors duration-200 w-full">
        <Search size={13} className="text-[rgb(var(--accent))] shrink-0 opacity-70" />
        <input
          id="memory-search-input"
          ref={inputRef}
          type="text"
          value={value}
          onChange={handleChange}
          onFocus={() => setFocused(true)}
          onBlur={() => setTimeout(() => setFocused(false), 200)}
          onKeyDown={(e) => {
            if (results.length === 0) return;
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setActiveIndex((prev) => (prev + 1) % results.length);
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setActiveIndex((prev) => (prev <= 0 ? results.length - 1 : prev - 1));
            } else if (e.key === "Enter") {
              e.preventDefault();
              if (activeIndex >= 0 && activeIndex < results.length) {
                const fact = results[activeIndex];
                onSelectNode(fact.id);
                onCommitSearch(value);
              } else {
                onCommitSearch(value);
              }
            } else if (e.key === "Escape") {
              e.preventDefault();
              setFocused(false);
              setActiveIndex(-1);
              if (value) {
                setValue("");
              }
              onCommitSearch("");
            }
          }}
          aria-expanded={focused && results.length > 0}
          aria-controls="search-listbox"
          aria-activedescendant={activeIndex >= 0 ? `search-option-${results[activeIndex]?.id}` : undefined}
          aria-haspopup="listbox"
          placeholder={MEMORY_COPY.searchPlaceholder}
          className="flex-1 min-w-0 bg-transparent text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 focus:outline-none"
        />
        {value && (
          <button
            type="button"
            onClick={handleClear}
            className="p-0.5 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] cursor-pointer shrink-0 transition-colors"
            aria-label="Clear search"
          >
            <X size={12} />
          </button>
        )}
      </div>

      {/* Quick Search Dropdown */}
      {focused && results.length > 0 && (
        <div
          ref={dropdownRef}
          id="search-listbox"
          role="listbox"
          className={cn(
            "absolute left-1/2 -translate-x-1/2 rounded-2xl glass-card border border-[rgba(var(--accent),0.3)] bg-[rgba(var(--card),0.96)] backdrop-blur-2xl shadow-2xl p-2 z-50 flex flex-col gap-1 overflow-hidden",
            isTopDropdown ? "bottom-[calc(100%+8px)]" : "top-[calc(100%+8px)]"
          )}
          style={{ width: "min(360px, calc(100vw - 32px))" }}
        >
          <div className="px-2 py-1 text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))] border-b border-[rgba(var(--border),0.10)] flex items-center justify-between">
            <span>{MEMORY_COPY.matchingFacts}</span>
            <span className="opacity-70 font-bold">{results.length}</span>
          </div>
          {results.map((fact, index) => {
            const col = palette[fact.fact_type as MemoryCategory] ?? palette.objective;
            return (
              <button
                key={fact.id}
                type="button"
                role="option"
                id={`search-option-${fact.id}`}
                aria-selected={index === activeIndex}
                onMouseDown={(e) => {
                  e.preventDefault();
                  setActiveIndex(index);
                  onSelectNode(fact.id);
                  onCommitSearch(value);
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
                  {fact.text}
                </p>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
});

SearchBar.displayName = "SearchBar";
