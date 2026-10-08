import React from 'react';
import { ArrowRight } from 'lucide-react';
import { cn } from '@/shared/lib/utils';
import { WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

interface WizardFooterProps {
  onBack?: () => void;
  onNext?: () => void;
  onSkip?: () => void;
  nextLabel?: string;
  isNextDisabled?: boolean;
  isNextLoading?: boolean;
  showBack?: boolean;
  showSkip?: boolean;
  className?: string;
  error?: string;
  errorLabel?: string;
}

export const WizardFooter: React.FC<WizardFooterProps> = ({
  onBack,
  onNext,
  onSkip,
  nextLabel = WIZARD_CTA_LABELS.proceed,
  isNextDisabled = false,
  isNextLoading = false,
  showBack = true,
  showSkip = false,
  className,
  error,
  errorLabel = WIZARD_CTA_LABELS.errorTitle
}) => {
  return (
    <div className={cn("mt-auto pt-4 sm:pt-6 border-t border-[rgba(var(--border),0.06)] pb-[max(0.75rem,env(safe-area-inset-bottom))]", className)}>
      {error && (
        <div className="mb-4 p-3 sm:p-4 bg-[rgba(var(--danger),0.1)] border border-[rgba(var(--danger),0.2)] rounded-xl flex items-center gap-3">
          <div className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--danger))] shrink-0" />
          <div className="flex flex-col min-w-0">
            <span className="text-[11px] font-bold text-[rgb(var(--danger))] uppercase tracking-wider mb-0.5">{errorLabel}</span>
            <p className="text-[12px] text-[rgb(var(--danger))] font-medium leading-relaxed">{error}</p>
          </div>
        </div>
      )}
      
      <div className="flex flex-col-reverse sm:flex-row gap-3 sm:gap-4">
        {showBack && (
          <button
            onClick={onBack}
            className="px-4 sm:px-6 py-3.5 sm:py-4 min-h-[44px] text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
          >
            {WIZARD_CTA_LABELS.back}
          </button>
        )}

        {showSkip && onSkip && (
          <button
            onClick={onSkip}
            className="px-4 sm:px-6 py-3.5 sm:py-4 text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors border border-dashed border-[rgba(var(--border),0.12)] hover:border-[rgb(var(--accent))]/30 rounded-xl glass transition-all"
          >
            {WIZARD_CTA_LABELS.skip}
          </button>
        )}

        <button
          onClick={onNext}
          disabled={isNextDisabled || isNextLoading}
          className={cn(
            "group relative flex-1 py-3.5 sm:py-4 min-h-[48px] text-[rgb(var(--foreground))] font-bold rounded-xl overflow-hidden border transition-all glass-card",
            (isNextDisabled || isNextLoading) ? "opacity-50 cursor-not-allowed" : "hover:border-[rgb(var(--accent))]/70 active:scale-[0.98]"
          )}
        >
          <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
          <span className="relative z-10 flex items-center justify-center gap-3 uppercase tracking-widest text-[12px]">
            {isNextLoading ? WIZARD_CTA_LABELS.processing : nextLabel}
            {!isNextLoading && <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1 text-[rgb(var(--accent))]" />}
          </span>
        </button>
      </div>
    </div>
  );
};
