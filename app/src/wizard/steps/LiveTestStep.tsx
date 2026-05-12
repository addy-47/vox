import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import { Mic, Check, ArrowRight, Activity } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

interface Props {
  onNext: () => void;
}

export const LiveTestStep: React.FC<Props> = ({ onNext }) => {
  const [transcript, setTranscript] = useState('');
  const [isEngineReady, setIsEngineReady] = useState(false);
  const [testComplete, setTestComplete] = useState(false);
  const [energy, setEnergy] = useState(0);

  useEffect(() => {
    const setup = async () => {
      try {
        await invoke('launch_engine');
        setIsEngineReady(true);
      } catch (e) {
        console.error('Engine launch failed', e);
      }
    };
    setup();

    const unlistenTranscript = listen<{ text: string, is_final: boolean }>('stt_transcript', (event) => {
      setTranscript(event.payload.text);
      if (event.payload.is_final && event.payload.text.length > 2) {
        setTestComplete(true);
      }
    });

    const unlistenEnergy = listen<number>('audio_energy', (event) => {
      setEnergy(event.payload * 100);
    });

    return () => {
      unlistenTranscript.then(u => u());
      unlistenEnergy.then(u => u());
      invoke('stop_engine').catch(console.error);
    };
  }, []);

  return (
    <div className="flex flex-col gap-8 h-full">
      <header>
        <h2 className="text-3xl font-black text-white mb-2 tracking-tighter uppercase">SYSTEM VERIFICATION</h2>
        <p className="text-white/80 text-sm font-light">Verifying end-to-end communication loop.</p>
      </header>

      {/* Waveform / Visualizer Area */}
      <div className="flex-1 flex flex-col items-center justify-center relative py-12">
        <div className="absolute inset-0 flex items-center justify-center">
            <motion.div 
              animate={{ 
                scale: [1, 1.2, 1],
                opacity: [0.05, 0.1, 0.05]
              }}
              transition={{ duration: 4, repeat: Infinity }}
              className="w-64 h-64 bg-[#00dbe9] rounded-full blur-[100px]" 
            />
        </div>

        <div className="relative z-10 flex flex-col items-center gap-8">
          <div className="flex items-end gap-1 h-24">
            {[...Array(12)].map((_, i) => (
              <motion.div
                key={i}
                animate={{ 
                  height: energy > 5 ? [10, Math.random() * 80 + 10, 10] : 10,
                  opacity: energy > 5 ? 1 : 0.2
                }}
                transition={{ duration: 0.15, repeat: Infinity }}
                className="w-1.5 bg-[#00dbe9] rounded-full"
              />
            ))}
          </div>

          <motion.div 
            className={cn(
              "w-20 h-20 rounded-full flex items-center justify-center transition-all duration-500",
              testComplete ? "bg-[#00dbe9]/10 border-[#00dbe9]/50 text-[#00dbe9] shadow-[0_0_30px_rgba(0,219,233,0.2)]" : "bg-white/5 text-[#00dbe9] border border-[#00dbe9]/20"
            )}
          >
            {testComplete ? <Check className="w-10 h-10" /> : <Mic className="w-10 h-10" />}
          </motion.div>
        </div>
      </div>

      {/* Transcript Card */}
      <div className="min-h-[140px] px-6 py-4 bg-white/[0.02] border border-white/10 rounded-2xl relative overflow-hidden group">
        <div className="absolute top-4 left-4 flex items-center gap-2">
            <Activity className="w-3 h-3 text-[#00dbe9]" />
            <span className="text-[11px] font-black text-white/80 uppercase tracking-widest">Signal Processing</span>
        </div>
        
        <div className="mt-8 flex items-center justify-center">
          <AnimatePresence mode="wait">
            {transcript ? (
              <motion.p
                key="transcript"
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                className="text-xl text-white font-medium italic tracking-tight text-center"
              >
                "{transcript}"
              </motion.p>
            ) : (
              <motion.p
                key="waiting"
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                className="text-[11px] text-white/80 font-bold uppercase tracking-[0.4em]"
              >
                {isEngineReady ? "Speak to begin" : "Initializing STT..."}
              </motion.p>
            )}
          </AnimatePresence>
        </div>
      </div>

      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
      >
        <button
          onClick={onNext}
          disabled={!testComplete}
          className={cn(
            "group relative w-full py-5 font-bold rounded-2xl overflow-hidden transition-all flex items-center justify-center gap-4 shadow-2xl",
            testComplete 
              ? "bg-[#0a0a0a] border border-white/10 text-white hover:bg-zinc-900 active:scale-[0.98]" 
              : "bg-white/5 text-white/40 border border-white/5 cursor-not-allowed"
          )}
        >
          {testComplete && <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/10 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />}
          <span className="relative z-10 uppercase tracking-[0.3em] text-[11px]">Complete Integration</span>
          <ArrowRight className="w-4 h-4 relative z-10 group-hover:translate-x-1 transition-transform" />
        </button>
      </motion.div>
    </div>
  );
};
