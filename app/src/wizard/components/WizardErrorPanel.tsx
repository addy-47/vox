import React from 'react';
import { AlertTriangle } from 'lucide-react';
import { WIZARD_ERROR_COPY } from '@/data/welcomeCopy';

interface WizardErrorPanelProps {
  message?: string;
  onRetry: () => void;
  onBack: () => void;
}

/**
 * Recovery screen for the setup machine's `error` state. Previously this
 * state rendered the literal string "Unknown State" with no actions.
 */
export const WizardErrorPanel: React.FC<WizardErrorPanelProps> = ({ message, onRetry, onBack }) => (
  <div className="flex flex-col h-full relative">
    <header className="mb-4 sm:mb-6 lg:mb-8 relative shrink-0">
      <div className="mb-2 sm:mb-3">
        <span className="text-[11px] sm:text-[12px] font-mono font-bold tracking-[0.14em] uppercase text-[rgb(var(--danger))]">
          {WIZARD_ERROR_COPY.title}
        </span>
      </div>
      <h1 className="text-2xl sm:text-3xl lg:text-4xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight uppercase mb-2 sm:mb-3">
        {WIZARD_ERROR_COPY.title}
      </h1>
      <p className="text-[rgb(var(--foreground-muted))] text-xs sm:text-sm leading-relaxed max-w-md">
        {message}
      </p>
    </header>

    <div className="flex-1 flex flex-col items-center justify-center text-center">
      <div className="w-14 h-14 sm:w-16 sm:h-16 rounded-2xl bg-[rgba(var(--danger),0.1)] border border-[rgba(var(--danger),0.2)] flex items-center justify-center mb-6">
        <AlertTriangle className="w-6 h-6 sm:w-7 sm:h-7 text-[rgb(var(--danger))]" />
      </div>
    </div>

    <div className="mt-auto pt-4 sm:pt-6 border-t border-[rgba(var(--border),0.06)] pb-[max(0.75rem,env(safe-area-inset-bottom))]">
      <div className="flex flex-col-reverse sm:flex-row gap-3 sm:gap-4">
        <button
          onClick={onBack}
          className="px-4 sm:px-6 py-3.5 sm:py-4 min-h-[44px] text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
        >
          {WIZARD_ERROR_COPY.backToStart}
        </button>
        <button
          onClick={onRetry}
          className="group relative flex-1 py-3.5 sm:py-4 min-h-[48px] text-[rgb(var(--foreground))] font-bold rounded-xl overflow-hidden border transition-all glass-card hover:border-[rgb(var(--accent))]/70 active:scale-[0.98]"
        >
          <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
          <span className="relative z-10 flex items-center justify-center gap-3 uppercase tracking-widest text-[12px]">
            {WIZARD_ERROR_COPY.retry}
          </span>
        </button>
      </div>
    </div>
  </div>
);
