import React, { useEffect, useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import { Check, Activity, X, MessageSquare, Sparkles } from 'lucide-react';
import { cn } from '@/shared/lib/utils';

// --- Modular Components ---
import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';

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
    setIsEngineReady(false);
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

    const unlistenPartial = listen<{ text: string, turn_id: number }>('transcript_partial', (event) => {
      setTranscript(event.payload.text);
      
      if (transcriptTimeoutRef.current) clearTimeout(transcriptTimeoutRef.current);
      transcriptTimeoutRef.current = setTimeout(() => {
        if (event.payload.text.length > 2) {
          setTestComplete(true);
        }
      }, 2000);
    });

    const unlistenFinal = listen<{ text: string, turn_id: number }>('transcript_final', (event) => {
      setTranscript(event.payload.text);
      if (event.payload.text.length > 2) {
        setTestComplete(true);
      }
    });

    let lastTime = 0;
    let localEnergy = 0;
    const THROTTLE_MS = 40; // High-refresh responsive updates

    const unlistenEnergy = listen<number>('audio_energy', (event) => {
      const e = event.payload;
      
      const targetEnergy = e * 100;
      localEnergy = localEnergy * 0.75 + targetEnergy * 0.25;
      
      const now = Date.now();
      if (now - lastTime >= THROTTLE_MS) {
        setEnergy(localEnergy);
        lastTime = now;
      }
    });

    return () => {
      unlistenPartial.then(u => u());
      unlistenFinal.then(u => u());
      unlistenEnergy.then(u => u());
      if (transcriptTimeoutRef.current) clearTimeout(transcriptTimeoutRef.current);
      invoke('stop_engine').catch(console.error);
    };
  }, []);

  return (
    <div className="flex flex-col h-full max-h-[100vh] overflow-hidden justify-between relative select-none">
      <WizardHeader 
        step="Step 4.0 • Voice Showcase"
        title="Voice Experience"
        description="Experience real-time local Voice Activity Detection (VAD) and Speech-to-Text (STT) understanding. Say something to watch the live local transcription."
      />

      <div className="flex-1 flex flex-col gap-4 min-h-0 overflow-hidden justify-center">
        {/* Reactive Flat Waveform Visualization Strip */}
        <div className="bg-white/[0.01] border border-white/5 rounded-2xl p-6 flex flex-col items-center justify-center relative overflow-hidden h-28 shrink-0">
          <div className="absolute inset-0 bg-gradient-to-b from-[#00dbe9]/5 to-transparent opacity-20 pointer-events-none" />
          
          {error ? (
              <div className="flex items-center gap-4 relative z-10 text-left">
                  <div className="w-10 h-10 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center justify-center text-red-500 shrink-0">
                      <X className="w-5 h-5" />
                  </div>
                  <div className="flex flex-col">
                      <h3 className="text-white font-black uppercase tracking-widest text-[9px]">Engine Failure</h3>
                      <button 
                          onClick={setup}
                          className="text-[9px] font-black uppercase tracking-widest text-[#00dbe9] hover:underline text-left mt-0.5"
                      >
                          Retry Initialization
                      </button>
                  </div>
              </div>
          ) : (
              <div className="flex flex-col items-center justify-center w-full h-full relative z-10">
                  <div className="flex items-center gap-1.5 h-8">
                    {/* Array of sleek vertical wave bars responding to energy */}
                    {Array.from({ length: 15 }).map((_, i) => {
                      const centerDist = Math.abs(i - 7);
                      const multiplier = Math.max(0.15, 1 - centerDist * 0.12);
                      const heightPercent = energy > 2 ? Math.min(100, Math.max(12, energy * 3.5 * multiplier)) : 12;
                      
                      return (
                        <motion.div
                          key={i}
                          animate={{ height: `${heightPercent}%` }}
                          transition={{ type: "spring", stiffness: 350, damping: 25 }}
                          className={cn(
                            "w-1 rounded-full transition-colors duration-300",
                            energy > 2 ? "bg-[#00dbe9] shadow-[0_0_10px_rgba(0,219,233,0.5)]" : "bg-white/10"
                          )}
                          style={{ minHeight: "3px" }}
                        />
                      );
                    })}
                  </div>
                  
                  <span className="text-[9px] font-black text-white/20 uppercase tracking-[0.3em] mt-4">
                    {isEngineReady ? (energy > 2 ? "Active speech feedback" : "Awaiting microphone input") : "Initializing audio pipeline"}
                  </span>
              </div>
          )}
        </div>

        {/* Live Transcript Display Box */}
        <div className={cn(
            "relative z-10 bg-zinc-950/40 border backdrop-blur-md rounded-2xl p-5 flex flex-col justify-center flex-1 min-h-[90px] max-h-[140px] transition-all duration-500",
            testComplete ? "border-emerald-500/20 shadow-[0_0_30px_rgba(16,185,129,0.04)]" : "border-white/5"
        )}>
            <div className="flex items-center justify-between mb-2">
                <span className="text-[10px] font-black text-white/20 uppercase tracking-[0.3em] flex items-center gap-2">
                    <MessageSquare className="w-3.5 h-3.5 text-[#00dbe9]/60" /> Live Transcript
                </span>
                {testComplete && (
                    <motion.span 
                        initial={{ opacity: 0, scale: 0.8 }}
                        animate={{ opacity: 1, scale: 1 }}
                        className="text-[9px] font-black bg-emerald-500/20 text-emerald-400 px-2 py-0.5 rounded-full uppercase tracking-tighter"
                    >
                        Processed
                    </motion.span>
                )}
            </div>
            
            <div className="flex-1 flex items-center min-h-0">
                <AnimatePresence mode="wait">
                    {transcript ? (
                        <motion.p 
                            key="text"
                            initial={{ opacity: 0, y: 3 }}
                            animate={{ opacity: 1, y: 0 }}
                            className="text-base font-bold text-white tracking-tight leading-snug overflow-y-auto max-h-full custom-scrollbar pr-1"
                        >
                            {transcript}
                            {!testComplete && <motion.span animate={{ opacity: [1, 0.4, 1] }} transition={{ repeat: Infinity, duration: 1 }} className="inline-block w-1.5 h-3.5 bg-[#00dbe9] ml-1.5 align-middle" />}
                        </motion.p>
                    ) : (
                         <motion.p 
                            key="placeholder"
                            initial={{ opacity: 0 }}
                            animate={{ opacity: 1 }}
                            className="text-white/20 italic font-medium text-sm"
                        >
                            {isEngineReady ? "Speak to preview live transcript..." : "Initializing local voice models..."}
                        </motion.p>
                    )}
                </AnimatePresence>
            </div>
        </div>

        {/* Diagnostics & Verification Cards */}
        <div className="grid grid-cols-2 gap-3 shrink-0">
            <div className="p-3 bg-white/[0.02] border border-white/5 rounded-xl flex items-center gap-3">
                <div className={cn(
                    "w-8 h-8 rounded-lg flex items-center justify-center transition-all duration-300 shrink-0",
                    energy > 2 ? "bg-[#00dbe9]/10 text-[#00dbe9] scale-105" : "bg-white/5 text-white/40"
                )}>
                    <Activity className="w-4 h-4" />
                </div>
                <div className="flex flex-col min-w-0">
                    <span className="text-[8px] font-bold text-white/20 uppercase tracking-widest truncate">Voice Activity (VAD)</span>
                    <span className="text-xs font-black text-white truncate">
                        {isEngineReady ? (energy > 2 ? "Speech Detected" : "Listening...") : "---"}
                    </span>
                </div>
            </div>
            <div className="p-3 bg-white/[0.02] border border-white/5 rounded-xl flex items-center gap-3">
                <div className={cn(
                    "w-8 h-8 rounded-lg flex items-center justify-center transition-all duration-300 shrink-0",
                    testComplete ? "bg-emerald-500/10 text-emerald-400 scale-105" : "bg-white/5 text-white/40"
                )}>
                    {testComplete ? <Check className="w-4 h-4" /> : <Sparkles className="w-4 h-4 animate-pulse" />}
                </div>
                <div className="flex flex-col min-w-0">
                    <span className="text-[8px] font-bold text-white/20 uppercase tracking-widest truncate">Speech-To-Text (STT)</span>
                    <span className="text-xs font-black text-white truncate">
                        {testComplete ? "Text Received" : "Waiting..."}
                    </span>
                </div>
            </div>
        </div>
      </div>

      <WizardFooter 
        onBack={onBack}
        onNext={onNext}
        onSkip={onNext}
        nextLabel="Confirm & Continue"
        isNextDisabled={!testComplete}
        showBack={true}
        showSkip={true}
        className="mt-4 shrink-0"
      />
    </div>
  );
};
