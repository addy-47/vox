import React, { useEffect, useState, useRef } from 'react';
import { launchEngine, stopEngine } from '@/services/pipelineService';
import { onTranscriptPartial, onTranscriptFinal, onTelemetry } from '@/services/eventsService';
import { motion, AnimatePresence } from 'framer-motion';
import { Check, Activity, X, MessageSquare, Hourglass } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';
import { WIZARD_STEP_HEADERS, LIVE_TEST_COPY } from '@/data/welcomeCopy';

interface Props {
  onNext: () => void;
  onBack: () => void;
}

/**
 * Isolated, memoized waveform strip: re-renders on energy ticks without
 * touching the transcript or status cards, and drives the bars with
 * `scaleY` (compositor-only) instead of layout-triggering `height`.
 */
const WaveformBars = React.memo(({ energy }: { energy: number }) => (
  <div className="flex items-center gap-1.5 h-8">
    {/* Array of sleek vertical wave bars responding to energy */}
    {Array.from({ length: 15 }).map((_, i) => {
      const centerDist = Math.abs(i - 7);
      const multiplier = Math.max(0.15, 1 - centerDist * 0.12);
      const heightPercent = energy > 2 ? Math.min(100, Math.max(12, energy * 3.5 * multiplier)) : 12;

      return (
        <div
          key={i}
          className={cn(
            "w-1 h-full rounded-full origin-center transition-colors duration-300 transform-gpu",
            energy > 2 ? "bg-[rgb(var(--accent))] shadow-[0_0_10px_rgba(var(--accent),0.5)]" : "bg-[rgba(var(--foreground),0.1)]"
          )}
          style={{ transform: `scaleY(${heightPercent / 100})` }}
        />
      );
    })}
  </div>
));
WaveformBars.displayName = "WaveformBars";

export const LiveTestStep: React.FC<Props> = ({ onNext, onBack }) => {
  const [transcript, setTranscript] = useState('');
  const [isEngineReady, setIsEngineReady] = useState(false);
  const [testComplete, setTestComplete] = useState(false);
  const [energy, setEnergy] = useState(0);
  const [error, setError] = useState<string | null>(null);
  
  const transcriptTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  const setup = async () => {
    setError(null);
    setIsEngineReady(false);
    try {
      await launchEngine();
      setIsEngineReady(true);
    } catch (e) {
      console.error('Engine launch failed', e);
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  useEffect(() => {
    setup();

    const unlistenPartial = onTranscriptPartial((payload) => {
      setTranscript(payload.text);
      
      if (transcriptTimeoutRef.current) clearTimeout(transcriptTimeoutRef.current);
      transcriptTimeoutRef.current = setTimeout(() => {
        if (payload.text.length > 2) {
          setTestComplete(true);
        }
      }, 2000);
    });

    const unlistenFinal = onTranscriptFinal((payload) => {
      setTranscript(payload.text);
      if (payload.text.length > 2) {
        setTestComplete(true);
      }
    });

    let lastTime = 0;
    let localEnergy = 0;
    const THROTTLE_MS = 40; // High-refresh responsive updates

    const unlistenEnergy = onTelemetry((payload) => {
      const e = payload.energy ?? 0;
      
      const targetEnergy = e * 100;
      localEnergy = localEnergy * 0.75 + targetEnergy * 0.25;
      
      const now = Date.now();
      if (now - lastTime >= THROTTLE_MS) {
        setEnergy(localEnergy);
        lastTime = now;
      }
    });

    return () => {
      unlistenPartial();
      unlistenFinal();
      unlistenEnergy();
      if (transcriptTimeoutRef.current) clearTimeout(transcriptTimeoutRef.current);
      stopEngine().catch(console.error);
    };
  }, []);

  return (
    <div className="flex flex-col h-full max-h-[100vh] overflow-y-auto lg:overflow-hidden justify-between relative select-none">
      <WizardHeader
        step={WIZARD_STEP_HEADERS.testing.step}
        title={WIZARD_STEP_HEADERS.testing.title}
        description={WIZARD_STEP_HEADERS.testing.description}
      />

      <div className="flex-1 flex flex-col gap-4 min-h-0 overflow-visible lg:overflow-hidden justify-center">
        {/* Reactive Flat Waveform Visualization Strip */}
        <div className="glass rounded-xl p-4 sm:p-6 flex flex-col items-center justify-center relative overflow-hidden h-24 sm:h-28 shrink-0">
          <div className="absolute inset-0 bg-gradient-to-b from-[rgb(var(--accent))]/5 to-transparent opacity-20 pointer-events-none" />
          
          {error ? (
              <div className="flex items-center gap-4 relative z-10 text-left">
                  <div className="w-10 h-10 rounded-xl bg-[rgba(var(--danger),0.1)] border border-[rgba(var(--danger),0.2)] flex items-center justify-center text-[rgb(var(--danger))] shrink-0">
                      <X className="w-5 h-5" />
                  </div>
                  <div className="flex flex-col">
                      <h3 className="text-[rgb(var(--foreground))] font-bold uppercase tracking-wider text-[12px]">{LIVE_TEST_COPY.engineErrorTitle}</h3>
                      <button 
                          onClick={setup}
                          className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] hover:underline text-left mt-0.5"
                      >
                          {LIVE_TEST_COPY.tryAgain}
                      </button>
                  </div>
              </div>
          ) : (
              <div className="flex flex-col items-center justify-center w-full h-full relative z-10">
                   <WaveformBars energy={energy} />
                  
                   <span className="text-[12px] font-black text-[rgb(var(--foreground-muted))]/50 uppercase tracking-[0.3em] mt-4">
                     {isEngineReady ? (energy > 2 ? LIVE_TEST_COPY.voiceDetected : LIVE_TEST_COPY.waitingForVoice) : LIVE_TEST_COPY.engineStarting}
                   </span>
              </div>
          )}
        </div>

        {/* Live Transcript Display Box */}
        <div className={cn(
            "relative z-10 glass rounded-xl p-4 sm:p-5 flex flex-col justify-center flex-1 min-h-[90px] max-h-[140px] transition-colors",
            testComplete ? "border-[rgba(var(--accent),0.3)] shadow-[0_0_30px_rgba(var(--accent),0.05)]" : ""
        )}>
            <div className="flex items-center justify-between mb-2">
                <span className="text-[11px] sm:text-[12px] font-bold text-[rgb(var(--foreground-muted))]/60 uppercase tracking-wider flex items-center gap-2">
                    <MessageSquare className="w-3.5 h-3.5 text-[rgb(var(--accent))]/70" /> {LIVE_TEST_COPY.demoHint}
                </span>
                {testComplete && (
                    <motion.span 
                        initial={{ opacity: 0, scale: 0.8 }}
                        animate={{ opacity: 1, scale: 1 }}
                        className="text-[11px] font-mono font-bold text-[rgb(var(--accent))] flex items-center gap-1 uppercase tracking-wider"
                    >
                        <Check className="w-3.5 h-3.5" />
                        {LIVE_TEST_COPY.processed}
                    </motion.span>
                )}
            </div>
            
            <div className="flex-1 flex items-center min-h-0">
                <AnimatePresence mode="wait">
                    {transcript ? (
                        <motion.p 
                            key="text"
                            initial={{ opacity: 0, y: 3 }}
                            animate={{ opacity: 1, y: 0 }}
                            className="text-base font-bold text-[rgb(var(--foreground))] tracking-tight leading-snug overflow-y-auto max-h-full custom-scrollbar pr-1"
                        >
                            {transcript}
                            {!testComplete && <motion.span animate={{ opacity: [1, 0.4, 1] }} transition={{ repeat: Infinity, duration: 1 }} className="inline-block w-1.5 h-3.5 bg-[rgb(var(--accent))] ml-1.5 align-middle" />}
                        </motion.p>
                    ) : (
                         <motion.p 
                            key="placeholder"
                            initial={{ opacity: 0 }}
                            animate={{ opacity: 1 }}
                            className="text-[rgb(var(--foreground-muted))]/60 italic font-medium text-sm"
                        >
                            {isEngineReady ? LIVE_TEST_COPY.speakNow : LIVE_TEST_COPY.startingModels}
                        </motion.p>
                    )}
                </AnimatePresence>
            </div>
        </div>

        {/* Diagnostics & Verification Cards */}
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 shrink-0">
            <div className="p-3 glass rounded-xl flex items-center gap-3">
                <div className={cn(
                    "w-8 h-8 rounded-lg flex items-center justify-center transition-colors shrink-0",
                    energy > 2 ? "bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))]" : "bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/60"
                )}>
                    <Activity className="w-4 h-4" />
                </div>
                <div className="flex flex-col min-w-0">
                    <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))]/70 uppercase tracking-wider truncate">{LIVE_TEST_COPY.voiceLevel}</span>
                    <span className="text-xs font-semibold text-[rgb(var(--foreground))] truncate">
                        {isEngineReady ? (energy > 2 ? LIVE_TEST_COPY.voiceDetected : LIVE_TEST_COPY.listening) : "---"}
                    </span>
                </div>
            </div>
            <div className="p-3 glass rounded-xl flex items-center gap-3">
                <div className={cn(
                    "w-8 h-8 rounded-lg flex items-center justify-center transition-colors shrink-0",
                    testComplete ? "bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))]" : "bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/60"
                )}>
                    {testComplete ? <Check className="w-4 h-4" /> : <Hourglass className="w-4 h-4 animate-pulse" />}
                </div>
                <div className="flex flex-col min-w-0">
                    <span className="text-[11px] font-bold text-[rgb(var(--foreground-muted))]/70 uppercase tracking-wider truncate">{LIVE_TEST_COPY.demoHint}</span>
                    <span className="text-xs font-semibold text-[rgb(var(--foreground))] truncate">
                        {testComplete ? LIVE_TEST_COPY.textReceived : LIVE_TEST_COPY.waiting}
                    </span>
                </div>
            </div>
        </div>
      </div>

      <WizardFooter 
        onBack={onBack}
        onNext={onNext}
        onSkip={onNext}
        nextLabel={LIVE_TEST_COPY.confirmContinue}
        isNextDisabled={!testComplete}
        showBack={true}
        showSkip={true}
        className="mt-4 shrink-0"
      />
    </div>
  );
};
