import { useState, useEffect, useRef, memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { SlidersHorizontal, Mic, MicOff, Activity, Radio, Layers, Zap, AudioLines, BrainCircuit, ChevronRight, Ear } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { SegmentedControl, ToggleTile, Tooltip } from "@/shared/ui";
import { InteractionConfigDesk } from "./InteractionConfigDesk";
import { RealtimeConfigDesk } from "./RealtimeConfigDesk";
import { DictationConfigDesk } from "./DictationConfigDesk";
import { DICTATION_COPY, INTERACTION_CARD_COPY, TRIGGER_MODE_COPY, PIPELINE_MODE_COPY, PIPELINE_PROVIDERS_DESK_COPY } from "@/data/settingsCopy";

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
    const updateDraft = useSettingsStore((s) => s.updateDraft);
    const discardCategoryChanges = useSettingsStore((s) => s.discardCategoryChanges);

    const [activeView, setActiveView] = useState<"assistant" | "dictation">("assistant");
    const [activeCategory, setActiveCategory] = useState<"STT" | "LLM" | "TTS">("LLM");
    const [isDrillDownOpen, setIsDrillDownOpen] = useState(false);

    const prevCategoryRef = useRef<string>(activeCategory);

    // Auto-discard unsaved draft changes when leaving Level 2 back to Level 1
    const handleBackFromLevel2 = useCallback(() => {
      discardCategoryChanges(activeCategory.toLowerCase());
      setIsDrillDownOpen(false);
    }, [activeCategory, discardCategoryChanges]);

    const handleSelectCategory = useCallback((cat: "STT" | "LLM" | "TTS") => {
      setActiveCategory(cat);
      setIsDrillDownOpen(true);
    }, []);


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
            if (isDrillDownOpen) {
              discardCategoryChanges(activeCategory.toLowerCase());
            }
            prevCategoryRef.current = upperCat;
            setActiveCategory(upperCat);
          }
        }
      };
      window.addEventListener("sync_interaction_category", handleSync);
      return () => window.removeEventListener("sync_interaction_category", handleSync);
    }, [activeCategory, discardCategoryChanges, isDrillDownOpen]);

    if (!draftSettings || !settings) return null;
    const { interaction, dictation } = draftSettings;

    const dictationEnabled = dictation?.enabled ?? true;
    const dictationInteractionMode = dictation?.interaction_mode ?? "ptt";
    const interactionMode = interaction.mode ?? "passive";
    const isPassive = String(interactionMode).toLowerCase() === "passive";
    const pipelineMode = interaction.pipeline_mode ?? "modular";
    const isModular = pipelineMode === "modular";
    const pipelineCompact = layoutMode === "small";
    const pipelineIconSize = pipelineCompact ? 16 : 26;
    const pipelineChevronSize = pipelineCompact ? 12 : 14;
    const pipelineTextSize = pipelineCompact ? "text-[11px]" : "text-[14px]";
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
              <div className="grid grid-cols-2 gap-2 shrink-0">
              <ToggleTile
                title={TRIGGER_MODE_COPY.cardTitle}
                active={isPassive}
                activeLabel={TRIGGER_MODE_COPY.continuousTitle}
                inactiveLabel={TRIGGER_MODE_COPY.pttTitle}
                activeSublabel={TRIGGER_MODE_COPY.continuousSub}
                inactiveSublabel={TRIGGER_MODE_COPY.pttSub}
                icon={isPassive ? Activity : Radio}
                onToggle={() => updateDraft("interaction", "mode", isPassive ? "ptt" : "passive")}
                layoutMode={layoutMode}
                visualizer={
                  isPassive ? (
                    <div className="flex items-end gap-[1.5px] h-3">
                      <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-1" />
                      <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-2" />
                      <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-3" />
                      <span className="w-[2px] bg-[rgb(var(--accent))] rounded-full animate-wave-bar-4" />
                    </div>
                  ) : (
                    <div className="w-3 h-3 rounded-full border border-[rgb(var(--accent))]/40 flex items-center justify-center relative">
                      <span className="absolute inset-0 rounded-full border border-[rgb(var(--accent))] animate-ping opacity-60" />
                      <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
                    </div>
                  )
                }
              />
              <ToggleTile
                title={PIPELINE_MODE_COPY.cardTitle}
                active={isModular}
                activeLabel={PIPELINE_MODE_COPY.modularTitle}
                inactiveLabel={PIPELINE_MODE_COPY.realtimeTitle}
                activeSublabel={PIPELINE_MODE_COPY.modularSub}
                inactiveSublabel={PIPELINE_MODE_COPY.realtimeSub}
                icon={isModular ? Layers : Zap}
                onToggle={() => updateDraft("interaction", "pipeline_mode", isModular ? "realtime" : "modular")}
                layoutMode={layoutMode}
                visualizer={
                  isModular ? (
                    <div className="flex flex-col gap-[1.5px] items-center">
                      <span className="w-3.5 h-[2px] bg-[rgb(var(--accent))] rounded animate-pulse" />
                      <span className="w-2.5 h-[2px] bg-[rgb(var(--accent))]/60 rounded animate-pulse" />
                      <span className="w-3.5 h-[2px] bg-[rgb(var(--accent))] rounded animate-pulse" />
                    </div>
                  ) : (
                    <div className="w-7 h-2 bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--border),0.06)] rounded-full relative overflow-hidden flex items-center">
                      <span className="absolute w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] animate-flow-dot shadow-[0_0_6px_rgba(var(--accent),0.8)]" />
                    </div>
                  )
                }
              />
            </div>

              {/* Category & Provider Selector Subcomponent or Realtime Config Desk */}
              {isModular ? (
                isDrillDownOpen ? (
                  /* ── LEVEL 2: Unified Category Provider Desk ── */
                  <div
                    className={cn(
                      "flex-1 w-full flex flex-col min-h-0 rounded-xl relative animate-fade-in",
                      layoutMode === "small" ? "h-auto" : "h-full"
                    )}
                  >
                    <InteractionConfigDesk
                      activeCategory={activeCategory}
                      isModular={isModular}
                      onBack={handleBackFromLevel2}
                      layoutMode={layoutMode}
                    />
                  </div>
                ) : (
                  /* ── LEVEL 1: Unified Pipeline Stage Cards (STT | LLM | TTS) ── */
                  <div
                    className={cn(
                      "flex-1 w-full flex flex-col min-h-0 rounded-xl relative animate-fade-in mt-0.5",
                      layoutMode === "small" ? "h-auto" : "h-full"
                    )}
                  >
                    <div className="grid grid-cols-3 gap-2 w-full flex-1 min-h-0 select-none">
                      {[
                        { id: "STT" as const, card: PIPELINE_PROVIDERS_DESK_COPY.cards.stt, Icon: Ear, active: draftSettings.stt?.active },
                        { id: "LLM" as const, card: PIPELINE_PROVIDERS_DESK_COPY.cards.llm, Icon: BrainCircuit, active: draftSettings.llm?.active },
                        { id: "TTS" as const, card: PIPELINE_PROVIDERS_DESK_COPY.cards.tts, Icon: AudioLines, active: draftSettings.tts?.active },
                      ].map((stage) => {
                        const IconComponent = stage.Icon;
                        const category = stage.active?.category ?? "";
                        const model = stage.active?.model ?? "";

                        return (
                          <Tooltip
                            key={stage.id}
                            label={`${PIPELINE_PROVIDERS_DESK_COPY.configureAriaPrefix} ${stage.card.title}`}
                            side="top"
                            asChild
                          >
                            <button
                              type="button"
                              onClick={() => handleSelectCategory(stage.id)}
                              aria-label={`${PIPELINE_PROVIDERS_DESK_COPY.configureAriaPrefix} ${stage.card.title}`}
                              className={cn(
                                "group relative h-full w-full min-h-0 flex flex-col items-center justify-center gap-1.5 overflow-hidden text-center cursor-pointer rounded-xl border transition-colors duration-200",
                                pipelineCompact ? "px-4 py-3" : "px-5 py-3.5",
                                "border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.015)]",
                                "hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--accent),0.03)]",
                                "focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-[rgb(var(--accent))]"
                              )}
                            >
                              <ChevronRight
                                size={pipelineChevronSize}
                                strokeWidth={2.2}
                                className="absolute top-2 right-2 text-[rgb(var(--foreground-muted))]/35 transition-all duration-200 group-hover:translate-x-0.5 group-hover:text-[rgb(var(--accent))]"
                              />

                              <IconComponent size={pipelineIconSize} strokeWidth={2} className="shrink-0 text-[rgb(var(--accent))]" />

                              <span className="flex flex-col items-center gap-0 max-w-full">
                                {category && (
                                  <span className={cn("max-w-full leading-tight truncate", pipelineTextSize, "text-[rgb(var(--foreground))]/85")}>
                                    {category}
                                  </span>
                                )}
                                {model && (
                                  <span className={cn("max-w-full leading-tight truncate", pipelineTextSize, "text-[rgb(var(--foreground-muted))]/80")}>
                                    {model}
                                  </span>
                                )}
                              </span>
                            </button>
                          </Tooltip>
                        );
                      })}
                    </div>
                  </div>
                )
              ) : (
                <div
                  className={cn(
                    "flex-1 w-full flex flex-col min-h-0 rounded-xl p-3 relative border border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.02)] overflow-hidden",
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
              <div className="grid grid-cols-2 gap-2 shrink-0">
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
              <div
                className={cn(
                  "flex-1 w-full flex flex-col min-h-0 rounded-xl p-2.5 sm:p-3 relative border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] justify-between mt-0.5",
                  layoutMode === "small" ? "h-auto" : "h-full"
                )}
              >
                <DictationConfigDesk layoutMode={layoutMode} disabled={!dictationEnabled} />
              </div>
            </>
          )}
        </div>
      </div>
    );
  }
);

InteractionCard.displayName = "InteractionCard";
