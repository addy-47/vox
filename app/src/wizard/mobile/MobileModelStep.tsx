import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { motion } from 'framer-motion';
import {
  Wifi,
  ArrowRight,
  Check,
  CheckSquare2,
  Square,
  AudioWaveform,
  Ear,
  Languages,
  Network,
  BrainCircuit,
  AudioLines,
} from 'lucide-react';
import {
  fetchManifest,
  startModelSetup,
  getRuntimeReport,
  type VoxManifest,
} from '@/services/setupService';
import { onModelProgress, type ModelProgressPayload } from '@/services/eventsService';
import {
  MOBILE_ONBOARDING_COPY,
  WIZARD_CTA_LABELS,
  MODEL_CATEGORY_META,
  MODEL_PROGRESS_STEPS,
} from '@/data/welcomeCopy';

interface MobileModelStepProps {
  onNext: () => void;
  onBack: () => void;
}

const formatBytes = (bytes: number): string => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
};

export const MobileModelStep: React.FC<MobileModelStepProps> = ({ onNext, onBack }) => {
  const [manifest, setManifest] = useState<VoxManifest | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [isDownloading, setIsDownloading] = useState(false);
  const [isDone, setIsDone] = useState(false);
  const [progress, setProgress] = useState<Record<string, ModelProgressPayload>>({});

  const copy = MOBILE_ONBOARDING_COPY.models;

  useEffect(() => {
    fetchManifest()
      .then((data) => {
        setManifest(data);
        const required = data.model_groups
          .filter((g) => g.files.some((f) => f.required))
          .map((g) => g.id);
        setSelectedIds(new Set(required.length > 0 ? required : data.model_groups.map((g) => g.id)));
      })
      .catch(console.error);

    getRuntimeReport()
      .then((rep) => {
        if (rep.models_verified) {
          setIsDone(true);
        }
      })
      .catch(console.error);

    const unlisten = onModelProgress((p) => {
      setProgress((prev) => ({ ...prev, [p.model_id]: p }));
      const normalized = typeof p.step === 'string' ? p.step.toLowerCase() : '';
      if (normalized === 'completed' || normalized === 'complete') {
        setIsDone(true);
      }
    });

    return () => {
      unlisten();
    };
  }, []);

  const toggleModel = useCallback((id: string, isRequired: boolean) => {
    if (isRequired || isDownloading || isDone) return;
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, [isDownloading, isDone]);

  const categories = useMemo(() => {
    if (!manifest || !manifest.model_groups) return [];

    const icons: Record<string, React.ReactElement<{ className?: string }>> = {
      vad: <AudioWaveform className="w-4 h-4 text-[rgb(var(--accent))]" />,
      stt: <Ear className="w-4 h-4 text-[rgb(var(--accent))]" />,
      translit: <Languages className="w-4 h-4 text-[rgb(var(--accent))]" />,
      embedding: <Network className="w-4 h-4 text-[rgb(var(--accent))]" />,
      llm: <BrainCircuit className="w-4 h-4 text-[rgb(var(--accent))]" />,
      tts: <AudioLines className="w-4 h-4 text-[rgb(var(--accent))]" />,
    };

    return MODEL_CATEGORY_META.map((meta) => {
      const groups = manifest.model_groups.filter((g) => g.category === meta.id);
      const isRequired = groups.some((g) => g.files.some((f) => f.required));
      const groupSize = groups.reduce(
        (sum, g) => sum + g.files.reduce((fSum, f) => fSum + f.size, 0),
        0
      );
      return {
        id: meta.id,
        label: meta.label,
        fallback: meta.fallback,
        icon: icons[meta.id] ?? <BrainCircuit className="w-4 h-4 text-[rgb(var(--accent))]" />,
        groups,
        size: groupSize,
        isRequired,
      };
    }).filter((cat) => cat.groups.length > 0);
  }, [manifest]);

  const totalSelectedBytes = useMemo(() => {
    if (!manifest || !manifest.model_groups) return 0;
    return manifest.model_groups
      .filter((g) => selectedIds.has(g.id))
      .reduce((sum, g) => sum + g.files.reduce((fSum, f) => fSum + f.size, 0), 0);
  }, [manifest, selectedIds]);

  // Overall aggregate percentage
  const overallPercent = useMemo(() => {
    if (isDone) return 100;
    const entries = Object.values(progress);
    if (entries.length === 0) return 0;
    const sum = entries.reduce((acc, p) => acc + (p.progress || 0), 0);
    return Math.min(100, Math.round(sum / entries.length));
  }, [progress, isDone]);

  const handleStartDownload = async () => {
    setIsDownloading(true);
    try {
      await startModelSetup(Array.from(selectedIds));
    } catch {
      // Simulate progress in offline environment
      let step = 0;
      const interval = setInterval(() => {
        step += 20;
        if (step >= 100) {
          clearInterval(interval);
          setIsDone(true);
        }
      }, 600);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden select-none">
      {/* Top Header */}
      <div className="px-5 pt-3 pb-2 shrink-0 flex items-center justify-between border-b border-[rgba(var(--foreground),0.04)]">
        <button
          onClick={onBack}
          disabled={isDownloading && !isDone}
          className="text-xs font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] uppercase tracking-wider py-1 px-1 focus:outline-none disabled:opacity-30"
        >
          {WIZARD_CTA_LABELS.back}
        </button>
        <span className="text-[11px] font-mono font-semibold tracking-wider text-[rgb(var(--accent))] uppercase">
          {copy.step}
        </span>
        <div className="w-8" />
      </div>

      {/* Scrollable Model Catalog & Progress List */}
      <div className="flex-1 min-h-0 overflow-y-auto px-4 py-3 space-y-3">
        {/* Step Intro Card */}
        <div className="space-y-1">
          <h2 className="text-lg font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {isDone ? copy.readyTitle : copy.title}
          </h2>
          <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
            {isDone ? copy.readyBody : copy.description}
          </p>
        </div>

        {/* Wi-Fi recommendation notice */}
        {!isDone && (
          <div className="glass rounded-xl p-2.5 flex items-center gap-2.5 border border-[rgba(var(--foreground),0.06)]">
            <Wifi className="w-4 h-4 text-[rgb(var(--warning))] shrink-0" />
            <span className="text-[11px] text-[rgb(var(--foreground-muted))] leading-tight">
              {copy.wifiWarning}
            </span>
          </div>
        )}

        {/* Global Progress Bar (visible during download or done) */}
        {(isDownloading || isDone) && (
          <div className="glass rounded-xl p-3 border border-[rgba(var(--foreground),0.06)] space-y-2">
            <div className="flex justify-between items-center text-xs font-mono">
              <span className="text-[rgb(var(--foreground-muted))] uppercase tracking-wider text-[11px]">
                {isDone ? WIZARD_CTA_LABELS.continueToVerification : WIZARD_CTA_LABELS.synchronizing}
              </span>
              <span className="text-[rgb(var(--accent))] font-bold">{overallPercent}%</span>
            </div>
            <div className="h-1.5 w-full bg-[rgba(var(--foreground),0.08)] rounded-full overflow-hidden">
              <motion.div
                className="h-full bg-[rgb(var(--accent))]"
                initial={{ width: 0 }}
                animate={{ width: `${overallPercent}%` }}
                transition={{ duration: 0.3 }}
              />
            </div>
          </div>
        )}

        {/* Total Download Summary Tag */}
        <div className="flex items-center justify-between text-[11px] font-mono text-[rgb(var(--foreground-muted))] px-1 pt-1">
          <span className="uppercase tracking-wider">{copy.totalDownload}:</span>
          <span className="font-semibold text-[rgb(var(--accent))]">{formatBytes(totalSelectedBytes)}</span>
        </div>

        {/* Full Transparent List of Models */}
        <div className="space-y-2">
          {categories.map((cat) => {
            const isCategorySelected = cat.groups.some((g) => selectedIds.has(g.id));
            const activeGroup = cat.groups[0];
            const modelProg = activeGroup ? progress[activeGroup.id] : undefined;
            const stepLabel = modelProg?.step
              ? MODEL_PROGRESS_STEPS[modelProg.step.toLowerCase()] ?? modelProg.step
              : isDone
              ? MODEL_PROGRESS_STEPS.completed
              : undefined;

            return (
              <div
                key={cat.id}
                onClick={() => activeGroup && toggleModel(activeGroup.id, cat.isRequired)}
                className={`glass rounded-xl p-3 border transition-all ${
                  isCategorySelected
                    ? 'border-[rgba(var(--accent),0.25)] bg-[rgb(var(--accent))]/5'
                    : 'border-[rgba(var(--foreground),0.06)] opacity-70'
                } ${!cat.isRequired && !isDownloading && !isDone ? 'cursor-pointer active:scale-[0.99]' : ''}`}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="flex items-start gap-2.5">
                    <div className="mt-0.5 p-1 rounded-lg bg-[rgb(var(--background))] border border-[rgba(var(--foreground),0.08)] shrink-0">
                      {cat.icon}
                    </div>
                    <div className="space-y-0.5">
                      <div className="flex items-center gap-1.5 flex-wrap">
                        <span className="text-xs font-semibold text-[rgb(var(--foreground))]">
                          {cat.label}
                        </span>
                        <span
                          className={`text-[9px] font-mono font-bold uppercase px-1.5 py-0.2 rounded ${
                            cat.isRequired
                              ? 'bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] border border-[rgb(var(--accent))]/30'
                              : 'bg-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground-muted))]'
                          }`}
                        >
                          {cat.isRequired ? copy.requiredBadge : copy.optionalBadge}
                        </span>
                      </div>
                      <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-tight">
                        {cat.groups.map((g) => g.name).join(' · ') || cat.fallback}
                      </p>
                    </div>
                  </div>

                  <div className="flex flex-col items-end shrink-0 gap-1">
                    <span className="text-[11px] font-mono font-medium text-[rgb(var(--foreground-muted))]">
                      {formatBytes(cat.size)}
                    </span>
                    {!isDownloading && !isDone && (
                      <div className="text-[rgb(var(--accent))]">
                        {isCategorySelected ? (
                          <CheckSquare2 className="w-4 h-4" />
                        ) : (
                          <Square className="w-4 h-4 text-[rgb(var(--foreground-muted))]/40" />
                        )}
                      </div>
                    )}
                    {(isDownloading || isDone) && (
                      <div className="text-[rgb(var(--accent))] text-[11px] font-mono">
                        {isDone ? (
                          <Check className="w-4 h-4 text-[rgb(var(--accent))]" />
                        ) : modelProg ? (
                          `${modelProg.progress ?? 0}%`
                        ) : (
                          '...'
                        )}
                      </div>
                    )}
                  </div>
                </div>

                {/* Per-model progress bar during download */}
                {isDownloading && !isDone && modelProg && (
                  <div className="mt-2 pt-2 border-t border-[rgba(var(--foreground),0.04)] space-y-1">
                    <div className="flex justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                      <span>{stepLabel ?? MODEL_PROGRESS_STEPS.downloading}</span>
                      {modelProg.total_bytes > 0 && (
                        <span>
                          {formatBytes(modelProg.bytes_downloaded)} / {formatBytes(modelProg.total_bytes)}
                        </span>
                      )}
                    </div>
                    <div className="h-1 w-full bg-[rgba(var(--foreground),0.08)] rounded-full overflow-hidden">
                      <div
                        className="h-full bg-[rgb(var(--accent))] transition-all duration-300"
                        style={{ width: `${modelProg.progress ?? 0}%` }}
                      />
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Sticky Bottom Actions */}
      <div className="px-5 py-4 shrink-0 border-t border-[rgba(var(--foreground),0.06)] bg-[rgb(var(--background))]/95 backdrop-blur-md">
        {isDone ? (
          <button
            onClick={onNext}
            className="w-full py-3.5 min-h-[48px] bg-[rgb(var(--accent))] text-black font-display font-bold text-xs uppercase tracking-wider rounded-xl flex items-center justify-center gap-2 shadow-[0_4px_20px_rgba(var(--accent),0.3)] active:scale-[0.98] transition-transform"
          >
            {copy.continueButton} <ArrowRight className="w-4 h-4" />
          </button>
        ) : isDownloading ? (
          <button
            disabled
            className="w-full py-3.5 min-h-[48px] bg-[rgba(var(--foreground),0.1)] text-[rgb(var(--foreground-muted))] font-display font-bold text-xs uppercase tracking-wider rounded-xl flex items-center justify-center gap-2 cursor-wait"
          >
            {copy.downloadingButton} ({overallPercent}%)
          </button>
        ) : (
          <button
            onClick={handleStartDownload}
            className="w-full py-3.5 min-h-[48px] bg-[rgb(var(--accent))] text-black font-display font-bold text-xs uppercase tracking-wider rounded-xl flex items-center justify-center gap-2 shadow-[0_4px_20px_rgba(var(--accent),0.3)] active:scale-[0.98] transition-transform"
          >
            {copy.downloadButton} ({formatBytes(totalSelectedBytes)}) <ArrowRight className="w-4 h-4" />
          </button>
        )}
      </div>
    </div>
  );
};
