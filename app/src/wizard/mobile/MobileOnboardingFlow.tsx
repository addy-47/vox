import React, { useState } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { MobileHeroStep } from './MobileHeroStep';
import { MobilePermissionStep } from './MobilePermissionStep';
import { MobileModelStep } from './MobileModelStep';
import { MobileLiveGreetingStep } from './MobileLiveGreetingStep';

export type MobileStepId = 'hero' | 'permission' | 'model' | 'greeting';

interface MobileOnboardingFlowProps {
  onComplete?: () => void;
  initialStep?: MobileStepId;
}

export const MobileOnboardingFlow: React.FC<MobileOnboardingFlowProps> = ({
  onComplete,
  initialStep = 'hero',
}) => {
  const [currentStep, setCurrentStep] = useState<MobileStepId>(initialStep);

  const handleFinish = () => {
    if (onComplete) {
      onComplete();
    } else {
      // Default to returning to hero or reload
      setCurrentStep('hero');
    }
  };

  const stepsOrder: MobileStepId[] = ['hero', 'permission', 'model', 'greeting'];
  const currentIndex = stepsOrder.indexOf(currentStep);

  return (
    <div className="relative w-full h-full flex flex-col bg-[rgb(var(--background))] text-[rgb(var(--foreground))] overflow-hidden select-none">
      {/* Top step indicator bar (subtle 4-segment bar) */}
      <div className="shrink-0 h-4 flex items-center justify-center px-8 pt-2.5 z-20">
        <div className="flex items-center gap-1.5 w-full max-w-[160px]">
          {stepsOrder.map((step, idx) => (
            <div
              key={step}
              className={`h-1 flex-1 rounded-full transition-all duration-300 ${
                idx <= currentIndex
                  ? 'bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.4)]'
                  : 'bg-[rgb(var(--foreground))]/15'
              }`}
            />
          ))}
        </div>
      </div>

      <div className="flex-1 min-h-0 relative overflow-hidden flex flex-col">
        <AnimatePresence mode="wait">
          {currentStep === 'hero' && (
            <motion.div
              key="hero"
              initial={{ opacity: 0, scale: 0.98 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, x: -20 }}
              transition={{ duration: 0.25 }}
              className="w-full h-full flex flex-col min-h-0"
            >
            <MobileHeroStep
              onNext={() => setCurrentStep('permission')}
            />
          </motion.div>
        )}

        {currentStep === 'permission' && (
          <motion.div
            key="permission"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            transition={{ duration: 0.25 }}
            className="w-full h-full"
          >
            <MobilePermissionStep
              onNext={() => setCurrentStep('model')}
              onBack={() => setCurrentStep('hero')}
            />
          </motion.div>
        )}

        {currentStep === 'model' && (
          <motion.div
            key="model"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            transition={{ duration: 0.25 }}
            className="w-full h-full"
          >
            <MobileModelStep
              onNext={() => setCurrentStep('greeting')}
              onBack={() => setCurrentStep('permission')}
            />
          </motion.div>
        )}

        {currentStep === 'greeting' && (
          <motion.div
            key="greeting"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, scale: 0.98 }}
            transition={{ duration: 0.25 }}
            className="w-full h-full"
          >
            <MobileLiveGreetingStep
              onFinish={handleFinish}
              onBack={() => setCurrentStep('model')}
            />
          </motion.div>
        )}
      </AnimatePresence>
      </div>
    </div>
  );
};
