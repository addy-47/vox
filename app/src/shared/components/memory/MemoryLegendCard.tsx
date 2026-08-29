import React, { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Layers, ChevronDown, RotateCcw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";

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
  const [open, setOpen] = useState(false);
  const isFiltered = selectedCollection !== "all" || selectedRelation !== "all";

  return (
    <div className="relative flex flex-col items-end pointer-events-auto select-none">
      {/* Floating Dropup Tray (Opens above the button, wider than button width) */}
      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: 8, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.96 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            className="absolute bottom-[calc(100%+10px)] right-0 w-[310px] rounded-2xl glass-card border border-[rgba(var(--accent),0.18)] bg-[rgb(var(--card))]/95 backdrop-blur-2xl shadow-2xl p-3 z-50 overflow-hidden"
          >
            {/* Header with collection count & reset */}
            <div className="flex items-center justify-between pb-2 mb-2 border-b border-[rgba(var(--border),0.08)] px-1">
              <div className="flex items-center gap-1.5">
                <Layers size={13} className="text-[rgb(var(--accent))]" />
                <span className="text-[11px] font-sans font-bold uppercase tracking-[0.14em] text-[rgb(var(--foreground-muted))]">
                  Memory Legend
                </span>
              </div>
              {isFiltered && (
                <Tooltip label="Reset filters">
                  <button
                    type="button"
                    onClick={() => {
                      onSelectCollection("all");
                      onSelectRelation?.("all");
                    }}
                    className="p-1 rounded text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/10 transition-colors cursor-pointer"
                  >
                    <RotateCcw size={12} />
                  </button>
                </Tooltip>
              )}
            </div>

            {/* 2-Column Grid: Collections & Relations */}
            <div className="flex gap-3 max-h-[280px] overflow-y-auto custom-scrollbar">
              {/* Left column: Collections */}
              <div className="flex-1 min-w-0 flex flex-col gap-1">
                <span className="text-[10px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70 px-1">
                  Clusters
                </span>
                <div className="flex flex-col">
                  {COLLECTIONS_LIST.map((col) => {
                    const isSelected = selectedCollection === col.id;
                    return (
                      <button
                        key={col.id}
                        type="button"
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
                <span className="text-[10px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70 px-1">
                  Edges
                </span>
                <div className="flex flex-col">
                  {RELATIONS_LIST.map((rel) => {
                    const isSelected = selectedRelation === rel.id;
                    return (
                      <button
                        key={rel.id}
                        type="button"
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
          </motion.div>
        )}
      </AnimatePresence>

      {/* EdgeNav-Matched Compact Pill Button (h-[56px], rounded-full) */}
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        aria-expanded={open}
        className={cn(
          "h-[56px] px-4 rounded-full glass-card border transition-all duration-300 shadow-2xl flex items-center gap-2 cursor-pointer",
          open
            ? "border-[rgb(var(--accent))]/50 bg-[rgb(var(--card))]/95 shadow-[0_0_20px_rgba(var(--accent),0.25)] text-[rgb(var(--accent))]"
            : "border-[rgba(var(--accent),0.15)] bg-[rgb(var(--card))]/85 hover:bg-[rgb(var(--accent))]/10 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
        )}
      >
        <Layers size={18} className={cn("transition-colors", open ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--accent))]")} />
        <span className="text-[12px] font-sans font-bold uppercase tracking-[0.14em]">
          Legend
        </span>
        {isFiltered && (
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
        )}
        <ChevronDown
          size={15}
          className={cn(
            "transition-transform duration-300",
            open && "rotate-180 text-[rgb(var(--accent))]"
          )}
        />
      </button>
    </div>
  );
};