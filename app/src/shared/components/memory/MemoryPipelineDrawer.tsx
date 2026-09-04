import React, { useState, useEffect, useMemo, useCallback, memo, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  RefreshCw,
  X,
  Zap,
  Sparkles,
  CheckCircle2,
  Pause,
  Play,
  Filter,
  Box,
  GitBranch,
  Database,
  ShieldAlert,
  TrendingUp,
  Layers,
  Clock,
  ShieldCheck,
  Brain,
  AlertCircle,
  RotateCcw,
  Activity,
  Check,
} from "lucide-react";
import {
  MemoryNodeTopology,
  MemoryEdgeTopology,
  MemoryQueueSummary,
  togglePipelineProcessing,
  retryFailedQueue,
  retryFailedQueueItems,
} from "@/services/memoryService";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { parseRgb, rgbToHsl, hslToRgb } from "@/shared/components/monitoring/colorUtils";

interface MemoryPipelineDrawerProps {
  open: boolean;
  onClose: () => void;
  summary: MemoryQueueSummary | null;
  nodes: MemoryNodeTopology[];
  edges?: MemoryEdgeTopology[];
  onRefresh: () => void;
}

export const MemoryPipelineDrawer: React.FC<MemoryPipelineDrawerProps> = memo(({
  open,
  onClose,
  summary,
  nodes,
  edges = [],
  onRefresh,
}) => {
  const panelRef = useRef<HTMLDivElement>(null);

  // Gesture contract: registered with the global overlay stack so Escape pops
  // this right-side panel, and pointerdown outside the panel closes it.
  useOverlay({ onClose, active: open, ref: panelRef, dismissOnOutside: true });

  const [running, setRunning] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [lastProcessedCount, setLastProcessedCount] = useState<number | null>(null);
  const [lastRetriedCount, setLastRetriedCount] = useState<number | null>(null);
  const [retryError, setRetryError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"pipeline" | "failed">("pipeline");

  // Settings Store SSOT for Pipeline Processing Enabled
  const pipelineProcessingEnabled = useSettingsStore((s) => s.settings?.memory?.pipeline_processing_enabled ?? true);
  const accentSeed = useSettingsStore((s) => s.settings?.appearance.accent_seed);
  const theme = useSettingsStore((s) => s.settings?.appearance.theme);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const commitChanges = useSettingsStore((s) => s.commitChanges);

  // Dynamic CSS variable observer state for Primary Accent
  const [accentRgbStr, setAccentRgbStr] = useState<string>("0, 219, 233");

  const syncAccent = useCallback(() => {
    if (typeof window === "undefined") return;
    const val = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim();
    if (val && val !== accentRgbStr) {
      setAccentRgbStr(val);
    }
  }, [accentRgbStr]);

  // Keep colors in sync when theme changes while drawer is open
  useEffect(() => {
    if (!open) return;
    syncAccent();
    const observer = new MutationObserver(() => syncAccent());
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["style", "data-theme", "class"],
    });
    return () => observer.disconnect();
  }, [open, syncAccent, accentSeed, theme]);

  // Derive 4 Harmonic Pipeline Stage Colors from Primary Accent
  const stageColors = useMemo(() => {
    const primaryRgb = parseRgb(accentRgbStr);
    const [h, s, l] = rgbToHsl(...primaryRgb);

    const stage1Hsl = [h, s, l] as [number, number, number];
    const stage2Hsl = [(h + 45) % 360, Math.min(100, s + 10), l] as [number, number, number];
    const stage3Hsl = [(h + 90) % 360, Math.min(100, s + 15), l] as [number, number, number];
    const stage4Hsl = [(h + 140) % 360, Math.min(100, s + 20), l] as [number, number, number];

    const rgb1 = hslToRgb(...stage1Hsl);
    const rgb2 = hslToRgb(...stage2Hsl);
    const rgb3 = hslToRgb(...stage3Hsl);
    const rgb4 = hslToRgb(...stage4Hsl);

    return {
      stage1: `${rgb1[0]}, ${rgb1[1]}, ${rgb1[2]}`,
      stage2: `${rgb2[0]}, ${rgb2[1]}, ${rgb2[2]}`,
      stage3: `${rgb3[0]}, ${rgb3[1]}, ${rgb3[2]}`,
      stage4: `${rgb4[0]}, ${rgb4[1]}, ${rgb4[2]}`,
      failed: "239, 68, 68", // Fixed Semantic Red
    };
  }, [accentRgbStr]);

  // 10-second Polling Interval when drawer is open
  useEffect(() => {
    if (!open) return;
    onRefresh();
    const interval = setInterval(() => {
      onRefresh();
    }, 10000);
    return () => clearInterval(interval);
  }, [open, onRefresh]);

  const handleTrigger = async () => {
    setRunning(true);
    setLastProcessedCount(null);
    try {
      const nextState = await togglePipelineProcessing(true);
      updateDraft("memory", "pipeline_processing_enabled", nextState);
      await commitChanges();
      onRefresh();
    } catch (e) {
      console.error("Pipeline processing enable error:", e);
    } finally {
      setRunning(false);
    }
  };

  const handleTogglePause = async () => {
    try {
      const nextState = await togglePipelineProcessing();
      updateDraft("memory", "pipeline_processing_enabled", nextState);
      await commitChanges();
      onRefresh();
    } catch (e) {
      console.error("Toggle pipeline processing error:", e);
    }
  };

  const handleRetryAll = async () => {
    setRetrying(true);
    setLastRetriedCount(null);
    setRetryError(null);
    try {
      const count = await retryFailedQueue();
      setLastRetriedCount(count);
      onRefresh();
    } catch (e) {
      console.error("Retry failed queue error:", e);
      setRetryError(typeof e === "string" ? e : "Failed to retry queue items. Ensure pipeline processing is enabled.");
    } finally {
      setRetrying(false);
    }
  };

  const handleRetrySingleItem = async (itemId: number) => {
    setRetryError(null);
    try {
      await retryFailedQueueItems([itemId]);
      onRefresh();
    } catch (e) {
      console.error("Retry item error:", e);
      setRetryError(typeof e === "string" ? e : "Failed to retry item. Ensure pipeline processing is enabled.");
    }
  };

  // Real backend metrics from SQLite personal_memory_queue
  const stagedPendingCount = summary?.staged_pending ?? 0;
  const dedupPassCount = summary?.dedup_pass ?? 0;
  const nliEvaluatedCount = summary?.nli_evaluated ?? 0;
  const failedCount = summary?.failed ?? (summary?.failed_items?.length || 0);

  const totalPending = stagedPendingCount + dedupPassCount + nliEvaluatedCount;
  const activeNodesCount = nodes.length;
  const activeEdgesCount = edges.length;

  const recentItems = summary?.recent_items || [];
  const failedQueueItems = recentItems.filter((item) => item.status === "failed" || item.error_msg);

  return (
    <AnimatePresence>
      {open && (
        <div className="fixed inset-0 z-50 pointer-events-none overflow-hidden select-none">
          <motion.div
            initial={{ x: "100%" }}
            animate={{ x: 0 }}
            exit={{ x: "100%" }}
            transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
            className="fixed right-0 top-[var(--titlebar-height,40px)] bottom-0 z-50 w-[530px] max-w-[100vw] h-[calc(100vh-var(--titlebar-height,40px))] bg-[rgb(var(--card))]/95 backdrop-blur-3xl border-l border-[rgba(var(--border),0.12)] shadow-2xl flex flex-col pointer-events-auto overflow-hidden text-[rgb(var(--foreground))]"
            ref={panelRef}
          >
            {/* Header Section */}
            <div className="flex items-center justify-between px-6 py-3.5 border-b border-[rgba(var(--border),0.1)] shrink-0 bg-[rgba(var(--foreground),0.02)]">
              <div className="flex items-center gap-3">
                <div
                  style={{
                    backgroundColor: `rgba(${stageColors.stage1}, 0.12)`,
                    borderColor: `rgba(${stageColors.stage1}, 0.35)`,
                  }}
                  className="w-8 h-8 rounded-2xl border flex items-center justify-center shrink-0 shadow-md"
                >
                  <Brain size={18} style={{ color: `rgb(${stageColors.stage1})` }} />
                </div>
                <div className="flex flex-col">
                  <h2 className="font-display text-[13px] font-sans font-black tracking-wider uppercase text-[rgb(var(--foreground))]">
                    HOW VOX REMEMBERS
                  </h2>
                  <span
                    style={{ color: `rgba(${stageColors.stage1}, 0.85)` }}
                    className="text-[11px] font-sans font-medium"
                  >
                    Live memory activity
                  </span>
                </div>
              </div>

              <div className="flex items-center gap-2">
                <Tooltip
                  label={pipelineProcessingEnabled ? "Pause saving new memories" : "Resume saving new memories"}
                >
                  <button
                    onClick={handleTogglePause}
                    className={cn(
                      "p-1.5 rounded-xl transition-all cursor-pointer border flex items-center gap-1.5 px-2.5 text-[11px] font-bold uppercase shadow-sm",
                      !pipelineProcessingEnabled
                        ? "bg-amber-500/20 text-amber-400 border-amber-500/30 hover:bg-amber-500/30"
                        : "bg-emerald-500/20 text-emerald-400 border-emerald-500/30 hover:bg-emerald-500/30"
                    )}
                  >
                    {!pipelineProcessingEnabled ? <Play size={12} /> : <Pause size={12} />}
                    <span>{!pipelineProcessingEnabled ? "PAUSED" : "ACTIVE"}</span>
                  </button>
                </Tooltip>
                <Tooltip label="Refresh memory activity">
                  <button
                    onClick={onRefresh}
                    style={{ color: `rgb(${stageColors.stage1})` }}
                    className="p-1.5 rounded-xl hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer"
                  >
                    <RefreshCw size={14} className={cn(running && "animate-spin")} />
                  </button>
                </Tooltip>
                <button
                  onClick={onClose}
                  className="p-1.5 rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer"
                  aria-label="Close drawer"
                >
                  <X size={15} />
                </button>
              </div>
            </div>

            {/* Centered Top Navigation Tabs Switcher */}
            <div className="flex items-center justify-center gap-8 px-6 pt-3 pb-2 border-b border-[rgba(var(--border),0.08)] bg-[rgba(var(--foreground),0.01)] text-[12px] font-sans font-bold uppercase tracking-wider shrink-0">
              <button
                onClick={() => setActiveTab("pipeline")}
                style={{
                  borderBottomColor: activeTab === "pipeline" ? `rgb(${stageColors.stage1})` : "transparent",
                }}
                className={cn(
                  "transition-all cursor-pointer pb-2 border-b-2 flex items-center gap-2 text-center",
                  activeTab === "pipeline"
                    ? "font-black text-[rgb(var(--foreground))]"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                <Activity
                  size={14}
                  style={{
                    color: activeTab === "pipeline" ? `rgb(${stageColors.stage1})` : undefined,
                  }}
                />
                <span>How It Works</span>
              </button>

              <span className="text-[rgb(var(--foreground-muted))]/30 font-light select-none pb-2">|</span>

              <button
                onClick={() => setActiveTab("failed")}
                className={cn(
                  "transition-all cursor-pointer pb-2 border-b-2 flex items-center gap-2 text-center relative",
                  activeTab === "failed"
                    ? "border-red-400 font-black text-[rgb(var(--foreground))]"
                    : "border-transparent text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                <ShieldAlert
                  size={14}
                  className={cn(activeTab === "failed" ? "text-red-400" : undefined)}
                />
                <span>Failed Items</span>
                {failedCount > 0 && (
                  <span className="px-1.5 py-0.2 rounded-full bg-red-500/20 text-red-400 text-[11px] font-mono border border-red-500/30 font-bold">
                    {failedCount}
                  </span>
                )}
              </button>
            </div>

            {/* Scrollable Main Content Area */}
            <div className="flex-1 overflow-y-auto custom-scrollbar p-5 flex flex-col justify-between h-full min-h-0 gap-4">
              {activeTab === "pipeline" ? (
                /* TAB 1: Central Vertical Pipeline Conduit Stream (All 5 Stages with Harmonic Colors) */
                <div className="flex-1 flex flex-col justify-between relative py-2 min-h-[320px] h-full gap-2">
                  {/* Center Glowing Conduit Line */}
                  <div
                    style={{
                      background: `linear-gradient(to bottom, rgb(${stageColors.stage1}), rgb(${stageColors.stage2}), rgb(${stageColors.stage3}), rgb(${stageColors.stage4}), rgb(${stageColors.failed}))`,
                    }}
                    className="absolute left-1/2 top-4 bottom-4 w-[2px] -translate-x-1/2 pointer-events-none opacity-80"
                  />

                  {/* STAGE 1: Left Card | Center Node | Empty Right */}
                  <div className="relative flex items-center justify-between w-full min-h-0">
                    <div className="w-[44%] flex justify-end">
                      <div
                        style={{
                          borderColor: `rgba(${stageColors.stage1}, 0.25)`,
                        }}
                        className="w-full p-3 rounded-2xl bg-[rgba(var(--foreground),0.02)] border hover:border-opacity-60 transition-all flex flex-col gap-1 shadow-md"
                      >
                        <div className="flex items-center justify-between">
                          <span
                            style={{ color: `rgb(${stageColors.stage1})` }}
                            className="text-[11px] font-sans font-black tracking-wide uppercase"
                          >
                            1 REMOVE DUPLICATES
                          </span>
                          {stagedPendingCount === 0 ? (
                            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
                          ) : (
                            <span
                              style={{ backgroundColor: `rgb(${stageColors.stage1})` }}
                              className="w-2 h-2 rounded-full animate-ping"
                            />
                          )}
                        </div>
                        <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))]">
                          Removes repeated memories
                        </span>
                        <div className="flex items-center justify-between mt-1 pt-1 border-t border-[rgba(var(--border),0.06)]">
                          <div className="flex items-center gap-1">
                            <span
                              style={{ backgroundColor: `rgb(${stageColors.stage1})` }}
                              className="w-1.5 h-1.5 rounded-full animate-pulse"
                            />
                            <span
                              style={{ backgroundColor: `rgba(${stageColors.stage1}, 0.8)` }}
                              className="w-1.5 h-1.5 rounded-full"
                            />
                            <span
                              style={{ backgroundColor: `rgba(${stageColors.stage1}, 0.6)` }}
                              className="w-1.5 h-1.5 rounded-full"
                            />
                            <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.2)]" />
                          </div>
                          <span
                            style={{
                              backgroundColor: `rgba(${stageColors.stage1}, 0.12)`,
                              borderColor: `rgba(${stageColors.stage1}, 0.25)`,
                              color: `rgb(${stageColors.stage1})`,
                            }}
                            className="px-2 py-0.5 rounded-lg border text-[11px] font-bold"
                          >
                            {stagedPendingCount} staged
                          </span>
                        </div>
                      </div>
                    </div>

                    {/* Center Node 1 */}
                    <div
                      style={{
                        borderColor: `rgba(${stageColors.stage1}, 0.55)`,
                        color: `rgb(${stageColors.stage1})`,
                        boxShadow: `0 0 15px rgba(${stageColors.stage1}, 0.35)`,
                      }}
                      className="absolute left-1/2 -translate-x-1/2 w-9 h-9 rounded-full border bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10"
                    >
                      <Filter size={16} />
                    </div>

                    <div className="w-[44%]" />
                  </div>

                  {/* STAGE 2: Empty Left | Center Node | Right Card */}
                  <div className="relative flex items-center justify-between w-full min-h-0">
                    <div className="w-[44%]" />

                    {/* Center Node 2 */}
                    <div
                      style={{
                        borderColor: `rgba(${stageColors.stage2}, 0.55)`,
                        color: `rgb(${stageColors.stage2})`,
                        boxShadow: `0 0 15px rgba(${stageColors.stage2}, 0.35)`,
                      }}
                      className="absolute left-1/2 -translate-x-1/2 w-9 h-9 rounded-full border bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10"
                    >
                      <Box size={16} />
                    </div>

                    <div className="w-[44%] flex justify-start">
                      <div
                        style={{
                          borderColor: `rgba(${stageColors.stage2}, 0.25)`,
                        }}
                        className="w-full p-3 rounded-2xl bg-[rgba(var(--foreground),0.02)] border hover:border-opacity-60 transition-all flex flex-col gap-1 shadow-md"
                      >
                        <div className="flex items-center justify-between">
                          <span
                            style={{ color: `rgb(${stageColors.stage2})` }}
                            className="text-[11px] font-sans font-black tracking-wide uppercase"
                          >
                            2 UNDERSTANDING
                          </span>
                          {dedupPassCount === 0 ? (
                            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
                          ) : (
                            <span
                              style={{ backgroundColor: `rgb(${stageColors.stage2})` }}
                              className="w-2 h-2 rounded-full animate-ping"
                            />
                          )}
                        </div>
                        <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))]">
                          Working out what each memory means
                        </span>
                        <div className="flex items-center justify-between mt-1 pt-1 border-t border-[rgba(var(--border),0.06)]">
                          <div className="flex items-center gap-1">
                            <span
                              style={{ backgroundColor: `rgb(${stageColors.stage2})` }}
                              className="w-1.5 h-1.5 rounded-full animate-pulse"
                            />
                            <span
                              style={{ backgroundColor: `rgba(${stageColors.stage2}, 0.8)` }}
                              className="w-1.5 h-1.5 rounded-full"
                            />
                            <span
                              style={{ backgroundColor: `rgba(${stageColors.stage2}, 0.6)` }}
                              className="w-1.5 h-1.5 rounded-full"
                            />
                            <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.2)]" />
                          </div>
                          <span
                            style={{
                              backgroundColor: `rgba(${stageColors.stage2}, 0.12)`,
                              borderColor: `rgba(${stageColors.stage2}, 0.25)`,
                              color: `rgb(${stageColors.stage2})`,
                            }}
                            className="px-2 py-0.5 rounded-lg border text-[11px] font-bold"
                          >
                            {dedupPassCount} pending
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* STAGE 3: Left Card | Center Node | Empty Right */}
                  <div className="relative flex items-center justify-between w-full min-h-0">
                    <div className="w-[44%] flex justify-end">
                      <div
                        style={{
                          borderColor: `rgba(${stageColors.stage3}, 0.25)`,
                        }}
                        className="w-full p-3 rounded-2xl bg-[rgba(var(--foreground),0.02)] border hover:border-opacity-60 transition-all flex flex-col gap-1 shadow-md"
                      >
                        <div className="flex items-center justify-between">
                          <span
                            style={{ color: `rgb(${stageColors.stage3})` }}
                            className="text-[11px] font-sans font-black tracking-wide uppercase"
                          >
                            3 CHECK FACTS
                          </span>
                          {nliEvaluatedCount === 0 ? (
                            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
                          ) : (
                            <span
                              style={{ backgroundColor: `rgb(${stageColors.stage3})` }}
                              className="w-2 h-2 rounded-full animate-ping"
                            />
                          )}
                        </div>
                        <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))]">
                          Double-checks each memory
                        </span>
                        <div className="flex items-center justify-between mt-1 pt-1 border-t border-[rgba(var(--border),0.06)]">
                          <div className="flex items-center gap-1">
                            <span
                              style={{ backgroundColor: `rgb(${stageColors.stage3})` }}
                              className="w-1.5 h-1.5 rounded-full animate-pulse"
                            />
                            <span
                              style={{ backgroundColor: `rgba(${stageColors.stage3}, 0.8)` }}
                              className="w-1.5 h-1.5 rounded-full"
                            />
                            <span
                              style={{ backgroundColor: `rgba(${stageColors.stage3}, 0.6)` }}
                              className="w-1.5 h-1.5 rounded-full"
                            />
                            <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.2)]" />
                          </div>
                          <span
                            style={{
                              backgroundColor: `rgba(${stageColors.stage3}, 0.12)`,
                              borderColor: `rgba(${stageColors.stage3}, 0.25)`,
                              color: `rgb(${stageColors.stage3})`,
                            }}
                            className="px-2 py-0.5 rounded-lg border text-[11px] font-bold"
                          >
                            {nliEvaluatedCount} evaluated
                          </span>
                        </div>
                      </div>
                    </div>

                    {/* Center Node 3 */}
                    <div
                      style={{
                        borderColor: `rgba(${stageColors.stage3}, 0.55)`,
                        color: `rgb(${stageColors.stage3})`,
                        boxShadow: `0 0 15px rgba(${stageColors.stage3}, 0.35)`,
                      }}
                      className="absolute left-1/2 -translate-x-1/2 w-9 h-9 rounded-full border bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10"
                    >
                      <GitBranch size={16} />
                    </div>

                    <div className="w-[44%]" />
                  </div>

                  {/* STAGE 4: Empty Left | Center Node | Right Card */}
                  <div className="relative flex items-center justify-between w-full min-h-0">
                    <div className="w-[44%]" />

                    {/* Center Node 4 */}
                    <div
                      style={{
                        borderColor: `rgba(${stageColors.stage4}, 0.55)`,
                        color: `rgb(${stageColors.stage4})`,
                        boxShadow: `0 0 15px rgba(${stageColors.stage4}, 0.35)`,
                      }}
                      className="absolute left-1/2 -translate-x-1/2 w-9 h-9 rounded-full border bg-[rgb(var(--card))] flex items-center justify-center shrink-0 z-10"
                    >
                      <Database size={16} />
                    </div>

                    <div className="w-[44%] flex justify-start">
                      <div
                        style={{
                          borderColor: `rgba(${stageColors.stage4}, 0.25)`,
                        }}
                        className="w-full p-3 rounded-2xl bg-[rgba(var(--foreground),0.02)] border hover:border-opacity-60 transition-all flex flex-col gap-1 shadow-md"
                      >
                        <div className="flex items-center justify-between">
                          <span
                            style={{ color: `rgb(${stageColors.stage4})` }}
                            className="text-[11px] font-sans font-black tracking-wide uppercase"
                          >
                            4 SAVE
                          </span>
                          {running ? (
                            <span
                              style={{ borderColor: `rgb(${stageColors.stage4})` }}
                              className="w-3.5 h-3.5 rounded-full border-2 border-t-transparent animate-spin shrink-0"
                            />
                          ) : (
                            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
                          )}
                        </div>
                        <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))]">
                          Stores it for later
                        </span>
                        <div className="flex items-center justify-between mt-1 pt-1 border-t border-[rgba(var(--border),0.06)]">
                          <div className="flex items-center gap-1">
                            <span
                              style={{ backgroundColor: `rgb(${stageColors.stage4})` }}
                              className="w-1.5 h-1.5 rounded-full animate-pulse"
                            />
                            <span
                              style={{ backgroundColor: `rgba(${stageColors.stage4}, 0.8)` }}
                              className="w-1.5 h-1.5 rounded-full"
                            />
                            <span
                              style={{ backgroundColor: `rgba(${stageColors.stage4}, 0.6)` }}
                              className="w-1.5 h-1.5 rounded-full"
                            />
                            <span className="w-1.5 h-1.5 rounded-full bg-[rgba(var(--foreground),0.2)]" />
                          </div>
                          <span
                            style={{
                              backgroundColor: `rgba(${stageColors.stage4}, 0.12)`,
                              borderColor: `rgba(${stageColors.stage4}, 0.25)`,
                              color: `rgb(${stageColors.stage4})`,
                            }}
                            className="px-2 py-0.5 rounded-lg border text-[11px] font-bold"
                          >
                            {totalPending > 0 ? `${totalPending} ready` : "Synced"}
                          </span>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* STAGE 5: Left Card | Center Node | Empty Right */}
                  <div className="relative flex items-center justify-between w-full min-h-0">
                    <div className="w-[44%] flex justify-end">
                      <div
                        onClick={() => setActiveTab("failed")}
                        className={cn(
                          "w-full p-3 rounded-2xl bg-[rgba(var(--foreground),0.02)] border transition-all flex flex-col gap-1 shadow-md cursor-pointer group",
                          failedCount > 0 ? "border-[rgb(var(--danger))]/40 hover:border-[rgb(var(--danger))]/60" : "border-[rgba(var(--border),0.1)] hover:border-[rgba(var(--border),0.2)]"
                        )}
                      >
                        <div className="flex items-center justify-between">
                          <span className="text-[11px] font-sans font-black tracking-wide text-[rgb(var(--danger))] uppercase">
                            5 NEEDS YOUR ATTENTION
                          </span>
                          {failedCount > 0 ? (
                            <AlertCircle size={15} className="text-[rgb(var(--danger))] shrink-0" />
                          ) : (
                            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
                          )}
                        </div>
                        <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))]">
                          Memories you may want to check
                        </span>
                        <div className="flex items-center justify-between mt-1 pt-1 border-t border-[rgba(var(--border),0.06)]">
                          <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
                            {failedCount > 0 ? "Click to view errors" : "No errors"}
                          </span>
                          <span className={cn("px-2 py-0.5 rounded-lg text-[11px] font-bold border", failedCount > 0 ? "bg-[rgba(var(--danger),0.2)] text-[rgb(var(--danger))] border-[rgb(var(--danger))]/30" : "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] border-[rgba(var(--border),0.08)]")}>
                            {failedCount} failed
                          </span>
                        </div>
                      </div>
                    </div>

                    {/* Center Node 5 */}
                    <div className="absolute left-1/2 -translate-x-1/2 w-9 h-9 rounded-full border border-red-500/50 bg-[rgb(var(--card))] text-red-400 flex items-center justify-center shrink-0 z-10 shadow-[0_0_15px_rgba(239,68,68,0.35)]">
                      <ShieldAlert size={16} />
                    </div>

                    <div className="w-[44%]" />
                  </div>
                </div>
              ) : (
                /* TAB 2: Dedicated Failed Queue Items View (Clean Subtle Rows, No Pill Fatigue) */
                <div className="flex-1 flex flex-col justify-between h-full min-h-0 gap-4">
                  <div className="flex items-center justify-between px-1">
                    <span className="text-[11px] font-sans font-bold uppercase tracking-wider text-red-400 flex items-center gap-1.5">
                      <AlertCircle size={14} />
                      Needs Your Attention ({failedQueueItems.length})
                    </span>

                    {failedQueueItems.length > 0 && (
                      <button
                        onClick={handleRetryAll}
                        disabled={retrying}
                        style={{
                          backgroundColor: `rgba(${stageColors.stage1}, 0.15)`,
                          borderColor: `rgba(${stageColors.stage1}, 0.35)`,
                          color: `rgb(${stageColors.stage1})`,
                        }}
                        className="px-3 py-1.5 rounded-xl border text-[11px] font-sans font-bold transition-all flex items-center gap-1.5 cursor-pointer disabled:opacity-40"
                      >
                        <RotateCcw size={12} className={cn(retrying && "animate-spin")} />
                        <span>Retry All</span>
                      </button>
                    )}
                  </div>

                  {failedQueueItems.length === 0 ? (
                    <div className="flex-1 flex flex-col items-center justify-center p-8 text-center bg-[rgba(var(--foreground),0.02)] rounded-2xl border border-[rgba(var(--border),0.06)]">
                      <CheckCircle2 size={32} className="text-emerald-400 mb-2" />
                      <span className="text-[13px] font-sans font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">
                        Nothing Needs Attention
                      </span>
                      <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] mt-1 max-w-xs">
                        Everything was saved without problems.
                      </span>
                    </div>
                  ) : (
                    <div className="flex-1 flex flex-col gap-2 overflow-y-auto custom-scrollbar pr-1">
                      {failedQueueItems.map((item) => (
                        <div key={item.id} className="p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] hover:bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.06)] transition-all flex flex-col gap-1.5">
                          <div className="flex items-start justify-between gap-3">
                            <span className="text-[12px] font-sans text-[rgb(var(--foreground))] font-medium">
                              "{item.fact}"
                            </span>
                            <Tooltip label="Try this one again">
                            <button
                              onClick={() => handleRetrySingleItem(item.id)}
                              className="px-2.5 py-1 rounded-lg bg-[rgba(var(--foreground),0.05)] hover:bg-[rgba(var(--foreground),0.1)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] text-[11px] font-sans font-medium transition-colors cursor-pointer shrink-0 border border-[rgba(var(--border),0.1)] flex items-center gap-1"
                            >
                              <RotateCcw size={10} />
                              <span>Retry</span>
                            </button>
                          </Tooltip>
                          </div>

                          <div className="flex items-center justify-between text-[11px] font-sans text-[rgb(var(--foreground-muted))] pt-1 border-t border-[rgba(var(--border),0.06)]">
                            <span>Category: <strong style={{ color: `rgb(${stageColors.stage1})` }} className="font-semibold">{item.collection}</strong></span>
                            <span>Tries: <strong style={{ color: `rgb(${stageColors.stage3})` }} className="font-semibold">{item.attempts}</strong></span>
                          </div>

                          {item.error_msg && (
                            <span className="text-[11px] font-sans text-red-400/80 line-clamp-2">
                              {item.error_msg}
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  )}

                  {lastRetriedCount !== null && (
                    <div className="p-2.5 rounded-xl bg-emerald-500/15 border border-emerald-500/30 text-emerald-400 text-[11px] font-sans font-medium text-center flex items-center justify-center gap-1.5">
                      <Check size={14} />
                      <span>Added {lastRetriedCount} items back to the queue.</span>
                    </div>
                  )}

                  {retryError && (
                    <div className="p-2.5 rounded-xl bg-red-500/15 border border-red-500/30 text-red-400 text-[11px] font-sans font-medium text-center flex items-center justify-center gap-1.5">
                      <AlertCircle size={14} />
                      <span>{retryError}</span>
                    </div>
                  )}
                </div>
              )}

              {/* Expanded Prominent Bottom Telemetry Strip */}
              <div className="p-3.5 rounded-2xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.1)] grid grid-cols-4 gap-2 shrink-0 shadow-lg text-center">
                <div className="flex flex-col items-center">
                  <TrendingUp size={16} style={{ color: `rgb(${stageColors.stage1})` }} className="mb-0.5" />
                  <span className="text-[14px] font-sans font-black text-[rgb(var(--foreground))]">
                    {activeNodesCount.toLocaleString()}
                  </span>
                  <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] uppercase font-semibold">
                    Memories
                  </span>
                </div>

                <div className="flex flex-col items-center border-l border-[rgba(var(--border),0.08)]">
                  <Layers size={16} style={{ color: `rgb(${stageColors.stage2})` }} className="mb-0.5" />
                  <span className="text-[14px] font-sans font-black text-[rgb(var(--foreground))]">
                    {activeEdgesCount.toLocaleString()}
                  </span>
                  <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] uppercase font-semibold">
                    Connections
                  </span>
                </div>

                <div className="flex flex-col items-center border-l border-[rgba(var(--border),0.08)]">
                  <Clock size={16} style={{ color: `rgb(${stageColors.stage4})` }} className="mb-0.5" />
                  <span className="text-[14px] font-sans font-black text-[rgb(var(--foreground))]">
                    {totalPending}
                  </span>
                  <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] uppercase font-semibold">
                    Waiting to Save
                  </span>
                </div>

                <div className="flex flex-col items-center border-l border-[rgba(var(--border),0.08)]">
                  <ShieldCheck size={16} className={cn("mb-0.5", failedCount > 0 ? "text-red-400" : "text-emerald-400")} />
                  <span className="text-[14px] font-sans font-black text-[rgb(var(--foreground))]">
                    {failedCount > 0 ? `${failedCount} to check` : "All Good"}
                  </span>
                  <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] uppercase font-semibold">
                    Memory Health
                  </span>
                </div>
              </div>
            </div>

            {/* Bottom Immediate Queue Processing Control */}
            <div className="p-4 border-t border-[rgba(var(--border),0.1)] bg-[rgba(var(--foreground),0.02)] shrink-0 flex flex-col gap-2">
              {lastProcessedCount !== null && (
                <div className="flex items-center gap-1.5 text-emerald-400 text-[11px] font-sans justify-center font-medium">
                  <CheckCircle2 size={14} />
                  <span>Saved {lastProcessedCount} items into memory.</span>
                </div>
              )}

              <button
                onClick={handleTrigger}
                disabled={running}
                style={{
                  backgroundColor: "transparent",
                  borderColor: `rgba(${stageColors.stage1}, 0.45)`,
                  color: `rgb(${stageColors.stage1})`,
                }}
                className="w-full py-3.5 rounded-xl border hover:bg-[rgba(var(--foreground),0.04)] transition-all flex items-center justify-center gap-2 cursor-pointer disabled:opacity-40 text-[12px] font-sans font-bold uppercase tracking-wider shadow-sm"
              >
                {running ? (
                  <Sparkles size={16} className="animate-spin" />
                ) : (
                  <Zap size={16} />
                )}
                <span>{running ? MEMORY_COPY.consolidating : "SAVE PENDING MEMORIES"}</span>
              </button>
              <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))] text-center">
                Organizes the memories Vox hasn't saved yet
              </span>
            </div>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
});

MemoryPipelineDrawer.displayName = "MemoryPipelineDrawer";
