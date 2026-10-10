import { ArrowRight } from 'lucide-react';
import { VoxOrb } from '@/shared/components/home';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';
import { MobileStepFooter } from './MobileStepFooter';

interface MobileIntelligenceStepProps {
  onNext: () => void;
  onBack: () => void;
  onSkip?: () => void;
  onSelectStep?: (index: number) => void;
}

export const MobileIntelligenceStep: React.FC<MobileIntelligenceStepProps> = ({
  onNext,
  onBack,
  onSelectStep,
}) => {
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

      {/* Thumb-Anchored Bottom Action Surface with 5-Step Slider & Vertically Stacked Buttons */}
      <MobileStepFooter
        currentIndex={1}
        totalSteps={5}
        onNext={onNext}
        onBack={onBack}
        nextLabel={copy.next}
        nextIcon={<ArrowRight className="w-4 h-4" />}
        onSelectStep={onSelectStep}
      />
    </div>
  );
};
