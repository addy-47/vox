import React, { useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Activity } from 'lucide-react';
import { LiveWaveform } from '../../shared/ui/LiveWaveform';

interface TranscriptRendererProps {
  displayText: string;
  interactionState: string;
  pttStatus?: 'IDLE' | 'RECORDING' | 'PROCESSING';
  telemetryRef: React.MutableRefObject<{ energy: number; vad_prob: number }>;
}


export const TranscriptRenderer: React.FC<TranscriptRendererProps> = ({ 
  displayText, 
  interactionState,
  pttStatus = 'IDLE',
  telemetryRef
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
            className="h-full flex flex-col items-center justify-center w-full px-4 overflow-hidden"
          >
            <div className="w-full flex flex-col items-center gap-6">
              <LiveWaveform
                active={true}
                mode="scrolling"
                telemetryRef={telemetryRef}
                updateRate={30} // 30fps for smooth scrolling
                historySize={60}
                barWidth={3}
                barGap={2}
                barRadius={2}
                height={64}
                fadeEdges={true}
                fadeWidth={40}
                className="w-full"
              />
              <span className="text-[10px] font-black uppercase tracking-[0.4em] text-cyan-400/80 animate-pulse">
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
              {(interactionState === "Listening" || interactionState === "UserSpeaking" || pttStatus === 'PROCESSING') && (
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
            className="h-full flex flex-col items-center justify-center opacity-40 overflow-hidden"
          >
            {pttStatus === 'PROCESSING' || interactionState === "Thinking" ? (
              <div className="w-full flex flex-col items-center gap-6 px-4">
                <LiveWaveform
                  active={false}
                  processing={true}
                  mode="scrolling"
                  height={64}
                  barWidth={3}
                  barGap={2}
                  fadeEdges={true}
                  fadeWidth={40}
                  className="w-full"
                />
                <span className="text-[10px] text-[rgb(var(--accent))]/80 font-bold uppercase tracking-[0.3em]">Processing</span>
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
