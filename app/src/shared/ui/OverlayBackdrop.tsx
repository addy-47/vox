import { memo } from "react";
import { motion } from "framer-motion";
import { cn } from "@/shared/lib/utils";

/**
 * Single shared overlay backdrop (Tier 1 perf: one source of truth).
 * Visual contract is unchanged: dim + slight blur + click-outside close.
 * Modal, Drawer, and EdgePanel all render this — no more three divergent
 * backdrop implementations, and stacked overlays share identical styling
 * so double-open no longer double-tunes darkness/blur independently.
 */
export const OverlayBackdrop = memo(
  ({
    onClose,
    className,
    zIndex,
  }: {
    onClose?: () => void;
    className?: string;
    zIndex?: number;
  }) => (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.2 }}
      onClick={onClose}
      aria-hidden="true"
      style={zIndex !== undefined ? { zIndex } : undefined}
      className={cn(
        "fixed inset-0 bg-black/50 backdrop-blur-[2px] pointer-events-auto",
        className
      )}
    />
  )
);

OverlayBackdrop.displayName = "OverlayBackdrop";
