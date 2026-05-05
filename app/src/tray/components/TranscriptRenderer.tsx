import React, { useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Activity } from 'lucide-react';

interface TranscriptRendererProps {
  displayText: string;
  isListening: boolean;
}

export const TranscriptRenderer: React.FC<TranscriptRendererProps> = ({ displayText, isListening }) => {
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
        {displayText ? (
          <motion.div 
            key="text"
            initial={{ opacity: 0, y: 5 }}
            animate={{ opacity: 1, y: 0 }}
            className="space-y-2"
          >
            <div className="text-[17px] leading-snug font-medium tracking-tight text-white/90 drop-shadow-sm">
              {displayText}
              {isListening && (
                <motion.span 
                  animate={{ opacity: [0, 1, 0] }}
                  transition={{ repeat: Infinity, duration: 0.8 }}
                  className="inline-block w-[2px] h-[1em] ml-1 bg-cyan-400 align-middle shadow-[0_0_8px_rgba(0,219,233,0.8)]"
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
            <Activity size={24} className="mb-2 text-cyan-400/40 animate-pulse" />
            <p className="text-[9px] font-black uppercase tracking-[0.4em] text-white/20">
              Standby
            </p>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
