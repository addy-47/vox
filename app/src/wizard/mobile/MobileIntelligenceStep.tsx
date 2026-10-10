import React from 'react';
import { VoxOrb } from '@/shared/components/home';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

interface MobileIntelligenceStepProps {
  onNext?: () => void;
}

export const MobileIntelligenceStep: React.FC<MobileIntelligenceStepProps> = () => {
  const copy = MOBILE_ONBOARDING_COPY.intelligence;

  return (
    <div className="flex flex-col h-full w-full overflow-hidden select-none bg-transparent">
      {/* Centered Stage: Glowing Active Vox Orb & Architecture Copy */}
      <div className="flex-1 min-h-0 flex flex-col justify-between items-center px-6 py-4 text-center">
        {/* Glowing Active Vox Orb vertically centered - larger presence */}
        <div className="flex-1 flex items-center justify-center">
          <div className="relative w-64 h-64 sm:w-72 sm:h-72 flex items-center justify-center shrink-0">
            <div className="w-full h-full relative z-10 scale-110">
              <VoxOrb interactionState="Speaking" />
            </div>
            {/* Enhanced vibrant ambient aura */}
            <div className="absolute inset-0 bg-[rgb(var(--accent))]/25 blur-[100px] rounded-full pointer-events-none animate-pulse" />
            <div className="absolute inset-4 bg-[rgb(var(--accent))]/15 blur-[60px] rounded-full pointer-events-none" />
          </div>
        </div>

        {/* Headline & Body Copy */}
        <div className="space-y-2 max-w-sm mb-4 shrink-0">
          <h1 className="text-2xl sm:text-3xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {copy.title}
          </h1>
          <p className="text-sm text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
            {copy.description}
          </p>
        </div>
      </div>
    </div>
  );
};
