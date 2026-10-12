import React, { useEffect, useState } from 'react';
import { launchEngine, stopEngine } from '@/services/pipelineService';
import { onTelemetry } from '@/services/eventsService';
import { motion } from 'framer-motion';
import { AudioWaveform, Activity, CheckCircle2, AlertCircle } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';
import { WIZARD_STEP_HEADERS, AUDIO_SETUP_COPY, WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

interface Props {
  onNext: () => void;
  onBack: () => void;
}

export const AudioSetupStep: React.FC<Props> = ({ onNext, onBack }) => {
  const [energy, setEnergy] = useState(0);
  const [heardSignal, setHeardSignal] = useState(false);
  const [audioError, setAudioError] = useState(false);
  const [initAttempt, setInitAttempt] = useState(0);

  useEffect(() => {
    let isMounted = true;

    const init = async () => {
      try {
        await launchEngine();
        if (isMounted) setAudioError(false);
      } catch (e) {
        console.error('Audio initialization failed', e);
        if (isMounted) setAudioError(true);
      }
    };
    init();

    let lastTime = 0;
    let localEnergy = 0;
    const THROTTLE_MS = 60;

    const unlisten = onTelemetry((payload) => {
      const val = payload.energy ?? 0;
      const targetEnergy = val * 100;

      // Exponential moving average smoothing for organic signal rise and slow decay
      localEnergy = localEnergy * 0.85 + targetEnergy * 0.15;

      const now = Date.now();
      if (now - lastTime >= THROTTLE_MS) {
        setEnergy(localEnergy);
        if (localEnergy > 10) {
          setHeardSignal(true);
        }
        lastTime = now;
      }
    });

    return () => {
      isMounted = false;
      unlisten();
      stopEngine().catch(console.error);
    };
  }, [initAttempt]);

  return (
    <div className="flex flex-col h-full relative">
      <WizardHeader
        step={WIZARD_STEP_HEADERS.audio.step}
        title={WIZARD_STEP_HEADERS.audio.title}
        description={WIZARD_STEP_HEADERS.audio.description}
      />

      <div className="flex-1 flex flex-col gap-6 min-h-0 justify-center max-w-xl mx-auto w-full">
        {/* Live Analysis Card */}
        <div className="p-5 sm:p-6 glass rounded-2xl relative overflow-hidden">
          <div className="absolute inset-0 bg-gradient-to-br from-[rgb(var(--accent))]/5 to-transparent opacity-50 pointer-events-none" />

          <div className="relative z-10 flex items-center gap-4 sm:gap-6">
            <div className="relative shrink-0">
              {energy > 5 && (
                <div
                  className="absolute inset-0 bg-[rgb(var(--accent))]/20 rounded-full blur-xl animate-pulse"
                  style={{ animationDuration: '1.5s' }}
                />
              )}
              <div
                className={cn(
                  'w-14 h-14 sm:w-16 sm:h-16 rounded-2xl flex items-center justify-center transition-all duration-300 relative z-10 border',
                  energy > 5
                    ? 'bg-[rgb(var(--accent))]/15 border-[rgb(var(--accent))]/30 text-[rgb(var(--accent))] shadow-[0_0_24px_rgba(var(--accent),0.2)]'
                    : 'bg-[rgba(var(--foreground),0.05)] border-transparent text-[rgb(var(--foreground-muted))]/50'
                )}
              >
                <AudioWaveform className="w-7 h-7 sm:w-8 sm:h-8" />
              </div>
            </div>

            <div className="flex-1 min-w-0">
              <div className="flex items-center justify-between mb-2.5">
                <span className="text-[11px] sm:text-[12px] font-bold text-[rgb(var(--foreground))]/80 uppercase tracking-wider flex items-center gap-2">
                  <Activity className="w-3.5 h-3.5 text-[rgb(var(--accent))]" /> {AUDIO_SETUP_COPY.liveLabel}
                </span>
                <span className="text-[13px] font-bold text-[rgb(var(--accent))] font-mono">
                  {Math.round(energy)}%
                </span>
              </div>
              <div className="h-2 bg-[rgba(var(--foreground),0.06)] rounded-full overflow-hidden border border-[rgba(var(--foreground),0.08)]">
                <motion.div
                  className="h-full rounded-full"
                  style={{
                    background:
                      'linear-gradient(90deg, rgb(var(--accent)) 0%, rgba(var(--accent), 0.4) 100%)',
                  }}
                  animate={{ width: `${Math.min(energy, 100)}%` }}
                  transition={{ type: 'tween', ease: 'easeOut', duration: 0.15 }}
                />
              </div>
            </div>
          </div>
        </div>

        {/* Status card */}
        {audioError ? (
          <div className="p-4 bg-[rgba(var(--warning),0.1)] border border-[rgba(var(--warning),0.25)] rounded-xl flex items-center justify-between gap-3">
            <div className="flex items-center gap-2.5 min-w-0">
              <AlertCircle className="w-4 h-4 text-[rgb(var(--warning))] shrink-0" />
              <span className="text-[rgb(var(--warning))] text-xs sm:text-sm font-semibold">
                {AUDIO_SETUP_COPY.initFailed}
              </span>
            </div>
            <button
              type="button"
              onClick={() => {
                setAudioError(false);
                setInitAttempt((n) => n + 1);
              }}
              className="px-3.5 py-1.5 min-h-[36px] bg-[rgba(var(--warning),0.2)] hover:bg-[rgba(var(--warning),0.3)] text-[rgb(var(--warning))] rounded-lg text-[11px] font-bold uppercase tracking-wider transition-colors shrink-0 cursor-pointer"
            >
              {AUDIO_SETUP_COPY.retryInit}
            </button>
          </div>
        ) : (
          <div className="p-4 rounded-xl glass border border-[rgba(var(--border),0.15)] flex items-center gap-3">
            {heardSignal ? (
              <CheckCircle2 className="w-5 h-5 text-[rgb(var(--accent))] shrink-0" />
            ) : (
              <Activity className="w-5 h-5 text-[rgb(var(--foreground-muted))]/50 shrink-0" />
            )}
            <p className="text-xs sm:text-sm font-medium text-[rgb(var(--foreground-muted))]">
              {heardSignal ? AUDIO_SETUP_COPY.statusHeard : AUDIO_SETUP_COPY.statusReady}
            </p>
          </div>
        )}
      </div>

      <WizardFooter
        onBack={onBack}
        onNext={onNext}
        nextLabel={WIZARD_CTA_LABELS.continueToVerification}
        isNextDisabled={audioError}
        showBack={true}
      />
    </div>
  );
};
