import React, { useState, useMemo } from "react";
import { Search, X } from "lucide-react";
import { MemoryNodeTopology } from "@/services/memoryService";
import { getCollectionIcon, getCollectionColor } from "@/shared/components/memory/MemoryGraph";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";

interface SearchBarProps {
  nodes: MemoryNodeTopology[];
  onCommitSearch: (query: string) => void;
  onSelectNode: (nodeId: string | null) => void;
}

export const SearchBar: React.FC<SearchBarProps> = ({
  nodes,
  onCommitSearch,
  onSelectNode,
}) => {
  const [value, setValue] = useState("");
  const [focused, setFocused] = useState(false);

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

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setValue(val);
    onCommitSearch(val);
  };

  const handleClear = () => {
    setValue("");
    onCommitSearch("");
  };

  return (
    <div className="absolute top-4 left-1/2 -translate-x-1/2 z-30 pointer-events-auto w-[420px] max-w-[calc(100vw-32px)] text-[rgb(var(--foreground))]">
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
          type="text"
          value={value}
          onChange={handleChange}
          onFocus={() => setFocused(true)}
          onBlur={() => setTimeout(() => setFocused(false), 200)}
          placeholder="Search your memories..."
          className="w-full bg-transparent border-0 outline-none text-[12px] font-sans text-[rgb(var(--foreground))] placeholder-[rgb(var(--foreground-muted))] font-medium"
        />

        {value && (
          <Tooltip label="Clear search">
            <button
              onClick={handleClear}
              className="p-1 rounded-full text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer shrink-0"
            >
              <X size={14} />
            </button>
          </Tooltip>
        )}
      </div>

      {/* Popover Suggestions with Collection Icons and Fact Content */}
      {focused && results.length > 0 && (
        <div className="absolute top-full mt-2 left-0 right-0 p-2 rounded-2xl border border-[rgba(var(--accent),0.18)] bg-[rgba(var(--card),0.96)] shadow-2xl flex flex-col gap-1 z-40 max-h-[260px] overflow-y-auto">
          <span className="text-[11px] font-sans font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] px-2 py-1 block">
            Matching Memory Facts ({results.length})
          </span>
          {results.map((node) => {
            const IconComp = getCollectionIcon(node.collection);
            const palette = getCollectionColor(node.collection, node.is_superseded);
            const displayText = node.fact || node.id;

            return (
              <button
                key={node.id}
                onMouseDown={(e) => e.preventDefault()} // Prevent blur before click handler fires
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
};
