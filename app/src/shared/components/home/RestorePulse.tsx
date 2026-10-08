import React, { memo, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useSessionStore } from "@/store/sessionStore";

/**
 * Ambient inward ripple converging into the central orb edge while a previous session
 * is being restored/continued. Visibility is bound to live restore state (1400ms window).
 * Position is precisely locked to the central orb stage coordinates.
 * Target scale terminates at 0.55 (the outer perimeter of the 3D orb sphere).
 */
export const RestorePulse: React.FC<{ signal: number }> = memo(({ signal }) => {
  const isRestoring = useSessionStore((s) => s.isRestoring);

  useEffect(() => {
    if (isRestoring) {
      console.info(`[trace-restore] RestorePulse ripple mounted signal=${signal} targetScale=0.58 duration=1.7s`);
    }
  }, [isRestoring, signal]);

  return (
    <div
      className="absolute pointer-events-none z-10 flex items-center justify-center overflow-visible select-none"
      style={{
        left: "50%",
        top: "calc(50% - 36px)",
        transform: "translate(-50%, -50%)",
        width: "min(65vw, 56vh)",
        height: "min(65vw, 56vh)",
        maxWidth: 580,
        maxHeight: 580,
      }}
    >
      <AnimatePresence mode="wait">
        {isRestoring && (
          <motion.div
            key="restore-single-ripple"
            initial={{ opacity: 0, scale: 1.5 }}
            animate={{
              opacity: [0, 0.45, 0.45, 0],
              scale: [1.5, 1.2, 0.82, 0.58],
            }}
            exit={{ opacity: 0 }}
            transition={{
              duration: 1.7,
              times: [0, 0.15, 0.85, 1],
              ease: [0.22, 1, 0.36, 1], // Natural quintic deceleration terminating cleanly at orb edge
            }}
            className="w-full h-full rounded-full border border-[rgba(var(--accent),0.4)] transform-gpu will-change-transform shadow-[0_0_14px_rgba(var(--accent),0.25)]"
          />
        )}
      </AnimatePresence>
    </div>
  );
});

RestorePulse.displayName = "RestorePulse";
