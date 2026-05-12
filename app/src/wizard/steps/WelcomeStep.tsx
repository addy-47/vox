import React from 'react';
import { motion } from 'framer-motion';
import { ArrowRight, Mic2, ShieldCheck, Zap } from 'lucide-react';

interface Props {
  onNext: () => void;
}

export const WelcomeStep: React.FC<Props> = ({ onNext }) => {
  return (
    <div className="text-center">
      <motion.div 
        initial={{ scale: 0.8, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ delay: 0.2 }}
        className="mb-8 flex justify-center"
      >
        <div className="relative">
          <div className="absolute inset-0 bg-indigo-500 blur-2xl opacity-40 animate-pulse" />
          <div className="relative h-20 w-20 bg-neutral-900 border border-neutral-800 rounded-3xl flex items-center justify-center">
            <Mic2 className="h-10 w-10 text-indigo-500" />
          </div>
        </div>
      </motion.div>

      <h1 className="text-5xl font-bold tracking-tight text-white mb-4">
        Welcome to <span className="text-indigo-500">Vox</span>
      </h1>
      
      <p className="text-neutral-400 text-lg mb-12 max-w-md mx-auto leading-relaxed">
        Your local-first, privacy-focused voice assistant. 
        Let's get your system ready for real-time interaction.
      </p>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-12">
        <FeatureCard 
          icon={<ShieldCheck className="h-5 w-5 text-emerald-500" />}
          title="100% Local"
          desc="No audio ever leaves your device."
        />
        <FeatureCard 
          icon={<Zap className="h-5 w-5 text-amber-500" />}
          title="Real-time"
          desc="Sub-100ms latency inference."
        />
        <FeatureCard 
          icon={<Mic2 className="h-5 w-5 text-indigo-500" />}
          title="Voice UI"
          desc="Ephemeral, fluid interaction."
        />
      </div>

      <button
        onClick={onNext}
        className="group relative px-8 py-4 bg-white text-black font-semibold rounded-2xl overflow-hidden transition-all hover:pr-12 hover:bg-neutral-200 active:scale-95"
      >
        <span className="relative z-10 flex items-center gap-2">
          Start Setup
          <ArrowRight className="h-5 w-5 transition-transform group-hover:translate-x-1" />
        </span>
      </button>
    </div>
  );
};

const FeatureCard = ({ icon, title, desc }: { icon: React.ReactNode, title: string, desc: string }) => (
  <div className="p-4 bg-neutral-900/50 border border-neutral-800 rounded-2xl text-left backdrop-blur-sm">
    <div className="mb-3">{icon}</div>
    <h3 className="text-white font-medium mb-1">{title}</h3>
    <p className="text-neutral-500 text-sm leading-snug">{desc}</p>
  </div>
);
