import React from 'react';
import { motion } from 'framer-motion';
import { X, Copy, Check, Mic } from 'lucide-react';

interface HeaderProps {
  isListening: boolean;
  hasContent: boolean;
  copied: boolean;
  isPttActive: boolean;
  interactionMode: string;
  onCopy: () => void;
  onClose: () => void;
  onTogglePtt: () => void;
}

export const Header: React.FC<HeaderProps> = React.memo(({ 
  isListening, hasContent, copied, isPttActive, interactionMode,
  onCopy, onClose, onTogglePtt
}) => {
  return (
    <div className="px-6 py-4 flex items-center justify-between relative z-10" data-tauri-drag-region>
      <div className="flex items-center gap-3">
        <div className="relative flex items-center justify-center">
          <motion.div 
            animate={{ 
              scale: isListening ? [1, 1.3, 1] : 1, 
              opacity: isListening ? [0.4, 0.1, 0.4] : 0.05 
            }}
            transition={{ repeat: Infinity, duration: 2 }}
            className="absolute w-5 h-5 rounded-full bg-[rgb(var(--accent))] blur-md"
          />
          <div className={`w-2.5 h-2.5 rounded-full z-10 transition-all duration-700 ${isListening ? 'bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.8)]' : 'bg-[rgb(var(--foreground))]/10'}`} />
        </div>
        <span className="text-[11px] font-black tracking-[0.4em] text-[rgb(var(--foreground))]/60 uppercase">
          Vox <span className="text-[rgb(var(--accent))]">Live</span>
        </span>
      </div>
      
      <div className="flex items-center gap-2">
        {interactionMode?.toUpperCase() === 'PTT' && (
          <button 
            onClick={(e) => { e.stopPropagation(); onTogglePtt(); }}
            className={`p-2 rounded-lg transition-all active:scale-90 border border-transparent ${isPttActive ? 'text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/20' : 'text-[rgb(var(--foreground))]/40 hover:text-[rgb(var(--foreground))]/80 hover:bg-[rgb(var(--foreground))]/5 hover:border-[rgb(var(--foreground))]/10'}`}
          >
            <Mic size={16} />
          </button>
        )}

        {hasContent && (
          <button 
            onClick={(e) => { e.stopPropagation(); onCopy(); }}
            className="p-2 rounded-lg border border-transparent transition-all text-[rgb(var(--foreground))]/40 hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--foreground))]/5 hover:border-[rgb(var(--foreground))]/10 active:scale-90"
            aria-label="Copy to Clipboard"
          >
            {copied ? <Check size={16} /> : <Copy size={16} />}
          </button>
        )}

        <button 
          onClick={(e) => { e.stopPropagation(); onClose(); }}
          className="p-2 rounded-lg border border-transparent transition-all text-[rgb(var(--foreground))]/30 hover:text-[rgb(var(--foreground))]/80 hover:bg-[rgb(var(--foreground))]/5 hover:border-[rgb(var(--foreground))]/10 active:scale-90"
          aria-label="Close & Commit Session"
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
});
