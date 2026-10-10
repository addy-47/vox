import React, { useState, useCallback, useRef } from 'react';
import { motion } from 'framer-motion';
import { MobileHeroStep } from './MobileHeroStep';
import { MobileIntelligenceStep } from './MobileIntelligenceStep';
import { MobilePermissionStep } from './MobilePermissionStep';
import { MobileRuntimeStep } from './MobileRuntimeStep';
import { MobileModelStep } from './MobileModelStep';
import { MobileReadyStep } from './MobileReadyStep';
import { MobileStepFooter } from './MobileStepFooter';
import { completeSetupWizard } from '@/services/setupService';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

export type MobileStepId = 'hero' | 'intelligence' | 'permission' | 'runtime' | 'models' | 'ready';

interface MobileOnboardingFlowProps {
  onComplete?: () => void;
  initialStep?: MobileStepId;
}

export const MobileOnboardingFlow: React.FC<MobileOnboardingFlowProps> = ({
  onComplete,
  initialStep = 'hero',
}) => {
  const stepsOrder: MobileStepId[] = ['hero', 'intelligence', 'permission', 'runtime', 'models', 'ready'];
  const [currentStep, setCurrentStep] = useState<MobileStepId>(initialStep);
  const [permissionsGranted, setPermissionsGranted] = useState(false);
  const [modelStatus, setModelStatus] = useState<{
    isDone: boolean;
    isDownloading: boolean;
    label: string;
    action: () => void;
  }>({
    isDone: false,
    isDownloading: false,
    label: MOBILE_ONBOARDING_COPY.models.downloadButton,
    action: () => {},
  });

  const touchStartX = useRef<number | null>(null);
  const touchStartY = useRef<number | null>(null);

  const currentIndex = stepsOrder.indexOf(currentStep);
  const totalSteps = stepsOrder.length;

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

  const handleBack = useCallback(() => {
    if (currentIndex > 0) {
      setCurrentStep(stepsOrder[currentIndex - 1]);
    }
  }, [currentIndex, stepsOrder]);

  const handleNext = useCallback(() => {
    if (currentIndex < stepsOrder.length - 1) {
      setCurrentStep(stepsOrder[currentIndex + 1]);
    } else {
      handleFinish();
    }
  }, [currentIndex, stepsOrder, handleFinish]);

  // Fluid touch swipe gestures for natural page paging
  const handleTouchStart = (e: React.TouchEvent) => {
    touchStartX.current = e.touches[0].clientX;
    touchStartY.current = e.touches[0].clientY;
  };

  const handleTouchEnd = (e: React.TouchEvent) => {
    if (touchStartX.current === null || touchStartY.current === null) return;
    const deltaX = e.changedTouches[0].clientX - touchStartX.current;
    const deltaY = e.changedTouches[0].clientY - touchStartY.current;

    // Detect horizontal swipe with clean threshold
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

  // Determine current fixed bottom action button properties
  const getFooterConfig = () => {
    switch (currentStep) {
      case 'hero':
        return {
          label: MOBILE_ONBOARDING_COPY.hero.getStarted,
          onClick: handleNext,
          disabled: false,
          isLoading: false,
        };
      case 'intelligence':
        return {
          label: MOBILE_ONBOARDING_COPY.intelligence.next,
          onClick: handleNext,
          disabled: false,
          isLoading: false,
        };
      case 'permission':
        return {
          label: permissionsGranted
            ? MOBILE_ONBOARDING_COPY.permission.continueButton
            : MOBILE_ONBOARDING_COPY.permission.grantButton,
          onClick: handleNext,
          disabled: false,
          isLoading: false,
        };
      case 'runtime':
        return {
          label: MOBILE_ONBOARDING_COPY.runtime.continueButton,
          onClick: handleNext,
          disabled: false,
          isLoading: false,
        };
      case 'models':
        return {
          label: modelStatus.label,
          onClick: modelStatus.isDone ? handleNext : modelStatus.action,
          disabled: modelStatus.isDownloading,
          isLoading: modelStatus.isDownloading,
        };
      case 'ready':
        return {
          label: MOBILE_ONBOARDING_COPY.ready.continueButton,
          onClick: handleFinish,
          disabled: false,
          isLoading: false,
        };
      default:
        return {
          label: 'Continue',
          onClick: handleNext,
          disabled: false,
          isLoading: false,
        };
    }
  };

  const footerConfig = getFooterConfig();

  return (
    <div
      className="relative w-full h-full flex flex-col bg-[rgb(var(--background))] text-[rgb(var(--foreground))] overflow-hidden select-none"
      onTouchStart={handleTouchStart}
      onTouchEnd={handleTouchEnd}
    >
      {/* ── Fixed Static Header (Never slides during swipe transitions) ── */}
      <div className="shrink-0 pt-[max(0.75rem,env(safe-area-inset-top))] px-6 pb-2 z-30 flex items-center justify-between">
        {/* Left: App Logo + Wordmark */}
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

        {/* Right: Clean Skip Action with Step Counter */}
        <button
          type="button"
          onClick={handleSkipToVox}
          className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-white/[0.04] text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))] hover:text-white active:scale-95 transition-all touch-manipulation cursor-pointer"
        >
          <span>{MOBILE_ONBOARDING_COPY.header.skipButton}</span>
          <span className="opacity-40">·</span>
          <span className="opacity-75">{currentIndex + 1}/{totalSteps}</span>
        </button>
      </div>

      {/* ── Sliding Central Content Stage (Only central content translates) ── */}
      <div className="flex-1 min-h-0 relative overflow-hidden flex flex-col">
        <motion.div
          className="flex h-full w-[600%] flex-nowrap"
          animate={{ x: `-${currentIndex * (100 / totalSteps)}%` }}
          transition={{ type: "spring", stiffness: 350, damping: 35 }}
        >
          {/* Step 1: Hero */}
          <div className="w-[16.666%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobileHeroStep onNext={handleNext} />
          </div>

          {/* Step 2: Intelligence */}
          <div className="w-[16.666%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobileIntelligenceStep onNext={handleNext} />
          </div>

          {/* Step 3: Permissions */}
          <div className="w-[16.666%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobilePermissionStep
              onNext={handleNext}
              onPermissionsChange={setPermissionsGranted}
            />
          </div>

          {/* Step 4: Compute Runtime */}
          <div className="w-[16.666%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobileRuntimeStep onNext={handleNext} />
          </div>

          {/* Step 5: Models */}
          <div className="w-[16.666%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobileModelStep
              onNext={handleNext}
              onModelStatusChange={setModelStatus}
            />
          </div>

          {/* Step 6: Ready */}
          <div className="w-[16.666%] h-full flex flex-col min-h-0 overflow-hidden shrink-0">
            <MobileReadyStep onNext={handleFinish} />
          </div>
        </motion.div>
      </div>

      {/* ── Fixed Static Bottom Action Footer (Never slides during transitions) ── */}
      <MobileStepFooter
        currentIndex={currentIndex}
        totalSteps={totalSteps}
        label={footerConfig.label}
        onClick={footerConfig.onClick}
        disabled={footerConfig.disabled}
        isLoading={footerConfig.isLoading}
      />
    </div>
  );
};
