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
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, Settings2, Shield, Mic2, Sparkles, Home } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

export const WizardRoot: React.FC = () => {
  const [state, send] = useMachine(setupMachine);
  
  React.useEffect(() => {
    // Reveal window after a short delay to ensure React is hydrated and CSS is loaded
    // This prevents the initial white flash.
    const timer = setTimeout(() => {
      invoke('reveal_wizard').catch(() => {});
    }, 150);

    // Fetch manifest in background early
    invoke('fetch_manifest')
      .then(() => send({ type: 'MANIFEST_READY' }))
      .catch(() => {});
    
    return () => clearTimeout(timer);
  }, [send]);

  React.useEffect(() => {
    // Listen for model setup progress
    const unlisten = listen('model_setup_status', (event) => {
      const payload = event.payload as any;
      if (payload.step === 'completed') {
        send({ type: 'FINISH' });
      } else {
        send({ type: 'PROGRESS', data: payload });
      }
    });

    return () => {
      unlisten.then(f => f());
    };
  }, [send]);

  const steps = [
    { id: 'welcome', label: 'Welcome', icon: <Home className="w-4 h-4" /> },
    { id: 'checking', label: 'System Check', icon: <Shield className="w-4 h-4" /> },
    { id: 'downloading', label: 'Neural Models', icon: <Settings2 className="w-4 h-4" /> },
    { id: 'audio', label: 'Audio Pipeline', icon: <Mic2 className="w-4 h-4" /> },
    { id: 'testing', label: 'Live Test', icon: <Sparkles className="w-4 h-4" /> },
  ];

  const renderStep = () => {
    switch (true) {
      case state.matches('welcome'): return <WelcomeStep key="welcome" onNext={() => send({ type: 'NEXT' })} />;
      case state.matches('checking'): return <SystemCheckStep key="checking" onNext={() => send({ type: 'SUCCESS' })} />;
      case state.matches('downloading'): return <ModelSetupStep key="models" onNext={() => send({ type: 'FINISH' })} />;
      case state.matches('audio'): return <AudioSetupStep key="audio" onNext={() => send({ type: 'NEXT' })} />;
      case state.matches('testing'): return <LiveTestStep key="testing" onNext={() => send({ type: 'NEXT' })} />;
      case state.matches('completed'): return <CompletedStep key="completed" />;
      default: return <div>Unknown State</div>;
    }
  };

  const getStepStatus = (id: string) => {
    const currentIndex = steps.findIndex(s => state.matches(s.id));
    const stepIndex = steps.findIndex(s => s.id === id);
    
    if (state.matches(id)) return 'active';
    if (stepIndex < currentIndex) return 'completed';
    return 'pending';
  };

  return (
    <div className="flex h-screen w-full bg-[#050505] text-white overflow-hidden font-inter selection:bg-[#00dbe9]/30">
      <div className="flex-1 flex relative overflow-hidden">
        {/* Background Effects */}
        <div className="absolute inset-0 z-0 overflow-hidden pointer-events-none">
          <div className="absolute top-[-10%] left-[-10%] w-[60%] h-[60%] bg-[#00dbe9]/5 blur-[120px] rounded-full" />
          <div className="absolute bottom-[-10%] right-[-10%] w-[60%] h-[60%] bg-[#d8baff]/5 blur-[120px] rounded-full" />
          <div className="absolute inset-0 opacity-[0.03] pointer-events-none mix-blend-overlay" 
               style={{ backgroundImage: `url("data:image/svg+xml,%3Csvg viewBox='0 0 250 250' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noiseFilter'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.65' numOctaves='3' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noiseFilter)'/%3E%3C/svg%3E")` }} />
        </div>

        {/* Sidebar Navigation */}
        <div className="w-[280px] border-r border-white/5 bg-white/[0.01] backdrop-blur-3xl flex flex-col p-8 z-10">
          <div className="flex items-center gap-3 mb-12">
            <img src="/logo.png" className="w-8 h-8" alt="Vox" />
            <span className="text-lg font-black tracking-tighter text-white italic">VOX</span>
          </div>

          <nav className="flex-1 space-y-6">
            {steps.map((s) => {
              const status = getStepStatus(s.id);
              return (
                <div key={s.id} className={cn(
                  "flex items-center gap-4 transition-all duration-500",
                  status === 'pending' ? 'opacity-80 grayscale' : 'opacity-100'
                )}>
                  <div className={cn(
                    "w-8 h-8 rounded-xl flex items-center justify-center border transition-all duration-500",
                    status === 'active' ? 'bg-[#00dbe9]/10 border-[#00dbe9]/50 shadow-[0_0_15px_rgba(0,219,233,0.2)]' : 
                    status === 'completed' ? 'bg-[#00dbe9]/20 border-transparent' : 'bg-white/5 border-white/10'
                  )}>
                    {status === 'completed' ? <CheckCircle2 className="w-4 h-4 text-[#00dbe9]" /> : 
                     React.cloneElement(s.icon as React.ReactElement<{ className?: string }>, { 
                       className: cn("w-4 h-4", status === 'active' ? 'text-[#00dbe9]' : 'text-white/80') 
                     })}
                  </div>
                  <div className="flex flex-col">
                    <span className={cn(
                      "text-[11px] font-bold tracking-widest uppercase mb-0.5",
                      status === 'active' ? 'text-[#00dbe9]' : 'text-white/80'
                    )}>
                      {s.label}
                    </span>
                    {status === 'active' && (
                      <motion.div layoutId="active-indicator" className="h-0.5 w-4 bg-[#00dbe9] rounded-full" />
                    )}
                  </div>
                </div>
              );
            })}
          </nav>

          <div className="mt-auto pt-8 border-t border-white/5">
            <div className="flex items-center gap-2 px-3 py-2 bg-white/5 rounded-lg border border-white/5">
              <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
              <span className="text-[11px] font-bold text-white/80 tracking-wider uppercase">System Ready</span>
            </div>
          </div>
        </div>

        {/* Main Content Area */}
        <main className="flex-1 relative z-10 flex items-center justify-center p-12 overflow-hidden">
          <AnimatePresence mode="popLayout">
            <motion.div
              key={state.value as string}
              initial={{ x: 10, opacity: 0 }}
              animate={{ x: 0, opacity: 1 }}
              exit={{ x: -10, opacity: 0 }}
              transition={{ duration: 0.25, ease: [0.23, 1, 0.32, 1] }}
              className="w-full h-full flex items-center justify-center"
            >
              <div className="w-full max-w-2xl">
                {renderStep()}
              </div>
            </motion.div>
          </AnimatePresence>
        </main>
      </div>
    </div>
  );
};
