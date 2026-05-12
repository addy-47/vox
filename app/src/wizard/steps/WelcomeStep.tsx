import React from 'react';
import { motion } from 'framer-motion';
import { ArrowRight, ShieldCheck, Zap, Globe } from 'lucide-react';

interface Props {
  onNext: () => void;
}

export const WelcomeStep: React.FC<Props> = ({ onNext }) => {
  return (
    <div className="flex flex-col gap-10">
      <header>
        <motion.div 
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3 }}
          className="flex items-center gap-4 mb-4"
        >
          <div className="h-[1px] w-8 bg-[#00dbe9]/30" />
          <span className="text-[11px] font-black tracking-[0.4em] text-[#00dbe9] uppercase">Build 0.6.0-stable</span>
        </motion.div>

        <motion.h1 
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1, duration: 0.4 }}
          className="text-5xl font-black tracking-tighter text-white mb-6 leading-[0.9]"
        >
          INITIALIZE <br />
          <span className="text-transparent bg-clip-text bg-gradient-to-r from-white via-white to-white/50">VOX ENVIRONMENT.</span>
        </motion.h1>

        <motion.p 
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.15, duration: 0.4 }}
          className="text-white/40 text-sm leading-relaxed max-w-md"
        >
          Vox is a low-latency audio intelligence system designed to live in your system tray and provide real-time interaction.
        </motion.p>
      </header>

      <motion.div 
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.2, duration: 0.4 }}
        className="grid grid-cols-2 gap-4"
      >
        <FeatureCard 
          icon={<ShieldCheck className="w-4 h-4 text-[#00dbe9]" />}
          title="Privacy"
          desc="100% On-device"
        />
        <FeatureCard 
          icon={<Zap className="w-4 h-4 text-[#d8baff]" />}
          title="Latency"
          desc="Low-Latency Inference"
        />
        <FeatureCard 
          icon={<Globe className="w-4 h-4 text-white/50" />}
          title="Native"
          desc="System Integration"
        />
        <div className="p-5 bg-white/[0.03] border border-white/10 rounded-2xl flex flex-col justify-center gap-2">
          <div className="text-[11px] font-bold text-[#00dbe9] tracking-widest uppercase">Status</div>
          <div className="text-white font-medium text-sm">Awaiting Initialization</div>
        </div>
      </motion.div>

      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.25, duration: 0.4 }}
      >
        <button
          onClick={onNext}
          className="group relative w-full py-5 bg-[#0a0a0a] border border-white/10 text-white font-bold rounded-2xl overflow-hidden transition-all hover:bg-zinc-900 active:scale-[0.98] shadow-2xl hover:shadow-[#00dbe9]/10"
        >
          <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/10 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
          <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.3em] text-[11px]">
            Begin Environment Setup
            <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1" />
          </span>
        </button>
      </motion.div>
    </div>
  );
};

const FeatureCard = ({ icon, title, desc }: { icon: React.ReactNode, title: string, desc: string }) => (
  <div className="p-5 bg-white/[0.02] border border-white/5 rounded-2xl hover:bg-white/[0.04] transition-all hover:border-white/10 group">
    <div className="mb-3 w-8 h-8 rounded-lg bg-white/5 flex items-center justify-center group-hover:bg-white/10 transition-colors">
      {icon}
    </div>
    <div className="text-[11px] font-bold text-white/80 tracking-widest uppercase mb-1">{title}</div>
    <div className="text-white text-sm font-medium">{desc}</div>
  </div>
);
