import React, { useEffect, useState, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import { 
  Check, Database, BrainCircuit, Mic, 
  ArrowRight, Box, HardDrive, AlertCircle
} from 'lucide-react';
import { cn } from '@/shared/lib/utils';

// --- Sub-components (Moved to top to prevent TDZ) ---

const VolumeIcon = ({ className }: { className?: string }) => (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
        <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
        <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
    </svg>
);

interface ModelEntry {
  id: string;
  path: string;
  size: number;
  required: boolean;
}

const ModelCard = ({ model, metadata, selected, onToggle, formatSize }: { 
    model: ModelEntry, 
    metadata: any, 
    selected: boolean, 
    onToggle: () => void,
    formatSize: (b: number) => string
}) => (
    <button 
        onClick={onToggle}
        className={cn(
            "w-full p-4 rounded-xl border transition-all duration-300 flex items-center justify-between group text-left",
            selected 
                ? "bg-white/[0.04] border-white/20 shadow-lg" 
                : "bg-white/[0.01] border-white/5 opacity-60 hover:opacity-100 hover:bg-white/[0.02]"
        )}
    >
        <div className="flex items-center gap-4">
            <div className={cn(
                "p-2.5 rounded-lg transition-colors",
                selected ? "bg-[#00dbe9]/10 text-[#00dbe9]" : "bg-white/5 text-white/40"
            )}>
                {metadata.icon}
            </div>
            <div className="flex flex-col">
                <div className="flex items-center gap-2 mb-0.5">
                    <span className="text-[11px] font-black text-white uppercase tracking-wider">{metadata.label}</span>
                    {model.required && (
                        <span className="text-[8px] font-bold bg-white/10 text-white/40 px-1.5 py-0.5 rounded uppercase tracking-tighter">Core</span>
                    )}
                </div>
                <p className="text-[10px] text-white/30 font-medium leading-tight max-w-[200px]">
                    {metadata.desc}
                </p>
            </div>
        </div>
        <div className="flex flex-col items-end gap-1.5">
            <span className="text-[10px] font-mono text-white/60">{formatSize(model.size)}</span>
            <div className={cn(
                "w-4 h-4 rounded border flex items-center justify-center transition-all",
                selected ? "bg-[#00dbe9] border-transparent" : "bg-transparent border-white/10"
            )}>
                {selected && <Check className="w-3 h-3 text-black stroke-[4]" />}
            </div>
        </div>
    </button>
);

// --- Main Step Component ---

interface Manifest {
  version: string;
  models: ModelEntry[];
}

interface ModelProgress {
  model_id: string;
  progress: number;
  step: string;
  bytes_downloaded: number;
  total_bytes: number;
  error?: string;
}

interface Props {
  onNext: () => void;
  onBack: () => void;
  error?: string;
  isAlreadyComplete?: boolean;
}

export const ModelSetupStep: React.FC<Props> = ({ onNext, onBack, error: externalError, isAlreadyComplete }) => {
  const [view, setView] = useState<'catalog' | 'progress' | 'complete'>(isAlreadyComplete ? 'complete' : 'catalog');
  const [manifest, setManifest] = useState<Manifest | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [progress, setProgress] = useState<Record<string, ModelProgress>>({});
  const [isFetching, setIsFetching] = useState(false);
  const [internalError, setInternalError] = useState<string | null>(null);
  const [installPath, setInstallPath] = useState<string>('~/.vox/models');

  // Human-readable labels for jargon reduction
  const modelInfo: Record<string, { label: string, desc: string, icon: React.ReactNode }> = {
    'vad': { label: 'Silence Detection', desc: 'Optimizes CPU by ignoring silence.', icon: <Mic className="w-4 h-4" /> },
    'stt': { label: 'Voice Understanding', desc: 'Converts your speech into text locally.', icon: <Database className="w-4 h-4" /> },
    'llm': { label: 'Intelligence Layer', desc: 'Advanced reasoning and tool orchestration.', icon: <BrainCircuit className="w-4 h-4" /> },
    'tts': { label: 'Speech Synthesis', desc: 'High-fidelity voice output for interactions.', icon: <VolumeIcon className="w-4 h-4" /> },
  };

  const getModelMetadata = (id: string) => {
    if (id.includes('vad')) return modelInfo['vad'];
    if (id.includes('stt') || id.includes('asr')) return modelInfo['stt'];
    if (id.includes('llm') || id.includes('gemma')) return modelInfo['llm'];
    if (id.includes('tts') || id.includes('piper') || id.includes('kokoro')) return modelInfo['tts'];
    return { label: 'System Asset', desc: 'Core component for Vox functionality.', icon: <Box className="w-4 h-4" /> };
  };

  useEffect(() => {
    const fetchCatalog = async () => {
      setIsFetching(true);
      try {
        // Correct IPC command: fetch_manifest returns VoxManifest
        const data = await invoke<Manifest>('fetch_manifest');
        setManifest(data);
        // Default select all required
        const required = data.models.filter(m => m.required).map(m => m.id);
        setSelectedIds(new Set(required));
      } catch (e) {
        console.error('Failed to load model catalog', e);
        setInternalError('Failed to load model catalog.');
      } finally {
        setIsFetching(false);
      }
    };
    fetchCatalog();

    // Correct handling of RuntimeReport object
    invoke<any>('get_runtime_report').then(report => {
        if (report.models_verified && !isAlreadyComplete) {
            setView('complete');
        }
        // Use the actual path from the backend
        if (report.models_dir) {
            setInstallPath(report.models_dir);
        }
    }).catch(() => {});
  }, [isAlreadyComplete]);

  useEffect(() => {
    const unlistenStatus = listen<ModelProgress>('model_setup_status', (event) => {
      setProgress(prev => ({ ...prev, [event.payload.model_id]: event.payload }));
    });

    const unlistenComplete = listen<boolean>('model_setup_complete', () => {
      setView('complete');
    });

    const unlistenError = listen<string>('model_setup_error', (event) => {
        setInternalError(event.payload);
    });

    return () => {
      unlistenStatus.then(u => u());
      unlistenComplete.then(u => u());
      unlistenError.then(u => u());
    };
  }, []);

  const toggleModel = (id: string, required: boolean) => {
    if (required) return;
    setSelectedIds(prev => {
        const next = new Set(prev);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
    });
  };

  const startSetup = async () => {
    setView('progress');
    try {
      await invoke('start_model_setup', { selectedIds: Array.from(selectedIds) });
    } catch (e) {
      setInternalError(e as string);
    }
  };

  const totalSize = useMemo(() => {
    if (!manifest || !manifest.models) return 0;
    return manifest.models
        .filter(m => selectedIds.has(m.id))
        .reduce((acc, m) => acc + m.size, 0);
  }, [manifest, selectedIds]);

  const formatSize = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const groupedModels = useMemo(() => {
    if (!manifest || !manifest.models) return { core: [], optional: [] };
    return {
        core: manifest.models.filter(m => m.required),
        optional: manifest.models.filter(m => !m.required)
    };
  }, [manifest]);

  return (
    <div className="flex flex-col h-full relative">
      <AnimatePresence mode="wait">
        {view === 'catalog' && (
          <motion.div 
            key="catalog"
            initial={{ opacity: 0, x: 20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -20 }}
            className="flex flex-col h-full"
          >
            <header className="mb-8">
                <div className="flex items-center gap-4 mb-4">
                  <div className="h-[1px] w-8 bg-[#00dbe9]/30" />
                  <span className="text-[11px] font-black tracking-[0.4em] text-[#00dbe9] uppercase">Step 2.1 • Selection</span>
                </div>
                <h1 className="text-4xl font-black text-white tracking-tighter uppercase mb-4">Neural Components</h1>
                <p className="text-white/40 text-sm leading-relaxed max-w-md">
                    Choose the intelligence layers you want to deploy locally. Core components are required for basic interaction.
                </p>
            </header>

            <div className="flex-1 space-y-6 overflow-y-auto pr-2 custom-scrollbar">
                {/* Core Section */}
                <section className="space-y-3">
                    <div className="flex items-center justify-between px-1">
                        <span className="text-[10px] font-bold text-white/30 uppercase tracking-widest">Mandatory Core</span>
                        <span className="text-[10px] font-mono text-white/20">{installPath}</span>
                    </div>
                    <div className="grid gap-2">
                        {groupedModels.core.map(m => (
                            <ModelCard 
                                key={m.id} 
                                model={m} 
                                metadata={getModelMetadata(m.id)} 
                                selected={true} 
                                onToggle={() => {}} 
                                formatSize={formatSize}
                            />
                        ))}
                    </div>
                </section>

                {/* Optional Section */}
                <section className="space-y-3">
                    <div className="flex items-center justify-between px-1">
                        <span className="text-[10px] font-bold text-white/30 uppercase tracking-widest">Optional Intelligence</span>
                    </div>
                    <div className="grid gap-2">
                        {groupedModels.optional.map(m => (
                            <ModelCard 
                                key={m.id} 
                                model={m} 
                                metadata={getModelMetadata(m.id)} 
                                selected={selectedIds.has(m.id)} 
                                onToggle={() => toggleModel(m.id, false)} 
                                formatSize={formatSize}
                            />
                        ))}
                    </div>
                </section>
            </div>

            <div className="mt-8 pt-8 border-t border-white/5 space-y-6">
                <div className="flex items-center justify-between px-2">
                    <div className="flex flex-col">
                        <span className="text-[10px] text-white/40 font-bold uppercase tracking-widest mb-1">Total Weight</span>
                        <span className="text-lg font-black text-white">{formatSize(totalSize)}</span>
                    </div>
                    <div className="flex items-center gap-2 text-[11px] font-medium text-white/40">
                        <HardDrive className="w-3 h-3" />
                        <span>Target: Local Disk</span>
                    </div>
                </div>

                <div className="flex gap-4">
                    <button onClick={onBack} className="px-8 py-5 text-[11px] font-black uppercase tracking-[0.3em] text-white/40 hover:text-white transition-colors">
                        Back
                    </button>
                    <button 
                        onClick={startSetup}
                        disabled={isFetching}
                        className="group relative flex-1 py-5 bg-zinc-950 text-white font-black rounded-2xl overflow-hidden border border-white/10 transition-all hover:bg-zinc-900 hover:border-[#00dbe9]/50 active:scale-[0.98] shadow-[0_0_40px_rgba(0,0,0,0.5)]"
                    >
                        <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
                        <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.4em] text-[11px]">
                            {isFetching ? 'Fetching Catalog...' : 'Begin Synchronization'}
                            <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1 text-[#00dbe9]" />
                        </span>
                    </button>
                </div>
            </div>
          </motion.div>
        )}

        {view === 'progress' && (
          <motion.div 
            key="progress"
            initial={{ opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 1.02 }}
            className="flex flex-col h-full"
          >
             <header className="mb-8">
                <div className="flex items-center gap-4 mb-4">
                  <div className="h-[1px] w-8 bg-[#d8baff]/30" />
                  <span className="text-[11px] font-black tracking-[0.4em] text-[#d8baff] uppercase">Step 2.2 • Synchronizing</span>
                </div>
                <h1 className="text-4xl font-black text-white tracking-tighter uppercase mb-4">Downloading Brain</h1>
                <p className="text-white/40 text-sm leading-relaxed max-w-md">
                    Vox is deploying selected components to your local hardware. This process is fully encrypted and sandboxed.
                </p>
            </header>

            <div className="flex-1 space-y-4 overflow-y-auto pr-2 custom-scrollbar">
                {Array.from(selectedIds).map(id => {
                    const p = progress[id];
                    const meta = getModelMetadata(id);
                    return (
                        <div key={id} className="p-4 bg-white/[0.02] border border-white/5 rounded-xl">
                            <div className="flex items-center justify-between mb-3">
                                <div className="flex items-center gap-3">
                                    <div className="p-2 bg-white/5 rounded-lg text-white/60">
                                        {meta.icon}
                                    </div>
                                    <div className="flex flex-col">
                                        <span className="text-[11px] font-black text-white uppercase tracking-wider">{meta.label}</span>
                                        <span className="text-[10px] text-white/30 font-bold uppercase tracking-tighter">
                                            {p?.step || 'Waiting...'}
                                        </span>
                                    </div>
                                </div>
                                <span className="text-[11px] font-mono text-white/60">
                                    {p ? `${Math.round(p.progress)}%` : '0%'}
                                </span>
                            </div>
                            <div className="h-1 bg-white/5 rounded-full overflow-hidden mb-2">
                                <motion.div 
                                    className="h-full bg-gradient-to-r from-[#00dbe9] to-[#d8baff]"
                                    initial={{ width: 0 }}
                                    animate={{ width: `${p?.progress || 0}%` }}
                                    transition={{ duration: 0.3 }}
                                />
                            </div>
                            <div className="flex justify-between text-[9px] font-bold text-white/20 uppercase tracking-widest">
                                <span>{p ? formatSize(p.bytes_downloaded) : '0 MB'}</span>
                                <span>{p ? formatSize(p.total_bytes) : '...'}</span>
                            </div>
                        </div>
                    );
                })}
            </div>

            {(internalError || externalError) && (
                <div className="mt-4 p-4 bg-red-500/10 border border-red-500/20 rounded-xl flex items-start gap-3">
                    <AlertCircle className="w-4 h-4 text-red-500 shrink-0 mt-0.5" />
                    <div className="space-y-1">
                        <span className="text-[10px] font-black text-red-500 uppercase tracking-widest">Synchronization Error</span>
                        <p className="text-[11px] text-red-400/80 leading-relaxed font-medium">
                            {internalError || externalError}
                        </p>
                    </div>
                </div>
            )}

            <div className="mt-8 pt-8 border-t border-white/5 flex gap-4">
                <button 
                    onClick={() => setView('catalog')}
                    className="px-8 py-5 text-[11px] font-black uppercase tracking-[0.3em] text-white/40 hover:text-white transition-colors"
                >
                    Cancel
                </button>
                <div className="flex-1 flex items-center justify-center bg-zinc-950/50 rounded-2xl border border-white/5">
                    <span className="text-[10px] font-black text-white/10 uppercase tracking-[0.4em]">Processing Engine Queue</span>
                </div>
            </div>
          </motion.div>
        )}

        {view === 'complete' && (
            <motion.div 
                key="complete"
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                className="flex flex-col items-center justify-center text-center h-full"
            >
                <div className="relative w-24 h-24 mb-12">
                    <motion.div 
                        initial={{ scale: 0 }}
                        animate={{ scale: 1 }}
                        className="absolute inset-0 bg-[#00dbe9] rounded-full blur-2xl opacity-20"
                    />
                    <div className="relative w-full h-full bg-[#00dbe9]/10 rounded-full border border-[#00dbe9]/30 flex items-center justify-center">
                        <Check className="w-10 h-10 text-[#00dbe9]" />
                    </div>
                </div>

                <h1 className="text-4xl font-black text-white tracking-tighter uppercase mb-4">Neural Link Active</h1>
                <p className="text-white/40 text-sm max-w-sm leading-relaxed mb-12">
                    All selected components have been successfully synchronized and verified. Your local brain is ready for interaction.
                </p>

                <button 
                    onClick={onNext}
                    className="group relative w-full max-w-xs py-5 bg-zinc-950 text-white font-black rounded-2xl overflow-hidden border border-white/10 transition-all hover:bg-zinc-900 hover:border-[#00dbe9]/50 active:scale-[0.98] shadow-[0_0_40px_rgba(0,0,0,0.5)]"
                >
                    <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
                    <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.4em] text-[11px]">
                        Initialize Engine
                        <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1 text-[#00dbe9]" />
                    </span>
                </button>
            </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
