import React, {
  useMemo,
  useRef,
  useEffect,
  useState,
  useCallback,
} from "react";
import { AnimatePresence, motion } from "framer-motion";
import { useOverlay } from "@/shared/hooks/useOverlay";
import {
  RefreshCw,
  X,
  Skull,
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
  ModelResidencyTiles,
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

  // Subscribe to settings store to inspect exact variants and reactive theme
  const accentSeed = useSettingsStore((s) => s.settings?.appearance.accent_seed);
  const theme = useSettingsStore((s) => s.settings?.appearance.theme);
  const vadBackend = useSettingsStore((s) => s.settings?.vad?.vad_backend);
  const llmProvider = useSettingsStore((s) => s.settings?.llm?.active);
  const ttsProvider = useSettingsStore((s) => s.settings?.tts?.active);
  const sttProvider = useSettingsStore((s) => s.settings?.stt?.active);
  const contextRetrievalEnabled = useSettingsStore(
    (s) => s.settings?.personal_memory?.context_retrieval_enabled ?? true
  );
  const transliterateEnabled = useSettingsStore(
    (s) => s.settings?.stt?.transliterate_enabled ?? true
  );
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);
  const llmEmbeddedModel = useSettingsStore((s) => s.settings?.llm?.embedded?.model ?? "");
  const llmServerModel = useSettingsStore((s) => s.settings?.llm?.server?.model ?? "");
  const llmCloudModel = useSettingsStore((s) => s.settings?.llm?.cloud?.model ?? "");
  const sttEmbeddedModel = useSettingsStore((s) => s.settings?.stt?.embedded?.model ?? "");
  const themeIsLight = theme === "light";

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
  } = useMonitoringMetrics(!popover || open);

  // Residency is a property of the manifest group, not of provider names.
  // Cloud and remote providers run off-device and are never resident; a
  // built-in VAD backend has no weights to load.
  const activeVadGroup = modelCatalog?.vad?.find((m) => m.id === vadBackend);
  const activeTtsGroup = modelCatalog?.tts?.find((m) => m.id === ttsProvider);

  const isVadModel = activeVadGroup ? !activeVadGroup.is_built_in : false;
  const isSttModel = sttProvider === "embedded";
  const isLlmModel = llmProvider === "embedded";
  const isTtsModel = activeTtsGroup ? !activeTtsGroup.is_cloud && !activeTtsGroup.is_remote : false;
  const isEmbedderModel = Boolean(contextRetrievalEnabled);
  const isTranslitModel = Boolean(transliterateEnabled);

  const totalResidentModelsCount = useMemo(() => {
    return (
      (isVadModel ? 1 : 0) +
      (isSttModel ? 1 : 0) +
      (isLlmModel ? 1 : 0) +
      (isTtsModel ? 1 : 0) +
      (isEmbedderModel ? 1 : 0) +
      (isTranslitModel ? 1 : 0)
    );
  }, [isVadModel, isSttModel, isLlmModel, isTtsModel, isEmbedderModel, isTranslitModel]);

  const activeModelsCount = useMemo(() => {
    return (
      (isVadModel && latest?.is_vad_loaded ? 1 : 0) +
      (isSttModel && latest?.is_stt_loaded ? 1 : 0) +
      (isLlmModel && latest?.is_llm_loaded ? 1 : 0) +
      (isTtsModel && latest?.is_tts_loaded ? 1 : 0) +
      (isEmbedderModel && latest?.is_embedder_loaded ? 1 : 0) +
      (isTranslitModel && latest?.is_translit_loaded ? 1 : 0)
    );
  }, [
    isVadModel,
    isSttModel,
    isLlmModel,
    isTtsModel,
    isEmbedderModel,
    isTranslitModel,
    latest?.is_vad_loaded,
    latest?.is_stt_loaded,
    latest?.is_llm_loaded,
    latest?.is_tts_loaded,
    latest?.is_embedder_loaded,
    latest?.is_translit_loaded,
  ]);

  const isEngineLoaded = useMemo(() => {
    return (
      activeModelsCount > 0 ||
      !!(
        latest?.is_vad_loaded ||
        latest?.is_stt_loaded ||
        latest?.is_llm_loaded ||
        latest?.is_tts_loaded
      )
    );
  }, [activeModelsCount, latest]);

  // Dynamically resolve loaded model names to display in chamber
  const loadedModelNames = useMemo((): string[] => {
    if (!latest) return [];
    const list: string[] = [];
    if (latest.is_llm_loaded) {
      const llmName =
        llmProvider === "embedded"
          ? llmEmbeddedModel || "LLM"
          : llmProvider === "server"
            ? llmServerModel || "LLM Server"
            : llmCloudModel || "LLM Cloud";
      list.push(llmName);
    }
    if (latest.is_stt_loaded) {
      list.push(sttProvider === "embedded" ? sttEmbeddedModel || "STT" : "Cloud STT");
    }
    if (latest.is_tts_loaded) {
      list.push(activeTtsGroup?.name ?? ttsProvider ?? "TTS");
    }
    if (latest.is_vad_loaded && isVadModel) {
      list.push(activeVadGroup?.name ?? "VAD");
    }
    if (latest.is_embedder_loaded && isEmbedderModel) {
      list.push("Embedder");
    }
    if (latest.is_translit_loaded && isTranslitModel) {
      list.push("Transliteration");
    }
    return list;
  }, [
    latest,
    llmProvider,
    sttProvider,
    ttsProvider,
    llmEmbeddedModel,
    llmServerModel,
    llmCloudModel,
    sttEmbeddedModel,
    activeTtsGroup,
    activeVadGroup,
    isVadModel,
    isEmbedderModel,
    isTranslitModel,
  ]);

  // Derive model variant labels (thinking, hearing, speaking)
  const variants = useMemo(() => {
    let llmVariant = "On Device";
    if (llmProvider === "server") {
      llmVariant = "On Server";
    } else if (llmProvider === "cloud") {
      llmVariant = "In Cloud";
    }

    let ttsVariant = "On Device";
    const ttsModel = modelCatalog?.tts?.find((m) => m.id === ttsProvider);
    if (ttsModel?.is_cloud) {
      ttsVariant = "In Cloud";
    } else if (ttsModel?.is_remote) {
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
  }, [llmProvider, ttsProvider, sttProvider, modelCatalog]);

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
    <div className="flex flex-col h-full w-full select-none gap-2.5">
      {/* ── 1. Top Header Bar (items-start: button tops share the 16px
          container top with every other header on the page) ── */}
      <div className="flex items-start justify-between pb-2.5 border-b border-[rgba(var(--accent),0.12)] shrink-0">
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
          {/* Unload / Load Models Button with Skull Icon when Loaded */}
          <Tooltip
            label={isEngineLoaded ? MONITORING_COPY.forceOffloadDesc : MONITORING_COPY.reloadModelsDesc}
          >
            <button
              onClick={handleToggleEngine}
              disabled={togglingEngine}
              className={cn(
                "px-3 py-1.5 rounded-xl border flex items-center gap-1.5 text-[11px] font-bold tracking-wider uppercase cursor-pointer shadow-md hover:scale-[1.02] transition-transform",
                isEngineLoaded
                  ? "bg-red-500/15 border-red-500/40 text-red-400"
                  : "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))]",
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
              className="w-8 h-8 flex items-center justify-center rounded-xl text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer shrink-0"
              aria-label={MONITORING_COPY.closeMonitor}
            >
              <X size={16} />
            </button>
          )}
        </div>
      </div>

      {/* ── 2. Model Residency Tiles ── */}
      <ErrorBoundary name="MonitoringResidency">
        <div className="flex justify-center w-full">
          <ModelResidencyTiles
            latest={latest}
            colors={colors}
            variants={variants}
            isLight={themeIsLight}
          />
        </div>
      </ErrorBoundary>

      {/* ── 3. Central Liquid Chamber Container ── */}
      <ErrorBoundary name="LiquidChamber">
        <LiquidChamber
          colors={colors}
          isEngineLoaded={isEngineLoaded}
          activeModelsCount={activeModelsCount}
          totalModelsCount={totalResidentModelsCount}
          cpuPct={cpuPct}
          ramMb={ramMb}
          ramGb={ramGb}
          ramPct={ramPct}
          loadedModelNames={loadedModelNames}
          popover={popover}
          open={open}
        />
      </ErrorBoundary>
    </div>
  );

  // If Popover mode: wrap in animated floating container
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
            className="fixed z-[200] bottom-[72px] left-4 w-[386px] max-w-[calc(100vw-32px)] h-[458px] max-h-[calc(100vh-96px)] glass-card p-3.5 flex flex-col shadow-2xl rounded-3xl"
            role="dialog"
            aria-label={MONITORING_COPY.monitorAria}
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
