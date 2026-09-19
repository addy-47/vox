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
  const inputRef = useRef<HTMLInputElement>(null);
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
    <div className={cn("relative pointer-events-auto flex flex-col items-center w-full", className)}>
      {/* ── Underline Search Input: Clean, responsive, zero pill or bulky borders ── */}
      <div className="flex items-center gap-2 py-1 border-b border-[rgba(var(--foreground),0.18)] focus-within:border-[rgba(var(--accent),0.8)] transition-colors duration-200 w-full">
        <Search size={13} className="text-[rgb(var(--accent))] shrink-0 opacity-70" />
        <input
          ref={inputRef}
          type="text"
          value={value}
          onChange={handleChange}
          onFocus={() => setFocused(true)}
          onBlur={() => setTimeout(() => setFocused(false), 200)}
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
          {results.map((fact) => {
            const col = palette[fact.fact_type as MemoryCategory] ?? palette.objective;
            return (
              <button
                key={fact.id}
                type="button"
                onMouseDown={() => {
                  onSelectNode(fact.id);
                  onCommitSearch(value);
                }}
                className="flex flex-col text-left p-2 rounded-xl hover:bg-[rgba(var(--foreground),0.05)] transition-colors cursor-pointer"
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
