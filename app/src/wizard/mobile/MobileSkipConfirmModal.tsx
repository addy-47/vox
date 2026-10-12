import React, { memo } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { AlertCircle, ArrowRight, Settings } from 'lucide-react';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

interface MobileSkipConfirmModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirmSkip: () => void;
}

export const MobileSkipConfirmModal: React.FC<MobileSkipConfirmModalProps> = memo(({
  isOpen,
  onClose,
  onConfirmSkip,
}) => {
  const copy = MOBILE_ONBOARDING_COPY.skipModal;

  return (
    <AnimatePresence>
      {isOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 select-none pointer-events-auto">
          {/* Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            onClick={onClose}
            className="absolute inset-0 bg-black/70 backdrop-blur-sm"
          />

          {/* Modal Panel */}
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 8 }}
            transition={{ type: 'spring', damping: 25, stiffness: 350 }}
            className="relative w-full max-w-sm rounded-2xl bg-[rgb(var(--card))] border border-[rgba(var(--foreground),0.1)] p-6 shadow-lg flex flex-col items-center text-center space-y-4 overflow-hidden z-10"
          >
            {/* Warning Icon Badge */}
            <div className="w-12 h-12 rounded-2xl bg-[rgb(var(--warning))]/12 border border-[rgb(var(--warning))]/25 flex items-center justify-center text-[rgb(var(--warning))] shrink-0">
              <AlertCircle className="w-6 h-6 stroke-[2]" />
            </div>

            {/* Headline & Body */}
            <div className="space-y-2">
              <h2 className="text-lg font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
                {copy.title}
              </h2>
              <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
                {copy.description}
              </p>
            </div>

            {/* Explanatory Settings Location Callout */}
            <div className="w-full p-3 rounded-xl bg-white/[0.03] border border-white/[0.06] flex items-start gap-2.5 text-left">
              <Settings className="w-4 h-4 text-[rgb(var(--accent))] shrink-0 mt-0.5" />
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-normal">
                {copy.settingsNote}
              </p>
            </div>

            {/* Action Buttons */}
            <div className="w-full pt-2 flex flex-col gap-2">
              {/* Primary: Stay in Onboarding */}
              <button
                type="button"
                onClick={onClose}
                className="w-full min-h-[48px] rounded-xl bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] font-display font-semibold text-xs tracking-wide flex items-center justify-center gap-2 shadow-lg shadow-[rgba(var(--accent),0.2)] active:scale-[0.98] transition-transform touch-manipulation cursor-pointer"
              >
                <span>{copy.cancelButton}</span>
                <ArrowRight className="w-4 h-4" />
              </button>

              {/* Secondary Ghost: Proceed to Skip */}
              <button
                type="button"
                onClick={onConfirmSkip}
                className="w-full min-h-[48px] rounded-xl bg-white/[0.03] hover:bg-white/[0.06] active:bg-white/[0.08] text-[rgb(var(--foreground-muted))] font-mono text-[11px] uppercase tracking-wider transition-colors touch-manipulation cursor-pointer"
              >
                {copy.confirmButton}
              </button>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
});

MobileSkipConfirmModal.displayName = 'MobileSkipConfirmModal';
