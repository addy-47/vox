import { CheckCircle2, ShieldCheck } from 'lucide-react';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

interface MobileReadyStepProps {
  onNext?: () => void;
}

export const MobileReadyStep: React.FC<MobileReadyStepProps> = () => {
  const copy = MOBILE_ONBOARDING_COPY.ready;

  return (
    <div className="flex flex-col h-full w-full overflow-y-auto px-6 py-4 select-none bg-transparent">
      {/* Centered Stage: Clean SVG Completion Graphic & Summary */}
      <div className="flex-1 min-h-0 flex flex-col items-center justify-center text-center my-auto">
        {/* Clean SVG Constellation / Checkmark Graphic (No Vox Orb) */}
        <div className="relative my-8 flex items-center justify-center shrink-0">
          <svg
            className="w-32 h-32 text-[rgb(var(--accent))]"
            viewBox="0 0 120 120"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            {/* Outer subtle orbital dashed ring */}
            <circle
              cx="60"
              cy="60"
              r="54"
              stroke="currentColor"
              strokeWidth="1"
              strokeDasharray="4 6"
              className="opacity-30 animate-[spin_60s_linear_infinite]"
            />
            {/* Middle glowing aura ring */}
            <circle
              cx="60"
              cy="60"
              r="44"
              stroke="currentColor"
              strokeWidth="1.5"
              className="opacity-60"
            />
            {/* Inner background fill */}
            <circle
              cx="60"
              cy="60"
              r="34"
              fill="currentColor"
              className="opacity-10"
            />
            {/* Clean checkmark path */}
            <path
              d="M44 60L55 71L76 49"
              stroke="currentColor"
              strokeWidth="4"
              strokeLinecap="round"
              strokeLinejoin="round"
              className="drop-shadow-[0_0_8px_rgba(var(--accent),0.5)]"
            />
          </svg>
          <div className="absolute inset-0 bg-[rgb(var(--accent))]/15 blur-3xl rounded-full pointer-events-none" />
        </div>

        {/* Headline & Body Copy */}
        <div className="space-y-2 max-w-sm pt-2">
          <h1 className="text-3xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {copy.title}
          </h1>
          <p className="text-sm text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
            {copy.description}
          </p>
        </div>

        {/* Verification Summary Items (Minimalist text rows, zero heavy borders) */}
        <div className="w-full max-w-xs space-y-2.5 my-6 text-left">
          <div className="flex items-center gap-2.5 text-xs text-[rgb(var(--foreground))]">
            <CheckCircle2 className="w-4 h-4 text-[rgb(var(--accent))] shrink-0 stroke-[2.5]" />
            <span className="font-mono text-[11px]">Hardware & Audio Calibrated</span>
          </div>
          <div className="flex items-center gap-2.5 text-xs text-[rgb(var(--foreground))]">
            <CheckCircle2 className="w-4 h-4 text-[rgb(var(--accent))] shrink-0 stroke-[2.5]" />
            <span className="font-mono text-[11px]">Local Engine Ready</span>
          </div>
          <div className="flex items-center gap-2.5 text-xs text-[rgb(var(--foreground))]">
            <ShieldCheck className="w-4 h-4 text-[rgb(var(--accent))] shrink-0 stroke-[2.5]" />
            <span className="font-mono text-[11px]">On-Device Private & Offline</span>
          </div>
        </div>
      </div>
    </div>
  );
};
