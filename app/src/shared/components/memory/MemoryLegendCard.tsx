import React, { useState } from "react";
import { Layers, ChevronDown, ChevronUp, RotateCcw } from "lucide-react";
import { cn } from "@/shared/lib/utils";

interface MemoryLegendCardProps {
  selectedCollection: string;
  onSelectCollection: (col: string) => void;
  selectedRelation?: string;
  onSelectRelation?: (rel: string) => void;
}

const COLLECTIONS_LIST = [
  { id: "Identity", label: "Identity", darkColor: "#38bdf8", lightColor: "#0284c7" },
  { id: "Profile", label: "Profile", darkColor: "#34d399", lightColor: "#059669" },
  { id: "Directives", label: "Directives", darkColor: "#a78bfa", lightColor: "#7c3aed" },
  { id: "Narrative", label: "Narrative", darkColor: "#f472b6", lightColor: "#db2777" },
  { id: "Entities", label: "Entities", darkColor: "#facc15", lightColor: "#d97706" },
  { id: "Constraints", label: "Constraints", darkColor: "#f43f5e", lightColor: "#e11d48" },
  { id: "Inactive", label: "Inactive", darkColor: "#64748b", lightColor: "#475569" },
];

const RELATIONS_LIST = [
  { id: "SUPPORTS", label: "SUPPORTS", color: "#34d399", isDashed: false },
  { id: "SUPERSEDES", label: "SUPERSEDES", color: "#38bdf8", isDashed: false },
  { id: "SHAPES", label: "SHAPES", color: "#a78bfa", isDashed: false },
  { id: "DEPENDS_ON", label: "DEPENDS_ON", color: "#d97706", isDashed: false },
  { id: "CONFLICTS_WITH", label: "CONFLICTS_WITH", color: "#ef4444", isDashed: true },
];

export const MemoryLegendCard: React.FC<MemoryLegendCardProps> = ({
  selectedCollection,
  onSelectCollection,
  selectedRelation = "all",
  onSelectRelation,
}) => {
  const [minimized, setMinimized] = useState(false);

  return (
    <div className="glass-whisper rounded-xl border border-[rgba(var(--accent),0.08)] bg-[rgb(var(--card))]/40 backdrop-blur-[12px] pointer-events-auto transition-all duration-200 text-[rgb(var(--foreground))] w-fit max-w-[320px]">
      {minimized ? (
        <button
          onClick={() => setMinimized(false)}
          className="flex items-center gap-2 px-3 py-1.5 text-[10px] font-sans text-[rgb(var(--foreground))] hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
        >
          <Layers size={12} className="text-[rgb(var(--accent))]" />
          <span className="font-bold uppercase tracking-wider text-[10px]">Graph Legend</span>
          <ChevronDown size={13} className="text-[rgb(var(--accent))]" />
        </button>
      ) : (
        <div className="p-2.5 flex flex-col gap-2">
          {/* Header */}
          <div className="flex items-center justify-between px-1 pb-1.5 border-b border-[rgba(var(--border),0.08)]">
            <div className="flex items-center gap-1.5">
              <Layers size={12} className="text-[rgb(var(--accent))]" />
              <span className="text-[10px] font-mono font-bold uppercase tracking-[0.14em] text-[rgb(var(--foreground-muted))]">
                Graph Legend
              </span>
            </div>
            <div className="flex items-center gap-1">
              {(selectedCollection !== "all" || selectedRelation !== "all") && (
                <button
                  onClick={() => {
                    onSelectCollection("all");
                    onSelectRelation?.("all");
                  }}
                  className="p-1 rounded text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors cursor-pointer"
                  title="Reset filters"
                >
                  <RotateCcw size={11} />
                </button>
              )}
              <button
                onClick={() => setMinimized(true)}
                className="p-1 rounded text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-black/5 dark:hover:bg-white/10 transition-colors cursor-pointer"
                title="Collapse legend"
              >
                <ChevronUp size={13} />
              </button>
            </div>
          </div>

          {/* Two-column layout: Collections left, Relations right */}
          <div className="flex gap-3">
            {/* Left column: Collections */}
            <div className="flex-1 min-w-0 flex flex-col gap-1">
              <div className="flex flex-col">
                {COLLECTIONS_LIST.map((col) => {
                  const isSelected = selectedCollection === col.id;
                  return (
                    <button
                      key={col.id}
                      onClick={() => onSelectCollection(isSelected ? "all" : col.id)}
                      aria-pressed={isSelected}
                      className={cn(
                        "flex items-center gap-1.5 px-2 py-1 rounded-lg text-[11px] font-sans transition-all cursor-pointer select-none text-left",
                        isSelected
                          ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] font-bold ring-1 ring-[rgb(var(--accent))]/30"
                          : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-black/[0.04] dark:hover:bg-white/[0.05]"
                      )}
                    >
                      <span
                        className="w-2 h-2 rounded-full shrink-0"
                        style={{ backgroundColor: col.darkColor }}
                      />
                      <span className="truncate">{col.label}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Right column: Relations */}
            <div className="flex-1 min-w-0 flex flex-col gap-1">
              <div className="flex flex-col">
                {RELATIONS_LIST.map((rel) => {
                  const isSelected = selectedRelation === rel.id;
                  return (
                    <button
                      key={rel.id}
                      onClick={() => onSelectRelation?.(isSelected ? "all" : rel.id)}
                      aria-pressed={isSelected}
                      className={cn(
                        "flex items-center justify-between gap-1.5 px-2 py-1 rounded-lg text-[11px] font-sans transition-all cursor-pointer select-none",
                        isSelected
                          ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] font-bold ring-1 ring-[rgb(var(--accent))]/30"
                          : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-black/[0.04] dark:hover:bg-white/[0.05]"
                      )}
                    >
                      <span className="truncate font-medium">{rel.label}</span>
                      <span
                        className="w-4 h-[2px] rounded-full shrink-0"
                        style={{ backgroundColor: rel.color }}
                      />
                    </button>
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};