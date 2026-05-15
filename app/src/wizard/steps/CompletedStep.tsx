import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';
import { Sparkles, ArrowRight, Zap, ShieldCheck, Check } from 'lucide-react';

export const CompletedStep: React.FC = () => {
  const handleFinish = async () => {
    await invoke('complete_setup_wizard');
    window.location.reload(); // Refresh to trigger App.tsx routing logic
  };

  return (
    <div className="flex flex-col h-full relative">
      <header className="mb-8 text-center sm:text-left">
        <div className="flex items-center justify-center sm:justify-start gap-4 mb-4">
          <div className="h-[1px] w-8 bg-[#00dbe9]/30" />
          <span className="text-[11px] font-black tracking-[0.4em] text-[#00dbe9] uppercase">Onboarding Complete</span>
        </div>
        <h1 className="text-4xl sm:text-5xl font-black text-white tracking-tighter uppercase mb-6 leading-[0.9]">
            System 
            <span className="text-transparent bg-clip-text bg-gradient-to-r from-white via-white to-white/50"> Primed.</span>
        </h1>
        <p className="text-white/40 text-sm leading-relaxed max-w-md mx-auto sm:mx-0">
            Vox has been fully integrated into your system environment. All neural pathways are synchronized and ready for interaction.
        </p>
      </header>

      <div className="flex-1 flex flex-col items-center justify-center relative">
        <div className="relative">
            {/* Ambient Glow */}
            <motion.div 
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="absolute inset-0 bg-[#00dbe9] rounded-full blur-[80px] opacity-10"
            />
            
            <motion.div
                initial={{ scale: 0.8, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                transition={{ type: 'spring', bounce: 0.4 }}
                className="w-32 h-32 bg-zinc-950 border border-[#00dbe9]/30 rounded-[2.5rem] flex items-center justify-center text-[#00dbe9] shadow-[0_0_50px_rgba(0,219,233,0.1)] relative z-10"
            >
                <div className="absolute inset-0 bg-gradient-to-br from-[#00dbe9]/10 to-transparent rounded-[2.5rem]" />
                <ShieldCheck className="w-14 h-14 relative z-20" />
                
                <motion.div 
                    animate={{ scale: [1, 1.2, 1], opacity: [0.5, 1, 0.5] }}
                    transition={{ repeat: Infinity, duration: 3 }}
                    className="absolute -top-3 -right-3 text-[#d8baff] z-30"
                >
                    <Sparkles className="w-8 h-8" />
                </motion.div>
            </motion.div>
        </div>

        <div className="mt-12 grid grid-cols-2 gap-4 w-full max-w-sm">
            <div className="p-4 bg-white/[0.02] border border-white/5 rounded-2xl flex items-center gap-3">
                <Check className="w-3 h-3 text-[#00dbe9]" />
                <span className="text-[10px] font-black text-white/40 uppercase tracking-widest">STT Ready</span>
            </div>
            <div className="p-4 bg-white/[0.02] border border-white/5 rounded-2xl flex items-center gap-3">
                <Check className="w-3 h-3 text-[#00dbe9]" />
                <span className="text-[10px] font-black text-white/40 uppercase tracking-widest">VAD Ready</span>
            </div>
        </div>
      </div>

      {/* Tip Card */}
      <div className="mt-8 p-6 bg-zinc-950/50 border border-white/5 rounded-2xl relative overflow-hidden group">
          <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/5 to-transparent opacity-50" />
          <div className="flex items-center gap-2 mb-3 relative z-10">
              <Zap className="w-3 h-3 text-[#00dbe9]" />
              <span className="text-[10px] font-black text-[#00dbe9] uppercase tracking-[0.3em]">Neural Interface Tip</span>
          </div>
          <p className="text-[11px] text-white/40 leading-relaxed relative z-10">
            Access Vox instantly via your <span className="text-white font-bold">System Tray</span>. Tap your global hotkey to summon the HUD without interrupting your workflow.
          </p>
      </div>

      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.2 }}
        className="mt-8 pt-8 border-t border-white/5"
      >
        <button
          onClick={handleFinish}
          className="group relative w-full py-5 bg-zinc-950 text-white font-black rounded-2xl overflow-hidden border border-white/10 transition-all hover:bg-zinc-900 hover:border-[#00dbe9]/50 active:scale-[0.98] shadow-[0_0_40px_rgba(0,0,0,0.5)]"
        >
          <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
          <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.4em] text-[11px]">
            Launch Intelligence Engine
            <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1 text-[#00dbe9]" />
          </span>
        </button>
      </motion.div>
    </div>
  );
};
