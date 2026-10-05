import React, { useCallback, useEffect, useState, useMemo } from 'react';
import { startModelSetup, fetchManifest, getRuntimeReport, type VoxManifest } from '@/services/setupService';
import { onModelProgress, type ModelProgressPayload } from '@/services/eventsService';
import { motion, AnimatePresence } from 'framer-motion';
import {
  AudioLines,
  AudioWaveform,
  BrainCircuit,
  Check, ArrowRight, Ear,
  Languages,
  Network
} from 'lucide-react';
import { cn } from '@/shared/lib/utils';
import { WIZARD_CTA_LABELS, WIZARD_STEP_HEADERS, MODEL_SETUP_COPY, MODEL_CATEGORY_META, MODEL_PROGRESS_STEPS } from '@/data/welcomeCopy';

import { WizardHeader } from '../components/WizardHeader';
import { WizardFooter } from '../components/WizardFooter';
import { ModelCategory } from '../components/ModelCategory';

const formatSize = (bytes: number) => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};

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
  const [progress, setProgress] = useState<Record<string, ModelProgressPayload>>({});
  const [isFetching, setIsFetching] = useState(false);
  const [internalError, setInternalError] = useState<string | null>(null);
  const [isFinished, setIsFinished] = useState(false);

  useEffect(() => {
    const fetchCatalog = async () => {
      setIsFetching(true);
      try {
        const data = await fetchManifest();
        setManifest(data);
        const required = data.model_groups
            .filter((g) => g.files.some((f) => f.required))
            .map((g) => g.id);
        setSelectedIds(new Set(required));
      } catch (e) {
        console.error('Failed to load model catalog', e);
        setInternalError(MODEL_SETUP_COPY.catalogLoadError);
      } finally {
        setIsFetching(false);
      }
    };
    fetchCatalog();

    getRuntimeReport().then((report) => {
        if (report.models_verified && !isAlreadyComplete) {
            setIsFinished(true);
            setView('complete');
        }
    }).catch(() => {});
  }, [isAlreadyComplete]);

  useEffect(() => {
    const unlisten = onModelProgress((p) => {
      setProgress(prev => ({ ...prev, [p.model_id]: p }));
      const normalizedStep = typeof p.step === 'string' ? p.step.toLowerCase() : '';
      if (normalizedStep === 'completed' || normalizedStep === 'complete') {
        setIsFinished(true);
      } else if (normalizedStep === 'failed' && p.error) {
        setInternalError(p.error);
        setView('catalog');
      }
    });

    return () => {
      unlisten();
    };
  }, []);

  const toggleCategory = useCallback((ids: string[]) => {
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
  }, []);

  const toggleModel = useCallback((id: string) => {
    setSelectedIds(prev => {
        const next = new Set(prev);
        if (next.has(id)) {
            next.delete(id);
        } else {
            next.add(id);
        }
        return next;
    });
  }, []);

  const startSetup = async () => {
    setView('progress');
    try {
      await startModelSetup(Array.from(selectedIds));
    } catch (e) {
      setInternalError(e instanceof Error ? e.message : String(e));
    }
  };

  const totalSize = useMemo(() => {
    if (!manifest || !manifest.model_groups) return 0;
    return manifest.model_groups
        .filter(g => selectedIds.has(g.id))
        .reduce((acc, g) => acc + g.files.reduce((sum, f) => sum + f.size, 0), 0);
  }, [manifest, selectedIds]);

  // Categories are derived from the manifest, not a hardcoded list: only
  // categories the manifest actually ships render, and `required` reflects
  // whether any file in the category is required (never a literal). This
  // previously rendered phantom "Mandatory" rows for categories with no
  // models, and labelled the (required) LLM download "Optional".
  // Labels/fallbacks live in `welcomeCopy` (MODEL_CATEGORY_META) so the
  // user-facing wording is written in one place; only the icons are bound here.
  const categories = useMemo(() => {
    if (!manifest || !manifest.model_groups) return [];

    const icons: Record<string, React.ReactElement<{ className?: string }>> = {
      vad: <AudioWaveform />,
      stt: <Ear />,
      translit: <Languages />,
      embedding: <Network />,
      llm: <BrainCircuit />,
      tts: <AudioLines />,
    };

    return MODEL_CATEGORY_META
      .map((m) => {
        const groups = manifest.model_groups.filter((g) => g.category === m.id);
        return {
          id: m.id,
          label: m.label,
          subLabel: groups.length > 0 ? groups.map((g) => g.name).join(" / ") : m.fallback,
          icon: icons[m.id],
          required: groups.some((g) => g.files.some((f) => f.required)),
          groups,
        };
      })
      .filter((c) => c.groups.length > 0);
  }, [manifest]);

  // Stable per-category toggle handlers: progress events must not re-create
  // props (and therefore re-render rows). Rebuilt only when the manifest's
  // category list changes, never on selection or download progress.
  const toggleByCategory = useMemo(() => {
    const handlers = new Map<string, () => void>();
    for (const cat of categories) {
      const ids = cat.groups.map((g) => g.id);
      handlers.set(cat.id, () => toggleCategory(ids));
    }
    return handlers;
  }, [categories, toggleCategory]);

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
                step={WIZARD_STEP_HEADERS.selection.step}
                title={WIZARD_STEP_HEADERS.selection.title}
                description={WIZARD_STEP_HEADERS.selection.description}
                rightContent={
                    <div className="flex items-center gap-2">
                        <div className="h-1.5 w-1.5 rounded-full bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.8)]" />
                        <span className="text-[13px] font-black text-[rgb(var(--accent))] tracking-widest">
                            {formatSize(totalSize)} {MODEL_SETUP_COPY.totalSuffix}
                        </span>
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
                        {MODEL_SETUP_COPY.retryLoad}
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
                            onToggle={toggleByCategory.get(cat.id) ?? (() => {})}
                            formatSize={formatSize}
                            selectedIds={selectedIds}
                            onToggleModel={toggleModel}
                        />
                    ))}
                </div>
            </div>

            <div className="mt-8 pt-8 border-t border-[rgba(var(--foreground),0.1)]">
                <p className="text-center text-[11px] text-[rgb(var(--foreground-muted))]/70 mb-4">
                    {MODEL_SETUP_COPY.changeLaterNote}
                </p>
                <div className="flex gap-4">
                    <button onClick={onBack} className="px-8 py-5 text-[12px] font-black uppercase tracking-[0.3em] text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] transition-colors">
                        {MODEL_SETUP_COPY.back}
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
                step={WIZARD_STEP_HEADERS.syncing.step}
                title={WIZARD_STEP_HEADERS.syncing.title}
                description={WIZARD_STEP_HEADERS.syncing.description}
                color="rgb(var(--accent))"
            />

            <div className="flex-1 space-y-4 overflow-y-auto pr-2 custom-scrollbar">
                {categories.filter(cat => cat.groups.some(g => selectedIds.has(g.id))).map(cat => {
                    const selectedGroups = cat.groups.filter(g => selectedIds.has(g.id));
                    const allFiles = selectedGroups.flatMap(g => g.files);
                    if (allFiles.length === 0) return null;

                    const groupProgress = allFiles.reduce((acc, m) => acc + (progress[m.id]?.progress || 0), 0) / allFiles.length;
                    const isDone = allFiles.every(m => progress[m.id]?.step === 'completed');
                    const rawStep = allFiles
                        .map(m => progress[m.id])
                        .find(p => p && p.step !== 'completed')?.step;
                    const activeStep = MODEL_PROGRESS_STEPS[
                        (rawStep ?? (isDone ? 'completed' : 'queued')).toLowerCase()
                    ] ?? MODEL_PROGRESS_STEPS.unknown;

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
                nextLabel={isFinished ? WIZARD_CTA_LABELS.continueToVerification : WIZARD_CTA_LABELS.synchronizing}
                isNextDisabled={!isFinished}
                showBack={true}
                error={internalError || externalError}
                errorLabel={MODEL_SETUP_COPY.downloadError}
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

                <h1 className="text-4xl font-display font-black text-[rgb(var(--foreground))] tracking-tighter uppercase mb-4">{MODEL_SETUP_COPY.readyTitle}</h1>
                <p className="text-[rgb(var(--foreground-muted))]/80 text-sm max-w-sm leading-relaxed mb-12">
                    {MODEL_SETUP_COPY.readyBody}
                </p>

                <div className="flex flex-col gap-4 w-full max-w-xs">
                    <button 
                        onClick={onNext}
                        className="group relative w-full py-5 text-[rgb(var(--foreground))] font-black rounded-2xl overflow-hidden border transition-all active:scale-[0.98] glass-card hover:border-[rgb(var(--accent))]/70"
                    >
                        <div className="absolute inset-0 bg-gradient-to-r from-[rgb(var(--accent))]/10 to-[rgba(var(--accent),0.03)] opacity-0 group-hover:opacity-100 transition-opacity" style={{ background: `linear-gradient(90deg, rgba(var(--accent), 0.1) 0%, rgba(var(--accent), 0.03) 100%)` }} />
                        <span className="relative z-10 flex items-center justify-center gap-3 tracking-widest uppercase text-xs">
                            {WIZARD_CTA_LABELS.continueSetup} <ArrowRight className="w-4 h-4 group-hover:translate-x-1 transition-transform" />
                        </span>
                    </button>
                    
                    <button 
                        onClick={() => setView('catalog')}
                        className="py-3 text-xs font-bold text-[rgb(var(--foreground-muted))]/70 uppercase tracking-widest hover:text-[rgb(var(--foreground))]/60 transition-colors"
                    >
                        {WIZARD_CTA_LABELS.returnToSelection}
                    </button>
                </div>
            </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
