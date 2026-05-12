import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import { Mic, CheckCircle2 } from 'lucide-react';

interface Props {
  onNext: () => void;
}

export const LiveTestStep: React.FC<Props> = ({ onNext }) => {
  const [transcript, setTranscript] = useState('');
  const [isEngineReady, setIsEngineReady] = useState(false);
  const [testComplete, setTestComplete] = useState(false);

  useEffect(() => {
    const setup = async () => {
      try {
        // Launch engine in wizard mode
        await invoke('launch_engine');
        setIsEngineReady(true);
      } catch (e) {
        console.error('Engine launch failed', e);
      }
    };
    setup();

    const unlisten = listen<{ text: string, is_final: boolean }>('stt_transcript', (event) => {
      setTranscript(event.payload.text);
      if (event.payload.is_final && event.payload.text.length > 2) {
        setTestComplete(true);
      }
    });

    return () => {
      unlisten.then(u => u());
      invoke('stop_engine').catch(console.error);
    };
  }, []);

  return (
    <div className="space-y-8 text-center">
      <div>
        <h2 className="text-3xl font-bold text-white mb-2">Voice Test</h2>
        <p className="text-neutral-400">Let's verify everything is working. Say something!</p>
      </div>

      <div className="relative py-12">
        {/* Animated Rings */}
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          {[1, 2, 3].map(i => (
            <motion.div
              key={i}
              initial={{ scale: 0.5, opacity: 0 }}
              animate={{ scale: 2, opacity: 0 }}
              transition={{ 
                duration: 2, 
                repeat: Infinity, 
                delay: i * 0.6,
                ease: "easeOut"
              }}
              className="absolute w-32 h-32 border border-indigo-500/30 rounded-full"
            />
          ))}
        </div>

        <div className={`relative z-10 h-32 w-32 mx-auto rounded-full flex items-center justify-center transition-all duration-500 ${
          testComplete ? 'bg-emerald-500 text-white' : 'bg-neutral-900 border border-neutral-800 text-indigo-500'
        }`}>
          {testComplete ? <CheckCircle2 className="h-12 w-12" /> : <Mic className="h-12 w-12" />}
        </div>
      </div>

      <div className="min-h-[80px] px-6">
        <AnimatePresence mode="wait">
          {transcript ? (
            <motion.div
              initial={{ y: 10, opacity: 0 }}
              animate={{ y: 0, opacity: 1 }}
              className="text-xl text-white font-medium italic"
            >
              "{transcript}"
            </motion.div>
          ) : (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              className="text-neutral-500"
            >
              {isEngineReady ? 'Listening...' : 'Initializing Engine...'}
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      <div className="pt-4">
        <button
          onClick={onNext}
          disabled={!testComplete}
          className={`w-full py-4 font-bold rounded-2xl transition-all ${
            testComplete 
              ? 'bg-indigo-600 text-white hover:bg-indigo-500 shadow-xl shadow-indigo-500/20' 
              : 'bg-neutral-900 text-neutral-600 cursor-not-allowed'
          }`}
        >
          {testComplete ? 'Everything Looks Great!' : 'Speak to Continue'}
        </button>
      </div>
    </div>
  );
};
