import React from 'react';
import { ArrowRight } from 'lucide-react';
import { VoxOrb } from '@/shared/components/home';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';
import { MobileStepFooter } from './MobileStepFooter';

interface MobileHeroStepProps {
  onNext: () => void;
  onSkip?: () => void;
  onSelectStep?: (index: number) => void;
}

export const MobileHeroStep: React.FC<MobileHeroStepProps> = ({ onNext, onSelectStep }) => {
  const copy = MOBILE_ONBOARDING_COPY.hero;

  return (
    <div className="flex flex-col h-full w-full overflow-hidden select-none bg-transparent">
      {/* Centered Stage: Ambient Dim Vox Orb & Welcome Copy */}
      <div className="flex-1 min-h-0 flex flex-col justify-between items-center px-6 py-4 text-center">
        {/* Dim Ambient Vox Orb vertically centered - larger presence */}
        <div className="flex-1 flex items-center justify-center">
          <div className="relative w-64 h-64 sm:w-72 sm:h-72 flex items-center justify-center shrink-0">
            <div className="w-full h-full relative z-10 opacity-80 scale-105">
              <VoxOrb interactionState="Thinking" />
            </div>
            <div className="absolute inset-0 bg-[rgb(var(--accent))]/12 blur-[90px] rounded-full pointer-events-none" />
          </div>
        </div>

        {/* Headline & Body Copy positioned above step slider */}
        <div className="space-y-2 max-w-sm mb-2 shrink-0">
          <h1 className="text-2xl sm:text-3xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {copy.title}
          </h1>
          <p className="text-sm text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
            {copy.description}
          </p>
        </div>
      </div>

      {/* Thumb-Anchored Bottom Action Surface with 5-Step Slider & Buttons */}
      <MobileStepFooter
        currentIndex={0}
        totalSteps={5}
        onNext={onNext}
        nextLabel={copy.getStarted}
        nextIcon={<ArrowRight className="w-4 h-4" />}
        onSelectStep={onSelectStep}
      />
    </div>
  );
};

