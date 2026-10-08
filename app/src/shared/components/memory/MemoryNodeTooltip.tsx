import { memo, useEffect, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Clock } from "lucide-react";
import { ObservationRecord } from "@/services/memoryService";
import { getCollectionColor } from "./memoryGraphTypes";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { MEMORY_COPY } from "@/data/memoryCopy";

interface MemoryNodeTooltipProps {
  factDetail: ObservationRecord | null;
  pos: { x: number; y: number } | null;
  onClose: () => void;
  isLightMode?: boolean;
  sessionTitle?: string | null;
}

export const MemoryNodeTooltip = memo(({
  factDetail,
  pos,
  onClose,
  isLightMode = false,
  sessionTitle = null,
}: MemoryNodeTooltipProps) => {
  const tooltipRef = useRef<HTMLDivElement>(null);

  // Escape + outside-click dismissal via global overlay stack
  useOverlay({ onClose, ref: tooltipRef, dismissOnOutside: true });

  // NOTE: no early return here. AnimatePresence needs the tree mounted for the
  // exit animation to play — returning null first unmounts everything and the
  // `exit` below could never fire (the tooltip previously vanished instantly).
  const colStyle = factDetail
    ? getCollectionColor(factDetail.fact_type, false, isLightMode)
    : null;
  // Compact placement tracks viewport resizes/rotation instead of a
  // one-shot innerWidth read that goes stale.
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

  return (
    <AnimatePresence>
      {pos && factDetail && colStyle && (
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
          className="w-[340px] max-w-[calc(100vw-32px)] rounded-xl glass-card border border-[rgba(var(--border),0.18)] bg-[rgba(var(--card),0.96)] p-4 shadow-2xl"
        >
          {/* Header */}
          <div className="flex items-center justify-between pb-2.5 mb-2.5 border-b border-[rgba(var(--border),0.10)]">
            <div className="flex items-center gap-2">
              <span
                className="w-2.5 h-2.5 rounded-full shrink-0"
                style={{
                  background: colStyle.main,
                  boxShadow: `0 0 8px ${colStyle.glow}`,
                }}
              />
              <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {factDetail.fact_type.replace("_", " ")}
              </span>
            </div>

            <div className="flex items-center gap-2 max-w-[190px]">
              {factDetail.session_id !== null ? (
                <span
                  title={sessionTitle || undefined}
                  className="truncate text-[11px] font-sans font-medium text-[rgb(var(--foreground-muted))]/80 text-right"
                >
                  {sessionTitle || `${MEMORY_COPY.sessionPrefix}${factDetail.session_id}`}
                </span>
              ) : (
                <span className="text-[10.5px] font-mono text-[rgb(var(--accent))]">
                  {MEMORY_COPY.identityLayer}
                </span>
              )}
              <button
                type="button"
                onClick={onClose}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] transition-colors cursor-pointer shrink-0"
              >
                <X size={14} />
              </button>
            </div>
          </div>

          {/* Fact Content Text */}
          <div className="py-1">
            <p className="text-[13px] font-sans text-[rgb(var(--foreground))] leading-relaxed select-text m-0 break-words">
              {factDetail.text}
            </p>
          </div>

          {/* Footer Metadata */}
          <div className="mt-3 pt-2.5 border-t border-[rgba(var(--border),0.08)] flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
            <div className="flex items-center gap-1.5 opacity-75">
              <Clock size={11} />
              <span>{new Date(factDetail.created_at).toLocaleDateString()}</span>
            </div>
          </div>
        </motion.div>
      </div>
      )}
    </AnimatePresence>
  );
});

MemoryNodeTooltip.displayName = "MemoryNodeTooltip";
