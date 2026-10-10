import { ArrowRight, CheckCircle2, ShieldCheck } from 'lucide-react';
import { VoxOrb } from '@/shared/components/home';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';
import { MobileStepFooter } from './MobileStepFooter';

interface MobileReadyStepProps {
  onNext: () => void;
  onBack: () => void;
  onSelectStep?: (index: number) => void;
}

export const MobileReadyStep: React.FC<MobileReadyStepProps> = ({
  onNext,
  onBack,
  onSelectStep,
}) => {
  const copy = MOBILE_ONBOARDING_COPY.ready;

  return (
    <div className="flex flex-col h-full w-full overflow-hidden select-none bg-transparent">
      {/* Centered Stage: Verified Ready Vox Orb & Confirmation Copy */}
      <div className="flex-1 min-h-0 flex flex-col items-center justify-center px-6 text-center">
        {/* Active Vibrant Vox Orb with Ready Glow */}
        <div className="relative w-44 h-44 sm:w-52 sm:h-52 flex items-center justify-center shrink-0 mb-6">
          <div className="w-full h-full relative z-10">
            <VoxOrb interactionState="Idle" />
          </div>
          {/* Radiant green/accent confirmation aura */}
          <div className="absolute inset-0 bg-[rgb(var(--accent))]/20 blur-[80px] rounded-full pointer-events-none" />
          <div className="absolute -bottom-2 right-4 z-20 w-8 h-8 rounded-full bg-[rgb(var(--accent))] text-black flex items-center justify-center shadow-lg">
            <CheckCircle2 className="w-5 h-5 stroke-[2.5]" />
          </div>
        </div>

        {/* Headline & Body Copy */}
        <div className="space-y-2.5 max-w-sm">
          <h1 className="text-3xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {copy.title}
          </h1>
          <p className="text-sm text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
            {copy.description}
          </p>
        </div>

        {/* Verification Checklist Card */}
        <div className="w-full max-w-xs glass-card rounded-2xl p-3.5 space-y-2 border border-[rgba(var(--foreground),0.08)] mt-5 text-left">
          <div className="flex items-center gap-2.5 text-xs text-[rgb(var(--foreground))]">
            <CheckCircle2 className="w-3.5 h-3.5 text-[rgb(var(--accent))] shrink-0" />
            <span className="font-mono text-[11px]">Audio Stream Calibrated</span>
          </div>
          <div className="flex items-center gap-2.5 text-xs text-[rgb(var(--foreground))]">
            <CheckCircle2 className="w-3.5 h-3.5 text-[rgb(var(--accent))] shrink-0" />
            <span className="font-mono text-[11px]">On-Device Engine Active</span>
          </div>
          <div className="flex items-center gap-2.5 text-xs text-[rgb(var(--foreground))]">
            <ShieldCheck className="w-3.5 h-3.5 text-[rgb(var(--accent))] shrink-0" />
            <span className="font-mono text-[11px]">Zero Cloud Telemetry</span>
          </div>
        </div>
      </div>

      {/* Thumb-Anchored Bottom Action Surface with 5-Step Slider & Vertically Stacked Buttons */}
      <MobileStepFooter
        currentIndex={4}
        totalSteps={5}
        onNext={onNext}
        onBack={onBack}
        nextLabel={copy.continueButton}
        nextIcon={<ArrowRight className="w-4 h-4" />}
        onSelectStep={onSelectStep}
      />
    </div>
  );
};
