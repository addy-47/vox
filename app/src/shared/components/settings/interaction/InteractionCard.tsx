import { useState, useEffect, useRef, memo } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { Sliders, Mic, MicOff, Activity, Radio } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SegmentedControl, ToggleTile } from "@/shared/ui";
import { TriggerModeCard } from "./TriggerModeCard";
import { PipelineModeCard } from "./PipelineModeCard";
import { CategorySelector } from "./CategorySelector";
import { LlmConfigDesk } from "./LlmConfigDesk";
import { RealtimeConfigDesk } from "./RealtimeConfigDesk";
import { DictationConfigDesk } from "./DictationConfigDesk";
import { checkIfCloudUrl } from "@/data/providersCopy";
import { DICTATION_COPY } from "@/data/settingsCopy";

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
    const activeLlm = llm.active || "embedded";
    const activeRemoteUrl = activeLlm === "server" ? llm.server?.base_url : activeLlm === "cloud" ? llm.cloud?.base_url : "";
    const isCloudUrl = checkIfCloudUrl(activeRemoteUrl || "");

    const providerPill =
      activeLlm === "embedded"
        ? "local"
        : activeLlm === "cloud"
        ? "cloud"
        : isCloudUrl
        ? "cloud"
        : "remote";

    const isModular = interaction.pipeline_mode === "modular";

    const sttPill = sttPillOverride || "local";
    const llmPill = providerPill;
    const ttsKind = draftSettings.tts?.active || "supertonic";
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
        updateDraft("llm", "active", "embedded");
      } else if (value === "remote") {
        updateDraft("llm", "active", "server");
      } else if (value === "cloud") {
        updateDraft("llm", "active", "cloud");
      }
    };

    const handlePillChange = (value: "local" | "remote" | "cloud") => {
      if (activeCategory === "STT") {
        setSttPillOverride(value === "local" ? null : value);
        if (value === "local") {
          updateDraft("stt", "active", "embedded");
        } else if (value === "cloud") {
          updateDraft("stt", "active", "cloud");
        }
      } else if (activeCategory === "LLM") {
        handleLlmPillChange(value);
      } else if (activeCategory === "TTS") {
        setTtsPillOverride(null);
        if (value === "local") {
          updateDraft("tts", "active", "supertonic");
        } else if (value === "remote") {
          updateDraft("tts", "active", "chatterbox_remote");
        } else {
          updateDraft("tts", "active", "edge_tts");
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

              {/* Category & Provider Selector Subcomponent or Realtime Config Desk */}
              {isModular ? (
                <>
                  <CategorySelector
                    activeCategory={activeCategory}
                    activePill={activePill}
                    onCycleCategory={cycleCategory}
                    onSetCategory={setActiveCategory}
                    onPillChange={handlePillChange}
                    layoutMode={layoutMode}
                  />

                  {/* Configuration Desk Subcomponent */}
                  <LlmConfigDesk
                    activeCategory={activeCategory}
                    activePill={activePill}
                    isModular={isModular}
                    layoutMode={layoutMode}
                  />
                </>
              ) : (
                <RealtimeConfigDesk layoutMode={layoutMode} />
              )}
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
