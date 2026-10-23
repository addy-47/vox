import { useMemo, useEffect, useCallback, memo, Suspense, lazy } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { ErrorBoundary, OrbitalLoader } from "@/shared/components/common";
import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId } from "@/data/settingsCopy";
import { useRegisterPageDrawer } from "@/shared/context/PageDrawerContext";
import {
  loadAppearance,
  loadInteraction,
  loadLanguage,
  loadMemory,
  loadPersona,
  loadRealtime,
  loadSpeech,
  preloadSettingsCardChunks,
} from "@/shared/components/settings/settingsCardChunks";

// Lazy-loaded domain card components
const PersonaCard = lazy(loadPersona);
const SpeechCard = lazy(loadSpeech);
const LanguageCard = lazy(loadLanguage);
const RealtimeCard = lazy(loadRealtime);
const MemoryCard = lazy(loadMemory);
const AppearanceCard = lazy(loadAppearance);
const InteractionCard = lazy(loadInteraction);

import { SettingsCardSkeleton } from "@/shared/components/settings/SettingsCardSkeleton";

/** Slot lookups by domain id. Slots must not depend on array positions. */
const domainById = (id: DomainId) => {
  const match = DOMAINS.find((d) => d.id === id);
  if (!match) throw new Error(`Unknown settings domain: ${id}`);
  return match;
};

/** Compact single-column order, derived from the shared slot map. */
const MOBILE_ORDERED_DOMAINS = [...DOMAINS].sort(
  (a, b) => SETTINGS_DOMAIN_SLOTS[a.id].mobileOrder - SETTINGS_DOMAIN_SLOTS[b.id].mobileOrder
);

const DomainContent = memo(({ domain, layoutMode }: { domain: DomainId; layoutMode?: "full-max" | "full-min" | "small" }) => {
  const isRealtime = useSettingsStore((s) => s.draftSettings?.interaction.pipeline_mode === "realtime");
  return (
    <Suspense fallback={<SettingsCardSkeleton layoutMode={layoutMode} domain={domain} />}>
      {(() => {
        switch (domain) {
          case "persona":
            return <PersonaCard layoutMode={layoutMode} />;
          case "speech":
            return isRealtime ? <RealtimeCard layoutMode={layoutMode} /> : <SpeechCard layoutMode={layoutMode} />;
          case "language":
            return <LanguageCard layoutMode={layoutMode} />;
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

import { RadialHub, SettingsConnectorsOverlay } from "@/shared/components/settings/RadialHub";
import { getStackSize } from "@/shared/lib/overlayStack";
import { BottomDockFeather } from "@/shared/ui/BottomDockFeather";
import { SETTINGS_DOMAIN_SLOTS } from "@/shared/components/settings/shared/settingsLayout";

import { SettingsCardWrapper } from "@/shared/components/settings/SettingsCardWrapper";
import { SettingsSideCarouselCard } from "@/shared/components/settings/SettingsSideCarouselCard";
import { useSettingsPage } from "@/shared/hooks/useSettingsPage";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";

const preloadSettingsCards = () =>
  Promise.all([
    preloadSettingsCardChunks(),
    useSettingsStore.getState().loadRemoteModels(),
  ]);

export const Settings: React.FC = () => {
  useMemoryTrace("Settings");
  const isLoaded = useSettingsStore((s) => s.draftSettings !== null);

  const {
    containerRef,
    activeDomains,
    isCompact,
    isSideCarousel,
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

  const renderDomainContent = useCallback(
    (domain: DomainId, mode?: "full-max" | "full-min" | "small") => (
      <DomainContent domain={domain} layoutMode={mode} />
    ),
    []
  );

  // Escape collapses the topmost active Settings card (mirrors outside-click FILO pop).
  // Overlays registered on the global stack (modals, popovers) consume Escape
  // first; the card only collapses when no overlay owns the key.
  useEffect(() => {
    if (activeDomains.length === 0) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || getStackSize() > 0) return;
      setActiveDomains((prev) => prev.slice(0, -1));
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [activeDomains.length, setActiveDomains]);

  // Eagerly prewarm the 7 card chunks and remote model catalog after first paint
  // so opening radial cards is completely warm with zero cold fetch delay.
  useEffect(() => {
    let cancelled = false;
    const run = () => {
      if (cancelled) return;
      void preloadSettingsCards().catch((error: unknown) => {
        console.debug("[Settings] Failed to prewarm settings data:", error);
      });
    };
    // WebKitGTK may lack requestIdleCallback; feature-detect instead of
    // assuming the API surface.
    const hasIdle = "requestIdleCallback" in window;
    const handle: number = hasIdle
      ? window.requestIdleCallback(() => run())
      : window.setTimeout(run, 400);
    return () => {
      cancelled = true;
      if (hasIdle) window.cancelIdleCallback(handle);
      else window.clearTimeout(handle);
    };
  }, []);

  if (!isLoaded) {
    // The loader stands where the hub center will be, not in the middle of
    // the page. It reuses the hub's own slot, so nothing jumps sideways when
    // the settings arrive.
    return (
      <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent select-none p-0 lg:px-6 lg:pt-2 lg:pb-[72px]">
        {isCompact ? (
          <div className="flex-1 flex items-center justify-center">
            <OrbitalLoader size="md" />
          </div>
        ) : (
          <div className="flex-1 w-full grid grid-cols-12 grid-rows-6 gap-4 items-stretch relative min-h-0">
            <div className="col-start-5 col-span-4 row-start-3 row-span-2 flex items-center justify-center p-2 z-20">
              <OrbitalLoader size="md" />
            </div>
          </div>
        )}
      </div>
    );
  }

  const hasSelection = activeDomains.length > 0;

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent select-none p-0 lg:px-6 lg:pt-2 lg:pb-[72px]">


      {/* ── Desktop & Tablet Hexagon/Grid Layout (>= 1024px) ────────────────── */}
      {!isCompact ? (
        <div ref={containerRef} className="flex-1 w-full grid grid-cols-12 grid-rows-6 gap-4 items-stretch relative min-h-0 overflow-visible">
          {/* Restore-defaults control lives in AppHeader (right slot); page-level duplicate removed. */}

          {/* Dynamic SVG Overlay for Node-to-Card connections (rendered synchronously with active cards) */}
          <SettingsConnectorsOverlay
            domains={DOMAINS}
            activeDomains={activeDomains}
            lines={lines}
            isSideCarousel={isSideCarousel}
          />

          {/* Side Cards: Unified Carousel in constrained viewports or independent slots in wide viewports */}
          {isSideCarousel ? (
            <>
              {/* Left Side Carousel Card -> 10:00 (Interaction) & 8:00 (Speech) */}
              <div
                id="card-left-carousel"
                className="col-start-1 col-span-4 row-start-1 row-span-6 flex items-center justify-end p-2 relative z-10 overflow-visible"
              >
                <SettingsSideCarouselCard
                  side="left"
                  activeDomains={activeDomains}
                  layoutMode={layoutMode}
                  renderDomainContent={renderDomainContent}
                />
              </div>

              {/* Right Side Carousel Card -> 2:00 (Memory) & 4:00 (Language) */}
              <div
                id="card-right-carousel"
                className="col-start-9 col-span-4 row-start-1 row-span-6 flex items-center justify-start p-2 relative z-10 overflow-visible"
              >
                <SettingsSideCarouselCard
                  side="right"
                  activeDomains={activeDomains}
                  layoutMode={layoutMode}
                  renderDomainContent={renderDomainContent}
                />
              </div>
            </>
          ) : (
            <>
              {/* Top-Left Slot (Col 1-4, Row 1-3) -> 10:00 (Interaction Card) */}
              <div className="col-start-1 col-span-4 row-start-1 row-span-3 flex items-end justify-end p-2 relative z-10">
                <SettingsCardWrapper domain={domainById("interaction")} isActive={activeDomains.includes("interaction")} layoutMode={layoutMode}>
                  <DomainContent domain="interaction" layoutMode={layoutMode} />
                </SettingsCardWrapper>
              </div>

              {/* Top-Right Slot (Col 9-12, Row 1-3) -> 2:00 (Memory Card) */}
              <div className="col-start-9 col-span-4 row-start-1 row-span-3 flex items-end justify-start p-2 relative z-10">
                <SettingsCardWrapper domain={domainById("memory")} isActive={activeDomains.includes("memory")} layoutMode={layoutMode}>
                  <DomainContent domain="memory" layoutMode={layoutMode} />
                </SettingsCardWrapper>
              </div>

              {/* Middle-Left Slot (Col 1-4, Row 4-6) -> 8:00 (Speech Card) */}
              <div className="col-start-1 col-span-4 row-start-4 row-span-3 flex items-start justify-end p-2 relative z-10">
                <SettingsCardWrapper domain={domainById("speech")} isActive={activeDomains.includes("speech")} layoutMode={layoutMode}>
                  <DomainContent domain="speech" layoutMode={layoutMode} />
                </SettingsCardWrapper>
              </div>

              {/* Middle-Right Slot (Col 9-12, Row 4-6) -> 4:00 (Language Card) */}
              <div className="col-start-9 col-span-4 row-start-4 row-span-3 flex items-start justify-start p-2 relative z-10">
                <SettingsCardWrapper domain={domainById("language")} isActive={activeDomains.includes("language")} layoutMode={layoutMode}>
                  <DomainContent domain="language" layoutMode={layoutMode} />
                </SettingsCardWrapper>
              </div>
            </>
          )}

          {/* Top-Center Slot (Col 5-8, Row 1-2) -> 12:00 (Persona Card - elevated to avoid sibling collision & unclipped) */}
          <div className="col-start-5 col-span-4 row-start-1 row-span-2 flex items-start justify-center p-2 relative z-10 overflow-visible">
            <SettingsCardWrapper domain={domainById("persona")} isActive={activeDomains.includes("persona")} layoutMode={layoutMode}>
              <DomainContent domain="persona" layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

          {/* Middle-Center Slot (Col 5-8, Row 3-4) -> Radial Hub Center Grid Cell */}
          <div className="col-start-5 col-span-4 row-start-3 row-span-2 flex items-center justify-center p-2 z-20">
            <RadialHub
              activeDomains={activeDomains}
              radiusX={radiusX}
              radiusY={radiusY}
              onSelect={handleSelect}
              onCenterClick={handleCenterClick}
              hasSelection={hasSelection}
            />
          </div>

          {/* Bottom-Center Slot (Col 5-8, Row 5-6) -> 6:00 (Appearance Card) */}
          <div className="col-start-5 col-span-4 row-start-5 row-span-2 flex items-start justify-center p-2 relative z-10">
            <SettingsCardWrapper domain={domainById("appearance")} isActive={activeDomains.includes("appearance")} layoutMode={layoutMode}>
              <DomainContent domain="appearance" layoutMode={layoutMode} />
            </SettingsCardWrapper>
          </div>

        </div>
      ) : (
        /* ── Mobile & Compact Layout (Single vertical scroll list) ─────────── */
        <div className="flex-1 flex flex-col min-h-0 overflow-hidden w-full pt-1.5 sm:pt-2">
          <div className="flex-1 w-full relative overflow-hidden flex flex-col min-h-0">
            <div className="flex-1 w-full overflow-y-auto custom-scrollbar pb-[130px] space-y-4 px-3 sm:px-4.5">
              {MOBILE_ORDERED_DOMAINS.map((domain) => (
                <div key={domain.id} className="w-full">
                  <SettingsCardWrapper domain={domain} isActive={true} layoutMode="small">
                    <div className="w-full glass-card rounded-2xl p-3.5 sm:p-4.5">
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
