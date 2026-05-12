import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion } from 'framer-motion';
import { Package, Check, Loader2, Database, BrainCircuit, Mic, Sparkles } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

interface ModelProgress {
  model_id: string;
  progress: number;
  step: string;
  bytes_downloaded: number;
  total_bytes: number;
}

interface Props {
  onNext: () => void;
}

export const ModelSetupStep: React.FC<Props> = ({ onNext }) => {
  const [progress, setProgress] = useState<Record<string, ModelProgress>>({});
  const [isComplete, setIsComplete] = useState(false);

  useEffect(() => {
    const setup = async () => {
      try {
        await invoke('start_model_setup');
      } catch (e) {
        console.error('Model setup failed to start', e);
      }
    };
    setup();

    const unlisten = listen<ModelProgress>('model_setup_status', (event) => {
      setProgress(prev => ({ ...prev, [event.payload.model_id]: event.payload }));
    });

    const unlistenComplete = listen<boolean>('model_setup_complete', () => {
      setIsComplete(true);
    });

    return () => {
      unlisten.then(u => u());
      unlistenComplete.then(u => u());
    };
  }, []);

  const totalProgress = Object.values(progress).length > 0
    ? Object.values(progress).reduce((acc, m) => acc + m.progress, 0) / Object.values(progress).length
    : 0;

  const getModelIcon = (id: string) => {
    if (id.includes('vad')) return <Mic className="w-4 h-4" />;
    if (id.includes('stt') || id.includes('asr')) return <Database className="w-4 h-4" />;
    if (id.includes('llm') || id.includes('gemma')) return <BrainCircuit className="w-4 h-4" />;
    return <Package className="w-4 h-4" />;
  };

  return (
    <div className="flex flex-col gap-8">
      <header>
        <h2 className="text-3xl font-black text-white mb-2 tracking-tighter uppercase">NEURAL ENGINE SYNC</h2>
        <p className="text-white/80 text-sm font-light">Synchronizing local neural models for system interaction.</p>
      </header>

      {/* Main Progress Card */}
      <div className="p-6 bg-white/[0.02] border border-white/10 rounded-2xl relative overflow-hidden group">
        <div className="absolute inset-0 bg-gradient-to-br from-[#00dbe9]/5 to-transparent opacity-50" />
        
        <div className="relative z-10">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-[#00dbe9]/10 rounded-lg">
                <Sparkles className="w-4 h-4 text-[#00dbe9]" />
              </div>
              <span className="text-[11px] font-bold text-white/80 tracking-widest uppercase">System Initialization</span>
            </div>
            <span className="text-2xl font-black text-white tracking-tighter">{Math.round(totalProgress)}%</span>
          </div>

          <div className="h-1.5 bg-white/5 rounded-full overflow-hidden mb-6 border border-white/5">
            <motion.div 
              className="h-full bg-gradient-to-r from-[#00dbe9] to-[#d8baff] shadow-[0_0_20px_rgba(0,219,233,0.3)]"
              initial={{ width: 0 }}
              animate={{ width: `${totalProgress}%` }}
              transition={{ duration: 0.5 }}
            />
          </div>

          {/* Model Grid */}
          <div className="grid grid-cols-2 gap-3">
            {Object.entries(progress).map(([id, p]) => (
              <div key={id} className="p-3 bg-white/5 rounded-xl border border-white/5 flex items-center justify-between group/item">
                <div className="flex items-center gap-3">
                  <div className={cn(
                    "w-7 h-7 rounded-lg flex items-center justify-center transition-colors",
                    p.progress === 100 ? "bg-[#00dbe9]/20 text-[#00dbe9]" : "bg-white/5 text-white/80"
                  )}>
                    {getModelIcon(id)}
                  </div>
                  <div className="flex flex-col">
                    <span className="text-[11px] font-bold text-white uppercase tracking-tight truncate w-24">{id}</span>
                    <span className="text-[11px] text-white/80 font-medium uppercase tracking-widest">
                      {p.progress === 100 ? 'Verified' : `${Math.round(p.progress)}%`}
                    </span>
                  </div>
                </div>
                {p.progress === 100 && (
                  <Check className="w-3 h-3 text-[#00dbe9]" />
                )}
              </div>
            ))}
            {/* Fillers if models are still loading */}
            {Object.keys(progress).length === 0 && (
              <div className="col-span-2 py-8 flex flex-col items-center justify-center gap-3 opacity-80">
                <Loader2 className="w-5 h-5 animate-spin" />
                <span className="text-[11px] font-bold uppercase tracking-[0.2em]">Contacting Registry...</span>
              </div>
            )}
          </div>
        </div>
      </div>

      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.2 }}
      >
        <button
          disabled={!isComplete}
          onClick={onNext}
          className={cn(
            "group relative w-full py-5 font-bold rounded-2xl overflow-hidden transition-all flex items-center justify-center gap-4",
            isComplete 
              ? "bg-[#0a0a0a] border border-white/10 text-white shadow-2xl hover:bg-zinc-900 active:scale-[0.98]" 
              : "bg-white/5 text-white/40 border border-white/5 cursor-not-allowed"
          )}
        >
          {isComplete && (
            <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/10 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
          )}
          <span className="relative z-10 uppercase tracking-[0.3em] text-[11px]">
            {isComplete ? "Initialize Audio Core" : "Synchronizing Weights"}
          </span>
          {isComplete ? (
            <Check className="w-4 h-4 relative z-10" />
          ) : (
            <Loader2 className="w-4 h-4 relative z-10 animate-spin" />
          )}
        </button>
      </motion.div>
    </div>
  );
};
