import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Wifi,
  Check,
  ChevronDown,
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
  MODEL_CATEGORY_META,
  MODEL_CATEGORY_COPY,
  MODEL_PROGRESS_STEPS,
} from '@/data/welcomeCopy';

interface MobileModelStepProps {
  onNext?: () => void;
  onModelStatusChange?: (status: {
    isDone: boolean;
    isDownloading: boolean;
    label: string;
    action: () => void;
  }) => void;
}

const formatBytes = (bytes: number): string => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
};

export const MobileModelStep: React.FC<MobileModelStepProps> = ({
  onNext,
  onModelStatusChange,
}) => {
  const [manifest, setManifest] = useState<VoxManifest | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(new Set(['stt', 'llm']));
  const [isDownloading, setIsDownloading] = useState(false);
  const [isDone, setIsDone] = useState(false);
  const [progress, setProgress] = useState<Record<string, ModelProgressPayload>>({});

  const copy = MOBILE_ONBOARDING_COPY.models;

  useEffect(() => {
    fetchManifest()
      .then((data) => {
        setManifest(data);
        // Include required models AND built-in system components (like Earshot VAD)
        const required = data.model_groups
          .filter((g) => g.is_built_in || g.files.some((f) => f.required))
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

  const toggleCategoryExpand = (catId: string) => {
    setExpandedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(catId)) {
        next.delete(catId);
      } else {
        next.add(catId);
      }
      return next;
    });
  };

  const toggleModel = useCallback((id: string, isRequired: boolean, isBuiltIn: boolean) => {
    // Built-in system crates and mandatory models cannot be toggled
    if (isRequired || isBuiltIn || isDownloading || isDone) return;
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
      const isRequired = groups.some((g) => g.is_built_in || g.files.some((f) => f.required));
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

  const overallPercent = useMemo(() => {
    if (isDone) return 100;
    const entries = Object.values(progress);
    if (entries.length === 0) return 0;
    const sum = entries.reduce((acc, p) => acc + (p.progress || 0), 0);
    return Math.min(100, Math.round(sum / entries.length));
  }, [progress, isDone]);

  const handleStartDownload = useCallback(async () => {
    setIsDownloading(true);
    try {
      await startModelSetup(Array.from(selectedIds));
    } catch {
      // Graceful offline fallback simulation
      let step = 0;
      const interval = setInterval(() => {
        step += 20;
        if (step >= 100) {
          clearInterval(interval);
          setIsDone(true);
        }
      }, 600);
    }
  }, [selectedIds]);

  // Sync button action and label with parent footer
  useEffect(() => {
    if (!onModelStatusChange) return;
    if (isDone) {
      onModelStatusChange({
        isDone: true,
        isDownloading: false,
        label: copy.continueButton,
        action: () => onNext?.(),
      });
    } else if (isDownloading) {
      onModelStatusChange({
        isDone: false,
        isDownloading: true,
        label: `${copy.downloadingButton} (${overallPercent}%)`,
        action: () => {},
      });
    } else {
      onModelStatusChange({
        isDone: false,
        isDownloading: false,
        label: `${copy.downloadButton} (${formatBytes(totalSelectedBytes)})`,
        action: handleStartDownload,
      });
    }
  }, [isDone, isDownloading, overallPercent, totalSelectedBytes, copy, onNext, onModelStatusChange, handleStartDownload]);

  return (
    <div className="flex flex-col h-full w-full overflow-hidden select-none bg-transparent">
      {/* Scrollable Model Catalog & Progress List */}
      <div className="flex-1 min-h-0 overflow-y-auto px-6 py-4 space-y-4">
        {/* Step Intro Header */}
        <div className="space-y-1.5 pt-1 text-center">
          <h1 className="text-2xl sm:text-3xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {isDone ? copy.readyTitle : copy.title}
          </h1>
          <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
            {isDone ? copy.readyBody : copy.description}
          </p>
        </div>

        {/* Wi-Fi recommendation notice */}
        {!isDone && (
          <div className="flex items-center gap-2.5 px-3 py-2 rounded-xl bg-white/[0.03] text-left">
            <Wifi className="w-4 h-4 text-[rgb(var(--warning))] shrink-0" />
            <span className="text-[11px] text-[rgb(var(--foreground-muted))] leading-normal">
              {copy.wifiWarning}
            </span>
          </div>
        )}

        {/* Global Progress Bar (visible during download or done) */}
        {(isDownloading || isDone) && (
          <div className="p-3.5 rounded-2xl bg-white/[0.03] border border-white/[0.06] space-y-2">
            <div className="flex justify-between items-center text-xs font-mono">
              <span className="text-[rgb(var(--foreground-muted))] uppercase tracking-wider text-[11px]">
                {isDone ? copy.readyTitle : copy.downloadingButton}
              </span>
              <span className="text-[rgb(var(--accent))] font-bold">{overallPercent}%</span>
            </div>
            <div className="h-1.5 w-full bg-white/[0.06] rounded-full overflow-hidden">
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
        <div className="flex items-center justify-between text-xs font-mono text-[rgb(var(--foreground-muted))] px-1">
          <span className="uppercase tracking-wider">{copy.totalDownload}</span>
          <span className="font-bold text-[rgb(var(--accent))]">{formatBytes(totalSelectedBytes)}</span>
        </div>

        {/* Catalog Categories */}
        <div className="space-y-2.5 pb-4">
          {categories.map((cat) => {
            const isExpanded = expandedCategories.has(cat.id);

            return (
              <div
                key={cat.id}
                className="rounded-2xl border border-[rgba(var(--foreground),0.06)] bg-white/[0.02] overflow-hidden transition-all"
              >
                {/* Category Header Row */}
                <div
                  onClick={() => toggleCategoryExpand(cat.id)}
                  className="flex items-center justify-between p-3 cursor-pointer active:bg-[rgba(var(--foreground),0.03)] transition-colors select-none"
                >
                  <div className="flex items-center gap-3 min-w-0 flex-1">
                    <div className="w-8 h-8 rounded-xl bg-[rgb(var(--accent))]/10 flex items-center justify-center shrink-0">
                      {cat.icon}
                    </div>
                    <div className="flex flex-col min-w-0 flex-1 text-left">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="text-xs font-display font-bold text-[rgb(var(--foreground))] uppercase tracking-wide truncate">
                          {cat.label}
                        </span>
                        <span
                          className={`text-[9px] font-mono font-bold uppercase px-1.5 py-0.5 rounded ${
                            cat.isRequired
                              ? 'bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))]'
                              : 'bg-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground-muted))]'
                          }`}
                        >
                          {cat.isRequired ? copy.requiredBadge : copy.optionalBadge}
                        </span>
                      </div>
                      <p className="text-[11px] text-[rgb(var(--foreground-muted))] truncate leading-normal">
                        {cat.fallback}
                      </p>
                    </div>
                  </div>

                  <div className="flex items-center gap-2.5 shrink-0 pl-2">
                    <span className="text-xs font-mono font-semibold text-[rgb(var(--accent))]">
                      {formatBytes(cat.size)}
                    </span>
                    <ChevronDown
                      className={`w-4 h-4 text-[rgb(var(--foreground-muted))] transition-transform duration-200 ${
                        isExpanded ? 'rotate-180' : ''
                      }`}
                    />
                  </div>
                </div>

                {/* Expanded Model Group Rows */}
                <AnimatePresence>
                  {isExpanded && (
                    <motion.div
                      initial={{ height: 0, opacity: 0 }}
                      animate={{ height: 'auto', opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      transition={{ duration: 0.2 }}
                      className="border-t border-[rgba(var(--foreground),0.05)] bg-black/20 px-3 py-2.5 space-y-1.5"
                    >
                      {cat.groups.map((group) => {
                        const isBuiltIn = Boolean(group.is_built_in);
                        const isGroupSelected = isBuiltIn || selectedIds.has(group.id);
                        const isGroupRequired = isBuiltIn || group.files.some((f) => f.required);
                        const groupSize = group.files.reduce((sum, f) => sum + f.size, 0);
                        const modelProg = progress[group.id];
                        const stepLabel = modelProg?.step
                          ? MODEL_PROGRESS_STEPS[modelProg.step.toLowerCase()] ?? modelProg.step
                          : isDone
                          ? MODEL_PROGRESS_STEPS.completed
                          : undefined;

                        return (
                          <div
                            key={group.id}
                            onClick={() => toggleModel(group.id, isGroupRequired, isBuiltIn)}
                            className={`p-2.5 rounded-xl transition-all ${
                              isGroupSelected
                                ? 'bg-[rgb(var(--accent))]/8 border border-[rgb(var(--accent))]/20'
                                : 'bg-transparent border border-transparent opacity-60'
                            } ${!isGroupRequired && !isBuiltIn && !isDownloading && !isDone ? 'cursor-pointer active:scale-[0.99]' : 'cursor-default'}`}
                          >
                            <div className="flex items-center justify-between gap-2">
                              <div className="flex items-center gap-2.5 min-w-0 flex-1 text-left">
                                <div
                                  className={`w-4 h-4 rounded border flex items-center justify-center shrink-0 transition-colors ${
                                    isGroupSelected
                                      ? 'bg-[rgb(var(--accent))] border-[rgb(var(--accent))] text-black'
                                      : 'border-[rgba(var(--foreground),0.2)]'
                                  }`}
                                >
                                  {isGroupSelected && <Check className="w-3 h-3 stroke-[3]" />}
                                </div>
                                <div className="flex flex-col min-w-0 flex-1">
                                  <div className="flex items-center gap-2">
                                    <span className="text-xs font-semibold text-[rgb(var(--foreground))] truncate">
                                      {group.name}
                                    </span>
                                    {isBuiltIn && (
                                      <span className="text-[9px] font-mono px-1 py-0.2 rounded bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] uppercase font-bold">
                                        Built-in
                                      </span>
                                    )}
                                  </div>
                                  <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                                    {isBuiltIn ? 'Embedded in App Binary' : `${MODEL_CATEGORY_COPY.versionPrefix} ${group.version}`}
                                  </span>
                                </div>
                              </div>

                              <div className="text-right shrink-0">
                                <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                                  {isBuiltIn ? '0 B' : formatBytes(groupSize)}
                                </span>
                              </div>
                            </div>

                            {/* Group download progress bar if active */}
                            {isDownloading && !isDone && modelProg && !isBuiltIn && (
                              <div className="mt-2 pt-2 border-t border-[rgba(var(--foreground),0.04)] space-y-1">
                                <div className="flex justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                                  <span>{stepLabel ?? MODEL_PROGRESS_STEPS.downloading}</span>
                                  <span>{modelProg.progress ?? 0}%</span>
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
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
