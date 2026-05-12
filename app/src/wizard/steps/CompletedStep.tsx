import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';
import { PartyPopper, Sparkles, ChevronRight } from 'lucide-react';

export const CompletedStep: React.FC = () => {
  const handleFinish = async () => {
    await invoke('complete_setup_wizard');
    window.location.reload(); // Refresh to trigger App.tsx routing logic
  };

  return (
    <div className="text-center py-8">
      <div className="relative inline-block mb-8">
        <motion.div
          animate={{ rotate: [0, 10, -10, 0] }}
          transition={{ repeat: Infinity, duration: 2 }}
          className="h-24 w-24 bg-indigo-500/10 rounded-full flex items-center justify-center text-indigo-400"
        >
          <PartyPopper className="h-12 w-12" />
        </motion.div>
        <div className="absolute -top-2 -right-2 text-yellow-400">
          <Sparkles className="h-6 w-6" />
        </div>
      </div>

      <h1 className="text-4xl font-bold text-white mb-4 tracking-tight">You're All Set!</h1>
      <p className="text-neutral-400 text-lg mb-12 max-w-sm mx-auto">
        Vox is now fully configured and ready to enhance your workflow.
      </p>

      <div className="space-y-4">
        <div className="p-6 bg-white/5 rounded-3xl border border-white/10 text-left">
          <p className="text-sm font-bold text-white mb-2">Quick Tip</p>
          <p className="text-sm text-neutral-400 leading-relaxed">
            You can summon Vox anytime using the global hotkey. Check the settings to customize your experience.
          </p>
        </div>

        <button
          onClick={handleFinish}
          className="w-full py-5 bg-white text-black font-bold rounded-2xl hover:bg-neutral-200 transition-all flex items-center justify-center gap-2"
        >
          Enter Dashboard
          <ChevronRight className="h-5 w-5" />
        </button>
      </div>
    </div>
  );
};
