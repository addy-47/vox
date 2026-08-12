import { useState, useEffect, useRef, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Sliders, Eye, EyeOff, Activity, Radio } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SegmentedControl, ToggleTile } from "@/shared/ui";
import { TriggerModeCard } from "./TriggerModeCard";
import { PipelineModeCard } from "./PipelineModeCard";
import { CategorySelector } from "./CategorySelector";
import { LlmConfigDesk } from "./LlmConfigDesk";
import { checkIfCloudUrl, CLOUD_PROVIDERS } from "@/data/providers";

interface InteractionCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const VIEW_OPTIONS = [
  { id: "main" as const, label: "Main" },
  { id: "tray" as const, label: "Tray" },
];

export const InteractionCard = memo(
  ({ layoutMode = "full-max" }: InteractionCardProps) => {
    const settings = useSettingsStore((s) => s.settings);
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);

    const [activeView, setActiveView] = useState<"main" | "tray">("main");
    const [activeCategory, setActiveCategory] = useState<"STT" | "LLM" | "TTS">("LLM");
    const [sttPillOverride, setSttPillOverride] = useState<"local" | "remote" | "cloud" | null>(null);
    const [ttsPillOverride, setTtsPillOverride] = useState<"local" | "remote" | "cloud" | null>(null);

    const prevCategoryRef = useRef<string>(activeCategory);

    if (!draftSettings || !settings) return null;
    const { interaction, llm, ui } = draftSettings;

    const trayEnabled = ui.tray_enabled ?? true;
    const trayMode = interaction.tray_mode ?? "Passive";

    const currentProvider = llm.provider || { kind: "embedded" };
    const isCloudUrl = checkIfCloudUrl(currentProvider.base_url || "");

    const providerPill =
      currentProvider.kind === "embedded"
        ? "local"
        : isCloudUrl
          ? "cloud"
          : "remote";

    const isModular = interaction.pipeline_mode === "modular";

    const sttPill = sttPillOverride || "local";
    const llmPill = providerPill;
    const ttsKind = draftSettings.tts?.provider?.kind || "supertonic";
    const ttsPill =
      ttsPillOverride ||
      (ttsKind === "chatterbox_remote"
        ? "remote"
        : ttsKind === "supertonic" || ttsKind === "chatterbox"
          ? "local"
          : "cloud");
    const activePill =
      activeCategory === "STT"
        ? sttPill
        : activeCategory === "LLM"
          ? llmPill
          : ttsPill;

    const cycleCategory = () => {
      setActiveCategory((prev) => (prev === "STT" ? "LLM" : prev === "LLM" ? "TTS" : "STT"));
    };

    // Guard sync_pipeline_tab event dispatch to prevent ping-pong loop
    useEffect(() => {
      if (prevCategoryRef.current !== activeCategory) {
        prevCategoryRef.current = activeCategory;
        const event = new CustomEvent("sync_pipeline_tab", {
          detail: activeCategory.toLowerCase(),
        });
        window.dispatchEvent(event);
      }
    }, [activeCategory]);

    useEffect(() => {
      const handleSync = (e: Event) => {
        const cat = (e as CustomEvent).detail;
        if (cat === "stt" || cat === "llm" || cat === "tts") {
          const upperCat = cat.toUpperCase() as "STT" | "LLM" | "TTS";
          if (prevCategoryRef.current !== upperCat) {
            prevCategoryRef.current = upperCat;
            setActiveCategory(upperCat);
          }
        }
      };
      window.addEventListener("sync_interaction_category", handleSync);
      return () => window.removeEventListener("sync_interaction_category", handleSync);
    }, []);

    const handleLlmPillChange = (value: string) => {
      if (value === "local") {
        const savedModel = settings.llm.provider.kind === "embedded" ? settings.llm.provider.model : "";
        updateDraft("llm", "provider", { kind: "embedded", model: savedModel });
      } else if (value === "remote") {
        const savedRemote =
          settings.llm.provider.kind === "open_ai_compat" &&
          !checkIfCloudUrl(settings.llm.provider.base_url || "")
            ? settings.llm.provider
            : null;
        updateDraft("llm", "provider", {
          kind: "open_ai_compat",
          base_url: savedRemote?.base_url || "http://127.0.0.1:11434",
          api_key: savedRemote?.api_key || "",
          provider_name: savedRemote?.provider_name || "Ollama",
          model: savedRemote?.model || "",
        });
      } else if (value === "cloud") {
        const savedCloud =
          settings.llm.provider.kind === "open_ai_compat" &&
          checkIfCloudUrl(settings.llm.provider.base_url || "")
            ? settings.llm.provider
            : null;
        updateDraft("llm", "provider", {
          kind: "open_ai_compat",
          base_url: savedCloud?.base_url || CLOUD_PROVIDERS[0].url,
          api_key: savedCloud?.api_key || "",
          provider_name: savedCloud?.provider_name || CLOUD_PROVIDERS[0].name,
          model: savedCloud?.model || "",
        });
      }
    };

    const handlePillChange = (value: "local" | "remote" | "cloud") => {
      if (activeCategory === "STT") {
        setSttPillOverride(value === "local" ? null : value);
      } else if (activeCategory === "LLM") {
        handleLlmPillChange(value);
      } else if (activeCategory === "TTS") {
        if (value === "local") {
          setTtsPillOverride(null);
          updateDraft("tts", "provider", { kind: "supertonic" });
        } else if (value === "remote") {
          setTtsPillOverride(null);
          updateDraft("tts", "provider", {
            kind: "chatterbox_remote",
            endpoint: "http://127.0.0.1:7860",
            language: "en",
            quality_steps: 8,
            speed: 1.0,
            remote_path: "~/.vox",
          });
        } else {
          setTtsPillOverride(null);
          updateDraft("tts", "provider", {
            kind: "edge_tts",
            voice: (draftSettings.tts.provider as any)?.voice || "en-US-AriaNeural",
          });
        }
      }
    };

    return (
      <div
        className={cn(
          "w-full flex flex-col text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none justify-between",
          layoutMode === "small"
            ? "bg-transparent p-0 h-auto"
            : cn(
                "glass-card p-5 lg:h-[340px]",
                layoutMode === "full-min"
                  ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]"
                  : "lg:w-[520px]"
              )
        )}
      >
        {/* Header Section */}
        {layoutMode !== "small" && (
          <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
            <div className="flex items-center gap-2">
              <Sliders className="text-[rgb(var(--accent))]" size={18} />
              <span className="text-[13px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
                Interaction Console
              </span>
            </div>
            {/* Top Right Main / Tray Switcher */}
            <SegmentedControl
              options={VIEW_OPTIONS}
              value={activeView}
              onChange={setActiveView}
              size="sm"
            />
          </div>
        )}

        <div className="flex flex-col gap-3 flex-1">
          {/* Core Controls Dashboard Grid */}
          <div
            className={cn(
              "grid gap-2 shrink-0",
              layoutMode === "small" ? "grid-cols-1" : "grid-cols-2"
            )}
          >
            {activeView === "main" ? (
              <>
                <TriggerModeCard layoutMode={layoutMode} />
                <PipelineModeCard layoutMode={layoutMode} />
              </>
            ) : (
              <>
                <ToggleTile
                  title="HUD Window"
                  active={trayEnabled}
                  activeLabel="Enabled"
                  inactiveLabel="Disabled"
                  activeSublabel="Overlay Active"
                  inactiveSublabel="Background Run"
                  icon={trayEnabled ? Eye : EyeOff}
                  onToggle={() => updateDraft("ui", "tray_enabled", !trayEnabled)}
                  layoutMode={layoutMode}
                />

                <ToggleTile
                  title="Tray Mode"
                  active={trayMode === "Passive"}
                  activeLabel={trayMode === "Passive" ? "Continuous" : "Push-To-Talk"}
                  inactiveLabel="Push-To-Talk"
                  activeSublabel={trayMode === "Passive" ? "Passive Sense" : "Manual Trigger"}
                  inactiveSublabel="Manual Trigger"
                  icon={trayMode === "Passive" ? Activity : Radio}
                  onToggle={() => updateDraft("interaction", "tray_mode", trayMode === "Passive" ? "PTT" : "Passive")}
                  layoutMode={layoutMode}
                />
              </>
            )}
          </div>

          {/* Category & Provider Selector Subcomponent */}
          {isModular && (
            <CategorySelector
              activeCategory={activeCategory}
              activePill={activePill}
              onCycleCategory={cycleCategory}
              onPillChange={handlePillChange}
              layoutMode={layoutMode}
            />
          )}

          {/* Configuration Desk Subcomponent */}
          <LlmConfigDesk
            activeCategory={activeCategory}
            activePill={activePill}
            isModular={isModular}
            layoutMode={layoutMode}
          />
        </div>
      </div>
    );
  }
);

InteractionCard.displayName = "InteractionCard";
