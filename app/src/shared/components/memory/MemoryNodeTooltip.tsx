import { memo, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Clock } from "lucide-react";
import { FactRecord } from "@/services/memoryService";
import { getCollectionColor } from "./memoryGraphTypes";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { MEMORY_COPY } from "@/data/memoryCopy";

interface MemoryNodeTooltipProps {
  factDetail: FactRecord | null;
  pos: { x: number; y: number } | null;
  onClose: () => void;
  isLightMode?: boolean;
}

export const MemoryNodeTooltip = memo(({
  factDetail,
  pos,
  onClose,
  isLightMode = false,
}: MemoryNodeTooltipProps) => {
  const tooltipRef = useRef<HTMLDivElement>(null);

  // Escape + outside-click dismissal via global overlay stack
  useOverlay({ onClose, ref: tooltipRef, dismissOnOutside: true });

  if (!pos || !factDetail) return null;

  const colStyle = getCollectionColor(factDetail.fact_type, false, isLightMode);
  const isMobile = typeof window !== "undefined" ? window.innerWidth < 640 : false;
  const tooltipWidth = 360;
  const clampedX = isMobile
    ? 16
    : Math.min(window.innerWidth - tooltipWidth - 24, Math.max(24, pos.x + 16));
  const clampedY = isMobile
    ? 90
    : Math.min(window.innerHeight - 280, Math.max(80, pos.y - 16));

  return (
    <AnimatePresence>
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
          className="w-[340px] rounded-3xl glass-card border border-[rgba(255,255,255,0.12)] bg-[rgba(10,14,24,0.94)] backdrop-blur-2xl p-4 shadow-[0_16px_40px_rgba(0,0,0,0.7)]"
        >
          {/* Header */}
          <div className="flex items-center justify-between pb-2.5 mb-2.5 border-b border-[rgba(255,255,255,0.08)]">
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

            <div className="flex items-center gap-2">
              {factDetail.session_id !== null ? (
                <span className="text-[9px] font-mono px-2 py-0.5 rounded-full bg-[rgba(255,255,255,0.06)] text-[rgb(var(--foreground-muted))]">
                  {MEMORY_COPY.sessionPrefix}{factDetail.session_id}
                </span>
              ) : (
                <span className="text-[9px] font-mono px-2 py-0.5 rounded-full bg-[rgba(0,219,233,0.15)] text-[rgb(var(--accent))]">
                  Identity Core
                </span>
              )}
              <button
                type="button"
                onClick={onClose}
                className="p-1 rounded-lg text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(255,255,255,0.06)] transition-colors cursor-pointer"
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
          <div className="mt-3 pt-2.5 border-t border-[rgba(255,255,255,0.06)] flex items-center justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
            <div className="flex items-center gap-1.5 opacity-75">
              <Clock size={11} />
              <span>{new Date(factDetail.created_at).toLocaleDateString()}</span>
            </div>
            <span className="opacity-60 uppercase tracking-wider">
              {factDetail.status}
            </span>
          </div>
        </motion.div>
      </div>
    </AnimatePresence>
  );
});

MemoryNodeTooltip.displayName = "MemoryNodeTooltip";
