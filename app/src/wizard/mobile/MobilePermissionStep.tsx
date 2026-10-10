import React, { useState, useEffect } from 'react';
import { Mic, ShieldCheck, Check, ArrowRight } from 'lucide-react';
import { getRuntimeReport } from '@/services/setupService';
import { MOBILE_ONBOARDING_COPY, WIZARD_CTA_LABELS } from '@/data/welcomeCopy';
import { MobileStepFooter } from './MobileStepFooter';

interface MobilePermissionStepProps {
  onNext: () => void;
  onBack: () => void;
  onSkip?: () => void;
  onSelectStep?: (index: number) => void;
}

export const MobilePermissionStep: React.FC<MobilePermissionStepProps> = ({
  onNext,
  onBack,
  onSelectStep,
}) => {
  const [micGranted, setMicGranted] = useState<boolean | null>(null);
  const [isChecking, setIsChecking] = useState(false);

  const copy = MOBILE_ONBOARDING_COPY.permission;

  useEffect(() => {
    getRuntimeReport()
      .then((rep) => setMicGranted(rep.mic_access))
      .catch(() => setMicGranted(false));
  }, []);

  const handleRequestMic = async () => {
    setIsChecking(true);
    try {
      if (navigator.mediaDevices && navigator.mediaDevices.getUserMedia) {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        stream.getTracks().forEach((track) => track.stop());
      }
      const report = await getRuntimeReport();
      setMicGranted(report.mic_access);
      setTimeout(() => onNext(), 400);
    } catch {
      const report = await getRuntimeReport().catch(() => null);
      if (report && report.mic_access) {
        setMicGranted(true);
        setTimeout(() => onNext(), 400);
      } else {
        setMicGranted(false);
      }
    } finally {
      setIsChecking(false);
    }
  };

  return (
    <div className="flex flex-col h-full w-full overflow-hidden select-none bg-transparent">
      {/* Scrollable Center Body */}
      <div className="flex-1 min-h-0 overflow-y-auto px-6 py-4 flex flex-col items-center justify-center">
        {/* Glowing Mic Icon Stage */}
        <div className="w-full flex flex-col items-center justify-center">
          <div className="relative my-4 flex items-center justify-center shrink-0">
            <div className="w-24 h-24 rounded-3xl bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/30 flex items-center justify-center relative z-10 shadow-[0_0_40px_rgba(var(--accent),0.2)]">
              {micGranted ? (
                <Check className="w-10 h-10 text-[rgb(var(--accent))]" />
              ) : (
                <Mic className="w-10 h-10 text-[rgb(var(--accent))]" />
              )}
            </div>
            <div className="absolute inset-0 bg-[rgb(var(--accent))]/20 blur-2xl rounded-full" />
          </div>

          <div className="space-y-2 max-w-sm text-center pt-2">
            <h2 className="text-2xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
              {micGranted ? copy.grantedTitle : copy.title}
            </h2>
            <p className="text-sm text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
              {micGranted ? copy.grantedBody : copy.description}
            </p>
          </div>
        </div>

        {/* Privacy Card (Clean rectangular card, compliant with design guide §5) */}
        <div className="w-full max-w-sm glass rounded-2xl p-4 flex items-start gap-3.5 text-left border border-[rgba(var(--foreground),0.08)] my-5">
          <ShieldCheck className="w-5 h-5 text-[rgb(var(--accent))] shrink-0 mt-0.5" />
          <div className="space-y-1">
            <span className="text-[11px] font-mono font-semibold text-[rgb(var(--foreground))] uppercase tracking-wider block">
              {copy.privacyKicker}
            </span>
            <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
              {copy.privacyBody}
            </p>
          </div>
        </div>
      </div>

      {/* Thumb-Anchored Bottom Action Surface with 5-Step Slider & Buttons */}
      <MobileStepFooter
        currentIndex={2}
        totalSteps={5}
        onNext={micGranted ? onNext : handleRequestMic}
        onBack={onBack}
        nextLabel={
          micGranted
            ? copy.continueButton
            : isChecking
            ? WIZARD_CTA_LABELS.processing
            : copy.allowButton
        }
        nextIcon={
          micGranted ? (
            <Check className="w-4 h-4 stroke-[3]" />
          ) : (
            <ArrowRight className="w-4 h-4" />
          )
        }
        isLoading={isChecking}
        onSelectStep={onSelectStep}
      />
    </div>
  );
};
