import { useState, useEffect, useRef, memo, useCallback } from "react";
import { useSettingsStore, TtsActiveProvider } from "@/store/settingsStore";
import { SlidersHorizontal, Mic, MicOff, Activity, Radio } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SegmentedControl, ToggleTile } from "@/shared/ui";
import { TriggerModeCard } from "./TriggerModeCard";
import { PipelineModeCard } from "./PipelineModeCard";
import { CategorySelector } from "./CategorySelector";
import { ProviderSelectorView, ProviderTier } from "./ProviderSelectorView";
import { LlmConfigDesk } from "./LlmConfigDesk";
import { RealtimeConfigDesk } from "./RealtimeConfigDesk";
import { DictationConfigDesk } from "./DictationConfigDesk";
import { DICTATION_COPY, INTERACTION_CARD_COPY } from "@/data/settingsCopy";

interface InteractionCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

const VIEW_OPTIONS = [
  { id: "assistant" as const, label: INTERACTION_CARD_COPY.viewAssistant },
  { id: "dictation" as const, label: INTERACTION_CARD_COPY.viewDictation },
];

export const InteractionCard = memo(
  ({ layoutMode = "full-max" }: InteractionCardProps) => {
    const settings = useSettingsStore((s) => s.settings);
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const modelCatalog = useSettingsStore((s) => s.modelCatalog);
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const discardCategoryChanges = useSettingsStore((s) => s.discardCategoryChanges);

    const [activeView, setActiveView] = useState<"assistant" | "dictation">("assistant");
    const [activeCategory, setActiveCategory] = useState<"STT" | "LLM" | "TTS">("LLM");
    const [drillDownProvider, setDrillDownProvider] = useState<ProviderTier | null>(null);
    const [sttPillOverride, setSttPillOverride] = useState<ProviderTier | null>(null);
    const [ttsPillOverride, setTtsPillOverride] = useState<ProviderTier | null>(null);

    const prevCategoryRef = useRef<string>(activeCategory);

    // Auto-discard unsaved draft changes when leaving Level 2 back to Level 1
    const handleBackFromLevel2 = useCallback(() => {
      discardCategoryChanges(activeCategory.toLowerCase());
      setSttPillOverride(null);
      setTtsPillOverride(null);
      setDrillDownProvider(null);
    }, [activeCategory, discardCategoryChanges]);

    // Auto-discard unsaved draft changes when switching category tabs
    const handleSetCategory = useCallback(
      (cat: "STT" | "LLM" | "TTS") => {
        if (drillDownProvider !== null) {
          discardCategoryChanges(activeCategory.toLowerCase());
          setSttPillOverride(null);
          setTtsPillOverride(null);
        }
        setActiveCategory(cat);
        setDrillDownProvider(null);
      },
      [activeCategory, discardCategoryChanges, drillDownProvider]
    );

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
            if (drillDownProvider !== null) {
              discardCategoryChanges(activeCategory.toLowerCase());
              setSttPillOverride(null);
              setTtsPillOverride(null);
            }
            prevCategoryRef.current = upperCat;
            setActiveCategory(upperCat);
            setDrillDownProvider(null);
          }
        }
      };
      window.addEventListener("sync_interaction_category", handleSync);
      return () => window.removeEventListener("sync_interaction_category", handleSync);
    }, [activeCategory, discardCategoryChanges, drillDownProvider]);

    if (!draftSettings || !settings) return null;
    const { interaction, llm, dictation } = draftSettings;

    const dictationEnabled = dictation?.enabled ?? true;
    const dictationInteractionMode = dictation?.interaction_mode ?? "ptt";
    const isModular = interaction.pipeline_mode === "modular";

    const getTtsTier = (modelId: string): ProviderTier => {
      const model = modelCatalog?.tts?.find((m) => m.id === modelId);
      if (model?.is_remote) return "server";
      if (model?.is_cloud) return "cloud";
      return "embedded";
    };

    const savedLlmPill: ProviderTier = settings.llm?.active || "embedded";
    const savedSttPill: ProviderTier = settings.stt?.active === "cloud" ? "cloud" : "embedded";
    const savedTtsPill: ProviderTier = getTtsTier(settings.tts?.active || "");

    const savedPill: ProviderTier =
      activeCategory === "STT"
        ? savedSttPill
        : activeCategory === "LLM"
        ? savedLlmPill
        : savedTtsPill;

    const draftLlmPill: ProviderTier = llm?.active || "embedded";
    const draftSttPill: ProviderTier =
      sttPillOverride || (draftSettings.stt?.active === "cloud" ? "cloud" : "embedded");
    const draftTtsPill: ProviderTier =
      ttsPillOverride || getTtsTier(draftSettings.tts?.active || "");

    const draftPill: ProviderTier =
      activeCategory === "STT"
        ? draftSttPill
        : activeCategory === "LLM"
        ? draftLlmPill
        : draftTtsPill;

    const handlePillChange = (value: ProviderTier) => {
      if (activeCategory === "STT") {
        setSttPillOverride(value === "embedded" ? null : value);
        if (value === "embedded") {
          updateDraft("stt", "active", "embedded");
        }
      } else if (activeCategory === "LLM") {
        updateDraft("llm", "active", value);
      } else if (activeCategory === "TTS") {
        setTtsPillOverride(null);
        const currentTier = getTtsTier(draftSettings.tts?.active || "");
        if (currentTier !== value) {
          const targetModel = modelCatalog?.tts?.find((m) =>
            value === "server"
              ? m.is_remote
              : value === "cloud"
              ? m.is_cloud
              : !m.is_remote && !m.is_cloud
          );
          if (targetModel) {
            updateDraft("tts", "active", targetModel.id as TtsActiveProvider);
          }
        }
      }
    };

    const handleSelectProvider = (pill: ProviderTier) => {
      handlePillChange(pill);
      setDrillDownProvider(pill);
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
            <SlidersHorizontal className="text-[rgb(var(--accent))]" size={17} />
            <span className="font-display text-[13px] font-black uppercase tracking-[0.2em] text-[rgb(var(--foreground))]">
              {INTERACTION_CARD_COPY.cardTitle}
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

        <div className="flex flex-col gap-3 flex-1 min-h-0">
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
                drillDownProvider !== null ? (
                  /* ── LEVEL 2: Full-height config desk, category selector hidden ── */
                  <div
                    className={cn(
                      "flex-1 w-full flex flex-col min-h-0 rounded-xl relative animate-fade-in",
                      layoutMode === "small" ? "h-auto" : "h-full"
                    )}
                  >
                    <LlmConfigDesk
                      activeCategory={activeCategory}
                      activePill={drillDownProvider || draftPill}
                      isModular={isModular}
                      onBack={handleBackFromLevel2}
                      layoutMode={layoutMode}
                    />
                  </div>
                ) : (
                  /* ── LEVEL 1: Category carousel + provider pills ── */
                  <div
                    className={cn(
                      "flex-1 w-full flex flex-col min-h-0 rounded-xl p-2.5 sm:p-3 relative border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] justify-between mt-0.5",
                      layoutMode === "small" ? "h-auto" : "h-full"
                    )}
                  >
                    <CategorySelector
                      activeCategory={activeCategory}
                      onSetCategory={handleSetCategory}
                      layoutMode={layoutMode}
                    />
                    <ProviderSelectorView
                      activeCategory={activeCategory}
                      activePill={savedPill}
                      onSelectProvider={handleSelectProvider}
                      layoutMode={layoutMode}
                    />
                  </div>
                )
              ) : (
                <div
                  className={cn(
                    "flex-1 w-full flex flex-col min-h-0 rounded-xl p-3 relative border border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.02)]",
                    layoutMode === "small" ? "h-auto" : "h-full"
                  )}
                >
                  <RealtimeConfigDesk layoutMode={layoutMode} />
                </div>
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
                  title={INTERACTION_CARD_COPY.voiceTyping}
                  active={dictationEnabled}
                  activeLabel={INTERACTION_CARD_COPY.toggleEnabled}
                  inactiveLabel={INTERACTION_CARD_COPY.toggleDisabled}
                  activeSublabel={DICTATION_COPY.voiceTypingActive}
                  inactiveSublabel={DICTATION_COPY.voiceTypingInactive}
                  icon={dictationEnabled ? Mic : MicOff}
                  onToggle={() => updateDraft("dictation", "enabled", !dictationEnabled)}
                  layoutMode={layoutMode}
                />

                <ToggleTile
                  title={INTERACTION_CARD_COPY.triggerMode}
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
