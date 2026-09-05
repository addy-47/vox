import React, { memo, useEffect, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";

/**
 * Single reverse-flow ambient animation toward the central orb,
 * communicating that previous context is being ingested on session
 * restore (spec §B.15). Runs once per restore `signal`, never blocks
 * interaction (pointer-events-none), and never replays on subsequent
 * turns — the parent only bumps `signal` on a successful restore.
 */
export const RestorePulse: React.FC<{ signal: number }> = memo(({ signal }) => {
  const [visible, setVisible] = useState(false);
  const mountedRef = useRef(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!mountedRef.current) {
      mountedRef.current = true;
      return;
    }
    if (signal <= 0) return;
    setVisible(true);
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => setVisible(false), 1400);
    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [signal]);

  return (
    <div className="absolute inset-0 z-10 pointer-events-none flex items-center justify-center overflow-hidden">
      <AnimatePresence>
        {visible && (
          <motion.div
            key={signal}
            initial={{ opacity: 0.7, scale: 1.6 }}
            animate={{ opacity: 0, scale: 0.55 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 1.2, ease: [0.16, 1, 0.3, 1] }}
            className="w-[min(60vw,52vh)] h-[min(60vw,52vh)] max-w-[540px] max-h-[540px] rounded-full border-2 border-[rgb(var(--accent))]"
          />
        )}
      </AnimatePresence>
    </div>
  );
});
RestorePulse.displayName = "RestorePulse";
