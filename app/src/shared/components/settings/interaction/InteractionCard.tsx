import { useState, useEffect, useRef, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Sliders, Mic, MicOff, Activity, Radio } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SegmentedControl, ToggleTile } from "@/shared/ui";
import { TriggerModeCard } from "./TriggerModeCard";
import { PipelineModeCard } from "./PipelineModeCard";
import { CategorySelector } from "./CategorySelector";
import { LlmConfigDesk } from "./LlmConfigDesk";
import { DictationConfigDesk } from "./DictationConfigDesk";
import { checkIfCloudUrl, CLOUD_PROVIDERS } from "@/data/providers";
import { DICTATION_COPY } from "@/data/settingsData";

interface InteractionCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const VIEW_OPTIONS = [
  { id: "assistant" as const, label: "Assistant" },
  { id: "dictation" as const, label: "Dictation" },
];

export const InteractionCard = memo(
  ({ layoutMode = "full-max" }: InteractionCardProps) => {
    const settings = useSettingsStore((s) => s.settings);
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);

    const [activeView, setActiveView] = useState<"assistant" | "dictation">("assistant");
    const [activeCategory, setActiveCategory] = useState<"STT" | "LLM" | "TTS">("LLM");
    const [sttPillOverride, setSttPillOverride] = useState<"local" | "remote" | "cloud" | null>(null);
    const [ttsPillOverride, setTtsPillOverride] = useState<"local" | "remote" | "cloud" | null>(null);

    const prevCategoryRef = useRef<string>(activeCategory);

    if (!draftSettings || !settings) return null;
    const { interaction, llm, dictation } = draftSettings;

    const dictationEnabled = dictation?.enabled ?? true;
    const dictationInteractionMode = dictation?.interaction_mode ?? "ptt";

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
        <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
          <div className="flex items-center gap-2">
            <Sliders className="text-[rgb(var(--accent))]" size={layoutMode === "small" ? 15 : 18} />
            <span className="text-[12px] sm:text-[13px] font-display font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
              Interaction
            </span>
          </div>
          {/* Top Right Assistant / Dictation Switcher */}
          <SegmentedControl
            options={VIEW_OPTIONS}
            value={activeView}
            onChange={setActiveView}
            size="sm"
          />
        </div>

        <div className="flex flex-col gap-3 flex-1">
          {activeView === "assistant" ? (
            <>
              {/* Core Assistant Controls */}
              <div
                className={cn(
                  "grid gap-2 shrink-0",
                  layoutMode === "small" ? "grid-cols-1" : "grid-cols-2"
                )}
              >
                <TriggerModeCard layoutMode={layoutMode} />
                <PipelineModeCard layoutMode={layoutMode} />
              </div>

              {/* Category & Provider Selector Subcomponent */}
              {isModular && (
                <CategorySelector
                  activeCategory={activeCategory}
                  activePill={activePill}
                  onCycleCategory={cycleCategory}
                  onSetCategory={setActiveCategory}
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
            </>
          ) : (
            <>
              {/* Dictation Mode Controls */}
              <div
                className={cn(
                  "grid gap-2 shrink-0",
                  layoutMode === "small" ? "grid-cols-1" : "grid-cols-2"
                )}
              >
                <ToggleTile
                  title="Voice Typing"
                  active={dictationEnabled}
                  activeLabel="Enabled"
                  inactiveLabel="Disabled"
                  activeSublabel={DICTATION_COPY.voiceTypingActive}
                  inactiveSublabel={DICTATION_COPY.voiceTypingInactive}
                  icon={dictationEnabled ? Mic : MicOff}
                  onToggle={() => updateDraft("dictation", "enabled", !dictationEnabled)}
                  layoutMode={layoutMode}
                />

                <ToggleTile
                  title="Trigger Mode"
                  active={dictationInteractionMode === "passive"}
                  activeLabel={DICTATION_COPY.triggerContinuous}
                  inactiveLabel={DICTATION_COPY.triggerPtt}
                  activeSublabel={DICTATION_COPY.triggerContinuousSub}
                  inactiveSublabel={DICTATION_COPY.triggerPttSub}
                  icon={dictationInteractionMode === "passive" ? Activity : Radio}
                  disabled={!dictationEnabled}
                  onToggle={() =>
                    updateDraft(
                      "dictation",
                      "interaction_mode",
                      dictationInteractionMode === "passive" ? "ptt" : "passive"
                    )
                  }
                  layoutMode={layoutMode}
                />
              </div>

              {/* Dictation Output & Hotkey Configuration Desk */}
              <DictationConfigDesk layoutMode={layoutMode} disabled={!dictationEnabled} />
            </>
          )}
        </div>
      </div>
    );
  }
);

InteractionCard.displayName = "InteractionCard";
