import { useState, useMemo, useEffect, useCallback, memo, Suspense, lazy } from "react";
import { RotateCcw, AlertCircle, Check, RefreshCw, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore } from "@/store/settingsStore";
import { ErrorBoundary, OrbitalLoader } from "@/shared/components/common";
import { AnimatePresence, motion } from "framer-motion";
import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId, type SettingsDomain as Domain } from "@/data/settingsCopy";
import { SETTINGS_COPY } from "@/data/settingsCopy";

// Loader functions for eager prewarming
const loadPersona = () => import("@/shared/components/settings/persona/PersonaCard").then(m => ({ default: m.PersonaCard }));
const loadModels = () => import("@/shared/components/settings/models/ModelsCard").then(m => ({ default: m.ModelsCard }));
const loadRealtime = () => import("@/shared/components/settings/realtime/RealtimeCard").then(m => ({ default: m.RealtimeCard }));
const loadHistory = () => import("@/shared/components/settings/history/HistoryCard").then(m => ({ default: m.HistoryCard }));
const loadMemory = () => import("@/shared/components/settings/memory/MemoryCard").then(m => ({ default: m.MemoryCard }));
const loadAppearance = () => import("@/shared/components/settings/appearance/AppearanceCard").then(m => ({ default: m.AppearanceCard }));
const loadInteraction = () => import("@/shared/components/settings/interaction/InteractionCard").then(m => ({ default: m.InteractionCard }));

// Lazy-loaded domain card components
const PersonaCard = lazy(loadPersona);
const ModelsCard = lazy(loadModels);
const RealtimeCard = lazy(loadRealtime);
const HistoryCard = lazy(loadHistory);
const MemoryCard = lazy(loadMemory);
const AppearanceCard = lazy(loadAppearance);
const InteractionCard = lazy(loadInteraction);

import { SettingsCardSkeleton } from "@/shared/components/settings/SettingsCardSkeleton";

const DomainContent = memo(({ domain, layoutMode }: { domain: DomainId; layoutMode?: "full-max" | "full-min" | "small" }) => {
  const isRealtime = useSettingsStore((s) => s.draftSettings?.interaction?.pipeline_mode === "realtime");
  return (
    <Suspense fallback={<SettingsCardSkeleton layoutMode={layoutMode} />}>
      {(() => {
        switch (domain) {
          case "persona":
            return <PersonaCard layoutMode={layoutMode} />;
          case "models":
            return isRealtime ? <RealtimeCard layoutMode={layoutMode} /> : <ModelsCard layoutMode={layoutMode} />;
          case "history":
            return <HistoryCard layoutMode={layoutMode} />;
          case "memory":
            return <MemoryCard layoutMode={layoutMode} />;
          case "appearance":
            return <AppearanceCard layoutMode={layoutMode} />;
          case "interaction":
            return <InteractionCard layoutMode={layoutMode} />;
          default:
            return null;
        }
      })()}
    </Suspense>
  );
});
DomainContent.displayName = "DomainContent";

import { RadialNode, HubConnectors } from "@/shared/components/settings/RadialHub";
import { Tooltip } from "@/shared/ui/Tooltip";


interface HubCenterProps {
  onClick: () => void;
  hasActiveCards: boolean;
}

const HubCenter = memo(({ onClick, hasActiveCards }: HubCenterProps) => (
  <Tooltip
    label={hasActiveCards ? SETTINGS_COPY.closeAllDomains : SETTINGS_COPY.openAllDomains}
    side="top"
    wrapperClassName="absolute left-1/2 top-1/2 z-30"
    wrapperStyle={{ transform: "translate(-50%, -50%)" }}
  >
    <button
      id="center-node"
      onClick={onClick}
      className="relative w-16 h-16 rounded-full flex items-center justify-center transition-all duration-400 cursor-pointer"
      aria-label={hasActiveCards ? SETTINGS_COPY.closeAllDomains : SETTINGS_COPY.openAllDomains}
    >
    {/* Layer 1 (outermost): A circle ~52px diameter */}
    <div
      className="absolute rounded-full border border-dashed transition-all duration-400"
      style={{
        width: "52px",
        height: "52px",
        borderColor: `rgba(var(--accent), ${hasActiveCards ? 0.35 : 0.20})`,
        animation: hasActiveCards ? "border-rotate 18s linear infinite" : "none",
        background: "transparent",
      }}
    />

    {/* Layer 2: A circle ~38px diameter */}
    <div
      className="absolute rounded-full border transition-all duration-400"
      style={{
        width: "38px",
        height: "38px",
        borderColor: "rgba(var(--accent), 0.40)",
        background: "transparent",
        boxShadow: `inset 0 0 12px rgba(var(--accent), 0.15), 0 0 18px rgba(var(--accent), 0.10)`,
      }}
    />

    {/* Layer 3: A circle ~22px diameter */}
    <div
      className="absolute rounded-full border transition-all duration-400"
      style={{
        width: "22px",
        height: "22px",
        borderColor: "rgba(var(--accent), 0.60)",
        background: "radial-gradient(circle, rgba(var(--accent), 0.25) 0%, transparent 100%)",
        boxShadow: "0 0 16px rgba(var(--accent), 0.35)",
        animation: hasActiveCards ? "reactor-pulse 2.5s ease-in-out infinite" : "none",
      }}
    />

    {/* Layer 4 (innermost dot): Circle 6px */}
    <div
      className="absolute rounded-full transition-all duration-400"
      style={{
        width: "6px",
        height: "6px",
        backgroundColor: "rgb(var(--accent))",
        boxShadow: "0 0 8px rgba(var(--accent), 0.8)",
      }}
    />
    </button>
  </Tooltip>
));
HubCenter.displayName = "HubCenter";

import { useSettingsPage } from "@/shared/hooks/useSettingsPage";

interface SettingsCardWrapperProps {
  domain: Domain;
  isActive: boolean;
  layoutMode: "full-max" | "full-min" | "small";
}

const SettingsCardWrapper = memo(({ domain, isActive, layoutMode }: SettingsCardWrapperProps) => {
  const settings = useSettingsStore((s) => s.settings);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const commitChanges = useSettingsStore((s) => s.commitChanges);

  const hasChanges = useSettingsStore(useCallback((s: any) => Boolean(s.isDomainDirty(domain.id)), [domain.id]));

  const requiresRestart = useMemo(() => {
    if (!settings || !draftSettings) return false;
    if (domain.id === "models") {
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      if (isRealtime) return false;
      return (
        settings.vad.vad_backend !== draftSettings.vad.vad_backend ||
        settings.stt.active !== draftSettings.stt.active ||
        settings.stt.embedded.model !== draftSettings.stt.embedded.model ||
        settings.llm.active !== draftSettings.llm.active ||
        settings.llm.context_window !== draftSettings.llm.context_window ||
        settings.llm.threads !== draftSettings.llm.threads ||
        settings.tts.active !== draftSettings.tts.active
      );
    }
    return false;
  }, [domain.id, settings, draftSettings]);

  const isCloudLlmMissingKey =
    draftSettings?.llm?.active === "cloud" &&
    !draftSettings?.llm?.cloud?.api_key?.trim();
  const isCloudSttMissingKey =
    draftSettings?.stt?.active === "cloud" &&
    !draftSettings?.stt?.cloud?.api_key?.trim();
  const isRealtimeMissingKey =
    draftSettings?.interaction?.pipeline_mode === "realtime" &&
    ((draftSettings?.realtime?.active === "gemini_live" && !(draftSettings?.realtime?.gemini_live?.api_key || (draftSettings?.realtime as any)?.gemini?.api_key)?.trim()) ||
     (draftSettings?.realtime?.active === "deepgram_voice_agent" && !(draftSettings?.realtime?.deepgram_voice_agent?.api_key || (draftSettings?.realtime as any)?.deepgram?.api_key)?.trim()));
  const isMissingCloudKey = isCloudLlmMissingKey || isCloudSttMissingKey || isRealtimeMissingKey;

  const handleSave = () => {
    if (isMissingCloudKey) return;
    commitChanges();
  };

  const isAutoSavedHere = useSettingsStore((s) => s.autoSavedDomain === domain.id);

  return (
    <AnimatePresence>
      {isActive && (
        <motion.div
          initial={{ opacity: 0, scale: 0.96 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.96 }}
          transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
          className="w-full h-full flex items-center justify-center pointer-events-auto"
        >
          <div
            id={`card-${domain.id}`}
            className={cn(
              "shrink-0 flex flex-col gap-0",
              hasChanges && "has-unsaved-changes"
            )}
          >
            {/* Actual Card content */}
            <ErrorBoundary name={`Settings:${domain.id}`}>
              <DomainContent domain={domain.id} layoutMode={layoutMode} />
            </ErrorBoundary>

            {/* ─── Repurposed Dynamic Footer: Auto-Save Confirmation OR Heavy Restart Action Bar ─── */}
            {(layoutMode === "full-max" || layoutMode === "full-min") && (
              <AnimatePresence>
                {/* Mode A: Explicit Restart Required Bar (ONLY for Type 3 Restart or Missing Cloud Key) */}
                {hasChanges && (requiresRestart || isMissingCloudKey) && (
                  <motion.div
                    key="restart-footer"
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    transition={{ duration: 0.2 }}
                    className="w-full p-3 px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center justify-between overflow-hidden text-[12px]"
                  >
                    {isMissingCloudKey ? (
                      <>
                        <span className="font-bold uppercase tracking-wider text-rose-400 flex items-center gap-1.5">
                          <AlertCircle size={14} /> API Key Required for Cloud Provider
                        </span>
                        <div className="flex gap-2">
                          <button
                            disabled
                            className="px-3.5 py-1 rounded-lg bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground-muted))]/40 font-black text-[12px] uppercase tracking-wider cursor-not-allowed border border-[rgba(var(--border),0.1)]"
                          >
                            {SETTINGS_COPY.saveChanges}
                          </button>
                          <button
                            onClick={() => useSettingsStore.getState().discardDomainChanges(domain.id)}
                            className="px-3 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-rose-400 hover:bg-rose-500/10 border border-transparent hover:border-rose-500/20 text-[12px] font-bold uppercase tracking-wider transition-all cursor-pointer"
                          >
                            {SETTINGS_COPY.discardChanges}
                          </button>
                        </div>
                      </>
                    ) : (
                      <>
                        <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-1.5">
                          <RefreshCw size={14} /> {requiresRestart ? "Pipeline Restart Required" : SETTINGS_COPY.unsavedChanges}
                        </span>
                        <div className="flex gap-2">
                          <button
                            onClick={handleSave}
                            className="px-3.5 py-1 rounded-lg bg-[rgb(var(--accent))] text-black dark:text-white font-black text-[12px] uppercase tracking-wider hover:brightness-110 active:scale-95 transition-all cursor-pointer shadow-md flex items-center gap-1.5"
                          >
                            <span>{requiresRestart ? "Apply & Reload" : SETTINGS_COPY.saveChanges}</span>
                          </button>
                          <button
                            onClick={() => useSettingsStore.getState().discardDomainChanges(domain.id)}
                            className="px-3 py-1 rounded-lg bg-transparent text-[rgb(var(--foreground-muted))] hover:text-rose-400 hover:bg-rose-500/10 border border-transparent hover:border-rose-500/20 text-[12px] font-bold uppercase tracking-wider transition-all cursor-pointer"
                          >
                            {SETTINGS_COPY.discardChanges}
                          </button>
                        </div>
                      </>
                    )}
                  </motion.div>
                )}

                {/* Mode B: Debounced "Changes Saved" Auto-Toast (Only on the specific modified card, using Primary Accent) */}
                {!hasChanges && isAutoSavedHere && (
                  <motion.div
                    key="saved-toast-footer"
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    exit={{ opacity: 0, height: 0 }}
                    transition={{ duration: 0.2 }}
                    className="w-full py-2 px-5 rounded-b-[1.25rem] rounded-t-none bg-[rgba(var(--accent),0.08)] dark:bg-[rgba(var(--accent),0.12)] border border-t-0 border-[rgba(var(--accent),0.2)] flex items-center justify-between overflow-hidden text-[12px]"
                  >
                    <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-1.5">
                      <Check size={14} /> Changes Saved
                    </span>
                    <span className="text-[11px] text-[rgb(var(--accent))]/70 font-mono">Auto-synced</span>
                  </motion.div>
                )}
              </AnimatePresence>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
});
SettingsCardWrapper.displayName = "SettingsCardWrapper";

export const Settings: React.FC = () => {
  const settings = useSettingsStore((s) => s.settings);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const commitChanges = useSettingsStore((s) => s.commitChanges);
  const discardChanges = useSettingsStore((s) => s.discardChanges);
  const hasChanges = useSettingsStore((s) => s.hasChanges);
  const autoSavedDomain = useSettingsStore((s) => s.autoSavedDomain);
  const isAutoSaved = !!autoSavedDomain;
  const restoreDefaults = useSettingsStore((s) => s.restoreDefaults);
  const [isMobileConfirmRestore, setIsMobileConfirmRestore] = useState(false);

  const requiresRestart = useMemo(() => {
    if (!settings || !draftSettings) return false;
    const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
    if (isRealtime) return false;
    return (
      settings.vad.vad_backend !== draftSettings.vad.vad_backend ||
      settings.stt.active !== draftSettings.stt.active ||
      settings.stt.embedded.model !== draftSettings.stt.embedded.model ||
      settings.llm.active !== draftSettings.llm.active ||
      settings.llm.context_window !== draftSettings.llm.context_window ||
      settings.llm.threads !== draftSettings.llm.threads ||
      settings.tts.active !== draftSettings.tts.active
    );
  }, [settings, draftSettings]);

  const isCloudLlmMissingKey =
    draftSettings?.llm?.active === "cloud" &&
    !draftSettings?.llm?.cloud?.api_key?.trim();
  const isCloudSttMissingKey =
    draftSettings?.stt?.active === "cloud" &&
    !draftSettings?.stt?.cloud?.api_key?.trim();
  const isRealtimeMissingKey =
    draftSettings?.interaction?.pipeline_mode === "realtime" &&
    ((draftSettings?.realtime?.active === "gemini_live" && !(draftSettings?.realtime?.gemini_live?.api_key || (draftSettings?.realtime as any)?.gemini?.api_key)?.trim()) ||
     (draftSettings?.realtime?.active === "deepgram_voice_agent" && !(draftSettings?.realtime?.deepgram_voice_agent?.api_key || (draftSettings?.realtime as any)?.deepgram?.api_key)?.trim()));
  const isMissingCloudKey = isCloudLlmMissingKey || isCloudSttMissingKey || isRealtimeMissingKey;

  const {
    containerRef,
    activeDomains,
    isCompact,
    lines,
    radiusX,
    radiusY,
    layoutMode,
    handleSelect,
    handleCenterClick,
    setActiveDomains,
  } = useSettingsPage();

  // Escape collapses the topmost active Settings card (mirrors outside-click FILO pop).
  useEffect(() => {
    if (activeDomains.length === 0) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setActiveDomains((prev) => prev.slice(0, -1));
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [activeDomains.length, setActiveDomains]);

  if (!draftSettings) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center min-w-0 z-10 h-full relative overflow-hidden bg-transparent px-6 md:px-10 py-6 md:py-10">
        <OrbitalLoader
          size="md"
          title="Loading Settings..."
          subtitle="Reading hardware and model configurations"
          statusText="INITIALIZING CONFIG ENGINE"
        />
      </div>
    );
  }

  const hasSelection = activeDomains.length > 0;

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent select-none p-0 lg:p-6 lg:pb-[72px]">

      {/* ── Desktop & Tablet Hexagon/Grid Layout (>= 1024px) ────────────────── */}
      {!isCompact ? (
        <div ref={containerRef} className="flex-1 w-full grid grid-cols-12 grid-rows-6 gap-4 items-stretch relative min-h-0">

          {/* Dynamic SVG Overlay for Node-to-Card connections (rendered synchronously with active cards) */}
          <svg className="absolute inset-0 w-full h-full pointer-events-none z-10 overflow-visible">
            {DOMAINS.map((domain) => {
              if (!activeDomains.includes(domain.id)) return null;
              const line = lines[domain.id];
              if (!line) return null;

              const isVertical = domain.id === "persona" || domain.id === "appearance";
              let pathD = "";

              let nextX = line.x2;
              let nextY = line.y2;

              if (!isVertical) {
                const dx_mid = Math.abs(line.y2 - line.y1);
                if (domain.id === "models" || domain.id === "history") {
                  nextX = Math.min(line.x2, line.x1 + dx_mid);
                } else {
                  nextX = Math.max(line.x2, line.x1 - dx_mid);
                }
                nextY = line.y2;
              }

              const vx = nextX - line.x1;
              const vy = nextY - line.y1;
              const len = Math.sqrt(vx * vx + vy * vy) || 1;

              const startX = line.x1 + (vx / len) * 20;
              const startY = line.y1 + (vy / len) * 20;

              if (isVertical) {
                pathD = `M ${startX} ${startY} L ${line.x2} ${line.y2}`;
              } else {
                pathD = `M ${startX} ${startY} L ${nextX} ${line.y2} L ${line.x2} ${line.y2}`;
              }

              return (
                <g key={domain.id}>
                  <path
                    d={pathD}
                    fill="none"
                    stroke="var(--connection-glow)"
                    strokeWidth={4.5}
                  />
                  <path
                    d={pathD}
                    fill="none"
                    stroke="var(--connection-core)"
                    strokeWidth={1.5}
                  />
                  <path
                    d={pathD}
                    fill="none"
                    stroke="rgb(var(--accent))"
                    strokeWidth={1.75}
                    strokeLinecap="round"
                    strokeDasharray="4 560"
                    style={{
                      animation: `connector-flow 0.9s ease-out ${DOMAINS.indexOf(domain) * 0.12}s forwards`,
                    }}
                  />
                </g>
              );
            })}
          </svg>

          {/* Top-Left Slot (Col 1-4, Row 1-3) -> 10:00 (Interaction Card) */}
          <div className="col-start-1 col-span-4 row-start-1 row-span-3 flex items-end justify-end p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[5]} isActive={activeDomains.includes("interaction")} layoutMode={layoutMode} />
          </div>

          {/* Top-Center Slot (Col 5-8, Row 1-2) -> 12:00 (Persona Card) */}
          <div className="col-start-5 col-span-4 row-start-1 row-span-2 flex items-end justify-center p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[0]} isActive={activeDomains.includes("persona")} layoutMode={layoutMode} />
          </div>

          {/* Top-Right Slot (Col 9-12, Row 1-3) -> 2:00 (Models Card) */}
          <div className="col-start-9 col-span-4 row-start-1 row-span-3 flex items-end justify-start p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[1]} isActive={activeDomains.includes("models")} layoutMode={layoutMode} />
          </div>

          {/* Middle-Left Slot (Col 1-4, Row 4-6) -> 8:00 (Memory Card) */}
          <div className="col-start-1 col-span-4 row-start-4 row-span-3 flex items-start justify-end p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[4]} isActive={activeDomains.includes("memory")} layoutMode={layoutMode} />
          </div>

          {/* Middle-Center Slot (Col 5-8, Row 3-4) -> Radial Hub Center Grid Cell */}
          <div className="col-start-5 col-span-4 row-start-3 row-span-2 flex items-center justify-center p-2 z-20">
            <div
              className="relative shrink-0"
              style={{
                width: Math.max(radiusX, radiusY) * 2 + 100,
                height: Math.max(radiusX, radiusY) * 2 + 100,
              }}
            >
              <HubConnectors activeDomains={activeDomains} radiusX={radiusX} radiusY={radiusY} />

              {DOMAINS.map((domain) => (
                <RadialNode
                  key={domain.id}
                  domain={domain}
                  isActive={activeDomains.includes(domain.id)}
                  onSelect={handleSelect}
                  radiusX={radiusX}
                  radiusY={radiusY}
                />
              ))}

              <HubCenter onClick={handleCenterClick} hasActiveCards={hasSelection} />
            </div>
          </div>

          {/* Middle-Right Slot (Col 9-12, Row 4-6) -> 4:00 (History Card) */}
          <div className="col-start-9 col-span-4 row-start-4 row-span-3 flex items-start justify-start p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[2]} isActive={activeDomains.includes("history")} layoutMode={layoutMode} />
          </div>

          {/* Bottom-Center Slot (Col 5-8, Row 5-6) -> 6:00 (Appearance Card) */}
          <div className="col-start-5 col-span-4 row-start-5 row-span-2 flex items-start justify-center p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[3]} isActive={activeDomains.includes("appearance")} layoutMode={layoutMode} />
          </div>

        </div>
      ) : (
        /* ── Mobile & Compact Layout (Single vertical scroll list) ─────────── */
        <div className="flex-1 flex flex-col min-h-0 overflow-hidden w-full px-3.5 sm:px-5 pt-3.5 sm:pt-4">
          {/* Sticky Header - Standardized Across All Pages */}
          <div className="flex items-center justify-between pb-3 sm:pb-3.5 border-b border-[rgba(var(--accent),0.12)] mb-4 sm:mb-5 shrink-0">
            <div className="flex flex-col">
              <h1 className="text-[15px] sm:text-[16px] font-display font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
                {SETTINGS_COPY.settingsTitle}
              </h1>
              <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))] uppercase tracking-wider">
                {SETTINGS_COPY.settingsSubtitle}
              </span>
            </div>

            <div className="flex gap-1.5 items-center">
              {/* Auto-synced Toast Badge on Routine Saves */}
              <AnimatePresence>
                {!hasChanges && isAutoSaved && (
                  <motion.div
                    initial={{ opacity: 0, scale: 0.9 }}
                    animate={{ opacity: 1, scale: 1 }}
                    exit={{ opacity: 0, scale: 0.9 }}
                    className="flex items-center gap-1.5 px-2.5 py-1 rounded-xl bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.2)] text-[rgb(var(--accent))]"
                  >
                    <Check size={13} />
                    <span className="text-[11px] font-mono font-bold uppercase tracking-wider">
                      {SETTINGS_COPY.autoSynced}
                    </span>
                  </motion.div>
                )}
              </AnimatePresence>

              {/* Manual Changes Actions: Tick First (Commit) & Cross Second (Discard) */}
              {hasChanges && (
                <>
                  {/* Tick First: Commit Changes */}
                  <Tooltip
                    label={
                      isMissingCloudKey
                        ? SETTINGS_COPY.apiKeyRequired
                        : requiresRestart
                        ? SETTINGS_COPY.applyAndReload
                        : SETTINGS_COPY.saveChanges
                    }
                    side="bottom"
                  >
                    <button
                      onClick={() => commitChanges()}
                      disabled={isMissingCloudKey}
                      className={cn(
                        "p-1.5 rounded-xl border transition-all cursor-pointer flex items-center justify-center shrink-0",
                        isMissingCloudKey
                          ? "border-[rgba(var(--border),0.1)] bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/30 cursor-not-allowed"
                          : "border-emerald-500/30 bg-emerald-500/15 text-emerald-400 hover:bg-emerald-500/25"
                      )}
                      aria-label={SETTINGS_COPY.saveChanges}
                    >
                      <Check size={14} />
                    </button>
                  </Tooltip>

                  {/* Cross Second: Discard Changes */}
                  <Tooltip label={SETTINGS_COPY.discardChanges} side="bottom">
                    <button
                      onClick={() => discardChanges()}
                      className="p-1.5 rounded-xl border border-rose-500/20 bg-rose-500/10 text-rose-400 hover:bg-rose-500/20 transition-all cursor-pointer flex items-center justify-center shrink-0"
                      aria-label={SETTINGS_COPY.discardChanges}
                    >
                      <X size={14} />
                    </button>
                  </Tooltip>
                </>
              )}

              {/* Restore Defaults with confirm state */}
              <Tooltip
                label={isMobileConfirmRestore ? "Tap again to confirm reset" : SETTINGS_COPY.restoreDefaults}
                side="bottom"
              >
                <button
                  onClick={() => {
                    if (isMobileConfirmRestore) {
                      restoreDefaults();
                      setIsMobileConfirmRestore(false);
                    } else {
                      setIsMobileConfirmRestore(true);
                      setTimeout(() => setIsMobileConfirmRestore(false), 4000);
                    }
                  }}
                  className={cn(
                    "p-1.5 rounded-xl border transition-all duration-300 cursor-pointer flex items-center justify-center shrink-0",
                    isMobileConfirmRestore
                      ? "bg-[rgba(var(--danger),0.18)] border-[rgb(var(--danger))]/60 text-[rgb(var(--danger))]"
                      : "bg-[rgb(var(--foreground))]/[0.03] border-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground-muted))] hover:bg-[rgb(var(--accent))]/10 hover:text-[rgb(var(--accent))]"
                  )}
                  aria-label={SETTINGS_COPY.restoreDefaults}
                >
                  <RotateCcw size={14} />
                </button>
              </Tooltip>
            </div>
          </div>

          <div className="flex-1 w-full overflow-y-auto custom-scrollbar pb-[95px] space-y-5 sm:space-y-6 animate-fade-in pr-0.5">
            {[...DOMAINS].sort((a, b) => {
              const order = ["interaction", "models", "appearance", "memory", "history", "persona"];
              return order.indexOf(a.id) - order.indexOf(b.id);
            }).map((domain) => (
              <div key={domain.id} className="w-full glass-card rounded-2xl p-4 sm:p-5">
                <ErrorBoundary name={`SettingsMobile:${domain.id}`}>
                  <DomainContent domain={domain.id} layoutMode="small" />
                </ErrorBoundary>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
