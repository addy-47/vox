import React from 'react';
import { TRAY_COPY } from '@/data/trayCopy';
import { X, Copy, Check, Mic } from 'lucide-react';

interface HeaderProps {
  isListening: boolean;
  hasContent: boolean;
  copied: boolean;
  interactionMode: string;
  silenceAutoStopMs?: number;
  onCopy: () => void;
  onClose: () => void;
  onTogglePtt: () => void;
  pttBusy?: boolean;
  micError?: string | null;
}

export const Header: React.FC<HeaderProps> = React.memo(({
  isListening, hasContent, copied, interactionMode, silenceAutoStopMs,
  onCopy, onClose, onTogglePtt, pttBusy = false, micError = null
}) => {
  return (
    <div className="px-6 py-4 flex items-center justify-between relative z-10" data-tauri-drag-region>
      <div className="flex items-center gap-2.5">
        <div className="relative flex items-center justify-center">
          <div
            className={`w-2 h-2 rounded-full transition-all duration-300 ${
              isListening
                ? 'bg-[rgb(var(--accent))] ring-2 ring-[rgb(var(--accent))]/25'
                : 'bg-[rgb(var(--foreground))]/30'
            }`}
          />
        </div>
        <span className="text-[12px] font-black tracking-[0.3em] text-[rgb(var(--foreground))]/90 uppercase">
          {TRAY_COPY.brand} <span className="text-[rgb(var(--accent))]">{TRAY_COPY.live}</span>
        </span>
        {silenceAutoStopMs !== undefined && (
          silenceAutoStopMs > 0 ? (
            <span className="text-[8.5px] font-mono font-bold tracking-wider px-1.5 py-0.5 rounded-md bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.25)] select-none">
              {(silenceAutoStopMs / 1000).toFixed(1)}s Pause
            </span>
          ) : (
            <span className="text-[8.5px] font-mono font-bold tracking-wider px-1.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]/60 select-none">
              Manual
            </span>
          )
        )}
      </div>
      
      <div className="flex items-center gap-1">
        {interactionMode?.toUpperCase() === 'PTT' && (
          <button
            onClick={(e) => { e.stopPropagation(); onTogglePtt(); }}
            disabled={pttBusy}
            aria-label={isListening ? TRAY_COPY.stopRecording : TRAY_COPY.startRecording}
            title={micError ?? undefined}
            className={`p-2 rounded-lg transition-all active:scale-90 ${pttBusy ? 'opacity-50 animate-pulse pointer-events-none' : ''} ${micError ? 'text-[rgb(var(--error))]' : isListening ? 'text-[rgb(var(--accent))]' : 'text-[rgb(var(--foreground))]/60 hover:text-[rgb(var(--foreground))]/90'}`}
          >
            <Mic size={16} />
          </button>
        )}

        {hasContent && (
          <button 
            onClick={(e) => { e.stopPropagation(); onCopy(); }}
            className="p-2 rounded-lg transition-all text-[rgb(var(--foreground))]/60 hover:text-[rgb(var(--accent))] active:scale-90"
            aria-label={TRAY_COPY.copyClipboard}
          >
            {copied ? <Check size={16} /> : <Copy size={16} />}
          </button>
        )}

        <button 
          onClick={(e) => { e.stopPropagation(); onClose(); }}
          className="p-2 rounded-lg transition-all text-[rgb(var(--foreground))]/50 hover:text-[rgb(var(--foreground))]/90 active:scale-90"
          aria-label={TRAY_COPY.closeCommit}
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
});
