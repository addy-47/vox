import React, { useState, useCallback, useRef, useMemo } from 'react';
import { motion } from 'framer-motion';
import { MobileHeroStep } from './MobileHeroStep';
import { MobileIntelligenceStep } from './MobileIntelligenceStep';
import { MobilePermissionStep } from './MobilePermissionStep';
import { MobileRuntimeStep } from './MobileRuntimeStep';
import { MobileModelStep } from './MobileModelStep';
import { MobileReadyStep } from './MobileReadyStep';
import { MobileStepFooter } from './MobileStepFooter';
import { MobileSkipConfirmModal } from './MobileSkipConfirmModal';
import { completeSetupWizard } from '@/services/setupService';
import { MOBILE_ONBOARDING_COPY } from '@/data/welcomeCopy';

export type MobileStepId = 'hero' | 'intelligence' | 'permission' | 'runtime' | 'models' | 'ready';

const STEPS_ORDER: MobileStepId[] = ['hero', 'intelligence', 'permission', 'runtime', 'models', 'ready'];

interface MobileOnboardingFlowProps {
  onComplete?: () => void;
  initialStep?: MobileStepId;
}

export const MobileOnboardingFlow: React.FC<MobileOnboardingFlowProps> = ({
  onComplete,
  initialStep = 'hero',
}) => {
  const stepsOrder = STEPS_ORDER;
  const [currentStep, setCurrentStep] = useState<MobileStepId>(initialStep);
  const [permissionsGranted, setPermissionsGranted] = useState(false);
  const [permissionsRequesting, setPermissionsRequesting] = useState(false);
  const [isSkipModalOpen, setIsSkipModalOpen] = useState(false);
  const grantActionRef = useRef<(() => void) | null>(null);
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

  const handleModelStatusChange = useCallback((nextStatus: {
    isDone: boolean;
    isDownloading: boolean;
    label: string;
    action: () => void;
  }) => {
    setModelStatus((prev) => {
      if (
        prev.isDone === nextStatus.isDone &&
        prev.isDownloading === nextStatus.isDownloading &&
        prev.label === nextStatus.label
      ) {
        return prev;
      }
      return nextStatus;
    });
  }, []);

  const touchStartX = useRef<number | null>(null);
  const touchStartY = useRef<number | null>(null);

  const currentIndex = stepsOrder.indexOf(currentStep);
  const totalSteps = stepsOrder.length;

  const handleSkipToVox = useCallback(async () => {
    try {
      localStorage.setItem('vox_setup_completed', 'true');
      localStorage.setItem('vox_skip_wizard_dev', 'true');
    } catch (e) {
      console.debug("[MobileOnboardingFlow] Failed to set skip setup localStorage flag:", e);
    }

    try {
      await completeSetupWizard();
    } catch (e) {
      console.debug("[MobileOnboardingFlow] Failed to complete setup wizard on skip:", e);
    }

    if (onComplete) {
      onComplete();
    } else {
      window.location.reload();
    }
  }, [onComplete]);

  const handleFinish = useCallback(async () => {
    try {
      localStorage.setItem('vox_setup_completed', 'true');
    } catch (e) {
      console.debug("[MobileOnboardingFlow] Failed to set finish setup localStorage flag:", e);
    }

    try {
      await completeSetupWizard();
    } catch (e) {
      console.debug("[MobileOnboardingFlow] Failed to complete setup wizard on finish:", e);
    }

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
    // The swipe handler below calls this too, so the permission gate cannot live
    // in the footer alone — a horizontal swipe would otherwise walk straight past
    // a step whose whole purpose is to obtain the mic permission.
    if (currentStep === 'permission' && !permissionsGranted) {
      grantActionRef.current?.();
      return;
    }
    if (currentIndex < stepsOrder.length - 1) {
      setCurrentStep(stepsOrder[currentIndex + 1]);
    } else {
      void handleFinish();
    }
  }, [currentIndex, stepsOrder, handleFinish, currentStep, permissionsGranted]);

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
  const footerConfig = useMemo(() => {
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
          onClick: permissionsGranted ? handleNext : () => grantActionRef.current?.(),
          disabled: permissionsRequesting,
          isLoading: permissionsRequesting,
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
          onClick: () => {
            void handleFinish();
          },
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
  }, [
    currentStep,
    handleNext,
    permissionsGranted,
    permissionsRequesting,
    modelStatus,
    handleFinish,
  ]);

  return (
    <div
      className="relative w-full h-full flex flex-col bg-[rgb(var(--background))] text-[rgb(var(--foreground))] overflow-hidden select-none"
      onTouchStart={handleTouchStart}
      onTouchEnd={handleTouchEnd}
    >
      {/* ── Fixed Static Header (Never slides during swipe transitions) ── */}
      <div className="shrink-0 pt-4 px-4 sm:px-6 pb-2.5 z-30 flex items-center justify-between">
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

        {/* Center: Dedicated Step Progress Pill */}
        <div className="px-3 py-1 rounded-full bg-white/[0.03] border border-white/[0.06] text-[11px] font-mono tracking-wider text-[rgb(var(--foreground-muted))] uppercase">
          <span className="text-[rgb(var(--foreground))] font-semibold">{currentIndex + 1}</span>
          <span className="opacity-40 mx-1">/</span>
          <span>{totalSteps}</span>
        </div>

        {/* Right: Clean Skip Action Trigger (≥48px touch target) */}
        <button
          type="button"
          onClick={() => setIsSkipModalOpen(true)}
          className="min-h-[48px] px-3 flex items-center justify-center rounded-full text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] active:bg-white/[0.06] transition-colors touch-manipulation cursor-pointer"
        >
          {MOBILE_ONBOARDING_COPY.header.skipButton}
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
              onRegisterGrantAction={(grant) => {
                grantActionRef.current = grant;
              }}
              onRequestingChange={setPermissionsRequesting}
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
              onModelStatusChange={handleModelStatusChange}
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

      {/* ── Skip Confirmation Modal ── */}
      <MobileSkipConfirmModal
        isOpen={isSkipModalOpen}
        onClose={() => setIsSkipModalOpen(false)}
        onConfirmSkip={handleSkipToVox}
      />
    </div>
  );
};
