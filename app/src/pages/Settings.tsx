import { useMemo, useEffect, memo, Suspense, lazy } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { ErrorBoundary, OrbitalLoader } from "@/shared/components/common";
import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId } from "@/data/settingsCopy";
import { SETTINGS_COPY } from "@/data/settingsCopy";
import { useRegisterPageDrawer } from "@/shared/context/PageDrawerContext";

// Loader functions for eager prewarming
const loadPersona = () => import("@/shared/components/settings/persona/PersonaCard").then(m => ({ default: m.PersonaCard }));
const loadModels = () => import("@/shared/components/settings/models/ModelsCard").then(m => ({ default: m.ModelsCard }));
const loadRealtime = () => import("@/shared/components/settings/realtime/RealtimeCard").then(m => ({ default: m.RealtimeCard }));
const loadWorkingMemory = () => import("@/shared/components/settings/working_memory/WorkingMemoryCard").then(m => ({ default: m.WorkingMemoryCard }));
const loadPersonalMemory = () => import("@/shared/components/settings/personal_memory/PersonalMemoryCard").then(m => ({ default: m.PersonalMemoryCard }));
const loadAppearance = () => import("@/shared/components/settings/appearance/AppearanceCard").then(m => ({ default: m.AppearanceCard }));
const loadInteraction = () => import("@/shared/components/settings/interaction/InteractionCard").then(m => ({ default: m.InteractionCard }));

// Lazy-loaded domain card components
const PersonaCard = lazy(loadPersona);
const ModelsCard = lazy(loadModels);
const RealtimeCard = lazy(loadRealtime);
const WorkingMemoryCard = lazy(loadWorkingMemory);
const PersonalMemoryCard = lazy(loadPersonalMemory);
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
          case "working_memory":
            return <WorkingMemoryCard layoutMode={layoutMode} />;
          case "personal_memory":
            return <PersonalMemoryCard layoutMode={layoutMode} />;
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
import { BottomDockFeather } from "@/shared/ui/BottomDockFeather";
import {
  HubCenter,
  SettingsConnectorsOverlay,
} from "@/shared/components/settings/SettingsVisualConnectors";

import { SettingsCardWrapper } from "@/shared/components/settings/SettingsCardWrapper";
import { useSettingsPage } from "@/shared/hooks/useSettingsPage";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";

export const Settings: React.FC = () => {
  useMemoryTrace("Settings");
  const draftSettings = useSettingsStore((s) => s.draftSettings);

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

  const drawerHandlers = useMemo(() => ({
    open: () => setActiveDomains(DOMAINS.map((d) => d.id)),
    close: () => setActiveDomains([]),
  }), [setActiveDomains]);
  useRegisterPageDrawer(drawerHandlers);

  // Escape collapses the topmost active Settings card (mirrors outside-click FILO pop).
  useEffect(() => {
    if (activeDomains.length === 0) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setActiveDomains((prev) => prev.slice(0, -1));
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [activeDomains.length, setActiveDomains]);

  // Eagerly prewarm the 7 card chunks and remote model catalog after first paint
  // so opening radial cards is completely warm with zero cold fetch delay.
  useEffect(() => {
    const idle = window.requestIdleCallback ?? ((fn: () => void) => setTimeout(fn, 400));
    const id = idle(() => {
      void Promise.all([
        loadPersona(),
        loadModels(),
        loadRealtime(),
        loadWorkingMemory(),
        loadPersonalMemory(),
        loadAppearance(),
        loadInteraction(),
      ]).catch(() => {});

      // Prewarm remote LLM catalog into store cache
      void useSettingsStore.getState().loadRemoteModels().catch(() => {});
    });
    return () => {
      if (window.cancelIdleCallback && typeof id === "number") window.cancelIdleCallback(id);
    };
  }, []);

  if (!draftSettings) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center min-w-0 z-10 h-full relative overflow-hidden bg-transparent px-6 md:px-10 py-6 md:py-10">
        <OrbitalLoader size="md" />
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
          <div className="col-start-1 col-span-4 row-start-1 row-span-3 flex items-end justify-end p-2 relative z-10">
            <SettingsCardWrapper domain={DOMAINS[5]} isActive={activeDomains.includes("interaction")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[5].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

          {/* Top-Center Slot (Col 5-8, Row 1-2) -> 12:00 (Persona Card) */}
          <div className="col-start-5 col-span-4 row-start-1 row-span-2 flex items-end justify-center p-2 relative z-10">
            <SettingsCardWrapper domain={DOMAINS[0]} isActive={activeDomains.includes("persona")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[0].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

          {/* Top-Right Slot (Col 9-12, Row 1-3) -> 2:00 (Models Card) */}
          <div className="col-start-9 col-span-4 row-start-1 row-span-3 flex items-end justify-start p-2 relative z-10">
            <SettingsCardWrapper domain={DOMAINS[1]} isActive={activeDomains.includes("models")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[1].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

          {/* Middle-Left Slot (Col 1-4, Row 4-6) -> 8:00 (Personal Memory Card) */}
          <div className="col-start-1 col-span-4 row-start-4 row-span-3 flex items-start justify-end p-2 relative z-10">
            <SettingsCardWrapper domain={DOMAINS[4]} isActive={activeDomains.includes("personal_memory")} layoutMode={layoutMode}>
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

          {/* Middle-Right Slot (Col 9-12, Row 4-6) -> 4:00 (Working Memory Card) */}
          <div className="col-start-9 col-span-4 row-start-4 row-span-3 flex items-start justify-start p-2 relative z-10">
            <SettingsCardWrapper domain={DOMAINS[2]} isActive={activeDomains.includes("working_memory")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[2].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

          {/* Bottom-Center Slot (Col 5-8, Row 5-6) -> 6:00 (Appearance Card) */}
          <div className="col-start-5 col-span-4 row-start-5 row-span-2 flex items-start justify-center p-2 relative z-10">
            <SettingsCardWrapper domain={DOMAINS[3]} isActive={activeDomains.includes("appearance")} layoutMode={layoutMode}>
              <DomainContent domain={DOMAINS[3].id} layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

        </div>
      ) : (
        /* ── Mobile & Compact Layout (Single vertical scroll list) ─────────── */
        <div className="flex-1 flex flex-col min-h-0 overflow-hidden w-full px-4 sm:px-5 pt-4">
          {/* Sticky Header - Clean Title & Subtitle without top-right button conflicts */}
          <div className="flex items-start justify-between pb-3 sm:pb-3.5 border-b border-[rgba(var(--accent),0.12)] mb-4 sm:mb-5 shrink-0">
            <div className="flex flex-col">
              <h1 className="text-[15px] sm:text-[16px] font-display font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
                {SETTINGS_COPY.settingsTitle}
              </h1>
              <span className="text-[11px] font-mono font-bold text-[rgb(var(--accent))] uppercase tracking-wider">
                {SETTINGS_COPY.settingsSubtitle}
              </span>
            </div>
          </div>

          <div className="flex-1 w-full relative overflow-hidden flex flex-col min-h-0">
            <div className="flex-1 w-full overflow-y-auto custom-scrollbar pb-[95px] space-y-5 sm:space-y-6 animate-fade-in pr-0.5">
              {[...DOMAINS].sort((a, b) => {
                const order = ["interaction", "models", "appearance", "working_memory", "personal_memory", "persona"];
                return order.indexOf(a.id) - order.indexOf(b.id);
              }).map((domain) => (
                <div key={domain.id} className="w-full">
                  <SettingsCardWrapper domain={domain} isActive={true} layoutMode="small">
                    <div className="w-full glass-card rounded-2xl p-4 sm:p-5">
                      <ErrorBoundary name={`SettingsMobile:${domain.id}`}>
                        <DomainContent domain={domain.id} layoutMode="small" />
                      </ErrorBoundary>
                    </div>
                  </SettingsCardWrapper>
                </div>
              ))}
            </div>
            {/* Selective bottom dissolve for small-screen settings card list */}
            <BottomDockFeather className="absolute bottom-0 left-0 right-0 h-[80px] pointer-events-none z-10" />
          </div>
        </div>
      )}
    </div>
  );
};
