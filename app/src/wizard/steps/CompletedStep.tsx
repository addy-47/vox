import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';
import { Sparkles, ChevronRight, Zap, ShieldCheck } from 'lucide-react';

export const CompletedStep: React.FC = () => {
  const handleFinish = async () => {
    await invoke('complete_setup_wizard');
    window.location.reload(); // Refresh to trigger App.tsx routing logic
  };

  return (
    <div className="flex flex-col gap-8 h-full">
      <header>
        <h2 className="text-3xl font-black text-white mb-2 tracking-tighter uppercase">INITIALIZATION COMPLETE</h2>
        <p className="text-white/80 text-sm font-light">System environment is fully configured.</p>
      </header>

      <div className="flex-1 flex flex-col items-center justify-center py-8">
        <div className="relative">
          <motion.div
            animate={{ 
              scale: [1, 1.1, 1],
              rotate: [0, 5, -5, 0] 
            }}
            transition={{ repeat: Infinity, duration: 6, ease: "easeInOut" }}
            className="w-32 h-32 bg-[#00dbe9]/10 rounded-[2.5rem] flex items-center justify-center text-[#00dbe9] border border-[#00dbe9]/20 shadow-[0_0_50px_rgba(0,219,233,0.1)] relative group"
          >
            <div className="absolute inset-0 bg-[#00dbe9]/10 blur-[40px] rounded-full opacity-50" />
            <ShieldCheck className="w-16 h-16 relative z-10" />
          </motion.div>
          <motion.div 
            animate={{ scale: [1, 1.5, 1], opacity: [0.5, 1, 0.5] }}
            transition={{ repeat: Infinity, duration: 2 }}
            className="absolute -top-4 -right-4 text-[#d8baff]"
          >
            <Sparkles className="w-10 h-10" />
          </motion.div>
        </div>

        <div className="mt-12 text-center">
            <h1 className="text-4xl font-black text-white mb-3 tracking-tighter uppercase">Ready for Interaction</h1>
            <p className="text-white/80 text-sm font-light max-w-[280px] mx-auto leading-relaxed">
                Your workspace is now equipped with local audio interaction capabilities.
            </p>
        </div>
      </div>

      {/* Tip Card */}
      <div className="p-6 bg-white/[0.02] border border-white/10 rounded-2xl relative overflow-hidden group">
          <div className="flex items-center gap-2 mb-3">
              <Zap className="w-3 h-3 text-[#00dbe9]" />
              <span className="text-[11px] font-black text-[#00dbe9] uppercase tracking-widest">Pro Tip</span>
          </div>
          <p className="text-[11px] text-white/80 leading-relaxed">
            Use <span className="text-white font-bold">Tray HUD</span> to interact instantly without breaking focus. Tap your global hotkey to begin.
          </p>
      </div>

      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
      >
        <button
          onClick={handleFinish}
          className="group relative w-full py-5 bg-[#0a0a0a] border border-white/10 text-white font-bold rounded-2xl overflow-hidden transition-all flex items-center justify-center gap-4 hover:bg-zinc-900 active:scale-[0.98] shadow-2xl"
        >
          <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/10 to-[#d8baff]/10 opacity-0 group-hover:opacity-100 transition-opacity" />
          <span className="relative z-10 uppercase tracking-[0.3em] text-[11px]">Launch Dashboard</span>
          <ChevronRight className="w-4 h-4 relative z-10 group-hover:translate-x-1 transition-transform" />
        </button>
      </motion.div>
    </div>
  );
};
