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
    <header className="mb-8 relative shrink-0">
      <div className="mb-4">
        <span className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg text-[12px] font-black tracking-[0.4em] uppercase glass text-[rgb(var(--danger))]">
          {WIZARD_ERROR_COPY.title}
        </span>
      </div>
      <h1 className="text-4xl font-display font-black text-[rgb(var(--foreground))] tracking-tighter uppercase mb-4">
        {WIZARD_ERROR_COPY.title}
      </h1>
      <p className="text-[rgb(var(--foreground-muted))] text-sm leading-relaxed max-w-md">
        {message}
      </p>
    </header>

    <div className="flex-1 flex flex-col items-center justify-center text-center">
      <div className="w-16 h-16 rounded-2xl bg-[rgba(var(--danger),0.1)] border border-[rgb(var(--danger))]/20 flex items-center justify-center mb-6">
        <AlertTriangle className="w-7 h-7 text-[rgb(var(--danger))]" />
      </div>
    </div>

    <div className="mt-auto pt-8 border-t border-[rgba(var(--border),0.05)]">
      <div className="flex gap-4">
        <button
          onClick={onBack}
          className="px-8 py-5 text-[12px] font-black uppercase tracking-[0.3em] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
        >
          {WIZARD_ERROR_COPY.backToStart}
        </button>
        <button
          onClick={onRetry}
          className="group relative flex-1 py-5 text-[rgb(var(--foreground))] font-black rounded-2xl overflow-hidden border transition-all glass-card hover:border-[rgb(var(--accent))]/70 active:scale-[0.98]"
        >
          <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
          <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.4em] text-[12px]">
            {WIZARD_ERROR_COPY.retry}
          </span>
        </button>
      </div>
    </div>
  </div>
);
