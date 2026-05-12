import React from 'react';
import { useMachine } from '@xstate/react';
import { setupMachine } from './state/setupMachine';
import { motion, AnimatePresence } from 'framer-motion';
import { WelcomeStep } from "./steps/WelcomeStep";
import { SystemCheckStep } from "./steps/SystemCheckStep";
import { ModelSetupStep } from "./steps/ModelSetupStep";
import { AudioSetupStep } from "./steps/AudioSetupStep";
import { LiveTestStep } from "./steps/LiveTestStep";
import { CompletedStep } from "./steps/CompletedStep";

export const WizardRoot: React.FC = () => {
  const [state, send] = useMachine(setupMachine);

  // Transitions for step changes
  const variants = {
    enter: { x: 50, opacity: 0 },
    center: { x: 0, opacity: 1 },
    exit: { x: -50, opacity: 0 }
  };

  const renderStep = () => {
    switch (true) {
      case state.matches('welcome'):
        return <WelcomeStep key="welcome" onNext={() => send({ type: 'NEXT' })} />;
      case state.matches('checking'):
        return <SystemCheckStep key="checking" onNext={() => send({ type: 'SUCCESS' })} />;
      case state.matches('downloading'):
        return <ModelSetupStep key="models" onNext={() => send({ type: 'FINISH' })} />;
      case state.matches('audio'):
        return <AudioSetupStep key="audio" onNext={() => send({ type: 'NEXT' })} />;
      case state.matches('testing'):
        return <LiveTestStep key="testing" onNext={() => send({ type: 'NEXT' })} />;
      case state.matches('completed'):
        return <CompletedStep key="completed" />;
      default:
        return <div>Unknown State</div>;
    }
  };

  return (
    <div className="fixed inset-0 bg-neutral-950 flex items-center justify-center overflow-hidden font-geist">
      {/* Dynamic Background */}
      <div className="absolute inset-0 z-0">
        <div className="absolute top-[-10%] left-[-10%] w-[40%] h-[40%] bg-indigo-500/20 blur-[120px] rounded-full animate-pulse" />
        <div className="absolute bottom-[-10%] right-[-10%] w-[40%] h-[40%] bg-purple-500/20 blur-[120px] rounded-full animate-pulse" />
      </div>

      <div className="relative z-10 w-full max-w-2xl px-6">
        <AnimatePresence mode="wait">
          <motion.div
            key={state.value as string}
            variants={variants}
            initial="enter"
            animate="center"
            exit="exit"
            transition={{ duration: 0.4, ease: "circOut" }}
            className="w-full"
          >
            {renderStep()}
          </motion.div>
        </AnimatePresence>
      </div>

      {/* Progress Indicator */}
      <div className="absolute bottom-8 left-0 right-0 flex justify-center gap-2">
        {['welcome', 'checking', 'downloading', 'audio', 'testing'].map((s) => (
          <div 
            key={s}
            className={`h-1.5 rounded-full transition-all duration-500 ${
              state.matches(s) ? 'w-8 bg-indigo-500' : 'w-2 bg-neutral-800'
            }`}
          />
        ))}
      </div>
    </div>
  );
};
