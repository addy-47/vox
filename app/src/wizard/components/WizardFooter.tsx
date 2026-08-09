import React from 'react';
import { ArrowRight } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

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
  nextLabel = "Proceed",
  isNextDisabled = false,
  isNextLoading = false,
  showBack = true,
  showSkip = false,
  className,
  error,
  errorLabel = "Error"
}) => {
  return (
    <div className={cn("mt-auto pt-8 border-t border-[rgba(var(--border),0.05)]", className)}>
      {error && (
        <div className="mb-6 p-4 bg-red-500/10 border border-red-500/20 rounded-xl flex items-center gap-3">
          <div className="w-1.5 h-1.5 rounded-full bg-red-500 animate-pulse" />
          <div className="flex flex-col">
            <span className="text-[12px] font-black text-red-500 uppercase tracking-widest mb-0.5">{errorLabel}</span>
            <p className="text-[12px] text-red-400 font-medium uppercase tracking-wider">{error}</p>
          </div>
        </div>
      )}
      
      <div className="flex gap-4">
        {showBack && (
          <button
            onClick={onBack}
            className="px-8 py-5 text-[12px] font-black uppercase tracking-[0.3em] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
          >
            Back
          </button>
        )}

        {showSkip && onSkip && (
          <button
            onClick={onSkip}
            className="px-6 py-5 text-[12px] font-black uppercase tracking-[0.3em] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] transition-colors border border-dashed border-[rgba(var(--border),0.08)] hover:border-[rgb(var(--accent))]/30 rounded-2xl glass transition-all duration-300"
          >
            Skip
          </button>
        )}

        <button
          onClick={onNext}
          disabled={isNextDisabled || isNextLoading}
          className={cn(
            "group relative flex-1 py-5 text-white font-black rounded-2xl overflow-hidden border transition-all glass-card",
            (isNextDisabled || isNextLoading) ? "opacity-50 cursor-not-allowed" : "hover:border-[rgb(var(--accent))]/70 active:scale-[0.98]"
          )}
        >
          <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
          <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.4em] text-[12px]">
            {isNextLoading ? 'Processing...' : nextLabel}
            {!isNextLoading && <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1 text-[rgb(var(--accent))]" />}
          </span>
        </button>
      </div>
    </div>
  );
};
