import React, {
  useMemo,
  useRef,
  useEffect,
  useState,
  useCallback,
} from "react";
import { AnimatePresence, motion } from "framer-motion";
import { useProfilerDrawer } from "@/shared/components/profiler/ProfilerDrawer";
import { useOverlay } from "@/shared/hooks/useOverlay";
import { HelpTriggerButton } from "@/shared/components/help/HelpTriggerButton";
import {
  RefreshCw,
  X,
  Skull,
  Layers,
} from "lucide-react";
import {
  stopEngine,
  launchEngine,
} from "@/services/pipelineService";
import { useMonitoringMetrics } from "@/shared/hooks/useMonitoringMetrics";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import {
  parseRgb,
  rgbToHsl,
  hslToRgb,
  MetricCarousel,
  LiquidChamber,
} from "@/shared/components/monitoring";
import { Tooltip } from "@/shared/ui/Tooltip";
import { ErrorBoundary } from "@/shared/components/common";
import { MONITORING_COPY } from "@/data/monitoringCopy";

interface MonitoringProps {
  popover?: boolean;
  open?: boolean;
  onClose?: () => void;
  anchorRef?: React.RefObject<HTMLButtonElement | null>;
}

export const Monitoring: React.FC<MonitoringProps> = ({
  popover = false,
  open = true,
  onClose,
  anchorRef,
}) => {
  const modalRef = useRef<HTMLDivElement>(null);
  const { openProfiler } = useProfilerDrawer();

  // Subscribe to settings store to inspect exact variants and reactive theme
  const accentSeed = useSettingsStore((s) => s.settings?.appearance.accent_seed);
  const theme = useSettingsStore((s) => s.settings?.appearance.theme);
  const llmProvider = useSettingsStore((s) => s.settings?.llm?.active);
  const ttsProvider = useSettingsStore((s) => s.settings?.tts?.active);
  const sttProvider = useSettingsStore((s) => s.settings?.stt?.active);

  // Dynamic CSS variable observer state
  const [accentRgbStr, setAccentRgbStr] = useState<string>("0, 219, 233");

  const syncAccent = useCallback(() => {
    if (typeof window === "undefined") return;
    const val = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim();
    if (val && val !== accentRgbStr) {
      setAccentRgbStr(val);
    }
  }, [accentRgbStr]);

  // Keep colors continuously in sync when theme changes or DOM attribute shifts
  useEffect(() => {
    syncAccent();
    const observer = new MutationObserver(() => syncAccent());
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["style", "data-theme", "class"],
    });
    return () => observer.disconnect();
  }, [syncAccent, accentSeed, theme]);

  const {
    latest,
    engineToggling: togglingEngine,
    setEngineToggling: setTogglingEngine,
    formatLatency,
  } = useMonitoringMetrics(!popover || open);

  const isEngineLoaded = useMemo(() => {
    return !!(
      latest?.is_vad_loaded ||
      latest?.is_stt_loaded ||
      latest?.is_llm_loaded ||
      latest?.is_tts_loaded
    );
  }, [latest]);

  const isEdgeLoaded = useMemo(() => {
    return !!(latest?.is_intra_edge_classifier_loaded || latest?.is_inter_edge_classifier_loaded);
  }, [latest]);

  const activeModelsCount = useMemo(() => {
    return (
      (latest?.is_vad_loaded ? 1 : 0) +
      (latest?.is_stt_loaded ? 1 : 0) +
      (latest?.is_llm_loaded ? 1 : 0) +
      (latest?.is_tts_loaded ? 1 : 0) +
      (latest?.is_embedder_loaded ? 1 : 0) +
      (latest?.is_query_classifier_loaded ? 1 : 0) +
      (isEdgeLoaded ? 1 : 0) +
      (latest?.is_translit_loaded ? 1 : 0)
    );
  }, [latest, isEdgeLoaded]);

  // Derive model variant labels (thinking, hearing, speaking)
  const variants = useMemo(() => {
    let llmVariant = "On Device";
    if (llmProvider === "server") {
      llmVariant = "On Server";
    } else if (llmProvider === "cloud") {
      llmVariant = "In Cloud";
    }

    let ttsVariant = "On Device";
    if (ttsProvider === "edge_tts") {
      ttsVariant = "In Cloud";
    } else if (ttsProvider === "chatterbox_remote") {
      ttsVariant = "On Server";
    }

    let sttVariant = "On Device";
    if (sttProvider === "cloud") {
      sttVariant = "In Cloud";
    }

    return {
      llm: llmVariant,
      tts: ttsVariant,
      stt: sttVariant,
    };
  }, [llmProvider, ttsProvider, sttProvider]);

  // Derived Dynamic Color Palette (Primary Accent + Harmonized Violet/Magenta)
  const colors = useMemo(() => {
    const primaryRgb = parseRgb(accentRgbStr);
    const [h, s, l] = rgbToHsl(...primaryRgb);
    const compHue = (h + 140) % 360;
    const compRgb = hslToRgb(compHue, Math.min(100, s + 15), l);

    return {
      primary: `${primaryRgb[0]}, ${primaryRgb[1]}, ${primaryRgb[2]}`,
      complementary: `${compRgb[0]}, ${compRgb[1]}, ${compRgb[2]}`,
      primaryRgb,
      compRgb,
    };
  }, [accentRgbStr]);

  // Register with the global overlay stack for FILO Escape dismissal.
  useOverlay({ onClose: () => onClose?.(), active: !!(popover && open), ref: modalRef, dismissOnOutside: false });

  // Close handlers for popover mode
  useEffect(() => {
    if (!popover || !open) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (
        modalRef.current &&
        !modalRef.current.contains(e.target as Node) &&
        anchorRef?.current &&
        !anchorRef.current.contains(e.target as Node)
      ) {
        onClose?.();
      }
    };

    document.addEventListener("mousedown", handleClickOutside);

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [popover, open, onClose, anchorRef]);

  const handleToggleEngine = useCallback(async () => {
    if (togglingEngine) return;
    setTogglingEngine(true);
    try {
      if (isEngineLoaded) {
        await stopEngine();
      } else {
        await launchEngine();
      }
    } catch (e) {
      console.error("[Monitoring] Failed to toggle engine:", e);
    } finally {
      setTogglingEngine(false);
    }
  }, [isEngineLoaded, togglingEngine, setTogglingEngine]);

  const cpuPct = latest?.vox_cpu_usage || 0;
  const ramMb = latest?.vox_ram_mb || 0;
  const totalRamMb = latest?.total_ram_mb || 8192;
  const ramGb = (ramMb / 1024).toFixed(2);
  const ramPct = Math.min(100, Math.max(0, (ramMb / totalRamMb) * 100));

  const containerContent = (
    <div className="flex flex-col h-full w-full select-none">
      {/* ── 1. Top Header Bar ── */}
      <div className="flex items-center justify-between pb-3 border-b border-[rgba(var(--accent),0.12)] shrink-0 mb-3">
        <div className="flex items-center gap-3">
          <div className="flex flex-col">
            <h1 className="text-[15px] sm:text-[16px] font-display font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
              {MONITORING_COPY.monitoringTitle}
            </h1>
            <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))] uppercase tracking-wider">
              {MONITORING_COPY.monitoringSubtitle}
            </span>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <HelpTriggerButton deepLink="page:monitoring" size="sm" />
          {/* Memory Profiler Quick Launch Button (Disabled by default, can be toggled on for diagnostic sessions) */}
          {false && (
            <Tooltip label="Open UI Memory Attribution & RCA Profiler">
              <button
                onClick={() => {
                  onClose?.();
                  openProfiler();
                }}
                style={{
                  backgroundColor: `rgba(${colors.primary}, 0.10)`,
                  borderColor: `rgba(${colors.primary}, 0.25)`,
                  color: `rgb(${colors.primary})`,
                }}
                className="px-2.5 py-1.5 rounded-xl border transition-all duration-300 flex items-center gap-1.5 text-[11px] font-bold tracking-wider uppercase cursor-pointer shadow-md hover:scale-[1.02]"
              >
                <Layers size={13} />
                <span>PROFILER</span>
              </button>
            </Tooltip>
          )}

          {/* Unload / Load Models Button with Skull Icon when Loaded */}
          <Tooltip
            label={isEngineLoaded ? MONITORING_COPY.forceOffloadDesc : MONITORING_COPY.reloadModelsDesc}
          >
            <button
              onClick={handleToggleEngine}
              disabled={togglingEngine}
              style={{
                backgroundColor: isEngineLoaded
                  ? "rgba(239, 68, 68, 0.12)"
                  : `rgba(${colors.primary}, 0.12)`,
                borderColor: isEngineLoaded
                  ? "rgba(239, 68, 68, 0.35)"
                  : `rgba(${colors.primary}, 0.35)`,
                color: isEngineLoaded ? "#ef4444" : `rgb(${colors.primary})`,
              }}
              className={cn(
                "px-3 py-1.5 rounded-xl border transition-all duration-300 flex items-center gap-1.5 text-[11px] font-bold tracking-wider uppercase cursor-pointer shadow-md hover:scale-[1.02]",
                togglingEngine && "opacity-50 cursor-wait"
              )}
            >
              {togglingEngine ? (
                <RefreshCw size={12} className="animate-spin" />
              ) : isEngineLoaded ? (
                <Skull size={13} className="text-red-400" />
              ) : (
                <RefreshCw size={12} />
              )}
              <span>{isEngineLoaded ? "UNLOAD ALL" : "LOAD MODELS"}</span>
            </button>
          </Tooltip>

          {popover && onClose && (
            <button
              onClick={onClose}
              className="p-1.5 rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer"
              aria-label="Close monitor"
            >
              <X size={16} />
            </button>
          )}
        </div>
      </div>

      {/* ── 2. Top Metric Cards Carousel ── */}
      <ErrorBoundary name="MonitoringMetrics">
        <MetricCarousel
          latest={latest}
          colors={colors}
          formatLatency={formatLatency}
        />
      </ErrorBoundary>

      {/* ── 3. Central Liquid Chamber Container ── */}
      <ErrorBoundary name="LiquidChamber">
        <LiquidChamber
          latest={latest}
          colors={colors}
          isEngineLoaded={isEngineLoaded}
          activeModelsCount={activeModelsCount}
          cpuPct={cpuPct}
          ramMb={ramMb}
          ramGb={ramGb}
          ramPct={ramPct}
          variants={variants}
          popover={popover}
          open={open}
        />
      </ErrorBoundary>
    </div>
  );

  // If Popover mode: wrap in animated floating container with 10% reduced width (414px)
  if (popover) {
    return (
      <AnimatePresence>
        {open && (
          <motion.div
            ref={modalRef}
            initial={{ opacity: 0, y: 14, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 14, scale: 0.98 }}
            transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}
            className="fixed z-[200] bottom-[72px] left-4 w-[414px] max-w-[calc(100vw-32px)] h-[580px] max-h-[calc(100vh-96px)] glass-card p-4 flex flex-col shadow-2xl rounded-3xl"
            role="dialog"
            aria-label="System Monitoring"
          >
            {containerContent}
          </motion.div>
        )}
      </AnimatePresence>
    );
  }

  // Full-page route mode
  return (
    <div className="flex-1 flex flex-col h-full overflow-hidden bg-transparent px-4 sm:px-8 pt-4 pb-20 z-10 select-none max-w-4xl mx-auto w-full">
      {containerContent}
    </div>
  );
};
export default Monitoring;
