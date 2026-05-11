import React, { useRef, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Activity } from 'lucide-react';
import { LiveWaveform } from '../../shared/components/LiveWaveform';
import { cn } from '../../shared/lib/utils';

interface TranscriptRendererProps {
  displayText: string;
  interactionState: string;
  pttStatus?: 'IDLE' | 'RECORDING' | 'PROCESSING';
  telemetryRef: React.MutableRefObject<{ energy: number; vad_prob: number }>;
}


export const TranscriptRenderer: React.FC<TranscriptRendererProps> = React.memo(({ 
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
      <div className="min-h-full flex flex-col items-center justify-center relative">
        {/* Waveform Layer (Recording or Processing) */}
        {(pttStatus !== 'IDLE' || interactionState === "Thinking") && !displayText && (
          <motion.div 
            key="waveform-layer"
            initial={{ opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.98 }}
            className="w-full flex flex-col items-center gap-6 px-4"
          >
            <LiveWaveform
              active={pttStatus === 'RECORDING'}
              processing={pttStatus === 'PROCESSING' || interactionState === "Thinking"}
              mode="scrolling"
              telemetryRef={telemetryRef}
              updateRate={50}
              historySize={80}
              barWidth={2}
              barGap={2}
              height={64}
              fadeEdges={true}
              fadeWidth={40}
              className="w-full"
            />
                <span className={cn(
              "text-[11px] font-black uppercase tracking-[0.4em] transition-colors duration-500",
              pttStatus === 'RECORDING' ? "text-[rgb(var(--accent))]/80 animate-pulse" : "text-[rgb(var(--accent))]/80"
            )}>
              {pttStatus === 'RECORDING' ? "Recording" : "Processing"}
            </span>
          </motion.div>
        )}

        {/* Text Layer */}
        {displayText && (
          <motion.div 
            key="text-layer"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            className="w-full space-y-2"
          >
            <div className="text-[17px] leading-snug font-medium tracking-tight text-[rgb(var(--foreground))]/90 drop-shadow-sm">
              {displayText}
              {(interactionState === "Listening" || interactionState === "UserSpeaking" || pttStatus === 'PROCESSING') && (
                <motion.span 
                  animate={{ opacity: [0, 1, 0] }}
                  transition={{ repeat: Infinity, duration: 0.8 }}
                  className={`inline-block w-[2px] h-[1em] ml-1 align-middle shadow-[0_0_8px_rgba(var(--accent),0.8)] bg-[rgb(var(--accent))]`}
                />
              )}
            </div>
          </motion.div>
        )}

        {/* Idle Layer */}
        {!displayText && pttStatus === 'IDLE' && interactionState !== "Thinking" && (
          <motion.div 
            key="idle-layer"
            initial={{ opacity: 0 }}
            animate={{ opacity: 0.4 }}
            className="flex flex-col items-center justify-center"
          >
            <Activity size={24} className="mb-2 text-[rgb(var(--accent))]/40 animate-pulse" />
            <p className="text-[11px] font-black uppercase tracking-[0.4em] text-[rgb(var(--foreground))]/40">
              Standby
            </p>
          </motion.div>
        )}
      </div>
    </div>
  );
});

