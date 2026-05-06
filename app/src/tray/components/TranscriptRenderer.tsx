import React, { useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Activity } from 'lucide-react';
import { Waveform } from './Waveform';

interface TranscriptRendererProps {
  displayText: string;
  isListening: boolean;
  pttStatus?: 'IDLE' | 'RECORDING' | 'PROCESSING';
  amplitudeBuffer?: number[];
}

export const TranscriptRenderer: React.FC<TranscriptRendererProps> = ({ 
  displayText, 
  isListening,
  pttStatus = 'IDLE',
  amplitudeBuffer = []
}) => {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [displayText]);

  return (
    <div 
      ref={scrollRef} 
      className="flex-1 overflow-y-auto px-5 py-4 custom-scrollbar relative z-10"
    >
      <AnimatePresence mode="wait">
        {pttStatus === 'RECORDING' ? (
          <motion.div 
            key="waveform"
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.95 }}
            className="h-full flex flex-col items-center justify-center w-full px-4"
          >
            <div className="w-full flex flex-col items-center gap-4">
               <Waveform 
                data={amplitudeBuffer} 
                height={60} 
                barWidth={3} 
                barGap={2}
                className="w-full"
              />
              <span className="text-[9px] font-black uppercase tracking-[0.4em] text-cyan-400/60 animate-pulse">
                Recording
              </span>
            </div>
          </motion.div>
        ) : displayText ? (
          <motion.div 
            key="text"
            initial={{ opacity: 0, y: 5 }}
            animate={{ opacity: 1, y: 0 }}
            className="space-y-2"
          >
            <div className="text-[17px] leading-snug font-medium tracking-tight text-[rgb(var(--foreground))]/90 drop-shadow-sm">
              {displayText}
              {(isListening || pttStatus === 'PROCESSING') && (
                <motion.span 
                  animate={{ opacity: [0, 1, 0] }}
                  transition={{ repeat: Infinity, duration: 0.8 }}
                  className={`inline-block w-[2px] h-[1em] ml-1 align-middle shadow-[0_0_8px_rgba(var(--accent),0.8)] ${pttStatus === 'PROCESSING' ? 'bg-amber-400' : 'bg-[rgb(var(--accent))]'}`}
                />
              )}
            </div>
          </motion.div>
        ) : (
          <motion.div 
            key="empty"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className="h-full flex flex-col items-center justify-center opacity-40"
          >
            {pttStatus === 'PROCESSING' ? (
              <div className="flex flex-col items-center gap-3">
                <div className="w-8 h-8 border-2 border-[rgb(var(--accent))]/30 border-t-[rgb(var(--accent))] rounded-full animate-spin" />
                <span className="text-[10px] text-[rgb(var(--accent))]/60 font-bold uppercase tracking-[0.2em]">Processing</span>
              </div>
            ) : (
              <>
                <Activity size={24} className="mb-2 text-[rgb(var(--accent))]/40 animate-pulse" />
                <p className="text-[9px] font-black uppercase tracking-[0.4em] text-[rgb(var(--foreground))]/40">
                  Standby
                </p>
              </>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
