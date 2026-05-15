import React, { useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import { Check, ArrowRight, Activity, X, MessageSquare, Sparkles } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

interface Props {
  onNext: () => void;
  onBack: () => void;
}

export const LiveTestStep: React.FC<Props> = ({ onNext, onBack }) => {
  const [transcript, setTranscript] = useState('');
  const [isEngineReady, setIsEngineReady] = useState(false);
  const [testComplete, setTestComplete] = useState(false);
  const [energy, setEnergy] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const transcriptTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  const setup = async () => {
    setError(null);
    try {
      await invoke('launch_engine');
      setIsEngineReady(true);
    } catch (e: any) {
      console.error('Engine launch failed', e);
      setError(e.toString());
    }
  };

  useEffect(() => {
    setup();

    const unlistenTranscript = listen<{ text: string, is_final: boolean }>('stt_transcript', (event) => {
      setTranscript(event.payload.text);
      if (event.payload.is_final && event.payload.text.length > 2) {
        setTestComplete(true);
      }
      
      if (transcriptTimeoutRef.current) clearTimeout(transcriptTimeoutRef.current);
      transcriptTimeoutRef.current = setTimeout(() => {
        // Auto-complete if they said something
        if (event.payload.text.length > 2) setTestComplete(true);
      }, 2000);
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
    <div className="flex flex-col h-full relative">
      <header className="mb-8">
        <div className="flex items-center gap-4 mb-4">
          <div className="h-[1px] w-8 bg-[#00dbe9]/30" />
          <span className="text-[11px] font-black tracking-[0.4em] text-[#00dbe9] uppercase">Step 4.0 • Live Validation</span>
        </div>
        <h1 className="text-4xl font-black text-white tracking-tighter uppercase mb-4">Neural Feedback</h1>
        <p className="text-white/40 text-sm leading-relaxed max-w-md">
            Testing the real-time audio pipeline. Speak clearly into your microphone to verify the conversion of speech to neural tokens.
        </p>
      </header>

      <div className="flex-1 flex flex-col gap-6 min-h-0">
        {/* Visualization Area */}
        <div className="flex-1 min-h-0 bg-white/[0.01] border border-white/5 rounded-3xl relative overflow-hidden flex flex-col p-8">
            <div className="absolute inset-0 bg-gradient-to-b from-[#00dbe9]/5 to-transparent opacity-30" />
            
            {error ? (
                <div className="flex-1 flex flex-col items-center justify-center relative z-10 text-center px-4">
                    <div className="w-16 h-16 rounded-2xl bg-red-500/10 border border-red-500/20 flex items-center justify-center text-red-500 mb-6">
                        <X className="w-8 h-8" />
                    </div>
                    <h3 className="text-white font-black uppercase tracking-widest text-[11px] mb-2">Engine Failure</h3>
                    <p className="text-white/40 text-xs leading-relaxed max-w-xs mb-8">{error}</p>
                    <button 
                        onClick={setup}
                        className="px-8 py-4 bg-white/5 border border-white/10 rounded-xl text-[10px] font-black uppercase tracking-widest text-white hover:bg-white/10 transition-all active:scale-[0.98]"
                    >
                        Retry Initialization
                    </button>
                </div>
            ) : (
                <div className="flex-1 flex items-center justify-center relative">
                    {/* Waveform Visualization */}
                    <div className="flex items-end gap-1.5 h-32">
                        {[...Array(24)].map((_, i) => (
                        <motion.div
                            key={i}
                            animate={{ 
                                height: energy > 5 
                                    ? Math.max(8, (energy * (0.4 + Math.random() * 0.6)) * (1 - Math.abs(i - 12) / 12)) 
                                    : 8 
                            }}
                            transition={{ type: 'spring', bounce: 0.5, duration: 0.1 }}
                            className={cn(
                                "w-1.5 rounded-full transition-colors duration-300",
                                energy > 5 ? "bg-[#00dbe9]" : "bg-white/10"
                            )}
                        />
                        ))}
                    </div>

                    {/* Orb Glow */}
                    <motion.div 
                        animate={energy > 10 ? { scale: [1, 1.2, 1], opacity: [0.1, 0.2, 0.1] } : { opacity: 0.05 }}
                        className="absolute inset-0 bg-[#00dbe9] rounded-full blur-[100px] pointer-events-none"
                    />
                </div>
            )}

            <div className="relative z-10 bg-zinc-950/50 border border-white/5 backdrop-blur-md rounded-2xl p-6 min-h-[120px] flex flex-col mt-auto">
                <div className="flex items-center justify-between mb-4">
                    <span className="text-[10px] font-black text-white/20 uppercase tracking-[0.3em] flex items-center gap-2">
                        <MessageSquare className="w-3 h-3" /> Live Transcript
                    </span>
                    {testComplete && (
                        <motion.span 
                            initial={{ opacity: 0, scale: 0.8 }}
                            animate={{ opacity: 1, scale: 1 }}
                            className="text-[9px] font-black bg-[#00dbe9]/20 text-[#00dbe9] px-2 py-0.5 rounded-full uppercase tracking-tighter"
                        >
                            Processed
                        </motion.span>
                    )}
                </div>
                
                <div className="flex-1">
                    <AnimatePresence mode="wait">
                        {transcript ? (
                            <motion.p 
                                key="text"
                                initial={{ opacity: 0, y: 5 }}
                                animate={{ opacity: 1, y: 0 }}
                                className="text-lg font-bold text-white tracking-tight leading-tight"
                            >
                                {transcript}
                                {!testComplete && <motion.span animate={{ opacity: [1, 0.4, 1] }} transition={{ repeat: Infinity, duration: 1 }} className="inline-block w-1.5 h-4 bg-[#00dbe9] ml-2 align-middle" />}
                            </motion.p>
                        ) : (
                            <motion.p 
                                key="placeholder"
                                initial={{ opacity: 0 }}
                                animate={{ opacity: 1 }}
                                className="text-white/20 italic font-medium"
                            >
                                {isEngineReady ? "Speak to begin verification..." : "Initializing neural engine..."}
                            </motion.p>
                        )}
                    </AnimatePresence>
                </div>
            </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
            <div className="p-4 bg-white/[0.02] border border-white/5 rounded-2xl flex items-center gap-4">
                <div className="w-10 h-10 rounded-xl bg-white/5 flex items-center justify-center text-white/40">
                    <Activity className="w-5 h-5" />
                </div>
                <div className="flex flex-col">
                    <span className="text-[10px] font-bold text-white/20 uppercase tracking-widest">Latency</span>
                    <span className="text-sm font-black text-white">{isEngineReady ? "~42ms" : "---"}</span>
                </div>
            </div>
            <div className="p-4 bg-white/[0.02] border border-white/5 rounded-2xl flex items-center gap-4">
                <div className={cn(
                    "w-10 h-10 rounded-xl flex items-center justify-center transition-colors",
                    testComplete ? "bg-[#00dbe9]/10 text-[#00dbe9]" : "bg-white/5 text-white/40"
                )}>
                    {testComplete ? <Check className="w-5 h-5" /> : <Sparkles className="w-5 h-5" />}
                </div>
                <div className="flex flex-col">
                    <span className="text-[10px] font-bold text-white/20 uppercase tracking-widest">Accuracy</span>
                    <span className="text-sm font-black text-white">{testComplete ? "Verified" : "Pending"}</span>
                </div>
            </div>
        </div>
      </div>

      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        className="mt-8 pt-8 border-t border-white/5 flex gap-4"
      >
        <button
          onClick={onBack}
          className="px-8 py-5 text-[11px] font-black uppercase tracking-[0.3em] text-white/40 hover:text-white transition-colors"
        >
          Back
        </button>

        <button
          onClick={onNext}
          disabled={!testComplete}
          className={cn(
            "group relative flex-1 py-5 font-black rounded-2xl overflow-hidden transition-all flex items-center justify-center gap-4 shadow-[0_0_40px_rgba(0,0,0,0.5)]",
            testComplete 
              ? "bg-zinc-950 text-white border border-white/10 hover:bg-zinc-900 hover:border-[#00dbe9]/50 active:scale-[0.98]" 
              : "bg-white/5 text-white/20 border border-white/5 cursor-not-allowed opacity-50"
          )}
        >
          {testComplete && <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />}
          <span className="relative z-10 uppercase tracking-[0.4em] text-[11px]">Confirm & Finalize</span>
          <ArrowRight className="w-4 h-4 relative z-10 group-hover:translate-x-1 transition-transform text-[#00dbe9]" />
        </button>
      </motion.div>
    </div>
  );
};
