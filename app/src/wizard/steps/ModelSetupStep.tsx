import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion } from 'framer-motion';
import { Package, Check, Loader2 } from 'lucide-react';

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
      const { model_id, progress: p } = event.payload;
      setProgress(prev => ({ ...prev, [model_id]: event.payload }));
      
      // If we got a status with 100% for all expected models, or a specific completion event
      // For now, we'll assume the backend sends a final event or we check progress.
      if (p === 100) {
        // Simple logic: if all models are 100, we're done.
        // In a real app, the manifest would tell us how many models.
      }
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

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-3xl font-bold text-white mb-2">Downloading Models</h2>
        <p className="text-neutral-400">Vox is fetching the latest neural models for STT, TTS, and VAD.</p>
      </div>

      <div className="p-8 bg-neutral-900 border border-neutral-800 rounded-3xl">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <Package className="h-5 w-5 text-indigo-400" />
            <span className="font-medium text-white text-lg">Overall Progress</span>
          </div>
          <span className="text-indigo-400 font-bold text-xl">{Math.round(totalProgress)}%</span>
        </div>
        
        <div className="h-3 bg-neutral-800 rounded-full overflow-hidden mb-6">
          <motion.div 
            className="h-full bg-indigo-500 shadow-[0_0_20px_rgba(99,102,241,0.5)]"
            initial={{ width: 0 }}
            animate={{ width: `${totalProgress}%` }}
          />
        </div>

        <div className="space-y-4">
          {Object.entries(progress).map(([id, p]) => (
            <div key={id} className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2">
                {p.progress === 100 ? <Check className="h-4 w-4 text-emerald-500" /> : <Loader2 className="h-4 w-4 text-indigo-500 animate-spin" />}
                <span className="text-neutral-300 uppercase tracking-widest text-[10px] font-bold">{id}</span>
              </div>
              <span className="text-neutral-500 font-mono">{(p.bytes_downloaded / 1024 / 1024).toFixed(1)} MB</span>
            </div>
          ))}
        </div>
      </div>

      <button
        disabled={!isComplete}
        onClick={onNext}
        className={`w-full py-4 font-bold rounded-2xl transition-all ${
          isComplete 
            ? 'bg-indigo-600 text-white hover:bg-indigo-500 shadow-lg shadow-indigo-500/20' 
            : 'bg-neutral-900 text-neutral-600 cursor-not-allowed'
        }`}
      >
        {isComplete ? 'Ready to Calibrate' : 'Downloading...'}
      </button>
    </div>
  );
};
