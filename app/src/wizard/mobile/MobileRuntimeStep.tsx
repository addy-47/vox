import React from 'react';
import { Cpu, Server, ShieldCheck, Zap } from 'lucide-react';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

interface MobileRuntimeStepProps {
  onNext?: () => void;
}

export const MobileRuntimeStep: React.FC<MobileRuntimeStepProps> = () => {
  const copy = MOBILE_ONBOARDING_COPY.runtime;

  return (
    <div className="flex flex-col h-full w-full overflow-y-auto px-6 py-4 select-none bg-transparent">
      <div className="flex-1 min-h-0 flex flex-col items-center justify-center text-center my-auto">
        {/* Dynamic Dual-Compute Glyph Graphic */}
        <div className="relative my-6 flex items-center justify-center shrink-0">
          <div className="flex items-center gap-4 relative z-10">
            {/* On-Device Node */}
            <div className="flex flex-col items-center gap-2">
              <div className="w-16 h-16 rounded-2xl bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/30 flex items-center justify-center shadow-[0_0_24px_rgba(var(--accent),0.2)]">
                <Cpu className="w-8 h-8 text-[rgb(var(--accent))] stroke-[1.5]" />
              </div>
              <span className="text-[11px] font-mono font-semibold text-[rgb(var(--foreground))] uppercase tracking-wider">
                On-Device
              </span>
            </div>

            {/* Connecting Wireless/Neural Bridge */}
            <div className="flex items-center gap-1 opacity-60">
              <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
              <span className="w-6 h-0.5 bg-gradient-to-r from-[rgb(var(--accent))] to-[rgba(var(--foreground),0.2)]" />
              <Zap className="w-3.5 h-3.5 text-[rgb(var(--accent))]" />
              <span className="w-6 h-0.5 bg-gradient-to-r from-[rgba(var(--foreground),0.2)] to-[rgb(var(--accent))]" />
              <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
            </div>

            {/* Remote Node */}
            <div className="flex flex-col items-center gap-2">
              <div className="w-16 h-16 rounded-2xl bg-white/[0.04] border border-white/[0.08] flex items-center justify-center shadow-lg">
                <Server className="w-8 h-8 text-[rgb(var(--foreground-muted))] stroke-[1.5]" />
              </div>
              <span className="text-[11px] font-mono font-semibold text-[rgb(var(--foreground-muted))] uppercase tracking-wider">
                Remote Vox
              </span>
            </div>
          </div>
          <div className="absolute inset-0 bg-[rgb(var(--accent))]/10 blur-3xl rounded-full pointer-events-none" />
        </div>

        {/* Title & Body */}
        <div className="space-y-2 max-w-sm pt-2">
          <h1 className="text-2xl sm:text-3xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {copy.title}
          </h1>
          <p className="text-sm text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
            {copy.description}
          </p>
        </div>

        {/* Informational Cards (Clean, transparent, no heavy card borders) */}
        <div className="w-full max-w-sm space-y-3 my-6 text-left">
          <div className="flex items-start gap-3">
            <ShieldCheck className="w-4 h-4 text-[rgb(var(--accent))] shrink-0 mt-0.5 stroke-[2]" />
            <div className="space-y-0.5">
              <span className="text-xs font-semibold text-[rgb(var(--foreground))] block">
                {copy.localTitle}
              </span>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed">
                {copy.localDesc}
              </p>
            </div>
          </div>

          <div className="flex items-start gap-3">
            <Server className="w-4 h-4 text-[rgb(var(--accent))] shrink-0 mt-0.5 stroke-[2]" />
            <div className="space-y-0.5">
              <span className="text-xs font-semibold text-[rgb(var(--foreground))] block">
                {copy.remoteTitle}
              </span>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed">
                {copy.remoteDesc}
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
