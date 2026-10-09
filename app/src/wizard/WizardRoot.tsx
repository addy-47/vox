import React from 'react';
import { useMachine } from '@xstate/react';
import { setupMachine, type SetupStep } from './state/setupMachine';
import { motion, AnimatePresence } from 'framer-motion';
import { WelcomeStep } from "./steps/WelcomeStep";
import { SystemCheckStep } from "./steps/SystemCheckStep";
import { ModelSetupStep } from "./steps/ModelSetupStep";
import { AudioSetupStep } from "./steps/AudioSetupStep";
import { LiveTestStep } from "./steps/LiveTestStep";
import { CompletedStep } from "./steps/CompletedStep";
import { WizardErrorPanel } from "./components/WizardErrorPanel";
import { revealWizard, fetchManifest } from '@/services/setupService';


import { CheckCircle2 } from 'lucide-react';
import { cn } from '@/shared/lib/utils';
import { TitleBar } from '@/layout/TitleBar';
import { AmbientBackground, VoxLogoLoader } from '@/shared/components/common';
import { WIZARD_STEPS, WIZARD_STATUS_COPY } from '@/data/welcomeCopy';

import { MobileOnboardingFlow } from './mobile/MobileOnboardingFlow';
import { Smartphone, Monitor, Maximize2, Minimize2, RotateCcw } from 'lucide-react';

// Module scope: the step table never changes, so resolve the icon
// components and index lookup once instead of rebuilding (with JSX +
// cloneElement) on every render.
const STEPS = WIZARD_STEPS.map(s => ({ ...s, Icon: s.icon }));
const STEP_INDEX = new Map(STEPS.map((s, i) => [s.id, i] as const));

export const WizardRoot: React.FC = () => {
  const [state, send] = useMachine(setupMachine);
  const [mobileResetKey, setMobileResetKey] = React.useState(0);
  const [viewMode, setViewMode] = React.useState<'desktop' | 'mobile'>(() => {
    if (typeof window !== 'undefined') {
      const params = new URLSearchParams(window.location.search);
      if (params.get('mode') === 'mobile') return 'mobile';
      if (window.innerWidth < 768) return 'mobile';
    }
    return 'desktop';
  });
  const [frameMode, setFrameMode] = React.useState<'device' | 'fullscreen'>('device');
  
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
      
      case state.matches('error'):
        return <WizardErrorPanel
          key="error"
          message={error}
          onRetry={() => send({ type: 'RETRY' })}
          onBack={() => send({ type: 'BACK' })}
        />;
      
      default: return <div>{WIZARD_STATUS_COPY.unknownState}</div>;
    }
  };

  const getStepStatus = (id: SetupStep) => {
    const stepIndex = STEP_INDEX.get(id) ?? -1;

    if (state.matches(id)) return 'active';
    if (stepIndex <= state.context.maxReachedIndex) return 'completed';
    return 'pending';
  };

  return (
    <div className="flex flex-col h-screen w-full bg-[rgb(var(--background))] text-[rgb(var(--foreground))] overflow-hidden font-sans selection:bg-[rgb(var(--accent))]/30">
      <AmbientBackground instanceId="wizard" />
      <TitleBar />

      {/* Mode Switcher Banner on Desktop */}
      <div className="hidden sm:flex items-center justify-between px-6 py-2 border-b border-[rgba(var(--accent),0.06)] bg-black/20 backdrop-blur-sm z-20 text-xs">
        <div className="flex items-center gap-2">
          <span className="text-[11px] font-mono uppercase tracking-widest text-[rgb(var(--foreground-muted))]">
            View Mode:
          </span>
          <div className="inline-flex rounded-lg p-0.5 bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--foreground),0.1)]">
            <button
              onClick={() => setViewMode('desktop')}
              className={cn(
                "flex items-center gap-1.5 px-2.5 py-1 rounded-md transition-all font-medium",
                viewMode === 'desktop'
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-sm"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              )}
            >
              <Monitor className="w-3.5 h-3.5" />
              Desktop Setup
            </button>
            <button
              onClick={() => setViewMode('mobile')}
              className={cn(
                "flex items-center gap-1.5 px-2.5 py-1 rounded-md transition-all font-medium",
                viewMode === 'mobile'
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-sm"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              )}
            >
              <Smartphone className="w-3.5 h-3.5" />
              Android Flow
            </button>
          </div>
        </div>

        {viewMode === 'mobile' && (
          <div className="flex items-center gap-2">
            <button
              onClick={() => setFrameMode(f => f === 'device' ? 'fullscreen' : 'device')}
              className="flex items-center gap-1 px-2 py-1 rounded border border-[rgba(var(--foreground),0.1)] hover:bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              title="Toggle Device Frame"
            >
              {frameMode === 'device' ? <Maximize2 className="w-3.5 h-3.5" /> : <Minimize2 className="w-3.5 h-3.5" />}
              <span>{frameMode === 'device' ? 'Fill Viewport' : 'Phone Frame'}</span>
            </button>
            <button
              onClick={() => setMobileResetKey(k => k + 1)}
              className="flex items-center gap-1 px-2 py-1 rounded border border-[rgba(var(--foreground),0.1)] hover:bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
              title="Restart Flow"
            >
              <RotateCcw className="w-3.5 h-3.5" />
              <span>Restart Flow</span>
            </button>
          </div>
        )}
      </div>

      {viewMode === 'mobile' ? (
        frameMode === 'device' ? (
          <div className="flex-1 flex flex-col items-center justify-center p-2 sm:p-4 overflow-hidden relative z-10">
            {/* Realistic Android Phone Mockup Frame */}
            <div className="relative w-[380px] max-w-full h-full max-h-[min(740px,calc(100vh-110px))] rounded-[40px] p-2 bg-[#0e1017] border-[3px] border-[#2b303c] shadow-[0_25px_60px_-10px_rgba(0,0,0,0.9)] ring-1 ring-white/10 flex flex-col overflow-hidden">
              {/* Android Punch-Hole Camera */}
              <div className="shrink-0 h-6 flex items-center justify-center relative pointer-events-none z-40">
                <div className="w-3.5 h-3.5 rounded-full bg-black border border-white/20 flex items-center justify-center">
                  <div className="w-1.5 h-1.5 rounded-full bg-[#161c28]" />
                </div>
              </div>

              {/* Mobile Screen Shell */}
              <div className="flex-1 min-h-0 w-full rounded-[30px] overflow-hidden bg-[rgb(var(--background))] relative flex flex-col">
                <MobileOnboardingFlow
                  key={mobileResetKey}
                  onComplete={() => setMobileResetKey(k => k + 1)}
                />
              </div>

              {/* Android Gesture Navigation Bar */}
              <div className="shrink-0 h-5 flex items-center justify-center pointer-events-none z-40">
                <div className="w-28 h-1 rounded-full bg-white/25" />
              </div>
            </div>
          </div>
        ) : (
          <div className="flex-1 w-full h-full relative z-10 flex flex-col min-h-0 overflow-hidden">
            <MobileOnboardingFlow
              key={mobileResetKey}
              onComplete={() => setMobileResetKey(k => k + 1)}
            />
          </div>
        )
      ) : (
        <div className="flex-1 flex relative overflow-hidden">
          {/* Sidebar Navigation — hidden on phones; the step header carries progress there */}
          <div className="hidden md:flex w-[228px] glass border-r border-[rgba(var(--accent),0.06)] flex-col p-6 z-10">
            {/* Step Navigation */}
            <nav className="flex-1 space-y-5 pt-6">
              {STEPS.map((s) => {
                const status = getStepStatus(s.id);
                const isReachable = (STEP_INDEX.get(s.id) ?? -1) <= state.context.maxReachedIndex;
                return (
                  <button 
                    key={s.id} 
                    onClick={() => {
                      if (isReachable) send({ type: 'GO_TO', targetStep: s.id });
                    }}
                    className={cn(
                      "flex items-center gap-4 transition-transform w-full text-left outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[rgb(var(--accent))] rounded-lg",
                      status === 'pending' ? 'opacity-50 grayscale cursor-not-allowed' : 'opacity-100 hover:scale-[1.02] active:scale-95 cursor-pointer'
                    )}
                  >
                    <div className={cn(
                      "w-8 h-8 rounded-xl flex items-center justify-center border transition-colors shrink-0",
                      status === 'active' 
                        ? 'bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/50 shadow-[0_0_15px_rgba(var(--accent),0.2)]' 
                        : status === 'completed' 
                          ? 'bg-[rgb(var(--accent))]/20 border-[rgb(var(--accent))]/20' 
                          : 'glass border-[rgba(var(--border),0.06)]'
                    )}>
                      {status === 'completed' ? <CheckCircle2 className="w-4 h-4 text-[rgb(var(--accent))]" /> :
                        <s.Icon className={cn("w-4 h-4", status === 'active' ? 'text-[rgb(var(--accent))]' : 'text-[rgb(var(--foreground-muted))]')} />}
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

            {/* Bottom Brand Mark — flush to the bottom corner */}
            <div className="mt-auto -mx-6 -mb-6 px-6 py-4 flex flex-row items-center gap-2.5 border-t border-[rgba(var(--border),0.05)]">
              <VoxLogoLoader size={28} animated={false} />
              <span className="text-sm font-black tracking-tighter text-[rgb(var(--foreground))] italic">VOX</span>
            </div>
          </div>

          {/* Main Content Area — scrolls on small screens, fixed on desktop */}
          <main className="flex-1 relative z-10 flex flex-col px-3.5 py-4 sm:px-8 sm:py-6 lg:px-12 lg:py-8 overflow-y-auto lg:overflow-hidden">
            {/* Phone step progress — the sidebar nav is hidden below md */}
            <div className="md:hidden w-full max-w-2xl mx-auto flex items-center gap-1.5 pb-3 sm:pb-4 shrink-0" aria-hidden="true">
              {STEPS.map((s) => {
                const status = getStepStatus(s.id);
                return (
                  <div
                    key={s.id}
                    className={cn(
                      "h-1 flex-1 rounded-full transition-colors",
                      status === "completed"
                        ? "bg-[rgb(var(--accent))]"
                        : status === "active"
                          ? "bg-[rgb(var(--accent))]/60"
                          : "bg-[rgb(var(--foreground))]/10"
                    )}
                  />
                );
              })}
            </div>
            <AnimatePresence mode="popLayout">
              <motion.div
                key={state.value}
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
      )}
    </div>
  );
};
