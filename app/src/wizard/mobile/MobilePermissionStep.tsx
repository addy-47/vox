import React, { useState, useEffect } from 'react';
import { Mic, ShieldCheck, Check, ArrowRight } from 'lucide-react';
import { getRuntimeReport } from '@/services/setupService';
import { MOBILE_ONBOARDING_COPY, WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

interface MobilePermissionStepProps {
  onNext: () => void;
  onBack: () => void;
}

export const MobilePermissionStep: React.FC<MobilePermissionStepProps> = ({ onNext, onBack }) => {
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
      setTimeout(() => onNext(), 500);
    } catch {
      const report = await getRuntimeReport().catch(() => null);
      if (report && report.mic_access) {
        setMicGranted(true);
        setTimeout(() => onNext(), 500);
      } else {
        setMicGranted(false);
      }
    } finally {
      setIsChecking(false);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden select-none">
      {/* Top Header */}
      <div className="px-5 pt-3 pb-2 shrink-0 flex items-center justify-between border-b border-[rgba(var(--foreground),0.04)]">
        <button
          onClick={onBack}
          className="text-xs font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] uppercase tracking-wider py-1 px-1 focus:outline-none"
        >
          {WIZARD_CTA_LABELS.back}
        </button>
        <span className="text-[11px] font-mono font-semibold tracking-wider text-[rgb(var(--accent))] uppercase">
          {copy.step}
        </span>
        <button
          onClick={onNext}
          className="text-xs font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] uppercase tracking-wider py-1 px-1 focus:outline-none"
        >
          {WIZARD_CTA_LABELS.skip}
        </button>
      </div>

      {/* Scrollable Center Body */}
      <div className="flex-1 min-h-0 overflow-y-auto px-5 py-4 flex flex-col items-center justify-center text-center space-y-4">
        {/* Glowing Mic Icon Stage */}
        <div className="relative my-2 flex items-center justify-center shrink-0">
          <div className="w-20 h-20 rounded-full bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 flex items-center justify-center relative z-10 shadow-[0_0_30px_rgba(var(--accent),0.15)]">
            {micGranted ? (
              <Check className="w-9 h-9 text-[rgb(var(--accent))]" />
            ) : (
              <Mic className="w-9 h-9 text-[rgb(var(--accent))]" />
            )}
          </div>
          <div className="absolute inset-0 bg-[rgb(var(--accent))]/15 blur-xl rounded-full" />
        </div>

        <div className="space-y-1.5 max-w-xs">
          <h2 className="text-xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {copy.title}
          </h2>
          <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
            {copy.description}
          </p>
        </div>

        {/* Privacy Card (No faux-pill, crisp rectangular rounded-xl card) */}
        <div className="glass rounded-xl p-3 flex items-start gap-3 text-left max-w-xs border border-[rgba(var(--foreground),0.06)]">
          <ShieldCheck className="w-4 h-4 text-[rgb(var(--accent))] shrink-0 mt-0.5" />
          <div className="space-y-0.5">
            <span className="text-[11px] font-mono font-semibold text-[rgb(var(--foreground))] uppercase block">
              {copy.privacyKicker}
            </span>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-normal">
              {copy.privacyBody}
            </p>
          </div>
        </div>
      </div>

      {/* Sticky Bottom Actions */}
      <div className="px-5 py-4 shrink-0 border-t border-[rgba(var(--foreground),0.06)] bg-[rgb(var(--background))]/95 backdrop-blur-md space-y-2">
        <button
          onClick={micGranted ? onNext : handleRequestMic}
          disabled={isChecking}
          className="w-full py-3.5 min-h-[48px] bg-[rgb(var(--accent))] text-black font-display font-bold text-xs uppercase tracking-wider rounded-xl flex items-center justify-center gap-2 shadow-[0_4px_20px_rgba(var(--accent),0.3)] active:scale-[0.98] transition-transform disabled:opacity-60"
        >
          {micGranted ? (
            <>
              {copy.grantedTitle} <Check className="w-4 h-4" />
            </>
          ) : isChecking ? (
            WIZARD_CTA_LABELS.processing
          ) : (
            <>
              {copy.allowButton} <ArrowRight className="w-4 h-4" />
            </>
          )}
        </button>

        <button
          onClick={onNext}
          className="w-full py-2 text-[11px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] uppercase tracking-wider text-center focus:outline-none"
        >
          {copy.skipButton}
        </button>
      </div>
    </div>
  );
};
