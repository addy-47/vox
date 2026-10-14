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
  AlertCircle,
  RotateCcw,
  Sparkles,
  Layers,
  Cpu,
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

type PresetType = 'minimal' | 'full' | 'custom';

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export const MobileModelStep: React.FC<MobileModelStepProps> = ({
  onNext,
  onModelStatusChange,
}) => {
  const [manifest, setManifest] = useState<VoxManifest | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [preset, setPreset] = useState<PresetType>('minimal');
  const [showCustomInspector, setShowCustomInspector] = useState(false);
  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(new Set(['stt', 'llm']));
  const [isDownloading, setIsDownloading] = useState(false);
  const [isDone, setIsDone] = useState(false);
  const [downloadError, setDownloadError] = useState<string | null>(null);
  const [progress, setProgress] = useState<Record<string, ModelProgressPayload>>({});

  const copy = MOBILE_ONBOARDING_COPY.models;

  // Dynamically derive required (coupled core) group IDs from manifest
  const requiredGroupIds = useMemo(() => {
    if (!manifest?.model_groups) return new Set<string>();
    return new Set(
      manifest.model_groups
        .filter((g) => g.is_built_in || g.required || g.files.some((f) => f.required))
        .map((g) => g.id)
    );
  }, [manifest]);

  // Dynamically derive full offline group IDs (core + local offline LLM + local offline TTS)
  const fullGroupIds = useMemo(() => {
    if (!manifest?.model_groups) return new Set<string>();
    const ids = new Set(
      manifest.model_groups
        .filter((g) => g.is_built_in || g.required || g.files.some((f) => f.required))
        .map((g) => g.id)
    );

    // Primary on-device LLM
    const localLlm = manifest.model_groups.find(
      (g) => g.category === 'llm' && !g.is_cloud && !g.is_remote
    );
    if (localLlm) ids.add(localLlm.id);

    // Primary on-device TTS
    const localTts = manifest.model_groups.find(
      (g) => g.category === 'tts' && !g.is_cloud && !g.is_remote && g.subcategory === 'main'
    );
    if (localTts) ids.add(localTts.id);

    return ids;
  }, [manifest]);

  // D5.4: Strictly sequence fetchManifest before getRuntimeReport to avoid manifest race
  useEffect(() => {
    let isMounted = true;

    fetchManifest()
      .then((data) => {
        if (!isMounted) return;
        setManifest(data);
        const required = data.model_groups
          .filter((g) => g.is_built_in || g.required || g.files.some((f) => f.required))
          .map((g) => g.id);
        const initialSelected = new Set(required.length > 0 ? required : data.model_groups.map((g) => g.id));
        setSelectedIds(initialSelected);
        setPreset('minimal');

        return getRuntimeReport();
      })
      .then((rep) => {
        if (!isMounted || !rep) return;
        if (rep.models_verified) {
          setIsDone(true);
        }
      })
      .catch((err) => {
        console.error('[MobileModelStep] Initialization failed:', err);
      });

    // D5.1 & D5.2: Track progress payloads per model file
    const unlisten = onModelProgress((p) => {
      setProgress((prev) => ({ ...prev, [p.model_id]: p }));
    });

    return () => {
      isMounted = false;
      unlisten();
    };
  }, []);

  const handleSelectPreset = useCallback(
    (newPreset: PresetType) => {
      if (isDownloading || isDone) return;
      setPreset(newPreset);
      if (newPreset === 'minimal') {
        setSelectedIds(new Set(requiredGroupIds));
      } else if (newPreset === 'full') {
        setSelectedIds(new Set(fullGroupIds));
      }
    },
    [isDownloading, isDone, requiredGroupIds, fullGroupIds]
  );

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

  const toggleModel = useCallback(
    (id: string, isRequired: boolean, isBuiltIn: boolean) => {
      if (isRequired || isBuiltIn || isDownloading || isDone) return;
      setPreset('custom');
      setSelectedIds((prev) => {
        const next = new Set(prev);
        if (next.has(id)) {
          next.delete(id);
        } else {
          next.add(id);
        }
        return next;
      });
    },
    [isDownloading, isDone]
  );

  // List of all non-built-in model files that must be downloaded
  const selectedFiles = useMemo(() => {
    if (!manifest || !manifest.model_groups) return [];
    return manifest.model_groups
      .filter((g) => !g.is_built_in && selectedIds.has(g.id))
      .flatMap((g) => g.files);
  }, [manifest, selectedIds]);

  // List of failed model files
  const failedFiles = useMemo(() => {
    return selectedFiles.filter((f) => {
      const p = progress[f.id];
      const step = p?.step?.toLowerCase();
      return step === 'failed' || step === 'cancelled';
    });
  }, [selectedFiles, progress]);

  // Dynamic category calculations
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
      const isRequired = groups.some((g) => g.is_built_in || g.required || g.files.some((f) => f.required));
      const selectedCategoryGroups = groups.filter((g) => g.is_built_in || selectedIds.has(g.id));
      const groupSize = selectedCategoryGroups.reduce(
        (sum, g) => sum + g.files.reduce((fSum, f) => fSum + f.size, 0),
        0
      );

      const catFiles = selectedCategoryGroups.flatMap((g) => g.files);
      const isCatDone =
        catFiles.length > 0 &&
        catFiles.every((f) => {
          const step = progress[f.id]?.step?.toLowerCase();
          return step === 'completed' || step === 'complete';
        });

      return {
        id: meta.id,
        label: meta.label,
        fallback: meta.fallback,
        icon: icons[meta.id] ?? <BrainCircuit className="w-4 h-4 text-[rgb(var(--accent))]" />,
        groups,
        size: groupSize,
        isRequired,
        isDone: isCatDone,
      };
    }).filter((cat) => cat.groups.length > 0);
  }, [manifest, selectedIds, progress]);

  // Preset size metrics
  const minimalSizeBytes = useMemo(() => {
    if (!manifest?.model_groups) return 0;
    return manifest.model_groups
      .filter((g) => requiredGroupIds.has(g.id))
      .reduce((sum, g) => sum + g.files.reduce((fSum, f) => fSum + f.size, 0), 0);
  }, [manifest, requiredGroupIds]);

  const fullSizeBytes = useMemo(() => {
    if (!manifest?.model_groups) return 0;
    return manifest.model_groups
      .filter((g) => fullGroupIds.has(g.id))
      .reduce((sum, g) => sum + g.files.reduce((fSum, f) => fSum + f.size, 0), 0);
  }, [manifest, fullGroupIds]);

  const totalSelectedBytes = useMemo(() => {
    if (!manifest || !manifest.model_groups) return 0;
    return manifest.model_groups
      .filter((g) => selectedIds.has(g.id))
      .reduce((sum, g) => sum + g.files.reduce((fSum, f) => fSum + f.size, 0), 0);
  }, [manifest, selectedIds]);

  // Truthful bytes downloaded calculation
  const downloadedBytes = useMemo(() => {
    return selectedFiles.reduce((sum, f) => {
      const p = progress[f.id];
      if (!p) return sum;
      const step = p.step?.toLowerCase();
      if (step === 'completed' || step === 'complete') {
        return sum + f.size;
      }
      return sum + (p.bytes_downloaded || (f.size * (p.progress || 0)) / 100);
    }, 0);
  }, [selectedFiles, progress]);

  const overallPercent = useMemo(() => {
    if (isDone) return 100;
    const total = selectedFiles.reduce((sum, f) => sum + f.size, 0);
    if (total === 0) return 0;
    return Math.min(100, Math.round((downloadedBytes / total) * 100));
  }, [isDone, selectedFiles, downloadedBytes]);

  // D5.1: isDone is strictly achieved when ALL selected files complete
  useEffect(() => {
    if (!isDownloading || isDone || selectedFiles.length === 0) return;
    const allCompleted = selectedFiles.every((f) => {
      const step = progress[f.id]?.step?.toLowerCase();
      return step === 'completed' || step === 'complete';
    });
    if (allCompleted) {
      setIsDone(true);
      setIsDownloading(false);
    } else if (
      selectedFiles.every((f) => {
        const step = progress[f.id]?.step?.toLowerCase();
        return step === 'completed' || step === 'complete' || step === 'failed' || step === 'cancelled';
      })
    ) {
      setIsDownloading(false);
    }
  }, [isDownloading, isDone, selectedFiles, progress]);

  // D5.3: Honest download triggering
  const handleStartDownload = useCallback(async () => {
    setDownloadError(null);
    setIsDownloading(true);
    try {
      await startModelSetup(Array.from(selectedIds));
    } catch (err) {
      console.error('[MobileModelStep] startModelSetup failed:', err);
      setIsDownloading(false);
      setDownloadError(err instanceof Error ? err.message : String(err));
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
    } else if (failedFiles.length > 0 || downloadError) {
      onModelStatusChange({
        isDone: false,
        isDownloading: false,
        label: 'Retry Download',
        action: handleStartDownload,
      });
    } else {
      onModelStatusChange({
        isDone: false,
        isDownloading: false,
        label: `${copy.downloadButton} (${formatBytes(totalSelectedBytes)})`,
        action: handleStartDownload,
      });
    }
  }, [
    isDone,
    isDownloading,
    overallPercent,
    totalSelectedBytes,
    failedFiles.length,
    downloadError,
    copy,
    onNext,
    onModelStatusChange,
    handleStartDownload,
  ]);

  return (
    <div className="flex flex-col h-full w-full overflow-hidden select-none bg-transparent">
      {/* Scrollable Stage with Centered Max-Width Container */}
      <div className="flex-1 min-h-0 overflow-y-auto px-6 py-4 space-y-4">
        {/* Step Intro Header (Centered) */}
        <div className="space-y-1.5 pt-1 text-center max-w-sm mx-auto">
          <h1 className="text-2xl sm:text-3xl font-display font-bold text-[rgb(var(--foreground))] tracking-tight">
            {isDone ? copy.readyTitle : copy.title}
          </h1>
          <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed max-w-xs mx-auto">
            {isDone ? copy.readyBody : copy.description}
          </p>
        </div>

        {/* Wi-Fi recommendation notice (Centered) */}
        {!isDone && (
          <div className="max-w-sm mx-auto flex items-center justify-center gap-2 px-3 py-1.5 rounded-full bg-white/[0.03] border border-white/[0.06] text-center">
            <Wifi className="w-3.5 h-3.5 text-[rgb(var(--warning))] shrink-0" />
            <span className="text-[11px] text-[rgb(var(--foreground-muted))] leading-tight">
              {copy.wifiWarning}
            </span>
          </div>
        )}

        {/* Surface download error or failed models */}
        {(downloadError || failedFiles.length > 0) && (
          <div className="max-w-sm mx-auto p-4 rounded-2xl bg-[rgb(var(--error))]/10 border border-[rgba(var(--error),0.2)] text-center space-y-2">
            <div className="flex items-center justify-center gap-2 text-xs font-semibold text-[rgb(var(--error))]">
              <AlertCircle className="w-4 h-4 shrink-0" />
              <span>
                {failedFiles.length > 0
                  ? `${failedFiles.length} model(s) didn't finish downloading`
                  : 'Download connection failed'}
              </span>
            </div>
            {downloadError && (
              <p className="text-[11px] font-mono text-[rgb(var(--error))]/80 break-words">
                {downloadError}
              </p>
            )}
            <button
              type="button"
              onClick={handleStartDownload}
              className="mx-auto min-h-[48px] px-4 py-2 rounded-xl bg-[rgb(var(--error))]/20 text-[rgb(var(--error))]/80 text-xs font-medium flex items-center justify-center gap-2 touch-manipulation cursor-pointer active:bg-[rgb(var(--error))]/30 transition-colors"
            >
              <RotateCcw className="w-3.5 h-3.5" />
              <span>Retry Download</span>
            </button>
          </div>
        )}

        {/* Global Progress Bar (visible during download or done) */}
        {(isDownloading || isDone) && (
          <div className="max-w-sm mx-auto p-3.5 rounded-2xl bg-white/[0.03] border border-white/[0.06] space-y-2">
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

        {/* ── Intelligence Stack Presets (Centered Cards) ── */}
        {!isDone && (
          <div className="max-w-sm mx-auto space-y-2.5">
            <div className="text-center pb-0.5">
              <span className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                {copy.presetsTitle}
              </span>
            </div>

            {/* Preset 1: Minimal Voice Core */}
            <div
              onClick={() => handleSelectPreset('minimal')}
              className={`p-3.5 rounded-2xl transition-all cursor-pointer touch-manipulation text-left border ${
                preset === 'minimal'
                  ? 'bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))]/40 shadow-[0_0_20px_rgba(var(--accent),0.12)]'
                  : 'bg-white/[0.02] border-white/[0.06] active:bg-white/[0.05]'
              }`}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-3">
                  <div
                    className={`w-5 h-5 rounded-full border flex items-center justify-center shrink-0 transition-colors ${
                      preset === 'minimal'
                        ? 'border-[rgb(var(--accent))] bg-[rgb(var(--accent))] text-black'
                        : 'border-[rgba(var(--foreground),0.25)]'
                    }`}
                  >
                    {preset === 'minimal' && <Check className="w-3.5 h-3.5 stroke-[3]" />}
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-display font-bold text-[rgb(var(--foreground))]">
                        {copy.presetMinimalTitle}
                      </span>
                      <span className="text-[9px] font-mono px-1.5 py-0.5 rounded bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] uppercase font-bold">
                        {copy.presetMinimalTag}
                      </span>
                    </div>
                    <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-semibold">
                      {formatBytes(minimalSizeBytes)}
                    </span>
                  </div>
                </div>
                <Cpu className="w-4 h-4 text-[rgb(var(--foreground-muted))] shrink-0 mt-0.5" />
              </div>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed mt-2 pl-8">
                {copy.presetMinimalDesc}
              </p>
            </div>

            {/* Preset 2: Full Offline Assistant */}
            <div
              onClick={() => handleSelectPreset('full')}
              className={`p-3.5 rounded-2xl transition-all cursor-pointer touch-manipulation text-left border ${
                preset === 'full'
                  ? 'bg-[rgb(var(--success))]/10 border-[rgb(var(--success))]/40 shadow-[0_0_20px_rgba(var(--success),0.12)]'
                  : 'bg-white/[0.02] border-white/[0.06] active:bg-white/[0.05]'
              }`}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-3">
                  <div
                    className={`w-5 h-5 rounded-full border flex items-center justify-center shrink-0 transition-colors ${
                      preset === 'full'
                        ? 'border-[rgb(var(--success))] bg-[rgb(var(--success))] text-black'
                        : 'border-[rgba(var(--foreground),0.25)]'
                    }`}
                  >
                    {preset === 'full' && <Check className="w-3.5 h-3.5 stroke-[3]" />}
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-display font-bold text-[rgb(var(--foreground))]">
                        {copy.presetFullTitle}
                      </span>
                      <span className="text-[9px] font-mono px-1.5 py-0.5 rounded bg-[rgb(var(--success))]/15 text-[rgb(var(--success))] uppercase font-bold">
                        {copy.presetFullTag}
                      </span>
                    </div>
                    <span className="text-[11px] font-mono text-[rgb(var(--success))] font-semibold">
                      {formatBytes(fullSizeBytes)}
                    </span>
                  </div>
                </div>
                <Sparkles className="w-4 h-4 text-[rgb(var(--success))]/80 shrink-0 mt-0.5" />
              </div>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed mt-2 pl-8">
                {copy.presetFullDesc}
              </p>
            </div>
          </div>
        )}

        {/* Total Selection Footnote */}
        <div className="max-w-sm mx-auto flex items-center justify-between text-xs font-mono text-[rgb(var(--foreground-muted))] px-1">
          <span className="uppercase tracking-wider">{copy.totalDownload}</span>
          <span className="font-bold text-[rgb(var(--accent))]">{formatBytes(totalSelectedBytes)}</span>
        </div>

        {/* ── Custom Inspector Toggle (Centered) ── */}
        {!isDone && (
          <div className="max-w-sm mx-auto text-center pt-1">
            <button
              type="button"
              onClick={() => setShowCustomInspector((v) => !v)}
              className="mx-auto min-h-[48px] px-4 py-2 flex items-center justify-center gap-2 rounded-xl text-xs font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] bg-white/[0.02] border border-white/[0.06] active:bg-white/[0.05] transition-colors touch-manipulation cursor-pointer"
            >
              <Layers className="w-3.5 h-3.5 text-[rgb(var(--accent))]" />
              <span>{copy.customInspectorToggle}</span>
              <ChevronDown
                className={`w-3.5 h-3.5 transition-transform duration-200 ${
                  showCustomInspector ? 'rotate-180' : ''
                }`}
              />
            </button>
          </div>
        )}

        {/* ── Expandable Individual Model Catalog ── */}
        <AnimatePresence>
          {showCustomInspector && !isDone && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: 'auto', opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.25 }}
              className="max-w-sm mx-auto space-y-2.5 pt-2 pb-4 overflow-hidden"
            >
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
                      className="flex items-center justify-between p-3 min-h-[48px] cursor-pointer active:bg-[rgba(var(--foreground),0.03)] transition-colors select-none touch-manipulation"
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
                            const isCloud = Boolean(group.is_cloud);
                            const isRemote = Boolean(group.is_remote);
                            const isGroupSelected = isBuiltIn || selectedIds.has(group.id);
                            const isGroupRequired = isBuiltIn || Boolean(group.required) || group.files.some((f) => f.required);
                            const groupSize = group.files.reduce((sum, f) => sum + f.size, 0);

                            const firstFile = group.files[0];
                            const modelProg = firstFile ? progress[firstFile.id] : undefined;
                            const isGroupFailed = group.files.some((f) => {
                              const s = progress[f.id]?.step?.toLowerCase();
                              return s === 'failed' || s === 'cancelled';
                            });
                            const isGroupDone = group.files.length > 0 && group.files.every((f) => {
                              const s = progress[f.id]?.step?.toLowerCase();
                              return s === 'completed' || s === 'complete';
                            });

                            const stepLabel = isGroupFailed
                              ? MODEL_PROGRESS_STEPS.failed
                              : isGroupDone
                              ? MODEL_PROGRESS_STEPS.completed
                              : modelProg?.step
                              ? MODEL_PROGRESS_STEPS[modelProg.step.toLowerCase()] ?? modelProg.step
                              : undefined;

                            return (
                              <div
                                key={group.id}
                                onClick={() => toggleModel(group.id, isGroupRequired, isBuiltIn)}
                                className={`p-2.5 min-h-[48px] rounded-xl transition-all touch-manipulation ${
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
                                      <div className="flex items-center gap-2 flex-wrap">
                                        <span className="text-xs font-semibold text-[rgb(var(--foreground))] truncate">
                                          {group.name}
                                        </span>
                                        {isBuiltIn && (
                                          <span className="text-[9px] font-mono px-1 py-0.2 rounded bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] uppercase font-bold">
                                            {copy.builtInBadge}
                                          </span>
                                        )}
                                        {isCloud && (
                                          <span className="text-[9px] font-mono px-1 py-0.2 rounded bg-[rgb(var(--info))]/15 text-[rgb(var(--info))] uppercase font-bold">
                                            {copy.cloudBadge}
                                          </span>
                                        )}
                                        {isRemote && (
                                          <span className="text-[9px] font-mono px-1 py-0.2 rounded bg-[rgb(var(--violet))]/15 text-[rgb(var(--violet))] uppercase font-bold">
                                            {copy.remoteBadge}
                                          </span>
                                        )}
                                        {isGroupRequired && !isBuiltIn && (
                                          <span className="text-[9px] font-mono px-1 py-0.2 rounded bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] uppercase font-bold">
                                            {copy.coreStackBadge}
                                          </span>
                                        )}
                                      </div>
                                      <span className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                                        {isBuiltIn
                                          ? 'Embedded in App Binary'
                                          : isCloud
                                          ? 'Zero Local Download'
                                          : `${MODEL_CATEGORY_COPY.versionPrefix} ${group.version}`}
                                      </span>
                                      {group.description && (
                                        <span className="text-[10px] text-[rgb(var(--foreground-muted))]/80 line-clamp-1 mt-0.5">
                                          {group.description}
                                        </span>
                                      )}
                                      <div className="flex items-center gap-1.5 mt-1 flex-wrap">
                                        {group.parameters && (
                                          <span className="text-[9px] font-mono px-1 py-0.2 rounded bg-white/[0.04] text-[rgb(var(--foreground-muted))]">
                                            {group.parameters}
                                          </span>
                                        )}
                                        {group.ram_usage && (
                                          <span className="text-[9px] font-mono px-1 py-0.2 rounded bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))]">
                                            RAM: {group.ram_usage}
                                          </span>
                                        )}
                                      </div>
                                    </div>
                                  </div>

                                  <div className="text-right shrink-0">
                                    <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]">
                                      {isBuiltIn || isCloud || isRemote ? '0 B' : formatBytes(groupSize)}
                                    </span>
                                  </div>
                                </div>

                                {/* Group download progress bar */}
                                {isDownloading && !isDone && !isBuiltIn && !isCloud && (
                                  <div className="mt-2 pt-2 border-t border-[rgba(var(--foreground),0.04)] space-y-1">
                                    <div className="flex justify-between text-[10px] font-mono text-[rgb(var(--foreground-muted))]">
                                      <span className={isGroupFailed ? 'text-[rgb(var(--error))] font-bold' : ''}>
                                        {stepLabel ?? MODEL_PROGRESS_STEPS.downloading}
                                      </span>
                                      <span>{modelProg?.progress ?? (isGroupDone ? 100 : 0)}%</span>
                                    </div>
                                    <div className="h-1 w-full bg-[rgba(var(--foreground),0.08)] rounded-full overflow-hidden">
                                      <div
                                        className={`h-full transition-all duration-300 ${
                                          isGroupFailed ? 'bg-[rgb(var(--error))]' : 'bg-[rgb(var(--accent))]'
                                        }`}
                                        style={{ width: `${modelProg?.progress ?? (isGroupDone ? 100 : 0)}%` }}
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
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
};
