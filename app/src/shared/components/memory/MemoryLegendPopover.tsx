import { useState, useRef, memo, useCallback } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Layers, RotateCcw, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { MEMORY_COPY } from "@/data/memoryCopy";
import {
  MemoryCategory,
  getActiveDynamicPalette,
} from "./memoryGraphTypes";

interface MemoryLegendPopoverProps {
  selectedCollection: string;
  onSelectCollection: (col: string) => void;
  counts?: Partial<Record<MemoryCategory, number>>;
  isLightMode?: boolean;
}

const CATEGORY_KEYS: MemoryCategory[] = [
  "personal",
  "objective",
  "workdone",
  "blocker",
  "next_step",
  "pitfall",
];

export const MemoryLegendPopover = memo<MemoryLegendPopoverProps>(({
  selectedCollection,
  onSelectCollection,
  counts = {},
  isLightMode = false,
}) => {
  const [open, setOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);
  const isFiltered = selectedCollection !== "all";

  const handleClose = useCallback(() => {
    setOpen(false);
  }, []);

  useOverlay({
    onClose: handleClose,
    ref: popoverRef,
    dismissOnOutside: true,
    active: open,
  });

  const palette = getActiveDynamicPalette(isLightMode);

  return (
    <div className="relative pointer-events-auto select-none" ref={popoverRef}>
      {/* Popover Trigger Capsule */}
      <Tooltip label={MEMORY_COPY.topologyLegend} side="top">
        <button
          type="button"
          onClick={() => setOpen((prev) => !prev)}
          className={cn(
            "flex items-center gap-2 px-3.5 py-2 rounded-2xl glass-card border backdrop-blur-2xl text-[12px] font-mono transition-all cursor-pointer shadow-xl",
            isFiltered
              ? "border-[rgba(var(--accent),0.5)] bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))]"
              : "border-[rgba(var(--border),0.14)] bg-[rgba(var(--card),0.85)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.25)]"
          )}
          aria-label={MEMORY_COPY.topologyLegend}
        >
          <Layers size={14} className={cn("transition-transform", isFiltered ? "text-[rgb(var(--accent))]" : "opacity-80")} />
          <span className="font-semibold uppercase tracking-wider text-[11px]">
            {isFiltered ? MEMORY_COPY.categories[selectedCollection as MemoryCategory] || selectedCollection : MEMORY_COPY.legendTitle}
          </span>
          {isFiltered && (
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
          )}
        </button>
      </Tooltip>

      {/* Popover Body (anchored bottom-right) */}
      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: 10, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 10, scale: 0.96 }}
            transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
            className="absolute bottom-[calc(100%+10px)] right-0 w-[280px] rounded-2xl glass-card border border-[rgba(var(--border),0.16)] bg-[rgba(var(--card),0.92)] backdrop-blur-2xl shadow-2xl p-3 z-50 overflow-hidden"
          >
            {/* Header */}
            <div className="flex items-center justify-between pb-2.5 mb-2 border-b border-[rgba(var(--border),0.10)] px-1">
              <div className="flex items-center gap-2">
                <span className="text-[10px] font-display font-bold uppercase tracking-[0.16em] text-[rgb(var(--foreground))]">
                  {MEMORY_COPY.legendTitle}
                </span>
                {isFiltered && (
                  <span className="px-1.5 py-0.5 rounded text-[9px] font-mono bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-medium">
                    1 filter
                  </span>
                )}
              </div>

              <div className="flex items-center gap-1">
                {isFiltered && (
                  <Tooltip label={MEMORY_COPY.clearSearch}>
                    <button
                      type="button"
                      onClick={() => onSelectCollection("all")}
                      className="p-1 rounded-lg text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] transition-colors cursor-pointer"
                    >
                      <RotateCcw size={11} />
                    </button>
                  </Tooltip>
                )}
                <button
                  type="button"
                  onClick={handleClose}
                  className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer"
                  aria-label="Close"
                >
                  <X size={12} />
                </button>
              </div>
            </div>

            {/* Category Options */}
            <div className="flex flex-col gap-1">
              {/* Show All */}
              <button
                type="button"
                onClick={() => {
                  onSelectCollection("all");
                  handleClose();
                }}
                className={cn(
                  "flex items-center justify-between px-2.5 py-2 rounded-xl text-[11px] font-mono transition-colors cursor-pointer",
                  selectedCollection === "all"
                    ? "bg-[rgba(var(--accent),0.14)] text-[rgb(var(--accent))] font-bold border border-[rgba(var(--accent),0.3)]"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)] border border-transparent"
                )}
              >
                <div className="flex items-center gap-2.5">
                  <span className="w-2 h-2 rounded-full bg-[rgba(var(--foreground),0.4)]" />
                  <span>{MEMORY_COPY.showAllCategories}</span>
                </div>
              </button>

              {/* 6 Category Rows */}
              {CATEGORY_KEYS.map((catKey) => {
                const isSelected = selectedCollection === catKey;
                const count = counts[catKey] ?? 0;
                const catColor = palette[catKey]?.main || "#00dbe9";
                const label = MEMORY_COPY.categories[catKey];

                return (
                  <button
                    key={catKey}
                    type="button"
                    onClick={() => {
                      onSelectCollection(isSelected ? "all" : catKey);
                    }}
                    className={cn(
                      "flex items-center justify-between px-2.5 py-1.5 rounded-xl text-[11px] font-mono transition-colors cursor-pointer",
                      isSelected
                        ? "bg-[rgba(var(--accent),0.14)] text-[rgb(var(--accent))] font-bold border border-[rgba(var(--accent),0.3)]"
                        : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.05)] border border-transparent"
                    )}
                  >
                    <div className="flex items-center gap-2.5">
                      <span
                        className="w-2.5 h-2.5 rounded-full shrink-0"
                        style={{
                          background: catColor,
                          boxShadow: `0 0 6px ${catColor}80`,
                        }}
                      />
                      <span>{label}</span>
                    </div>
                    <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70">
                      {count}
                    </span>
                  </button>
                );
              })}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
});

MemoryLegendPopover.displayName = "MemoryLegendPopover";
