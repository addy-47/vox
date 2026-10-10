import React, { useState, useCallback } from 'react';
import { motion } from 'framer-motion';
import { MobileHeroStep } from './MobileHeroStep';
import { MobileIntelligenceStep } from './MobileIntelligenceStep';
import { MobilePermissionStep } from './MobilePermissionStep';
import { MobileModelStep } from './MobileModelStep';
import { MobileReadyStep } from './MobileReadyStep';
import { completeSetupWizard } from '@/services/setupService';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

export type MobileStepId = 'hero' | 'intelligence' | 'permission' | 'models' | 'ready';

interface MobileOnboardingFlowProps {
  onComplete?: () => void;
  initialStep?: MobileStepId;
}

export const MobileOnboardingFlow: React.FC<MobileOnboardingFlowProps> = ({
  onComplete,
  initialStep = 'hero',
}) => {
  const [currentStep, setCurrentStep] = useState<MobileStepId>(initialStep);
  const touchStartX = React.useRef<number | null>(null);
  const touchStartY = React.useRef<number | null>(null);

  const stepsOrder: MobileStepId[] = ['hero', 'intelligence', 'permission', 'models', 'ready'];
  const currentIndex = stepsOrder.indexOf(currentStep);

  const handleSkipToVox = useCallback(async () => {
    try {
      localStorage.setItem('vox_setup_completed', 'true');
      localStorage.setItem('vox_skip_wizard_dev', 'true');
    } catch (_) {}

    try {
      await completeSetupWizard();
    } catch (_) {}

    if (onComplete) {
      onComplete();
    } else {
      window.location.reload();
    }
  }, [onComplete]);

  const handleFinish = useCallback(async () => {
    try {
      localStorage.setItem('vox_setup_completed', 'true');
    } catch (_) {}

    try {
      await completeSetupWizard();
    } catch (_) {}

    if (onComplete) {
      onComplete();
    } else {
      window.location.reload();
    }
  }, [onComplete]);

  const handleBack = () => {
    if (currentIndex > 0) {
      setCurrentStep(stepsOrder[currentIndex - 1]);
    }
  };

  const handleNext = () => {
    if (currentIndex < stepsOrder.length - 1) {
      setCurrentStep(stepsOrder[currentIndex + 1]);
    } else {
      handleFinish();
    }
  };

  // Fluid touch swipe gesture detection for Android
  const handleTouchStart = (e: React.TouchEvent) => {
    touchStartX.current = e.touches[0].clientX;
    touchStartY.current = e.touches[0].clientY;
  };

  const handleTouchEnd = (e: React.TouchEvent) => {
    if (touchStartX.current === null || touchStartY.current === null) return;
    const deltaX = e.changedTouches[0].clientX - touchStartX.current;
    const deltaY = e.changedTouches[0].clientY - touchStartY.current;

    // Detect horizontal swipe with enough threshold
    if (Math.abs(deltaX) > 45 && Math.abs(deltaX) > Math.abs(deltaY) * 1.3) {
      if (deltaX < 0) {
        // Swiped left -> advance
        handleNext();
      } else {
        // Swiped right -> go back
        handleBack();
      }
    }
    touchStartX.current = null;
    touchStartY.current = null;
  };

  return (
    <div
      className="relative w-full h-full flex flex-col bg-[rgb(var(--background))] text-[rgb(var(--foreground))] overflow-hidden select-none"
      onTouchStart={handleTouchStart}
      onTouchEnd={handleTouchEnd}
    >
      {/* Seamless Edge-to-Edge Top App Bar (Zero borders, native Android typography) */}
      <div className="shrink-0 pt-[max(0.75rem,env(safe-area-inset-top))] px-6 pb-2 z-30 flex items-center justify-between">
        {/* Left: App Logo + Wordmark (Fixed SSOT branding) */}
        <div className="flex items-center gap-2.5">
          <img
            src="/logo.png"
            alt="Vox Logo"
            className="w-7 h-7 rounded-lg object-contain"
          />
          <span className="font-display font-bold text-sm tracking-wider text-[rgb(var(--foreground))] uppercase">
            {MOBILE_ONBOARDING_COPY.header.appName}
          </span>
        </div>

        {/* Right: Small Skip Button with Breadcrumb Pill */}
        <button
          onClick={handleSkipToVox}
          className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-white/[0.05] border border-white/[0.08] text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))] hover:text-white active:bg-white/10 active:scale-95 transition-all touch-manipulation cursor-pointer"
        >
          <span>{MOBILE_ONBOARDING_COPY.header.skipButton}</span>
          <span className="opacity-40">·</span>
          <span className="opacity-75">{currentIndex + 1}/5</span>
        </button>
      </div>

      {/* Main Step Views with Instant In-Place Swipe Track (No full unmount / DOM re-render) */}
      <div className="flex-1 min-h-0 relative overflow-hidden flex flex-col">
        <motion.div
          className="flex h-full w-[500%] flex-nowrap"
          animate={{ x: `-${currentIndex * 20}%` }}
          transition={{ type: "spring", stiffness: 350, damping: 35 }}
        >
          {/* Step 1: Hero */}
          <div className="w-[20%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobileHeroStep
              onNext={handleNext}
              onSkip={handleSkipToVox}
              onSelectStep={(idx) => setCurrentStep(stepsOrder[idx])}
            />
          </div>

          {/* Step 2: Intelligence */}
          <div className="w-[20%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobileIntelligenceStep
              onNext={handleNext}
              onBack={handleBack}
              onSkip={handleSkipToVox}
              onSelectStep={(idx) => setCurrentStep(stepsOrder[idx])}
            />
          </div>

          {/* Step 3: Permissions */}
          <div className="w-[20%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobilePermissionStep
              onNext={handleNext}
              onBack={handleBack}
              onSkip={handleSkipToVox}
              onSelectStep={(idx) => setCurrentStep(stepsOrder[idx])}
            />
          </div>

          {/* Step 4: Models */}
          <div className="w-[20%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobileModelStep
              onNext={handleNext}
              onBack={handleBack}
              onSkip={handleSkipToVox}
              onSelectStep={(idx) => setCurrentStep(stepsOrder[idx])}
            />
          </div>

          {/* Step 5: Ready */}
          <div className="w-[20%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobileReadyStep
              onNext={handleFinish}
              onBack={handleBack}
              onSelectStep={(idx) => setCurrentStep(stepsOrder[idx])}
            />
          </div>
        </motion.div>
      </div>
    </div>
  );
};
