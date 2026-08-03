import { useState, useCallback, memo, useEffect, useMemo, useRef, Suspense } from "react";
import { Brain, Palette, Eye, Database, UserCircle, Sliders, RotateCcw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";
import { useSettingsStore } from "@/store/settingsStore";
import { GlassSkeleton, ErrorBoundary } from "@/shared/components/common";
import { AnimatePresence, motion } from "framer-motion";

import { PersonaCard } from "@/shared/components/settings/cards/PersonaCard";
import { ModelsCard } from "@/shared/components/settings/cards/ModelsCard";
import { RealtimeCard } from "@/shared/components/settings/cards/RealtimeCard";
import { TrayCard } from "@/shared/components/settings/cards/TrayCard";
import { MemoryCard } from "@/shared/components/settings/cards/MemoryCard";
import { AppearanceCard } from "@/shared/components/settings/cards/AppearanceCard";
import { InteractionCard } from "@/shared/components/settings/cards/InteractionCard";

import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId, type SettingsDomain as Domain } from "@/data/settingsDomains";

// ─── Radial hub geometry ──────────────────────────────────────────────────────


// ─── Domain content map ───────────────────────────────────────────────────────

const DomainContent = memo(({ domain, layoutMode }: { domain: DomainId; layoutMode?: "full-max" | "full-min" | "small" }) => {
  const { draftSettings } = useSettings();
  const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
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

// ─── Radial Hub Node ──────────────────────────────────────────────────────────

interface RadialNodeProps {
  domain: Domain;
  isActive: boolean;
  onSelect: (id: DomainId) => void;
  radiusX: number;
  radiusY: number;
}

const RadialNode = memo(({ domain, isActive, onSelect, radiusX, radiusY }: RadialNodeProps) => {
  const rad = (domain.angle * Math.PI) / 180;
  const pos = {
    x: radiusX * Math.cos(rad),
    y: radiusY * Math.sin(rad),
  };
  const Icon = domain.icon;
  const isUpper = domain.angle < 0;

  return (
    <button
      id={`node-${domain.id}`}
      onClick={() => onSelect(domain.id)}
      className={cn(
         "absolute w-10 h-10 rounded-full flex items-center justify-center border transition-all duration-400 group z-25",
         isActive
           ?            "text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.4)]"
           : "text-[rgb(var(--foreground-muted))] dark:text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.15)] dark:border-[rgba(var(--border),0.08)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--accent),0.06)]"
      )}
      style={{
        left: "50%",
        top: "50%",
        transform: `translate(calc(-50% + ${pos.x}px), calc(-50% + ${pos.y}px))`,
      }}
      aria-label={`${domain.label} settings`}
    >
      <Icon size={20} strokeWidth={isActive ? 2.5 : 1.5} />
      {/* Label */}
      <span 
        className={cn(
          "absolute left-1/2 -translate-x-1/2 text-[11px] font-bold uppercase tracking-[0.15em] leading-none whitespace-nowrap pointer-events-none text-center transition-all duration-400",
          isUpper ? "bottom-[calc(100%+8px)]" : "top-[calc(100%+8px)]"
        )}
      >
        {domain.label}
      </span>
    </button>
  );
});
RadialNode.displayName = "RadialNode";

// ─── Hub Connectors ────────────────────────────────────────────────────────────

interface HubConnectorsProps {
  activeDomains: DomainId[];
  radiusX: number;
  radiusY: number;
}

const HubConnectors: React.FC<HubConnectorsProps> = memo(({ activeDomains, radiusX, radiusY }) => {
  const maxRadius = Math.max(radiusX, radiusY);
  const size = maxRadius * 2 + 120; // viewBox size
  const cx = size / 2;
  const cy = size / 2;

  return (
    <svg
      width={size}
      height={size}
      className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 pointer-events-none z-5 overflow-visible"
      aria-hidden="true"
    >
      {DOMAINS.map((d) => {
        const rad = (d.angle * Math.PI) / 180;
        const pos = {
          x: radiusX * Math.cos(rad),
          y: radiusY * Math.sin(rad),
        };
        const isActive = activeDomains.includes(d.id);
        
        const x1 = cx;
        const y1 = cy;
        const x2 = cx + pos.x;
        const y2 = cy + pos.y;
        
        // Math for perpendicular ticks at fraction t:
        const dx = x2 - x1;
        const dy = y2 - y1;
        const len = Math.sqrt(dx * dx + dy * dy) || 1;
        const nx = -dy / len;
        const ny = dx / len;
        
        // Shorten the line to stop at the borders
        const cosAngle = pos.x / len;
        const sinAngle = pos.y / len;
        const R_center = 26; // outer border of center reactor
        const R_node = 20;   // border of radial node icon circle
        
        const lineX1 = cx + cosAngle * R_center;
        const lineY1 = cy + sinAngle * R_center;
        const lineX2 = cx + pos.x - cosAngle * R_node;
        const lineY2 = cy + pos.y - sinAngle * R_node;
        
        // Ticks at 35%
        const t35 = 0.35;
        const px35 = x1 + dx * t35;
        const py35 = y1 + dy * t35;
        const halfLen35 = isActive ? 4 : 3; // 8px total active vs 6px idle
        const t35_1_x = px35 + nx * halfLen35;
        const t35_1_y = py35 + ny * halfLen35;
        const t35_2_x = px35 - nx * halfLen35;
        const t35_2_y = py35 - ny * halfLen35;
        
        // Ticks at 65%
        const t65 = 0.65;
        const px65 = x1 + dx * t65;
        const py65 = y1 + dy * t65;
        const halfLen65 = isActive ? 5.5 : 4.5; // 11px total active vs 9px idle
        const t65_1_x = px65 + nx * halfLen65;
        const t65_1_y = py65 + ny * halfLen65;
        const t65_2_x = px65 - nx * halfLen65;
        const t65_2_y = py65 - ny * halfLen65;
        
        return (
          <g key={d.id} className="transition-all duration-400">
            {/* Main connector line */}
            <line
              x1={lineX1}
              y1={lineY1}
              x2={lineX2}
              y2={lineY2}
              className="transition-all duration-400"
              stroke={isActive ? "rgba(var(--accent), var(--hub-connector-active-opacity, 0.55))" : "rgba(var(--accent), 0.12)"}
              strokeWidth={isActive ? 1.5 : 1}
              strokeDasharray={isActive ? "none" : "3 5"}
            />
            
            {/* Tick marks at 35% */}
            <line
              x1={t35_1_x}
              y1={t35_1_y}
              x2={t35_2_x}
              y2={t35_2_y}
              className="transition-all duration-400"
              stroke={isActive ? "rgba(var(--accent), var(--hub-connector-tick35-opacity, 0.45))" : "rgba(var(--accent), 0.25)"}
              strokeWidth={1}
              opacity={isActive ? 1 : 0.4}
            />
            
            {/* Tick marks at 65% */}
            <line
              x1={t65_1_x}
              y1={t65_1_y}
              x2={t65_2_x}
              y2={t65_2_y}
              className="transition-all duration-400"
              stroke={isActive ? "rgba(var(--accent), var(--hub-connector-tick65-opacity, 0.35))" : "rgba(var(--accent), 0.20)"}
              strokeWidth={1}
              opacity={isActive ? 1 : 0.4}
            />
          </g>
        );
      })}
    </svg>
  );
});
HubConnectors.displayName = "HubConnectors";

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

const hasCardChanges = (domainId: DomainId, settings: any, draftSettings: any) => {
  if (!settings || !draftSettings) return false;
  switch (domainId) {
    case "models": {
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      if (isRealtime) {
        const provId = draftSettings.realtime?.provider || "gemini_live";
        const subkey = provId === "gemini_live" ? "gemini" :
                       provId === "openai_realtime" ? "openai" :
                       provId === "deepgram_voice_agent" ? "deepgram" : "elevenlabs";
                       
        const savedProvConfig = settings.realtime?.[subkey] || {};
        const draftProvConfig = draftSettings.realtime?.[subkey] || {};
        
        // Exclude api_key when checking model-specific card changes
        const { api_key: _, ...savedClean } = savedProvConfig;
        const { api_key: __, ...draftClean } = draftProvConfig;
        
        return JSON.stringify(savedClean) !== JSON.stringify(draftClean);
      }
      return (
        JSON.stringify(settings.vad) !== JSON.stringify(draftSettings.vad) ||
        JSON.stringify(settings.asr) !== JSON.stringify(draftSettings.asr) ||
        JSON.stringify(settings.tts) !== JSON.stringify(draftSettings.tts) ||
        settings.llm.model !== draftSettings.llm.model ||
        settings.llm.ctx_size !== draftSettings.llm.ctx_size ||
        settings.llm.threads !== draftSettings.llm.threads ||
        (settings.llm.provider?.model !== draftSettings.llm.provider?.model)
      );
    }
    case "tray":
      return (
        settings.ui.tray_enabled !== draftSettings.ui.tray_enabled ||
        settings.ui.tray_blur_density !== draftSettings.ui.tray_blur_density ||
        settings.ui.tray_glass_tint !== draftSettings.ui.tray_glass_tint ||
        settings.ui.tray_history_limit !== draftSettings.ui.tray_history_limit ||
        settings.interaction.tray_mode !== draftSettings.interaction.tray_mode
      );
    case "persona":
      return JSON.stringify(settings.assistant) !== JSON.stringify(draftSettings.assistant);
    case "memory":
      return JSON.stringify(settings.persistence) !== JSON.stringify(draftSettings.persistence);
    case "appearance":
      return false; // Appearance changes are applied instantly and saved automatically
    case "interaction": {
      const { model: _, ...provSettings } = settings.llm.provider || {};
      const { model: __, ...provDraft } = draftSettings.llm.provider || {};
      
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      let realtimeChanges = false;
      if (isRealtime) {
        if (settings.realtime?.provider !== draftSettings.realtime?.provider) {
          realtimeChanges = true;
        } else {
          const provId = draftSettings.realtime?.provider || "gemini_live";
          const subkey = provId === "gemini_live" ? "gemini" :
                         provId === "openai_realtime" ? "openai" :
                         provId === "deepgram_voice_agent" ? "deepgram" : "elevenlabs";
          if (settings.realtime?.[subkey]?.api_key !== draftSettings.realtime?.[subkey]?.api_key) {
            realtimeChanges = true;
          }
        }
      }
      
      return (
        settings.interaction.main_app_mode !== draftSettings.interaction.main_app_mode ||
        settings.interaction.auto_sleep_timeout !== draftSettings.interaction.auto_sleep_timeout ||
        settings.interaction.pipeline_mode !== draftSettings.interaction.pipeline_mode ||
        JSON.stringify(provSettings) !== JSON.stringify(provDraft) ||
        realtimeChanges
      );
    }
    default:
      return false;
  }
};

const discardCardChanges = (domainId: DomainId, settings: any, updateDraft: any, draftSettings?: any) => {
  if (!settings) return;
  switch (domainId) {
    case "models": {
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      if (isRealtime) {
        const provId = draftSettings.realtime?.provider || "gemini_live";
        const subkey = provId === "gemini_live" ? "gemini" :
                       provId === "openai_realtime" ? "openai" :
                       provId === "deepgram_voice_agent" ? "deepgram" : "elevenlabs";
                       
        const savedProvConfig = settings.realtime?.[subkey] || {};
        const currentDraftProvConfig = draftSettings.realtime?.[subkey] || {};
        
        const { api_key: _, ...savedClean } = savedProvConfig;
        updateDraft("realtime", subkey, {
          ...currentDraftProvConfig,
          ...savedClean
        });
      } else {
        Object.keys(settings.vad).forEach(k => updateDraft("vad", k, (settings.vad as any)[k]));
        Object.keys(settings.asr).forEach(k => updateDraft("asr", k, (settings.asr as any)[k]));
        updateDraft("llm", "model", settings.llm.model);
        updateDraft("llm", "ctx_size", settings.llm.ctx_size);
        updateDraft("llm", "threads", settings.llm.threads);
        Object.keys(settings.tts).forEach(k => updateDraft("tts", k, (settings.tts as any)[k]));
        if (settings.llm.provider && draftSettings?.llm.provider) {
          updateDraft("llm", "provider", {
            ...draftSettings.llm.provider,
            model: settings.llm.provider.model
          });
        }
      }
      break;
    }
    case "tray":
      updateDraft("ui", "tray_enabled", settings.ui.tray_enabled);
      updateDraft("ui", "tray_blur_density", settings.ui.tray_blur_density);
      updateDraft("ui", "tray_glass_tint", settings.ui.tray_glass_tint);
      updateDraft("ui", "tray_history_limit", settings.ui.tray_history_limit);
      updateDraft("interaction", "tray_mode", settings.interaction.tray_mode);
      break;
    case "persona":
      Object.keys(settings.assistant).forEach(k => updateDraft("assistant", k, (settings.assistant as any)[k]));
      break;
    case "memory":
      Object.keys(settings.persistence).forEach(k => updateDraft("persistence", k, (settings.persistence as any)[k]));
      Object.keys(settings.memory).forEach(k => updateDraft("memory", k, (settings.memory as any)[k]));
      break;
    case "appearance":
      updateDraft("ui", "theme", settings.ui.theme);
      updateDraft("ui", "accent_seed", settings.ui.accent_seed);
      break;
    case "interaction": {
      updateDraft("interaction", "main_app_mode", settings.interaction.main_app_mode);
      updateDraft("interaction", "auto_sleep_timeout", settings.interaction.auto_sleep_timeout);
      updateDraft("interaction", "pipeline_mode", settings.interaction.pipeline_mode);
      const currentDraftModel = draftSettings?.llm.provider?.model || "";
      updateDraft("llm", "provider", {
        ...settings.llm.provider,
        model: currentDraftModel
      });
      
      const isRealtime = draftSettings?.interaction?.pipeline_mode === "realtime";
      if (isRealtime) {
        updateDraft("realtime", "provider", settings.realtime.provider);
        const subkeys = ["gemini", "openai", "deepgram", "elevenlabs"] as const;
        subkeys.forEach(subkey => {
          if (settings.realtime?.[subkey] && draftSettings?.realtime?.[subkey]) {
            updateDraft("realtime", subkey, {
              ...draftSettings.realtime[subkey],
              api_key: settings.realtime[subkey].api_key
            });
          }
        });
      }
      break;
    }
  }
};

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
  const updateDraft = useSettingsStore(s => s.updateDraft);
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
                        onClick={() => discardCardChanges(domain.id, settings, updateDraft, draftSettings)}
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

// ─── Main Settings Component ──────────────────────────────────────────────────

export const Settings: React.FC = () => {
  const { draftSettings, settings, commitChanges, discardChanges, hasChanges, restoreDefaults } = useSettings();
  const updateDraft = useSettingsStore(s => s.updateDraft);
  const [activeDomains, setActiveDomains] = useState<DomainId[]>([]);


  const [isCompact, setIsCompact] = useState(false);
  const [windowWidth, setWindowWidth] = useState(typeof window !== "undefined" ? window.innerWidth : 1440);
  const [windowHeight, setWindowHeight] = useState(typeof window !== "undefined" ? window.innerHeight : 800);
  const containerRef = useRef<HTMLDivElement>(null);
  const [lines, setLines] = useState<Record<DomainId, { x1: number; y1: number; x2: number; y2: number } | null>>({
    persona: null,
    models: null,
    tray: null,
    memory: null,
    appearance: null,
    interaction: null,
  });

  const lastActiveDomains = useRef<DomainId[]>([]);
  useEffect(() => {
    const closed = lastActiveDomains.current.filter((d) => !activeDomains.includes(d));
    if (closed.length > 0 && settings) {
      closed.forEach((domainId) => {
        discardCardChanges(domainId, settings, updateDraft, draftSettings);
      });
    }
    lastActiveDomains.current = activeDomains;
  }, [activeDomains, settings, updateDraft, draftSettings]);

  // Resize listener with requestAnimationFrame throttling to eliminate React rendering churn
  useEffect(() => {
    let rafId: number;
    const checkSize = () => {
      cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(() => {
        setWindowWidth(window.innerWidth);
        setWindowHeight(window.innerHeight);
        setIsCompact(window.innerWidth < 1024);
      });
    };
    checkSize();
    window.addEventListener("resize", checkSize);
    return () => {
      window.removeEventListener("resize", checkSize);
      cancelAnimationFrame(rafId);
    };
  }, []);

  const radiusX = useMemo(() => Math.max(90, Math.min(120, windowWidth * 0.09 - 10)), [windowWidth]);
  const radiusY = useMemo(() => Math.max(75, Math.min(120, windowHeight * 0.14 - 8)), [windowHeight]);

  // Calculate layoutMode dynamically
  const layoutMode = useMemo(() => {
    if (isCompact) return "small";
    if (windowWidth < 1366 || activeDomains.length > 1) return "full-min";
    return "full-max";
  }, [isCompact, windowWidth, activeDomains.length]);

  // Outside click handler to pop active domains (LIFO order)
  useEffect(() => {
    if (isCompact) return; // Disable outside click close behavior on mobile/tablet flat view

    const handleOutsideClick = (e: MouseEvent) => {
      if (activeDomains.length === 0) return;

      const target = e.target as HTMLElement;

      // Only close if we clicked on the background of the settings page itself
      if (!containerRef.current || !containerRef.current.contains(target)) {
        return;
      }

      const clickedInsideNodeOrCard = DOMAINS.some((domain) => {
        const nodeEl = document.getElementById(`node-${domain.id}`);
        const cardEl = document.getElementById(`card-${domain.id}`);
        return (
          (nodeEl && nodeEl.contains(target)) ||
          (cardEl && cardEl.contains(target))
        );
      });

      const centerNodeEl = document.getElementById("center-node");
      const clickedCenter = centerNodeEl && centerNodeEl.contains(target);

      if (!clickedInsideNodeOrCard && !clickedCenter) {
        setActiveDomains((prev) => prev.slice(0, -1));
      }
    };

    document.addEventListener("mousedown", handleOutsideClick);
    return () => document.removeEventListener("mousedown", handleOutsideClick);
  }, [activeDomains, isCompact]);

  // Calculate dynamic line positions between active nodes and cards (without continuously polling layout rects)
  useEffect(() => {
    if (isCompact || activeDomains.length === 0) {
      setLines({
        persona: null,
        models: null,
        tray: null,
        memory: null,
        appearance: null,
        interaction: null,
      });
      return;
    }

    let calcRafId: number;
    const calculate = () => {
      if (!containerRef.current) return;
      cancelAnimationFrame(calcRafId);
      calcRafId = requestAnimationFrame(() => {
        if (!containerRef.current) return;
        const containerRect = containerRef.current.getBoundingClientRect();
        const newLines = { ...lines };
        let changed = false;

        DOMAINS.forEach((domain) => {
          if (!activeDomains.includes(domain.id)) {
            if (newLines[domain.id] !== null) {
              newLines[domain.id] = null;
              changed = true;
            }
            return;
          }

          const nodeEl = document.getElementById(`node-${domain.id}`);
          const cardEl = document.getElementById(`card-${domain.id}`);

          if (nodeEl && cardEl) {
            const nodeRect = nodeEl.getBoundingClientRect();
            const cardRect = cardEl.getBoundingClientRect();

            const x1 = (nodeRect.left + nodeRect.right) / 2 - containerRect.left;
            const y1 = (nodeRect.top + nodeRect.bottom) / 2 - containerRect.top;

            let x2 = 0;
            let y2 = 0;

            // Connect to the edge of the card nearest to the node
            switch (domain.id) {
              case "persona":
                x2 = (cardRect.left + cardRect.right) / 2 - containerRect.left;
                y2 = cardRect.bottom - containerRect.top;
                break;
              case "appearance":
                x2 = (cardRect.left + cardRect.right) / 2 - containerRect.left;
                y2 = cardRect.top - containerRect.top;
                break;
              case "models":
              case "tray":
                x2 = cardRect.left - containerRect.left;
                y2 = (cardRect.top + cardRect.bottom) / 2 - containerRect.top;
                break;
              case "memory":
              case "interaction":
                x2 = cardRect.right - containerRect.left;
                y2 = (cardRect.top + cardRect.bottom) / 2 - containerRect.top;
                break;
            }

            if (!isNaN(x1) && !isNaN(y1) && !isNaN(x2) && !isNaN(y2)) {
              const existing = newLines[domain.id];
              if (
                !existing ||
                Math.abs(existing.x1 - x1) > 0.5 ||
                Math.abs(existing.y1 - y1) > 0.5 ||
                Math.abs(existing.x2 - x2) > 0.5 ||
                Math.abs(existing.y2 - y2) > 0.5
              ) {
                newLines[domain.id] = { x1, y1, x2, y2 };
                changed = true;
              }
            }
          } else {
            if (newLines[domain.id] !== null) {
              newLines[domain.id] = null;
              changed = true;
            }
          }
        });

        if (changed) {
          setLines(newLines);
        }
      });
    };

    // Calculate immediately on selection or size changes
    calculate();

    // Trigger recalculation after card entry animation finishes
    const timer = setTimeout(calculate, 320);

    return () => {
      clearTimeout(timer);
      cancelAnimationFrame(calcRafId);
    };
  }, [activeDomains, isCompact, windowWidth, windowHeight]);

  const handleSelect = useCallback((id: DomainId) => {
    setActiveDomains((prev) => {
      if (prev.includes(id)) {
        return prev.filter((d) => d !== id);
      } else {
        if (isCompact) {
          return [id];
        }
        return [...prev, id];
      }
    });
  }, [isCompact]);

  const handleCenterClick = useCallback(() => {
    setActiveDomains((prev) => (prev.length > 0 ? [] : DOMAINS.map((d) => d.id)));
  }, []);



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
