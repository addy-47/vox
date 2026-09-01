import React, { useEffect, useState, useMemo } from 'react';
import { startModelSetup, fetchManifest, getRuntimeReport, type VoxManifest } from '@/services/modelService';
import { listen } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import { 
  Database, BrainCircuit, Mic, 
  Check, ArrowRight, Languages,
  Layers, ShieldCheck, Filter
} from 'lucide-react';
import { cn } from '@/shared/lib/utils';
import { WIZARD_CTA_LABELS } from '@/data/welcomeCopy';

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
        const data = await fetchManifest();
        setManifest(data);
        const required = data.model_groups
            .filter((g: any) => g.files.some((f: any) => f.required))
            .map((g: any) => g.id);
        setSelectedIds(new Set(required));
      } catch (e) {
        console.error('Failed to load model catalog', e);
        setInternalError('Failed to load model catalog.');
      } finally {
        setIsFetching(false);
      }
    };
    fetchCatalog();

    getRuntimeReport().then((report: any) => {
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
    const unlistenStatus = listen<ModelProgress>('model_progress', (event) => {
      const p = event.payload;
      setProgress(prev => ({ ...prev, [p.model_id]: p }));
      if (p.step === 'Complete' || p.step === 'Completed') {
        setIsFinished(true);
      } else if (p.step === 'Failed' && p.error) {
        setInternalError(p.error);
        setView('catalog');
      }
    });

    return () => {
      unlistenStatus.then(u => u());
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
      await startModelSetup(Array.from(selectedIds));
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

    const getSubLabel = (category: string, defaultFallback: string) => {
      const groups = manifest.model_groups.filter((g) => g.category === category);
      if (groups.length > 0) {
        return groups.map((g) => g.name).join(" / ");
      }
      return defaultFallback;
    };

    return [
      {
        id: "vad",
        label: "Speech Detection",
        subLabel: getSubLabel("vad", "Knows when you start and stop speaking"),
        icon: <Mic />,
        required: true,
        groups: manifest.model_groups.filter((g) => g.category === "vad"),
      },
      {
        id: "stt",
        label: "Speech to Text",
        subLabel: getSubLabel("stt", "Turns your speech into words"),
        icon: <Database />,
        required: true,
        groups: manifest.model_groups.filter((g) => g.category === "stt"),
      },
      {
        id: "translit",
        label: "Hindi & English Spelling",
        subLabel: getSubLabel("translit", "Writes spoken Hindi in English letters"),
        icon: <Languages />,
        required: true,
        groups: manifest.model_groups.filter((g) => g.category === "translit"),
      },
      {
        id: "embedding",
        label: "Memory Understanding",
        subLabel: getSubLabel("embedding", "Helps Vox connect related memories"),
        icon: <Layers />,
        required: true,
        groups: manifest.model_groups.filter((g) => g.category === "embedding"),
      },
      {
        id: "nli",
        label: "Memory Checking",
        subLabel: getSubLabel("nli", "Checks new memories against old ones"),
        icon: <ShieldCheck />,
        required: true,
        groups: manifest.model_groups.filter((g) => g.category === "nli"),
      },
      {
        id: "classifier",
        label: "Smart Sorting",
        subLabel: getSubLabel("classifier", "Keeps memories tidy and relevant"),
        icon: <Filter />,
        required: true,
        groups: manifest.model_groups.filter((g) => g.category === "classifier"),
      },
      {
        id: "llm",
        label: "Conversation Brain",
        subLabel: getSubLabel("llm", "Generates Vox's replies"),
        icon: <BrainCircuit />,
        required: false,
        groups: manifest.model_groups.filter((g) => g.category === "llm"),
      },
      {
        id: "tts",
        label: "Voice Generator",
        subLabel: getSubLabel("tts", "Speaks Vox's replies aloud"),
        icon: <VolumeIcon />,
        required: false,
        groups: manifest.model_groups.filter((g) => g.category === "tts"),
      },
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
                step="Step 3 of 6 · Choosing Voice Models"
                title="Choose Your Voice Models"
                description="Pick which voice features Vox uses. The essential ones make conversation work; the extra ones unlock smarter replies and memory."
                rightContent={
                    <div className="flex flex-col items-end">
                        <span className="text-[14px] font-bold text-[rgb(var(--foreground))]/80  tracking-tight mb-1">
                            {installPath}
                        </span>
                        <div className="flex items-center gap-2">
                            <div className="h-1.5 w-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.8)]" />
                            <span className="text-[13px] font-black text-[rgb(var(--accent))]  tracking-widest">
                                {formatSize(totalSize)} Total
                            </span>
                        </div>
                    </div>
                }
            />

            {internalError && (
                <div className="mx-2 mb-2 p-3 bg-red-500/10 border border-red-500/20 rounded-xl flex items-center justify-between">
                    <span className="text-red-400 text-xs font-bold">{internalError}</span>
                    <button 
                        type="button"
                        onClick={() => window.location.reload()} 
                        className="px-3 py-1 bg-red-500/20 hover:bg-red-500/30 text-red-300 rounded text-[11px] font-bold uppercase tracking-wider transition-all"
                    >
                        Retry Load
                    </button>
                </div>
            )}

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

            <div className="mt-8 pt-8 border-t border-[rgba(var(--foreground),0.1)]">
                <div className="flex gap-4">
                    <button onClick={onBack} className="px-8 py-5 text-[12px] font-black uppercase tracking-[0.3em] text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] transition-colors">
                        Back
                    </button>
                    <button 
                        onClick={startSetup}
                        disabled={isFetching || selectedIds.size === 0}
                        className="group relative flex-1 py-5 text-[rgb(var(--foreground))] font-black rounded-2xl overflow-hidden border transition-all active:scale-[0.98] glass-card hover:border-[rgb(var(--accent))]/70"
                    >
                        <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity" />
                        <span className="relative z-10 flex items-center justify-center gap-4 uppercase tracking-[0.4em] text-[12px]">
                            {isFetching ? WIZARD_CTA_LABELS.fetchingCatalog : WIZARD_CTA_LABELS.beginSynchronization}
                            <ArrowRight className="w-4 h-4 transition-transform group-hover:translate-x-1 text-[rgb(var(--accent))]" />
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
                step="Step 3 of 6 · Downloading Voice Models"
                title="Downloading Voice Models"
                description="Vox is downloading the voice models to your computer. They run locally, so your voice never leaves your device."
                color="rgb(var(--accent))"
            />

            <div className="flex-1 space-y-4 overflow-y-auto pr-2 custom-scrollbar">
                {categories.filter(cat => cat.groups.some(g => selectedIds.has(g.id))).map(cat => {
                    const selectedGroups = cat.groups.filter(g => selectedIds.has(g.id));
                    const allFiles = selectedGroups.flatMap(g => g.files);
                    if (allFiles.length === 0) return null;

                    const groupProgress = allFiles.reduce((acc, m) => acc + (progress[m.id]?.progress || 0), 0) / allFiles.length;
                    const isDone = allFiles.every(m => progress[m.id]?.step === 'completed');
                    const activeStep = allFiles
                        .map(m => progress[m.id])
                        .find(p => p && p.step !== 'completed')?.step || (isDone ? 'Ready' : 'Queued');

                    return (
                        <div key={cat.id} className="p-4 glass">
                            <div className="flex items-center justify-between mb-3">
                                <div className="flex items-center gap-3">
                                    <div className={cn(
                                        "p-2 rounded-lg transition-colors",
                                        isDone ? "bg-[rgb(var(--accent))]/20 text-[rgb(var(--accent))]" : "bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/60"
                                    )}>
                                        {cat.icon}
                                    </div>
                                    <div className="flex flex-col">
                                        <span className="text-[12px] font-black text-[rgb(var(--foreground))] uppercase tracking-wider">{cat.label}</span>
                                        <span className="text-[12px] text-[rgb(var(--accent))]/60 font-bold uppercase tracking-tighter">
                                            {activeStep}
                                        </span>
                                    </div>
                                </div>
                                <span className="text-[12px] font-mono text-[rgb(var(--foreground-muted))]/80">
                                    {Math.round(groupProgress)}%
                                </span>
                            </div>
                            <div className="h-1 bg-[rgba(var(--foreground),0.05)] rounded-full overflow-hidden mb-2">
                                <motion.div 
                                    className="h-full"
                  style={{ background: `linear-gradient(90deg, rgb(var(--accent)) 0%, rgba(var(--accent), 0.3) 100%)` }}
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
                nextLabel={isFinished ? "Continue" : "Downloading..."}
                isNextDisabled={!isFinished}
                showBack={true}
                error={internalError || externalError}
                errorLabel="Download Error"
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
                        className="absolute inset-0 bg-[rgb(var(--accent))] rounded-full blur-2xl opacity-20"
                    />
                    <div className="relative w-full h-full bg-[rgb(var(--accent))]/10 rounded-full border border-[rgb(var(--accent))]/30 flex items-center justify-center">
                        <Check className="w-10 h-10 text-[rgb(var(--accent))]" />
                    </div>
                </div>

                <h1 className="text-4xl font-display font-black text-[rgb(var(--foreground))] tracking-tighter uppercase mb-4">Models Ready</h1>
                <p className="text-[rgb(var(--foreground-muted))]/80 text-sm max-w-sm leading-relaxed mb-12">
                    All selected voice models have been downloaded and checked on your system.
                </p>

                <div className="flex flex-col gap-4 w-full max-w-xs">
                    <button 
                        onClick={onNext}
                        className="group relative w-full py-5 text-[rgb(var(--foreground))] font-black rounded-2xl overflow-hidden border transition-all active:scale-[0.98] glass-card hover:border-[rgb(var(--accent))]/70"
                    >
                        <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/10 to-[rgba(var(--accent),0.03)] opacity-0 group-hover:opacity-100 transition-opacity" style={{ background: `linear-gradient(90deg, rgba(var(--accent), 0.1) 0%, rgba(var(--accent), 0.03) 100%)` }} />
                        <span className="relative z-10 flex items-center justify-center gap-3 tracking-widest uppercase text-xs">
                            Continue Setup <ArrowRight className="w-4 h-4 group-hover:translate-x-1 transition-transform" />
                        </span>
                    </button>
                    
                    <button 
                        onClick={() => setView('catalog')}
                        className="py-3 text-xs font-bold text-[rgb(var(--foreground-muted))]/70 uppercase tracking-widest hover:text-[rgb(var(--foreground))]/60 transition-colors"
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
