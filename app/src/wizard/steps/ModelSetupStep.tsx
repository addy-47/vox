import React, { useEffect, useState, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import { 
  Database, BrainCircuit, Mic, 
  Check, ArrowRight, Languages
} from 'lucide-react';
import { cn } from '@/shared/lib/utils';

// --- Modular Components ---
import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';
import { ModelCategory } from '../components/ModelCategory';

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

interface ModelGroup {
  id: string;
  name: string;
  category: string;
  version: string;
  files: ModelEntry[];
}

interface VoxManifest {
  models_version: string;
  release_notes?: string[];
  total_size_bytes: number;
  model_groups: ModelGroup[];
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
  const [manifest, setManifest] = useState<VoxManifest | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [progress, setProgress] = useState<Record<string, ModelProgress>>({});
  const [isFetching, setIsFetching] = useState(false);
  const [internalError, setInternalError] = useState<string | null>(null);
  const [installPath, setInstallPath] = useState<string>('Detecting path...');
  const [isFinished, setIsFinished] = useState(false);

  useEffect(() => {
    const fetchCatalog = async () => {
      setIsFetching(true);
      try {
        const data = await invoke<VoxManifest>('fetch_manifest');
        setManifest(data);
        const required = data.model_groups
            .filter(g => g.files.some(f => f.required))
            .map(g => g.id);
        setSelectedIds(new Set(required));
      } catch (e) {
        console.error('Failed to load model catalog', e);
        setInternalError('Failed to load model catalog.');
      } finally {
        setIsFetching(false);
      }
    };
    fetchCatalog();

    invoke<any>('get_runtime_report').then(report => {
        if (report.models_verified && !isAlreadyComplete) {
            setIsFinished(true);
            setView('complete');
        }
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
      console.log('Model setup complete signal received');
      setIsFinished(true);
    });

    const unlistenError = listen<string>('model_setup_error', (event) => {
        setInternalError(event.payload);
        setView('catalog'); 
    });

    return () => {
      unlistenStatus.then(u => u());
      unlistenComplete.then(u => u());
      unlistenError.then(u => u());
    };
  }, []);

  const toggleCategory = (ids: string[]) => {
    setSelectedIds(prev => {
        const next = new Set(prev);
        const anyPresent = ids.some(id => next.has(id));
        if (anyPresent) {
            ids.forEach(id => next.delete(id));
        } else {
            ids.forEach(id => next.add(id));
        }
        return next;
    });
  };

  const toggleModel = (id: string) => {
    setSelectedIds(prev => {
        const next = new Set(prev);
        if (next.has(id)) {
            next.delete(id);
        } else {
            next.add(id);
        }
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
    if (!manifest || !manifest.model_groups) return 0;
    return manifest.model_groups
        .filter(g => selectedIds.has(g.id))
        .reduce((acc, g) => acc + g.files.reduce((sum, f) => sum + f.size, 0), 0);
  }, [manifest, selectedIds]);

  const formatSize = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const categories = useMemo(() => {
    if (!manifest || !manifest.model_groups) return [];
    
    return [
        {
            id: 'vad',
            label: 'Silence Detection (VAD)',
            subLabel: 'Ten-VAD / 10ms Window',
            icon: <Mic />,
            required: true,
            groups: manifest.model_groups.filter(g => g.category === 'vad')
        },
        {
            id: 'stt',
            label: 'Voice Understanding (ASR)',
            subLabel: 'Nemotron-3.5 ASR / Int8 Quant',
            icon: <Database />,
            required: true,
            groups: manifest.model_groups.filter(g => g.category === 'stt')
        },
        {
            id: 'translit',
            label: 'HI-EN Transliteration',
            subLabel: 'Deep learning based transliteration',
            icon: <Languages />,
            required: true,
            groups: manifest.model_groups.filter(g => g.category === 'translit')
        },
        {
            id: 'llm',
            label: 'Intelligence Layer (LLM)',
            subLabel: 'Gemma / Llama Reasoning',
            icon: <BrainCircuit />,
            required: false,
            groups: manifest.model_groups.filter(g => g.category === 'llm')
        },
        {
            id: 'tts',
            label: 'Speech Synthesis (TTS)',
            subLabel: 'Kokoro + Piper Multi-Voice',
            icon: <VolumeIcon />,
            required: false,
            groups: manifest.model_groups.filter(g => g.category === 'tts')
        }
    ];
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
            <WizardHeader 
                step="Step 2.1 • Selection"
                title="AI Components"
                description="Customize your local AI stack. Mandatory core ensures functional interaction, while optional layers unlock deep reasoning."
                rightContent={
                    <div className="flex flex-col items-end">
                        <span className="text-[13px] font-bold text-white/80  tracking-tight mb-1">
                            {installPath}
                        </span>
                        <div className="flex items-center gap-2">
                            <div className="h-1.5 w-1.5 rounded-full bg-[#00dbe9] shadow-[0_0_8px_rgba(0,219,233,0.8)]" />
                            <span className="text-[12px] font-black text-[#00dbe9]  tracking-widest">
                                {formatSize(totalSize)} Total
                            </span>
                        </div>
                    </div>
                }
            />

            <div className="flex-1 space-y-4 overflow-y-auto pr-2 custom-scrollbar -mx-2 px-2">
                <div className="grid gap-4 py-2">
                    {categories.map(cat => (
                        <ModelCategory 
                            key={cat.id}
                            id={cat.id}
                            label={cat.label}
                            subLabel={cat.subLabel}
                            icon={cat.icon}
                            groups={cat.groups}
                            selected={cat.groups.length > 0 && cat.groups.some(g => selectedIds.has(g.id))}
                            required={cat.required}
                            onToggle={() => toggleCategory(cat.groups.map(g => g.id))}
                            formatSize={formatSize}
                            selectedIds={selectedIds}
                            onToggleModel={toggleModel}
                        />
                    ))}
                </div>
            </div>

            <div className="mt-8 pt-8 border-t border-white/10">
                <div className="flex gap-4">
                    <button onClick={onBack} className="px-8 py-5 text-[11px] font-black uppercase tracking-[0.3em] text-white/40 hover:text-white transition-colors">
                        Back
                    </button>
                    <button 
                        onClick={startSetup}
                        disabled={isFetching || selectedIds.size === 0}
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
             <WizardHeader 
                step="Step 2.2 • Synchronizing"
                title="Deploying AI"
                description="Vox is deploying selected components to your local hardware. This process is fully encrypted and sandboxed."
                color="#d8baff"
            />

            <div className="flex-1 space-y-4 overflow-y-auto pr-2 custom-scrollbar">
                {categories.filter(cat => cat.groups.some(g => selectedIds.has(g.id))).map(cat => {
                    const selectedGroups = cat.groups.filter(g => selectedIds.has(g.id));
                    const allFiles = selectedGroups.flatMap(g => g.files);
                    if (allFiles.length === 0) return null;

                    const groupProgress = allFiles.reduce((acc, m) => acc + (progress[m.id]?.progress || 0), 0) / allFiles.length;
                    const isDone = allFiles.every(m => progress[m.id]?.step === 'Verified');
                    const activeStep = allFiles
                        .map(m => progress[m.id])
                        .find(p => p && p.step !== 'Verified')?.step || (isDone ? 'Ready' : 'Queued');

                    return (
                        <div key={cat.id} className="p-4 bg-white/[0.02] border border-white/5 rounded-xl">
                            <div className="flex items-center justify-between mb-3">
                                <div className="flex items-center gap-3">
                                    <div className={cn(
                                        "p-2 rounded-lg transition-colors",
                                        isDone ? "bg-[#00dbe9]/20 text-[#00dbe9]" : "bg-white/5 text-white/40"
                                    )}>
                                        {cat.icon}
                                    </div>
                                    <div className="flex flex-col">
                                        <span className="text-[11px] font-black text-white uppercase tracking-wider">{cat.label}</span>
                                        <span className="text-[10px] text-[#00dbe9]/60 font-bold uppercase tracking-tighter">
                                            {activeStep}
                                        </span>
                                    </div>
                                </div>
                                <span className="text-[11px] font-mono text-white/60">
                                    {Math.round(groupProgress)}%
                                </span>
                            </div>
                            <div className="h-1 bg-white/5 rounded-full overflow-hidden mb-2">
                                <motion.div 
                                    className="h-full bg-gradient-to-r from-[#00dbe9] to-[#d8baff]"
                                    initial={{ width: 0 }}
                                    animate={{ width: `${groupProgress}%` }}
                                    transition={{ duration: 0.3 }}
                                />
                            </div>
                        </div>
                    );
                })}
            </div>

            <WizardFooter 
                onBack={() => setView('catalog')}
                onNext={() => setView('complete')}
                nextLabel={isFinished ? "Continue to Verification" : "Synchronizing..."}
                isNextDisabled={!isFinished}
                showBack={true}
                error={internalError || externalError}
                errorLabel="Synchronization Error"
            />
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

                <h1 className="text-4xl font-black text-white tracking-tighter uppercase mb-4">Models Ready</h1>
                <p className="text-white/40 text-sm max-w-sm leading-relaxed mb-12">
                    All selected AI models have been successfully downloaded and verified on your system.
                </p>

                <div className="flex flex-col gap-4 w-full max-w-xs">
                    <button 
                        onClick={onNext}
                        className="group relative w-full py-5 bg-zinc-950 text-white font-black rounded-2xl overflow-hidden border border-white/10 transition-all hover:bg-zinc-900 hover:border-[#00dbe9]/50 active:scale-[0.98] shadow-[0_0_40px_rgba(0,0,0,0.5)]"
                    >
                        <div className="absolute inset-0 bg-gradient-to-r from-[#00dbe9]/10 to-[#d8baff]/10 opacity-0 group-hover:opacity-100 transition-opacity" />
                        <span className="relative z-10 flex items-center justify-center gap-3 tracking-widest uppercase text-xs">
                            Continue Setup <ArrowRight className="w-4 h-4 group-hover:translate-x-1 transition-transform" />
                        </span>
                    </button>
                    
                    <button 
                        onClick={() => setView('catalog')}
                        className="py-3 text-xs font-bold text-white/30 uppercase tracking-widest hover:text-white/60 transition-colors"
                    >
                        Return to Selection
                    </button>
                </div>
            </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
