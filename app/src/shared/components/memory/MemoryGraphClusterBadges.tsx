import { memo } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import {
  ClusterBadgeData,
  getCollectionColor,
  getCollectionIcon,
} from "./memoryGraphTypes";

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
          const IconComp = getCollectionIcon(badge.collection);
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
                  backgroundColor: isLightMode ? "#ffffff" : "rgba(var(--glass-navy), 0.98)",
                  border: `1.5px solid ${badge.color}${isSelected ? "ff" : "70"}`,
                  boxShadow: isSelected
                    ? `0 0 16px ${badge.color}60`
                    : isLightMode
                    ? "0 4px 14px -2px rgba(15, 23, 42, 0.12), 0 1px 4px -1px rgba(0, 0, 0, 0.06)"
                    : "0 6px 18px -2px rgba(0, 0, 0, 0.6), 0 2px 4px -1px rgba(0, 0, 0, 0.4)",
                }}
                className="flex items-center gap-2 px-3.5 py-1.5 rounded-full hover:scale-105 transition-transform cursor-pointer select-none text-[rgb(var(--foreground))]"
              >
                <IconComp size={16} style={{ color: badge.color }} className="shrink-0" />
                <span className="text-[12px] font-sans font-black tracking-wider text-[rgb(var(--foreground))] uppercase">
                  {badge.collection}
                </span>
                <span
                  className="text-[11px] font-mono font-bold px-2.5 py-0.5 rounded-full shadow-xs"
                  style={{ backgroundColor: `${badge.color}25`, color: badge.color }}
                >
                  {badge.factCount}
                </span>
              </motion.button>
            </div>
          );
        })}

        {/* Floating / Fixed Overlay Detail Card */}
        <AnimatePresence>
          {expandedBadge && (() => {
            const activeBadge = clusterBadges.find((b) => b.collection === expandedBadge);
            if (!activeBadge) return null;
            const IconComp = getCollectionIcon(activeBadge.collection);
            const isMobile = typeof window !== "undefined" ? window.innerWidth < 640 : false;

            return (
              <motion.div
                key={`expanded-card-${activeBadge.collection}`}
                id={`badge-card-${activeBadge.collection}`}
                initial={{ opacity: 0, scale: 0.94, y: 8 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                exit={{ opacity: 0, scale: 0.94, y: 8 }}
                transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
                onClick={(e) => e.stopPropagation()}
                style={
                  isMobile
                    ? {
                        left: "16px",
                        right: "16px",
                        bottom: "76px",
                        maxWidth: "380px",
                        margin: "0 auto",
                        backgroundColor: isLightMode ? "#ffffff" : "rgba(var(--glass-navy), 0.98)",
                        border: `1.5px solid ${activeBadge.color}`,
                        boxShadow: isLightMode
                          ? "0 12px 32px -4px rgba(15, 23, 42, 0.20), 0 3px 8px -1px rgba(0, 0, 0, 0.10)"
                          : "0 16px 40px -4px rgba(0, 0, 0, 0.85), 0 3px 8px -1px rgba(0, 0, 0, 0.5)",
                      }
                    : {
                        left: `${Math.min(
                          typeof window !== "undefined" ? window.innerWidth - 340 : 500,
                          Math.max(20, activeBadge.screenX - 160)
                        )}px`,
                        top: `${Math.min(
                          typeof window !== "undefined" ? window.innerHeight - 380 : 400,
                          Math.max(80, activeBadge.screenY + 24)
                        )}px`,
                        backgroundColor: isLightMode ? "#ffffff" : "rgba(var(--glass-navy), 0.98)",
                        border: `1.5px solid ${activeBadge.color}`,
                        boxShadow: isLightMode
                          ? "0 12px 32px -4px rgba(15, 23, 42, 0.20), 0 3px 8px -1px rgba(0, 0, 0, 0.10)"
                          : "0 16px 40px -4px rgba(0, 0, 0, 0.85), 0 3px 8px -1px rgba(0, 0, 0, 0.5)",
                      }
                }
                className={cn(
                  "fixed z-40 p-4 sm:p-5 rounded-3xl cursor-default select-none text-[rgb(var(--foreground))] pointer-events-auto shadow-2xl",
                  isMobile
                    ? "max-h-[calc(100vh-150px)] overflow-y-auto custom-scrollbar"
                    : "w-[320px] max-h-[520px] overflow-y-auto custom-scrollbar"
                )}
              >
                <div className="flex flex-col gap-3 w-full">
                  {/* Header with Close Button */}
                  <div
                    className="flex items-center justify-between border-b pb-2.5"
                    style={{ borderColor: `${activeBadge.color}25` }}
                  >
                    <div className="flex items-center gap-2.5">
                      <div
                        className="p-2 rounded-xl flex items-center justify-center shrink-0 shadow-xs"
                        style={{ backgroundColor: `${activeBadge.color}20`, color: activeBadge.color }}
                      >
                        <IconComp size={16} />
                      </div>
                      <div className="flex flex-col">
                        <span className="text-[12px] font-sans font-black tracking-wider uppercase text-[rgb(var(--foreground))]">
                          {activeBadge.collection}
                        </span>
                        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                          {activeBadge.activeFacts} Active Facts
                        </span>
                      </div>
                    </div>

                    <div className="flex items-center gap-2">
                      <span
                        className="text-[11px] font-mono font-bold px-2.5 py-1 rounded-full shadow-xs"
                        style={{ backgroundColor: `${activeBadge.color}25`, color: activeBadge.color }}
                      >
                        {activeBadge.factCount} Memories
                      </span>
                      <Tooltip label={MEMORY_COPY.closeDetails}>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            onToggleBadge(null);
                          }}
                          className="p-1.5 rounded-xl hover:bg-black/10 dark:hover:bg-white/10 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
                        >
                          <X size={14} />
                        </button>
                      </Tooltip>
                    </div>
                  </div>

                  {/* Description */}
                  <p className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] leading-relaxed">
                    {activeBadge.desc}
                  </p>

                  {/* Cross-Collection Directed Edges */}
                  {activeBadge.crossRelations.length > 0 && (
                    <div
                      className="flex flex-col gap-1.5 pt-2 border-t"
                      style={{ borderColor: `${activeBadge.color}20` }}
                    >
                      <div className="flex items-center justify-between px-0.5">
                        <span
                          className="text-[11px] font-bold uppercase tracking-wider"
                          style={{ color: activeBadge.color }}
                        >
                          {MEMORY_COPY.connectedClusters}
                        </span>
                        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                          {activeBadge.totalRelations} Edges
                        </span>
                      </div>

                      <div className="flex flex-col gap-1.5">
                        {activeBadge.crossRelations.map((rel) => {
                          const targetColColor = getCollectionColor(rel.targetCollection, false, isLightMode).main;
                          return (
                            <div
                              key={`${activeBadge.collection}-${rel.relation}-${rel.targetCollection}`}
                              className={cn(
                                "flex items-center justify-between text-[11px] font-sans p-2 rounded-xl border shadow-xs",
                                isLightMode
                                  ? "bg-slate-100 border-slate-200 text-slate-800"
                                  : "bg-white/[0.06] border-white/10 text-white"
                              )}
                            >
                              <div className="flex items-center gap-1.5 font-mono text-[11px] truncate">
                                <span className="font-bold" style={{ color: activeBadge.color }}>
                                  {rel.relation}
                                </span>
                                <span className="text-[rgb(var(--foreground-muted))]">➔</span>
                                <span
                                  className="font-semibold text-[rgb(var(--foreground))] truncate"
                                  style={{ color: targetColColor }}
                                >
                                  {rel.targetCollection}
                                </span>
                              </div>
                              <span
                                className="font-mono font-bold text-[11px] px-2 py-0.5 rounded-full shrink-0 shadow-xs"
                                style={{ backgroundColor: `${activeBadge.color}20`, color: activeBadge.color }}
                              >
                                {rel.count}
                              </span>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  )}
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
