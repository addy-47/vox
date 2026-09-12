import { memo } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { X, Target } from "lucide-react";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ClusterBadgeData } from "./memoryGraphTypes";

interface MemoryGraphClusterBadgesProps {
  clusterBadges: ClusterBadgeData[];
  expandedBadge: string | null;
  onToggleBadge: (collection: string | null) => void;
  isLightMode: boolean;
}

export const MemoryGraphClusterBadges = memo(
  ({
    clusterBadges,
    expandedBadge,
    onToggleBadge,
    isLightMode,
  }: MemoryGraphClusterBadgesProps) => {
    return (
      <div className="absolute inset-0 pointer-events-none overflow-hidden z-10">
        {clusterBadges.map((badge) => {
          const isSelected = expandedBadge === badge.collection;

          return (
            <div
              key={badge.collection}
              id={`badge-pill-${badge.collection}`}
              style={{
                left: `${badge.screenX}px`,
                top: `${badge.screenY}px`,
                transform: "translate(-50%, -50%)",
              }}
              className="absolute pointer-events-auto z-20"
            >
              {/* Persistent Badge Button */}
              <motion.button
                type="button"
                initial={{ opacity: 0, scale: 0.92 }}
                animate={{ opacity: 1, scale: isSelected ? 1.06 : 1 }}
                exit={{ opacity: 0, scale: 0.92 }}
                transition={{ duration: 0.12, ease: "easeOut" }}
                onClick={(e) => {
                  e.stopPropagation();
                  onToggleBadge(isSelected ? null : badge.collection);
                }}
                style={{
                  backgroundColor: isLightMode ? "#ffffff" : "rgba(10, 14, 24, 0.95)",
                  border: `1.5px solid ${badge.color}${isSelected ? "ff" : "70"}`,
                  boxShadow: isSelected
                    ? `0 0 16px ${badge.color}60`
                    : isLightMode
                    ? "0 4px 14px -2px rgba(15, 23, 42, 0.12)"
                    : "0 4px 20px -2px rgba(0, 0, 0, 0.75)",
                }}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1 rounded-full backdrop-blur-md cursor-pointer select-none transition-all duration-150",
                  isSelected
                    ? "text-[rgb(var(--foreground))]"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                <span
                  className="w-2 h-2 rounded-full shrink-0"
                  style={{
                    backgroundColor: badge.color,
                    boxShadow: `0 0 6px ${badge.color}`,
                  }}
                />
                <span className="font-mono text-[10px] font-bold tracking-wider uppercase">
                  {badge.collection}
                </span>
                <span className="font-mono text-[9px] opacity-70">
                  ({badge.factCount})
                </span>
              </motion.button>
            </div>
          );
        })}

        {/* Floating Expanded Detail Overlay for Selected Cluster */}
        <AnimatePresence>
          {expandedBadge && (() => {
            const activeBadge = clusterBadges.find((b) => b.collection === expandedBadge);
            if (!activeBadge) return null;

            return (
              <motion.div
                key="badge-detail-modal"
                id={`badge-card-${expandedBadge}`}
                initial={{ opacity: 0, scale: 0.95, y: 8 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                exit={{ opacity: 0, scale: 0.95, y: 8 }}
                transition={{ duration: 0.15, ease: "easeOut" }}
                style={{
                  left: `${Math.min(window.innerWidth - 300, Math.max(16, activeBadge.screenX - 140))}px`,
                  top: `${Math.min(window.innerHeight - 200, Math.max(70, activeBadge.screenY + 24))}px`,
                  backgroundColor: isLightMode ? "#ffffff" : "rgba(10, 14, 24, 0.96)",
                  border: `1.5px solid ${activeBadge.color}60`,
                }}
                className="fixed z-40 p-4 rounded-2xl cursor-default select-none text-[rgb(var(--foreground))] pointer-events-auto shadow-2xl backdrop-blur-2xl w-[280px]"
              >
                <div className="flex flex-col gap-2.5 w-full">
                  <div className="flex items-center justify-between border-b pb-2 border-[rgba(255,255,255,0.08)]">
                    <div className="flex items-center gap-2">
                      <Target size={14} style={{ color: activeBadge.color }} />
                      <span className="text-[12px] font-mono font-bold uppercase tracking-wider">
                        {activeBadge.collection}
                      </span>
                    </div>

                    <div className="flex items-center gap-1.5">
                      <span className="text-[10px] font-mono px-2 py-0.5 rounded-full bg-[rgba(255,255,255,0.08)]">
                        {activeBadge.factCount} Facts
                      </span>
                      <Tooltip label={MEMORY_COPY.closeDetails}>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            onToggleBadge(null);
                          }}
                          className="p-1 rounded-lg hover:bg-white/10 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                        >
                          <X size={13} />
                        </button>
                      </Tooltip>
                    </div>
                  </div>

                  <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] leading-relaxed m-0">
                    {activeBadge.desc}
                  </p>
                </div>
              </motion.div>
            );
          })()}
        </AnimatePresence>
      </div>
    );
  }
);

MemoryGraphClusterBadges.displayName = "MemoryGraphClusterBadges";
