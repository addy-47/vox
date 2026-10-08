import { memo, useEffect, useRef, useState, useMemo } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Clock, Layers, ArrowRight } from "lucide-react";
import { ObservationRecord } from "@/services/memoryService";
import {
  type SessionRow,
  resolveSessionTitle,
  formatSessionRecency,
} from "@/services/historyService";
import {
  getActiveDynamicPalette,
  toMemoryCategory,
  type MemoryCategory,
} from "./memoryGraphTypes";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { MEMORY_COPY } from "@/data/memoryCopy";

interface SessionNodeTooltipProps {
  session: SessionRow | null;
  facts: ObservationRecord[];
  pos: { x: number; y: number } | null;
  onClose: () => void;
  onFocusSession?: (sessionId: string) => void;
  isLightMode?: boolean;
}

const CATEGORY_ORDER: MemoryCategory[] = [
  "objective",
  "workdone",
  "blocker",
  "next_step",
  "pitfall",
  "personal",
];

export const SessionNodeTooltip = memo(({
  session,
  facts,
  pos,
  onClose,
  onFocusSession,
  isLightMode = false,
}: SessionNodeTooltipProps) => {
  const tooltipRef = useRef<HTMLDivElement>(null);

  // Global overlay stack integration for Escape + outside click
  useOverlay({ onClose, ref: tooltipRef, dismissOnOutside: true });

  const palette = useMemo(() => getActiveDynamicPalette(isLightMode), [isLightMode]);

  const [isCompactViewport, setIsCompactViewport] = useState(
    () => typeof window !== "undefined" && window.matchMedia("(max-width: 639px)").matches
  );

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const query = window.matchMedia("(max-width: 639px)");
    const onChange = (e: MediaQueryListEvent) => setIsCompactViewport(e.matches);
    setIsCompactViewport(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  const isMobile = isCompactViewport;
  const tooltipWidth = 360;
  const clampedX =
    !pos || isMobile
      ? 16
      : Math.min(window.innerWidth - tooltipWidth - 24, Math.max(24, pos.x + 16));

  const isUpperHalf = pos && typeof window !== "undefined" ? pos.y < window.innerHeight / 2 : true;
  const mobileTop = isUpperHalf
    ? 80
    : typeof window !== "undefined"
    ? Math.max(80, window.innerHeight - 300)
    : 80;

  const clampedY =
    !pos
      ? 80
      : isMobile
      ? mobileTop
      : Math.min(window.innerHeight - 280, Math.max(80, pos.y - 16));

  const title = session ? resolveSessionTitle(session) : "";
  const recency = session ? formatSessionRecency(session.updated_at) : "";
  const createdDate = session ? new Date(session.created_at).toLocaleDateString() : "";

  // Category counts
  const categoryCounts = useMemo(() => {
    const counts = new Map<MemoryCategory, number>();
    for (const f of facts) {
      const cat = toMemoryCategory(f.fact_type) || "objective";
      counts.set(cat, (counts.get(cat) || 0) + 1);
    }
    return counts;
  }, [facts]);

  const presentCategories = useMemo(() => {
    return CATEGORY_ORDER.filter((cat) => (categoryCounts.get(cat) ?? 0) > 0);
  }, [categoryCounts]);

  return (
    <AnimatePresence>
      {pos && session && (
        <div
          ref={tooltipRef}
          style={{ left: `${clampedX}px`, top: `${clampedY}px` }}
          className="fixed z-50 pointer-events-auto select-none"
        >
          <motion.div
            initial={{ opacity: 0, scale: 0.94, y: 6 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.94, y: 6 }}
            transition={{ duration: 0.15, ease: "easeOut" }}
            className="w-[350px] max-w-[calc(100vw-32px)] rounded-xl glass-card border border-[rgba(var(--accent),0.22)] bg-[rgba(var(--card),0.96)] p-4 shadow-2xl"
          >
            {/* Header: Pure unboxed typography - Project text then bigger session title (NO PILLS) */}
            <div className="pb-3 border-b border-[rgba(var(--border),0.10)]">
              <div className="flex items-center justify-between gap-2">
                <span className="text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/70">
                  {session.project_id ? `Project · ${session.project_id}` : "Default Project"}
                </span>
                <button
                  type="button"
                  onClick={onClose}
                  className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer shrink-0"
                >
                  <X size={14} />
                </button>
              </div>

              <h4 className="font-display font-semibold text-[14px] text-[rgb(var(--foreground))] leading-snug mt-1 break-words">
                {title}
              </h4>

              <div className="flex items-center gap-2 mt-1.5 text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/70">
                <Clock size={11} className="shrink-0" />
                <span>{recency}</span>
                <span className="opacity-40">·</span>
                <span>{createdDate}</span>
                {session.turn_count > 0 && (
                  <>
                    <span className="opacity-40">·</span>
                    <span>{session.turn_count} turns</span>
                  </>
                )}
              </div>
            </div>

            {/* Memory Distribution Breakdown */}
            <div className="py-3 space-y-2">
              <div className="flex items-center justify-between text-[11px] font-mono text-[rgb(var(--foreground-muted))]/75">
                <span className="flex items-center gap-1.5 font-semibold uppercase tracking-wider text-[10px]">
                  <Layers size={11} className="text-[rgb(var(--accent))]" />
                  Extracted Memories
                </span>
                <span className="text-[rgb(var(--accent))] font-medium">
                  {facts.length}
                </span>
              </div>

              {presentCategories.length === 0 ? (
                <div className="py-2 text-[11.5px] font-sans text-[rgb(var(--foreground-muted))]/40 italic">
                  No memories linked to this session
                </div>
              ) : (
                <div className="grid grid-cols-2 gap-x-5 gap-y-1.5 pt-1">
                  {presentCategories.map((catKey) => {
                    const count = categoryCounts.get(catKey) || 0;
                    const catColor = palette[catKey]?.main || "#00dbe9";
                    const label = MEMORY_COPY.categories[catKey] || catKey;

                    return (
                      <div
                        key={catKey}
                        className="flex items-center justify-between py-0.5 text-[11px] font-mono"
                      >
                        <div className="flex items-center gap-2 min-w-0">
                          <span
                            className="w-1.5 h-1.5 rounded-full shrink-0"
                            style={{
                              backgroundColor: catColor,
                              boxShadow: `0 0 3px ${catColor}80`,
                            }}
                          />
                          <span className="truncate text-[11.5px] font-sans text-[rgb(var(--foreground-muted))]">
                            {label}
                          </span>
                        </div>
                        <span className="font-mono text-[11px] font-medium text-[rgb(var(--foreground))] ml-1 shrink-0">
                          {count}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Footer Action: Frameless interactive action */}
            {onFocusSession && (
              <div className="pt-2.5 border-t border-[rgba(var(--border),0.08)] flex justify-end">
                <button
                  type="button"
                  onClick={() => {
                    onFocusSession(String(session.id));
                    onClose();
                  }}
                  className="flex items-center gap-1.5 text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer bg-transparent border-none p-0"
                >
                  <span>Focus in Timeline</span>
                  <ArrowRight size={12} />
                </button>
              </div>
            )}
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
});

SessionNodeTooltip.displayName = "SessionNodeTooltip";
