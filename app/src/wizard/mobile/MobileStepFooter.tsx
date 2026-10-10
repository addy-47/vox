import React from 'react';
import { ArrowLeft, ArrowRight } from 'lucide-react';
import { WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

interface MobileStepFooterProps {
  currentIndex: number;
  totalSteps?: number;
  onNext: () => void;
  onBack?: () => void;
  nextLabel: string;
  nextIcon?: React.ReactNode;
  isLoading?: boolean;
  disabled?: boolean;
  onSelectStep?: (index: number) => void;
}

export const MobileStepFooter: React.FC<MobileStepFooterProps> = ({
  currentIndex,
  totalSteps = 5,
  onNext,
  onBack,
  nextLabel,
  nextIcon,
  isLoading = false,
  disabled = false,
  onSelectStep,
}) => {
  const showBack = currentIndex > 0 && !!onBack;

  return (
    <div className="w-full px-6 pt-2 pb-[max(1.5rem,env(safe-area-inset-bottom))] shrink-0 bg-transparent flex flex-col items-center gap-3.5 select-none">
      {/* Step-wise Slider / 5-segment indicator */}
      <div className="flex items-center justify-center gap-2 py-1">
        {Array.from({ length: totalSteps }).map((_, i) => (
          <button
            key={i}
            onClick={() => onSelectStep?.(i)}
            disabled={!onSelectStep}
            className="p-1 focus:outline-none touch-manipulation cursor-pointer"
            aria-label={`Step ${i + 1}`}
          >
            <div
              className={`h-1.5 rounded-full transition-all duration-300 ${
                i === currentIndex
                  ? 'w-7 bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.5)]'
                  : 'w-2 bg-[rgba(var(--foreground),0.15)]'
              }`}
            />
          </button>
        ))}
      </div>

      {/* Navigation Buttons: Refined ergonomic layout with subtle luminous glass accent */}
      <div className="flex flex-col items-center gap-2 w-full max-w-sm">
        <button
          type="button"
          onClick={onNext}
          disabled={disabled || isLoading}
          className="w-full h-12 bg-[rgba(var(--accent),0.16)] hover:bg-[rgba(var(--accent),0.24)] active:bg-[rgba(var(--accent),0.32)] border border-[rgba(var(--accent),0.4)] text-[rgb(var(--foreground))] hover:text-white font-display font-semibold text-[13px] tracking-wide rounded-xl flex items-center justify-center gap-2 shadow-[0_2px_16px_rgba(var(--accent),0.12)] active:scale-[0.98] transition-all touch-manipulation cursor-pointer disabled:opacity-40"
        >
          <span>{nextLabel}</span>
          {nextIcon ?? <ArrowRight className="w-3.5 h-3.5 text-[rgb(var(--accent))]" />}
        </button>

        {showBack && (
          <button
            type="button"
            onClick={onBack}
            className="w-full h-10 bg-transparent hover:bg-white/[0.04] active:bg-white/[0.08] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] font-display font-medium text-xs tracking-wide rounded-xl flex items-center justify-center gap-1.5 active:scale-[0.98] transition-all touch-manipulation cursor-pointer"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span>{WIZARD_CTA_LABELS.back}</span>
          </button>
        )}
      </div>
    </div>
  );
};
