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
    <div className={cn("mt-auto pt-8 border-t border-white/5", className)}>
      {error && (
        <div className="mb-6 p-4 bg-red-500/10 border border-red-500/20 rounded-xl flex items-center gap-3">
          <div className="w-1.5 h-1.5 rounded-full bg-red-500 animate-pulse" />
          <div className="flex flex-col">
            <span className="text-[9px] font-black text-red-500 uppercase tracking-widest mb-0.5">{errorLabel}</span>
            <p className="text-[11px] text-red-400 font-medium uppercase tracking-wider">{error}</p>
          </div>
        </div>
      )}
      
      <div className="flex gap-4">
        {showBack && (
          <button
            onClick={onBack}
            className="px-8 py-5 text-[11px] font-black uppercase tracking-[0.3em] text-white/40 hover:text-white transition-colors"
          >
            Back
          </button>
        )}

        {showSkip && onSkip && (
          <button
            onClick={onSkip}
            className="px-6 py-5 text-[11px] font-black uppercase tracking-[0.3em] text-white/40 hover:text-[#00dbe9] transition-colors border border-dashed border-white/10 hover:border-[#00dbe9]/30 rounded-2xl bg-white/[0.01] transition-all duration-300"
          >
            Skip
          </button>
        )}

        <button
          onClick={onNext}
          disabled={isNextDisabled || isNextLoading}
          className={cn(
            "group relative flex-1 py-5 bg-zinc-950 text-white font-black rounded-2xl overflow-hidden border border-white/10 transition-all shadow-[0_0_40px_rgba(0,0,0,0.5)]",
            (isNextDisabled || isNextLoading) ? "opacity-50 cursor-not-allowed" : "hover:bg-zinc-900 hover:border-[#00dbe9]/50 active:scale-[0.98]"
          )}
        >
          <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
          <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.4em] text-[11px]">
            {isNextLoading ? 'Processing...' : nextLabel}
            {!isNextLoading && <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1 text-[#00dbe9]" />}
          </span>
        </button>
      </div>
    </div>
  );
};
