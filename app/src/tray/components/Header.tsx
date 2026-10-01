import React from 'react';
import { TRAY_COPY } from '@/data/trayCopy';
import { motion } from 'framer-motion';
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
          <motion.div 
            animate={{ 
              scale: isListening ? [1, 1.3, 1] : 1, 
              opacity: isListening ? [0.5, 0.2, 0.5] : 0.1 
            }}
            transition={{ repeat: Infinity, duration: 2 }}
            className="absolute w-5 h-5 rounded-full bg-[rgb(var(--accent))] blur-md"
          />
          <div className={`w-2.5 h-2.5 rounded-full z-10 transition-all duration-700 ${isListening ? 'bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.8)]' : 'bg-[rgb(var(--foreground))]/30'}`} />
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
            className={`p-2 rounded-lg transition-all active:scale-90 ${pttBusy ? 'opacity-50 animate-pulse pointer-events-none' : ''} ${micError ? 'text-red-400' : isListening ? 'text-[rgb(var(--accent))]' : 'text-[rgb(var(--foreground))]/60 hover:text-[rgb(var(--foreground))]/90'}`}
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
