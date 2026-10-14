import React from 'react';
import { Sparkles, ShieldCheck, AudioWaveform } from 'lucide-react';
import { VoxOrb } from '@/shared/components/home';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

interface MobileReadyStepProps {
  onNext?: () => void;
}

export const MobileReadyStep: React.FC<MobileReadyStepProps> = () => {
  const copy = MOBILE_ONBOARDING_COPY.ready;

  return (
    <div className="flex flex-col h-full w-full overflow-hidden select-none bg-transparent">
      {/* Centered Stage: Ambient Glowing Vox Orb & Ready Celebration */}
      <div className="flex-1 min-h-0 flex flex-col justify-between items-center px-6 py-4 text-center">
        {/* Glowing Active Vox Orb in Ready/Idle State */}
        <div className="flex-1 flex items-center justify-center">
          <div className="relative w-64 h-64 sm:w-72 sm:h-72 flex items-center justify-center shrink-0">
            <div className="w-full h-full relative z-10 scale-105">
              <VoxOrb interactionState="Idle" />
            </div>
            {/* Vibrant celebratory aura */}
            <div className="absolute inset-0 bg-[rgb(var(--success))]/20 blur-[100px] rounded-full pointer-events-none animate-pulse" />
            <div className="absolute inset-4 bg-[rgb(var(--accent))]/15 blur-[60px] rounded-full pointer-events-none" />
          </div>
        </div>

        {/* Headline, Status Badges & Usage Prompt (Centered) */}
        <div className="space-y-3 max-w-sm mb-4 shrink-0 text-center">
          {/* Status Badge Capsule */}
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-[rgb(var(--success))]/12 border border-[rgb(var(--success))]/25 text-[10px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--success))]">
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--success))] animate-pulse" />
            <span>{copy.statusBadge}</span>
          </div>

          {/* Heading & Body */}
          <div className="space-y-1.5">
            <h1 className="text-2xl sm:text-3xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
              {copy.title}
            </h1>
            <p className="text-xs sm:text-sm text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
              {copy.description}
            </p>
          </div>

          {/* Centered Capabilities Summary Badges */}
          <div className="flex items-center justify-center gap-2 pt-1 flex-wrap">
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/[0.03] border border-white/[0.06] text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
              <AudioWaveform className="w-3 h-3 text-[rgb(var(--accent))]" />
              <span>Voice Calibrated</span>
            </div>
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/[0.03] border border-white/[0.06] text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
              <ShieldCheck className="w-3 h-3 text-[rgb(var(--success))]" />
              <span>100% Private</span>
            </div>
          </div>

          {/* Clean Interactive Prompt Callout */}
          <div className="pt-1">
            <div className="inline-flex items-center gap-2 px-3 py-1.5 rounded-xl bg-[rgb(var(--accent))]/8 border border-[rgb(var(--accent))]/20 text-xs text-[rgb(var(--accent))]">
              <Sparkles className="w-3.5 h-3.5" />
              <span>{copy.actionPrompt}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
