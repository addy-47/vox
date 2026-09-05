import { useState, useMemo, useEffect, memo, Suspense, lazy } from "react";
import { RotateCcw, Check, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore } from "@/store/settingsStore";
import { ErrorBoundary, OrbitalLoader, TopRightCluster } from "@/shared/components/common";
import { AnimatePresence, motion } from "framer-motion";
import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId } from "@/data/settingsCopy";
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
import {
  HubCenter,
  SettingsConnectorsOverlay,
} from "@/shared/components/settings/SettingsVisualConnectors";

import { SettingsCardWrapper } from "@/shared/components/settings/SettingsCardWrapper";
import { useSettingsPage } from "@/shared/hooks/useSettingsPage";

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
  // TODO: re-enable when STT cloud config desk exists (LlmConfigDesk.tsx placeholder at :364).
  const isCloudSttMissingKey = false;
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
          title={SETTINGS_COPY.loadingSettings}
          subtitle={SETTINGS_COPY.loadingHint}
          statusText={SETTINGS_COPY.initializingEngine}
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
          <SettingsConnectorsOverlay
            domains={DOMAINS}
            activeDomains={activeDomains}
            lines={lines}
          />

          {/* Top-Left Slot (Col 1-4, Row 1-3) -> 10:00 (Interaction Card) */}
          <div className="col-start-1 col-span-4 row-start-1 row-span-3 flex items-end justify-end p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[5]} isActive={activeDomains.includes("interaction")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[5].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

          {/* Top-Center Slot (Col 5-8, Row 1-2) -> 12:00 (Persona Card) */}
          <div className="col-start-5 col-span-4 row-start-1 row-span-2 flex items-end justify-center p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[0]} isActive={activeDomains.includes("persona")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[0].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

          {/* Top-Right Slot (Col 9-12, Row 1-3) -> 2:00 (Models Card) */}
          <div className="col-start-9 col-span-4 row-start-1 row-span-3 flex items-end justify-start p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[1]} isActive={activeDomains.includes("models")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[1].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

          {/* Middle-Left Slot (Col 1-4, Row 4-6) -> 8:00 (Memory Card) */}
          <div className="col-start-1 col-span-4 row-start-4 row-span-3 flex items-start justify-end p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[4]} isActive={activeDomains.includes("memory")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[4].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
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
            <SettingsCardWrapper domain={DOMAINS[2]} isActive={activeDomains.includes("history")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[2].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

          {/* Bottom-Center Slot (Col 5-8, Row 5-6) -> 6:00 (Appearance Card) */}
          <div className="col-start-5 col-span-4 row-start-5 row-span-2 flex items-start justify-center p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[3]} isActive={activeDomains.includes("appearance")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[3].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
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

              {/* Help & Notifications */}
              <TopRightCluster deepLink="settings:overview" className="pointer-events-auto" />

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
