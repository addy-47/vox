import React, { useState } from "react";
import { Layers, ChevronDown, ChevronUp, RotateCcw, GitFork } from "lucide-react";
import { cn } from "@/shared/lib/utils";

interface MemoryLegendCardProps {
  selectedCollection: string;
  onSelectCollection: (col: string) => void;
  selectedRelation?: string;
  onSelectRelation?: (rel: string) => void;
}

const COLLECTIONS_LIST = [
  { id: "Identity", label: "Identity", color: "#00f2fe" },
  { id: "Profile", label: "Profile", color: "#10b981" },
  { id: "Directives", label: "Directives", color: "#c084fc" },
  { id: "Narrative", label: "Narrative", color: "#f43f5e" },
  { id: "Entities", label: "Entities", color: "#3b82f6" },
  { id: "Constraints", label: "Constraints", color: "#ef4444" },
  { id: "Inactive", label: "Inactive", color: "#64748b" },
];

const RELATIONS_LIST = [
  { id: "SUPPORTS", label: "SUPPORTS", color: "#10b981", isDashed: false },
  { id: "SUPERSEDES", label: "SUPERSEDES", color: "#00f2fe", isDashed: false },
  { id: "SHAPES", label: "SHAPES", color: "#c084fc", isDashed: false },
  { id: "DEPENDS_ON", label: "DEPENDS_ON", color: "#fbbf24", isDashed: false },
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
    <div className="glass-card rounded-2xl border border-[rgba(var(--accent),0.2)] bg-[rgb(var(--card))]/90 backdrop-blur-2xl shadow-2xl pointer-events-auto transition-all duration-200 text-[rgb(var(--foreground))]">
      {/* Minimized Pill Mode */}
      {minimized ? (
        <button
          onClick={() => setMinimized(false)}
          className="flex items-center gap-2 px-3.5 py-2 text-[11px] font-mono text-[rgb(var(--foreground))] hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
        >
          <Layers size={13} className="text-[rgb(var(--accent))]" />
          <span className="font-bold uppercase tracking-wider text-[10px]">Graph Legend</span>
          <ChevronDown size={14} className="text-[rgb(var(--accent))]" />
        </button>
      ) : (
        /* Expanded Card Mode */
        <div className="p-4 w-[280px] flex flex-col gap-3.5">
          {/* Header */}
          <div className="flex items-center justify-between border-b border-[rgba(var(--border),0.15)] pb-2.5">
            <div className="flex items-center gap-2">
              <Layers size={14} className="text-[rgb(var(--accent))]" />
              <span className="text-[11px] font-mono font-bold tracking-widest uppercase text-[rgb(var(--foreground))]">
                Graph Legend
              </span>
            </div>
            <div className="flex items-center gap-1.5">
              {(selectedCollection !== "all" || selectedRelation !== "all") && (
                <button
                  onClick={() => {
                    onSelectCollection("all");
                    if (onSelectRelation) onSelectRelation("all");
                  }}
                  className="p-1 rounded-lg text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/15 transition-colors cursor-pointer"
                  title="Reset Filters"
                >
                  <RotateCcw size={12} />
                </button>
              )}
              <button
                onClick={() => setMinimized(true)}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/10 transition-colors cursor-pointer"
                title="Collapse Legend"
              >
                <ChevronUp size={14} />
              </button>
            </div>
          </div>

          {/* Section 1: Collections */}
          <div className="flex flex-col gap-1.5">
            <div className="flex items-center justify-between px-1">
              <span className="text-[9px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                Collections
              </span>
            </div>
            <div className="grid grid-cols-2 gap-1.5">
              {COLLECTIONS_LIST.map((col) => {
                const isSelected = selectedCollection === col.id;
                return (
                  <button
                    key={col.id}
                    onClick={() => onSelectCollection(selectedCollection === col.id ? "all" : col.id)}
                    className={cn(
                      "flex items-center gap-2 px-2.5 py-1.5 rounded-xl text-[10px] font-mono transition-all duration-150 cursor-pointer border select-none text-left",
                      isSelected
                        ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] font-bold border-[rgb(var(--accent))]/40 shadow-sm"
                        : "bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))] border-transparent hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/10 hover:border-[rgba(var(--accent),0.2)]"
                    )}
                  >
                    <span
                      className="w-2.5 h-2.5 rounded-full shrink-0 shadow-sm"
                      style={{ backgroundColor: col.color }}
                    />
                    <span className="truncate">{col.label}</span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Section 2: Relations */}
          <div className="flex flex-col gap-1.5 border-t border-[rgba(var(--border),0.15)] pt-3">
            <div className="flex items-center gap-1.5 px-1">
              <GitFork size={12} className="text-[rgb(var(--accent))]" />
              <span className="text-[9px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                Relations
              </span>
            </div>
            <div className="flex flex-col gap-1">
              {RELATIONS_LIST.map((rel) => {
                const isSelected = selectedRelation === rel.id;
                return (
                  <button
                    key={rel.id}
                    onClick={() => {
                      if (onSelectRelation) {
                        onSelectRelation(selectedRelation === rel.id ? "all" : rel.id);
                      }
                    }}
                    className={cn(
                      "flex items-center justify-between px-2.5 py-1 rounded-xl text-[10px] font-mono transition-all duration-150 cursor-pointer border select-none",
                      isSelected
                        ? "bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] font-bold border-[rgb(var(--accent))]/40"
                        : "bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))] border-transparent hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/10 hover:border-[rgba(var(--accent),0.2)]"
                    )}
                  >
                    <span className="truncate">{rel.label}</span>
                    <div className="flex items-center gap-2">
                      <span
                        className="w-6 h-0.5 rounded-full shrink-0"
                        style={{
                          backgroundColor: rel.color,
                          borderStyle: rel.isDashed ? "dashed" : "solid",
                        }}
                      />
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
