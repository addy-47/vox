import React from 'react';
import { motion } from 'framer-motion';
import { X, Copy, Check, Mic } from 'lucide-react';

interface HeaderProps {
  isListening: boolean;
  hasContent: boolean;
  copied: boolean;
  isPttActive: boolean;
  onCopy: () => void;
  onClose: () => void;
  onTogglePtt: () => void;
}

export const Header: React.FC<HeaderProps> = ({ 
  isListening, hasContent, copied, isPttActive,
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
            className="absolute w-5 h-5 rounded-full bg-cyan-400 blur-md"
          />
          <div className={`w-2.5 h-2.5 rounded-full z-10 transition-all duration-700 ${isListening ? 'bg-cyan-400 shadow-[0_0_10px_rgba(0,219,233,0.8)]' : 'bg-white/10'}`} />
        </div>
        <span className="text-[11px] font-black tracking-[0.4em] text-white/30 uppercase">
          Vox <span className="text-cyan-400/80">Live</span>
        </span>
      </div>
      
      <div className="flex items-center gap-2">
        <button 
          onClick={(e) => { e.stopPropagation(); onTogglePtt(); }}
          className={`p-2 rounded-lg transition-all active:scale-90 ${isPttActive ? 'text-cyan-400 bg-cyan-400/10' : 'text-white/20 hover:bg-white/5 hover:text-white/60'}`}
        >
          <Mic size={16} />
        </button>

        {hasContent && (
          <button 
            onClick={(e) => { e.stopPropagation(); onCopy(); }}
            className="p-2 rounded-lg hover:bg-cyan-400/10 transition-all text-white/20 hover:text-cyan-400 active:scale-90"
          >
            {copied ? <Check size={16} /> : <Copy size={16} />}
          </button>
        )}

        <button 
          onClick={(e) => { e.stopPropagation(); onClose(); }}
          className="p-2 rounded-lg hover:bg-white/5 transition-all text-white/10 hover:text-white/60 active:scale-90"
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
};
