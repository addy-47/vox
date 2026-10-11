import React, { memo } from 'react';
import { motion, AnimatePresence } from 'framer-motion';

interface MobileStepFooterProps {
  currentIndex: number;
  totalSteps: number;
  label: string;
  onClick: () => void;
  isLoading?: boolean;
  disabled?: boolean;
}

export const MobileStepFooter: React.FC<MobileStepFooterProps> = memo(({
  currentIndex,
  totalSteps,
  label,
  onClick,
  isLoading = false,
  disabled = false,
}) => {
  return (
    <div className="w-full px-6 pt-2 pb-[max(1.75rem,env(safe-area-inset-bottom))] shrink-0 bg-transparent flex flex-col items-center gap-3.5 select-none z-30 pointer-events-auto transform-gpu">
      {/* Fixed Step Transition Slider (Stays stationary while center content swipes) */}
      <div className="h-3 flex items-center justify-center gap-2">
        {Array.from({ length: totalSteps }).map((_, i) => (
          <div
            key={i}
            className={`h-1.5 rounded-full transition-all duration-300 ${
              i === currentIndex
                ? 'w-7 bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.5)]'
                : 'w-2 bg-[rgba(var(--foreground),0.15)]'
            }`}
          />
        ))}
      </div>

      {/* Contextual Action Button */}
      <div className="w-full max-w-sm">
        <button
          type="button"
          onClick={onClick}
          disabled={disabled || isLoading}
          className="w-full h-12 min-h-[48px] bg-[rgba(var(--accent),0.14)] active:bg-[rgba(var(--accent),0.3)] border border-[rgba(var(--accent),0.35)] text-[rgb(var(--foreground))] active:text-white font-display font-semibold text-[13px] tracking-wide rounded-xl flex items-center justify-center shadow-[0_2px_16px_rgba(var(--accent),0.1)] transition-colors duration-150 touch-manipulation cursor-pointer disabled:opacity-40 disabled:pointer-events-none transform-gpu overflow-hidden"
        >
          <AnimatePresence mode="wait" initial={false}>
            <motion.span
              key={label}
              initial={{ opacity: 0, y: 1 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -1 }}
              transition={{ duration: 0.12, ease: "easeOut" }}
              className="truncate px-2"
            >
              {label}
            </motion.span>
          </AnimatePresence>
        </button>
      </div>
    </div>
  );
});

MobileStepFooter.displayName = 'MobileStepFooter';
