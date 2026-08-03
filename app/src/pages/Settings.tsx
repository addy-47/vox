import { useState, useMemo, useEffect, memo, Suspense } from "react";
import { RotateCcw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";
import { useSettingsStore } from "@/store/settingsStore";
import { GlassSkeleton, ErrorBoundary } from "@/shared/components/common";
import { AnimatePresence, motion } from "framer-motion";

import { PersonaCard } from "@/shared/components/settings/persona/PersonaCard";
import { ModelsCard } from "@/shared/components/settings/models/ModelsCard";
import { RealtimeCard } from "@/shared/components/settings/realtime/RealtimeCard";
import { TrayCard } from "@/shared/components/settings/tray/TrayCard";
import { MemoryCard } from "@/shared/components/settings/memory/MemoryCard";
import { AppearanceCard } from "@/shared/components/settings/appearance/AppearanceCard";
import { InteractionCard } from "@/shared/components/settings/interaction/InteractionCard";

import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId, type SettingsDomain as Domain } from "@/data/settingsDomains";

// ─── Radial hub geometry ──────────────────────────────────────────────────────


// ─── Domain content map ───────────────────────────────────────────────────────

const DomainContent = memo(({ domain, layoutMode }: { domain: DomainId; layoutMode?: "full-max" | "full-min" | "small" }) => {
  const isRealtime = useSettingsStore((s) => s.draftSettings?.interaction?.pipeline_mode === "realtime");
  return (
    <Suspense fallback={<GlassSkeleton variant="card" />}>
      {(() => {
        switch (domain) {
          case "persona":
            return <PersonaCard layoutMode={layoutMode} />;
          case "models":
            return isRealtime ? <RealtimeCard layoutMode={layoutMode} /> : <ModelsCard layoutMode={layoutMode} />;
          case "tray":
            return <TrayCard layoutMode={layoutMode} />;
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

// ─── Hub center ───────────────────────────────────────────────────────────────

interface HubCenterProps {
  onClick: () => void;
  hasActiveCards: boolean;
}

const HubCenter = memo(({ onClick, hasActiveCards }: HubCenterProps) => (
  <button
    id="center-node"
    onClick={onClick}
    className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-16 h-16 rounded-full flex items-center justify-center transition-all duration-400 z-30 cursor-pointer"
    aria-label={hasActiveCards ? "Clear all selections" : "Configure all domains"}
  >
    {/* Layer 1 (outermost): A circle ~52px diameter */}
    <div
      className="absolute rounded-full border border-dashed transition-all duration-400"
      style={{
        width: "52px",
        height: "52px",
        borderColor: `rgba(var(--accent), ${hasActiveCards ? 0.35 : 0.20})`,
        animation: "border-rotate 18s linear infinite",
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
));
HubCenter.displayName = "HubCenter";

const hasCardChanges = (domainId: DomainId, _settings: any, _draftSettings: any) => {
  return useSettingsStore.getState().isDomainDirty(domainId);
};

import { useSettingsPage } from "@/shared/hooks/useSettingsPage";

// Custom styles to seamlessly merge the card and the footer tray
const unsavedStyles = `
  .has-unsaved-changes > div:first-child {
    border-bottom-left-radius: 0px !important;
    border-bottom-right-radius: 0px !important;
    border-bottom-color: rgba(var(--accent), 0.25) !important;
  }
  [data-theme='dark'] .has-unsaved-changes > div:first-child {
    border-bottom-color: rgba(var(--accent), 0.1) !important;
  }
`;

interface SettingsCardWrapperProps {
  domain: Domain;
  isActive: boolean;
  layoutMode: "full-max" | "full-min" | "small";
}

const SettingsCardWrapper: React.FC<SettingsCardWrapperProps> = memo(({ domain, isActive, layoutMode }) => {
  const { settings, draftSettings, commitChanges } = useSettings();
  const [showRestartConfirm, setShowRestartConfirm] = useState(false);
  
  const hasChanges = useMemo(() => {
    return hasCardChanges(domain.id, settings, draftSettings);
  }, [domain.id, settings, draftSettings]);

  const requiresRestart = useMemo(() => {
    if (!settings || !draftSettings) return false;
    if (domain.id === "models") {
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      if (isRealtime) return false;
      return (
        settings.vad.vad_backend !== draftSettings.vad.vad_backend ||
        settings.asr.model !== draftSettings.asr.model ||
        settings.llm.model !== draftSettings.llm.model ||
        settings.llm.ctx_size !== draftSettings.llm.ctx_size ||
        settings.llm.threads !== draftSettings.llm.threads ||
        settings.tts.voice !== draftSettings.tts.voice ||
        settings.llm.provider?.model !== draftSettings.llm.provider?.model
      );
    }
    return false;
  }, [domain.id, settings, draftSettings]);

  useEffect(() => {
    if (!hasChanges) {
      setShowRestartConfirm(false);
    }
  }, [hasChanges]);

  const handleSave = () => {
    if (requiresRestart && !showRestartConfirm) {
      setShowRestartConfirm(true);
    } else {
      commitChanges();
      setShowRestartConfirm(false);
    }
  };

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
          <style>{unsavedStyles}</style>
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

            {/* Dynamic Save/Discard Expanded Footer */}
            {hasChanges && (layoutMode === "full-max" || layoutMode === "full-min") && (
              <motion.div
                initial={{ opacity: 0, height: 0, y: -4 }}
                animate={{ opacity: 1, height: "auto", y: 0 }}
                exit={{ opacity: 0, height: 0, y: -4 }}
                className="w-full p-3 px-5 rounded-b-[1.25rem] rounded-t-none glass-card border-t-0 flex items-center justify-between overflow-hidden text-[11px]"
              >
                {showRestartConfirm ? (
                  <>
                    <span className="font-bold uppercase tracking-wider text-yellow-500 animate-pulse">Restart Required. Confirm?</span>
                    <div className="flex gap-2">
                      <button
                        onClick={handleSave}
                        className="px-3.5 py-1 rounded-lg bg-yellow-500 text-black text-[11px] font-black uppercase tracking-wider hover:brightness-110 active:scale-95 transition-all"
                      >
                        Yes
                      </button>
                      <button
                        onClick={() => setShowRestartConfirm(false)}
                        className="px-3 py-1 rounded-lg bg-[rgba(var(--foreground),0.08)] dark:bg-[rgba(var(--foreground),0.15)] text-[rgb(var(--foreground))] border border-[rgba(var(--accent),0.15)] text-[11px] font-bold uppercase tracking-wider hover:bg-[rgb(var(--accent))]/10 transition-colors"
                      >
                        No
                      </button>
                    </div>
                  </>
                ) : (
                  <>
                    <span className="font-bold uppercase tracking-wider text-[rgb(var(--accent))]">Unsaved Changes</span>
                    <div className="flex gap-2">
                      <button
                        onClick={handleSave}
                        className="px-3 py-1 rounded-lg bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[11px] font-bold uppercase tracking-wider hover:scale-[1.02] active:scale-95 transition-all"
                      >
                        Save
                      </button>
                      <button
                        onClick={() => useSettingsStore.getState().discardDomainChanges(domain.id)}
                        className="px-3 py-1 rounded-lg bg-[rgba(var(--foreground),0.08)] dark:bg-[rgba(var(--foreground),0.15)] text-[rgb(var(--foreground))] border border-[rgba(var(--accent),0.15)] text-[11px] font-bold uppercase tracking-wider hover:bg-[rgb(var(--accent))]/10 transition-colors"
                      >
                        Discard
                      </button>
                    </div>
                  </>
                )}
              </motion.div>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
});
SettingsCardWrapper.displayName = "SettingsCardWrapper";

export const Settings: React.FC = () => {
  const { draftSettings, commitChanges, discardChanges, hasChanges, restoreDefaults } = useSettings();
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
  } = useSettingsPage();



  if (!draftSettings) {
    return (
      <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent px-6 md:px-10 py-6 md:py-10">
        <div className="w-full max-w-md mx-auto space-y-6">
          <GlassSkeleton variant="card" />
          <GlassSkeleton variant="card" />
        </div>
      </div>
    );
  }

  const hasSelection = activeDomains.length > 0;

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent select-none p-6">
      
      {/* ── Desktop & Tablet Hexagon/Grid Layout (>= 1024px) ────────────────── */}
      {!isCompact ? (
        <div ref={containerRef} className="flex-1 w-full grid grid-cols-12 grid-rows-6 gap-4 items-stretch relative min-h-0">
          
          {/* Dynamic SVG Overlay for Node-to-Card connections */}
          <svg className="absolute inset-0 w-full h-full pointer-events-none z-10 overflow-visible">
            {DOMAINS.map((domain) => {
              const line = lines[domain.id];
              if (!line) return null;

              const isVertical = domain.id === "persona" || domain.id === "appearance";
              let pathD = "";

              // Determine the next point the line goes to after the start point (line.x1, line.y1)
              let nextX = line.x2;
              let nextY = line.y2;

              if (!isVertical) {
                const dx_mid = Math.abs(line.y2 - line.y1);
                if (domain.id === "models" || domain.id === "tray") {
                  // Card is on the right
                  nextX = Math.min(line.x2, line.x1 + dx_mid);
                } else {
                  // Card is on the left (memory, interaction)
                  nextX = Math.max(line.x2, line.x1 - dx_mid);
                }
                nextY = line.y2;
              }

              // Compute the unit vector from node center to the next point
              const vx = nextX - line.x1;
              const vy = nextY - line.y1;
              const len = Math.sqrt(vx * vx + vy * vy) || 1;

              // Offset the start point by the node's radius (20px)
              const startX = line.x1 + (vx / len) * 20;
              const startY = line.y1 + (vy / len) * 20;

              if (isVertical) {
                pathD = `M ${startX} ${startY} L ${line.x2} ${line.y2}`;
              } else {
                pathD = `M ${startX} ${startY} L ${nextX} ${line.y2} L ${line.x2} ${line.y2}`;
              }

              return (
                <g key={domain.id}>
                  {/* Outer glow line */}
                  <path
                    d={pathD}
                    fill="none"
                    stroke="var(--connection-glow)"
                    strokeWidth={4.5}
                  />
                  {/* Sharp core line */}
                  <path
                    d={pathD}
                    fill="none"
                    stroke="var(--connection-core)"
                    strokeWidth={1.5}
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
              {/* Hub connector lines */}
              <HubConnectors activeDomains={activeDomains} radiusX={radiusX} radiusY={radiusY} />

              {/* Domain nodes */}
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

              {/* Center node */}
              <HubCenter onClick={handleCenterClick} hasActiveCards={hasSelection} />
            </div>
          </div>

          {/* Middle-Right Slot (Col 9-12, Row 4-6) -> 4:00 (Tray Card) */}
          <div className="col-start-9 col-span-4 row-start-4 row-span-3 flex items-start justify-start p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[2]} isActive={activeDomains.includes("tray")} layoutMode={layoutMode} />
          </div>

          {/* Bottom-Center Slot (Col 5-8, Row 5-6) -> 6:00 (Appearance Card) */}
          <div className="col-start-5 col-span-4 row-start-5 row-span-2 flex items-start justify-center p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[3]} isActive={activeDomains.includes("appearance")} layoutMode={layoutMode} />
          </div>

        </div>
      ) : (
        /* ── Mobile & Compact Layout (Single vertical scroll list) ─────────── */
        <div className="flex-1 flex flex-col min-h-0 overflow-hidden w-full">
          {/* Sticky Header - Always Visible */}
          <div className="flex items-center justify-between pb-3 border-b border-[rgba(var(--accent),0.12)] mb-4 px-1 shrink-0">
            <span className="text-[10px] font-black uppercase tracking-[0.15em] text-[rgb(var(--foreground))]/75">
              System Settings
            </span>
            <div className="flex gap-2 items-center">
              <button
                onClick={() => commitChanges()}
                disabled={!hasChanges}
                className={cn(
                  "px-3.5 py-1.5 rounded-xl text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                  hasChanges
                    ? "hover:scale-[1.02] active:scale-95 cursor-pointer"
                    : "bg-[rgb(var(--foreground))]/5 border border-[rgba(var(--border),0.08)] text-[rgb(var(--foreground-muted))]/40 cursor-not-allowed"
                )}
                style={
                  hasChanges
                    ? {
                        backgroundColor: "rgb(var(--accent))",
                        color: "rgb(var(--accent-foreground))",
                      }
                    : undefined
                }
              >
                Save
              </button>
              <button
                onClick={() => discardChanges()}
                disabled={!hasChanges}
                className={cn(
                  "px-3.5 py-1.5 rounded-xl border text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                  hasChanges
                    ? "bg-[rgba(var(--foreground),0.08)] dark:bg-[rgba(var(--foreground),0.15)] text-[rgb(var(--foreground))] border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/10 cursor-pointer"
                    : "bg-[rgb(var(--foreground))]/5 border-[rgba(var(--border),0.04)] text-[rgb(var(--foreground-muted))]/40 cursor-not-allowed"
                )}
              >
                Discard
              </button>
              <div className="relative group">
                <button
                  onClick={() => restoreDefaults()}
                  className="p-1.5 rounded-xl bg-[rgb(var(--foreground))]/[0.03] border border-[rgba(var(--accent),0.15)] text-[rgb(var(--foreground-muted))]/80 hover:bg-[rgb(var(--accent))]/10 hover:text-[rgb(var(--accent))] transition-all duration-300 cursor-pointer flex items-center justify-center shrink-0"
                  aria-label="Restore Defaults"
                >
                  <RotateCcw size={14} />
                </button>
                <div className="absolute bottom-full mb-2 right-0 translate-y-1 scale-95 opacity-0 group-hover:translate-y-0 group-hover:scale-100 group-hover:opacity-100 transition-all duration-300 ease-out pointer-events-none whitespace-nowrap px-3 py-1.5 rounded-xl border border-[rgba(var(--accent),0.25)] bg-[rgb(var(--background))]/95 dark:bg-zinc-950/95 backdrop-blur-md text-[rgb(var(--accent))] shadow-[0_8px_30px_rgba(0,0,0,0.12)] dark:shadow-[0_8px_30px_rgba(0,0,0,0.35)] text-[10px] font-bold tracking-wide uppercase z-50">
                  Restore Defaults
                </div>
              </div>
            </div>
          </div>

           <div className="flex-1 w-full overflow-y-auto custom-scrollbar px-3 py-4 pb-[85px] space-y-7 animate-fade-in">
             {[...DOMAINS].sort((a, b) => {
               const order = ["interaction", "tray", "models", "appearance", "memory", "persona"];
               return order.indexOf(a.id) - order.indexOf(b.id);
             }).map((domain) => {
               const Icon = domain.icon;
               return (
                 <div key={domain.id} className="w-full glass rounded-2xl p-4 md:p-5 space-y-4">
                  {/* Category Header */}
                  <div className="flex items-center gap-2.5 px-1">
                    <div className="p-2 rounded-lg bg-[rgba(var(--accent),0.1)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.15)] flex items-center justify-center">
                      <Icon size={18} />
                    </div>
                    <div className="flex flex-col">
                      <span className="text-[15px] font-black uppercase tracking-[0.18em] text-[rgb(var(--foreground))]">
                        {domain.label}
                      </span>
                      <span className="text-[10px] font-semibold tracking-wider uppercase text-[rgb(var(--foreground-muted))]/60">
                        {domain.sublabel}
                      </span>
                    </div>
                  </div>
                  
                  {/* Divider and Content */}
                  <div className="border-t border-[rgba(var(--accent),0.05)] pt-4">
                    <ErrorBoundary name={`SettingsMobile:${domain.id}`}>
                      <DomainContent domain={domain.id} layoutMode="small" />
                    </ErrorBoundary>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
};
