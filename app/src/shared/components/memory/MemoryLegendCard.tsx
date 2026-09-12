import React, { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Layers, ChevronDown, RotateCcw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { MemoryCategory } from "./memoryGraphTypes";

interface MemoryLegendCardProps {
  selectedCollection: string;
  onSelectCollection: (col: string) => void;
  counts?: Partial<Record<MemoryCategory, number>>;
}

const CATEGORIES_LIST: { id: MemoryCategory; label: string; darkColor: string; lightColor: string }[] = [
  { id: "personal", label: "Identity", darkColor: "#00dbe9", lightColor: "#0891b2" },
  { id: "objective", label: "Objective", darkColor: "#a78bfa", lightColor: "#7c3aed" },
  { id: "workdone", label: "Work Done", darkColor: "#34d399", lightColor: "#059669" },
  { id: "blocker", label: "Blocker", darkColor: "#f43f5e", lightColor: "#e11d48" },
  { id: "next_step", label: "Next Step", darkColor: "#f59e0b", lightColor: "#d97706" },
  { id: "pitfall", label: "Pitfall", darkColor: "#facc15", lightColor: "#ca8a04" },
];

export const MemoryLegendCard: React.FC<MemoryLegendCardProps> = ({
  selectedCollection,
  onSelectCollection,
  counts = {},
}) => {
  const [open, setOpen] = useState(false);
  const isFiltered = selectedCollection !== "all";

  return (
    <div className="relative flex flex-col items-start pointer-events-auto select-none">
      {/* Trigger Button */}
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        className={cn(
          "flex items-center gap-2 px-3 py-2 rounded-2xl glass-card border border-[rgba(255,255,255,0.08)] backdrop-blur-xl text-[12px] font-mono transition-all cursor-pointer shadow-lg",
          isFiltered
            ? "border-[rgba(0,219,233,0.5)] text-[rgb(var(--accent))]"
            : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(255,255,255,0.18)]"
        )}
      >
        <Layers size={14} className={isFiltered ? "text-[rgb(var(--accent))]" : "opacity-70"} />
        <span className="font-semibold uppercase tracking-wider text-[11px]">
          {isFiltered ? selectedCollection : MEMORY_COPY.legendTitle}
        </span>
        <ChevronDown
          size={13}
          className={cn("transition-transform duration-200 opacity-60", open && "rotate-180")}
        />
      </button>

      {/* Floating Dropdown Tray */}
      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: 8, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.96 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            className="absolute top-[calc(100%+8px)] left-0 w-[260px] rounded-2xl glass-card border border-[rgba(0,219,233,0.22)] bg-[rgba(10,14,24,0.95)] backdrop-blur-2xl shadow-2xl p-3 z-50 overflow-hidden"
          >
            {/* Header */}
            <div className="flex items-center justify-between pb-2 mb-2 border-b border-[rgba(255,255,255,0.08)] px-1">
              <span className="text-[10px] font-display font-bold uppercase tracking-[0.16em] text-[rgb(var(--foreground-muted))]">
                {MEMORY_COPY.legendTitle}
              </span>
              {isFiltered && (
                <Tooltip label={MEMORY_COPY.clearSearch}>
                  <button
                    type="button"
                    onClick={() => onSelectCollection("all")}
                    className="p-1 rounded text-[rgb(var(--accent))] hover:bg-[rgba(0,219,233,0.1)] transition-colors cursor-pointer"
                  >
                    <RotateCcw size={11} />
                  </button>
                </Tooltip>
              )}
            </div>

            {/* Category Options */}
            <div className="flex flex-col gap-1">
              <button
                type="button"
                onClick={() => onSelectCollection("all")}
                className={cn(
                  "flex items-center justify-between px-2.5 py-1.5 rounded-xl text-[11px] font-mono transition-colors cursor-pointer",
                  selectedCollection === "all"
                    ? "bg-[rgba(0,219,233,0.15)] text-[rgb(var(--accent))] font-bold"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(255,255,255,0.04)]"
                )}
              >
                <div className="flex items-center gap-2">
                  <span className="w-2 h-2 rounded-full bg-[rgba(255,255,255,0.5)]" />
                  <span>Show All Categories</span>
                </div>
              </button>

              {CATEGORIES_LIST.map((cat) => {
                const isSelected = selectedCollection === cat.id;
                const count = counts[cat.id];
                return (
                  <button
                    key={cat.id}
                    type="button"
                    onClick={() => onSelectCollection(isSelected ? "all" : cat.id)}
                    className={cn(
                      "flex items-center justify-between px-2.5 py-1.5 rounded-xl text-[11px] font-mono transition-colors cursor-pointer",
                      isSelected
                        ? "bg-[rgba(0,219,233,0.15)] text-[rgb(var(--accent))] font-bold"
                        : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(255,255,255,0.04)]"
                    )}
                  >
                    <div className="flex items-center gap-2">
                      <span
                        className="w-2.5 h-2.5 rounded-full shrink-0"
                        style={{
                          background: cat.darkColor,
                          boxShadow: `0 0 6px ${cat.darkColor}80`,
                        }}
                      />
                      <span>{cat.label}</span>
                    </div>
                    {count !== undefined && (
                      <span className="text-[10px] opacity-60 font-mono">
                        {count}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};