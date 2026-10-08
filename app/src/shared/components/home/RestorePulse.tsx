import React, { memo, useEffect, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useSessionStore } from "@/store/sessionStore";

/**
 * Ambient ingestion pulse toward the central orb while a previous session
 * is being continued. Visibility is driven by the live restore state —
 * never by navigation — with a minimum display floor so fast restores
 * still read as intentional instead of a flicker. Never blocks
 * interaction (pointer-events-none).
 */
const MIN_VISIBLE_MS = 300;
const RESTORE_STALL_CAP_MS = 8000;

export const RestorePulse: React.FC<{ signal: number }> = memo(({ signal }) => {
  const [visible, setVisible] = useState(false);
  const shownAtRef = useRef(0);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isRestoring = useSessionStore((s) => s.isRestoring);
  const restoringSessionId = useSessionStore((s) => s.restoringSessionId);

  useEffect(() => {
    console.info(`[trace-restore] Pulse state signal=${signal} isRestoring=${isRestoring} restoringSessionId=${restoringSessionId} @${performance.now().toFixed(1)}ms`);
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    if (isRestoring) {
      shownAtRef.current = performance.now();
      setVisible(true);
      // Safety: never trap the pulse on screen if the store stalls mid-restore.
      timerRef.current = setTimeout(() => setVisible(false), RESTORE_STALL_CAP_MS);
    } else if (shownAtRef.current > 0) {
      const wait = Math.max(0, MIN_VISIBLE_MS - (performance.now() - shownAtRef.current));
      timerRef.current = setTimeout(() => setVisible(false), wait);
    }
    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [isRestoring, signal, restoringSessionId]);

  return (
    <div
      className="absolute inset-0 z-10 pointer-events-none flex items-center justify-center overflow-hidden"
      style={{ transform: "translateY(-36px)" }}
    >
      <AnimatePresence>
        {visible && (
          <motion.div
            key={signal}
            initial={{ opacity: 0.42, scale: 1.65 }}
            animate={{ opacity: 0, scale: 0.5 }}
            exit={{ opacity: 0, transition: { duration: 0.3 } }}
            transition={{ duration: 2.4, ease: [0.16, 1, 0.3, 1] }}
            className="w-[min(60vw,52vh)] h-[min(60vw,52vh)] max-w-[540px] max-h-[540px] rounded-full border border-[rgba(var(--accent),0.28)] blur-[1px] shadow-[0_0_24px_rgba(var(--accent),0.16),inset_0_0_16px_rgba(var(--accent),0.1)]"
          />
        )}
      </AnimatePresence>
    </div>
  );
});
RestorePulse.displayName = "RestorePulse";
