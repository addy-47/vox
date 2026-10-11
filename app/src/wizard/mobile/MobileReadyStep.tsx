import React from 'react';
import {
  CheckCircle2,
  ShieldCheck,
  AudioWaveform,
  Sparkles,
  Mic,
  MessageSquare,
} from 'lucide-react';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

interface MobileReadyStepProps {
  onNext?: () => void;
}

export const MobileReadyStep: React.FC<MobileReadyStepProps> = () => {
  const copy = MOBILE_ONBOARDING_COPY.ready;

  return (
    <div className="flex flex-col h-full w-full overflow-y-auto px-6 py-4 select-none bg-transparent">
      <div className="flex-1 min-h-0 flex flex-col justify-center space-y-6 max-w-sm mx-auto w-full my-auto">
        {/* Top Status Badge & Title */}
        <div className="space-y-2 text-left">
          <div className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-[rgb(var(--accent))]/12 border border-[rgb(var(--accent))]/25 text-[10px] font-mono font-bold tracking-wider uppercase text-[rgb(var(--accent))]">
            <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-pulse" />
            <span>Setup Complete</span>
          </div>

          <h1 className="text-2xl sm:text-3xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {copy.title}
          </h1>
          <p className="text-xs sm:text-sm text-[rgb(var(--foreground-muted))] leading-relaxed">
            {copy.description}
          </p>
        </div>

        {/* System Capabilities Cards */}
        <div className="space-y-2.5 text-left">
          <div className="p-3.5 rounded-2xl bg-white/[0.03] border border-white/[0.06] flex items-center justify-between gap-3">
            <div className="flex items-center gap-3 min-w-0">
              <div className="w-9 h-9 rounded-xl bg-[rgb(var(--accent))]/12 flex items-center justify-center shrink-0 text-[rgb(var(--accent))]">
                <AudioWaveform className="w-4 h-4" />
              </div>
              <div className="flex flex-col min-w-0">
                <span className="text-xs font-semibold text-[rgb(var(--foreground))] truncate">
                  Audio & Voice Engine
                </span>
                <span className="text-[11px] text-[rgb(var(--foreground-muted))] truncate">
                  Local microphone & speech calibrated
                </span>
              </div>
            </div>
            <span className="shrink-0 text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] flex items-center gap-1">
              <CheckCircle2 className="w-3 h-3 stroke-[2.5]" />
              <span>Ready</span>
            </span>
          </div>

          <div className="p-3.5 rounded-2xl bg-white/[0.03] border border-white/[0.06] flex items-center justify-between gap-3">
            <div className="flex items-center gap-3 min-w-0">
              <div className="w-9 h-9 rounded-xl bg-emerald-500/12 flex items-center justify-center shrink-0 text-emerald-400">
                <ShieldCheck className="w-4 h-4" />
              </div>
              <div className="flex flex-col min-w-0">
                <span className="text-xs font-semibold text-[rgb(var(--foreground))] truncate">
                  Private & Offline
                </span>
                <span className="text-[11px] text-[rgb(var(--foreground-muted))] truncate">
                  Voice processing stays on your device
                </span>
              </div>
            </div>
            <span className="shrink-0 text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded bg-emerald-500/15 text-emerald-400 flex items-center gap-1">
              <CheckCircle2 className="w-3 h-3 stroke-[2.5]" />
              <span>Active</span>
            </span>
          </div>

          <div className="p-3.5 rounded-2xl bg-white/[0.03] border border-white/[0.06] flex items-center justify-between gap-3">
            <div className="flex items-center gap-3 min-w-0">
              <div className="w-9 h-9 rounded-xl bg-indigo-500/12 flex items-center justify-center shrink-0 text-indigo-400">
                <Sparkles className="w-4 h-4" />
              </div>
              <div className="flex flex-col min-w-0">
                <span className="text-xs font-semibold text-[rgb(var(--foreground))] truncate">
                  Neural Models
                </span>
                <span className="text-[11px] text-[rgb(var(--foreground-muted))] truncate">
                  Verified weights ready for conversation
                </span>
              </div>
            </div>
            <span className="shrink-0 text-[10px] font-mono font-bold uppercase px-2 py-0.5 rounded bg-indigo-500/15 text-indigo-400 flex items-center gap-1">
              <CheckCircle2 className="w-3 h-3 stroke-[2.5]" />
              <span>Ready</span>
            </span>
          </div>
        </div>

        {/* Quick Tips Section */}
        <div className="p-3.5 rounded-2xl bg-white/[0.02] border border-white/[0.04] space-y-2 text-left">
          <div className="text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
            Quick Usage Tips
          </div>
          <div className="space-y-2 text-[11px] text-[rgb(var(--foreground-muted))]">
            <div className="flex items-start gap-2">
              <Mic className="w-3.5 h-3.5 text-[rgb(var(--accent))] shrink-0 mt-0.5" />
              <span>Tap the center orb anytime to start speaking.</span>
            </div>
            <div className="flex items-start gap-2">
              <MessageSquare className="w-3.5 h-3.5 text-[rgb(var(--accent))] shrink-0 mt-0.5" />
              <span>Speak naturally in English or Hindi with real-time feedback.</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
