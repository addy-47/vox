import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ArrowRight, Check, MessageSquare, Volume2 } from 'lucide-react';
import { launchEngine, stopEngine } from '@/services/pipelineService';
import { onTranscriptPartial, onTranscriptFinal, onTelemetry } from '@/services/eventsService';
import { completeSetupWizard } from '@/services/setupService';
import { VoxOrb } from '@/shared/components/home';
import { MOBILE_ONBOARDING_COPY, WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

interface MobileLiveGreetingStepProps {
  onBack: () => void;
  onFinish?: () => void;
}

export const MobileLiveGreetingStep: React.FC<MobileLiveGreetingStepProps> = ({ onBack, onFinish }) => {
  const [transcript, setTranscript] = useState('');
  const [isEngineReady, setIsEngineReady] = useState(false);
  const [testComplete, setTestComplete] = useState(false);
  const [isFinishing, setIsFinishing] = useState(false);
  const [energy, setEnergy] = useState(0);

  const copy = MOBILE_ONBOARDING_COPY.liveGreeting;
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    let isMounted = true;

    launchEngine()
      .then(() => {
        if (isMounted) setIsEngineReady(true);
      })
      .catch((e) => console.error('Engine launch error', e));

    const unlistenPartial = onTranscriptPartial((payload) => {
      setTranscript(payload.text);
      if (payload.text.length > 2) {
        if (timeoutRef.current) clearTimeout(timeoutRef.current);
        timeoutRef.current = setTimeout(() => {
          setTestComplete(true);
        }, 1200);
      }
    });

    const unlistenFinal = onTranscriptFinal((payload) => {
      setTranscript(payload.text);
      if (payload.text.length > 2) {
        setTestComplete(true);
      }
    });

    const unlistenEnergy = onTelemetry((payload) => {
      setEnergy((payload.energy ?? 0) * 100);
    });

    return () => {
      isMounted = false;
      unlistenPartial();
      unlistenFinal();
      unlistenEnergy();
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
      stopEngine().catch(console.error);
    };
  }, []);

  const handleFinish = async () => {
    setIsFinishing(true);
    try {
      await completeSetupWizard();
      if (onFinish) {
        onFinish();
      } else {
        window.location.reload();
      }
    } catch {
      if (onFinish) {
        onFinish();
      } else {
        window.location.reload();
      }
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden select-none">
      {/* Top Header */}
      <div className="px-5 pt-3 pb-2 shrink-0 flex items-center justify-between border-b border-[rgba(var(--foreground),0.04)]">
        <button
          onClick={onBack}
          disabled={isFinishing}
          className="text-xs font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] uppercase tracking-wider py-1 px-1 focus:outline-none disabled:opacity-30"
        >
          {WIZARD_CTA_LABELS.back}
        </button>
        <span className="text-[11px] font-mono font-semibold tracking-wider text-[rgb(var(--accent))] uppercase">
          {copy.step}
        </span>
        <div className="w-8" />
      </div>

      {/* Scrollable Center Body */}
      <div className="flex-1 min-h-0 overflow-y-auto px-5 py-3 flex flex-col items-center justify-center text-center space-y-4">
        {/* Reactive VoxOrb */}
        <div className="relative w-40 h-40 sm:w-48 sm:h-48 flex items-center justify-center shrink-0">
          <div className="w-full h-full">
            <VoxOrb
              interactionState={
                testComplete
                  ? 'Speaking'
                  : energy > 5
                  ? 'Listening'
                  : isEngineReady
                  ? 'Ready'
                  : 'Thinking'
              }
            />
          </div>
          <div className="absolute inset-0 bg-[rgb(var(--accent))]/15 blur-[60px] rounded-full pointer-events-none" />
        </div>

        {/* Dynamic Titles */}
        <div className="space-y-1 max-w-xs">
          <h2 className="text-xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {testComplete
              ? copy.successKicker
              : energy > 5
              ? copy.listeningLabel
              : copy.title}
          </h2>
          <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
            {testComplete ? copy.successBody : copy.hint}
          </p>
        </div>

        {/* Live Floating Transcript Surface */}
        <div className="w-full max-w-xs min-h-[50px] glass rounded-xl p-3 flex items-center justify-center text-center border border-[rgba(var(--foreground),0.06)] relative overflow-hidden">
          <AnimatePresence mode="wait">
            {transcript ? (
              <motion.div
                key="text"
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                className="flex items-center gap-2 text-xs font-medium text-[rgb(var(--foreground))]"
              >
                <MessageSquare className="w-3.5 h-3.5 text-[rgb(var(--accent))] shrink-0" />
                <span className="truncate">{transcript}</span>
              </motion.div>
            ) : (
              <motion.div
                key="placeholder"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60 italic flex items-center gap-1.5"
              >
                <Volume2 className="w-3.5 h-3.5" />
                {isEngineReady ? copy.readyPrompt : WIZARD_CTA_LABELS.processing}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>

      {/* Sticky Bottom Actions */}
      <div className="px-5 py-4 shrink-0 border-t border-[rgba(var(--foreground),0.06)] bg-[rgb(var(--background))]/95 backdrop-blur-md">
        <button
          onClick={handleFinish}
          disabled={isFinishing}
          className="w-full py-3.5 min-h-[48px] bg-[rgb(var(--accent))] text-black font-display font-bold text-xs uppercase tracking-wider rounded-xl flex items-center justify-center gap-2 shadow-[0_4px_20px_rgba(var(--accent),0.3)] active:scale-[0.98] transition-transform disabled:opacity-60"
        >
          {testComplete ? (
            <>
              {copy.finishButton} <Check className="w-4 h-4" />
            </>
          ) : (
            <>
              {copy.finishButton} <ArrowRight className="w-4 h-4" />
            </>
          )}
        </button>
      </div>
    </div>
  );
};
