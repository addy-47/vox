import React, { useState, useMemo, useRef, useEffect, useCallback, memo } from "react";
import { Search, X } from "lucide-react";
import { MemoryNodeTopology } from "@/services/memoryService";
import { getCollectionIcon, getCollectionColor } from "@/shared/components/memory/MemoryGraph";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { MEMORY_COPY } from "@/data/memoryCopy";

interface SearchBarProps {
  nodes: MemoryNodeTopology[];
  onCommitSearch: (query: string) => void;
  onSelectNode: (nodeId: string | null) => void;
  variant?: "full" | "expandable";
  onClose?: () => void;
  autoFocus?: boolean;
  className?: string;
}

export const SearchBar = memo<SearchBarProps>(({
  nodes,
  onCommitSearch,
  onSelectNode,
  variant = "full",
  onClose,
  autoFocus,
  className,
}) => {
  const [expanded, setExpanded] = useState(false);
  const [value, setValue] = useState("");
  const [focused, setFocused] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const blurTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
      if (blurTimerRef.current) clearTimeout(blurTimerRef.current);
    };
  }, []);

  useEffect(() => {
    if ((variant === "expandable" && expanded && inputRef.current) || (autoFocus && inputRef.current)) {
      inputRef.current.focus();
    }
  }, [variant, expanded, autoFocus]);

  const results = useMemo(() => {
    const q = value.trim().toLowerCase();
    if (!q) return [];
    return nodes
      .filter(
        (n) =>
          (n.fact && n.fact.toLowerCase().includes(q)) ||
          n.id.toLowerCase().includes(q) ||
          n.collection.toLowerCase().includes(q)
      )
      .slice(0, 8);
  }, [value, nodes]);

  const handleChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setValue(val);
    if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
    debounceTimerRef.current = setTimeout(() => {
      onCommitSearch(val);
    }, 150);
  }, [onCommitSearch]);

  const handleClose = useCallback(() => {
    setValue("");
    setExpanded(false);
    setFocused(false);
    if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
    onCommitSearch("");
  }, [onCommitSearch]);

  const handleClear = useCallback(() => {
    setValue("");
    if (debounceTimerRef.current) clearTimeout(debounceTimerRef.current);
    onCommitSearch("");
    if (inputRef.current) inputRef.current.focus();
  }, [onCommitSearch]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      if (variant === "expandable") {
        handleClose();
      } else {
        handleClear();
      }
    }
  }, [variant, handleClose, handleClear]);

  useEffect(() => {
    if (variant !== "expandable" || !expanded) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        if (!value.trim()) {
          setExpanded(false);
        }
        setFocused(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [variant, expanded, value]);

  // ── Full Desktop / Mobile Overlay Search Bar ──────────────────────────────
  if (variant === "full") {
    return (
      <div
        ref={containerRef}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            if (onClose) onClose();
            else handleClear();
          }
        }}
        className={cn(
          "z-30 pointer-events-auto text-[rgb(var(--foreground))]",
          className || "absolute top-4 left-1/2 -translate-x-1/2 w-[420px] max-w-[calc(100vw-32px)]"
        )}
      >
        <div
          className={cn(
            "relative flex items-center px-3.5 py-2 rounded-full glass-card border transition-all duration-200 shadow-2xl",
            focused
              ? "border-[rgb(var(--accent))]/50 bg-[rgb(var(--card))]/95 shadow-[0_0_25px_rgba(var(--accent),0.2)]"
              : "border-[rgba(var(--accent),0.12)] bg-[rgb(var(--card))]/85"
          )}
        >
          <Search size={16} className="text-[rgb(var(--accent))] shrink-0 mr-2.5" />
          <input
            ref={inputRef}
            type="text"
            value={value}
            onChange={handleChange}
            onFocus={() => {
              if (blurTimerRef.current) clearTimeout(blurTimerRef.current);
              setFocused(true);
            }}
            onBlur={() => {
              if (blurTimerRef.current) clearTimeout(blurTimerRef.current);
              blurTimerRef.current = setTimeout(() => setFocused(false), 200);
            }}
            placeholder={MEMORY_COPY.searchPlaceholder}
            className="w-full bg-transparent border-0 outline-none text-[12px] font-sans text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 font-medium"
          />

          {value ? (
            <Tooltip label={MEMORY_COPY.clearSearch}>
              <button
                onClick={handleClear}
                className="p-1 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
                aria-label={MEMORY_COPY.clearSearch}
              >
                <X size={14} />
              </button>
            </Tooltip>
          ) : onClose ? (
            <button
              onClick={onClose}
              className="p-1 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
              aria-label="Close search"
            >
              <X size={14} />
            </button>
          ) : null}
        </div>

        {/* Popover Suggestions with Collection Icons and Fact Content */}
        {focused && results.length > 0 && (
          <div className="absolute top-full mt-2 left-0 right-0 p-2 rounded-2xl border border-[rgba(var(--accent),0.18)] bg-[rgb(var(--card))]/95 backdrop-blur-2xl shadow-2xl flex flex-col gap-1 z-40 max-h-[260px] overflow-y-auto custom-scrollbar">
            <span className="text-[11px] font-sans font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/75 px-2 py-1 block">
              Matching Memory Facts ({results.length})
            </span>
            {results.map((node) => {
              const IconComp = getCollectionIcon(node.collection);
              const palette = getCollectionColor(node.collection, node.is_superseded);
              const displayText = node.fact || node.id;

              return (
                <button
                  key={node.id}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => {
                    onSelectNode(node.id);
                    setFocused(false);
                  }}
                  className="flex items-center gap-2.5 px-3 py-2 rounded-xl text-left hover:bg-[rgb(var(--accent))]/10 transition-colors cursor-pointer group overflow-hidden"
                >
                  <div
                    className="p-1.5 rounded-lg flex items-center justify-center shrink-0"
                    style={{ backgroundColor: `${palette.main}20`, color: palette.main }}
                  >
                    <IconComp size={14} />
                  </div>
                  <span className="text-[12px] font-sans font-medium text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--accent))] truncate flex-1">
                    {displayText}
                  </span>
                  <span
                    className="text-[11px] font-sans font-semibold px-2 py-0.5 rounded-full shrink-0 shadow-xs"
                    style={{ backgroundColor: `${palette.main}20`, color: palette.main }}
                  >
                    {node.collection}
                  </span>
                </button>
              );
            })}
          </div>
        )}
      </div>
    );
  }

  // ── Small / Mobile Expandable Search Button ────────────────────────────────
  if (!expanded) {
    return (
      <Tooltip label={MEMORY_COPY.searchMemories}>
        <button
          onClick={() => setExpanded(true)}
          className="w-9 h-9 flex items-center justify-center rounded-2xl glass-card border border-[rgba(var(--accent),0.12)] bg-[rgb(var(--card))]/85 backdrop-blur-2xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-all cursor-pointer shadow-2xl shrink-0 pointer-events-auto"
          aria-label={MEMORY_COPY.searchMemories}
        >
          <Search size={16} />
        </button>
      </Tooltip>
    );
  }

  return (
    <div
      ref={containerRef}
      onKeyDown={handleKeyDown}
      className="relative z-30 pointer-events-auto w-[240px] sm:w-[320px] text-[rgb(var(--foreground))] transition-all duration-300 animate-fade-in"
    >
      <div
        className={cn(
          "relative flex items-center h-9 px-3 rounded-2xl glass-card border transition-all duration-200 shadow-2xl",
          focused
            ? "border-[rgb(var(--accent))]/50 bg-[rgb(var(--card))]/95 shadow-[0_0_25px_rgba(var(--accent),0.2)]"
            : "border-[rgba(var(--accent),0.18)] bg-[rgb(var(--card))]/90"
        )}
      >
        <Search size={15} className="text-[rgb(var(--accent))] shrink-0 mr-2" />
        <input
          ref={inputRef}
          type="text"
          value={value}
          onChange={handleChange}
          onFocus={() => {
            if (blurTimerRef.current) clearTimeout(blurTimerRef.current);
            setFocused(true);
          }}
          onBlur={() => {
            if (blurTimerRef.current) clearTimeout(blurTimerRef.current);
            blurTimerRef.current = setTimeout(() => setFocused(false), 200);
          }}
          placeholder={MEMORY_COPY.searchPlaceholder}
          className="w-full bg-transparent border-0 outline-none text-[12px] font-sans text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/50 font-medium"
        />

        {value ? (
          <Tooltip label={MEMORY_COPY.clearSearch}>
            <button
              onClick={handleClear}
              className="p-1 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
              aria-label={MEMORY_COPY.clearSearch}
            >
              <X size={14} />
            </button>
          </Tooltip>
        ) : (
          <button
            onClick={handleClose}
            className="p-1 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
            aria-label={MEMORY_COPY.clearSearch}
          >
            <X size={14} />
          </button>
        )}
      </div>

      {/* Popover Suggestions with Collection Icons and Fact Content */}
      {focused && results.length > 0 && (
        <div className="absolute top-full mt-2 right-0 w-[280px] sm:w-[340px] p-2 rounded-2xl border border-[rgba(var(--accent),0.18)] bg-[rgb(var(--card))]/95 backdrop-blur-2xl shadow-2xl flex flex-col gap-1 z-40 max-h-[260px] overflow-y-auto custom-scrollbar">
          <span className="text-[11px] font-sans font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/75 px-2 py-1 block">
            Matching Memory Facts ({results.length})
          </span>
          {results.map((node) => {
            const IconComp = getCollectionIcon(node.collection);
            const palette = getCollectionColor(node.collection, node.is_superseded);
            const displayText = node.fact || node.id;

            return (
              <button
                key={node.id}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => {
                  onSelectNode(node.id);
                  setFocused(false);
                }}
                className="flex items-center gap-2.5 px-3 py-2 rounded-xl text-left hover:bg-[rgb(var(--accent))]/10 transition-colors cursor-pointer group overflow-hidden"
              >
                <div
                  className="p-1.5 rounded-lg flex items-center justify-center shrink-0"
                  style={{ backgroundColor: `${palette.main}20`, color: palette.main }}
                >
                  <IconComp size={14} />
                </div>
                <span className="text-[12px] font-sans font-medium text-[rgb(var(--foreground))] group-hover:text-[rgb(var(--accent))] truncate flex-1">
                  {displayText}
                </span>
                <span
                  className="text-[11px] font-sans font-semibold px-2 py-0.5 rounded-full shrink-0 shadow-xs"
                  style={{ backgroundColor: `${palette.main}20`, color: palette.main }}
                >
                  {node.collection}
                </span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
});

SearchBar.displayName = "SearchBar";
