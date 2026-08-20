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
import { revealWizard, fetchManifest } from '@/services/modelService';
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2 } from 'lucide-react';
import { cn } from '@/shared/lib/utils';
import { TitleBar } from '@/layout/TitleBar';
import { AmbientBackground } from '@/shared/components/common';
import { useSettingsStore } from '@/store/settingsStore';
import logo from '@/assets/logo.webp';
import logoLight from '@/assets/logo-light.webp';

import { WIZARD_STEPS } from '@/data/welcomeCopy';

export const WizardRoot: React.FC = () => {
  const [state, send] = useMachine(setupMachine);
  const theme = useSettingsStore(s => s.draftSettings?.ui.theme || 'dark');
  
  React.useEffect(() => {
    // Reveal window after a short delay to ensure React is hydrated and CSS is loaded
    // This prevents the initial white flash.
    const timer = setTimeout(() => {
      revealWizard().catch(() => {});
    }, 150);

    // Fetch manifest in background early
    fetchManifest()
      .then(() => send({ type: 'MANIFEST_READY' }))
      .catch(() => {});
    
    return () => clearTimeout(timer);
  }, []);

  React.useEffect(() => {
    // Listen for model setup progress
    const unlisten = listen('model_setup_status', (event) => {
      send({ type: 'PROGRESS', data: event.payload });
    });

    return () => {
      unlisten.then(f => f());
    };
  }, [send]);

  const steps = WIZARD_STEPS.map(s => ({ ...s, icon: <s.icon className="w-4 h-4" /> }));

  const renderStep = () => {
    const error = state.context.error;
    const onBack = () => send({ type: 'BACK' });

    switch (true) {
      case state.matches('welcome'): 
        return <WelcomeStep key="welcome" onNext={() => send({ type: 'NEXT' })} />;
      
      case state.matches('checking'): 
        return <SystemCheckStep 
          key="checking" 
          onNext={() => send({ type: 'SUCCESS' })} 
          onBack={onBack} 
          error={error}
        />;
      
      case state.matches('downloading'): 
        return <ModelSetupStep 
          key="models" 
          onNext={() => send({ type: 'FINISH' })} 
          onBack={onBack}
          error={error}
          isAlreadyComplete={state.context.setupComplete}
        />;
      
      case state.matches('audio'): 
        return <AudioSetupStep 
          key="audio" 
          onNext={() => send({ type: 'NEXT' })} 
          onBack={onBack}
        />;
      
      case state.matches('testing'): 
        return <LiveTestStep 
          key="testing" 
          onNext={() => send({ type: 'NEXT' })} 
          onBack={onBack}
        />;
      
      case state.matches('completed'): 
        return <CompletedStep key="completed" onBack={onBack} />;
      
      default: return <div>Unknown State</div>;
    }
  };

  const getStepStatus = (id: string) => {
    const stepIndex = steps.findIndex(s => s.id === id);
    
    if (state.matches(id)) return 'active';
    if (stepIndex <= state.context.maxReachedIndex) return 'completed';
    return 'pending';
  };

  return (
    <div className="flex flex-col h-screen w-full bg-[rgb(var(--background))] text-[rgb(var(--foreground))] overflow-hidden font-sans selection:bg-[rgb(var(--accent))]/30">
      <AmbientBackground />
      <TitleBar />
      <div className="flex-1 flex relative overflow-hidden">
        {/* Sidebar Navigation */}
        <div className="w-[228px] glass border-r border-[rgba(var(--accent),0.06)] flex flex-col p-6 z-10">
          {/* Logo Area */}
          <div className="flex flex-col items-center justify-center mb-10 mt-4">
            <div className="relative group">
              <div className="absolute inset-[-4px] rounded-full bg-[rgb(var(--accent))]/10 blur-md opacity-0 group-hover:opacity-100 transition-opacity duration-500" />
              <div className="relative transition-all duration-300 hover:scale-110 active:scale-95" style={{ width: 32, height: 32 }}>
              <div 
                className="absolute inset-0 bg-[rgb(var(--accent))]"
                style={{
                  maskImage: `url(${theme === 'light' ? logoLight : logo})`,
                  WebkitMaskImage: `url(${theme === 'light' ? logoLight : logo})`,
                  maskSize: 'contain',
                  WebkitMaskSize: 'contain',
                  maskRepeat: 'no-repeat',
                  WebkitMaskRepeat: 'no-repeat',
                }}
              />
            </div>
            </div>
            <span className="text-sm font-black tracking-tighter text-[rgb(var(--foreground))] italic mt-3">VOX</span>
          </div>

          {/* Step Navigation */}
          <nav className="flex-1 space-y-5">
            {steps.map((s) => {
              const status = getStepStatus(s.id);
              const isReachable = steps.findIndex(step => step.id === s.id) <= state.context.maxReachedIndex;
              return (
                <button 
                  key={s.id} 
                  onClick={() => {
                    if (isReachable) send({ type: 'GO_TO', targetStep: s.id });
                  }}
                  className={cn(
                    "flex items-center gap-4 transition-all duration-500 w-full text-left outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[rgb(var(--accent))] rounded-lg",
                    status === 'pending' ? 'opacity-50 grayscale cursor-not-allowed' : 'opacity-100 hover:scale-[1.02] active:scale-95 cursor-pointer'
                  )}
                >
                  <div className={cn(
                    "w-8 h-8 rounded-xl flex items-center justify-center border transition-all duration-500 shrink-0",
                    status === 'active' 
                      ? 'bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/50 shadow-[0_0_15px_rgba(var(--accent),0.2)]' 
                      : status === 'completed' 
                        ? 'bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/20' 
                        : 'glass border-[rgba(var(--border),0.06)]'
                  )}>
                    {status === 'completed' ? <CheckCircle2 className="w-4 h-4 text-[rgb(var(--accent))]" /> : 
                     React.cloneElement(s.icon as React.ReactElement<{ className?: string }>, { 
                       className: cn("w-4 h-4", status === 'active' ? 'text-[rgb(var(--accent))]' : 'text-[rgb(var(--foreground-muted))]') 
                     })}
                  </div>
                  <div className="flex flex-col relative w-full">
                    <span className={cn(
                      "text-[12px] font-bold tracking-widest uppercase mb-0.5 transition-colors",
                      status === 'active' ? 'text-[rgb(var(--accent))]' : 'text-[rgb(var(--foreground-muted))]'
                    )}>
                      {s.label}
                    </span>
                    {status === 'active' && (
                      <motion.div layoutId="active-indicator" className="h-0.5 w-4 bg-[rgb(var(--accent))] rounded-full absolute -bottom-1" />
                    )}
                  </div>
                </button>
              );
            })}
          </nav>

          {/* System Ready Badge */}
          <div className="mt-auto pt-6 border-t border-[rgba(var(--border),0.05)]">
            <div className="flex items-center gap-2 px-3 py-2 glass">
              <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
              <span className="text-[12px] font-bold text-[rgb(var(--foreground-muted))] tracking-wider uppercase">System Ready</span>
            </div>
          </div>
        </div>

        {/* Main Content Area */}
        <main className="flex-1 relative z-10 flex flex-col px-12 py-8 overflow-hidden">
          <AnimatePresence mode="popLayout">
            <motion.div
              key={state.value as string}
              initial={{ x: 10, opacity: 0 }}
              animate={{ x: 0, opacity: 1 }}
              exit={{ x: -10, opacity: 0 }}
              transition={{ duration: 0.25, ease: [0.23, 1, 0.32, 1] }}
              className="w-full h-full flex flex-col"
            >
              <div className="w-full max-w-2xl mx-auto h-full flex flex-col">
                {renderStep()}
              </div>
            </motion.div>
          </AnimatePresence>
        </main>
      </div>
    </div>
  );
};
