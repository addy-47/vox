import { memo, useMemo } from "react";
import { RotateCcw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { MEMORY_COPY } from "@/data/memoryCopy";
import {
  MemoryCategory,
  getActiveDynamicPalette,
} from "./memoryGraphTypes";

interface MemoryLegendOverlayProps {
  selectedCollection: string;
  onSelectCollection: (col: string) => void;
  counts?: Partial<Record<MemoryCategory, number>>;
  isLightMode?: boolean;
}

/**
 * 3x2 Grid: 3 columns x 2 rows
 * Row 1: Identity (personal)   | Objective (objective) | Work Done (workdone)
 * Row 2: Blocker (blocker)     | Next Step (next_step) | Pitfall (pitfall)
 */
const CATEGORIES: MemoryCategory[] = [
  "personal",
  "objective",
  "workdone",
  "blocker",
  "next_step",
  "pitfall",
];

export const MemoryLegendOverlay = memo<MemoryLegendOverlayProps>(({
  selectedCollection,
  onSelectCollection,
  counts = {},
  isLightMode = false,
}) => {
  const isFiltered = selectedCollection !== "all";
  const palette = useMemo(() => getActiveDynamicPalette(isLightMode), [isLightMode]);

  return (
    <div
      aria-label={MEMORY_COPY.topologyLegend}
      className="flex flex-col gap-1.5 select-none pointer-events-auto bg-transparent border-none shadow-none"
    >
      {/* 3x2 Ambient Text Grid — No box, no border, no pill */}
      <div className="grid grid-cols-3 gap-x-5 gap-y-2 items-center">
        {CATEGORIES.map((catKey) => {
          const isSelected = selectedCollection === catKey;
          const count = counts[catKey] ?? 0;
          const catColor = palette[catKey]?.main || "#00dbe9";
          const label = MEMORY_COPY.categories[catKey];
          const desc = palette[catKey]?.desc || "";

          return (
            <Tooltip
              key={catKey}
              label={`${label} (${count}) — ${desc}`}
              side="top"
            >
              <button
                type="button"
                onClick={() => onSelectCollection(isSelected ? "all" : catKey)}
                className={cn(
                  "flex items-center gap-1.5 text-[11px] font-mono leading-none transition-all cursor-pointer group text-left min-w-0 bg-transparent border-none p-0",
                  isSelected
                    ? "text-[rgb(var(--foreground))] font-semibold"
                    : isFiltered
                    ? "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--foreground))]"
                    : "text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))]"
                )}
                aria-pressed={isSelected}
              >
                {/* Category Chromatic Jewel Dot */}
                <span
                  className={cn(
                    "w-1.5 h-1.5 rounded-full shrink-0 transition-transform duration-200",
                    isSelected
                      ? "scale-125 ring-2 ring-[rgb(var(--foreground))]/30"
                      : "group-hover:scale-110"
                  )}
                  style={{
                    backgroundColor: catColor,
                    boxShadow: isSelected
                      ? `0 0 6px ${catColor}`
                      : `0 0 3px ${catColor}80`,
                  }}
                />

                {/* Category Label */}
                <span className="truncate">{label}</span>
              </button>
            </Tooltip>
          );
        })}
      </div>

      {/* Active Filter Clear Prompt (Subtle inline text when filtered) */}
      {isFiltered && (
        <div className="flex items-center gap-2 pt-0.5">
          <span className="text-[9px] font-mono text-[rgb(var(--accent))] font-medium">
            Filtered: {MEMORY_COPY.categories[selectedCollection as MemoryCategory] || selectedCollection}
          </span>
          <button
            type="button"
            onClick={() => onSelectCollection("all")}
            className="flex items-center gap-1 text-[9px] font-mono text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer bg-transparent border-none p-0"
          >
            <RotateCcw size={9} />
            <span>{MEMORY_COPY.clearSearch}</span>
          </button>
        </div>
      )}
    </div>
  );
});

MemoryLegendOverlay.displayName = "MemoryLegendOverlay";
