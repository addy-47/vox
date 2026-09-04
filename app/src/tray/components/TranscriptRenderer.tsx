import React, { useRef, useEffect } from 'react';
import { motion } from 'framer-motion';
import { Activity } from 'lucide-react';
import { LiveWaveform } from '@/shared/components/common';
import { cn } from '@/shared/lib/utils';
import type { InteractionState } from '@/services/eventsService';

interface TranscriptRendererProps {
  displayText: string;
  interactionState: InteractionState;
  telemetryRef: React.MutableRefObject<{ energy: number; vad_prob: number }>;
}

export const TranscriptRenderer: React.FC<TranscriptRendererProps> = React.memo(({ 
  displayText, 
  interactionState,
  telemetryRef
}) => {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [displayText]);

  const isListening = interactionState === "Listening";
  const isThinking = interactionState === "Thinking";
  const showWaveform = (isListening || isThinking) && !displayText;

  return (
    <div 
      ref={scrollRef} 
      className="flex-1 overflow-y-auto px-5 py-2 custom-scrollbar relative z-10 mx-3"
    >
      <div className="min-h-full flex flex-col items-center justify-center relative">
        {/* Waveform Layer (Listening or Thinking) */}
        {showWaveform && (
          <motion.div 
            key="waveform-layer"
            initial={{ opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.98 }}
            className="w-full flex flex-col items-center gap-6 px-4"
          >
            <LiveWaveform
              active={isListening}
              processing={isThinking}
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
              "text-[12px] font-black uppercase tracking-[0.4em] transition-colors duration-500",
              isListening ? "text-[rgb(var(--accent))]/80 animate-pulse" : "text-[rgb(var(--accent))]/80"
            )}>
              {isListening ? "Listening" : "Processing"}
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
            <div className="text-[18px] leading-snug font-medium tracking-tight text-[rgb(var(--foreground))]/90 drop-shadow-sm whitespace-pre-wrap">
              {displayText}
              {(isListening || isThinking) && (
                <motion.span 
                  animate={{ opacity: [0, 1, 0] }}
                  transition={{ repeat: Infinity, duration: 0.8 }}
                  className={`inline-block w-[2px] h-[1em] ml-1 align-middle shadow-[0_0_8px_rgba(var(--accent),0.8)] bg-[rgb(var(--accent))]`}
                />
              )}
            </div>
          </motion.div>
        )}

        {/* Standby Layer */}
        {!displayText && !showWaveform && (
          <motion.div 
            key="idle-layer"
            initial={{ opacity: 0 }}
            animate={{ opacity: 0.7 }}
            className="flex flex-col items-center justify-center px-5 py-6"
          >
            <Activity size={24} className="mb-2 text-[rgb(var(--accent))]/50 animate-pulse " />
            <p className="text-[12px] font-black uppercase tracking-[0.4em] text-[rgb(var(--foreground))]/60 ">
              Standby
            </p>
          </motion.div>
        )}
      </div>
    </div>
  );
});

TranscriptRenderer.displayName = "TranscriptRenderer";
